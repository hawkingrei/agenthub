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

use agenthub_auth_domain::UserCapability;
use agenthub_config::{
    normalize_optional_codex_acp_mode_id, normalize_optional_runtime_model,
    normalize_optional_thinking_level,
};

use crate::acp::{
    AcpActorSkillContext, AcpPermissionRecord, AcpPermissionRespondResult, DEFAULT_ACTOR_CHANNEL,
};
use crate::agent::{
    AgentConfig, AgentRecord, AgentSendInputError, AgentTimeTriggerCreateInput,
    AgentTimeTriggerManager, AgentTimeTriggerRecord, WorktreeMode, normalize_target_node_id,
};
use crate::api::authz::require_capability;
use crate::api::error::ApiError;
use crate::api::ok_response;
use crate::api::teams::prune_deleted_agent_from_team_specs;
use crate::api::uploads::{
    DownloadRequest, UploadRequest, download_scoped_object, upload_scoped_object,
};
use crate::object_upload::{ObjectUploadKind, ObjectUploadOwnerScope};
use crate::state::AppState;
use crate::team::effective_team_member_skills;

const AGENT_SOURCE_MANUAL: &str = "manual";
const AGENT_SOURCE_TEAM_FORGE: &str = "team_forge";
const AGENT_EVENTS_PAGE_LIMIT: i64 = 20;

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
    pub codex_acp_default_mode: Option<String>,
    pub runtime_model: Option<String>,
    pub thinking_level: Option<String>,
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
    pub team_member_role: Option<String>,
    pub skills: Vec<String>,
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
    pub codex_acp_default_mode: Option<String>,
    pub runtime_model: Option<String>,
    pub thinking_level: Option<String>,
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
pub struct SetCodexAcpDefaultModeRequest {
    pub mode_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SetRuntimeProfileRequest {
    pub runtime_model: Option<String>,
    pub thinking_level: Option<String>,
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
        .route("/{id}/uploads", post(upload_agent_object))
        .route("/{id}/uploads/downloads", post(download_agent_object))
        .route("/{id}/images", post(upload_agent_image))
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
        .route("/{id}/events/{event_id}", get(get_event))
        .route("/{id}/code_mode", post(set_code_mode))
        .route(
            "/{id}/codex_acp_default_mode",
            post(set_codex_acp_default_mode),
        )
        .route("/{id}/runtime_profile", post(set_runtime_profile))
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
    let CreateAgentRequest {
        name,
        workdir,
        command,
        args,
        target_node_id,
        source,
        worktree_mode,
        worktree_repo,
        worktree_ref,
        code_mode,
        codex_acp_default_mode,
        runtime_model,
        thinking_level,
        agent_loop_enabled,
        agent_loop_idle_seconds,
        agent_loop_prompt,
    } = payload;
    let name = normalize_required_create_agent_field("name", name)?;
    let command = normalize_required_create_agent_field("command", command)?;
    let worktree_mode = parse_worktree_mode(worktree_mode.as_deref())?;
    let worktree_repo = normalize_optional_request_field("worktree_repo", worktree_repo)?;
    let worktree_ref = normalize_optional_request_field("worktree_ref", worktree_ref)?;
    let codex_acp_default_mode =
        normalize_codex_acp_default_mode_request(codex_acp_default_mode.as_deref())?;
    let (runtime_model, thinking_level) = normalize_runtime_profile_request(
        state.agents.acp_provider_for_agent(&command, &args),
        runtime_model.as_deref(),
        thinking_level.as_deref(),
    )?;
    let agent_loop_enabled = agent_loop_enabled.unwrap_or(false);
    let agent_loop_prompt =
        normalize_optional_request_field("agent_loop.prompt", agent_loop_prompt)?;
    validate_create_agent_loop_config(
        agent_loop_enabled,
        agent_loop_idle_seconds,
        agent_loop_prompt.as_deref(),
    )?;
    let source = parse_agent_source(source.as_deref())?;
    let target_node_id = normalize_target_node_id(target_node_id.as_deref());
    require_create_agent_capability(&headers, &state, target_node_id.as_deref()).await?;
    let default_worktree_root = resolve_create_agent_default_worktree_root(
        &state,
        target_node_id.as_deref(),
        &worktree_mode,
    )
    .await?;
    let workdir = resolve_create_agent_workdir(
        &workdir,
        &name,
        &worktree_mode,
        default_worktree_root.as_deref(),
    )?;
    let config = AgentConfig {
        name,
        workdir,
        command,
        args,
        target_node_id,
        worktree_mode,
        worktree_repo,
        worktree_ref,
        code_mode: code_mode.unwrap_or(true),
        codex_acp_default_mode,
        runtime_model,
        thinking_level,
        agent_loop_enabled,
        agent_loop_idle_seconds,
        agent_loop_prompt,
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

async fn require_create_agent_capability(
    headers: &HeaderMap,
    state: &AppState,
    target_node_id: Option<&str>,
) -> Result<(), ApiError> {
    let user = require_capability(headers, state, UserCapability::AgentsManage).await?;
    if target_node_id.is_some() && !user.has_capability(UserCapability::NodesManage) {
        return Err(ApiError::unauthorized(&format!(
            "{} required",
            UserCapability::NodesManage.as_str()
        )));
    }
    Ok(())
}

async fn list_agents(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentRecord>>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let agents = state.agents.list_agents().await?;
    Ok(Json(agents))
}

async fn get_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Json<AgentRecord>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let agent = state.agents.get_agent(&agent_id).await?;
    Ok(Json(agent))
}

async fn get_agent_discovery_card(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Json<AgentDiscoveryCardResponse>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let agent = state.agents.get_agent(&agent_id).await?;
    let provider = state
        .agents
        .acp_provider_for_agent(&agent.command, &agent.args);
    let member_profile = resolve_team_member_profile(&state, user.id.as_str(), &agent.id).await;
    Ok(Json(build_agent_discovery_card(
        &agent,
        provider,
        member_profile.as_ref(),
    )))
}

async fn start_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    body: Bytes,
) -> Result<Json<StartAgentResponse>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
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
    let _user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    state.agents.stop_agent(&agent_id).await?;
    Ok(ok_response())
}

async fn delete_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::AgentsManage).await?;
    let _ = state.agents.stop_agent(&agent_id).await;
    state.agents.delete_agent(&agent_id).await?;
    if let Err(err) = prune_deleted_agent_from_team_specs(&state, &agent_id).await {
        tracing::warn!(
            agent_id = %agent_id,
            error = ?err,
            "delete_agent completed after best-effort team spec prune failed"
        );
    }
    Ok(ok_response())
}

