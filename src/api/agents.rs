use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{delete, get, post},
};
use serde::Deserialize;

use crate::acp::AcpPermissionRecord;
use crate::agent::{AgentConfig, AgentRecord, WorktreeMode};
use crate::api::authz::require_user;
use crate::api::error::ApiError;
use crate::state::AppState;

#[derive(Debug, serde::Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub workdir: String,
    pub command: String,
    pub args: Vec<String>,
    pub worktree_mode: Option<String>,
    pub worktree_repo: Option<String>,
    pub worktree_ref: Option<String>,
    pub code_mode: Option<bool>,
}

#[derive(Debug, serde::Serialize)]
pub struct StartAgentResponse {
    pub session_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ListEventsQuery {
    pub limit: Option<i64>,
    pub session_id: Option<String>,
    pub before_seq: Option<i64>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SetCodeModeRequest {
    pub code_mode: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct SendInputRequest {
    pub input: String,
    pub message_id: Option<String>,
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
        .route("/:id", get(get_agent))
        .route("/:id/start", post(start_agent))
        .route("/:id/stop", post(stop_agent))
        .route("/:id/input", post(send_input))
        .route("/:id", delete(delete_agent))
        .route("/:id/events", get(list_events))
        .route("/:id/code_mode", post(set_code_mode))
        .route("/:id/acp/session/clear", post(clear_acp_session))
        .route("/:id/acp/mode", post(set_acp_mode))
        .route("/:id/acp/model", post(set_acp_model))
        .route("/:id/acp/config", post(set_acp_config))
        .route("/:id/acp/cancel", post(cancel_acp))
        .route("/:id/permissions", get(list_permissions))
        .route(
            "/:id/permissions/:permission_id/respond",
            post(respond_permission),
        )
        .with_state(state)
}

async fn create_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAgentRequest>,
) -> Result<Json<AgentRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let config = AgentConfig {
        name: payload.name,
        workdir: payload.workdir,
        command: payload.command,
        args: payload.args,
        worktree_mode: parse_worktree_mode(payload.worktree_mode.as_deref()),
        worktree_repo: payload.worktree_repo,
        worktree_ref: payload.worktree_ref,
        code_mode: payload.code_mode.unwrap_or(true),
    };
    let agent = state.agents.create_agent(config).await?;
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

async fn start_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Json<StartAgentResponse>, ApiError> {
    let _user = require_user(&headers, &state).await?;
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
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn send_input(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SendInputRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state
        .agents
        .send_input(&agent_id, &payload.input, payload.message_id.as_deref())
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn list_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Query(query): Query<ListEventsQuery>,
) -> Result<Json<Vec<crate::agent::AgentEvent>>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let limit = query.limit.unwrap_or(500);
    let before_seq = query.before_seq;
    let events = if let Some(session_id) = query.session_id.as_deref() {
        state
            .agents
            .list_events_for_session(&agent_id, session_id, limit, before_seq)
            .await?
    } else {
        state
            .agents
            .list_events(&agent_id, limit, before_seq)
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

async fn clear_acp_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<ClearSessionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let provider = payload.provider.as_deref().unwrap_or("codex");
    state
        .agents
        .clear_persistent_session(&agent_id, provider)
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

async fn respond_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((agent_id, permission_id)): Path<(String, String)>,
    Json(payload): Json<PermissionResponseRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let _ = agent_id;
    let outcome = if let Some(option_id) = payload.option_id.clone() {
        agent_client_protocol::RequestPermissionOutcome::Selected(
            agent_client_protocol::SelectedPermissionOutcome::new(option_id.clone()),
        )
    } else if payload.outcome.as_deref() == Some("cancelled") {
        agent_client_protocol::RequestPermissionOutcome::Cancelled
    } else {
        agent_client_protocol::RequestPermissionOutcome::Cancelled
    };
    state
        .acp_permissions
        .respond(&permission_id, outcome, payload.option_id)
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

fn parse_worktree_mode(value: Option<&str>) -> WorktreeMode {
    match value {
        Some("create_worktree") => WorktreeMode::CreateWorktree,
        Some("reuse_worktree") => WorktreeMode::ReuseWorktree,
        _ => WorktreeMode::UseExisting,
    }
}

#[cfg(test)]
mod tests {
    use super::{WorktreeMode, parse_worktree_mode};

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
}
