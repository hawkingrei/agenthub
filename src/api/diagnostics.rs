#[cfg(debug_assertions)]
use axum::{
    Json, Router,
    extract::{Query, State},
    http::HeaderMap,
    routing::get,
};
#[cfg(debug_assertions)]
use serde::Deserialize;

#[cfg(debug_assertions)]
use crate::api::{ApiError, authz};
#[cfg(debug_assertions)]
use crate::diagnostics::agent_trace::{
    AgentTraceReport, AgentTraceRequest, apply_live_overlay, collect_from_pool,
};
#[cfg(debug_assertions)]
use crate::state::AppState;

#[cfg(debug_assertions)]
#[derive(Debug, Deserialize)]
struct AgentTraceQuery {
    agent_id: Option<String>,
    team_id: Option<String>,
    member_id: Option<String>,
    session_id: Option<String>,
    event_limit: Option<i64>,
}

#[cfg(debug_assertions)]
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/agent_trace", get(agent_trace))
        .with_state(state)
}

#[cfg(debug_assertions)]
async fn agent_trace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AgentTraceQuery>,
) -> Result<Json<AgentTraceReport>, ApiError> {
    authz::require_root(&headers, &state).await?;
    let request = AgentTraceRequest {
        agent_id: query.agent_id,
        team_id: query.team_id,
        member_id: query.member_id,
        session_id: query.session_id,
        event_limit: query.event_limit.unwrap_or(16),
    };
    let mut report = collect_from_pool(
        &state.db,
        state.agents.event_db_base_dir().to_path_buf(),
        request,
    )
    .await
    .map_err(ApiError::from)?;
    let overlay = state
        .agents
        .collect_agent_trace_live_overlay(&report.target.agent_id)
        .await;
    apply_live_overlay(&mut report, overlay);
    Ok(Json(report))
}
