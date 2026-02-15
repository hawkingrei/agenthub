use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};

use crate::api::authz::require_user;
use crate::api::error::ApiError;
use crate::state::AppState;

#[derive(Debug, serde::Serialize)]
pub struct RuntimeDefaultsResponse {
    pub default_worktree_root: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/defaults", get(runtime_defaults))
        .with_state(state)
}

async fn runtime_defaults(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RuntimeDefaultsResponse>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    Ok(Json(RuntimeDefaultsResponse {
        default_worktree_root: state.default_worktree_root.clone(),
    }))
}
