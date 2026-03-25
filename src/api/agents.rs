use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{delete, get, post},
};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::acp::{
    AcpActorSkillContext, AcpPermissionRecord, AcpPermissionRespondResult, DEFAULT_ACTOR_CHANNEL,
    default_actor_cli_path, normalize_actor_cli_path,
};
use crate::agent::{
    AgentConfig, AgentRecord, AgentSendInputError, AgentTimeTriggerCreateInput,
    AgentTimeTriggerManager, AgentTimeTriggerRecord, WorktreeMode, normalize_target_node_id,
};
use crate::api::authz::require_user;
use crate::api::error::ApiError;
use crate::api::teams::prune_deleted_agent_from_team_specs;
use crate::state::AppState;

const AGENT_SOURCE_MANUAL: &str = "manual";
const AGENT_SOURCE_TEAM_FORGE: &str = "team_forge";

#[derive(Debug, serde::Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub workdir: String,
    pub command: String,
    pub args: Vec<String>,
    pub target_node_id: Option<String>,
    pub source: Option<String>,
    pub worktree_mode: Option<String>,
    pub worktree_repo: Option<String>,
    pub worktree_ref: Option<String>,
    pub code_mode: Option<bool>,
    pub agent_loop_enabled: Option<bool>,
    pub agent_loop_idle_seconds: Option<i64>,
    pub agent_loop_prompt: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct StartAgentResponse {
    pub session_id: String,
}

