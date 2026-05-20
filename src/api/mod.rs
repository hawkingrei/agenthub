use axum::Router;

use crate::state::AppState;
use axum::http::HeaderMap;

mod admin;
mod agent_nodes;
mod agents;
mod auth;
mod authz;
#[cfg(debug_assertions)]
mod diagnostics;
mod error;
mod join;
mod linkers;
mod openapi;
mod push;
mod settings;
mod teams;

pub(crate) use self::error::ApiError;
pub(crate) use self::error::ok_response;
pub(crate) use self::teams::load_team_for_user;
#[cfg(test)]
pub(crate) use self::teams::tests as team_tests;

pub(crate) fn extract_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
}

pub(crate) fn extract_ua(headers: &HeaderMap) -> Option<String> {
    headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

pub fn router(state: AppState) -> Router {
    let router = Router::new()
        .nest("/agents", agents::router(state.clone()))
        .nest("/agent_nodes", agent_nodes::router(state.clone()))
        .nest("/teams", teams::router(state.clone()))
        .nest("/admin", admin::router(state.clone()))
        .nest("/auth", auth::router(state.clone()))
        .nest("/join", join::router(state.clone()))
        .nest("/linkers", linkers::router(state.clone()))
        .nest("/settings", settings::router(state.clone()))
        .merge(openapi::router(state.clone()))
        .nest("/push", push::router(state.clone()));
    #[cfg(debug_assertions)]
    let router = router.nest("/diagnostics", diagnostics::router(state));
    router
}

pub async fn health() -> &'static str {
    "ok"
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn health_returns_ok() {
        assert_eq!(super::health().await, "ok");
    }
}
