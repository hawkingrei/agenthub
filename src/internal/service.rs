use std::path::PathBuf;
use std::time::Duration;

use agenthub_team_actor::{
    ActorAckRequest, ActorInboxRequest, ActorMailboxService, ActorMessageStatus, ActorSendRequest,
    ActorServiceError, ActorServiceErrorCode, parse_actor_transport,
};
use serde_json::Value;
use sqlx::Row;
use tonic::{Request, Response, Status, metadata::MetadataMap};

use crate::acp::AcpActorSkillContext;
use crate::acp::AcpPermissionRespondResult;
use crate::agent::{AgentConfig, AgentTimeTriggerCreateInput, AgentTimeTriggerManager};
use crate::state::AppState;
use crate::team::{
    TeamContextLookupError, TeamManager, TeamStepRecord, TeamStepStatus, TeamTaskRecord,
    TeamTaskStatus, build_actor_mailbox_immediate_hint_prompt, plan_actor_mailbox_immediate_hint,
    resolve_team_permission_review_target,
};

use super::auth::{InternalAction, InternalAuthz, InternalRole};
use super::p2p::{CredentialProvider, NodeCredentialRequest};
use super::proto::agenthub::internal::v1::team_internal_control_server::TeamInternalControl;
use super::proto::agenthub::internal::v1::{
    AckActorMessageRequest, AckActorMessageResponse, ActorMessage, AgentEventRecord,
    CancelTimeTriggerRequest, CancelTimeTriggerResponse, CreateTeamTaskRequest,
    CreateTeamTaskResponse, CreateTimeTriggerRequest, CreateTimeTriggerResponse,
    DeleteManagedAgentRequest, DeleteManagedAgentResponse, DescribeTeamContextRequest,
    DescribeTeamContextResponse, EnsureAgentRecordRequest, EnsureAgentRecordResponse,
    GetAgentRecordRequest, GetAgentRecordResponse, IssueNodeCredentialRequest,
    IssueNodeCredentialResponse, ListActorInboxRequest, ListActorInboxResponse,
    ListAgentEventsRequest, ListAgentEventsResponse, ListTeamTasksRequest, ListTeamTasksResponse,
    ListTimeTriggersRequest, ListTimeTriggersResponse, RespondPermissionReviewRequest,
    RespondPermissionReviewResponse, SendActorMessageRequest, SendActorMessageResponse,
    SendAgentInputRequest, SendAgentInputResponse, StartManagedAgentRequest,
    StartManagedAgentResponse, StopManagedAgentRequest, StopManagedAgentResponse,
    TransitionStepRequest, TransitionStepResponse, UpdateTeamTaskRequest, UpdateTeamTaskResponse,
};
use super::tls::{InternalGrpcSecurityMode, load_bootstrap_client_identity};

const BOOTSTRAP_TOKEN_HEADER: &str = "x-agenthub-bootstrap-token";
const MAILBOX_HINT_NOTIFY_TIMEOUT: Duration = Duration::from_millis(300);
const MAX_INTERNAL_TEAM_TASK_LIST_LIMIT: i64 = 500;
const TEAM_SHARED_THREAD_TITLE: &str = "all";
const TEAM_SHARED_THREAD_BOOTSTRAP_KIND: &str = "shared_thread";
const DEFAULT_TOKEN_TTL_SECONDS: i64 = 3600;
const MAX_TOKEN_TTL_SECONDS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone)]
struct ChannelReplicaRequest {
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

#[tonic::async_trait]
impl TeamInternalControl for TeamInternalControlService {
    async fn send_actor_message(
        &self,
        request: Request<SendActorMessageRequest>,
    ) -> Result<Response<SendActorMessageResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::MessageSend)?;
        let payload = request.into_inner();

        let run_id = required_field(&payload.run_id, "run_id")?;
        self.authz.ensure_run_scope(&principal, run_id)?;

        let from_actor_id = required_field(&payload.from_actor_id, "from_actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, from_actor_id, "from_actor_id")?;

        let to_actor_id = optional_trimmed(&payload.to_actor_id);
        let channel_id = optional_trimmed(&payload.channel_id);
        let channel = optional_trimmed(&payload.channel).unwrap_or("default");
        let transport = parse_actor_transport(optional_trimmed(&payload.transport))
            .map_err(|err| Status::invalid_argument(err.to_string()))?;
        let route = optional_json_object(optional_trimmed(&payload.route_json), "route_json")?;
        let payload_json = parse_json_required(&payload.payload_json, "payload_json")?;
        let channel_replica = resolve_channel_replica_request(&payload_json);
        let idempotency_key = optional_trimmed(&payload.idempotency_key);
        let from_peer_id = optional_trimmed(&payload.from_peer_id);
        let to_peer_id = optional_trimmed(&payload.to_peer_id);

        if let Some(replica) = channel_replica.as_ref() {
            self.validate_channel_replica_request(run_id, replica)
                .await?;
        }

        let message = self
            .state
            .teams
            .actor_mailbox_service()
            .actor_send(ActorSendRequest {
                run_id: run_id.to_string(),
                from_actor_id: from_actor_id.to_string(),
                from_peer_id: from_peer_id.map(str::to_string),
                to_actor_id: to_actor_id.map(str::to_string),
                channel_id: channel_id.map(str::to_string),
                to_peer_id: to_peer_id.map(str::to_string),
                channel: Some(channel.to_string()),
                transport: Some(transport),
                route,
                payload: payload_json.clone(),
                idempotency_key: idempotency_key.map(str::to_string),
            })
            .await
            .map_err(map_actor_service_status)?;

        if let (Some(replica), Some(source_node_id)) =
            (channel_replica, principal.source_node_id.as_deref())
        {
            self.state
                .teams
                .append_channel_replica_message(
                    replica.authority_message_id,
                    run_id,
                    &replica.team_id,
                    &replica.conversation_id,
                    &replica.task_id,
                    &replica.channel_id,
                    from_actor_id,
                    source_node_id,
                    &payload_json,
                )
                .await
                .map_err(map_manager_error)?;
        }

        match tokio::time::timeout(
            MAILBOX_HINT_NOTIFY_TIMEOUT,
            maybe_notify_actor_new_mailbox_message_type(&self.state, run_id, &message),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                tracing::warn!(
                    run_id = %run_id,
                    to_actor_id = %message.message.to_actor_id,
                    message_id = message.message_id,
                    "team mailbox type hint notify failed: {}",
                    err
                );
            }
            Err(_) => {
                tracing::warn!(
                    run_id = %run_id,
                    to_actor_id = %message.message.to_actor_id,
                    message_id = message.message_id,
                    timeout_ms = MAILBOX_HINT_NOTIFY_TIMEOUT.as_millis(),
                    "team mailbox type hint notify timed out"
                );
            }
        }