#[derive(Debug, serde::Serialize)]
pub struct AgentDiscoveryCardResponse {
    pub card_id: String,
    pub schema_version: String,
    pub description: String,
    pub identity: AgentDiscoveryIdentity,
    pub runtime: AgentDiscoveryRuntime,
    pub capability_tags: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct AgentDiscoveryIdentity {
    pub agent_id: String,
    pub name: String,
    pub status: crate::agent::AgentStatus,
}

#[derive(Debug, serde::Serialize)]
pub struct AgentDiscoveryRuntime {
    pub acp_provider: Option<String>,
    pub code_mode: bool,
    pub agent_loop_enabled: bool,
    pub agent_loop_idle_seconds: Option<i64>,
    pub target_node_id: Option<String>,
    pub worktree_mode: WorktreeMode,
    pub worktree_repo: Option<String>,
    pub worktree_ref: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct StartAgentRequest {
    pub actor_runtime: Option<StartAgentActorRuntimeRequest>,
}

#[derive(Debug, serde::Deserialize)]
pub struct StartAgentActorRuntimeRequest {
    pub team_id: Option<String>,
    pub run_id: Option<String>,
    pub actor_id: String,
    pub member_role: Option<String>,
    pub channel: Option<String>,
    pub actor_cli_path: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ListEventsQuery {
    pub limit: Option<i64>,
    pub session_id: Option<String>,
    pub before_id: Option<i64>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SetCodeModeRequest {
    pub code_mode: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct SetAgentLoopRequest {
    pub enabled: bool,
    pub idle_seconds: Option<i64>,
    pub prompt: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SendInputRequest {
    pub input: String,
    pub message_id: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateAgentTimeTriggerRequest {
    pub delay_seconds: i64,
    pub message: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ListAgentTimeTriggersQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ClearSessionRequest {
    pub provider: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SetAcpModeRequest {
    pub mode_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct SetAcpModelRequest {
    pub model_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct SetAcpConfigRequest {
    pub config_id: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct PermissionListQuery {
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PermissionResponseRequest {
    pub option_id: Option<String>,
    pub outcome: Option<String>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", post(create_agent).get(list_agents))
        .route("/{id}", get(get_agent))
        .route(
            "/{id}/.well-known/agent-card",
            get(get_agent_discovery_card),
        )
        .route("/{id}/start", post(start_agent))
        .route("/{id}/stop", post(stop_agent))
        .route("/{id}/input", post(send_input))
        .route(
            "/{id}/triggers",
            post(create_agent_time_trigger).get(list_agent_time_triggers),
        )
        .route(
            "/{id}/triggers/{trigger_id}/cancel",
            post(cancel_agent_time_trigger),
        )
        .route("/{id}", delete(delete_agent))
        .route("/{id}/events", get(list_events))
        .route("/{id}/code_mode", post(set_code_mode))
        .route("/{id}/agent_loop", post(set_agent_loop))
        .route("/{id}/acp/session/clear", post(clear_acp_session))
        .route("/{id}/acp/mode", post(set_acp_mode))
        .route("/{id}/acp/model", post(set_acp_model))
        .route("/{id}/acp/config", post(set_acp_config))
        .route("/{id}/acp/cancel", post(cancel_acp))
        .route("/{id}/permissions", get(list_permissions))
        .route(
            "/{id}/permissions/{permission_id}/respond",
            post(respond_permission),
        )
        .with_state(state)
}

fn map_create_agent_error(err: anyhow::Error) -> ApiError {
    let message = err.to_string();
    if message.contains("not found")
        || message.contains("required")
        || message.contains("must ")
        || message.contains("invalid ")
        || message.contains("reserved")
        || message.contains("legacy schema")
        || message.contains("internal gRPC peer config")
    {
        return ApiError::bad_request(&message);
    }
    ApiError::from(err)
}

async fn create_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAgentRequest>,
) -> Result<Json<AgentRecord>, ApiError> {
    let user = require_user(&headers, &state).await?;
    let source = parse_agent_source(payload.source.as_deref())?;
    let target_node_id = normalize_target_node_id(payload.target_node_id.as_deref());
    if target_node_id.is_some() && user.role != "root" {
        return Err(ApiError::unauthorized(
            "root required for remote target node",
        ));
    }
    let worktree_mode = parse_worktree_mode(payload.worktree_mode.as_deref());
    let default_worktree_root = resolve_create_agent_default_worktree_root(
        &state,
        target_node_id.as_deref(),
        &worktree_mode,
    )
    .await?;
    let workdir = resolve_create_agent_workdir(
        &payload.workdir,
        &payload.name,
        &worktree_mode,
        default_worktree_root.as_deref(),
    )?;
    let config = AgentConfig {
        name: payload.name,
        workdir,
        command: payload.command,
        args: payload.args,
        target_node_id,
        worktree_mode,
        worktree_repo: payload.worktree_repo,
        worktree_ref: payload.worktree_ref,
        code_mode: payload.code_mode.unwrap_or(true),
        agent_loop_enabled: payload.agent_loop_enabled.unwrap_or(false),
        agent_loop_idle_seconds: payload.agent_loop_idle_seconds,
        agent_loop_prompt: payload.agent_loop_prompt,
    };
    let agent = if source == AGENT_SOURCE_MANUAL {
        state
            .agents
            .create_agent(config)
            .await
            .map_err(map_create_agent_error)?
    } else {
        state
            .agents
            .create_agent_with_source(config, source)
            .await
            .map_err(map_create_agent_error)?
    };
    Ok(Json(agent))
}

async fn list_agents(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentRecord>>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let agents = state.agents.list_agents().await?;
    Ok(Json(agents))
}

async fn get_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Json<AgentRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let agent = state.agents.get_agent(&agent_id).await?;
    Ok(Json(agent))
}

async fn get_agent_discovery_card(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Json<AgentDiscoveryCardResponse>, ApiError> {
    let user = require_user(&headers, &state).await?;
    let agent = state.agents.get_agent(&agent_id).await?;
    let provider = state
        .agents
        .acp_provider_for_agent(&agent.command, &agent.args);
    let member_description =
        resolve_team_member_description(&state, user.id.as_str(), &agent.id).await;
    Ok(Json(build_agent_discovery_card(
        &agent,
        provider,
        member_description.as_deref(),
    )))
}

async fn start_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    body: Bytes,
) -> Result<Json<StartAgentResponse>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let payload = parse_optional_start_agent_request(body)?;
    let actor_context = parse_start_actor_runtime_context(payload)?;
    if actor_context.is_some() {
        return Err(ApiError::bad_request(
            "actor_runtime is reserved for team orchestrator and is not supported by /api/agents/:id/start",
        ));
    }
    let session_id = state.agents.start_agent(&agent_id).await?;
    Ok(Json(StartAgentResponse { session_id }))
}

async fn stop_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state.agents.stop_agent(&agent_id).await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn delete_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let _ = state.agents.stop_agent(&agent_id).await;
    state.agents.delete_agent(&agent_id).await?;
    if let Err(err) = prune_deleted_agent_from_team_specs(&state, &agent_id).await {
        tracing::warn!(
            agent_id = %agent_id,
            error = ?err,
            "delete_agent completed after best-effort team spec prune failed"
        );
    }
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn send_input(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SendInputRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    match state
        .agents
        .send_input(
            &agent_id,
            &payload.input,
            payload.message_id.as_deref(),
            payload.session_id.as_deref(),
        )
        .await
    {
        Ok(()) => {}
        Err(err) => {
            if let Some(AgentSendInputError::SessionMismatch { .. }) =
                err.downcast_ref::<AgentSendInputError>()
            {
                return Err(ApiError::conflict(&err.to_string()));
            }
            return Err(err.into());
        }
    }
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn create_agent_time_trigger(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<CreateAgentTimeTriggerRequest>,
) -> Result<Json<AgentTimeTriggerRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    ensure_agent_exists(&state, &agent_id).await?;
    if !(1..=60 * 60 * 24 * 30).contains(&payload.delay_seconds) {
        return Err(ApiError::bad_request(
            "delay_seconds must be between 1 and 2592000",
        ));
    }
    let delay_seconds = payload.delay_seconds;
    let fire_at = chrono::Utc::now().timestamp() + delay_seconds;
    let manager = AgentTimeTriggerManager::new(state.db.clone());
    let record = manager
        .create_time_trigger(AgentTimeTriggerCreateInput {
            agent_id: agent_id.clone(),
            created_by_actor_id: agent_id,
            message_text: payload.message,
            fire_at,
        })
        .await?;
    Ok(Json(record))
}

async fn list_agent_time_triggers(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Query(query): Query<ListAgentTimeTriggersQuery>,
) -> Result<Json<Vec<AgentTimeTriggerRecord>>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    ensure_agent_exists(&state, &agent_id).await?;
    let manager = AgentTimeTriggerManager::new(state.db.clone());
    let records = manager
        .list_triggers_for_agent(&agent_id, query.limit.unwrap_or(50))
        .await?;
    Ok(Json(records))
}

async fn cancel_agent_time_trigger(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((agent_id, trigger_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    ensure_agent_exists(&state, &agent_id).await?;
    let manager = AgentTimeTriggerManager::new(state.db.clone());
    let canceled = manager.cancel_trigger(&agent_id, &trigger_id).await?;
    if !canceled {
        return Err(ApiError::not_found("agent time trigger not found"));
    }
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn list_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Query(query): Query<ListEventsQuery>,
) -> Result<Json<Vec<crate::agent::AgentEvent>>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let limit = query.limit.unwrap_or(500).clamp(1, 1000);
    let before_id = query.before_id;
    let events = if let Some(session_id) = query.session_id.as_deref() {
        state
            .agents
            .list_events_for_session(&agent_id, session_id, limit, before_id)
            .await?
    } else {
        state
            .agents
            .list_events(&agent_id, limit, before_id)
            .await?
    };
    Ok(Json(events))
}

async fn set_code_mode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetCodeModeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state
        .agents
        .set_code_mode(&agent_id, payload.code_mode)
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn set_agent_loop(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetAgentLoopRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let prompt = payload
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if payload.enabled && prompt.is_none() {
        return Err(ApiError::bad_request(
            "agent_loop.prompt is required when enabling agent loop",
        ));
    }
    if payload.enabled
        && !payload
            .idle_seconds
            .is_some_and(|value| (10..=86_400).contains(&value))
    {
        return Err(ApiError::bad_request(
            "agent_loop.idle_seconds must be between 10 and 86400 when enabling agent loop",
        ));
    }
    state
        .agents
        .set_agent_loop_config(
            &agent_id,
            payload.enabled,
            payload.idle_seconds,
            prompt.as_deref(),
        )
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn clear_acp_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<ClearSessionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let provider = match payload.provider {
        Some(provider) => provider,
        None => match state.agents.get_agent(&agent_id).await {
            Ok(agent) => state
                .agents
                .acp_provider_for_agent(&agent.command, &agent.args)
                .unwrap_or("codex")
                .to_string(),
            Err(err) => {
                if err
                    .downcast_ref::<sqlx::Error>()
                    .map(|e| matches!(e, sqlx::Error::RowNotFound))
                    .unwrap_or(false)
                {
                    "codex".to_string()
                } else {
                    return Err(err.into());
                }
            }
        },
    };
    state
        .agents
        .clear_persistent_session(&agent_id, &provider)
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn set_acp_mode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetAcpModeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state
        .agents
        .set_acp_mode(&agent_id, &payload.mode_id)
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn set_acp_model(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetAcpModelRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state
        .agents
        .set_acp_model(&agent_id, &payload.model_id)
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn set_acp_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetAcpConfigRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state
        .agents
        .set_acp_config(&agent_id, &payload.config_id, &payload.value)
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn cancel_acp(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state.agents.cancel_acp(&agent_id).await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn list_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Query(query): Query<PermissionListQuery>,
) -> Result<Json<Vec<AcpPermissionRecord>>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let status = query.status.as_deref();
    let records = state.acp_permissions.list(&agent_id, status).await?;
    Ok(Json(records))
}

async fn ensure_agent_exists(state: &AppState, agent_id: &str) -> Result<(), ApiError> {
    state
        .agents
        .get_agent(agent_id)
        .await
        .map(|_| ())
        .map_err(|error| {
            if error
                .downcast_ref::<sqlx::Error>()
                .is_some_and(|inner| matches!(inner, sqlx::Error::RowNotFound))
            {
                ApiError::not_found("agent not found")
            } else {
                error.into()
            }
        })
}

async fn respond_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((agent_id, permission_id)): Path<(String, String)>,
    Json(payload): Json<PermissionResponseRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    if !state
        .acp_permissions
        .belongs_to_agent(&permission_id, &agent_id)
        .await?
    {
        return Err(ApiError::not_found(
            "permission request not found for agent",
        ));
    }
    let outcome = if let Some(option_id) = payload.option_id.as_ref() {
        agent_client_protocol::RequestPermissionOutcome::Selected(
            agent_client_protocol::SelectedPermissionOutcome::new(option_id.clone()),
        )
    } else {
        match payload.outcome.as_deref() {
            Some("cancelled") | None => agent_client_protocol::RequestPermissionOutcome::Cancelled,
            Some(_other) => {
                return Err(ApiError::bad_request(
                    "unsupported outcome, expected 'cancelled'",
                ));
            }
        }
    };
    let respond_result = state
        .acp_permissions
        .respond(&permission_id, outcome, payload.option_id, None)
        .await?;
    let status = match respond_result {
        AcpPermissionRespondResult::Applied => "ok",
        AcpPermissionRespondResult::AlreadyResolved => "already_resolved",
    };
    Ok(Json(serde_json::json!({ "status": status })))
}

fn parse_worktree_mode(value: Option<&str>) -> WorktreeMode {
    match value {
        Some("create_worktree") => WorktreeMode::CreateWorktree,
        Some("reuse_worktree") => WorktreeMode::ReuseWorktree,
        _ => WorktreeMode::UseExisting,
    }
}

fn parse_agent_source(value: Option<&str>) -> Result<&'static str, ApiError> {
    let normalized = value
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .unwrap_or(AGENT_SOURCE_MANUAL)
        .to_ascii_lowercase();
    match normalized.as_str() {
        AGENT_SOURCE_MANUAL => Ok(AGENT_SOURCE_MANUAL),
        AGENT_SOURCE_TEAM_FORGE => Ok(AGENT_SOURCE_TEAM_FORGE),
        _ => Err(ApiError::bad_request(
            "source must be one of: manual, team_forge",
        )),
    }
}

async fn resolve_create_agent_default_worktree_root(
    state: &AppState,
    target_node_id: Option<&str>,
    worktree_mode: &WorktreeMode,
) -> Result<Option<String>, ApiError> {
    if !matches!(worktree_mode, WorktreeMode::CreateWorktree) {
        return Ok(None);
    }
    let Some(target_node_id) = normalize_target_node_id(target_node_id) else {
        return Ok(Some(state.default_worktree_root.clone()));
    };
    let node = state
        .agents
        .get_agent_node(&target_node_id)
        .await
        .map_err(map_create_agent_error)?;
    Ok(node.default_worktree_root)
}

fn resolve_create_agent_workdir(
    requested_workdir: &str,
    agent_name: &str,
    worktree_mode: &WorktreeMode,
    default_worktree_root: Option<&str>,
) -> Result<String, ApiError> {
    let trimmed = requested_workdir.trim();
    if !trimmed.is_empty() {
        return Ok(trimmed.to_string());
    }
    if !matches!(worktree_mode, WorktreeMode::CreateWorktree) {
        return Err(ApiError::bad_request("workdir is required"));
    }
    let Some(default_worktree_root) = default_worktree_root
        .map(str::trim)
        .filter(|root| !root.is_empty())
    else {
        return Err(ApiError::bad_request(
            "workdir is required for remote-target agents unless the selected node defines default_worktree_root",
        ));
    };
    Ok(default_worktree_path(agent_name, default_worktree_root))
}

fn default_worktree_path(agent_name: &str, default_worktree_root: &str) -> String {
    let root = default_worktree_root
        .trim()
        .trim_end_matches('/')
        .trim_end_matches('\\');
    let name = sanitize_worktree_segment(agent_name);
    let suffix = Uuid::new_v4().simple().to_string();
    let segment = format!("{name}-{}", &suffix[..8]);
    std::path::PathBuf::from(root)
        .join(segment)
        .to_string_lossy()
        .to_string()
}

fn sanitize_worktree_segment(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
            continue;
        }
        if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "agent".to_string()
    } else {
        trimmed.to_string()
    }
}

fn build_agent_discovery_card(
    agent: &AgentRecord,
    acp_provider: Option<&str>,
    member_description: Option<&str>,
) -> AgentDiscoveryCardResponse {
    let mut capability_tags = vec![
        "team_mailbox_v1".to_string(),
        "team_step_execution_v1".to_string(),
    ];
    if agent.code_mode {
        capability_tags.push("code_mode".to_string());
    }
    if agent.agent_loop_enabled {
        capability_tags.push("agent_loop".to_string());
    }
    if matches!(
        agent.worktree_mode,
        WorktreeMode::CreateWorktree | WorktreeMode::ReuseWorktree
    ) {
        capability_tags.push("git_worktree".to_string());
    }
    if let Some(provider) = acp_provider {
        capability_tags.push(format!("acp_{provider}"));
    }
    let provider_desc = acp_provider.unwrap_or("unknown");
    let capability_desc = capability_tags.join(", ");
    let description = member_description
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "AgentHub team member {} (provider: {}) supports {}",
                agent.name, provider_desc, capability_desc
            )
        });

    AgentDiscoveryCardResponse {
        card_id: format!("agenthub://agents/{}", agent.id),
        schema_version: "agenthub.a2a.discovery_card.v1".to_string(),
        description,
        identity: AgentDiscoveryIdentity {
            agent_id: agent.id.clone(),
            name: agent.name.clone(),
            status: agent.status.clone(),
        },
        runtime: AgentDiscoveryRuntime {
            acp_provider: acp_provider.map(str::to_string),
            code_mode: agent.code_mode,
            agent_loop_enabled: agent.agent_loop_enabled,
            agent_loop_idle_seconds: agent.agent_loop_idle_seconds,
            target_node_id: agent.target_node_id.clone(),
            worktree_mode: agent.worktree_mode.clone(),
            worktree_repo: agent.worktree_repo.clone(),
            worktree_ref: agent.worktree_ref.clone(),
        },
        capability_tags,
    }
}

async fn resolve_team_member_description(
    state: &AppState,
    user_id: &str,
    member_id: &str,
) -> Option<String> {
    let teams = state.teams.list_teams().await.ok()?;
    teams
        .into_iter()
        .filter(|team| match team.owner_user_id.as_deref() {
            Some(owner_user_id) => owner_user_id == user_id,
            None => true,
        })
        .filter_map(|team| {
            resolve_member_description_from_spec(&team.spec, member_id)
                .map(|description| (team.updated_at, description))
        })
        .max_by_key(|(updated_at, _)| *updated_at)
        .map(|(_, description)| description)
}

fn resolve_member_description_from_spec(spec: &Value, member_id: &str) -> Option<String> {
    let target = member_id.trim();
    if target.is_empty() {
        return None;
    }
    spec.get("members")
        .and_then(Value::as_array)
        .and_then(|members| {
            members.iter().find_map(|member| {
                let member_obj = member.as_object()?;
                let candidate_member_id = member_obj
                    .get("member_id")
                    .and_then(Value::as_str)
                    .map(str::trim)?;
                if candidate_member_id != target {
                    return None;
                }
                member_obj
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
        })
}

fn parse_start_actor_runtime_context(
    payload: Option<StartAgentRequest>,
) -> Result<Option<AcpActorSkillContext>, ApiError> {
    let Some(payload) = payload else {
        return Ok(None);
    };
    let Some(actor_runtime) = payload.actor_runtime else {
        return Ok(None);
    };

    let team_id = actor_runtime
        .team_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let current_run_id = actor_runtime
        .run_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if team_id.is_none() && current_run_id.is_none() {
        return Err(ApiError::bad_request(
            "actor_runtime.team_id or actor_runtime.run_id is required",
        ));
    }
    let actor_id = actor_runtime.actor_id.trim();
    if actor_id.is_empty() {
        return Err(ApiError::bad_request("actor_runtime.actor_id is required"));
    }
    let default_channel = actor_runtime
        .channel
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_ACTOR_CHANNEL)
        .to_string();
    let actor_cli_path = match actor_runtime.actor_cli_path.as_deref() {
        Some(path) => normalize_actor_cli_path(Some(path))
            .map_err(|err| ApiError::bad_request(err.to_string().as_str()))?,
        None => default_actor_cli_path()?,
    };

    Ok(Some(AcpActorSkillContext {
        team_id,
        current_run_id,
        actor_id: actor_id.to_string(),
        default_channel,
        actor_cli_path,
        member_role: actor_runtime
            .member_role
            .map(|value| value.trim().to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    }))
}

fn parse_optional_start_agent_request(body: Bytes) -> Result<Option<StartAgentRequest>, ApiError> {
    if body.is_empty() || body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Ok(None);
    }
    serde_json::from_slice::<StartAgentRequest>(&body)
        .map(Some)
        .map_err(|err| ApiError::bad_request(&format!("invalid request body JSON: {err}")))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;
    use std::process::Command as StdCommand;
    use std::sync::Arc;
    use std::time::Duration;

    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode, header};
    use axum::response::IntoResponse;
    use serde_json::{Value, json};
    use sqlx::Row;
    use sqlx::SqlitePool;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::acp::AcpPermissionService;
    use crate::acp::default_actor_cli_path;
    use crate::agent::AgentManager;
    use crate::auth::AuthService;
    use crate::internal::client::InternalGrpcPeerClientConfig;
    use crate::internal::tls::InternalGrpcSecurityMode;
    use crate::push::PushService;
    use crate::state::AppState;
    use crate::team::TeamManager;
    use agenthub_config::{AppConfig, PushConfig, WebConfig};

    use super::{
        StartAgentActorRuntimeRequest, StartAgentRequest, WorktreeMode, build_agent_discovery_card,
        map_create_agent_error, parse_agent_source, parse_optional_start_agent_request,
        parse_start_actor_runtime_context, parse_worktree_mode, resolve_create_agent_workdir,
        resolve_member_description_from_spec, router, sanitize_worktree_segment,
    };

    #[test]
    fn parse_worktree_mode_defaults() {
        assert!(matches!(
            parse_worktree_mode(None),
            WorktreeMode::UseExisting
        ));
        assert!(matches!(
            parse_worktree_mode(Some("unknown")),
            WorktreeMode::UseExisting
        ));
    }

    #[test]
    fn parse_worktree_mode_explicit() {
        assert!(matches!(
            parse_worktree_mode(Some("create_worktree")),
            WorktreeMode::CreateWorktree
        ));
        assert!(matches!(
            parse_worktree_mode(Some("reuse_worktree")),
            WorktreeMode::ReuseWorktree
        ));
    }

    #[test]
    fn parse_agent_source_defaults_and_validates() {
        assert_eq!(parse_agent_source(None).expect("default source"), "manual");
        assert_eq!(
            parse_agent_source(Some("team_forge")).expect("team forge source"),
            "team_forge"
        );
        assert!(parse_agent_source(Some("invalid")).is_err());
    }

    #[test]
    fn build_agent_discovery_card_includes_runtime_tags() {
        let agent = crate::agent::AgentRecord {
            id: "agent-1".to_string(),
            name: "Worker One".to_string(),
            workdir: "/tmp/agent-1".to_string(),
            command: "agenthub-codex-acp".to_string(),
            args: vec![],
            target_node_id: None,
            worktree_mode: WorktreeMode::CreateWorktree,
            worktree_repo: Some("/tmp/repo".to_string()),
            worktree_ref: Some("main".to_string()),
            code_mode: true,
            agent_loop_enabled: false,
            agent_loop_idle_seconds: None,
            agent_loop_prompt: None,
            status: crate::agent::AgentStatus::Running,
            created_at: 1,
            updated_at: 2,
        };

        let card = build_agent_discovery_card(&agent, Some("codex"), None);
        assert_eq!(card.card_id, "agenthub://agents/agent-1");
        assert_eq!(card.schema_version, "agenthub.a2a.discovery_card.v1");
        assert!(
            card.description
                .contains("AgentHub team member Worker One (provider: codex)"),
            "unexpected description: {}",
            card.description
        );
        assert_eq!(card.identity.agent_id, "agent-1");
        assert_eq!(card.identity.name, "Worker One");
        assert_eq!(card.runtime.acp_provider.as_deref(), Some("codex"));
        assert!(
            card.capability_tags
                .iter()
                .any(|tag| tag == "team_mailbox_v1")
        );
        assert!(
            card.capability_tags
                .iter()
                .any(|tag| tag == "team_step_execution_v1")
        );
        assert!(card.capability_tags.iter().any(|tag| tag == "code_mode"));
        assert!(card.capability_tags.iter().any(|tag| tag == "git_worktree"));
        assert!(card.capability_tags.iter().any(|tag| tag == "acp_codex"));
    }

    #[test]
    fn build_agent_discovery_card_prefers_member_description() {
        let agent = crate::agent::AgentRecord {
            id: "agent-2".to_string(),
            name: "Worker Two".to_string(),
            workdir: "/tmp/agent-2".to_string(),
            command: "agenthub-codex-acp".to_string(),
            args: vec![],
            target_node_id: None,
            worktree_mode: WorktreeMode::UseExisting,
            worktree_repo: None,
            worktree_ref: None,
            code_mode: false,
            agent_loop_enabled: false,
            agent_loop_idle_seconds: None,
            agent_loop_prompt: None,
            status: crate::agent::AgentStatus::Created,
            created_at: 1,
            updated_at: 2,
        };

        let card = build_agent_discovery_card(&agent, Some("codex"), Some("Database schema owner"));
        assert_eq!(card.description, "Database schema owner");
    }

    #[test]
    fn resolve_member_description_from_spec_matches_member_entry() {
        let spec = json!({
            "spec_version": 1,
            "members": [
                {"member_id": "leader", "role": "leader", "description": "Lead architect"},
                {"member_id": "worker-a", "role": "worker", "description": "Primary implementer"},
                {"member_id": "worker-b", "role": "worker"}
            ]
        });
        assert_eq!(
            resolve_member_description_from_spec(&spec, "worker-a").as_deref(),
            Some("Primary implementer")
        );
        assert_eq!(
            resolve_member_description_from_spec(&spec, "worker-b"),
            None
        );
        assert_eq!(resolve_member_description_from_spec(&spec, "missing"), None);
    }

    #[test]
    fn resolve_create_agent_workdir_uses_explicit_value() {
        let resolved = resolve_create_agent_workdir(
            " /tmp/work ",
            "planner",
            &WorktreeMode::CreateWorktree,
            Some("~/.agenthub/worktrees"),
        )
        .expect("resolve workdir");
        assert_eq!(resolved, "/tmp/work");
    }

    #[test]
    fn resolve_create_agent_workdir_generates_default_for_create_mode() {
        let resolved = resolve_create_agent_workdir(
            "",
            "Team Planner",
            &WorktreeMode::CreateWorktree,
            Some("~/.agenthub/worktrees"),
        )
        .expect("resolve default workdir");
        assert!(resolved.starts_with("~/.agenthub/worktrees/team-planner-"));
    }

    #[test]
    fn resolve_create_agent_workdir_rejects_blank_for_non_create_mode() {
        let err = resolve_create_agent_workdir(
            "",
            "planner",
            &WorktreeMode::ReuseWorktree,
            Some("~/.agenthub/worktrees"),
        )
        .expect_err("blank workdir should be rejected");
        let _ = err;
    }

    #[test]
    fn resolve_create_agent_workdir_rejects_blank_remote_target_defaults() {
        let err = resolve_create_agent_workdir("", "planner", &WorktreeMode::CreateWorktree, None)
            .expect_err("blank remote-target workdir should be rejected");
        assert_eq!(err.into_response().status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn resolve_create_agent_workdir_uses_remote_target_default_root() {
        let resolved = resolve_create_agent_workdir(
            "",
            "planner",
            &WorktreeMode::CreateWorktree,
            Some("~/.agenthub/worktrees/node-east"),
        )
        .expect("remote target default workdir");
        assert!(resolved.starts_with("~/.agenthub/worktrees/node-east/planner-"));
    }

    #[test]
    fn sanitize_worktree_segment_trims_mixed_edge_separators() {
        let sanitized = sanitize_worktree_segment("_-...Planner Team...-_");
        assert_eq!(sanitized, "planner-team");
    }

    #[test]
    fn sanitize_worktree_segment_collapses_repeated_internal_separators() {
        assert_eq!(sanitize_worktree_segment("agent...name"), "agent-name");
        assert_eq!(sanitize_worktree_segment("agent__name"), "agent-name");
        assert_eq!(sanitize_worktree_segment("agent._-name"), "agent-name");
        assert_eq!(
            sanitize_worktree_segment("agent   ---   name"),
            "agent-name"
        );
    }

    #[test]
    fn parse_start_actor_runtime_context_accepts_valid_payload() {
        let default_cli = default_actor_cli_path().expect("default actor cli path");
        let context = parse_start_actor_runtime_context(Some(StartAgentRequest {
            actor_runtime: Some(StartAgentActorRuntimeRequest {
                team_id: Some("team-7".to_string()),
                run_id: Some("run-7".to_string()),
                actor_id: "planner".to_string(),
                member_role: Some("leader".to_string()),
                channel: Some("coordination".to_string()),
                actor_cli_path: Some(default_cli.clone()),
            }),
        }))
        .expect("parse actor runtime context")
        .expect("context");
        assert_eq!(context.team_id.as_deref(), Some("team-7"));
        assert_eq!(context.current_run_id.as_deref(), Some("run-7"));
        assert_eq!(context.actor_id, "planner");
        assert_eq!(context.member_role.as_deref(), Some("leader"));
        assert_eq!(context.default_channel, "coordination");
        assert_eq!(context.actor_cli_path, default_cli);
    }

    #[test]
    fn parse_start_actor_runtime_context_defaults_channel_and_cli_path() {
        let context = parse_start_actor_runtime_context(Some(StartAgentRequest {
            actor_runtime: Some(StartAgentActorRuntimeRequest {
                team_id: Some("team-9".to_string()),
                run_id: Some("run-9".to_string()),
                actor_id: "planner".to_string(),
                member_role: None,
                channel: None,
                actor_cli_path: None,
            }),
        }))
        .expect("parse actor runtime context")
        .expect("context");
        assert_eq!(context.default_channel, "default");
        assert_eq!(
            context.actor_cli_path,
            default_actor_cli_path().expect("default actor cli path")
        );
    }

    #[test]
    fn parse_start_actor_runtime_context_rejects_blank_required_fields() {
        let scope_err = parse_start_actor_runtime_context(Some(StartAgentRequest {
            actor_runtime: Some(StartAgentActorRuntimeRequest {
                team_id: Some(" ".to_string()),
                run_id: Some(" ".to_string()),
                actor_id: "planner".to_string(),
                member_role: None,
                channel: None,
                actor_cli_path: None,
            }),
        }))
        .expect_err("team_id or run_id should be required");
        assert!(
            scope_err
                .into_response()
                .status()
                .eq(&axum::http::StatusCode::BAD_REQUEST)
        );

        let actor_id_err = parse_start_actor_runtime_context(Some(StartAgentRequest {
            actor_runtime: Some(StartAgentActorRuntimeRequest {
                team_id: Some("team-2".to_string()),
                run_id: Some("run-2".to_string()),
                actor_id: " ".to_string(),
                member_role: None,
                channel: None,
                actor_cli_path: None,
            }),
        }))
        .expect_err("actor_id should be required");
        let _ = actor_id_err;
    }

    #[test]
    fn parse_start_actor_runtime_context_rejects_untrusted_actor_cli_path() {
        let err = parse_start_actor_runtime_context(Some(StartAgentRequest {
            actor_runtime: Some(StartAgentActorRuntimeRequest {
                team_id: Some("team-10".to_string()),
                run_id: Some("run-10".to_string()),
                actor_id: "planner".to_string(),
                member_role: None,
                channel: None,
                actor_cli_path: Some("/tmp/not-allowed-agenthub".to_string()),
            }),
        }))
        .expect_err("actor_cli_path should be validated");
        let _ = err;
    }

    #[test]
    fn parse_optional_start_agent_request_accepts_empty_json_body() {
        let payload = parse_optional_start_agent_request(Bytes::from_static(b"   "))
            .expect("empty request body should be accepted");
        assert!(payload.is_none());
    }

    async fn create_test_db_with_options(options: SqliteConnectOptions) -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect sqlite")
    }

    async fn create_test_db() -> SqlitePool {
        create_test_db_with_options(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
    }

    async fn create_test_db_at(path: &Path) -> SqlitePool {
        create_test_db_with_options(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
    }

    async fn init_test_schema(db: &SqlitePool) {
        sqlx::query(
            r#"
            CREATE TABLE users (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL,
                role TEXT NOT NULL,
                password_hash TEXT,
                created_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create users");

        sqlx::query(
            r#"
            CREATE TABLE auth_sessions (
                token TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                revoked_at INTEGER,
                FOREIGN KEY(user_id) REFERENCES users(id)
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create auth_sessions");

        sqlx::query(
            r#"
            CREATE TABLE agents (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                workdir TEXT NOT NULL,
                command TEXT NOT NULL,
                args TEXT NOT NULL,
                worktree_mode TEXT NOT NULL,
                worktree_repo TEXT,
                worktree_ref TEXT,
                code_mode INTEGER NOT NULL DEFAULT 0,
                agent_loop_enabled INTEGER NOT NULL DEFAULT 0,
                agent_loop_idle_seconds INTEGER,
                agent_loop_prompt TEXT,
                source TEXT NOT NULL DEFAULT 'manual',
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create agents");

        sqlx::query(
            r#"
            CREATE TABLE safe_paths (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                created_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create safe_paths");

        sqlx::query(
            r#"
            CREATE TABLE agent_sessions (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                status TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                ended_at INTEGER,
                FOREIGN KEY(agent_id) REFERENCES agents(id)
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create agent_sessions");

        sqlx::query(
            r#"
            CREATE TABLE agent_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                seq TEXT NOT NULL,
                ts INTEGER NOT NULL,
                stream TEXT NOT NULL,
                message BLOB NOT NULL,
                FOREIGN KEY(agent_id) REFERENCES agents(id),
                FOREIGN KEY(session_id) REFERENCES agent_sessions(id)
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create agent_events");

        sqlx::query(
            r#"
            CREATE TABLE team_runs (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                context_id TEXT NOT NULL,
                status TEXT NOT NULL,
                input_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                started_at INTEGER,
                ended_at INTEGER
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create team_runs");

        sqlx::query(
            r#"
            CREATE TABLE team_steps (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                step_key TEXT NOT NULL,
                member_id TEXT NOT NULL,
                remote_task_id TEXT,
                status TEXT NOT NULL,
                attempt INTEGER NOT NULL DEFAULT 0,
                depends_on_json TEXT NOT NULL,
                input_json TEXT,
                output_json TEXT,
                error_text TEXT,
                started_at INTEGER,
                ended_at INTEGER,
                FOREIGN KEY(run_id) REFERENCES team_runs(id)
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create team_steps");

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
        .execute(db)
        .await
        .expect("create agent_time_triggers");

        sqlx::query(
            r#"
            CREATE TABLE acp_permission_requests (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                acp_session_id TEXT,
                team_id TEXT,
                requester_actor_id TEXT,
                requester_role TEXT,
                review_target_actor_id TEXT,
                review_dispatch_status TEXT,
                review_delivery_run_id TEXT,
                review_message_id INTEGER,
                review_dispatched_at INTEGER,
                reviewed_by_actor_id TEXT,
                human_review_notified_at INTEGER,
                tool_call_id TEXT,
                options_json TEXT NOT NULL,
                tool_call_json TEXT,
                status TEXT NOT NULL,
                selected_option_id TEXT,
                created_at INTEGER NOT NULL,
                responded_at INTEGER,
                FOREIGN KEY(agent_id) REFERENCES agents(id),
                FOREIGN KEY(session_id) REFERENCES agent_sessions(id)
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create acp_permission_requests");
    }

    async fn build_test_state_with_db_and_internal_peer(
        db: SqlitePool,
        internal_peer_client: Option<InternalGrpcPeerClientConfig>,
    ) -> AppState {
        let keys_dir = std::env::temp_dir().join(format!("agenthub-api-agents-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&keys_dir).expect("create keys dir");
        let keys_path = keys_dir.join("vapid.json");
        let config = AppConfig {
            web: Some(WebConfig {
                rp_id: Some("localhost".to_string()),
                rp_origin: Some("http://localhost:8080".to_string()),
                rp_name: Some("AgentHub Test".to_string()),
            }),
            push: Some(PushConfig {
                subject: Some("mailto:test@example.com".to_string()),
                keys_path: Some(keys_path.to_string_lossy().to_string()),
            }),
            ..Default::default()
        };
        let push = Arc::new(PushService::new(db.clone(), &config).expect("create push service"));
        remove_dir_best_effort(&keys_dir);
        let auth = Arc::new(
            AuthService::new(db.clone(), &config)
                .await
                .expect("create auth service"),
        );
        let permissions = Arc::new(AcpPermissionService::new(db.clone()));
        let event_dbs = agenthub_db::AgentEventDbRouter::new(
            std::env::temp_dir().join(format!("agenthub-api-agents-eventdb-{}", Uuid::new_v4())),
        );
        let agents = Arc::new(AgentManager::new_with_internal_grpc(
            db.clone(),
            event_dbs.clone(),
            None,
            push.clone(),
            Vec::new(),
            "agenthub-codex-acp".to_string(),
            None,
            permissions.clone(),
            auth.clone(),
            internal_peer_client,
        ));
        let teams = Arc::new(TeamManager::new_with_event_dbs(db.clone(), event_dbs));
        AppState {
            db,
            agents,
            teams,
            push,
            auth,
            acp_permissions: permissions,
            default_worktree_root: config.default_worktree_root(),
        }
    }

    async fn build_test_state_with_db(db: SqlitePool) -> AppState {
        build_test_state_with_db_and_internal_peer(db, None).await
    }

    async fn build_test_state() -> AppState {
        let db = create_test_db().await;
        init_test_schema(&db).await;
        build_test_state_with_db(db).await
    }

    fn remove_dir_best_effort(path: &std::path::Path) {
        if let Err(err) = std::fs::remove_dir_all(path) {
            eprintln!(
                "warning: failed to remove temp directory {}: {err}",
                path.display()
            );
        }
    }

    async fn add_agent_node_support(db: &SqlitePool) {
        sqlx::query(
            r#"
            CREATE TABLE agent_nodes (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                grpc_target TEXT NOT NULL,
                tls_server_name TEXT,
                default_worktree_root TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create agent_nodes table");
        sqlx::query("ALTER TABLE agents ADD COLUMN target_node_id TEXT")
            .execute(db)
            .await
            .expect("add target_node_id column");
    }

    async fn create_auth_token(state: &AppState) -> String {
        let user_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO users (id, username, display_name, role, password_hash, created_at)
            VALUES (?1, ?2, ?3, 'root', NULL, ?4)
            "#,
        )
        .bind(&user_id)
        .bind(format!("root-{}", Uuid::new_v4()))
        .bind("Root")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert user");
        state
            .auth
            .create_session(&user_id)
            .await
            .expect("create session token")
    }

    async fn create_non_root_auth_token(state: &AppState) -> String {
        let user_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO users (id, username, display_name, role, password_hash, created_at)
            VALUES (?1, ?2, ?3, 'user', NULL, ?4)
            "#,
        )
        .bind(&user_id)
        .bind(format!("user-{}", Uuid::new_v4()))
        .bind("User")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert non-root user");
        state
            .auth
            .create_session(&user_id)
            .await
            .expect("create non-root session token")
    }

    fn build_json_request(
        method: Method,
        path: &str,
        token: Option<&str>,
        payload: Option<Value>,
    ) -> Request<Body> {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        match payload {
            Some(payload) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .expect("build json request"),
            None => builder.body(Body::empty()).expect("build request"),
        }
    }

    async fn decode_json_body(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        serde_json::from_slice(&bytes).expect("decode response json")
    }

    async fn decode_text_body(response: axum::response::Response) -> String {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        String::from_utf8(bytes.to_vec()).expect("decode response text")
    }

    fn run_git(repo_dir: &std::path::Path, args: &[&str]) {
        let status = StdCommand::new("git")
            .arg("-C")
            .arg(repo_dir)
            .args(args)
            .status()
            .expect(
                "failed to execute `git` command; ensure `git` is installed and available on PATH",
            );
        assert!(status.success(), "git command failed: {:?}", args);
    }

    fn is_git_available() -> bool {
        let status = StdCommand::new("git")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        matches!(status, Ok(status) if status.success())
    }

    #[tokio::test]
    async fn create_worktree_agent_can_start_again_after_stop() {
        if !is_git_available() {
            eprintln!(
                "skipping create_worktree_agent_can_start_again_after_stop: `git` is not available on PATH"
            );
            return;
        }
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());

        let base =
            std::env::temp_dir().join(format!("agenthub-create-worktree-{}", Uuid::new_v4()));
        let repo_dir = base.join("repo");
        let workdir = base.join("worktree-agent");
        std::fs::create_dir_all(&repo_dir).expect("create repo dir");

        run_git(&repo_dir, &["init"]);
        run_git(
            &repo_dir,
            &["config", "user.email", "agenthub-test@example.com"],
        );
        run_git(&repo_dir, &["config", "user.name", "AgentHub Test"]);
        std::fs::write(repo_dir.join("README.md"), "seed\n").expect("write seed file");
        run_git(&repo_dir, &["add", "README.md"]);
        run_git(&repo_dir, &["commit", "-m", "init"]);

        let now = chrono::Utc::now().timestamp();
        for path in [&repo_dir, &workdir] {
            sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
                .bind(path.to_string_lossy().to_string())
                .bind(now)
                .execute(&state.db)
                .await
                .expect("insert safe path");
        }

        let create_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "create-worktree-restart",
                    "workdir": workdir.to_string_lossy().to_string(),
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 30"],
                    "worktree_mode": "create_worktree",
                    "worktree_repo": repo_dir.to_string_lossy().to_string(),
                    "worktree_ref": "HEAD",
                    "code_mode": true
                })),
            ))
            .await
            .expect("create create_worktree agent");
        assert_eq!(create_resp.status(), StatusCode::OK);
        let created = decode_json_body(create_resp).await;
        let agent_id = created["id"].as_str().expect("agent id");

        let start_first = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/start"),
                Some(&token),
                None,
            ))
            .await
            .expect("start create_worktree agent (first)");
        assert_eq!(start_first.status(), StatusCode::OK);

        let stop_first = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/stop"),
                Some(&token),
                None,
            ))
            .await
            .expect("stop create_worktree agent (first)");
        assert_eq!(stop_first.status(), StatusCode::OK);

        let start_second = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/start"),
                Some(&token),
                None,
            ))
            .await
            .expect("start create_worktree agent (second)");
        assert_eq!(start_second.status(), StatusCode::OK);

        let stop_second = app
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/stop"),
                Some(&token),
                None,
            ))
            .await
            .expect("stop create_worktree agent (second)");
        assert_eq!(stop_second.status(), StatusCode::OK);

        remove_dir_best_effort(&base);
    }

    #[tokio::test]
    async fn create_agent_route_rejects_remote_target_without_internal_peer_client() {
        let db = create_test_db().await;
        init_test_schema(&db).await;
        add_agent_node_support(&db).await;
        let state = build_test_state_with_db(db).await;
        state
            .agents
            .create_agent_node(crate::agent::AgentNodeConfig {
                id: "node-east".to_string(),
                name: "Node East".to_string(),
                grpc_target: "https://node-east.internal:50051".to_string(),
                tls_server_name: Some("node-east.internal".to_string()),
                default_worktree_root: None,
            })
            .await
            .expect("create agent node");
        let token = create_auth_token(&state).await;
        let app = router(state);

        let response = app
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "remote-no-peer-config",
                    "workdir": "/remote/workdir",
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 10"],
                    "target_node_id": "node-east",
                    "code_mode": true
                })),
            ))
            .await
            .expect("create remote-target agent");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = decode_json_body(response).await;
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|value| value.contains("internal gRPC peer config")),
            "unexpected error body: {body}"
        );
    }

    #[tokio::test]
    async fn create_agent_route_rejects_remote_target_for_non_root_user() {
        let db = create_test_db().await;
        init_test_schema(&db).await;
        add_agent_node_support(&db).await;
        let state = build_test_state_with_db_and_internal_peer(
            db,
            Some(InternalGrpcPeerClientConfig {
                shared_secret: "phase1-shared-secret".to_string(),
                expected_issuer: Some("agenthub".to_string()),
                expected_audience: Some("agenthub-internal".to_string()),
                source_node_id: "main".to_string(),
                cert_dir: std::env::temp_dir().to_string_lossy().to_string(),
                security_mode: InternalGrpcSecurityMode::Tls,
            }),
        )
        .await;
        state
            .agents
            .create_agent_node(crate::agent::AgentNodeConfig {
                id: "node-east".to_string(),
                name: "Node East".to_string(),
                grpc_target: "https://node-east.internal:50051".to_string(),
                tls_server_name: Some("node-east.internal".to_string()),
                default_worktree_root: None,
            })
            .await
            .expect("create agent node");
        let token = create_non_root_auth_token(&state).await;
        let app = router(state);

        let response = app
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "remote-non-root",
                    "workdir": "/remote/workdir",
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 10"],
                    "target_node_id": "node-east",
                    "code_mode": true
                })),
            ))
            .await
            .expect("create remote-target agent as non-root");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = decode_json_body(response).await;
        assert_eq!(body["error"], "root required for remote target node");
    }

