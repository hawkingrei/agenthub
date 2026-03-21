use axum::http::HeaderMap;

use crate::api::error::ApiError;
use crate::auth::UserRecord;
use crate::state::AppState;

pub async fn require_user(headers: &HeaderMap, state: &AppState) -> Result<UserRecord, ApiError> {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = auth
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::unauthorized("missing authorization token"))?;

    let user = state
        .auth
        .validate_session(token)
        .await
        .map_err(|_| ApiError::unauthorized("invalid token"))?;
    Ok(user)
}

pub async fn require_root(headers: &HeaderMap, state: &AppState) -> Result<UserRecord, ApiError> {
    let user = require_user(headers, state).await?;
    if user.role != "root" {
        return Err(ApiError::unauthorized("root required"));
    }
    Ok(user)
}