        Ok(Response::new(SendActorMessageResponse {
            message_id: message.message_id,
            status: message.state.as_str().to_string(),
            idempotency_key: idempotency_key.unwrap_or("").to_string(),
            message_json: serde_json::to_string(&message.message).unwrap_or_default(),
        }))
    }

    async fn list_actor_inbox(
        &self,
        request: Request<ListActorInboxRequest>,
    ) -> Result<Response<ListActorInboxResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::InboxList)?;
        let payload = request.into_inner();

        let run_id = required_field(&payload.run_id, "run_id")?;
        self.authz.ensure_run_scope(&principal, run_id)?;

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;

        let limit = payload.limit.clamp(1, 1000);
        let after_id = if payload.after_message_id > 0 {
            Some(payload.after_message_id)
        } else {
            None
        };

        let states = if payload.include_delivered {
            Some(vec![
                ActorMessageStatus::Pending,
                ActorMessageStatus::Delivered,
            ])
        } else {
            None
        };
        let response = self
            .state
            .teams
            .actor_mailbox_service()
            .actor_inbox(ActorInboxRequest {
                run_id: run_id.to_string(),
                actor_id: actor_id.to_string(),
                cursor: after_id,
                limit: Some(limit),
                states,
            })
            .await
            .map_err(map_actor_service_status)?;
        let messages = response
            .messages
            .into_iter()
            .map(|message| ActorMessage {
                message_id: message.message_id,
                run_id: message.run_id,
                from_actor_id: message.from_actor_id,
                to_actor_id: message.to_actor_id,
                channel: message.channel,
                transport: message.transport.as_str().to_string(),
                route_json: message
                    .route
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .ok()
                    .flatten()
                    .unwrap_or_default(),
                payload_json: serde_json::to_string(&message.payload).unwrap_or_default(),
                status: message.status.as_str().to_string(),
                created_at: message.created_at,
                delivered_at: message.delivered_at.unwrap_or_default(),
                idempotency_key: String::new(),
                from_peer_id: message.from_peer_id,
                to_peer_id: message.to_peer_id,
            })
            .collect::<Vec<_>>();

        Ok(Response::new(ListActorInboxResponse {
            messages,
            pending_count: response.pending_count,
        }))
    }

    async fn ack_actor_message(
        &self,
        request: Request<AckActorMessageRequest>,
    ) -> Result<Response<AckActorMessageResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::MessageAck)?;
        let payload = request.into_inner();

        let run_id = required_field(&payload.run_id, "run_id")?;
        self.authz.ensure_run_scope(&principal, run_id)?;

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;

        if payload.message_id <= 0 {
            return Err(Status::invalid_argument("message_id must be positive"));
        }
        let message = self
            .state
            .teams
            .actor_mailbox_service()
            .actor_ack(ActorAckRequest {
                run_id: run_id.to_string(),
                actor_id: actor_id.to_string(),
                message_id: payload.message_id,
                ack_token: None,
                result: None,
            })
            .await
            .map_err(map_actor_service_status)?;
        let acked_message = message.message;
        let route_json = acked_message
            .route
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .ok()
            .flatten()
            .unwrap_or_default();
        let payload_json = serde_json::to_string(&acked_message.payload).unwrap_or_default();
        let status = acked_message.status.as_str().to_string();
        let delivered_at = acked_message.delivered_at.unwrap_or(message.acked_at);
        let created_at = acked_message.created_at;
        let message_id = acked_message.message_id;
        let run_id = acked_message.run_id;
        let from_actor_id = acked_message.from_actor_id;
        let to_actor_id = acked_message.to_actor_id;
        let from_peer_id = acked_message.from_peer_id;
        let to_peer_id = acked_message.to_peer_id;
        let channel = acked_message.channel;
        let transport = acked_message.transport.as_str().to_string();

        Ok(Response::new(AckActorMessageResponse {
            message: Some(ActorMessage {
                message_id,
                run_id,
                from_actor_id,
                to_actor_id,
                channel,
                transport,
                route_json,
                payload_json,
                status,
                created_at,
                delivered_at,
                idempotency_key: String::new(),
                from_peer_id,
                to_peer_id,
            }),
        }))
    }

    async fn describe_team_context(
        &self,
        request: Request<DescribeTeamContextRequest>,
    ) -> Result<Response<DescribeTeamContextResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamRead)?;
        let payload = request.into_inner();

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;

        let team_id = optional_trimmed(&payload.team_id);
        let mut run_id = optional_trimmed(&payload.run_id);
        if let Some(principal_run_id) = principal.run_id.as_deref() {
            if let Some(request_run_id) = run_id {
                self.authz.ensure_run_scope(&principal, request_run_id)?;
            } else {
                run_id = Some(principal_run_id);
            }
        } else if let Some(request_run_id) = run_id {
            self.authz.ensure_run_scope(&principal, request_run_id)?;
        }

        let context =
            load_team_context_for_actor(&self.state.teams, team_id, run_id, actor_id).await?;
        Ok(Response::new(DescribeTeamContextResponse {
            context_json: serde_json::to_string(&context).map_err(map_serde_status)?,
        }))
    }

    async fn list_team_tasks(
        &self,
        request: Request<ListTeamTasksRequest>,
    ) -> Result<Response<ListTeamTasksResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamRead)?;
        let payload = request.into_inner();

        let team_id = required_field(&payload.team_id, "team_id")?;
        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        ensure_team_member_access(&self.state.teams, team_id, actor_id).await?;

        let mut tasks = self
            .state
            .teams
            .list_tasks(
                team_id,
                payload.limit.clamp(1, MAX_INTERNAL_TEAM_TASK_LIST_LIMIT),
            )
            .await
            .map_err(map_manager_error)?;
        if !payload.include_shared_thread {
            tasks.retain(|task| !is_shared_thread_task(task));
        }
        if let Some(status) = optional_trimmed(&payload.status) {
            let status = parse_team_task_status(status)?;
            tasks.retain(|task| task.status == status);
        }

        Ok(Response::new(ListTeamTasksResponse {
            tasks_json: serde_json::to_string(&tasks).map_err(map_serde_status)?,
        }))
    }

    async fn create_team_task(
        &self,
        request: Request<CreateTeamTaskRequest>,
    ) -> Result<Response<CreateTeamTaskResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamTaskWrite)?;
        let payload = request.into_inner();

        let team_id = required_field(&payload.team_id, "team_id")?;
        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        ensure_leader_team_access(&self.state.teams, team_id, actor_id).await?;

        let title = required_field(&payload.title, "title")?;
        let status = parse_team_task_status(required_field(&payload.status, "status")?)?;
        let topic = optional_trimmed(&payload.topic);
        let context = parse_json_required(&payload.context_json, "context_json")?;

        let (task, conversation) = self
            .state
            .teams
            .create_task(team_id, title, actor_id, context, "group_chat", topic)
            .await
            .map_err(map_manager_error)?;
        let task = if status == TeamTaskStatus::Open {
            task
        } else {
            self.state
                .teams
                .update_task_status(&task.id, status)
                .await
                .map_err(map_manager_error)?
        };
        Ok(Response::new(CreateTeamTaskResponse {
            output_json: serde_json::to_string(&serde_json::json!({
                "task": task,
                "conversation": conversation,
            }))
            .map_err(map_serde_status)?,
        }))
    }

    async fn update_team_task(
        &self,
        request: Request<UpdateTeamTaskRequest>,
    ) -> Result<Response<UpdateTeamTaskResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamTaskWrite)?;
        let payload = request.into_inner();

        let team_id = required_field(&payload.team_id, "team_id")?;
        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        ensure_leader_team_access(&self.state.teams, team_id, actor_id).await?;

        let task_id = required_field(&payload.task_id, "task_id")?;
        let status = parse_team_task_status(required_field(&payload.status, "status")?)?;
        let existing = self
            .state
            .teams
            .get_task(task_id)
            .await
            .map_err(map_manager_error)?;
        if existing.team_id != team_id {
            return Err(Status::permission_denied(
                "task does not belong to this team",
            ));
        }
        let task = self
            .state
            .teams
            .update_task_status(task_id, status)
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(UpdateTeamTaskResponse {
            task_json: serde_json::to_string(&task).map_err(map_serde_status)?,
        }))
    }

    async fn create_time_trigger(
        &self,
        request: Request<CreateTimeTriggerRequest>,
    ) -> Result<Response<CreateTimeTriggerResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TimeTriggerManage)?;
        let payload = request.into_inner();

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        if payload.fire_at <= chrono::Utc::now().timestamp() {
            return Err(Status::invalid_argument("fire_at must be in the future"));
        }
        self.state
            .agents
            .get_agent(actor_id)
            .await
            .map_err(map_agent_lookup_error)?;
        let manager = AgentTimeTriggerManager::new(self.state.db.clone());
        let trigger = manager
            .create_time_trigger(AgentTimeTriggerCreateInput {
                agent_id: actor_id.to_string(),
                created_by_actor_id: actor_id.to_string(),
                message_text: required_field(&payload.message_text, "message_text")?.to_string(),
                fire_at: payload.fire_at,
            })
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(CreateTimeTriggerResponse {
            trigger_json: serde_json::to_string(&trigger).map_err(map_serde_status)?,
        }))
    }

    async fn list_time_triggers(
        &self,
        request: Request<ListTimeTriggersRequest>,
    ) -> Result<Response<ListTimeTriggersResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TimeTriggerManage)?;
        let payload = request.into_inner();

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        let manager = AgentTimeTriggerManager::new(self.state.db.clone());
        let triggers = manager
            .list_triggers_for_agent(actor_id, payload.limit)
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(ListTimeTriggersResponse {
            triggers_json: serde_json::to_string(&triggers).map_err(map_serde_status)?,
        }))
    }

    async fn cancel_time_trigger(
        &self,
        request: Request<CancelTimeTriggerRequest>,
    ) -> Result<Response<CancelTimeTriggerResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TimeTriggerManage)?;
        let payload = request.into_inner();

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        let trigger_id = required_field(&payload.trigger_id, "trigger_id")?;
        let manager = AgentTimeTriggerManager::new(self.state.db.clone());
        let canceled = manager
            .cancel_trigger(actor_id, trigger_id)
            .await
            .map_err(map_manager_error)?;
        if !canceled {
            return Err(Status::not_found("time trigger not found"));
        }
        Ok(Response::new(CancelTimeTriggerResponse {
            output_json: serde_json::to_string(&serde_json::json!({
                "status": "ok",
                "trigger_id": trigger_id,
            }))
            .map_err(map_serde_status)?,
        }))
    }

    async fn respond_permission_review(
        &self,
        request: Request<RespondPermissionReviewRequest>,
    ) -> Result<Response<RespondPermissionReviewResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::PermissionReview)?;
        let payload = request.into_inner();

        let team_id = required_field(&payload.team_id, "team_id")?;
        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;

        let permission_id = required_field(&payload.permission_id, "permission_id")?;
        let Some(record) = self
            .state
            .acp_permissions
            .get(permission_id)
            .await
            .map_err(map_manager_error)?
        else {
            return Err(Status::not_found("permission request not found"));
        };
        if record.team_id.as_deref() != Some(team_id) {
            return Err(Status::permission_denied(
                "permission request does not belong to this team",
            ));
        }
        if !self
            .state
            .teams
            .team_has_member(team_id, actor_id)
            .await
            .map_err(map_manager_error)?
        {
            return Err(Status::permission_denied(
                "current actor is not a member of this team",
            ));
        }
        if record.status != "pending" {
            return Ok(Response::new(RespondPermissionReviewResponse {
                status: "already_resolved".to_string(),
                permission_id: permission_id.to_string(),
                request_status: record.status,
                reviewed_by_actor_id: record.reviewed_by_actor_id.unwrap_or_default(),
            }));
        }
        if record.requester_actor_id.as_deref() == Some(actor_id) {
            return Err(Status::permission_denied(
                "requester cannot review its own permission request",
            ));
        }
        let team = self
            .state
            .teams
            .get_team(team_id)
            .await
            .map_err(map_manager_error)?;
        let active_reviewer =
            if let Some(review_target_actor_id) = record.review_target_actor_id.as_deref() {
                Some(review_target_actor_id.to_string())
            } else {
                let requester_actor_id = record.requester_actor_id.as_deref().ok_or_else(|| {
                    Status::failed_precondition("permission request is missing requester actor")
                })?;
                Some(
                    resolve_team_permission_review_target(
                        &team.spec,
                        requester_actor_id,
                        record.requester_role.as_deref().unwrap_or_default(),
                    )
                    .map(|(reviewer, _)| reviewer)
                    .map_err(|err| {
                        Status::failed_precondition(format!(
                            "failed to resolve active reviewer for permission request: {err}"
                        ))
                    })?,
                )
            };
        if active_reviewer.as_deref() != Some(actor_id) {
            return Err(Status::permission_denied(
                "current actor is not the active reviewer for this permission request",
            ));
        }

        let option_id = optional_trimmed(payload.option_id.as_str()).map(str::to_string);
        let requested_outcome = optional_trimmed(payload.outcome.as_str());
        if option_id.is_some() && requested_outcome.is_some() {
            return Err(Status::invalid_argument(
                "option_id and outcome cannot be set together",
            ));
        }
        let outcome = if let Some(selected_option_id) = option_id.as_ref() {
            agent_client_protocol::RequestPermissionOutcome::Selected(
                agent_client_protocol::SelectedPermissionOutcome::new(selected_option_id.clone()),
            )
        } else {
            match requested_outcome {
                Some("cancelled") | None => {
                    agent_client_protocol::RequestPermissionOutcome::Cancelled
                }
                Some(other) => {
                    return Err(Status::invalid_argument(format!(
                        "unsupported outcome '{other}', expected 'cancelled'"
                    )));
                }
            }
        };

        let respond_result = self
            .state
            .acp_permissions
            .respond(
                permission_id,
                outcome,
                option_id,
                Some(actor_id.to_string()),
            )
            .await
            .map_err(map_manager_error)?;
        let status = match respond_result {
            AcpPermissionRespondResult::Applied => "ok",
            AcpPermissionRespondResult::AlreadyResolved => "already_resolved",
        };
        let request_status = self
            .state
            .acp_permissions
            .get(permission_id)
            .await
            .map_err(map_manager_error)?
            .map(|current| current.status)
            .unwrap_or_else(|| "resolved".to_string());
        Ok(Response::new(RespondPermissionReviewResponse {
            status: status.to_string(),
            permission_id: permission_id.to_string(),
            request_status,
            reviewed_by_actor_id: actor_id.to_string(),
        }))
    }

    async fn transition_step(
        &self,
        request: Request<TransitionStepRequest>,
    ) -> Result<Response<TransitionStepResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::StepTransition)?;
        if principal.role == InternalRole::Worker {
            return Err(Status::permission_denied(
                "worker token cannot transition team steps",
            ));
        }
        let payload = request.into_inner();

        let run_id = required_field(&payload.run_id, "run_id")?;
        self.authz.ensure_run_scope(&principal, run_id)?;

        let step_id = required_field(&payload.step_id, "step_id")?;
        let action = required_field(&payload.action, "action")?;

        let step = match action {
            "start" => {
                self.state
                    .teams
                    .start_step(step_id, optional_trimmed(&payload.remote_task_id))
                    .await
            }
            "complete" => {
                self.state
                    .teams
                    .complete_step(
                        step_id,
                        optional_trimmed(&payload.output_json)
                            .map(serde_json::from_str::<Value>)
                            .transpose()
                            .map_err(|err| Status::invalid_argument(err.to_string()))?,
                    )
                    .await
            }
            "fail" => {
                let err_text = required_field(&payload.error_text, "error_text")?;
                self.state.teams.fail_step(step_id, err_text).await
            }
            "input_required" => {
                self.state
                    .teams
                    .set_step_input_required(
                        step_id,
                        optional_trimmed(&payload.reason),
                        optional_trimmed(&payload.input_json)
                            .map(serde_json::from_str::<Value>)
                            .transpose()
                            .map_err(|err| Status::invalid_argument(err.to_string()))?,
                    )
                    .await
            }
            "resume" => {
                self.state
                    .teams
                    .resume_step(
                        step_id,
                        optional_trimmed(&payload.input_json)
                            .map(serde_json::from_str::<Value>)
                            .transpose()
                            .map_err(|err| Status::invalid_argument(err.to_string()))?,
                    )
                    .await
            }
            _ => return Err(Status::invalid_argument("unsupported action")),
        }
        .map_err(map_manager_error)?;

        if step.run_id != run_id {
            return Err(Status::permission_denied(
                "step does not belong to requested run scope",
            ));
        }
        Ok(Response::new(step_to_transition_response(step)))
    }

    async fn issue_node_credential(
        &self,
        request: Request<IssueNodeCredentialRequest>,
    ) -> Result<Response<IssueNodeCredentialResponse>, Status> {
        self.ensure_bootstrap_token(request.metadata())?;
        let payload = request.into_inner();

        let node_id = required_field(&payload.node_id, "node_id")?;
        let role_raw = required_field(&payload.role, "role")?;
        let role = InternalRole::parse(role_raw)
            .ok_or_else(|| Status::invalid_argument("unsupported role, expected leader/worker"))?;
        if role == InternalRole::Orchestrator {
            return Err(Status::invalid_argument(
                "role 'orchestrator' is reserved and cannot be issued via bootstrap",
            ));
        }

        let actor_id = optional_trimmed(&payload.actor_id);
        let run_id = optional_trimmed(&payload.run_id);
        if role == InternalRole::Worker {
            if actor_id.is_none() {
                return Err(Status::invalid_argument(
                    "worker bootstrap requires actor_id",
                ));
            }
            if run_id.is_none() {
                return Err(Status::invalid_argument("worker bootstrap requires run_id"));
            }
        }

        let requested_permissions = payload
            .permissions
            .into_iter()
            .filter_map(|value| normalize_permission(&value))
            .collect::<Vec<_>>();
        let mut permissions = if requested_permissions.is_empty() {
            default_permissions_for_role(role)
        } else {
            requested_permissions
        };
        permissions.sort();
        permissions.dedup();
        validate_role_permissions(role, &permissions)?;

        let ttl_seconds = if payload.ttl_seconds > 0 {
            payload.ttl_seconds.clamp(60, MAX_TOKEN_TTL_SECONDS)
        } else {
            DEFAULT_TOKEN_TTL_SECONDS
        };
        let issued = self
            .authz
            .issue_node_access_token(NodeCredentialRequest {
                source_node_id: node_id.to_string(),
                role: role.as_str().to_string(),
                actor_id: actor_id.map(str::to_string),
                run_id: run_id.map(str::to_string),
                permissions,
                scope: Vec::new(),
                audience: Vec::new(),
                ttl_seconds,
            })
            .map_err(|err| Status::internal(err.to_string()))?;

        let (cert_pem, key_pem, ca_cert_pem) =
            if self.security_mode == InternalGrpcSecurityMode::Disabled {
                (String::new(), String::new(), String::new())
            } else {
                let identity = load_bootstrap_client_identity(&self.cert_dir)
                    .map_err(|err| Status::internal(err.to_string()))?;
                (
                    String::from_utf8_lossy(&identity.cert_pem).to_string(),
                    String::from_utf8_lossy(&identity.key_pem).to_string(),
                    String::from_utf8_lossy(&identity.ca_cert_pem).to_string(),
                )
            };

        Ok(Response::new(IssueNodeCredentialResponse {
            node_id: node_id.to_string(),
            role: role.as_str().to_string(),
            access_token: issued.access_token,
            expires_at: issued.expires_at,
            cert_pem,
            key_pem,
            ca_cert_pem,
            security_mode: security_mode_to_str(self.security_mode).to_string(),
            cluster_id: issued.cluster_id,
            scope: issued.scope,
            audience: issued.audience,
            kid: issued.kid,
            issued_at: issued.issued_at,
            source_node_id: issued.source_node_id,
        }))
    }

    async fn ensure_agent_record(
        &self,
        request: Request<EnsureAgentRecordRequest>,
    ) -> Result<Response<EnsureAgentRecordResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::AgentManage)?;
        let payload = request.into_inner();

        let agent_id = required_field(&payload.agent_id, "agent_id")?;
        let config = parse_json_as::<AgentConfig>(&payload.config_json, "config_json")?;
        let source = optional_trimmed(&payload.source).unwrap_or("manual");
        let agent = self
            .state
            .agents
            .ensure_remote_managed_agent(agent_id, config, source)
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(EnsureAgentRecordResponse {
            agent_json: serde_json::to_string(&agent)
                .map_err(|err| Status::internal(err.to_string()))?,
        }))
    }

    async fn get_agent_record(
        &self,
        request: Request<GetAgentRecordRequest>,
    ) -> Result<Response<GetAgentRecordResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::AgentManage)?;
        let payload = request.into_inner();

        let agent_id = required_field(&payload.agent_id, "agent_id")?;
        let agent = self
            .state
            .agents
            .get_agent(agent_id)
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(GetAgentRecordResponse {
            agent_json: serde_json::to_string(&agent)
                .map_err(|err| Status::internal(err.to_string()))?,
        }))
    }

    async fn start_managed_agent(
        &self,
        request: Request<StartManagedAgentRequest>,
    ) -> Result<Response<StartManagedAgentResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::AgentManage)?;
        let payload = request.into_inner();

        let agent_id = required_field(&payload.agent_id, "agent_id")?;
        let actor_context = optional_trimmed(&payload.actor_context_json)
            .map(|raw| parse_json_str_as::<AcpActorSkillContext>(raw, "actor_context_json"))
            .transpose()?;
        let session_id = self
            .state
            .agents
            .start_agent_with_actor_context(agent_id, actor_context)
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(StartManagedAgentResponse { session_id }))
    }

    async fn stop_managed_agent(
        &self,
        request: Request<StopManagedAgentRequest>,
    ) -> Result<Response<StopManagedAgentResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::AgentManage)?;
        let payload = request.into_inner();

        let agent_id = required_field(&payload.agent_id, "agent_id")?;
        self.state
            .agents
            .stop_agent(agent_id)
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(StopManagedAgentResponse {}))
    }

    async fn delete_managed_agent(
        &self,
        request: Request<DeleteManagedAgentRequest>,
    ) -> Result<Response<DeleteManagedAgentResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::AgentManage)?;
        let payload = request.into_inner();

        let agent_id = required_field(&payload.agent_id, "agent_id")?;
        let _ = self.state.agents.stop_agent(agent_id).await;
        self.state
            .agents
            .delete_agent(agent_id)
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(DeleteManagedAgentResponse {}))
    }

    async fn send_agent_input(
        &self,
        request: Request<SendAgentInputRequest>,
    ) -> Result<Response<SendAgentInputResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::AgentManage)?;
        let payload = request.into_inner();

        let agent_id = required_field(&payload.agent_id, "agent_id")?;
        let input = required_field(&payload.input, "input")?;
        self.state
            .agents
            .send_input(
                agent_id,
                input,
                optional_trimmed(&payload.message_id),
                optional_trimmed(&payload.session_id),
            )
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(SendAgentInputResponse {}))
    }

    async fn list_agent_events(
        &self,
        request: Request<ListAgentEventsRequest>,
    ) -> Result<Response<ListAgentEventsResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::AgentManage)?;
        let payload = request.into_inner();

        let agent_id = required_field(&payload.agent_id, "agent_id")?;
        let limit = payload.limit.clamp(1, 1000);
        let before_event_id = (payload.before_event_id > 0).then_some(payload.before_event_id);
        let session_id = optional_trimmed(&payload.session_id);
        let events = if let Some(session_id) = session_id {
            self.state
                .agents
                .list_events_for_session(agent_id, session_id, limit, before_event_id)
                .await
        } else {
            self.state
                .agents
                .list_events(agent_id, limit, before_event_id)
                .await
        }
        .map_err(map_manager_error)?;
        Ok(Response::new(ListAgentEventsResponse {
            events: events.into_iter().map(agent_event_record).collect(),
        }))
    }
}