    #[tokio::test]
    async fn create_agent_treats_main_target_node_as_local() {
        let db = create_test_db().await;
        init_test_schema(&db).await;
        add_agent_node_support(&db).await;
        let state = build_test_state_with_db(db).await;
        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind("/tmp/main-target-normalizes-local")
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert safe path");

        let agent = state
            .agents
            .create_agent(crate::agent::AgentConfig {
                name: "main-target-normalizes-local".to_string(),
                workdir: "/tmp/main-target-normalizes-local".to_string(),
                command: "/bin/sh".to_string(),
                args: vec!["-lc".to_string(), "sleep 10".to_string()],
                target_node_id: Some("main".to_string()),
                worktree_mode: crate::agent::WorktreeMode::UseExisting,
                worktree_repo: None,
                worktree_ref: None,
                code_mode: true,
                agent_loop_enabled: false,
                agent_loop_idle_seconds: None,
                agent_loop_prompt: None,
            })
            .await
            .expect("main target should normalize to local agent");

        assert!(
            agent.target_node_id.is_none(),
            "main should be stored as local target"
        );
        let reloaded = state
            .agents
            .get_agent(&agent.id)
            .await
            .expect("reload created agent");
        assert!(
            reloaded.target_node_id.is_none(),
            "reloaded agent should remain local"
        );
    }

