mod helpers;
mod rpc;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub(super) use agenthub_team_actor::{
    ActorAckRequest, ActorInboxRequest, ActorMailboxService, ActorMessageStatus, ActorSendRequest,
    ActorServiceError, ActorServiceErrorCode, ActorTaskLinkRequest, ActorTriageRequest,
    parse_actor_transport,
};
use helpers::*;
pub(super) use serde_json::Value;
pub(super) use sqlx::Row;
pub(super) use tonic::{Request, Response, Status, metadata::MetadataMap};

pub(super) use crate::acp::{
    AcpActorSkillContext, AcpPermissionRespondResult, AcpPermissionService,
};
pub(super) use crate::agent::{
    AgentConfig, AgentManager, AgentTimeTriggerCreateInput, AgentTimeTriggerManager,
};
pub(super) use crate::team::{
    TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX, TeamContextLookupError, TeamManager, TeamStepRecord,
    TeamStepStatus, TeamTaskAssignmentUpdate, TeamTaskContextPatch, TeamTaskCreateInput,
    TeamTaskListQuery, TeamTaskNoteCreateInput, TeamTaskNoteKind, TeamTaskPriority, TeamTaskStatus,
    TeamTaskUpdateWithNoteInput, dispatch_actor_mailbox_immediate_hint,
    plan_actor_mailbox_immediate_hint,
};

pub(super) use super::auth::{InternalAction, InternalAuthz, InternalRole};
pub(super) use super::p2p::{CredentialProvider, NodeCredentialRequest};
pub(super) use super::proto::agenthub::internal::v1::team_internal_control_server::TeamInternalControl;
pub(super) use super::proto::agenthub::internal::v1::{
    AckActorMessageRequest, AckActorMessageResponse, ActorMessage, AgentEventRecord,
    AppendTeamTaskNoteRequest, AppendTeamTaskNoteResponse, CancelTimeTriggerRequest,
    CancelTimeTriggerResponse, CreateTeamChannelRequest, CreateTeamChannelResponse,
    CreateTeamTaskRequest, CreateTeamTaskResponse, CreateTimeTriggerRequest,
    CreateTimeTriggerResponse, DeleteManagedAgentRequest, DeleteManagedAgentResponse,
    DeleteTeamChannelRequest, DeleteTeamChannelResponse, DescribeTeamContextRequest,
    DescribeTeamContextResponse, EnsureAgentRecordRequest, EnsureAgentRecordResponse,
    GetAgentRecordRequest, GetAgentRecordResponse, GetTeamTaskRequest, GetTeamTaskResponse,
    IssueNodeCredentialRequest, IssueNodeCredentialResponse, LinkActorMessageTaskRequest,
    LinkActorMessageTaskResponse, ListActorInboxRequest, ListActorInboxResponse,
    ListAgentEventsRequest, ListAgentEventsResponse, ListTeamTasksRequest, ListTeamTasksResponse,
    ListTimeTriggersRequest, ListTimeTriggersResponse, OpenTeamThreadRequest,
    OpenTeamThreadResponse, ReplyTeamThreadRequest, ReplyTeamThreadResponse,
    ResolveActorRunScopeRequest, ResolveActorRunScopeResponse, RespondPermissionReviewRequest,
    RespondPermissionReviewResponse, SendActorMessageRequest, SendActorMessageResponse,
    SendAgentInputRequest, SendAgentInputResponse, StartManagedAgentRequest,
    StartManagedAgentResponse, StopManagedAgentRequest, StopManagedAgentResponse,
    TransitionStepRequest, TransitionStepResponse, TriageActorMessageRequest,
    TriageActorMessageResponse, UpdateTeamTaskRequest, UpdateTeamTaskResponse,
};
pub(super) use super::tls::{InternalGrpcSecurityMode, load_bootstrap_client_identity};

const BOOTSTRAP_TOKEN_HEADER: &str = "x-agenthub-bootstrap-token";
const MAILBOX_HINT_NOTIFY_TIMEOUT: Duration = Duration::from_millis(300);
const MAX_INTERNAL_TEAM_TASK_LIST_LIMIT: i64 = 500;
const DEFAULT_TOKEN_TTL_SECONDS: i64 = 3600;
const MAX_TOKEN_TTL_SECONDS: i64 = 24 * 60 * 60;

/// Byte-for-byte equality that doesn't short-circuit on the first mismatching byte, so comparing a
/// bootstrap token candidate against the real secret doesn't leak how many leading bytes matched
/// through response timing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |diff, (x, y)| diff | (x ^ y))
        == 0
}

#[derive(Clone)]
pub(crate) struct TeamInternalControlDeps {
    pub db: sqlx::SqlitePool,
    pub agents: Arc<AgentManager>,
    pub teams: Arc<TeamManager>,
    pub acp_permissions: Arc<AcpPermissionService>,
}

