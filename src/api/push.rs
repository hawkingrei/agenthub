use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};

use crate::api::authz::require_user;
use crate::api::error::ApiError;
use crate::push::PushSubscription;
use crate::state::AppState;

#[derive(Debug, serde::Serialize)]
pub struct VapidPublicKey {
    pub public_key: String,
}

#[derive(Debug, serde::Serialize)]
pub struct VapidInfo {
    pub public_key: String,
    pub subject: String,
    pub keys_path: String,
}

#[derive(Debug, serde::Serialize)]
pub struct VapidRotateResponse {
    pub public_key: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/vapid_info", get(vapid_info))
        .route("/vapid_rotate", post(vapid_rotate))
        .route("/subscribe", post(subscribe))
        .route("/vapid_public", get(vapid_public))
        .with_state(state)
}

async fn subscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PushSubscription>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = require_user(&headers, &state).await?;
    state.push.save_subscription(&user.id, payload).await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn vapid_public(State(state): State<AppState>) -> Result<Json<VapidPublicKey>, ApiError> {
    let public_key = state.push.public_key();
    Ok(Json(VapidPublicKey { public_key }))
}

async fn vapid_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<VapidInfo>, ApiError> {
    let user = require_user(&headers, &state).await?;
    if user.role != "root" {
        return Err(ApiError::unauthorized("root required"));
    }
    Ok(Json(VapidInfo {
        public_key: state.push.public_key(),
        subject: state.push.subject().to_string(),
        keys_path: state.push.keys_path().display().to_string(),
    }))
}

async fn vapid_rotate(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<VapidRotateResponse>, ApiError> {
    let user = require_user(&headers, &state).await?;
    if user.role != "root" {
        return Err(ApiError::unauthorized("root required"));
    }
    let public_key = state.push.rotate_keys()?;
    Ok(Json(VapidRotateResponse { public_key }))
}
