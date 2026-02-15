use std::path::PathBuf;

use agenthub_team_actor::parse_actor_transport;
use serde_json::Value;
use tonic::{Request, Response, Status, metadata::MetadataMap};

use crate::state::AppState;
use crate::team::{TeamManager, TeamStepRecord, TeamStepStatus};

use super::auth::{InternalAction, InternalAuthz, InternalRole};
use super::proto::agenthub::internal::v1::team_internal_control_server::TeamInternalControl;
use super::proto::agenthub::internal::v1::{
    AckActorMessageRequest, AckActorMessageResponse, ActorMessage, IssueNodeCredentialRequest,
    IssueNodeCredentialResponse, ListActorInboxRequest, ListActorInboxResponse,
    SendActorMessageRequest, SendActorMessageResponse, TransitionStepRequest,
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
            .ensure_permission(&principal, InternalAction::TeamMessageSend)?;
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

        let message = self
            .state
            .teams
            .send_actor_message(
                run_id,
                from_actor_id,
                to_actor_id,
                channel,
                transport,
                route,
                payload_json,
                idempotency_key,
            )
            .await
            .map_err(|err| {
                if TeamManager::is_actor_message_idempotency_conflict(&err) {
                    return Status::already_exists(
                        "idempotency key already exists with different payload",
                    );
                }
                map_manager_error(err)
            })?;

        Ok(Response::new(SendActorMessageResponse {
            message_id: message.message_id,
            status: message.status.as_str().to_string(),
            idempotency_key: idempotency_key.unwrap_or("").to_string(),
        }))
    }

    async fn list_actor_inbox(
        &self,
        request: Request<ListActorInboxRequest>,
    ) -> Result<Response<ListActorInboxResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamInboxList)?;
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

        let messages = self
            .state
            .teams
            .list_actor_inbox(run_id, actor_id, limit, after_id, payload.include_delivered)
            .await
            .map_err(map_manager_error)?;
        let messages = messages
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
            .ensure_permission(&principal, InternalAction::TeamMessageAck)?;
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
            .ack_actor_message(run_id, actor_id, payload.message_id)
            .await
            .map_err(map_manager_error)?;

        Ok(Response::new(AckActorMessageResponse {
            message: Some(ActorMessage {
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
            }),
        }))
    }

    async fn transition_step(
        &self,
        request: Request<TransitionStepRequest>,
    ) -> Result<Response<TransitionStepResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamStepTransition)?;
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
                            .map(|raw| serde_json::from_str::<Value>(raw))
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
                            .map(|raw| serde_json::from_str::<Value>(raw))
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
                            .map(|raw| serde_json::from_str::<Value>(raw))
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
        let (access_token, expires_at) = self
            .authz
            .issue_access_token(role, actor_id, run_id, permissions, ttl_seconds)
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
            access_token,
            expires_at,
            cert_pem,
            key_pem,
            ca_cert_pem,
            security_mode: security_mode_to_str(self.security_mode).to_string(),
        }))
    }
}

fn step_to_transition_response(step: TeamStepRecord) -> TransitionStepResponse {
    TransitionStepResponse {
        step_id: step.id,
        run_id: step.run_id,
        step_key: step.step_key,
        member_id: step.member_id,
        remote_task_id: step.remote_task_id.unwrap_or_default(),
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
            InternalAction::TeamMessageSend.as_str().to_string(),
            InternalAction::TeamInboxList.as_str().to_string(),
            InternalAction::TeamMessageAck.as_str().to_string(),
            InternalAction::TeamStepTransition.as_str().to_string(),
            InternalAction::TeamNodeIssue.as_str().to_string(),
        ],
        InternalRole::Worker => vec![
            InternalAction::TeamMessageSend.as_str().to_string(),
            InternalAction::TeamInboxList.as_str().to_string(),
            InternalAction::TeamMessageAck.as_str().to_string(),
        ],
        InternalRole::Orchestrator => vec![
            InternalAction::TeamMessageSend.as_str().to_string(),
            InternalAction::TeamInboxList.as_str().to_string(),
            InternalAction::TeamMessageAck.as_str().to_string(),
            InternalAction::TeamStepTransition.as_str().to_string(),
            InternalAction::TeamNodeIssue.as_str().to_string(),
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
                || permission == InternalAction::TeamStepTransition.as_str()
                || permission == InternalAction::TeamNodeIssue.as_str()
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