async fn send_input(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SendInputRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let input = payload.input;
    if input.trim().is_empty() {
        return Err(ApiError::bad_request("input is required"));
    }
    let message_id = normalize_optional_request_field("message_id", payload.message_id)?;
    let session_id = normalize_optional_request_field("session_id", payload.session_id)?;
    match state
        .agents
        .send_input(
            &agent_id,
            &input,
            message_id.as_deref(),
            session_id.as_deref(),
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
    Ok(ok_response())
}

async fn upload_agent_object(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<UploadRequest>,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    upload_agent_scoped_object(state, headers, agent_id, payload, ObjectUploadKind::Object).await
}

async fn upload_agent_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<UploadRequest>,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    upload_agent_scoped_object(state, headers, agent_id, payload, ObjectUploadKind::Image).await
}

async fn download_agent_object(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<DownloadRequest>,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::AgentsManage).await?;
    let agent = state.agents.get_agent(&agent_id).await?;
    download_scoped_object(
        State(state),
        &user,
        ObjectUploadOwnerScope::Agent(agent.id),
        payload,
    )
    .await
}

async fn upload_agent_scoped_object(
    state: AppState,
    headers: HeaderMap,
    agent_id: String,
    payload: UploadRequest,
    kind: ObjectUploadKind,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::AgentsManage).await?;
    let agent = state.agents.get_agent(&agent_id).await?;
    upload_scoped_object(
        State(state),
        &user,
        ObjectUploadOwnerScope::Agent(agent.id),
        payload,
        kind,
    )
    .await
}

async fn create_agent_time_trigger(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<CreateAgentTimeTriggerRequest>,
) -> Result<Json<AgentTimeTriggerRecord>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
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
    let _user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
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
    let _user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    ensure_agent_exists(&state, &agent_id).await?;
    let manager = AgentTimeTriggerManager::new(state.db.clone());
    let canceled = manager.cancel_trigger(&agent_id, &trigger_id).await?;
    if !canceled {
        return Err(ApiError::not_found("agent time trigger not found"));
    }
    Ok(ok_response())
}

async fn list_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Query(query): Query<ListEventsQuery>,
) -> Result<Json<Vec<crate::agent::AgentEvent>>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let limit = query
        .limit
        .unwrap_or(AGENT_EVENTS_PAGE_LIMIT)
        .clamp(1, AGENT_EVENTS_PAGE_LIMIT);
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

async fn get_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((agent_id, event_id)): Path<(String, i64)>,
) -> Result<Json<crate::agent::AgentEvent>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let event = state
        .agents
        .get_event(&agent_id, event_id)
        .await
        .map_err(|err| {
            if err.to_string().contains("agent event not found") {
                ApiError::not_found("agent event not found")
            } else {
                ApiError::from(err)
            }
        })?;
    Ok(Json(event))
}

async fn set_code_mode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetCodeModeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::AgentsManage).await?;
    state
        .agents
        .set_code_mode(&agent_id, payload.code_mode)
        .await?;
    Ok(ok_response())
}

async fn set_codex_acp_default_mode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetCodexAcpDefaultModeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::AgentsManage).await?;
    let mode_id = normalize_codex_acp_default_mode_request(payload.mode_id.as_deref())?;
    state
        .agents
        .set_codex_acp_default_mode(&agent_id, mode_id.as_deref())
        .await?;
    Ok(ok_response())
}

async fn set_runtime_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetRuntimeProfileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::AgentsManage).await?;
    let agent = state.agents.get_agent(&agent_id).await?;
    let provider = state
        .agents
        .acp_provider_for_agent(&agent.command, &agent.args);
    let (runtime_model, thinking_level) = normalize_runtime_profile_request(
        provider,
        payload.runtime_model.as_deref(),
        payload.thinking_level.as_deref(),
    )?;
    state
        .agents
        .set_runtime_profile(
            &agent_id,
            runtime_model.as_deref(),
            thinking_level.as_deref(),
        )
        .await?;
    Ok(ok_response())
}

async fn set_agent_loop(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetAgentLoopRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::AgentsManage).await?;
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
    Ok(ok_response())
}

async fn clear_acp_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<ClearSessionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
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
    Ok(ok_response())
}

async fn set_acp_mode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetAcpModeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::AgentsManage).await?;
    state
        .agents
        .set_acp_mode(&agent_id, &payload.mode_id)
        .await?;
    Ok(ok_response())
}

async fn set_acp_model(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetAcpModelRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::AgentsManage).await?;
    state
        .agents
        .set_acp_model(&agent_id, &payload.model_id)
        .await?;
    Ok(ok_response())
}

async fn set_acp_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetAcpConfigRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::AgentsManage).await?;
    state
        .agents
        .set_acp_config(&agent_id, &payload.config_id, &payload.value)
        .await?;
    Ok(ok_response())
}

async fn cancel_acp(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    state.agents.cancel_acp(&agent_id).await?;
    Ok(ok_response())
}

async fn list_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Query(query): Query<PermissionListQuery>,
) -> Result<Json<Vec<AcpPermissionRecord>>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
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
    let _user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
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
        agent_client_protocol::schema::v1::RequestPermissionOutcome::Selected(
            agent_client_protocol::schema::v1::SelectedPermissionOutcome::new(option_id.clone()),
        )
    } else {
        match payload.outcome.as_deref() {
            Some("cancelled") | None => {
                agent_client_protocol::schema::v1::RequestPermissionOutcome::Cancelled
            }
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

fn normalize_required_create_agent_field(
    field_name: &'static str,
    value: String,
) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(&format!("{field_name} is required")));
    }
    Ok(trimmed.to_string())
}

fn normalize_optional_request_field(
    field_name: &'static str,
    value: Option<String>,
) -> Result<Option<String>, ApiError> {
    match value {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(ApiError::bad_request(&format!(
                    "{field_name} must not be blank"
                )));
            }
            Ok(Some(trimmed.to_string()))
        }
        None => Ok(None),
    }
}

fn normalize_codex_acp_default_mode_request(
    value: Option<&str>,
) -> Result<Option<String>, ApiError> {
    let normalized = normalize_optional_codex_acp_mode_id(value);
    if let Some(mode_id) = normalized.as_deref()
        && !matches!(mode_id, "read-only" | "auto" | "full-access")
    {
        return Err(ApiError::bad_request(
            "codex_acp_default_mode must be one of read-only, auto, full-access, or yolo",
        ));
    }
    Ok(normalized)
}

