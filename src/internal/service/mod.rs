mod helpers;
mod rpc;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::time::Duration;

pub(super) use agenthub_team_actor::{
    ActorAckRequest, ActorInboxRequest, ActorMailboxService, ActorMessageStatus, ActorSendRequest,
    ActorServiceError, ActorServiceErrorCode, parse_actor_transport,
};
use helpers::*;
pub(super) use serde_json::Value;
pub(super) use sqlx::Row;
pub(super) use tonic::{Request, Response, Status, metadata::MetadataMap};

pub(super) use crate::acp::{AcpActorSkillContext, AcpPermissionRespondResult};
pub(super) use crate::agent::{AgentConfig, AgentTimeTriggerCreateInput, AgentTimeTriggerManager};
pub(super) use crate::state::AppState;
pub(super) use crate::team::{
    TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX, TeamContextLookupError, TeamManager, TeamStepRecord,
    TeamStepStatus, TeamTaskAssignmentUpdate, TeamTaskContextPatch, TeamTaskListQuery,
    TeamTaskStatus, build_actor_mailbox_immediate_hint_prompt, plan_actor_mailbox_immediate_hint,
    resolve_team_permission_review_target,
};

pub(super) use super::auth::{InternalAction, InternalAuthz, InternalRole};
pub(super) use super::p2p::{CredentialProvider, NodeCredentialRequest};
pub(super) use super::proto::agenthub::internal::v1::team_internal_control_server::TeamInternalControl;
pub(super) use super::proto::agenthub::internal::v1::{
    AckActorMessageRequest, AckActorMessageResponse, ActorMessage, AgentEventRecord,
    AppendTeamTaskNoteRequest, AppendTeamTaskNoteResponse, CancelTimeTriggerRequest,
    CancelTimeTriggerResponse, CreateTeamTaskRequest, CreateTeamTaskResponse,
    CreateTimeTriggerRequest, CreateTimeTriggerResponse, DeleteManagedAgentRequest,
    DeleteManagedAgentResponse, DescribeTeamContextRequest, DescribeTeamContextResponse,
    EnsureAgentRecordRequest, EnsureAgentRecordResponse, GetAgentRecordRequest,
    GetAgentRecordResponse, GetTeamTaskRequest, GetTeamTaskResponse, IssueNodeCredentialRequest,
    IssueNodeCredentialResponse, ListActorInboxRequest, ListActorInboxResponse,
    ListAgentEventsRequest, ListAgentEventsResponse, ListTeamTasksRequest, ListTeamTasksResponse,
    ListTimeTriggersRequest, ListTimeTriggersResponse, RespondPermissionReviewRequest,
    RespondPermissionReviewResponse, SendActorMessageRequest, SendActorMessageResponse,
    SendAgentInputRequest, SendAgentInputResponse, StartManagedAgentRequest,
    StartManagedAgentResponse, StopManagedAgentRequest, StopManagedAgentResponse,
    TransitionStepRequest, TransitionStepResponse, UpdateTeamTaskRequest, UpdateTeamTaskResponse,
};
pub(super) use super::tls::{InternalGrpcSecurityMode, load_bootstrap_client_identity};

const BOOTSTRAP_TOKEN_HEADER: &str = "x-agenthub-bootstrap-token";
const MAILBOX_HINT_NOTIFY_TIMEOUT: Duration = Duration::from_millis(300);
const MAX_INTERNAL_TEAM_TASK_LIST_LIMIT: i64 = 500;
const DEFAULT_TOKEN_TTL_SECONDS: i64 = 3600;
const MAX_TOKEN_TTL_SECONDS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone)]
pub(super) struct ChannelReplicaRequest {
    authority_message_id: i64,
    team_id: String,
    conversation_id: String,
    task_id: String,
    channel_id: String,
}

#[derive(Clone)]
pub struct TeamInternalControlService {
    state: AppState,
    authz: InternalAuthz,
    security_mode: InternalGrpcSecurityMode,
    cert_dir: PathBuf,
    bootstrap_token: String,
}

impl TeamInternalControlService {
    pub fn new(
        state: AppState,
        authz: InternalAuthz,
        security_mode: InternalGrpcSecurityMode,
        cert_dir: PathBuf,
        bootstrap_token: String,
    ) -> Self {
        Self {
            state,
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
        if provided != self.bootstrap_token {
            return Err(Status::permission_denied("bootstrap token mismatch"));
        }
        Ok(())
    }

    async fn validate_channel_replica_request(
        &self,
        run_id: &str,
        replica: &ChannelReplicaRequest,
    ) -> Result<(), Status> {
        let row = sqlx::query(
            r#"
            SELECT
                tr.team_id AS run_team_id,
                tt.team_id AS task_team_id,
                tc.task_id AS conversation_task_id,
                tc.mode AS conversation_mode
            FROM team_runs tr
            LEFT JOIN team_tasks tt ON tt.id = ?2
            LEFT JOIN team_conversations tc ON tc.id = ?3
            WHERE tr.id = ?1
            "#,
        )
        .bind(run_id)
        .bind(&replica.task_id)
        .bind(&replica.conversation_id)
        .fetch_optional(&self.state.db)
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

        if replica.team_id != run_team_id
            || task_team_id.as_deref() != Some(replica.team_id.as_str())
            || conversation_task_id.as_deref() != Some(replica.task_id.as_str())
            || conversation_mode.trim() != "group_chat"
        {
            return Err(Status::invalid_argument(
                "channel replica payload does not match run/team context",
            ));
        }
        Ok(())
    }
}
