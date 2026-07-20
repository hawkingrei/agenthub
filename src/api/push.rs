use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};

use agenthub_auth_domain::UserCapability;

use crate::api::authz::{require_capability, require_root};
use crate::api::error::ApiError;
use crate::api::ok_response;
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
    let user = require_capability(&headers, &state, UserCapability::PushSubscribe).await?;
    state.push.save_subscription(&user.id, payload).await?;
    Ok(ok_response())
}

async fn vapid_public(State(state): State<AppState>) -> Result<Json<VapidPublicKey>, ApiError> {
    let public_key = state.push.public_key();
    Ok(Json(VapidPublicKey { public_key }))
}

async fn vapid_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<VapidInfo>, ApiError> {
    let _user = require_root(&headers, &state).await?;
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
    let _user = require_root(&headers, &state).await?;
    let public_key = state.push.rotate_keys()?;
    Ok(Json(VapidRotateResponse { public_key }))
}

#[cfg(test)]
mod tests {
    use agenthub_auth_domain::UserRole;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode, header};
    use serde_json::json;
    use sqlx::Row;
    use tower::util::ServiceExt;
    use uuid::Uuid;

    use super::router;
    use crate::api::teams::tests::build_test_state;
    use crate::state::AppState;

    fn subscription_payload() -> serde_json::Value {
        json!({
            "endpoint": format!("https://push.example.test/{}", Uuid::new_v4()),
            "keys": {
                "p256dh": "test-p256dh",
                "auth": "test-auth"
            }
        })
    }

    async fn create_auth_token_with_role(state: &AppState, role: Option<UserRole>) -> String {
        let user_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        let role_str = role.map(UserRole::as_str).unwrap_or("unknown");
        sqlx::query(
            r#"
            INSERT INTO users (id, username, display_name, role, password_hash, created_at)
            VALUES (?, ?, ?, ?, NULL, ?)
            "#,
        )
        .bind(&user_id)
        .bind(format!("{role_str}-{}", Uuid::new_v4()))
        .bind("Push Test User")
        .bind(role_str)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert user");

        if role == Some(UserRole::Device) {
            sqlx::query(
                r#"
                INSERT INTO devices (id, user_id, name, user_agent, status, created_at)
                VALUES (?, ?, ?, ?, 'active', ?)
                "#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&user_id)
            .bind("Test Device")
            .bind("agenthub-test")
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert active device");
        }

        state
            .auth
            .create_session(&user_id)
            .await
            .expect("create session token")
    }

    fn build_subscribe_request(token: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder()
            .method(Method::POST)
            .uri("/subscribe")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        builder
            .body(Body::from(subscription_payload().to_string()))
            .expect("build subscribe request")
    }

    async fn subscription_count(state: &AppState) -> i64 {
        sqlx::query("SELECT COUNT(*) AS count FROM push_subscriptions")
            .fetch_one(&state.db)
            .await
            .expect("count push subscriptions")
            .get("count")
    }

    async fn ensure_push_subscriptions_table(state: &AppState) {
        sqlx::query(
            r#"
            CREATE TABLE push_subscriptions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id TEXT NOT NULL,
                endpoint TEXT NOT NULL,
                p256dh TEXT NOT NULL,
                auth TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&state.db)
        .await
        .expect("create push_subscriptions table");
    }

    #[tokio::test]
    async fn subscribe_requires_push_subscribe_capability() {
        let state = build_test_state().await;
        ensure_push_subscriptions_table(&state).await;
        let app = router(state.clone());

        let missing_auth = app
            .clone()
            .oneshot(build_subscribe_request(None))
            .await
            .expect("subscribe without auth");
        assert_eq!(missing_auth.status(), StatusCode::UNAUTHORIZED);

        let unknown_role_token = create_auth_token_with_role(&state, None).await;
        let unknown_role = app
            .clone()
            .oneshot(build_subscribe_request(Some(&unknown_role_token)))
            .await
            .expect("subscribe with unknown role");
        assert_eq!(unknown_role.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(subscription_count(&state).await, 0);

        let viewer_token = create_auth_token_with_role(&state, Some(UserRole::Viewer)).await;
        let viewer = app
            .clone()
            .oneshot(build_subscribe_request(Some(&viewer_token)))
            .await
            .expect("subscribe with viewer role");
        assert_eq!(viewer.status(), StatusCode::OK);

        let device_token = create_auth_token_with_role(&state, Some(UserRole::Device)).await;
        let device = app
            .oneshot(build_subscribe_request(Some(&device_token)))
            .await
            .expect("subscribe with device role");
        assert_eq!(device.status(), StatusCode::OK);
        assert_eq!(subscription_count(&state).await, 2);
    }
}