/// Normalize and validate the runtime profile fields from a create/update request, and gate them to
/// providers that support runtime profiles. `runtime_model` accepts any non-blank string (unknown
/// model names are allowed); `thinking_level` must be one of `low|medium|high|max`. A profile may only
/// be set when the agent's derived ACP provider is Codex or Claude — Gemini/Kimi and non-ACP agents are
/// rejected. Returns the normalized `(runtime_model, thinking_level)`.
fn normalize_runtime_profile_request(
    provider: Option<&str>,
    runtime_model: Option<&str>,
    thinking_level: Option<&str>,
) -> Result<(Option<String>, Option<String>), ApiError> {
    let model = normalize_optional_runtime_model(runtime_model);
    let level =
        match thinking_level
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            None => None,
            Some(value) => Some(normalize_optional_thinking_level(Some(value)).ok_or_else(
                || ApiError::bad_request("thinking_level must be one of low, medium, high, or max"),
            )?),
        };
    if (model.is_some() || level.is_some()) && !matches!(provider, Some("codex") | Some("claude")) {
        return Err(ApiError::bad_request(
            "runtime_model and thinking_level require a Codex or Claude ACP agent",
        ));
    }
    Ok((model, level))
}

fn validate_create_agent_loop_config(
    enabled: bool,
    idle_seconds: Option<i64>,
    prompt: Option<&str>,
) -> Result<(), ApiError> {
    if enabled && prompt.is_none() {
        return Err(ApiError::bad_request(
            "agent_loop.prompt is required when enabling agent loop",
        ));
    }
    if enabled && !idle_seconds.is_some_and(|value| (10..=86_400).contains(&value)) {
        return Err(ApiError::bad_request(
            "agent_loop.idle_seconds must be between 10 and 86400 when enabling agent loop",
        ));
    }
    Ok(())
}

