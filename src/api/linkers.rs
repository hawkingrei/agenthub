use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};

use crate::api::error::ApiError;
use crate::linkers::{AppLinkerRecord, AppLinkerService, parse_slock_exchange_code};
use crate::state::AppState;

#[derive(Debug, serde::Deserialize)]
struct SlockCallbackQuery {
    code: Option<String>,
    state: Option<String>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/slock/callback", get(slock_callback))
        .with_state(state)
}

async fn slock_callback(
    State(state): State<AppState>,
    Query(query): Query<SlockCallbackQuery>,
) -> Result<Json<AppLinkerRecord>, ApiError> {
    let code = parse_slock_exchange_code(query.code.as_deref(), None, query.state.as_deref())
        .map_err(map_linker_callback_error)?;
    AppLinkerService::new(state.db.clone())
        .exchange_slock_code(None, code)
        .await
        .map(Json)
        .map_err(map_linker_callback_error)
}

fn map_linker_callback_error(err: anyhow::Error) -> ApiError {
    let message = err.to_string();
    if message.contains("token exchange failed") {
        return ApiError::bad_request("Slock token exchange failed");
    }
    if message.contains("userinfo failed") {
        return ApiError::bad_request("Slock userinfo failed");
    }
    if message.contains("required")
        || message.contains("invalid")
        || message.contains("expired")
        || message.contains("mismatch")
        || message.contains("unsupported")
    {
        return ApiError::bad_request(&message);
    }
    err.into()
}
