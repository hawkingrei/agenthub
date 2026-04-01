use argon2::password_hash::PasswordVerifier;
use axum::{Json, Router, extract::State, http::HeaderMap, routing::post};
use chrono::Utc;
use sqlx::Row;

use crate::api::error::ApiError;
use crate::state::AppState;

#[derive(Debug, serde::Deserialize)]
pub struct JoinStartRequest {
    pub token: String,
    pub pin: String,
    pub username: String,
    pub display_name: String,
    pub password: String,
    pub device_name: String,
}

#[derive(Debug, serde::Serialize)]
pub struct JoinStartResponse {
    pub challenge_id: Option<String>,
    pub options: Option<serde_json::Value>,
    pub user_id: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct JoinFinishRequest {
    pub challenge_id: String,
    pub credential: serde_json::Value,
}

#[derive(Debug, serde::Serialize)]
pub struct JoinFinishResponse {
    pub user_id: String,
    pub token: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/start", post(join_start))
        .route("/finish", post(join_finish))
        .with_state(state)
}

async fn join_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<JoinStartRequest>,
) -> Result<Json<JoinStartResponse>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT token, pin_hash, expires_at
        FROM join_challenges
        WHERE token = ?1
        "#,
    )
    .bind(&payload.token)
    .fetch_optional(&state.db)
    .await?;

    let row = row.ok_or_else(|| ApiError::unauthorized("invalid join token"))?;
    let expires_at: i64 = row.get("expires_at");
    if expires_at < Utc::now().timestamp() {
        return Err(ApiError::unauthorized("join token expired"));
    }

    let pin_hash: String = row.get("pin_hash");
    if !verify_pin(&payload.pin, &pin_hash)? {
        return Err(ApiError::unauthorized("invalid pin"));
    }

    sqlx::query("DELETE FROM join_challenges WHERE token = ?1")
        .bind(&payload.token)
        .execute(&state.db)
        .await?;

    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let result = state
        .auth
        .register_start(
            &payload.username,
            &payload.display_name,
            "device",
            Some(&payload.password),
            Some(payload.device_name),
            Some(user_agent),
        )
        .await?;

    match result {
        crate::auth::RegisterStartResult::Challenge {
            challenge_id,
            options,
        } => Ok(Json(JoinStartResponse {
            challenge_id: Some(challenge_id),
            options: Some(serde_json::to_value(options)?),
            user_id: None,
            token: None,
        })),
        crate::auth::RegisterStartResult::Complete { user_id, .. } => {
            let token = state.auth.create_session(&user_id).await?;
            Ok(Json(JoinStartResponse {
                challenge_id: None,
                options: None,
                user_id: Some(user_id),
                token: Some(token),
            }))
        }
    }
}

async fn join_finish(
    State(state): State<AppState>,
    Json(payload): Json<JoinFinishRequest>,
) -> Result<Json<JoinFinishResponse>, ApiError> {
    let credential = serde_json::from_value(payload.credential)?;
    let user_id = state
        .auth
        .register_finish(&payload.challenge_id, credential)
        .await?;
    let token = state.auth.create_session(&user_id).await?;
    Ok(Json(JoinFinishResponse { user_id, token }))
}

fn verify_pin(pin: &str, hash: &str) -> anyhow::Result<bool> {
    let parsed = argon2::password_hash::PasswordHash::new(hash)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let argon2 = argon2::Argon2::default();
    Ok(argon2.verify_password(pin.as_bytes(), &parsed).is_ok())
}