fn parse_worktree_mode(value: Option<&str>) -> Result<WorktreeMode, ApiError> {
    match value
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        None | Some("use_existing") => Ok(WorktreeMode::UseExisting),
        Some("create_worktree") => Ok(WorktreeMode::CreateWorktree),
        Some("reuse_worktree") => Ok(WorktreeMode::ReuseWorktree),
        Some(_) => Err(ApiError::bad_request(
            "worktree_mode must be one of: use_existing, create_worktree, reuse_worktree",
        )),
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
    member_profile: Option<&TeamMemberProfileRecord>,
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
    let description = member_profile
        .and_then(|profile| {
            profile
                .description
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
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
            codex_acp_default_mode: agent.codex_acp_default_mode.clone(),
            runtime_model: agent.runtime_model.clone(),
            thinking_level: agent.thinking_level.clone(),
            agent_loop_enabled: agent.agent_loop_enabled,
            agent_loop_idle_seconds: agent.agent_loop_idle_seconds,
            target_node_id: agent.target_node_id.clone(),
            worktree_mode: agent.worktree_mode.clone(),
            worktree_repo: agent.worktree_repo.clone(),
            worktree_ref: agent.worktree_ref.clone(),
        },
        team_member_role: member_profile.map(|profile| profile.role.clone()),
        skills: member_profile
            .map(|profile| effective_team_member_skills(&profile.role))
            .unwrap_or_default(),
        capability_tags,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TeamMemberProfileRecord {
    role: String,
    description: Option<String>,
}

async fn resolve_team_member_profile(
    state: &AppState,
    user_id: &str,
    member_id: &str,
) -> Option<TeamMemberProfileRecord> {
    let teams = state.teams.list_teams().await.ok()?;
    teams
        .into_iter()
        .filter(|team| match team.owner_user_id.as_deref() {
            Some(owner_user_id) => owner_user_id == user_id,
            None => true,
        })
        .filter_map(|team| {
            resolve_member_profile_from_spec(&team.spec, member_id)
                .map(|profile| (team.updated_at, profile))
        })
        .max_by_key(|(updated_at, _)| *updated_at)
        .map(|(_, profile)| profile)
}

fn resolve_member_profile_from_spec(
    spec: &Value,
    member_id: &str,
) -> Option<TeamMemberProfileRecord> {
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
                let role = member_obj
                    .get("role")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("worker")
                    .to_string();
                let description = member_obj
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
                Some(TeamMemberProfileRecord { role, description })
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

    Ok(Some(AcpActorSkillContext {
        team_id,
        current_run_id,
        actor_id: actor_id.to_string(),
        default_channel,
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

    use agenthub_auth_domain::UserRole;
    use axum::body::{Body, Bytes, to_bytes};
    use axum::http::{Method, Request, StatusCode, header};
    use axum::response::IntoResponse;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use sqlx::Row;
    use sqlx::SqlitePool;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::acp::AcpPermissionService;
    use crate::agent::AgentManager;
    use crate::auth::AuthService;
    use crate::internal::client::InternalGrpcPeerClientConfig;
    use crate::internal::tls::InternalGrpcSecurityMode;
    use crate::object_upload::ObjectUploadService;
    use crate::push::PushService;
    use crate::state::AppState;
    use crate::team::TeamManager;
    use agenthub_config::{AppConfig, PushConfig, WebConfig};

    use super::{
        StartAgentActorRuntimeRequest, StartAgentRequest, TeamMemberProfileRecord, WorktreeMode,
        build_agent_discovery_card, map_create_agent_error, parse_agent_source,
        parse_optional_start_agent_request, parse_start_actor_runtime_context, parse_worktree_mode,
        resolve_create_agent_workdir, resolve_member_profile_from_spec, router,
        sanitize_worktree_segment,
    };

    #[test]
    fn parse_worktree_mode_defaults() {
        assert!(matches!(
            parse_worktree_mode(None).expect("default worktree mode"),
            WorktreeMode::UseExisting
        ));
        assert!(matches!(
            parse_worktree_mode(Some("use_existing")).expect("explicit use_existing"),
            WorktreeMode::UseExisting
        ));
    }

    #[test]
    fn parse_worktree_mode_explicit() {
        assert!(matches!(
            parse_worktree_mode(Some("create_worktree")).expect("create_worktree"),
            WorktreeMode::CreateWorktree
        ));
        assert!(matches!(
            parse_worktree_mode(Some("reuse_worktree")).expect("reuse_worktree"),
            WorktreeMode::ReuseWorktree
        ));
    }

    #[test]
    fn parse_worktree_mode_rejects_invalid_value() {
        let err = parse_worktree_mode(Some("invalid")).expect_err("invalid worktree mode");
        assert_eq!(
            err.into_response().status(),
            axum::http::StatusCode::BAD_REQUEST
        );
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
            codex_acp_default_mode: None,
            runtime_model: None,
            thinking_level: None,
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
            codex_acp_default_mode: None,
            runtime_model: None,
            thinking_level: None,
            agent_loop_enabled: false,
            agent_loop_idle_seconds: None,
            agent_loop_prompt: None,
            status: crate::agent::AgentStatus::Created,
            created_at: 1,
            updated_at: 2,
        };

        let profile = TeamMemberProfileRecord {
            role: "worker".to_string(),
            description: Some("Database schema owner".to_string()),
        };
        let card = build_agent_discovery_card(&agent, Some("codex"), Some(&profile));
        assert_eq!(card.description, "Database schema owner");
    }

    #[test]
    fn resolve_member_profile_from_spec_matches_member_entry() {
        let spec = json!({
            "spec_version": 1,
            "members": [
                {"member_id": "coordinator", "role": "coordinator", "description": "Lead architect"},
                {"member_id": "worker-a", "role": "worker", "description": "Primary implementer"},
                {"member_id": "worker-b", "role": "worker"}
            ]
        });
        let worker = resolve_member_profile_from_spec(&spec, "worker-a").expect("worker profile");
        assert_eq!(worker.role, "worker");
        assert_eq!(worker.description.as_deref(), Some("Primary implementer"));
        let no_description =
            resolve_member_profile_from_spec(&spec, "worker-b").expect("worker profile");
        assert_eq!(no_description.role, "worker");
        assert_eq!(no_description.description, None);
        assert_eq!(resolve_member_profile_from_spec(&spec, "missing"), None);
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
        let context = parse_start_actor_runtime_context(Some(StartAgentRequest {
            actor_runtime: Some(StartAgentActorRuntimeRequest {
                team_id: Some("team-7".to_string()),
                run_id: Some("run-7".to_string()),
                actor_id: "planner".to_string(),
                member_role: Some("coordinator".to_string()),
                channel: Some("coordination".to_string()),
            }),
        }))
        .expect("parse actor runtime context")
        .expect("context");
        assert_eq!(context.team_id.as_deref(), Some("team-7"));
        assert_eq!(context.current_run_id.as_deref(), Some("run-7"));
        assert_eq!(context.actor_id, "planner");
        assert_eq!(context.member_role.as_deref(), Some("coordinator"));
        assert_eq!(context.default_channel, "coordination");
    }

    #[test]
    fn parse_start_actor_runtime_context_defaults_channel() {
        let context = parse_start_actor_runtime_context(Some(StartAgentRequest {
            actor_runtime: Some(StartAgentActorRuntimeRequest {
                team_id: Some("team-9".to_string()),
                run_id: Some("run-9".to_string()),
                actor_id: "planner".to_string(),
                member_role: None,
                channel: None,
            }),
        }))
        .expect("parse actor runtime context")
        .expect("context");
        assert_eq!(context.default_channel, "default");
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
            }),
        }))
        .expect_err("actor_id should be required");
        let _ = actor_id_err;
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
                codex_acp_default_mode TEXT,
                runtime_model TEXT,
                thinking_level TEXT,
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
            CREATE TABLE object_uploads (
                id TEXT PRIMARY KEY,
                owner_scope TEXT NOT NULL,
                backend TEXT NOT NULL,
                object_key TEXT NOT NULL,
                original_filename TEXT NOT NULL,
                content_type TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                sha256 TEXT NOT NULL,
                public_url TEXT,
                created_by_actor_id TEXT NOT NULL,
                publish_state TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                published_at INTEGER,
                cleanup_after INTEGER
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create object_uploads");
        sqlx::query(
            r#"
            CREATE UNIQUE INDEX idx_object_uploads_object_key
            ON object_uploads(object_key)
            "#,
        )
        .execute(db)
        .await
        .expect("create object upload object key index");

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
                group_id TEXT,
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
                passkey_enabled: None,
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
            true,
            permissions.clone(),
            auth.clone(),
            internal_peer_client,
        ));
        let teams = Arc::new(TeamManager::new_with_event_dbs(db.clone(), event_dbs));
        let object_uploads = Arc::new(test_object_upload_service(db.clone()));
        AppState {
            db,
            linker_http: crate::linkers::AppLinkerService::default_http_client(),
            agents,
            teams,
            push,
            auth,
            acp_permissions: permissions,
            object_uploads,
            agent_node_join_bootstrap: crate::agent::AgentNodeJoinBootstrapInfo::disabled(),
            default_worktree_root: config.default_worktree_root(),
            body_store: None,
            message_index: None,
            message_read_repair: None,
        }
    }

    fn test_object_upload_service(db: SqlitePool) -> ObjectUploadService {
        let root = std::env::temp_dir()
            .join(format!("agenthub-api-agents-objects-{}", Uuid::new_v4()))
            .to_string_lossy()
            .to_string();
        let config = AppConfig {
            object_store: Some(agenthub_config::ObjectStoreConfig {
                backend: Some("fs".to_string()),
                root: Some(root),
                public_base_url: None,
                prefix: None,
                bucket: None,
                endpoint: None,
                region: None,
                access_key_id_env: None,
                secret_access_key_env: None,
                download_max_bytes: Some(1024 * 1024),
                download_max_redirects: Some(3),
                download_timeout_seconds: Some(10),
                download_retry_attempts: Some(1),
                download_retry_backoff_millis: Some(0),
                download_max_concurrent_per_host: Some(4),
                download_allow_private_networks: Some(true),
                download_allowed_hosts: None,
                download_denied_hosts: None,
            }),
            ..Default::default()
        };
        ObjectUploadService::from_config(db, &config).expect("create object upload service")
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

    async fn create_role_auth_token(state: &AppState, role: UserRole) -> String {
        let user_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        let role_str = role.as_str();
        sqlx::query(
            r#"
            INSERT INTO users (id, username, display_name, role, password_hash, created_at)
            VALUES (?, ?, ?, ?, NULL, ?)
            "#,
        )
        .bind(&user_id)
        .bind(format!("{role_str}-{}", Uuid::new_v4()))
        .bind(role_str)
        .bind(role_str)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert role user");

        if role == UserRole::Device {
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS devices (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    user_agent TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    last_login_at INTEGER,
                    FOREIGN KEY(user_id) REFERENCES users(id)
                )
                "#,
            )
            .execute(&state.db)
            .await
            .expect("create devices table");
            sqlx::query(
                r#"
                INSERT INTO devices (id, user_id, name, user_agent, status, created_at)
                VALUES (?, ?, ?, ?, 'active', ?)
                "#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&user_id)
            .bind("Test Device")
            .bind("agenthub-test")
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert active device");
        }

        state
            .auth
            .create_session(&user_id)
            .await
            .expect("create role session token")
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

    fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    async fn spawn_download_source(bytes: Vec<u8>) -> String {
        let bytes = Arc::new(bytes);
        let app = axum::Router::new().route(
            "/artifact",
            axum::routing::get({
                let bytes = Arc::clone(&bytes);
                move || {
                    let bytes = Arc::clone(&bytes);
                    async move { bytes.as_ref().clone() }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind download source");
        let address = listener.local_addr().expect("read download source address");
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("serve download source");
        });
        format!("http://{address}/artifact")
    }

    #[tokio::test]
    async fn agent_inspect_routes_require_runtime_inspect_capability() {
        let state = build_test_state().await;
        let viewer_token = create_role_auth_token(&state, UserRole::Viewer).await;
        let device_token = create_role_auth_token(&state, UserRole::Device).await;
        let app = router(state);

        let viewer_response = app
            .clone()
            .oneshot(build_json_request(
                Method::GET,
                "/",
                Some(&viewer_token),
                None,
            ))
            .await
            .expect("list agents with viewer role");
        assert_eq!(viewer_response.status(), StatusCode::OK);

        let denied_routes = [
            "/",
            "/missing-agent",
            "/missing-agent/.well-known/agent-card",
            "/missing-agent/events",
            "/missing-agent/events/1",
            "/missing-agent/permissions",
        ];
        for route in denied_routes {
            let response = app
                .clone()
                .oneshot(build_json_request(
                    Method::GET,
                    route,
                    Some(&device_token),
                    None,
                ))
                .await
                .expect("run denied inspect request");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{route}");
            let body = decode_json_body(response).await;
            assert_eq!(body["error"], json!("runtime:inspect required"), "{route}");
        }
    }

    #[tokio::test]
    async fn agent_runtime_routes_require_runtime_operate_capability() {
        let state = build_test_state().await;
        let operator_token = create_role_auth_token(&state, UserRole::Operator).await;
        let viewer_token = create_role_auth_token(&state, UserRole::Viewer).await;
        let app = router(state.clone());

        let denied_routes = vec![
            (Method::POST, "/missing-agent/start", None),
            (Method::POST, "/missing-agent/stop", None),
            (
                Method::POST,
                "/missing-agent/input",
                Some(json!({
                    "input": "hello"
                })),
            ),
            (
                Method::POST,
                "/missing-agent/triggers",
                Some(json!({
                    "delay_seconds": 60,
                    "message": "Re-check."
                })),
            ),
            (
                Method::POST,
                "/missing-agent/triggers/missing-trigger/cancel",
                None,
            ),
            (
                Method::POST,
                "/missing-agent/acp/session/clear",
                Some(json!({
                    "provider": "codex"
                })),
            ),
            (Method::POST, "/missing-agent/acp/cancel", None),
            (
                Method::POST,
                "/missing-agent/permissions/missing-permission/respond",
                Some(json!({
                    "outcome": "cancelled"
                })),
            ),
        ];

        for (method, route, payload) in denied_routes {
            let response = app
                .clone()
                .oneshot(build_json_request(
                    method,
                    route,
                    Some(&viewer_token),
                    payload,
                ))
                .await
                .expect("run denied runtime operate request");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{route}");
            let body = decode_json_body(response).await;
            assert_eq!(body["error"], json!("runtime:operate required"), "{route}");
        }

        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, 1, 'manual', 'created', ?, ?)
            "#,
        )
        .bind("runtime-capability-agent")
        .bind("runtime-capability-agent")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert runtime capability agent");

        let create_response = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/runtime-capability-agent/triggers",
                Some(&operator_token),
                Some(json!({
                    "delay_seconds": 60,
                    "message": "Re-check runtime capability gate."
                })),
            ))
            .await
            .expect("create trigger with operator");
        assert_eq!(create_response.status(), StatusCode::OK);
        let created = decode_json_body(create_response).await;
        let trigger_id = created["id"].as_str().expect("trigger id").to_string();

        let list_response = app
            .clone()
            .oneshot(build_json_request(
                Method::GET,
                "/runtime-capability-agent/triggers",
                Some(&viewer_token),
                None,
            ))
            .await
            .expect("list triggers with viewer");
        assert_eq!(list_response.status(), StatusCode::OK);
        let listed = decode_json_body(list_response).await;
        assert_eq!(listed.as_array().map(Vec::len), Some(1));
        assert_eq!(listed[0]["id"], Value::from(trigger_id.clone()));

        let cancel_response = app
            .oneshot(build_json_request(
                Method::POST,
                &format!("/runtime-capability-agent/triggers/{trigger_id}/cancel"),
                Some(&operator_token),
                None,
            ))
            .await
            .expect("cancel trigger with operator");
        assert_eq!(cancel_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn agent_management_routes_require_agents_manage_capability() {
        let state = build_test_state().await;
        let operator_token = create_role_auth_token(&state, UserRole::Operator).await;
        let viewer_token = create_role_auth_token(&state, UserRole::Viewer).await;
        let app = router(state.clone());
        let workdir = std::env::temp_dir()
            .join(format!("agenthub-managed-agent-{}", Uuid::new_v4()))
            .to_string_lossy()
            .to_string();
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?, ?)")
            .bind(&workdir)
            .bind(chrono::Utc::now().timestamp())
            .execute(&state.db)
            .await
            .expect("insert managed agent safe path");

        let create_payload = json!({
            "name": "managed-agent",
            "workdir": workdir,
            "command": "agenthub-codex-acp",
            "args": [],
            "worktree_mode": "use_existing"
        });
        let viewer_create = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&viewer_token),
                Some(create_payload.clone()),
            ))
            .await
            .expect("create agent with viewer");
        assert_eq!(viewer_create.status(), StatusCode::UNAUTHORIZED);
        let body = decode_json_body(viewer_create).await;
        assert_eq!(body["error"], json!("agents:manage required"));

        let operator_create = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&operator_token),
                Some(create_payload),
            ))
            .await
            .expect("create agent with operator");
        assert_eq!(operator_create.status(), StatusCode::OK);

        add_agent_node_support(&state.db).await;
        let remote_create = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&operator_token),
                Some(json!({
                    "name": "remote-managed-agent",
                    "workdir": "",
                    "command": "agenthub-codex-acp",
                    "args": [],
                    "target_node_id": "worker-node",
                    "worktree_mode": "use_existing"
                })),
            ))
            .await
            .expect("create remote agent with operator");
        assert_eq!(remote_create.status(), StatusCode::UNAUTHORIZED);
        let body = decode_json_body(remote_create).await;
        assert_eq!(body["error"], json!("nodes:manage required"));

        let denied_routes = vec![
            (Method::DELETE, "/missing-agent", None),
            (
                Method::POST,
                "/missing-agent/code_mode",
                Some(json!({
                    "code_mode": true
                })),
            ),
            (
                Method::POST,
                "/missing-agent/codex_acp_default_mode",
                Some(json!({
                    "mode_id": "auto"
                })),
            ),
            (
                Method::POST,
                "/missing-agent/runtime_profile",
                Some(json!({
                    "runtime_model": "gpt-5",
                    "thinking_level": "high"
                })),
            ),
            (
                Method::POST,
                "/missing-agent/agent_loop",
                Some(json!({
                    "enabled": false
                })),
            ),
            (
                Method::POST,
                "/missing-agent/acp/mode",
                Some(json!({
                    "mode_id": "default"
                })),
            ),
            (
                Method::POST,
                "/missing-agent/acp/model",
                Some(json!({
                    "model_id": "gpt-5"
                })),
            ),
            (
                Method::POST,
                "/missing-agent/acp/config",
                Some(json!({
                    "config_id": "approval_policy",
                    "value": "on-request"
                })),
            ),
        ];
        for (method, route, payload) in denied_routes {
            let response = app
                .clone()
                .oneshot(build_json_request(
                    method,
                    route,
                    Some(&viewer_token),
                    payload,
                ))
                .await
                .expect("run denied agent management request");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{route}");
            let body = decode_json_body(response).await;
            assert_eq!(body["error"], json!("agents:manage required"), "{route}");
        }
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
                group_id: None,
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
    async fn create_agent_route_rejects_remote_target_without_node_capability() {
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
                group_id: None,
            })
            .await
            .expect("create agent node");
        let token = create_role_auth_token(&state, UserRole::Operator).await;
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
            .expect("create remote-target agent without node capability");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = decode_json_body(response).await;
        assert_eq!(body["error"], "nodes:manage required");
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
                codex_acp_default_mode: None,
                runtime_model: None,
                thinking_level: None,
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
                group_id: None,
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
    async fn create_agent_route_rejects_blank_name() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state);

        let response = app
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "   ",
                    "workdir": "/tmp/ignored",
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 10"],
                    "code_mode": true
                })),
            ))
            .await
            .expect("create agent with blank name");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = decode_json_body(response).await;
        assert_eq!(body["error"], Value::from("name is required"));
    }

    #[tokio::test]
    async fn agent_upload_routes_publish_agent_scoped_metadata() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let viewer_token = create_role_auth_token(&state, UserRole::Viewer).await;
        let workdir = std::env::temp_dir()
            .join(format!("agenthub-upload-agent-{}", Uuid::new_v4()))
            .to_string_lossy()
            .to_string();
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind(&workdir)
            .bind(chrono::Utc::now().timestamp())
            .execute(&state.db)
            .await
            .expect("insert safe path");
        let agent = state
            .agents
            .create_agent(crate::agent::AgentConfig {
                name: "upload-agent".to_string(),
                workdir,
                command: "/bin/sh".to_string(),
                args: vec!["-lc".to_string(), "sleep 10".to_string()],
                target_node_id: None,
                worktree_mode: WorktreeMode::UseExisting,
                worktree_repo: None,
                worktree_ref: None,
                code_mode: true,
                codex_acp_default_mode: None,
                runtime_model: None,
                thinking_level: None,
                agent_loop_enabled: false,
                agent_loop_idle_seconds: None,
                agent_loop_prompt: None,
            })
            .await
            .expect("seed upload agent");
        let agent_id = agent.id;
        let app = router(state);

        let viewer_response = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/uploads"),
                Some(&viewer_token),
                Some(json!({
                    "file_name": "viewer.txt",
                    "content_type": "text/plain",
                    "bytes_base64": STANDARD.encode(b"viewer evidence"),
                    "expected_size_bytes": 15,
                    "expected_sha256": sha256_hex(b"viewer evidence")
                })),
            ))
            .await
            .expect("viewer upload agent object");
        assert_eq!(viewer_response.status(), StatusCode::UNAUTHORIZED);
        let viewer_body = decode_json_body(viewer_response).await;
        assert_eq!(viewer_body["error"], Value::from("agents:manage required"));

        let bytes = b"agent evidence";
        let sha256 = sha256_hex(bytes);
        let object_response = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/uploads"),
                Some(&token),
                Some(json!({
                    "file_name": "evidence.txt",
                    "content_type": "text/plain",
                    "bytes_base64": STANDARD.encode(bytes),
                    "expected_size_bytes": bytes.len(),
                    "expected_sha256": sha256
                })),
            ))
            .await
            .expect("upload agent object");
        let object_status = object_response.status();
        let object_upload = decode_json_body(object_response).await;
        assert_eq!(
            object_status,
            StatusCode::OK,
            "unexpected object upload body: {object_upload}"
        );
        assert_eq!(
            object_upload["owner_scope"],
            Value::from(format!("agents/{agent_id}"))
        );
        assert!(
            object_upload["object_key"]
                .as_str()
                .is_some_and(|value| value.starts_with(&format!("uploads/agents/{agent_id}/")))
        );
        assert_eq!(
            object_upload["original_filename"],
            Value::from("evidence.txt")
        );

        let download_bytes = b"agent downloaded evidence";
        let download_sha256 = sha256_hex(download_bytes);
        let download_response = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/uploads/downloads"),
                Some(&token),
                Some(json!({
                    "source_url": spawn_download_source(download_bytes.to_vec()).await,
                    "file_name": "downloaded.txt",
                    "content_type": "text/plain",
                    "expected_size_bytes": download_bytes.len(),
                    "expected_sha256": download_sha256
                })),
            ))
            .await
            .expect("download agent object");
        let download_status = download_response.status();
        let download_upload = decode_json_body(download_response).await;
        assert_eq!(
            download_status,
            StatusCode::OK,
            "unexpected object download body: {download_upload}"
        );
        assert_eq!(
            download_upload["owner_scope"],
            Value::from(format!("agents/{agent_id}"))
        );
        assert!(
            download_upload["object_key"]
                .as_str()
                .is_some_and(|value| value.starts_with(&format!("uploads/agents/{agent_id}/")))
        );
        assert_eq!(
            download_upload["original_filename"],
            Value::from("downloaded.txt")
        );
        assert_eq!(
            download_upload["size_bytes"],
            Value::from(download_bytes.len() as i64)
        );
        assert_eq!(download_upload["sha256"], Value::from(download_sha256));

        let image_bytes = [5_u8, 6, 7, 8];
        let image_sha256 = sha256_hex(image_bytes);
        let image_response = app
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/images"),
                Some(&token),
                Some(json!({
                    "file_name": "evidence.png",
                    "content_type": "image/png",
                    "bytes_base64": STANDARD.encode(image_bytes),
                    "expected_size_bytes": image_bytes.len(),
                    "expected_sha256": image_sha256
                })),
            ))
            .await
            .expect("upload agent image");
        assert_eq!(image_response.status(), StatusCode::OK);
        let image_upload = decode_json_body(image_response).await;
        assert_eq!(
            image_upload["owner_scope"],
            Value::from(format!("agents/{agent_id}"))
        );
        assert!(
            image_upload["object_key"]
                .as_str()
                .is_some_and(|value| value.starts_with(&format!("images/agents/{agent_id}/")))
        );
        assert_eq!(image_upload["content_type"], Value::from("image/png"));
    }

    #[tokio::test]
    async fn create_agent_route_rejects_blank_command() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state);

        let response = app
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "blank-command",
                    "workdir": "/tmp/ignored",
                    "command": "   ",
                    "args": ["-lc", "sleep 10"],
                    "code_mode": true
                })),
            ))
            .await
            .expect("create agent with blank command");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = decode_json_body(response).await;
        assert_eq!(body["error"], Value::from("command is required"));
    }

    #[tokio::test]
    async fn create_agent_route_rejects_invalid_worktree_mode() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state);

        let response = app
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "invalid-worktree-mode",
                    "workdir": "/tmp/ignored",
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 10"],
                    "worktree_mode": "invalid_mode",
                    "code_mode": true
                })),
            ))
            .await
            .expect("create agent with invalid worktree mode");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = decode_json_body(response).await;
        assert_eq!(
            body["error"],
            Value::from(
                "worktree_mode must be one of: use_existing, create_worktree, reuse_worktree"
            )
        );
    }

    #[tokio::test]
    async fn create_agent_route_validates_agent_loop_when_enabled() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state);

        let missing_prompt = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "agent-loop-no-prompt",
                    "workdir": "/tmp/ignored",
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 10"],
                    "agent_loop_enabled": true,
                    "agent_loop_idle_seconds": 900,
                    "code_mode": true
                })),
            ))
            .await
            .expect("create agent without loop prompt");
        assert_eq!(missing_prompt.status(), StatusCode::BAD_REQUEST);
        let missing_prompt_body = decode_json_body(missing_prompt).await;
        assert_eq!(
            missing_prompt_body["error"],
            Value::from("agent_loop.prompt is required when enabling agent loop")
        );

        let invalid_idle = app
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "agent-loop-invalid-idle",
                    "workdir": "/tmp/ignored",
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 10"],
                    "agent_loop_enabled": true,
                    "agent_loop_idle_seconds": 1,
                    "agent_loop_prompt": "Resume work",
                    "code_mode": true
                })),
            ))
            .await
            .expect("create agent with invalid loop idle");
        assert_eq!(invalid_idle.status(), StatusCode::BAD_REQUEST);
        let invalid_idle_body = decode_json_body(invalid_idle).await;
        assert_eq!(
            invalid_idle_body["error"],
            Value::from(
                "agent_loop.idle_seconds must be between 10 and 86400 when enabling agent loop"
            )
        );
    }

    #[tokio::test]
    async fn create_agent_route_normalizes_codex_acp_default_mode() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());
        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind("/tmp/codex-mode-agent")
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert safe path");

        let response = app
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "codex-mode-agent",
                    "workdir": "/tmp/codex-mode-agent",
                    "command": "agenthub-codex-acp",
                    "args": [],
                    "code_mode": true,
                    "codex_acp_default_mode": "yolo"
                })),
            ))
            .await
            .expect("create codex mode agent");

        assert_eq!(response.status(), StatusCode::OK);
        let body = decode_json_body(response).await;
        let agent_id = body["id"].as_str().expect("agent id");
        assert_eq!(body["codex_acp_default_mode"], Value::from("full-access"));
        let row = sqlx::query("SELECT codex_acp_default_mode FROM agents WHERE id = ?1")
            .bind(agent_id)
            .fetch_one(&state.db)
            .await
            .expect("load created agent row");
        assert_eq!(
            row.get::<Option<String>, _>("codex_acp_default_mode")
                .as_deref(),
            Some("full-access")
        );
    }

    #[tokio::test]
    async fn create_agent_route_persists_runtime_profile_for_codex() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());
        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind("/tmp/runtime-profile-agent")
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert safe path");

        let response = app
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "runtime-profile-agent",
                    "workdir": "/tmp/runtime-profile-agent",
                    "command": "agenthub-codex-acp",
                    "args": [],
                    "runtime_model": "gpt-5.4-codex",
                    "thinking_level": "HIGH"
                })),
            ))
            .await
            .expect("create runtime profile agent");

        assert_eq!(response.status(), StatusCode::OK);
        let body = decode_json_body(response).await;
        let agent_id = body["id"].as_str().expect("agent id");
        let row = sqlx::query("SELECT runtime_model, thinking_level FROM agents WHERE id = ?1")
            .bind(agent_id)
            .fetch_one(&state.db)
            .await
            .expect("load created agent row");
        assert_eq!(
            row.get::<Option<String>, _>("runtime_model").as_deref(),
            Some("gpt-5.4-codex")
        );
        assert_eq!(
            row.get::<Option<String>, _>("thinking_level").as_deref(),
            Some("high")
        );
    }

    #[tokio::test]
    async fn create_agent_route_rejects_invalid_thinking_level() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());
        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind("/tmp/bad-thinking-agent")
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert safe path");

        let response = app
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "bad-thinking-agent",
                    "workdir": "/tmp/bad-thinking-agent",
                    "command": "agenthub-codex-acp",
                    "args": [],
                    "thinking_level": "ultra"
                })),
            ))
            .await
            .expect("create bad thinking agent");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_agent_route_rejects_runtime_profile_for_non_acp_provider() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());
        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind("/tmp/non-acp-profile-agent")
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert safe path");

        let response = app
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "non-acp-profile-agent",
                    "workdir": "/tmp/non-acp-profile-agent",
                    "command": "bash",
                    "args": [],
                    "runtime_model": "gpt-5.4-codex"
                })),
            ))
            .await
            .expect("create non-acp profile agent");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn set_runtime_profile_route_updates_agent_config() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());
        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind("/tmp/set-runtime-profile-agent")
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert safe path");

        let create_response = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "set-runtime-profile-agent",
                    "workdir": "/tmp/set-runtime-profile-agent",
                    "command": "agenthub-codex-acp",
                    "args": []
                })),
            ))
            .await
            .expect("create agent");
        assert_eq!(create_response.status(), StatusCode::OK);
        let create_body = decode_json_body(create_response).await;
        let agent_id = create_body["id"].as_str().expect("agent id").to_string();

        let update_response = app
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/runtime_profile"),
                Some(&token),
                Some(json!({
                    "runtime_model": "gpt-5.4-codex",
                    "thinking_level": "medium"
                })),
            ))
            .await
            .expect("set runtime profile");
        assert_eq!(update_response.status(), StatusCode::OK);

        let row = sqlx::query("SELECT runtime_model, thinking_level FROM agents WHERE id = ?1")
            .bind(&agent_id)
            .fetch_one(&state.db)
            .await
            .expect("load updated agent row");
        assert_eq!(
            row.get::<Option<String>, _>("runtime_model").as_deref(),
            Some("gpt-5.4-codex")
        );
        assert_eq!(
            row.get::<Option<String>, _>("thinking_level").as_deref(),
            Some("medium")
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
                group_id: None,
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
                group_id: None,
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
                codex_acp_default_mode: None,
                runtime_model: None,
                thinking_level: None,
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
    async fn send_input_route_rejects_blank_input_and_identifiers() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state);

        let blank_input = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/agent-123/input",
                Some(&token),
                Some(json!({
                    "input": "   ",
                })),
            ))
            .await
            .expect("send blank input");
        assert_eq!(blank_input.status(), StatusCode::BAD_REQUEST);
        let blank_input_body = decode_json_body(blank_input).await;
        assert_eq!(blank_input_body["error"], Value::from("input is required"));

        let blank_message_id = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/agent-123/input",
                Some(&token),
                Some(json!({
                    "input": "hello",
                    "message_id": "   ",
                })),
            ))
            .await
            .expect("send blank message_id");
        assert_eq!(blank_message_id.status(), StatusCode::BAD_REQUEST);
        let blank_message_id_body = decode_json_body(blank_message_id).await;
        assert_eq!(
            blank_message_id_body["error"],
            Value::from("message_id must not be blank")
        );

        let blank_session_id = app
            .oneshot(build_json_request(
                Method::POST,
                "/agent-123/input",
                Some(&token),
                Some(json!({
                    "input": "hello",
                    "session_id": "   ",
                })),
            ))
            .await
            .expect("send blank session_id");
        assert_eq!(blank_session_id.status(), StatusCode::BAD_REQUEST);
        let blank_session_id_body = decode_json_body(blank_session_id).await;
        assert_eq!(
            blank_session_id_body["error"],
            Value::from("session_id must not be blank")
        );
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
                        "channel": "coordination"
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
            "entrypoint": "coordinator-main",
            "members": [
                {"member_id": "coordinator-main", "role": "coordinator"},
                {
                    "member_id": agent_id,
                    "role": "worker",
                    "description": "TiDB fuzz/bugfix specialist"
                }
            ],
            "steps": [
                {"step_key": "coordinator_plan", "member_id": "coordinator-main", "depends_on": []}
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
        assert_eq!(card["team_member_role"].as_str(), Some("worker"));
        let skill_names: HashSet<&str> = card["skills"]
            .as_array()
            .expect("skills array")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(skill_names.contains("agenthub-actor-runtime"));
        assert!(skill_names.contains("team-agents-index"));
        assert!(skill_names.contains("team-worker-agents-index"));
        assert!(skill_names.contains("team-worker-executor"));
        assert!(skill_names.contains("team-actor-mailbox"));
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
                "coordinator_member_id": "coordinator",
                "entrypoint": "coordinator_plan",
                "members": [
                    {"member_id": "coordinator", "role": "coordinator"},
                    {"member_id": "worker-1", "role": "worker"}
                ],
                "steps": [
                    {"step_key": "coordinator_plan", "member_id": "coordinator", "depends_on": []},
                    {"step_key": worker_step_key, "member_id": "worker-1", "depends_on": ["coordinator_plan"]},
                    {
                        "step_key": "coordinator_synthesize",
                        "member_id": "coordinator",
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
        assert_eq!(members[0]["member_id"], Value::from("coordinator"));

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
            .find(|step| step["step_key"].as_str() == Some("coordinator_synthesize"))
            .expect("coordinator synth step");
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
                "coordinator_member_id": "missing-coordinator",
                "entrypoint": "coordinator_plan",
                "members": [
                    {"member_id": "missing-coordinator", "role": "coordinator"},
                    {"member_id": "worker-1", "role": "worker"}
                ],
                "steps": [
                    {"step_key": "coordinator_plan", "member_id": "missing-coordinator", "depends_on": []},
                    {
                        "step_key": "worker_1_worker_1",
                        "member_id": "worker-1",
                        "depends_on": ["coordinator_plan"]
                    },
                    {
                        "step_key": "coordinator_synthesize",
                        "member_id": "missing-coordinator",
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
                codex_acp_default_mode: None,
                runtime_model: None,
                thinking_level: None,
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
                codex_acp_default_mode: None,
                runtime_model: None,
                thinking_level: None,
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
                codex_acp_default_mode: None,
                runtime_model: None,
                thinking_level: None,
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
                    codex_acp_default_mode: None,
                    runtime_model: None,
                    thinking_level: None,
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
    async fn set_codex_acp_default_mode_route_updates_agent_config_only() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'manual', 'running', ?7, ?8)
            "#,
        )
        .bind("codex-mode-agent")
        .bind("codex-mode-agent")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert codex mode agent");

        let update_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/codex-mode-agent/codex_acp_default_mode",
                Some(&token),
                Some(json!({
                    "mode_id": "yolo"
                })),
            ))
            .await
            .expect("update codex acp default mode");
        assert_eq!(update_resp.status(), StatusCode::OK);
        let update_body = decode_json_body(update_resp).await;
        assert_eq!(update_body["status"], Value::from("ok"));

        let row = sqlx::query("SELECT codex_acp_default_mode, status FROM agents WHERE id = ?1")
            .bind("codex-mode-agent")
            .fetch_one(&state.db)
            .await
            .expect("load codex mode agent row");
        assert_eq!(
            row.get::<Option<String>, _>("codex_acp_default_mode")
                .as_deref(),
            Some("full-access")
        );
        assert_eq!(row.get::<String, _>("status"), "running");

        let invalid_resp = app
            .oneshot(build_json_request(
                Method::POST,
                "/codex-mode-agent/codex_acp_default_mode",
                Some(&token),
                Some(json!({
                    "mode_id": "sandbox"
                })),
            ))
            .await
            .expect("reject invalid codex acp default mode");
        assert_eq!(invalid_resp.status(), StatusCode::BAD_REQUEST);
        let invalid_body = decode_json_body(invalid_resp).await;
        assert_eq!(
            invalid_body["error"],
            Value::from(
                "codex_acp_default_mode must be one of read-only, auto, full-access, or yolo"
            )
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
    async fn list_events_route_clamps_limit_to_twenty() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'manual', 'running', ?7, ?8)
            "#,
        )
        .bind("events-agent")
        .bind("events-agent")
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
        .bind("session-events")
        .bind("events-agent")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert session");

        let event_db = state
            .agents
            .test_event_pool_for_agent("events-agent")
            .await
            .expect("event pool");
        for index in 0..25 {
            sqlx::query(
                r#"
                INSERT INTO agent_events (session_id, seq, ts, stream, message)
                VALUES (?1, ?2, ?3, 'stdout', ?4)
                "#,
            )
            .bind("session-events")
            .bind(format!("seq-{index}"))
            .bind(now + i64::from(index))
            .bind(format!("event-{index}"))
            .execute(&event_db)
            .await
            .expect("insert event");
        }

        let response = app
            .oneshot(build_json_request(
                Method::GET,
                "/events-agent/events?limit=100&session_id=session-events",
                Some(&token),
                None,
            ))
            .await
            .expect("list events");
        assert_eq!(response.status(), StatusCode::OK);
        let body = decode_json_body(response).await;
        let events = body.as_array().expect("events array");
        assert_eq!(events.len(), 20);
        assert_eq!(
            events.first().and_then(|event| event["message"].as_str()),
            Some("event-5")
        );
        assert_eq!(
            events.last().and_then(|event| event["message"].as_str()),
            Some("event-24")
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
