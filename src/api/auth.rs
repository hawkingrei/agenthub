use axum::{Json, Router, extract::State, http::HeaderMap, routing::post};
use serde::{Deserialize, Serialize};

use crate::api::error::ApiError;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/status", axum::routing::get(status))
        .route("/register/start", post(register_start))
        .route("/register/finish", post(register_finish))
        .route("/login/start", post(login_start))
        .route("/login/finish", post(login_finish))
        .route("/login/register_finish", post(login_register_finish))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
pub struct RegisterStartRequest {
    pub username: String,
    pub display_name: String,
    pub role: Option<String>,
    pub password: Option<String>,
    pub device_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegisterStartResponse {
    pub challenge_id: Option<String>,
    pub options: Option<serde_json::Value>,
    pub user_id: Option<String>,
    pub token: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterFinishRequest {
    pub challenge_id: String,
    pub credential: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct LoginStartRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginStartResponse {
    pub challenge_id: Option<String>,
    pub options: Option<serde_json::Value>,
    pub registration_options: Option<serde_json::Value>,
    pub user_id: Option<String>,
    pub token: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginFinishRequest {
    pub challenge_id: String,
    pub credential: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct AuthFinishResponse {
    pub user_id: String,
    pub token: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct AuthStatusResponse {
    pub root_initialized: bool,
    pub passkey_enabled: bool,
}

async fn status(State(state): State<AppState>) -> Result<Json<AuthStatusResponse>, ApiError> {
    let root_initialized = state.auth.root_exists().await?;
    let passkey_enabled = state.auth.is_passkey_enabled().await?;
    Ok(Json(AuthStatusResponse {
        root_initialized,
        passkey_enabled,
    }))
}

async fn register_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RegisterStartRequest>,
) -> Result<Json<RegisterStartResponse>, ApiError> {
    let role = payload.role.as_deref().unwrap_or("device");
    let ua = extract_ua(&headers);
    if role == "root" && state.auth.root_exists().await? {
        return Err(ApiError::unauthorized("root already initialized"));
    }
    if role == "root" && payload.password.is_none() {
        return Err(ApiError::unauthorized("root requires password"));
    }
    let result = state
        .auth
        .register_start(
            &payload.username,
            &payload.display_name,
            role,
            payload.password.as_deref(),
            payload.device_name,
            ua,
        )
        .await?;

    match result {
        crate::auth::RegisterStartResult::Challenge {
            challenge_id,
            options,
        } => Ok(Json(RegisterStartResponse {
            challenge_id: Some(challenge_id),
            options: Some(serde_json::to_value(options)?),
            user_id: None,
            token: None,
            role: None,
        })),
        crate::auth::RegisterStartResult::Complete { user_id, role } => {
            let token = state.auth.create_session(&user_id).await?;
            Ok(Json(RegisterStartResponse {
                challenge_id: None,
                options: None,
                user_id: Some(user_id),
                token: Some(token),
                role: Some(role),
            }))
        }
    }
}

async fn register_finish(
    State(state): State<AppState>,
    Json(payload): Json<RegisterFinishRequest>,
) -> Result<Json<AuthFinishResponse>, ApiError> {
    if payload.challenge_id.trim().is_empty() {
        return Err(ApiError::bad_request("challenge_id cannot be empty"));
    }
    let credential = serde_json::from_value(payload.credential)?;
    let user_id = state
        .auth
        .register_finish(&payload.challenge_id, credential)
        .await?;
    let token = state.auth.create_session(&user_id).await?;
    let user = state.auth.get_user_by_id(&user_id).await?;
    Ok(Json(AuthFinishResponse {
        user_id,
        token,
        role: user.role,
    }))
}

async fn login_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginStartRequest>,
) -> Result<Json<LoginStartResponse>, ApiError> {
    let ip = extract_ip(&headers);
    let ua = extract_ua(&headers);
    let result = state
        .auth
        .login_start(&payload.username, &payload.password)
        .await;
    let auth_result = match result {
        Ok(value) => value,
        Err(err) => {
            let detail = format!("user={}, error={}", payload.username, err);
            let _ = state
                .auth
                .record_audit(
                    None,
                    None,
                    "login_start_failed",
                    Some(&detail),
                    ip.as_deref(),
                    ua.as_deref(),
                )
                .await;
            return Err(err.into());
        }
    };

    match auth_result {
        crate::auth::LoginStartResult::Challenge {
            challenge_id,
            options,
        } => Ok(Json(LoginStartResponse {
            challenge_id: Some(challenge_id),
            options: Some(serde_json::to_value(options)?),
            registration_options: None,
            user_id: None,
            token: None,
            role: None,
        })),
        crate::auth::LoginStartResult::Registration {
            challenge_id,
            options,
            role,
        } => Ok(Json(LoginStartResponse {
            challenge_id: Some(challenge_id),
            options: None,
            registration_options: Some(serde_json::to_value(options)?),
            user_id: None,
            token: None,
            role: Some(role),
        })),
        crate::auth::LoginStartResult::Complete { user_id, role } => {
            let token = state.auth.create_session(&user_id).await?;
            if role == "device" {
                let _ = state.auth.touch_device_login(&user_id).await;
            }
            let device_id = if role == "device" {
                state
                    .auth
                    .get_device_id_for_user(&user_id)
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            };
            let _ = state
                .auth
                .record_audit(
                    Some(&user_id),
                    device_id.as_deref(),
                    "login_success",
                    None,
                    ip.as_deref(),
                    ua.as_deref(),
                )
                .await;

            Ok(Json(LoginStartResponse {
                challenge_id: None,
                options: None,
                registration_options: None,
                user_id: Some(user_id),
                token: Some(token),
                role: Some(role),
            }))
        }
    }
}

async fn login_finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginFinishRequest>,
) -> Result<Json<AuthFinishResponse>, ApiError> {
    if payload.challenge_id.trim().is_empty() {
        return Err(ApiError::bad_request("challenge_id cannot be empty"));
    }
    let ip = extract_ip(&headers);
    let ua = extract_ua(&headers);
    let credential = serde_json::from_value(payload.credential)?;
    let result = state
        .auth
        .login_finish(&payload.challenge_id, credential)
        .await;

    let user_id = match result {
        Ok(uid) => uid,
        Err(err) => {
            let detail = format!("challenge={}, error={}", payload.challenge_id, err);
            let _ = state
                .auth
                .record_audit(
                    None,
                    None,
                    "login_finish_failed",
                    Some(&detail),
                    ip.as_deref(),
                    ua.as_deref(),
                )
                .await;
            return Err(err.into());
        }
    };

    let token = state.auth.create_session(&user_id).await?;
    let user = state.auth.get_user_by_id(&user_id).await?;

    if user.role == "device" {
        let _ = state.auth.touch_device_login(&user_id).await;
    }
    let device_id = if user.role == "device" {
        state
            .auth
            .get_device_id_for_user(&user_id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };
    let _ = state
        .auth
        .record_audit(
            Some(&user_id),
            device_id.as_deref(),
            "login_success",
            None,
            ip.as_deref(),
            ua.as_deref(),
        )
        .await;

    Ok(Json(AuthFinishResponse {
        user_id,
        token,
        role: user.role,
    }))
}

async fn login_register_finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginFinishRequest>,
) -> Result<Json<AuthFinishResponse>, ApiError> {
    if payload.challenge_id.trim().is_empty() {
        return Err(ApiError::bad_request("challenge_id cannot be empty"));
    }
    let ip = extract_ip(&headers);
    let ua = extract_ua(&headers);
    let credential = serde_json::from_value(payload.credential)?;
    let result = state
        .auth
        .register_finish(&payload.challenge_id, credential)
        .await;

    let user_id = match result {
        Ok(uid) => uid,
        Err(err) => {
            let detail = format!("challenge={}, error={}", payload.challenge_id, err);
            let _ = state
                .auth
                .record_audit(
                    None,
                    None,
                    "login_register_finish_failed",
                    Some(&detail),
                    ip.as_deref(),
                    ua.as_deref(),
                )
                .await;
            return Err(err.into());
        }
    };

    let token = state.auth.create_session(&user_id).await?;
    let user = state.auth.get_user_by_id(&user_id).await?;

    if user.role == "device" {
        let _ = state.auth.touch_device_login(&user_id).await;
    }
    let device_id = if user.role == "device" {
        state
            .auth
            .get_device_id_for_user(&user_id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };
    let _ = state
        .auth
        .record_audit(
            Some(&user_id),
            device_id.as_deref(),
            "login_success",
            Some("passkey_registered_during_login"),
            ip.as_deref(),
            ua.as_deref(),
        )
        .await;

    Ok(Json(AuthFinishResponse {
        user_id,
        token,
        role: user.role,
    }))
}

fn extract_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
}

fn extract_ua(headers: &HeaderMap) -> Option<String> {
    headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}