impl TeamInternalControlDeps {
    pub fn new(
        db: sqlx::SqlitePool,
        agents: Arc<AgentManager>,
        teams: Arc<TeamManager>,
        acp_permissions: Arc<AcpPermissionService>,
    ) -> Self {
        Self {
            db,
            agents,
            teams,
            acp_permissions,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ChannelReplicaRequest {
    authority_message_id: i64,
    correlation_id: String,
    team_id: String,
    conversation_id: String,
    task_id: String,
    channel_id: String,
}

#[derive(Clone)]
pub(crate) struct TeamInternalControlService {
    deps: TeamInternalControlDeps,
    authz: InternalAuthz,
    security_mode: InternalGrpcSecurityMode,
    cert_dir: PathBuf,
    bootstrap_token: String,
}

impl TeamInternalControlService {
    pub fn new(
        deps: TeamInternalControlDeps,
        authz: InternalAuthz,
        security_mode: InternalGrpcSecurityMode,
        cert_dir: PathBuf,
        bootstrap_token: String,
    ) -> Self {
        Self {
            deps,
            authz,
            security_mode,
            cert_dir,
            bootstrap_token,
        }
    }

    fn ensure_bootstrap_token(&self, metadata: &MetadataMap) -> Result<(), Status> {
        let provided = metadata
            .get(BOOTSTRAP_TOKEN_HEADER)
            .ok_or_else(|| Status::unauthenticated("missing bootstrap token"))?
            .to_str()
            .map_err(|_| Status::unauthenticated("invalid bootstrap token metadata"))?
            .trim();
        if provided.is_empty() {
            return Err(Status::unauthenticated("empty bootstrap token"));
        }
        if !constant_time_eq(provided.as_bytes(), self.bootstrap_token.as_bytes()) {
            return Err(Status::permission_denied("bootstrap token mismatch"));
        }
        Ok(())
    }

    async fn validate_channel_replica_request(
        &self,
        run_id: &str,
        from_actor_id: &str,
        replica: &ChannelReplicaRequest,
    ) -> Result<(), Status> {
        let row = sqlx::query(
            r#"
            SELECT
                tr.team_id AS run_team_id,
                tt.team_id AS task_team_id,
                tc.task_id AS conversation_task_id,
                tc.mode AS conversation_mode,
                tcm.conversation_id AS authority_conversation_id,
                tcm.task_id AS authority_task_id,
                tcm.from_actor_id AS authority_from_actor_id,
                tcm.payload_json AS authority_payload_json
            FROM team_runs tr
            LEFT JOIN team_tasks tt ON tt.id = ?2
            LEFT JOIN team_conversations tc ON tc.id = ?3
            LEFT JOIN team_conversation_messages tcm ON tcm.id = ?4
            WHERE tr.id = ?1
            "#,
        )
        .bind(run_id)
        .bind(&replica.task_id)
        .bind(&replica.conversation_id)
        .bind(replica.authority_message_id)
        .fetch_optional(&self.deps.db)
        .await
        .map_err(|err| map_manager_error(err.into()))?
        .ok_or_else(|| Status::not_found("run not found"))?;

        let run_team_id = row.get::<String, _>("run_team_id");
        let task_team_id = row
            .try_get::<Option<String>, _>("task_team_id")
            .ok()
            .flatten();
        let conversation_task_id = row
            .try_get::<Option<String>, _>("conversation_task_id")
            .ok()
            .flatten();
        let conversation_mode = row
            .try_get::<Option<String>, _>("conversation_mode")
            .ok()
            .flatten()
            .unwrap_or_default();
        let authority_conversation_id = row
            .try_get::<Option<String>, _>("authority_conversation_id")
            .ok()
            .flatten();
        let authority_task_id = row
            .try_get::<Option<String>, _>("authority_task_id")
            .ok()
            .flatten();
        let authority_from_actor_id = row
            .try_get::<Option<String>, _>("authority_from_actor_id")
            .ok()
            .flatten();
        let authority_payload_json = row
            .try_get::<Option<String>, _>("authority_payload_json")
            .ok()
            .flatten();

        if replica.team_id != run_team_id
            || task_team_id.as_deref() != Some(replica.team_id.as_str())
            || conversation_task_id.as_deref() != Some(replica.task_id.as_str())
            || conversation_mode.trim() != "group_chat"
        {
            return Err(Status::invalid_argument(
                "channel replica payload does not match run/team context",
            ));
        }
        if authority_conversation_id.as_deref() != Some(replica.conversation_id.as_str())
            || authority_task_id.as_deref() != Some(replica.task_id.as_str())
        {
            return Err(Status::invalid_argument(
                "channel replica payload authority_message_id does not match canonical conversation context",
            ));
        }
        if authority_from_actor_id.as_deref() != Some(from_actor_id) {
            return Err(Status::invalid_argument(
                "channel replica payload sender does not match canonical authority message",
            ));
        }
        let authority_payload_json = authority_payload_json.ok_or_else(|| {
            Status::invalid_argument(
                "channel replica payload authority_message_id does not match canonical conversation context",
            )
        })?;
        let authority_payload: Value =
            serde_json::from_str(&authority_payload_json).map_err(|_| {
                Status::failed_precondition(
                    "canonical authority message payload is not valid JSON for replica validation",
                )
            })?;
        let authority_correlation_id = authority_payload
            .get("correlation_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        if authority_correlation_id != replica.correlation_id {
            return Err(Status::invalid_argument(
                "channel replica payload correlation_id does not match canonical authority message",
            ));
        }
        Ok(())
    }
}