fn step_to_transition_response(step: TeamStepRecord) -> TransitionStepResponse {
    TransitionStepResponse {
        step_id: step.id,
        run_id: step.run_id,
        step_key: step.step_key,
        member_id: step.member_id,
        remote_task_id: step.runtime_handle_id.unwrap_or_default(),
        status: step_status_to_str(&step.status).to_string(),
        error_text: step.error_text.unwrap_or_default(),
    }
}

fn step_status_to_str(status: &TeamStepStatus) -> &'static str {
    match status {
        TeamStepStatus::Submitted => "submitted",
        TeamStepStatus::Working => "working",
        TeamStepStatus::InputRequired => "input_required",
        TeamStepStatus::Completed => "completed",
        TeamStepStatus::Failed => "failed",
        TeamStepStatus::Canceled => "canceled",
    }
}

fn required_field<'a>(raw: &'a str, field: &str) -> Result<&'a str, Status> {
    optional_trimmed(raw).ok_or_else(|| Status::invalid_argument(format!("{field} is required")))
}

fn optional_trimmed(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn parse_json_required(raw: &str, field: &str) -> Result<Value, Status> {
    let raw = required_field(raw, field)?;
    serde_json::from_str(raw)
        .map_err(|err| Status::invalid_argument(format!("{field} must be valid JSON: {err}")))
}

fn resolve_channel_replica_request(payload: &Value) -> Option<ChannelReplicaRequest> {
    let payload_obj = payload.as_object()?;
    let delivery_scope = payload_obj.get("delivery_scope")?.as_str()?.trim();
    if delivery_scope != "channel_broadcast" {
        return None;
    }
    let authority_message_id = payload_obj.get("authority_message_id")?.as_i64()?;
    if authority_message_id <= 0 {
        return None;
    }
    let team_id = payload_obj.get("team_id")?.as_str()?.trim();
    let conversation_id = payload_obj.get("channel_conversation_id")?.as_str()?.trim();
    let task_id = payload_obj.get("task_id")?.as_str()?.trim();
    let channel_id = payload_obj.get("channel_id")?.as_str()?.trim();
    if team_id.is_empty()
        || conversation_id.is_empty()
        || task_id.is_empty()
        || channel_id.is_empty()
    {
        return None;
    }
    Some(ChannelReplicaRequest {
        authority_message_id,
        team_id: team_id.to_string(),
        conversation_id: conversation_id.to_string(),
        task_id: task_id.to_string(),
        channel_id: channel_id.to_string(),
    })
}

async fn maybe_notify_actor_new_mailbox_message_type(
    state: &AppState,
    run_id: &str,
    send_result: &agenthub_team_actor::ActorSendResponse,
) -> anyhow::Result<()> {
    let Some(plan) = plan_actor_mailbox_immediate_hint(&state.teams, run_id, send_result).await?
    else {
        return Ok(());
    };
    let prompt = build_actor_mailbox_immediate_hint_prompt(run_id, plan.reason);
    let reason_label = match plan.reason {
        crate::team::ActorMailboxImmediateHintReason::DirectAgentMessage => "direct_agent_message",
        crate::team::ActorMailboxImmediateHintReason::LeaderChannelMention => {
            "leader_channel_mention"
        }
    };
    let mut sent_targets = Vec::new();
    let mut failed_targets = Vec::new();
    for target_actor_id in &plan.target_actor_ids {
        match state
            .agents
            .send_input(target_actor_id, &prompt, None, None)
            .await
        {
            Ok(()) => sent_targets.push(target_actor_id.clone()),
            Err(err) => {
                tracing::debug!(
                    run_id = %run_id,
                    actor_id = %target_actor_id,
                    reason = ?plan.reason,
                    "skip mailbox hint push because agent input is unavailable: {}",
                    err
                );
                failed_targets.push(target_actor_id.clone());
            }
        }
    }
    if let Err(err) = state
        .teams
        .append_run_event(
            run_id,
            "actor_mailbox_type_hint",
            serde_json::json!({
                "status": if failed_targets.is_empty() { "sent" } else if sent_targets.is_empty() { "send_failed" } else { "partial" },
                "message_id": send_result.message_id,
                "reason": reason_label,
                "target_actor_ids": plan.target_actor_ids,
                "sent_actor_ids": sent_targets,
                "failed_actor_ids": failed_targets,
            }),
        )
        .await
    {
        tracing::warn!(
            run_id = %run_id,
            "failed to append actor_mailbox_type_hint event: {}",
            err
        );
    }
    Ok(())
}

async fn load_team_context_for_actor(
    manager: &TeamManager,
    team_id: Option<&str>,
    run_id: Option<&str>,
    actor_id: &str,
) -> Result<crate::team::TeamContextRecord, Status> {
    let context = manager
        .describe_team_context(team_id, run_id)
        .await
        .map_err(map_team_context_error)?;
    ensure_team_member_access(manager, &context.team_id, actor_id).await?;
    Ok(context)
}

async fn ensure_team_member_access(
    manager: &TeamManager,
    team_id: &str,
    actor_id: &str,
) -> Result<(), Status> {
    if manager
        .team_has_member(team_id, actor_id)
        .await
        .map_err(map_manager_error)?
    {
        return Ok(());
    }
    Err(Status::permission_denied(
        "current actor is not a member of this team",
    ))
}

async fn ensure_leader_team_access(
    manager: &TeamManager,
    team_id: &str,
    actor_id: &str,
) -> Result<(), Status> {
    ensure_team_member_access(manager, team_id, actor_id).await?;
    let team = manager.get_team(team_id).await.map_err(map_manager_error)?;
    let leader_member_id = resolve_team_leader_member_id(&team.spec)?;
    if actor_id == leader_member_id {
        return Ok(());
    }
    Err(Status::permission_denied(
        "only leader may create or update Team tasks",
    ))
}

fn parse_team_task_status(raw: &str) -> Result<TeamTaskStatus, Status> {
    raw.trim().parse::<TeamTaskStatus>().map_err(|other| {
        Status::invalid_argument(format!(
            "invalid task status '{other}', expected one of: open, in_progress, in_review, completed, canceled"
        ))
    })
}

fn map_team_context_error(err: anyhow::Error) -> Status {
    if let Some(cause) = err.downcast_ref::<TeamContextLookupError>() {
        return match cause {
            TeamContextLookupError::MissingSelector
            | TeamContextLookupError::RunTeamMismatch { .. } => {
                Status::invalid_argument(cause.to_string())
            }
        };
    }
    map_manager_error(err)
}

fn map_agent_lookup_error(err: anyhow::Error) -> Status {
    if err
        .downcast_ref::<sqlx::Error>()
        .is_some_and(|cause| matches!(cause, sqlx::Error::RowNotFound))
    {
        return Status::not_found("agent not found");
    }
    map_manager_error(err)
}

fn is_shared_thread_task(task: &TeamTaskRecord) -> bool {
    if task
        .title
        .trim()
        .eq_ignore_ascii_case(TEAM_SHARED_THREAD_TITLE)
    {
        return true;
    }
    task.context
        .as_object()
        .and_then(|obj| obj.get("bootstrap_kind"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| value.eq_ignore_ascii_case(TEAM_SHARED_THREAD_BOOTSTRAP_KIND))
}

fn parse_json_as<T>(raw: &str, field: &str) -> Result<T, Status>
where
    T: serde::de::DeserializeOwned,
{
    let raw = required_field(raw, field)?;
    parse_json_str_as(raw, field)
}

fn parse_json_str_as<T>(raw: &str, field: &str) -> Result<T, Status>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(raw)
        .map_err(|err| Status::invalid_argument(format!("{field} must be valid JSON: {err}")))
}

fn optional_json_object(raw: Option<&str>, field: &str) -> Result<Option<Value>, Status> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let parsed: Value = serde_json::from_str(raw)
        .map_err(|err| Status::invalid_argument(format!("{field} must be valid JSON: {err}")))?;
    if !parsed.is_object() {
        return Err(Status::invalid_argument(format!(
            "{field} must be a JSON object"
        )));
    }
    Ok(Some(parsed))
}

