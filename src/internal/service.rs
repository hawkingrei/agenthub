use std::path::PathBuf;

use agenthub_team_actor::{
    ActorAckRequest, ActorInboxRequest, ActorMailboxService, ActorMessageStatus, ActorSendRequest,
    ActorServiceError, ActorServiceErrorCode, parse_actor_transport,
};
use serde_json::Value;
use tonic::{Request, Response, Status, metadata::MetadataMap};

use crate::state::AppState;
use crate::team::{TeamStepRecord, TeamStepStatus};
use crate::{acp::AcpActorSkillContext, agent::AgentConfig};

use super::auth::{InternalAction, InternalAuthz, InternalRole};
use super::p2p::{CredentialProvider, NodeCredentialRequest};
use super::proto::agenthub::internal::v1::team_internal_control_server::TeamInternalControl;
use super::proto::agenthub::internal::v1::{
    AckActorMessageRequest, AckActorMessageResponse, ActorMessage, AgentEventRecord,
    DeleteManagedAgentRequest, DeleteManagedAgentResponse, EnsureAgentRecordRequest,
    EnsureAgentRecordResponse, GetAgentRecordRequest, GetAgentRecordResponse,
    IssueNodeCredentialRequest, IssueNodeCredentialResponse, ListActorInboxRequest,
    ListActorInboxResponse, ListAgentEventsRequest, ListAgentEventsResponse,
    SendActorMessageRequest, SendActorMessageResponse, SendAgentInputRequest,
    SendAgentInputResponse, StartManagedAgentRequest, StartManagedAgentResponse,
    StopManagedAgentRequest, StopManagedAgentResponse, TransitionStepRequest,
    TransitionStepResponse,
};
use super::tls::{InternalGrpcSecurityMode, load_bootstrap_client_identity};

const BOOTSTRAP_TOKEN_HEADER: &str = "x-agenthub-bootstrap-token";
const DEFAULT_TOKEN_TTL_SECONDS: i64 = 3600;
const MAX_TOKEN_TTL_SECONDS: i64 = 24 * 60 * 60;

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

        let to_actor_id = required_field(&payload.to_actor_id, "to_actor_id")?;
        let channel = optional_trimmed(&payload.channel).unwrap_or("default");
        let transport = parse_actor_transport(optional_trimmed(&payload.transport))
            .map_err(|err| Status::invalid_argument(err.to_string()))?;
        let route = optional_json_object(optional_trimmed(&payload.route_json), "route_json")?;
        let payload_json = parse_json_required(&payload.payload_json, "payload_json")?;
        let idempotency_key = optional_trimmed(&payload.idempotency_key);
        let from_peer_id = optional_trimmed(&payload.from_peer_id);
        let to_peer_id = optional_trimmed(&payload.to_peer_id);

        let message = self
            .state
            .teams
            .actor_mailbox_service()
            .actor_send(ActorSendRequest {
                run_id: run_id.to_string(),
                from_actor_id: from_actor_id.to_string(),
                from_peer_id: from_peer_id.map(str::to_string),
                to_actor_id: to_actor_id.to_string(),
                to_peer_id: to_peer_id.map(str::to_string),
                channel: Some(channel.to_string()),
                transport: Some(transport),
                route,
                payload: payload_json,
                idempotency_key: idempotency_key.map(str::to_string),
            })
            .await
            .map_err(map_actor_service_status)?;

        Ok(Response::new(SendActorMessageResponse {
            message_id: message.message_id,
            status: message.state.as_str().to_string(),
            idempotency_key: idempotency_key.unwrap_or("").to_string(),
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
        let messages = self
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
            .map_err(map_actor_service_status)?
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

        Ok(Response::new(ListActorInboxResponse { messages }))
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
            InternalAction::StepTransition.as_str().to_string(),
            InternalAction::NodeIssue.as_str().to_string(),
        ],
        InternalRole::Worker => vec![
            InternalAction::MessageSend.as_str().to_string(),
            InternalAction::InboxList.as_str().to_string(),
            InternalAction::MessageAck.as_str().to_string(),
        ],
        InternalRole::Orchestrator => vec![
            InternalAction::MessageSend.as_str().to_string(),
            InternalAction::InboxList.as_str().to_string(),
            InternalAction::MessageAck.as_str().to_string(),
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
    use serde_json::json;
    use tonic::{Code, Request, metadata::MetadataValue};
    use uuid::Uuid;

    use super::super::auth::{InternalAction, InternalAuthz, InternalAuthzConfig, InternalRole};
    use super::super::proto::agenthub::internal::v1::team_internal_control_server::TeamInternalControl;
    use super::super::proto::agenthub::internal::v1::{
        AckActorMessageRequest, IssueNodeCredentialRequest, ListActorInboxRequest,
        SendActorMessageRequest,
    };
    use super::{BOOTSTRAP_TOKEN_HEADER, TeamInternalControlService, map_actor_service_status};
    use crate::api::team_tests::build_test_state;
    use crate::team::TeamDefinitionConfig;

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
                    "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
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