    #[tokio::test]
    async fn create_agent_route_uses_remote_node_default_worktree_root_when_blank() {
        let db = create_test_db().await;
        init_test_schema(&db).await;
        add_agent_node_support(&db).await;
        let state = build_test_state_with_db_and_internal_peer(
            db,
            Some(InternalGrpcPeerClientConfig {
                shared_secret: "phase1-shared-secret".to_string(),
                expected_issuer: Some("agenthub".to_string()),
                expected_audience: Some("agenthub-internal".to_string()),
                source_node_id: "main".to_string(),
                cert_dir: std::env::temp_dir().to_string_lossy().to_string(),
                security_mode: InternalGrpcSecurityMode::Tls,
            }),
        )
        .await;
        state
            .agents
            .create_agent_node(crate::agent::AgentNodeConfig {
                id: "node-east".to_string(),
                name: "Node East".to_string(),
                grpc_target: "https://node-east.internal:50051".to_string(),
                tls_server_name: Some("node-east.internal".to_string()),
                default_worktree_root: Some("~/.agenthub/worktrees/node-east".to_string()),
            })
            .await
            .expect("create agent node");
        let token = create_auth_token(&state).await;
        let app = router(state);

        let response = app
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "remote-default-worktree",
                    "workdir": "",
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 10"],
                    "target_node_id": "node-east",
                    "worktree_mode": "create_worktree",
                    "worktree_repo": "/tmp/repo",
                    "worktree_ref": "HEAD",
                    "code_mode": true
                })),
            ))
            .await
            .expect("create remote-target agent with node default root");
        let status = response.status();
        let body = decode_json_body(response).await;
        assert_eq!(status, StatusCode::OK, "unexpected response body: {body}");
        assert!(
            body["workdir"].as_str().is_some_and(|value| value
                .starts_with("~/.agenthub/worktrees/node-east/remote-default-worktree-")),
            "unexpected response body: {body}"
        );
    }

    #[tokio::test]
    async fn create_team_forge_agent_route_rejects_remote_target_on_legacy_schema() {
        let db = create_test_db().await;
        init_test_schema(&db).await;
        sqlx::query(
            r#"
            CREATE TABLE agent_nodes (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                grpc_target TEXT NOT NULL,
                tls_server_name TEXT,
                default_worktree_root TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&db)
        .await
        .expect("create agent_nodes table");
        let state = build_test_state_with_db_and_internal_peer(
            db,
            Some(InternalGrpcPeerClientConfig {
                shared_secret: "phase1-shared-secret".to_string(),
                expected_issuer: Some("agenthub".to_string()),
                expected_audience: Some("agenthub-internal".to_string()),
                source_node_id: "main".to_string(),
                cert_dir: std::env::temp_dir().to_string_lossy().to_string(),
                security_mode: InternalGrpcSecurityMode::Tls,
            }),
        )
        .await;
        state
            .agents
            .create_agent_node(crate::agent::AgentNodeConfig {
                id: "node-east".to_string(),
                name: "Node East".to_string(),
                grpc_target: "https://node-east.internal:50051".to_string(),
                tls_server_name: Some("node-east.internal".to_string()),
                default_worktree_root: None,
            })
            .await
            .expect("create agent node");
        let token = create_auth_token(&state).await;
        let app = router(state);

        let response = app
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "legacy-team-forge-remote",
                    "workdir": "/remote/workdir",
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 10"],
                    "target_node_id": "node-east",
                    "source": "team_forge",
                    "code_mode": true
                })),
            ))
            .await
            .expect("create remote-target team_forge agent");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = decode_json_body(response).await;
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|value| value.contains("agents.target_node_id column")),
            "unexpected error body: {body}"
        );
    }

    #[tokio::test]
    async fn create_agent_rejects_remote_target_on_legacy_schema() {
        let db = create_test_db().await;
        init_test_schema(&db).await;
        sqlx::query(
            r#"
            CREATE TABLE agent_nodes (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                grpc_target TEXT NOT NULL,
                tls_server_name TEXT,
                default_worktree_root TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&db)
        .await
        .expect("create agent_nodes table");
        let state = build_test_state_with_db_and_internal_peer(
            db,
            Some(InternalGrpcPeerClientConfig {
                shared_secret: "phase1-shared-secret".to_string(),
                expected_issuer: Some("agenthub".to_string()),
                expected_audience: Some("agenthub-internal".to_string()),
                source_node_id: "main".to_string(),
                cert_dir: std::env::temp_dir().to_string_lossy().to_string(),
                security_mode: InternalGrpcSecurityMode::Tls,
            }),
        )
        .await;
        state
            .agents
            .create_agent_node(crate::agent::AgentNodeConfig {
                id: "node-east".to_string(),
                name: "Node East".to_string(),
                grpc_target: "https://node-east.internal:50051".to_string(),
                tls_server_name: Some("node-east.internal".to_string()),
                default_worktree_root: None,
            })
            .await
            .expect("create agent node");

        let err = state
            .agents
            .create_agent(crate::agent::AgentConfig {
                name: "legacy-remote-target".to_string(),
                workdir: "/remote/workdir".to_string(),
                command: "/bin/sh".to_string(),
                args: vec!["-lc".to_string(), "sleep 10".to_string()],
                target_node_id: Some("node-east".to_string()),
                worktree_mode: crate::agent::WorktreeMode::UseExisting,
                worktree_repo: None,
                worktree_ref: None,
                code_mode: true,
                agent_loop_enabled: false,
                agent_loop_idle_seconds: None,
                agent_loop_prompt: None,
            })
            .await
            .expect_err("legacy schema should reject remote target");
        assert!(
            err.to_string().contains("agents.target_node_id column"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn map_create_agent_error_classifies_validation_messages() {
        let bad_request = map_create_agent_error(anyhow::anyhow!(
            "remote-target agents require agents.target_node_id column on a legacy schema"
        ))
        .into_response();
        assert_eq!(bad_request.status(), StatusCode::BAD_REQUEST);

        let internal = map_create_agent_error(anyhow::anyhow!("sqlite busy")).into_response();
        assert_eq!(internal.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn create_worktree_agent_can_start_after_state_rebuild() {
        if !is_git_available() {
            eprintln!(
                "skipping create_worktree_agent_can_start_after_state_rebuild: `git` is not available on PATH"
            );
            return;
        }

        let base = std::env::temp_dir().join(format!(
            "agenthub-create-worktree-rebuild-{}",
            Uuid::new_v4()
        ));
        let repo_dir = base.join("repo");
        let workdir = base.join("worktree-agent");
        let db_path = base.join("agents.sqlite");
        std::fs::create_dir_all(&repo_dir).expect("create repo dir");

        run_git(&repo_dir, &["init"]);
        run_git(
            &repo_dir,
            &["config", "user.email", "agenthub-test@example.com"],
        );
        run_git(&repo_dir, &["config", "user.name", "AgentHub Test"]);
        std::fs::write(repo_dir.join("README.md"), "seed\n").expect("write seed file");
        run_git(&repo_dir, &["add", "README.md"]);
        run_git(&repo_dir, &["commit", "-m", "init"]);

        let db = create_test_db_at(&db_path).await;
        init_test_schema(&db).await;
        let state = build_test_state_with_db(db).await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());

        let now = chrono::Utc::now().timestamp();
        for path in [&repo_dir, &workdir] {
            sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
                .bind(path.to_string_lossy().to_string())
                .bind(now)
                .execute(&state.db)
                .await
                .expect("insert safe path");
        }

        let create_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "create-worktree-state-rebuild",
                    "workdir": workdir.to_string_lossy().to_string(),
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 30"],
                    "worktree_mode": "create_worktree",
                    "worktree_repo": repo_dir.to_string_lossy().to_string(),
                    "worktree_ref": "HEAD",
                    "code_mode": true
                })),
            ))
            .await
            .expect("create create_worktree agent");
        assert_eq!(create_resp.status(), StatusCode::OK);
        let created = decode_json_body(create_resp).await;
        let agent_id = created["id"].as_str().expect("agent id").to_string();

        let start_first = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/start"),
                Some(&token),
                None,
            ))
            .await
            .expect("start create_worktree agent (first)");
        assert_eq!(start_first.status(), StatusCode::OK);

        let stop_first = app
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/stop"),
                Some(&token),
                None,
            ))
            .await
            .expect("stop create_worktree agent (first)");
        assert_eq!(stop_first.status(), StatusCode::OK);

        drop(state);

        let reloaded_db = create_test_db_at(&db_path).await;
        let reloaded_state = build_test_state_with_db(reloaded_db).await;
        reloaded_state
            .agents
            .mark_exited_on_startup()
            .await
            .expect("mark exited on startup");
        let reloaded_app = router(reloaded_state.clone());

        let start_after_rebuild = reloaded_app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/start"),
                Some(&token),
                None,
            ))
            .await
            .expect("start create_worktree agent (after state rebuild)");
        assert_eq!(start_after_rebuild.status(), StatusCode::OK);

        let stop_after_rebuild = reloaded_app
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/stop"),
                Some(&token),
                None,
            ))
            .await
            .expect("stop create_worktree agent (after state rebuild)");
        assert_eq!(stop_after_rebuild.status(), StatusCode::OK);

        drop(reloaded_state);
        remove_dir_best_effort(&base);
    }

    #[tokio::test]
    async fn create_worktree_reuses_existing_after_ref_state_changes() {
        if !is_git_available() {
            eprintln!(
                "skipping create_worktree_reuses_existing_after_ref_state_changes: `git` is not available on PATH"
            );
            return;
        }
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());

        let base =
            std::env::temp_dir().join(format!("agenthub-create-worktree-{}", Uuid::new_v4()));
        let repo_dir = base.join("repo");
        let workdir = base.join("worktree-agent");
        std::fs::create_dir_all(&repo_dir).expect("create repo dir");

        run_git(&repo_dir, &["init"]);
        run_git(
            &repo_dir,
            &["config", "user.email", "agenthub-test@example.com"],
        );
        run_git(&repo_dir, &["config", "user.name", "AgentHub Test"]);
        std::fs::write(repo_dir.join("README.md"), "seed\n").expect("write seed file");
        run_git(&repo_dir, &["add", "README.md"]);
        run_git(&repo_dir, &["commit", "-m", "init"]);

        let branch_name = format!("agenthub-worktree-ref-{}", Uuid::new_v4().simple());
        let status = StdCommand::new("git")
            .arg("-C")
            .arg(&repo_dir)
            .arg("checkout")
            .arg("-b")
            .arg(&branch_name)
            .status()
            .expect("create branch for create_worktree ref");
        assert!(
            status.success(),
            "git checkout -b failed for branch {branch_name}"
        );
        run_git(&repo_dir, &["checkout", "-"]);

        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind(repo_dir.to_string_lossy().to_string())
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert repo safe path");
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind(workdir.to_string_lossy().to_string())
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert workdir safe path");

        let create_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "create-worktree-ref-mismatch-restart",
                    "workdir": workdir.to_string_lossy().to_string(),
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 30"],
                    "worktree_mode": "create_worktree",
                    "worktree_repo": repo_dir.to_string_lossy().to_string(),
                    "worktree_ref": branch_name,
                    "code_mode": true
                })),
            ))
            .await
            .expect("create create_worktree agent");
        assert_eq!(create_resp.status(), StatusCode::OK);
        let created = decode_json_body(create_resp).await;
        let agent_id = created["id"].as_str().expect("agent id");

        let start_first = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/start"),
                Some(&token),
                None,
            ))
            .await
            .expect("start create_worktree agent (first)");
        assert_eq!(start_first.status(), StatusCode::OK);

        let stop_first = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/stop"),
                Some(&token),
                None,
            ))
            .await
            .expect("stop create_worktree agent (first)");
        assert_eq!(stop_first.status(), StatusCode::OK);

        run_git(&workdir, &["checkout", "--detach"]);

        let start_second = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/start"),
                Some(&token),
                None,
            ))
            .await
            .expect("start create_worktree agent (second)");
        assert_eq!(start_second.status(), StatusCode::OK);

        let stop_second = app
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/stop"),
                Some(&token),
                None,
            ))
            .await
            .expect("stop create_worktree agent (second)");
        assert_eq!(stop_second.status(), StatusCode::OK);

        remove_dir_best_effort(&base);
    }

    #[tokio::test]
    async fn create_worktree_rejects_reuse_by_other_agent() {
        if !is_git_available() {
            eprintln!(
                "skipping create_worktree_rejects_reuse_by_other_agent: `git` is not available on PATH"
            );
            return;
        }
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());

        let base =
            std::env::temp_dir().join(format!("agenthub-create-worktree-{}", Uuid::new_v4()));
        let repo_dir = base.join("repo");
        let workdir = base.join("worktree-agent");
        std::fs::create_dir_all(&repo_dir).expect("create repo dir");

        run_git(&repo_dir, &["init"]);
        run_git(
            &repo_dir,
            &["config", "user.email", "agenthub-test@example.com"],
        );
        run_git(&repo_dir, &["config", "user.name", "AgentHub Test"]);
        std::fs::write(repo_dir.join("README.md"), "seed\n").expect("write seed file");
        run_git(&repo_dir, &["add", "README.md"]);
        run_git(&repo_dir, &["commit", "-m", "init"]);

        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind(repo_dir.to_string_lossy().to_string())
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert repo safe path");
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind(workdir.to_string_lossy().to_string())
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert workdir safe path");

        let first_create = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "create-worktree-owner",
                    "workdir": workdir.to_string_lossy().to_string(),
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 30"],
                    "worktree_mode": "create_worktree",
                    "worktree_repo": repo_dir.to_string_lossy().to_string(),
                    "worktree_ref": "HEAD",
                    "code_mode": true
                })),
            ))
            .await
            .expect("create owner agent");
        assert_eq!(first_create.status(), StatusCode::OK);
        let first_agent = decode_json_body(first_create).await;
        let first_id = first_agent["id"].as_str().expect("first agent id");

        let first_start = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{first_id}/start"),
                Some(&token),
                None,
            ))
            .await
            .expect("start owner agent");
        assert_eq!(first_start.status(), StatusCode::OK);

        let first_stop = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{first_id}/stop"),
                Some(&token),
                None,
            ))
            .await
            .expect("stop owner agent");
        assert_eq!(first_stop.status(), StatusCode::OK);

        let second_create = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "create-worktree-attacker",
                    "workdir": workdir.to_string_lossy().to_string(),
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 30"],
                    "worktree_mode": "create_worktree",
                    "worktree_repo": repo_dir.to_string_lossy().to_string(),
                    "worktree_ref": "HEAD",
                    "code_mode": true
                })),
            ))
            .await
            .expect("create second agent");
        assert_eq!(second_create.status(), StatusCode::OK);
        let second_agent = decode_json_body(second_create).await;
        let second_id = second_agent["id"].as_str().expect("second agent id");

        let second_start = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{second_id}/start"),
                Some(&token),
                None,
            ))
            .await
            .expect("start second agent");
        assert_eq!(second_start.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = decode_text_body(second_start).await;
        assert!(
            body.contains("existing worktree belongs to another agent"),
            "unexpected error body: {body}"
        );

        remove_dir_best_effort(&base);
    }

    #[tokio::test]
    async fn send_input_rejects_stale_session_id_with_conflict() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());

        let workdir = std::env::temp_dir().join(format!("agenthub-send-input-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&workdir).expect("create workdir");
        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind(workdir.to_string_lossy().to_string())
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert safe path");

        let create_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "send-input-session-guard",
                    "workdir": workdir.to_string_lossy().to_string(),
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 30"],
                    "code_mode": true
                })),
            ))
            .await
            .expect("create agent");
        assert_eq!(create_resp.status(), StatusCode::OK);
        let created = decode_json_body(create_resp).await;
        let agent_id = created["id"].as_str().expect("agent id");

        let start_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/start"),
                Some(&token),
                None,
            ))
            .await
            .expect("start agent");
        assert_eq!(start_resp.status(), StatusCode::OK);
        let started = decode_json_body(start_resp).await;
        let running_session_id = started["session_id"]
            .as_str()
            .expect("running session id")
            .to_string();

        let input_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/input"),
                Some(&token),
                Some(json!({
                    "input": "hello",
                    "message_id": "msg-1",
                    "session_id": format!("{running_session_id}-stale")
                })),
            ))
            .await
            .expect("send input");
        assert_eq!(input_resp.status(), StatusCode::CONFLICT);
        let conflict_body = decode_text_body(input_resp).await;
        assert!(
            conflict_body.contains("agent session mismatch:"),
            "unexpected conflict body: {conflict_body}"
        );

        let stop_resp = app
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/stop"),
                Some(&token),
                None,
            ))
            .await
            .expect("stop agent");
        assert_eq!(stop_resp.status(), StatusCode::OK);

        remove_dir_best_effort(&workdir);
    }

    #[tokio::test]
    async fn start_route_rejects_actor_runtime_payload_for_agent_mode() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());

        let workdir = std::env::temp_dir().join(format!("agenthub-start-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&workdir).expect("create workdir");
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO safe_paths (path, created_at)
            VALUES (?1, ?2)
            "#,
        )
        .bind(workdir.to_string_lossy().to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert safe path");

        let env_file = workdir.join("actor-runtime-env.txt");
        let script = "printf '%s\\n' \"$AGENTHUB_ACTOR_CURRENT_RUN_ID\" > actor-runtime-env.txt; \
             printf '%s\\n' \"$AGENTHUB_ACTOR_ID\" >> actor-runtime-env.txt; \
             printf '%s\\n' \"$AGENTHUB_ACTOR_CHANNEL\" >> actor-runtime-env.txt; \
             printf '%s\\n' \"$AGENTHUB_ACTOR_CLI\" >> actor-runtime-env.txt; \
             sleep 30"
            .to_string();

        let create_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "actor-runtime-api-agent",
                    "workdir": workdir.to_string_lossy().to_string(),
                    "command": "/bin/sh",
                    "args": ["-lc", script],
                    "code_mode": true
                })),
            ))
            .await
            .expect("create agent via router");
        assert_eq!(create_resp.status(), StatusCode::OK);
        let created = decode_json_body(create_resp).await;
        let agent_id = created["id"].as_str().expect("agent id");

        let cli_path = default_actor_cli_path().expect("resolve default cli path");
        let start_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/start"),
                Some(&token),
                Some(json!({
                    "actor_runtime": {
                        "run_id": "run-api-start",
                        "actor_id": "planner",
                        "channel": "coordination",
                        "actor_cli_path": cli_path
                    }
                })),
            ))
            .await
            .expect("start agent with actor runtime context should be rejected");
        assert_eq!(start_resp.status(), StatusCode::BAD_REQUEST);
        let body = decode_json_body(start_resp).await;
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|msg| msg.contains("actor_runtime is reserved")),
            "unexpected error body: {body}"
        );

        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            !env_file.exists(),
            "agent mode should reject actor_runtime payload and must not spawn process"
        );

        remove_dir_best_effort(&workdir);
    }

    #[tokio::test]
    async fn start_route_accepts_empty_body_with_json_content_type() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());

        let workdir =
            std::env::temp_dir().join(format!("agenthub-start-empty-json-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&workdir).expect("create workdir");
        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind(workdir.to_string_lossy().to_string())
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert safe path");

        let create_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "empty-json-start-agent",
                    "workdir": workdir.to_string_lossy().to_string(),
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 30"],
                    "code_mode": true
                })),
            ))
            .await
            .expect("create agent");
        assert_eq!(create_resp.status(), StatusCode::OK);
        let created = decode_json_body(create_resp).await;
        let agent_id = created["id"].as_str().expect("agent id");

        let start_request = Request::builder()
            .method(Method::POST)
            .uri(format!("/{agent_id}/start"))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::empty())
            .expect("build empty-json start request");
        let start_resp = app
            .clone()
            .oneshot(start_request)
            .await
            .expect("start agent with empty json header");
        assert_eq!(start_resp.status(), StatusCode::OK);

        let stop_resp = app
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/stop"),
                Some(&token),
                None,
            ))
            .await
            .expect("stop agent");
        assert_eq!(stop_resp.status(), StatusCode::OK);

        remove_dir_best_effort(&workdir);
    }

    #[tokio::test]
    async fn discovery_card_route_exposes_agent_capabilities() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());

        let workdir =
            std::env::temp_dir().join(format!("agenthub-discovery-card-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&workdir).expect("create workdir");
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO safe_paths (path, created_at)
            VALUES (?1, ?2)
            "#,
        )
        .bind(workdir.to_string_lossy().to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert safe path");

        let create_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "discovery-card-agent",
                    "workdir": workdir.to_string_lossy().to_string(),
                    "command": "agenthub-codex-acp",
                    "args": [],
                    "worktree_mode": "create_worktree",
                    "worktree_repo": workdir.to_string_lossy().to_string(),
                    "worktree_ref": "main",
                    "code_mode": true
                })),
            ))
            .await
            .expect("create agent");
        assert_eq!(create_resp.status(), StatusCode::OK);
        let created = decode_json_body(create_resp).await;
        let agent_id = created["id"].as_str().expect("agent id");

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS team_definitions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                spec_json TEXT NOT NULL,
                owner_user_id TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&state.db)
        .await
        .expect("create team_definitions table");

        let team_spec = json!({
            "spec_version": 1,
            "entrypoint": "leader-main",
            "members": [
                {"member_id": "leader-main", "role": "leader"},
                {
                    "member_id": agent_id,
                    "role": "worker",
                    "description": "TiDB fuzz/bugfix specialist"
                }
            ],
            "steps": [
                {"step_key": "leader_plan", "member_id": "leader-main", "depends_on": []}
            ]
        });
        sqlx::query(
            r#"
            INSERT INTO team_definitions (id, name, description, spec_json, owner_user_id, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind("discovery card binding")
        .bind("bind agent-card description to member profile")
        .bind(team_spec.to_string())
        .bind(now)
        .bind(now + 1)
        .execute(&state.db)
        .await
        .expect("insert team definition");

        let card_resp = app
            .oneshot(build_json_request(
                Method::GET,
                &format!("/{agent_id}/.well-known/agent-card"),
                Some(&token),
                None,
            ))
            .await
            .expect("get discovery card");
        assert_eq!(card_resp.status(), StatusCode::OK);
        let card = decode_json_body(card_resp).await;
        let expected_card_id = format!("agenthub://agents/{agent_id}");
        assert_eq!(card["card_id"].as_str(), Some(expected_card_id.as_str()));
        assert_eq!(
            card["schema_version"].as_str(),
            Some("agenthub.a2a.discovery_card.v1")
        );
        assert_eq!(
            card["description"].as_str(),
            Some("TiDB fuzz/bugfix specialist"),
            "missing/invalid discovery card description: {card}"
        );
        assert_eq!(card["identity"]["agent_id"].as_str(), Some(agent_id));
        assert_eq!(card["runtime"]["acp_provider"].as_str(), Some("codex"));
        assert_eq!(card["runtime"]["code_mode"].as_bool(), Some(true));
        assert_eq!(
            card["runtime"]["worktree_mode"].as_str(),
            Some("create_worktree")
        );
        let tags: HashSet<&str> = card["capability_tags"]
            .as_array()
            .expect("capability tags array")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(tags.contains("team_mailbox_v1"));
        assert!(tags.contains("team_step_execution_v1"));
        assert!(tags.contains("code_mode"));
        assert!(tags.contains("git_worktree"));
        assert!(tags.contains("acp_codex"));

        remove_dir_best_effort(&workdir);
    }

    #[tokio::test]
    async fn delete_agent_prunes_team_member_references_from_team_specs() {
        let state = crate::api::team_tests::build_test_state().await;
        let token = crate::api::team_tests::create_auth_token(&state).await;
        let app = router(state.clone());
        let now = chrono::Utc::now().timestamp();
        let team_id = Uuid::new_v4().to_string();
        let worker_step_key = "worker_1_worker_1";

        sqlx::query(
            r#"
            INSERT INTO team_definitions (id, name, description, spec_json, owner_user_id, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6)
            "#,
        )
        .bind(&team_id)
        .bind("delete-agent-prune-team")
        .bind("prune deleted team member references")
        .bind(
            json!({
                "spec_version": 1,
                "leader_member_id": "leader",
                "entrypoint": "leader_plan",
                "members": [
                    {"member_id": "leader", "role": "leader"},
                    {"member_id": "worker-1", "role": "worker"}
                ],
                "steps": [
                    {"step_key": "leader_plan", "member_id": "leader", "depends_on": []},
                    {"step_key": worker_step_key, "member_id": "worker-1", "depends_on": ["leader_plan"]},
                    {
                        "step_key": "leader_synthesize",
                        "member_id": "leader",
                        "depends_on": [worker_step_key]
                    }
                ]
            })
            .to_string(),
        )
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert team definition");

        let delete_resp = app
            .oneshot(build_json_request(
                Method::DELETE,
                "/worker-1",
                Some(&token),
                None,
            ))
            .await
            .expect("delete team member agent");
        assert_eq!(delete_resp.status(), StatusCode::OK);

        let team_row = sqlx::query("SELECT spec_json FROM team_definitions WHERE id = ?1")
            .bind(&team_id)
            .fetch_one(&state.db)
            .await
            .expect("load updated team definition");
        let spec_json = team_row.get::<String, _>("spec_json");
        let updated_spec: Value =
            serde_json::from_str(spec_json.as_str()).expect("parse updated spec json");

        let members = updated_spec["members"]
            .as_array()
            .expect("updated team members");
        assert_eq!(members.len(), 1);
        assert_eq!(members[0]["member_id"], Value::from("leader"));

        let steps = updated_spec["steps"]
            .as_array()
            .expect("updated team steps");
        assert!(
            steps
                .iter()
                .all(|step| step["member_id"].as_str() != Some("worker-1")),
            "deleted worker steps should be pruned: {updated_spec}"
        );
        let synth = steps
            .iter()
            .find(|step| step["step_key"].as_str() == Some("leader_synthesize"))
            .expect("leader synth step");
        assert_eq!(synth["depends_on"], json!([]));
    }

    #[tokio::test]
    async fn delete_agent_returns_ok_when_team_prune_follow_up_fails() {
        let state = crate::api::team_tests::build_test_state().await;
        let token = crate::api::team_tests::create_auth_token(&state).await;
        let app = router(state.clone());
        let now = chrono::Utc::now().timestamp();
        let team_id = Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO team_definitions (id, name, description, spec_json, owner_user_id, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6)
            "#,
        )
        .bind(&team_id)
        .bind("delete-agent-best-effort-prune")
        .bind("delete endpoint should stay successful after prune follow-up failure")
        .bind(
            json!({
                "spec_version": 1,
                "leader_member_id": "missing-leader",
                "entrypoint": "leader_plan",
                "members": [
                    {"member_id": "missing-leader", "role": "leader"},
                    {"member_id": "worker-1", "role": "worker"}
                ],
                "steps": [
                    {"step_key": "leader_plan", "member_id": "missing-leader", "depends_on": []},
                    {
                        "step_key": "worker_1_worker_1",
                        "member_id": "worker-1",
                        "depends_on": ["leader_plan"]
                    },
                    {
                        "step_key": "leader_synthesize",
                        "member_id": "missing-leader",
                        "depends_on": ["worker_1_worker_1"]
                    }
                ]
            })
            .to_string(),
        )
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert team definition");

        let delete_resp = app
            .oneshot(build_json_request(
                Method::DELETE,
                "/worker-1",
                Some(&token),
                None,
            ))
            .await
            .expect("delete team member agent");
        assert_eq!(delete_resp.status(), StatusCode::OK);

        let remaining_agents: i64 = sqlx::query("SELECT COUNT(*) AS cnt FROM agents WHERE id = ?1")
            .bind("worker-1")
            .fetch_one(&state.db)
            .await
            .expect("count deleted agent rows")
            .get("cnt");
        assert_eq!(remaining_agents, 0);

        let team_row = sqlx::query("SELECT spec_json FROM team_definitions WHERE id = ?1")
            .bind(&team_id)
            .fetch_one(&state.db)
            .await
            .expect("load updated team definition");
        let spec_json = team_row.get::<String, _>("spec_json");
        assert!(
            !spec_json.contains("worker-1"),
            "deleted worker reference should still be pruned from team spec"
        );
    }

    #[tokio::test]
    async fn list_agents_hides_team_working_member_agent() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());

        let workdir = std::env::temp_dir().join(format!("agenthub-list-agents-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&workdir).expect("create workdir");
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO safe_paths (path, created_at)
            VALUES (?1, ?2)
            "#,
        )
        .bind(workdir.to_string_lossy().to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert safe path");

        let hidden_member = state
            .agents
            .create_agent(crate::agent::AgentConfig {
                name: "team-hidden-member".to_string(),
                workdir: workdir.to_string_lossy().to_string(),
                command: "/bin/sh".to_string(),
                args: vec!["-lc".to_string(), "sleep 10".to_string()],
                target_node_id: None,
                worktree_mode: crate::agent::WorktreeMode::UseExisting,
                worktree_repo: None,
                worktree_ref: None,
                code_mode: false,
                agent_loop_enabled: false,
                agent_loop_idle_seconds: None,
                agent_loop_prompt: None,
            })
            .await
            .expect("create hidden member");

        let visible_agent = state
            .agents
            .create_agent(crate::agent::AgentConfig {
                name: "visible-agent".to_string(),
                workdir: workdir.to_string_lossy().to_string(),
                command: "/bin/sh".to_string(),
                args: vec!["-lc".to_string(), "sleep 10".to_string()],
                target_node_id: None,
                worktree_mode: crate::agent::WorktreeMode::UseExisting,
                worktree_repo: None,
                worktree_ref: None,
                code_mode: false,
                agent_loop_enabled: false,
                agent_loop_idle_seconds: None,
                agent_loop_prompt: None,
            })
            .await
            .expect("create visible agent");

        let hidden_session = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at)
            VALUES (?1, ?2, 'running', ?3)
            "#,
        )
        .bind(&hidden_session)
        .bind(&hidden_member.id)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert hidden session");

        let team_run = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO team_runs (id, team_id, context_id, status, input_json, created_at)
            VALUES (?1, ?2, ?3, 'working', '{}', ?4)
            "#,
        )
        .bind(&team_run)
        .bind(Uuid::new_v4().to_string())
        .bind(Uuid::new_v4().to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert team run");

        sqlx::query(
            r#"
            INSERT INTO team_steps (
                id,
                run_id,
                step_key,
                member_id,
                remote_task_id,
                status,
                attempt,
                depends_on_json
            )
            VALUES (?1, ?2, 'step-1', ?3, ?4, 'working', 0, '[]')
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&team_run)
        .bind(&hidden_member.id)
        .bind(&hidden_session)
        .execute(&state.db)
        .await
        .expect("insert team step");

        let list_resp = app
            .clone()
            .oneshot(build_json_request(Method::GET, "/", Some(&token), None))
            .await
            .expect("list agents");
        assert_eq!(list_resp.status(), StatusCode::OK);
        let listed = decode_json_body(list_resp).await;
        let ids: Vec<String> = listed
            .as_array()
            .expect("agents list")
            .iter()
            .filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_string))
            .collect();

        assert!(
            !ids.iter().any(|id| id == &hidden_member.id),
            "team working member should be hidden from /api/agents list"
        );
        assert!(
            ids.iter().any(|id| id == &visible_agent.id),
            "non-team agent should stay visible in /api/agents list"
        );

        sqlx::query(
            r#"
            UPDATE team_steps
            SET status = 'completed', ended_at = ?1
            WHERE run_id = ?2
            "#,
        )
        .bind(now + 1)
        .bind(&team_run)
        .execute(&state.db)
        .await
        .expect("complete team step");
        sqlx::query(
            r#"
            UPDATE team_runs
            SET status = 'completed', ended_at = ?1
            WHERE id = ?2
            "#,
        )
        .bind(now + 1)
        .bind(&team_run)
        .execute(&state.db)
        .await
        .expect("complete team run");

        let list_after_complete_resp = app
            .oneshot(build_json_request(Method::GET, "/", Some(&token), None))
            .await
            .expect("list agents after team completion");
        assert_eq!(list_after_complete_resp.status(), StatusCode::OK);
        let listed_after_complete = decode_json_body(list_after_complete_resp).await;
        let ids_after_complete: Vec<String> = listed_after_complete
            .as_array()
            .expect("agents list after completion")
            .iter()
            .filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_string))
            .collect();
        assert!(
            ids_after_complete.iter().any(|id| id == &hidden_member.id),
            "member agent should be visible again after team step completed"
        );

        remove_dir_best_effort(&workdir);
    }

    #[tokio::test]
    async fn list_agents_hides_team_forge_source_agents() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());

        let workdir =
            std::env::temp_dir().join(format!("agenthub-list-agents-source-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&workdir).expect("create workdir");
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO safe_paths (path, created_at)
            VALUES (?1, ?2)
            "#,
        )
        .bind(workdir.to_string_lossy().to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert safe path");

        let manual_agent = state
            .agents
            .create_agent(crate::agent::AgentConfig {
                name: "manual-visible".to_string(),
                workdir: workdir.to_string_lossy().to_string(),
                command: "/bin/sh".to_string(),
                args: vec!["-lc".to_string(), "sleep 10".to_string()],
                target_node_id: None,
                worktree_mode: crate::agent::WorktreeMode::UseExisting,
                worktree_repo: None,
                worktree_ref: None,
                code_mode: false,
                agent_loop_enabled: false,
                agent_loop_idle_seconds: None,
                agent_loop_prompt: None,
            })
            .await
            .expect("create manual agent");

        let team_forge_agent = state
            .agents
            .create_agent_with_source(
                crate::agent::AgentConfig {
                    name: "team-forge-hidden".to_string(),
                    workdir: workdir.to_string_lossy().to_string(),
                    command: "/bin/sh".to_string(),
                    args: vec!["-lc".to_string(), "sleep 10".to_string()],
                    target_node_id: None,
                    worktree_mode: crate::agent::WorktreeMode::UseExisting,
                    worktree_repo: None,
                    worktree_ref: None,
                    code_mode: false,
                    agent_loop_enabled: false,
                    agent_loop_idle_seconds: None,
                    agent_loop_prompt: None,
                },
                "team_forge",
            )
            .await
            .expect("create team forge agent");

        let list_resp = app
            .oneshot(build_json_request(Method::GET, "/", Some(&token), None))
            .await
            .expect("list agents");
        assert_eq!(list_resp.status(), StatusCode::OK);
        let listed = decode_json_body(list_resp).await;
        let ids: Vec<String> = listed
            .as_array()
            .expect("agents list")
            .iter()
            .filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_string))
            .collect();

        assert!(
            ids.iter().any(|id| id == &manual_agent.id),
            "manual agent should stay visible"
        );
        assert!(
            !ids.iter().any(|id| id == &team_forge_agent.id),
            "team_forge agent should be hidden from /api/agents list"
        );

        remove_dir_best_effort(&workdir);
    }

    #[tokio::test]
    async fn agent_time_trigger_routes_create_list_and_cancel() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'manual', 'created', ?7, ?8)
            "#,
        )
        .bind("trigger-agent")
        .bind("trigger-agent")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert trigger agent");

        let create_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/trigger-agent/triggers",
                Some(&token),
                Some(json!({
                    "delay_seconds": 60,
                    "message": "Re-check flaky test results."
                })),
            ))
            .await
            .expect("create trigger");
        assert_eq!(create_resp.status(), StatusCode::OK);
        let created = decode_json_body(create_resp).await;
        let trigger_id = created["id"].as_str().expect("trigger id").to_string();

        let list_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::GET,
                "/trigger-agent/triggers",
                Some(&token),
                None,
            ))
            .await
            .expect("list triggers");
        assert_eq!(list_resp.status(), StatusCode::OK);
        let listed = decode_json_body(list_resp).await;
        assert_eq!(listed.as_array().map(Vec::len), Some(1));
        assert_eq!(listed[0]["id"], Value::from(trigger_id.clone()));
        assert_eq!(listed[0]["status"], Value::from("scheduled"));

        let cancel_resp = app
            .oneshot(build_json_request(
                Method::POST,
                &format!("/trigger-agent/triggers/{trigger_id}/cancel"),
                Some(&token),
                Some(json!({})),
            ))
            .await
            .expect("cancel trigger");
        assert_eq!(cancel_resp.status(), StatusCode::OK);
        let canceled = decode_json_body(cancel_resp).await;
        assert_eq!(canceled["status"], Value::from("ok"));
    }

    #[tokio::test]
    async fn set_agent_loop_route_updates_agent_config_without_blocking() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'manual', 'created', ?7, ?8)
            "#,
        )
        .bind("loop-agent")
        .bind("loop-agent")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert loop agent");

        let enable_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/loop-agent/agent_loop",
                Some(&token),
                Some(json!({
                    "enabled": true,
                    "idle_seconds": 900,
                    "prompt": "Resume by checking the current ACP thread and taking the next step."
                })),
            ))
            .await
            .expect("enable agent loop");
        assert_eq!(enable_resp.status(), StatusCode::OK);
        let enabled_body = decode_json_body(enable_resp).await;
        assert_eq!(enabled_body["status"], Value::from("ok"));

        let enabled_row = sqlx::query(
            "SELECT agent_loop_enabled, agent_loop_idle_seconds, agent_loop_prompt FROM agents WHERE id = ?1",
        )
        .bind("loop-agent")
        .fetch_one(&state.db)
        .await
        .expect("load enabled loop agent row");
        assert_eq!(enabled_row.get::<i64, _>("agent_loop_enabled"), 1);
        assert_eq!(enabled_row.get::<i64, _>("agent_loop_idle_seconds"), 900);
        assert_eq!(
            enabled_row.get::<String, _>("agent_loop_prompt"),
            "Resume by checking the current ACP thread and taking the next step."
        );

        let invalid_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/loop-agent/agent_loop",
                Some(&token),
                Some(json!({
                    "enabled": true,
                    "idle_seconds": 900
                })),
            ))
            .await
            .expect("reject loop config without prompt");
        assert_eq!(invalid_resp.status(), StatusCode::BAD_REQUEST);
        let invalid_text = decode_text_body(invalid_resp).await;
        assert!(
            invalid_text.contains("agent_loop.prompt is required"),
            "unexpected error: {invalid_text}"
        );

        let disable_resp = app
            .oneshot(build_json_request(
                Method::POST,
                "/loop-agent/agent_loop",
                Some(&token),
                Some(json!({
                    "enabled": false
                })),
            ))
            .await
            .expect("disable agent loop");
        assert_eq!(disable_resp.status(), StatusCode::OK);

        let disabled_row = sqlx::query(
            "SELECT agent_loop_enabled, agent_loop_idle_seconds, agent_loop_prompt FROM agents WHERE id = ?1",
        )
        .bind("loop-agent")
        .fetch_one(&state.db)
        .await
        .expect("load disabled loop agent row");
        assert_eq!(disabled_row.get::<i64, _>("agent_loop_enabled"), 0);
        assert_eq!(disabled_row.get::<i64, _>("agent_loop_idle_seconds"), 900);
        assert_eq!(
            disabled_row.get::<String, _>("agent_loop_prompt"),
            "Resume by checking the current ACP thread and taking the next step."
        );
    }

    #[tokio::test]
    async fn respond_permission_route_rejects_permission_from_other_agent() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());
        let now = chrono::Utc::now().timestamp();

        for agent_id in ["agent-a", "agent-b"] {
            sqlx::query(
                r#"
                INSERT INTO agents (
                    id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'manual', 'created', ?7, ?8)
                "#,
            )
            .bind(agent_id)
            .bind(agent_id)
            .bind("/tmp")
            .bind("agenthub-codex-acp")
            .bind("[]")
            .bind("use_existing")
            .bind(now)
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert agent");
        }
        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind("session-a")
        .bind("agent-a")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert agent session");

        sqlx::query(
            r#"
            INSERT INTO acp_permission_requests (
                id,
                agent_id,
                session_id,
                acp_session_id,
                tool_call_id,
                options_json,
                tool_call_json,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8)
            "#,
        )
        .bind("perm-owned-by-agent-a")
        .bind("agent-a")
        .bind("session-a")
        .bind("acp-session-a")
        .bind("tool-call-a")
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

        let response = app
            .oneshot(build_json_request(
                Method::POST,
                "/agent-b/permissions/perm-owned-by-agent-a/respond",
                Some(&token),
                Some(json!({
                    "option_id": "allow"
                })),
            ))
            .await
            .expect("respond permission route");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = decode_text_body(response).await;
        assert!(
            body.contains("permission request not found for agent"),
            "unexpected error body: {body}"
        );
    }

    #[tokio::test]
    async fn respond_permission_route_reports_already_resolved_after_first_response() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'manual', 'created', ?7, ?8)
            "#,
        )
        .bind("agent-resolve")
        .bind("agent-resolve")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert agent");
        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind("session-resolve")
        .bind("agent-resolve")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert agent session");
        sqlx::query(
            r#"
            INSERT INTO acp_permission_requests (
                id,
                agent_id,
                session_id,
                acp_session_id,
                tool_call_id,
                options_json,
                tool_call_json,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8)
            "#,
        )
        .bind("perm-resolve")
        .bind("agent-resolve")
        .bind("session-resolve")
        .bind("acp-session-resolve")
        .bind("tool-call-resolve")
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

        let first = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/agent-resolve/permissions/perm-resolve/respond",
                Some(&token),
                Some(json!({
                    "option_id": "allow"
                })),
            ))
            .await
            .expect("first respond permission route");
        assert_eq!(first.status(), StatusCode::OK);
        let first_body = decode_json_body(first).await;
        assert_eq!(first_body["status"], "ok");

        let second = app
            .oneshot(build_json_request(
                Method::POST,
                "/agent-resolve/permissions/perm-resolve/respond",
                Some(&token),
                Some(json!({
                    "outcome": "cancelled"
                })),
            ))
            .await
            .expect("second respond permission route");
        assert_eq!(second.status(), StatusCode::OK);
        let second_body = decode_json_body(second).await;
        assert_eq!(second_body["status"], "already_resolved");
    }
}
