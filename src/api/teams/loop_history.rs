use agenthub_agent_domain::loop_history::{
    LoopEventHistoryPage, LoopHistoryPage, LoopSourceHistoryPage, LoopToolHistoryPage,
};
use agenthub_agent_domain::loop_runtime::{LoopActivation, validate_loop_id};
use agenthub_db::loop_runtime::LoopStore;

use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HistoryQuery {
    before_activation_id: Option<String>,
    limit: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourcesQuery {
    after_source_id: Option<String>,
    limit: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EventsQuery {
    after_event_id: Option<i64>,
    limit: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ToolsQuery {
    after_tool_id: Option<i64>,
    limit: Option<u32>,
}

async fn authorize_history(
    state: &AppState,
    headers: &HeaderMap,
    team_id: &str,
    actor_id: &str,
) -> Result<LoopStore, ApiError> {
    let user = require_capability(headers, state, UserCapability::RuntimeInspect).await?;
    load_team_for_user(state, team_id, &user).await?;
    validate_loop_id(actor_id).map_err(|_| ApiError::bad_request("invalid actor reference"))?;
    // Membership changes must not erase a Team's authority to inspect its own historical work.
    Ok(LoopStore::new(state.db.clone()))
}

pub(super) async fn list_activations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id)): Path<(String, String)>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<LoopHistoryPage>, ApiError> {
    let store = authorize_history(&state, &headers, &team_id, &actor_id).await?;
    let page = store
        .activation_history(
            &team_id,
            &actor_id,
            query.before_activation_id.as_deref(),
            query.limit.unwrap_or(25),
        )
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(page))
}

pub(super) async fn get_activation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id, id)): Path<(String, String, String)>,
) -> Result<Json<LoopActivation>, ApiError> {
    let store = authorize_history(&state, &headers, &team_id, &actor_id).await?;
    validate_loop_id(&id).map_err(|_| ApiError::bad_request("invalid activation reference"))?;
    let activation = store
        .activation(&team_id, &id)
        .await
        .map_err(map_team_internal_error)?
        .filter(|activation| activation.actor_id == actor_id)
        .ok_or_else(|| ApiError::not_found("activation not found"))?;
    Ok(Json(activation))
}

pub(super) async fn list_sources(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id, id)): Path<(String, String, String)>,
    Query(query): Query<SourcesQuery>,
) -> Result<Json<LoopSourceHistoryPage>, ApiError> {
    let store = authorize_history(&state, &headers, &team_id, &actor_id).await?;
    let page = store
        .activation_source_history(
            &team_id,
            &actor_id,
            &id,
            query.after_source_id.as_deref(),
            query.limit.unwrap_or(25),
        )
        .await
        .map_err(map_team_internal_error)?
        .ok_or_else(|| ApiError::not_found("activation not found"))?;
    Ok(Json(page))
}

pub(super) async fn list_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id, id)): Path<(String, String, String)>,
    Query(query): Query<EventsQuery>,
) -> Result<Json<LoopEventHistoryPage>, ApiError> {
    let store = authorize_history(&state, &headers, &team_id, &actor_id).await?;
    let page = store
        .activation_event_history(
            &team_id,
            &actor_id,
            &id,
            query.after_event_id,
            query.limit.unwrap_or(25),
        )
        .await
        .map_err(map_team_internal_error)?
        .ok_or_else(|| ApiError::not_found("activation not found"))?;
    Ok(Json(page))
}

pub(super) async fn list_tools(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id, id)): Path<(String, String, String)>,
    Query(query): Query<ToolsQuery>,
) -> Result<Json<LoopToolHistoryPage>, ApiError> {
    let store = authorize_history(&state, &headers, &team_id, &actor_id).await?;
    let page = store
        .activation_tool_history(
            &team_id,
            &actor_id,
            &id,
            query.after_tool_id,
            query.limit.unwrap_or(25),
        )
        .await
        .map_err(map_team_internal_error)?
        .ok_or_else(|| ApiError::not_found("activation not found"))?;
    Ok(Json(page))
}