fn map_serde_status(err: serde_json::Error) -> Status {
    Status::internal(err.to_string())
}

fn map_manager_error(err: anyhow::Error) -> Status {
    if err
        .downcast_ref::<sqlx::Error>()
        .is_some_and(|cause| matches!(cause, sqlx::Error::RowNotFound))
    {
        return Status::not_found("target record not found");
    }
    Status::internal(err.to_string())
}

fn agent_event_record(event: crate::agent::AgentEvent) -> AgentEventRecord {
    AgentEventRecord {
        event_id: event.event_id,
        agent_id: event.agent_id,
        session_id: event.session_id,
        seq: event.seq,
        ts: event.ts,
        stream: agent_output_stream_to_str(&event.stream).to_string(),
        message: event.message,
    }
}

fn agent_output_stream_to_str(stream: &crate::agent::OutputStream) -> &'static str {
    match stream {
        crate::agent::OutputStream::Stdout => "stdout",
        crate::agent::OutputStream::Stderr => "stderr",
        crate::agent::OutputStream::System => "system",
        crate::agent::OutputStream::Acp => "acp",
    }
}

fn map_actor_service_status(err: ActorServiceError) -> Status {
    match err.code {
        ActorServiceErrorCode::BadRequest | ActorServiceErrorCode::UnprocessableEntity => {
            Status::invalid_argument(err.message)
        }
        ActorServiceErrorCode::Unauthorized => Status::unauthenticated(err.message),
        ActorServiceErrorCode::Forbidden => Status::permission_denied(err.message),
        ActorServiceErrorCode::NotFound => Status::not_found(err.message),
        ActorServiceErrorCode::Conflict => Status::already_exists(err.message),
        ActorServiceErrorCode::Gone => Status::failed_precondition(err.message),
        ActorServiceErrorCode::TooManyRequests => Status::resource_exhausted(err.message),
        ActorServiceErrorCode::Internal => Status::internal(err.message),
    }
}

fn resolve_team_leader_member_id(spec: &Value) -> Result<String, Status> {
    if let Some(leader_member_id) = spec
        .get("leader_member_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(leader_member_id.to_string());
    }

    if let Some(members) = spec.get("members").and_then(Value::as_array) {
        for member in members {
            let role = member
                .get("role")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if let Some("leader") = role
                && let Some(member_id) = member
                    .get("member_id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            {
                return Ok(member_id.to_string());
            }
        }
    }

    if let Some(entrypoint) = spec
        .get("entrypoint")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(entrypoint.to_string());
    }

    Err(Status::failed_precondition(
        "team spec does not define a leader (leader_member_id, members[].role == 'leader', or entrypoint)",
    ))
}

