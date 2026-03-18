use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};

use crate::agent::{AgentNodeConfig, AgentNodeRecord};
use crate::api::authz::require_user;
use crate::api::error::ApiError;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", post(create_agent_node).get(list_agent_nodes))
        .route("/:id", get(get_agent_node).delete(delete_agent_node))
        .with_state(state)
}

fn map_agent_node_error(err: anyhow::Error, missing_msg: &str) -> ApiError {
    let message = err.to_string();
    if message.contains("no rows returned by a query") || message.contains("not found") {
        return ApiError::not_found(missing_msg);
    }
    if message.contains("UNIQUE constraint failed") {
        return ApiError::conflict("agent node id or name already exists");
    }
    if message.contains("required")
        || message.contains("must ")
        || message.contains("invalid ")
        || message.contains("reserved")
    {
        return ApiError::bad_request(&message);
    }
    ApiError::from(err)
}

async fn create_agent_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AgentNodeConfig>,
) -> Result<Json<AgentNodeRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let node = state
        .agents
        .create_agent_node(payload)
        .await
        .map_err(|err| map_agent_node_error(err, "agent node not found"))?;
    Ok(Json(node))
}

async fn list_agent_nodes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentNodeRecord>>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let nodes = state.agents.list_agent_nodes().await?;
    Ok(Json(nodes))
}

async fn get_agent_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<String>,
) -> Result<Json<AgentNodeRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let node = state
        .agents
        .get_agent_node(&node_id)
        .await
        .map_err(|err| map_agent_node_error(err, "agent node not found"))?;
    Ok(Json(node))
}

async fn delete_agent_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state
        .agents
        .delete_agent_node(&node_id)
        .await
        .map_err(|err| map_agent_node_error(err, "agent node not found"))?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}
