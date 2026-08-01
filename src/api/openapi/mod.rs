use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    response::{Html, IntoResponse},
    routing::get,
};
use serde_json::Value;

use agenthub_auth_domain::UserCapability;

use crate::api::authz::require_capability;
use crate::api::error::ApiError;
use crate::state::AppState;

mod docs_page;
mod spec;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/openapi.json", get(get_openapi_json))
        .route("/openapi/docs", get(get_openapi_docs))
        .with_state(state)
}

async fn get_openapi_json(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    Ok(Json(spec::openapi_spec()))
}

async fn get_openapi_docs() -> impl IntoResponse {
    Html(docs_page::OPENAPI_DOCS_HTML)
}

#[cfg(test)]
mod tests;
