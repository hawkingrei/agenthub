use axum::Router;

use crate::state::AppState;

mod admin;
mod agents;
mod auth;
mod authz;
mod error;
mod join;
mod openapi;
mod push;
mod settings;
mod teams;

pub(crate) use self::error::ApiError;
pub(crate) use self::teams::load_team_for_user;
#[cfg(test)]
pub(crate) use self::teams::tests as team_tests;

pub fn router(state: AppState) -> Router {
    Router::new()
        .nest("/agents", agents::router(state.clone()))
        .nest("/teams", teams::router(state.clone()))
        .nest("/admin", admin::router(state.clone()))
        .nest("/auth", auth::router(state.clone()))
        .nest("/join", join::router(state.clone()))
        .nest("/settings", settings::router(state.clone()))
        .merge(openapi::router(state.clone()))
        .nest("/push", push::router(state))
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