fn normalize_permission(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn default_permissions_for_role(role: InternalRole) -> Vec<String> {
    match role {
        InternalRole::Leader => vec![
            InternalAction::MessageSend.as_str().to_string(),
            InternalAction::InboxList.as_str().to_string(),
            InternalAction::MessageAck.as_str().to_string(),
            InternalAction::TeamRead.as_str().to_string(),
            InternalAction::TeamTaskWrite.as_str().to_string(),
            InternalAction::TimeTriggerManage.as_str().to_string(),
            InternalAction::PermissionReview.as_str().to_string(),
            InternalAction::StepTransition.as_str().to_string(),
            InternalAction::NodeIssue.as_str().to_string(),
        ],
        InternalRole::Worker => vec![
            InternalAction::MessageSend.as_str().to_string(),
            InternalAction::InboxList.as_str().to_string(),
            InternalAction::MessageAck.as_str().to_string(),
            InternalAction::TeamRead.as_str().to_string(),
            InternalAction::TeamTaskWrite.as_str().to_string(),
            InternalAction::TimeTriggerManage.as_str().to_string(),
            InternalAction::PermissionReview.as_str().to_string(),
        ],
        InternalRole::Orchestrator => vec![
            InternalAction::MessageSend.as_str().to_string(),
            InternalAction::InboxList.as_str().to_string(),
            InternalAction::MessageAck.as_str().to_string(),
            InternalAction::TeamRead.as_str().to_string(),
            InternalAction::TeamTaskWrite.as_str().to_string(),
            InternalAction::TimeTriggerManage.as_str().to_string(),
            InternalAction::PermissionReview.as_str().to_string(),
            InternalAction::StepTransition.as_str().to_string(),
            InternalAction::NodeIssue.as_str().to_string(),
        ],
    }
}

fn validate_role_permissions(role: InternalRole, permissions: &[String]) -> Result<(), Status> {
    if permissions.is_empty() {
        return Err(Status::invalid_argument("permissions cannot be empty"));
    }
    if role == InternalRole::Worker {
        for permission in permissions {
            if permission == "*"
                || permission == InternalAction::StepTransition.as_str()
                || permission == InternalAction::NodeIssue.as_str()
            {
                return Err(Status::invalid_argument(
                    "worker permissions cannot include wildcard/step transition/node issue actions",
                ));
            }
        }
    }
    Ok(())
}

fn security_mode_to_str(mode: InternalGrpcSecurityMode) -> &'static str {
    match mode {
        InternalGrpcSecurityMode::Disabled => "disabled",
        InternalGrpcSecurityMode::Tls => "tls",
        InternalGrpcSecurityMode::Mtls => "mtls",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use sqlx::Row;
    use tonic::{Code, Request, metadata::MetadataValue};
    use uuid::Uuid;

    use super::super::auth::{InternalAction, InternalAuthz, InternalAuthzConfig, InternalRole};
    use super::super::proto::agenthub::internal::v1::team_internal_control_server::TeamInternalControl;
    use super::super::proto::agenthub::internal::v1::{
        AckActorMessageRequest, CancelTimeTriggerRequest, CreateTeamTaskRequest,
        CreateTimeTriggerRequest, DescribeTeamContextRequest, IssueNodeCredentialRequest,
        ListActorInboxRequest, ListTeamTasksRequest, ListTimeTriggersRequest,
        RespondPermissionReviewRequest, SendActorMessageRequest, UpdateTeamTaskRequest,
    };
    use super::{BOOTSTRAP_TOKEN_HEADER, TeamInternalControlService, map_actor_service_status};
    use crate::agent::AgentTimeTriggerRecord;
    use crate::api::team_tests::build_test_state;
    use crate::team::{TeamDefinitionConfig, TeamTaskRecord};
    use agenthub_team_actor::ActorMessageStatus;

    const TEST_INTERNAL_SHARED_SECRET: &str = "agenthub-internal-service-test-secret";

    fn build_authz() -> InternalAuthz {
        InternalAuthz::new(InternalAuthzConfig {
            shared_secret: TEST_INTERNAL_SHARED_SECRET.to_string(),
            expected_issuer: Some("agenthub".to_string()),
            expected_audience: Some("agenthub-internal".to_string()),
        })
    }

    fn issue_token(
        authz: &InternalAuthz,
        role: InternalRole,
        actor_id: Option<&str>,
        run_id: Option<&str>,
    ) -> String {
        let permissions = vec![
            InternalAction::MessageSend.as_str().to_string(),
            InternalAction::InboxList.as_str().to_string(),
            InternalAction::MessageAck.as_str().to_string(),
            InternalAction::TeamRead.as_str().to_string(),
            InternalAction::TeamTaskWrite.as_str().to_string(),
            InternalAction::TimeTriggerManage.as_str().to_string(),
            InternalAction::PermissionReview.as_str().to_string(),
        ];
        let (token, _expires_at) = authz
            .issue_access_token(role, actor_id, run_id, permissions, 600)
            .expect("issue internal token");
        token
    }

    fn authenticated_request<T>(payload: T, token: &str) -> Request<T> {
        let mut request = Request::new(payload);
        request.metadata_mut().insert(
            "authorization",
            MetadataValue::try_from(format!("Bearer {token}")).expect("authorization metadata"),
        );
        request
    }

    async fn create_team_run(state: &crate::state::AppState) -> crate::team::TeamRunRecord {
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: format!("internal-grpc-mailbox-{}", Uuid::new_v4()),
                description: Some("internal grpc mailbox test team".to_string()),
                spec: json!({
                    "entrypoint":"planner",
                    "leader_member_id":"planner",
                    "members":[
                        {"member_id":"planner","role":"leader"},
                        {"member_id":"reviewer","role":"worker"}
                    ]
                }),
            })
            .await
            .expect("create test team");
        state
            .teams
            .create_run(
                &team.id,
                Some("ctx-internal-grpc-mailbox"),
                json!({"prompt":"validate internal grpc mailbox"}),
            )
            .await
            .expect("create test run")
    }

    fn default_permission_review_team_spec() -> Value {
        json!({
            "entrypoint":"planner",
            "leader_member_id":"planner",
            "members":[
                {"member_id":"planner","role":"leader"},
                {"member_id":"reviewer","role":"worker"},
                {"member_id":"observer","role":"worker"}
            ]
        })
    }

    async fn create_permission_review_run_with_spec(
        state: &crate::state::AppState,
        name_suffix: &str,
        prompt: &str,
        spec: Value,
    ) -> crate::team::TeamRunRecord {
        let context_id = format!("ctx-internal-grpc-{name_suffix}");
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: format!("internal-grpc-{name_suffix}-{}", Uuid::new_v4()),
                description: Some(format!("{name_suffix} permission review test")),
                spec,
            })
            .await
            .expect("create permission review team");
        state
            .teams
            .create_run(
                &team.id,
                Some(context_id.as_str()),
                json!({"prompt": prompt}),
            )
            .await
            .expect("create permission review run")
    }

    struct PermissionReviewFixture {
        state: crate::state::AppState,
        run: crate::team::TeamRunRecord,
        service: TeamInternalControlService,
        token: String,
        now: i64,
    }

    struct PermissionReviewSeed<'a> {
        request_id: &'a str,
        agent_id: &'a str,
        session_id: &'a str,
        acp_session_id: &'a str,
        requester_actor_id: &'a str,
        requester_role: &'a str,
        review_target_actor_id: Option<&'a str>,
        tool_call_id: &'a str,
        status: &'a str,
    }

    async fn setup_permission_review_fixture_with_spec(
        name_suffix: &str,
        prompt: &str,
        spec: Value,
        token_role: InternalRole,
        token_actor_id: &str,
    ) -> PermissionReviewFixture {
        let state = build_test_state().await;
        let run = create_permission_review_run_with_spec(&state, name_suffix, prompt, spec).await;
        let authz = build_authz();
        let token = issue_token(&authz, token_role, Some(token_actor_id), None);
        let service = TeamInternalControlService::new(
            state.clone(),
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );
        PermissionReviewFixture {
            state,
            run,
            service,
            token,
            now: chrono::Utc::now().timestamp(),
        }
    }

    async fn setup_permission_review_fixture(
        name_suffix: &str,
        prompt: &str,
    ) -> PermissionReviewFixture {
        setup_permission_review_fixture_with_spec(
            name_suffix,
            prompt,
            default_permission_review_team_spec(),
            InternalRole::Worker,
            "observer",
        )
        .await
    }

    async fn seed_permission_review_request(
        state: &crate::state::AppState,
        run: &crate::team::TeamRunRecord,
        seed: PermissionReviewSeed<'_>,
        now: i64,
    ) {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'running', ?7, ?8)
            "#,
        )
        .bind(seed.agent_id)
        .bind(seed.agent_id)
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert permission review agent");
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind(seed.session_id)
        .bind(seed.agent_id)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert permission review session");
        sqlx::query(
            r#"
            INSERT INTO acp_permission_requests (
                id,
                agent_id,
                session_id,
                acp_session_id,
                team_id,
                requester_actor_id,
                requester_role,
                review_target_actor_id,
                tool_call_id,
                options_json,
                tool_call_json,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
        )
        .bind(seed.request_id)
        .bind(seed.agent_id)
        .bind(seed.session_id)
        .bind(seed.acp_session_id)
        .bind(&run.team_id)
        .bind(seed.requester_actor_id)
        .bind(seed.requester_role)
        .bind(seed.review_target_actor_id)
        .bind(seed.tool_call_id)
        .bind(
            json!([
                {
                    "option_id": "allow",
                    "name": "Allow once",
                    "kind": "allow_once"
                }
            ])
            .to_string(),
        )
        .bind(json!({"tool":{"name":"mcp__fs__read"}}).to_string())
        .bind(seed.status)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert permission review request");
    }

    #[tokio::test]
    async fn internal_grpc_team_context_and_task_controls_are_wire_compatible() {
        let state = build_test_state().await;
        let run = create_team_run(&state).await;
        let authz = build_authz();
        let token = issue_token(&authz, InternalRole::Leader, Some("planner"), Some(&run.id));
        let service = TeamInternalControlService::new(
            state,
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );

        let context = TeamInternalControl::describe_team_context(
            &service,
            authenticated_request(
                DescribeTeamContextRequest {
                    team_id: String::new(),
                    run_id: run.id.clone(),
                    actor_id: "planner".to_string(),
                },
                &token,
            ),
        )
        .await
        .expect("describe team context")
        .into_inner();
        let context_json: serde_json::Value =
            serde_json::from_str(&context.context_json).expect("decode team context");
        assert_eq!(context_json["team_id"], json!(run.team_id));
        assert_eq!(context_json["run"]["run_id"], json!(run.id));
        assert_eq!(context_json["runtime"]["member_count"], json!(2));
        assert!(
            context_json["members"]
                .as_array()
                .expect("members array")
                .iter()
                .any(|member| member["member_id"] == json!("planner"))
        );
        assert!(
            context_json["members"]
                .as_array()
                .expect("members array")
                .iter()
                .any(|member| member["member_id"] == json!("reviewer"))
        );

        let created = TeamInternalControl::create_team_task(
            &service,
            authenticated_request(
                CreateTeamTaskRequest {
                    team_id: run.team_id.clone(),
                    actor_id: "planner".to_string(),
                    title: "Investigate authority-only actor CLI".to_string(),
                    status: "in_progress".to_string(),
                    topic: "actor-cli".to_string(),
                    context_json: json!({"goal":"remove sqlite fallback"}).to_string(),
                },
                &token,
            ),
        )
        .await
        .expect("create team task")
        .into_inner();
        let created_json: serde_json::Value =
            serde_json::from_str(&created.output_json).expect("decode created task output");
        let task_id = created_json["task"]["id"]
            .as_str()
            .expect("task id")
            .to_string();
        assert_eq!(created_json["task"]["status"], json!("in_progress"));
        assert_eq!(
            created_json["task"]["created_by_actor_id"],
            json!("planner")
        );
        assert_eq!(
            created_json["task"]["assigned_member_id"],
            serde_json::Value::Null
        );
        assert_eq!(created_json["conversation"]["topic"], json!("actor-cli"));

        let listed = TeamInternalControl::list_team_tasks(
            &service,
            authenticated_request(
                ListTeamTasksRequest {
                    team_id: run.team_id.clone(),
                    actor_id: "planner".to_string(),
                    limit: 20,
                    status: "in_progress".to_string(),
                    include_shared_thread: false,
                },
                &token,
            ),
        )
        .await
        .expect("list team tasks")
        .into_inner();
        let listed_tasks: Vec<TeamTaskRecord> =
            serde_json::from_str(&listed.tasks_json).expect("decode task list");
        let created_task = listed_tasks
            .iter()
            .find(|task| task.id == task_id)
            .expect("created task in filtered list");
        assert_eq!(created_task.status, crate::team::TeamTaskStatus::InProgress);
        assert!(created_task.assigned_member_id.is_none());

        let updated = TeamInternalControl::update_team_task(
            &service,
            authenticated_request(
                UpdateTeamTaskRequest {
                    team_id: run.team_id,
                    actor_id: "planner".to_string(),
                    task_id: task_id.clone(),
                    status: "completed".to_string(),
                },
                &token,
            ),
        )
        .await
        .expect("update team task")
        .into_inner();
        let updated_task: TeamTaskRecord =
            serde_json::from_str(&updated.task_json).expect("decode updated task");
        assert_eq!(updated_task.id, task_id);
        assert_eq!(updated_task.status, crate::team::TeamTaskStatus::Completed);
        assert!(updated_task.assigned_member_id.is_none());
    }

    #[tokio::test]
    async fn internal_grpc_describe_team_context_rejects_invalid_scope_inputs() {
        let state = build_test_state().await;
        let run = create_team_run(&state).await;
        let authz = build_authz();
        let token = issue_token(&authz, InternalRole::Leader, Some("planner"), None);
        let service = TeamInternalControlService::new(
            state.clone(),
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );

        let missing_selector_err = TeamInternalControl::describe_team_context(
            &service,
            authenticated_request(
                DescribeTeamContextRequest {
                    team_id: String::new(),
                    run_id: String::new(),
                    actor_id: "planner".to_string(),
                },
                &token,
            ),
        )
        .await
        .expect_err("missing team/run selector should fail");
        assert_eq!(missing_selector_err.code(), Code::InvalidArgument);
        assert_eq!(
            missing_selector_err.message(),
            "team_id or run_id is required"
        );

        let other_team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: format!("other-team-{}", Uuid::new_v4()),
                description: Some("other team".to_string()),
                spec: json!({
                    "entrypoint":"planner",
                    "leader_member_id":"planner",
                    "members":[
                        {"member_id":"planner","role":"leader"},
                        {"member_id":"reviewer","role":"worker"}
                    ]
                }),
            })
            .await
            .expect("create other team");
        let mismatch_err = TeamInternalControl::describe_team_context(
            &service,
            authenticated_request(
                DescribeTeamContextRequest {
                    team_id: other_team.id,
                    run_id: run.id,
                    actor_id: "planner".to_string(),
                },
                &token,
            ),
        )
        .await
        .expect_err("mismatched team/run should fail");
        assert_eq!(mismatch_err.code(), Code::InvalidArgument);
        assert!(mismatch_err.message().contains("belongs to team"));
    }

    #[tokio::test]
    async fn internal_grpc_describe_team_context_defaults_to_scoped_run() {
        let state = build_test_state().await;
        let run = create_team_run(&state).await;
        let authz = build_authz();
        let token = issue_token(&authz, InternalRole::Leader, Some("planner"), Some(&run.id));
        let service = TeamInternalControlService::new(
            state,
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );

        let context = TeamInternalControl::describe_team_context(
            &service,
            authenticated_request(
                DescribeTeamContextRequest {
                    team_id: String::new(),
                    run_id: String::new(),
                    actor_id: "planner".to_string(),
                },
                &token,
            ),
        )
        .await
        .expect("scoped run should default describe_team_context target")
        .into_inner();
        let context_json: serde_json::Value =
            serde_json::from_str(&context.context_json).expect("decode scoped run context");
        assert_eq!(context_json["team_id"], json!(run.team_id));
        assert_eq!(context_json["run"]["run_id"], json!(run.id));
    }

    #[tokio::test]
    async fn internal_grpc_time_trigger_controls_are_wire_compatible() {
        let state = build_test_state().await;
        sqlx::query(
            r#"
            CREATE TABLE agent_time_triggers (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                created_by_actor_id TEXT NOT NULL,
                message_text TEXT NOT NULL,
                fire_at INTEGER NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                fired_at INTEGER,
                last_error TEXT,
                FOREIGN KEY(agent_id) REFERENCES agents(id)
            )
            "#,
        )
        .execute(&state.db)
        .await
        .expect("create agent_time_triggers");
        let authz = build_authz();
        let token = issue_token(&authz, InternalRole::Worker, Some("reviewer"), None);
        let service = TeamInternalControlService::new(
            state,
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );
        let fire_at = chrono::Utc::now().timestamp() + 120;

        let created = TeamInternalControl::create_time_trigger(
            &service,
            authenticated_request(
                CreateTimeTriggerRequest {
                    actor_id: "reviewer".to_string(),
                    message_text: "Ping the reviewer inbox".to_string(),
                    fire_at,
                },
                &token,
            ),
        )
        .await
        .expect("create time trigger")
        .into_inner();
        let trigger: AgentTimeTriggerRecord =
            serde_json::from_str(&created.trigger_json).expect("decode trigger");
        assert_eq!(trigger.agent_id, "reviewer");
        assert_eq!(trigger.created_by_actor_id, "reviewer");
        assert_eq!(trigger.message_text, "Ping the reviewer inbox");
        assert_eq!(trigger.fire_at, fire_at);

        let listed = TeamInternalControl::list_time_triggers(
            &service,
            authenticated_request(
                ListTimeTriggersRequest {
                    actor_id: "reviewer".to_string(),
                    limit: 20,
                },
                &token,
            ),
        )
        .await
        .expect("list time triggers")
        .into_inner();
        let triggers: Vec<AgentTimeTriggerRecord> =
            serde_json::from_str(&listed.triggers_json).expect("decode trigger list");
        assert!(triggers.iter().any(|item| item.id == trigger.id));

        let canceled = TeamInternalControl::cancel_time_trigger(
            &service,
            authenticated_request(
                CancelTimeTriggerRequest {
                    actor_id: "reviewer".to_string(),
                    trigger_id: trigger.id.clone(),
                },
                &token,
            ),
        )
        .await
        .expect("cancel time trigger")
        .into_inner();
        let canceled_json: serde_json::Value =
            serde_json::from_str(&canceled.output_json).expect("decode cancel output");
        assert_eq!(canceled_json["status"], json!("ok"));
        assert_eq!(canceled_json["trigger_id"], json!(trigger.id.clone()));

        let listed_after_cancel = TeamInternalControl::list_time_triggers(
            &service,
            authenticated_request(
                ListTimeTriggersRequest {
                    actor_id: "reviewer".to_string(),
                    limit: 20,
                },
                &token,
            ),
        )
        .await
        .expect("list time triggers after cancel")
        .into_inner();
        let triggers_after_cancel: Vec<AgentTimeTriggerRecord> =
            serde_json::from_str(&listed_after_cancel.triggers_json)
                .expect("decode trigger list after cancel");
        let canceled_trigger = triggers_after_cancel
            .iter()
            .find(|item| item.id == trigger.id)
            .expect("canceled trigger remains queryable");
        assert_eq!(
            serde_json::to_value(&canceled_trigger.status).expect("serialize canceled status"),
            json!("canceled")
        );
    }

    #[tokio::test]
    async fn internal_grpc_time_trigger_rejects_past_fire_at() {
        let state = build_test_state().await;
        sqlx::query(
            r#"
            CREATE TABLE agent_time_triggers (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                created_by_actor_id TEXT NOT NULL,
                message_text TEXT NOT NULL,
                fire_at INTEGER NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                fired_at INTEGER,
                last_error TEXT,
                FOREIGN KEY(agent_id) REFERENCES agents(id)
            )
            "#,
        )
        .execute(&state.db)
        .await
        .expect("create agent_time_triggers");
        let authz = build_authz();
        let token = issue_token(&authz, InternalRole::Worker, Some("reviewer"), None);
        let service = TeamInternalControlService::new(
            state,
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );

        let err = TeamInternalControl::create_time_trigger(
            &service,
            authenticated_request(
                CreateTimeTriggerRequest {
                    actor_id: "reviewer".to_string(),
                    message_text: "late trigger".to_string(),
                    fire_at: chrono::Utc::now().timestamp() - 1,
                },
                &token,
            ),
        )
        .await
        .expect_err("past fire_at should be rejected");
        assert_eq!(err.code(), Code::InvalidArgument);
        assert_eq!(err.message(), "fire_at must be in the future");
    }

    #[tokio::test]
    async fn internal_grpc_time_trigger_rejects_unknown_agent() {
        let state = build_test_state().await;
        sqlx::query(
            r#"
            CREATE TABLE agent_time_triggers (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                created_by_actor_id TEXT NOT NULL,
                message_text TEXT NOT NULL,
                fire_at INTEGER NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                fired_at INTEGER,
                last_error TEXT,
                FOREIGN KEY(agent_id) REFERENCES agents(id)
            )
            "#,
        )
        .execute(&state.db)
        .await
        .expect("create agent_time_triggers");
        let missing_actor_id = format!("missing-reviewer-{}", Uuid::new_v4());
        state
            .agents
            .get_agent(&missing_actor_id)
            .await
            .expect_err("missing actor should not be seeded in test state");
        let authz = build_authz();
        let token = issue_token(&authz, InternalRole::Worker, Some(&missing_actor_id), None);
        let service = TeamInternalControlService::new(
            state,
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );

        let err = TeamInternalControl::create_time_trigger(
            &service,
            authenticated_request(
                CreateTimeTriggerRequest {
                    actor_id: missing_actor_id,
                    message_text: "missing agent".to_string(),
                    fire_at: chrono::Utc::now().timestamp() + 120,
                },
                &token,
            ),
        )
        .await
        .expect_err("missing actor agent should fail");
        assert_eq!(err.code(), Code::NotFound);
        assert_eq!(err.message(), "agent not found");
    }

    #[tokio::test]
    async fn internal_grpc_mailbox_send_list_ack_are_wire_compatible() {
        let state = build_test_state().await;
        let run = create_team_run(&state).await;
        let authz = build_authz();
        let token = issue_token(&authz, InternalRole::Leader, None, Some(&run.id));
        let service = TeamInternalControlService::new(
            state.clone(),
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );

        let send = TeamInternalControl::send_actor_message(
            &service,
            authenticated_request(
                SendActorMessageRequest {
                    run_id: run.id.clone(),
                    from_actor_id: "planner".to_string(),
                    to_actor_id: "reviewer".to_string(),
                    channel: "coordination".to_string(),
                    transport: "local".to_string(),
                    route_json: r#"{"topic":"review"}"#.to_string(),
                    payload_json: r#"{"text":"please review"}"#.to_string(),
                    idempotency_key: "internal-grpc-msg-1".to_string(),
                    from_peer_id: "node-a".to_string(),
                    to_peer_id: "main".to_string(),
                    channel_id: String::new(),
                },
                &token,
            ),
        )
        .await
        .expect("send actor message")
        .into_inner();
        assert!(send.message_id > 0);
        assert_eq!(send.status, "pending");
        assert_eq!(send.idempotency_key, "internal-grpc-msg-1");
        let sent_message: crate::team::TeamActorMessageRecord =
            serde_json::from_str(&send.message_json).expect("decode sent message_json");
        assert_eq!(sent_message.message_id, send.message_id);
        assert_eq!(sent_message.to_actor_id, "reviewer");
        assert_eq!(sent_message.status, ActorMessageStatus::Pending);

        let pending_inbox = TeamInternalControl::list_actor_inbox(
            &service,
            authenticated_request(
                ListActorInboxRequest {
                    run_id: run.id.clone(),
                    actor_id: "reviewer".to_string(),
                    limit: 100,
                    after_message_id: 0,
                    include_delivered: false,
                },
                &token,
            ),
        )
        .await
        .expect("list pending inbox")
        .into_inner();
        assert_eq!(pending_inbox.messages.len(), 1);
        let pending = &pending_inbox.messages[0];
        assert_eq!(pending.message_id, send.message_id);
        assert_eq!(pending.run_id, run.id);
        assert_eq!(pending.from_actor_id, "planner");
        assert_eq!(pending.to_actor_id, "reviewer");
        assert_eq!(pending.channel, "coordination");
        assert_eq!(pending.transport, "local");
        assert_eq!(pending.route_json, r#"{"topic":"review"}"#);
        assert_eq!(pending.payload_json, r#"{"text":"please review"}"#);
        assert_eq!(pending.status, "pending");
        assert_eq!(pending.from_peer_id, "node-a");
        assert_eq!(pending.to_peer_id, "main");

        let acked = TeamInternalControl::ack_actor_message(
            &service,
            authenticated_request(
                AckActorMessageRequest {
                    run_id: run.id.clone(),
                    actor_id: "reviewer".to_string(),
                    message_id: send.message_id,
                },
                &token,
            ),
        )
        .await
        .expect("ack actor message")
        .into_inner();
        let acked_message = acked.message.expect("acked message");
        assert_eq!(acked_message.message_id, send.message_id);
        assert_eq!(acked_message.status, "delivered");
        assert!(acked_message.delivered_at >= acked_message.created_at);
        assert_eq!(acked_message.from_peer_id, "node-a");
        assert_eq!(acked_message.to_peer_id, "main");

        let pending_after_ack = TeamInternalControl::list_actor_inbox(
            &service,
            authenticated_request(
                ListActorInboxRequest {
                    run_id: run.id.clone(),
                    actor_id: "reviewer".to_string(),
                    limit: 100,
                    after_message_id: 0,
                    include_delivered: false,
                },
                &token,
            ),
        )
        .await
        .expect("list pending inbox after ack")
        .into_inner();
        assert!(pending_after_ack.messages.is_empty());

        let inbox_with_delivered = TeamInternalControl::list_actor_inbox(
            &service,
            authenticated_request(
                ListActorInboxRequest {
                    run_id: run.id,
                    actor_id: "reviewer".to_string(),
                    limit: 100,
                    after_message_id: 0,
                    include_delivered: true,
                },
                &token,
            ),
        )
        .await
        .expect("list inbox including delivered")
        .into_inner();
        assert_eq!(inbox_with_delivered.messages.len(), 1);
        assert_eq!(inbox_with_delivered.messages[0].status, "delivered");
    }

    #[tokio::test]
    async fn internal_grpc_permission_review_respond_updates_pending_request() {
        let state = build_test_state().await;
        let run = create_team_run(&state).await;
        let authz = build_authz();
        let token = issue_token(&authz, InternalRole::Leader, Some("planner"), None);
        let service = TeamInternalControlService::new(
            state.clone(),
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'running', ?7, ?8)
            "#,
        )
        .bind("worker-agent")
        .bind("worker-agent")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker agent");
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind("worker-session")
        .bind("worker-agent")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker session");
        sqlx::query(
            r#"
            INSERT INTO acp_permission_requests (
                id,
                agent_id,
                session_id,
                acp_session_id,
                team_id,
                requester_actor_id,
                requester_role,
                tool_call_id,
                options_json,
                tool_call_json,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11)
            "#,
        )
        .bind("perm-internal-1")
        .bind("worker-agent")
        .bind("worker-session")
        .bind("acp-session-1")
        .bind(&run.team_id)
        .bind("reviewer")
        .bind("worker")
        .bind("tool-call-1")
        .bind(
            json!([
                {
                    "option_id": "allow",
                    "name": "Allow once",
                    "kind": "allow_once"
                }
            ])
            .to_string(),
        )
        .bind(json!({"tool":{"name":"mcp__fs__read"}}).to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert permission request");

        let response = TeamInternalControl::respond_permission_review(
            &service,
            authenticated_request(
                RespondPermissionReviewRequest {
                    team_id: run.team_id.clone(),
                    actor_id: "planner".to_string(),
                    permission_id: "perm-internal-1".to_string(),
                    option_id: "allow".to_string(),
                    outcome: String::new(),
                },
                &token,
            ),
        )
        .await
        .expect("respond permission review")
        .into_inner();

        assert_eq!(response.status, "ok");
        assert_eq!(response.permission_id, "perm-internal-1");
        assert_eq!(response.request_status, "responded");
        assert_eq!(response.reviewed_by_actor_id, "planner");

        let row = sqlx::query(
            "SELECT status, selected_option_id, reviewed_by_actor_id FROM acp_permission_requests WHERE id = ?1",
        )
        .bind("perm-internal-1")
        .fetch_one(&state.db)
        .await
        .expect("load permission request");
        assert_eq!(row.get::<String, _>("status"), "responded");
        assert_eq!(row.get::<String, _>("selected_option_id"), "allow");
        assert_eq!(row.get::<String, _>("reviewed_by_actor_id"), "planner");
    }

    #[tokio::test]
    async fn internal_grpc_permission_review_respond_accepts_legacy_team_leader_fallback() {
        let fixture = setup_permission_review_fixture_with_spec(
            "legacy-leader-fallback",
            "validate legacy leader fallback",
            json!({
                "entrypoint":"planner",
                "leader_member_id":"planner",
                "members":[
                    {"member_id":"planner","role":"leader"},
                    {"member_id":"reviewer","role":"worker"}
                ]
            }),
            InternalRole::Leader,
            "planner",
        )
        .await;
        seed_permission_review_request(
            &fixture.state,
            &fixture.run,
            PermissionReviewSeed {
                request_id: "perm-legacy-leader-1",
                agent_id: "legacy-worker-agent",
                session_id: "legacy-worker-session",
                acp_session_id: "acp-session-legacy-1",
                requester_actor_id: "reviewer",
                requester_role: "worker",
                review_target_actor_id: None,
                tool_call_id: "tool-call-legacy-1",
                status: "pending",
            },
            fixture.now,
        )
        .await;

        let response = TeamInternalControl::respond_permission_review(
            &fixture.service,
            authenticated_request(
                RespondPermissionReviewRequest {
                    team_id: fixture.run.team_id.clone(),
                    actor_id: "planner".to_string(),
                    permission_id: "perm-legacy-leader-1".to_string(),
                    option_id: "allow".to_string(),
                    outcome: String::new(),
                },
                &fixture.token,
            ),
        )
        .await
        .expect("respond permission review")
        .into_inner();

        assert_eq!(response.status, "ok");
        assert_eq!(response.request_status, "responded");
        assert_eq!(response.reviewed_by_actor_id, "planner");
    }

    #[tokio::test]
    async fn internal_grpc_permission_review_respond_accepts_legacy_team_peer_worker_fallback() {
        let fixture = setup_permission_review_fixture_with_spec(
            "legacy-peer-worker-fallback",
            "validate legacy peer worker fallback",
            json!({
                "entrypoint":"planner",
                "leader_member_id":"planner",
                "members":[
                    {"member_id":"planner","role":"leader"},
                    {"member_id":"requester","role":"worker"},
                    {"member_id":"reviewer","role":"worker"}
                ]
            }),
            InternalRole::Worker,
            "reviewer",
        )
        .await;
        seed_permission_review_request(
            &fixture.state,
            &fixture.run,
            PermissionReviewSeed {
                request_id: "perm-legacy-peer-worker-1",
                agent_id: "legacy-peer-worker-agent",
                session_id: "legacy-peer-worker-session",
                acp_session_id: "acp-session-legacy-peer-worker-1",
                requester_actor_id: "requester",
                requester_role: "worker",
                review_target_actor_id: None,
                tool_call_id: "tool-call-legacy-peer-worker-1",
                status: "pending",
            },
            fixture.now,
        )
        .await;

        let response = TeamInternalControl::respond_permission_review(
            &fixture.service,
            authenticated_request(
                RespondPermissionReviewRequest {
                    team_id: fixture.run.team_id.clone(),
                    actor_id: "reviewer".to_string(),
                    permission_id: "perm-legacy-peer-worker-1".to_string(),
                    option_id: "allow".to_string(),
                    outcome: String::new(),
                },
                &fixture.token,
            ),
        )
        .await
        .expect("respond permission review")
        .into_inner();

        assert_eq!(response.status, "ok");
        assert_eq!(response.request_status, "responded");
        assert_eq!(response.reviewed_by_actor_id, "reviewer");
    }

    #[tokio::test]
    async fn internal_grpc_permission_review_respond_surfaces_legacy_reviewer_resolution_errors() {
        let fixture = setup_permission_review_fixture_with_spec(
            "legacy-reviewer-resolution-error",
            "validate legacy reviewer resolution errors",
            json!({
                "entrypoint":"reviewer",
                "leader_member_id":"reviewer",
                "members":[
                    {"member_id":"reviewer","role":"worker"}
                ]
            }),
            InternalRole::Worker,
            "reviewer",
        )
        .await;
        seed_permission_review_request(
            &fixture.state,
            &fixture.run,
            PermissionReviewSeed {
                request_id: "perm-legacy-resolution-error-1",
                agent_id: "legacy-resolution-error-agent",
                session_id: "legacy-resolution-error-session",
                acp_session_id: "acp-session-legacy-resolution-error-1",
                requester_actor_id: "removed-planner",
                requester_role: "leader",
                review_target_actor_id: None,
                tool_call_id: "tool-call-legacy-resolution-error-1",
                status: "pending",
            },
            fixture.now,
        )
        .await;

        let err = TeamInternalControl::respond_permission_review(
            &fixture.service,
            authenticated_request(
                RespondPermissionReviewRequest {
                    team_id: fixture.run.team_id.clone(),
                    actor_id: "reviewer".to_string(),
                    permission_id: "perm-legacy-resolution-error-1".to_string(),
                    option_id: "allow".to_string(),
                    outcome: String::new(),
                },
                &fixture.token,
            ),
        )
        .await
        .expect_err("legacy reviewer resolution should fail");

        assert_eq!(err.code(), tonic::Code::FailedPrecondition);
        assert!(
            err.message()
                .contains("failed to resolve active reviewer for permission request"),
            "unexpected error message: {err}"
        );
    }

    #[tokio::test]
    async fn internal_grpc_permission_review_respond_reports_timeout_before_reviewer_check() {
        let fixture = setup_permission_review_fixture(
            "timeout-review",
            "validate resolved review precedence",
        )
        .await;
        seed_permission_review_request(
            &fixture.state,
            &fixture.run,
            PermissionReviewSeed {
                request_id: "perm-timeout-review-1",
                agent_id: "timeout-worker-agent",
                session_id: "timeout-worker-session",
                acp_session_id: "acp-session-timeout-1",
                requester_actor_id: "planner",
                requester_role: "leader",
                review_target_actor_id: None,
                tool_call_id: "tool-call-timeout-1",
                status: "timeout",
            },
            fixture.now,
        )
        .await;

        let response = TeamInternalControl::respond_permission_review(
            &fixture.service,
            authenticated_request(
                RespondPermissionReviewRequest {
                    team_id: fixture.run.team_id.clone(),
                    actor_id: "observer".to_string(),
                    permission_id: "perm-timeout-review-1".to_string(),
                    option_id: "allow".to_string(),
                    outcome: String::new(),
                },
                &fixture.token,
            ),
        )
        .await
        .expect("timeout permission review should report already resolved")
        .into_inner();

        assert_eq!(response.status, "already_resolved");
        assert_eq!(response.request_status, "timeout");
        assert!(
            response.reviewed_by_actor_id.is_empty(),
            "expected no reviewer for timed-out request"
        );
    }

    #[tokio::test]
    async fn internal_grpc_permission_review_respond_keeps_pending_reviewer_guard() {
        let fixture =
            setup_permission_review_fixture("pending-review", "validate pending reviewer guard")
                .await;
        seed_permission_review_request(
            &fixture.state,
            &fixture.run,
            PermissionReviewSeed {
                request_id: "perm-pending-review-1",
                agent_id: "pending-worker-agent",
                session_id: "pending-worker-session",
                acp_session_id: "acp-session-pending-1",
                requester_actor_id: "planner",
                requester_role: "leader",
                review_target_actor_id: Some("reviewer"),
                tool_call_id: "tool-call-pending-1",
                status: "pending",
            },
            fixture.now,
        )
        .await;

        let err = TeamInternalControl::respond_permission_review(
            &fixture.service,
            authenticated_request(
                RespondPermissionReviewRequest {
                    team_id: fixture.run.team_id.clone(),
                    actor_id: "observer".to_string(),
                    permission_id: "perm-pending-review-1".to_string(),
                    option_id: "allow".to_string(),
                    outcome: String::new(),
                },
                &fixture.token,
            ),
        )
        .await
        .expect_err("pending permission review should reject non-reviewer actor");

        assert_eq!(err.code(), tonic::Code::PermissionDenied);
        assert!(
            err.message()
                .contains("current actor is not the active reviewer for this permission request"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn internal_grpc_permission_review_respond_rejects_conflicting_outcome_fields() {
        let state = build_test_state().await;
        let run = create_team_run(&state).await;
        let authz = build_authz();
        let token = issue_token(&authz, InternalRole::Leader, Some("planner"), None);
        let service = TeamInternalControlService::new(
            state.clone(),
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'running', ?7, ?8)
            "#,
        )
        .bind("conflict-worker-agent")
        .bind("conflict-worker-agent")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker agent");
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind("conflict-worker-session")
        .bind("conflict-worker-agent")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker session");
        sqlx::query(
            r#"
            INSERT INTO acp_permission_requests (
                id,
                agent_id,
                session_id,
                acp_session_id,
                team_id,
                requester_actor_id,
                requester_role,
                tool_call_id,
                options_json,
                tool_call_json,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11)
            "#,
        )
        .bind("perm-conflict-review-1")
        .bind("conflict-worker-agent")
        .bind("conflict-worker-session")
        .bind("acp-session-conflict-1")
        .bind(&run.team_id)
        .bind("reviewer")
        .bind("worker")
        .bind("tool-call-conflict-1")
        .bind(
            json!([
                {
                    "option_id": "allow",
                    "name": "Allow once",
                    "kind": "allow_once"
                }
            ])
            .to_string(),
        )
        .bind(json!({"tool":{"name":"mcp__fs__read"}}).to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert permission request");

        let err = TeamInternalControl::respond_permission_review(
            &service,
            authenticated_request(
                RespondPermissionReviewRequest {
                    team_id: run.team_id.clone(),
                    actor_id: "planner".to_string(),
                    permission_id: "perm-conflict-review-1".to_string(),
                    option_id: "allow".to_string(),
                    outcome: "cancelled".to_string(),
                },
                &token,
            ),
        )
        .await
        .expect_err("conflicting response fields should be rejected");

        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(
            err.message()
                .contains("option_id and outcome cannot be set together"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn internal_grpc_mailbox_send_persists_channel_replica_history() {
        let state = build_test_state().await;
        let run = create_team_run(&state).await;
        let (task_id, conversation_id) = state
            .teams
            .ensure_shared_thread_target_for_team(&run.team_id, "planner")
            .await
            .expect("ensure shared thread target");
        let authz = build_authz();
        let token = issue_token(&authz, InternalRole::Leader, None, Some(&run.id));
        let service = TeamInternalControlService::new(
            state.clone(),
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );

        let authority_message_id = 4242_i64;
        let send = TeamInternalControl::send_actor_message(
            &service,
            authenticated_request(
                SendActorMessageRequest {
                    run_id: run.id.clone(),
                    from_actor_id: "planner".to_string(),
                    to_actor_id: "reviewer".to_string(),
                    channel: "coordination".to_string(),
                    transport: "local".to_string(),
                    route_json: String::new(),
                    payload_json: json!({
                        "type": "chat_message",
                        "text": "@reviewer please inspect p2p relay",
                        "delivery_scope": "channel_broadcast",
                        "authority_message_id": authority_message_id,
                        "team_id": run.team_id,
                        "channel_conversation_id": conversation_id,
                        "task_id": task_id,
                        "channel_id": "all",
                        "mention_actor_ids": ["reviewer"],
                        "mentioned_actor_ids": ["reviewer"]
                    })
                    .to_string(),
                    idempotency_key: "internal-grpc-channel-replica-1".to_string(),
                    from_peer_id: "main".to_string(),
                    to_peer_id: "main".to_string(),
                    channel_id: String::new(),
                },
                &token,
            ),
        )
        .await
        .expect("send actor channel replica message")
        .into_inner();
        assert!(send.message_id > 0);

        let replica = sqlx::query(
            r#"
            SELECT authority_message_id, run_id, team_id, conversation_id, task_id, channel_id, from_actor_id, source_node_id, payload_json
            FROM team_channel_message_replicas
            WHERE authority_message_id = ?1
            "#,
        )
        .bind(authority_message_id)
        .fetch_one(&state.db)
        .await
        .expect("load channel replica row");
        assert_eq!(
            replica.get::<i64, _>("authority_message_id"),
            authority_message_id
        );
        assert_eq!(replica.get::<String, _>("run_id"), run.id);
        assert_eq!(replica.get::<String, _>("channel_id"), "all");
        assert_eq!(replica.get::<String, _>("from_actor_id"), "planner");
        assert_eq!(replica.get::<String, _>("source_node_id"), "main");
        let payload: serde_json::Value =
            serde_json::from_str(replica.get::<String, _>("payload_json").as_str())
                .expect("decode replica payload");
        assert_eq!(payload["delivery_scope"], json!("channel_broadcast"));
        assert_eq!(payload["mention_actor_ids"], json!(["reviewer"]));
    }

    #[tokio::test]
    async fn internal_grpc_mailbox_send_rejects_mismatched_channel_replica_context() {
        let state = build_test_state().await;
        let run = create_team_run(&state).await;
        let authz = build_authz();
        let token = issue_token(&authz, InternalRole::Leader, None, Some(&run.id));
        let service = TeamInternalControlService::new(
            state,
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );

        let err = TeamInternalControl::send_actor_message(
            &service,
            authenticated_request(
                SendActorMessageRequest {
                    run_id: run.id,
                    from_actor_id: "planner".to_string(),
                    to_actor_id: "reviewer".to_string(),
                    channel: "coordination".to_string(),
                    transport: "local".to_string(),
                    route_json: String::new(),
                    payload_json: json!({
                        "type": "chat_message",
                        "text": "@reviewer please inspect p2p relay",
                        "delivery_scope": "channel_broadcast",
                        "authority_message_id": 999_i64,
                        "team_id": "wrong-team",
                        "channel_conversation_id": "conversation-all",
                        "task_id": "task-all",
                        "channel_id": "all"
                    })
                    .to_string(),
                    idempotency_key: "internal-grpc-channel-replica-bad-1".to_string(),
                    from_peer_id: "main".to_string(),
                    to_peer_id: "main".to_string(),
                    channel_id: String::new(),
                },
                &token,
            ),
        )
        .await
        .expect_err("mismatched replica payload should fail");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(
            err.message()
                .contains("channel replica payload does not match run/team context"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn issue_node_credential_returns_phase0_metadata() {
        let state = build_test_state().await;
        let authz = build_authz();
        let service = TeamInternalControlService::new(
            state,
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );
        let mut request = Request::new(IssueNodeCredentialRequest {
            node_id: "node-a".to_string(),
            role: "leader".to_string(),
            actor_id: String::new(),
            run_id: String::new(),
            permissions: vec![InternalAction::AgentManage.as_str().to_string()],
            ttl_seconds: 600,
        });
        request.metadata_mut().insert(
            BOOTSTRAP_TOKEN_HEADER,
            MetadataValue::try_from("bootstrap-token").expect("bootstrap metadata"),
        );

        let response = TeamInternalControl::issue_node_credential(&service, request)
            .await
            .expect("issue node credential")
            .into_inner();
        assert_eq!(response.node_id, "node-a");
        assert_eq!(response.source_node_id, "node-a");
        assert_eq!(response.cluster_id, "agenthub");
        assert_eq!(response.scope, vec!["agent:manage", "node:p2p"]);
        assert_eq!(response.audience, vec!["agenthub-internal"]);
        assert!(response.kid.starts_with("shared-hs256-"));
        assert!(response.issued_at > 0);
        assert!(response.expires_at > response.issued_at);
    }

    #[tokio::test]
    async fn issue_node_credential_rejects_bootstrap_token_mismatch() {
        let state = build_test_state().await;
        let authz = build_authz();
        let service = TeamInternalControlService::new(
            state,
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );
        let mut request = Request::new(IssueNodeCredentialRequest {
            node_id: "node-a".to_string(),
            role: "leader".to_string(),
            actor_id: String::new(),
            run_id: String::new(),
            permissions: vec![InternalAction::AgentManage.as_str().to_string()],
            ttl_seconds: 600,
        });
        request.metadata_mut().insert(
            BOOTSTRAP_TOKEN_HEADER,
            MetadataValue::try_from("wrong-token").expect("bootstrap metadata"),
        );

        let err = TeamInternalControl::issue_node_credential(&service, request)
            .await
            .expect_err("mismatched bootstrap token should fail");
        assert_eq!(err.code(), Code::PermissionDenied);
        assert_eq!(err.message(), "bootstrap token mismatch");
    }

    #[tokio::test]
    async fn issue_node_credential_requires_worker_actor_and_run() {
        let state = build_test_state().await;
        let authz = build_authz();
        let service = TeamInternalControlService::new(
            state,
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );

        let mut missing_actor_request = Request::new(IssueNodeCredentialRequest {
            node_id: "node-a".to_string(),
            role: "worker".to_string(),
            actor_id: String::new(),
            run_id: "run-1".to_string(),
            permissions: vec![InternalAction::AgentManage.as_str().to_string()],
            ttl_seconds: 600,
        });
        missing_actor_request.metadata_mut().insert(
            BOOTSTRAP_TOKEN_HEADER,
            MetadataValue::try_from("bootstrap-token").expect("bootstrap metadata"),
        );
        let missing_actor_err =
            TeamInternalControl::issue_node_credential(&service, missing_actor_request)
                .await
                .expect_err("worker bootstrap should require actor_id");
        assert_eq!(missing_actor_err.code(), Code::InvalidArgument);
        assert_eq!(
            missing_actor_err.message(),
            "worker bootstrap requires actor_id"
        );

        let mut missing_run_request = Request::new(IssueNodeCredentialRequest {
            node_id: "node-a".to_string(),
            role: "worker".to_string(),
            actor_id: "worker-a".to_string(),
            run_id: String::new(),
            permissions: vec![InternalAction::AgentManage.as_str().to_string()],
            ttl_seconds: 600,
        });
        missing_run_request.metadata_mut().insert(
            BOOTSTRAP_TOKEN_HEADER,
            MetadataValue::try_from("bootstrap-token").expect("bootstrap metadata"),
        );
        let missing_run_err =
            TeamInternalControl::issue_node_credential(&service, missing_run_request)
                .await
                .expect_err("worker bootstrap should require run_id");
        assert_eq!(missing_run_err.code(), Code::InvalidArgument);
        assert_eq!(
            missing_run_err.message(),
            "worker bootstrap requires run_id"
        );
    }

    #[tokio::test]
    async fn ack_actor_message_rejects_non_positive_message_id() {
        let state = build_test_state().await;
        let authz = build_authz();
        let token = issue_token(&authz, InternalRole::Leader, None, None);
        let service = TeamInternalControlService::new(
            state,
            authz,
            super::InternalGrpcSecurityMode::Disabled,
            std::env::temp_dir(),
            "bootstrap-token".to_string(),
        );

        let err = TeamInternalControl::ack_actor_message(
            &service,
            authenticated_request(
                AckActorMessageRequest {
                    run_id: "run-id".to_string(),
                    actor_id: "actor-id".to_string(),
                    message_id: 0,
                },
                &token,
            ),
        )
        .await
        .expect_err("message_id <= 0 should fail");
        assert_eq!(err.code(), Code::InvalidArgument);
        assert_eq!(err.message(), "message_id must be positive");
    }

    #[test]
    fn resolve_team_leader_member_id_supports_legacy_fallbacks() {
        assert_eq!(
            super::resolve_team_leader_member_id(&json!({
                "members":[{"member_id":"planner","role":"leader"}]
            }))
            .expect("resolve from role"),
            "planner"
        );
        assert_eq!(
            super::resolve_team_leader_member_id(&json!({
                "entrypoint":"planner"
            }))
            .expect("resolve from entrypoint"),
            "planner"
        );
    }

    #[test]
    fn actor_service_error_code_maps_to_expected_grpc_status() {
        let cases = [
            (
                agenthub_team_actor::ActorServiceErrorCode::BadRequest,
                Code::InvalidArgument,
            ),
            (
                agenthub_team_actor::ActorServiceErrorCode::UnprocessableEntity,
                Code::InvalidArgument,
            ),
            (
                agenthub_team_actor::ActorServiceErrorCode::Unauthorized,
                Code::Unauthenticated,
            ),
            (
                agenthub_team_actor::ActorServiceErrorCode::Forbidden,
                Code::PermissionDenied,
            ),
            (
                agenthub_team_actor::ActorServiceErrorCode::NotFound,
                Code::NotFound,
            ),
            (
                agenthub_team_actor::ActorServiceErrorCode::Conflict,
                Code::AlreadyExists,
            ),
            (
                agenthub_team_actor::ActorServiceErrorCode::Gone,
                Code::FailedPrecondition,
            ),
            (
                agenthub_team_actor::ActorServiceErrorCode::TooManyRequests,
                Code::ResourceExhausted,
            ),
            (
                agenthub_team_actor::ActorServiceErrorCode::Internal,
                Code::Internal,
            ),
        ];

        for (actor_code, grpc_code) in cases {
            let status = map_actor_service_status(agenthub_team_actor::ActorServiceError::new(
                actor_code, "boom",
            ));
            assert_eq!(status.code(), grpc_code);
        }
    }
}
