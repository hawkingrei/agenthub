use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};

use agenthub_auth_domain::UserCapability;

use crate::api::authz::require_capability;
use crate::api::error::ApiError;
use crate::state::AppState;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
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
    let _user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    Ok(Json(RuntimeDefaultsResponse {
        default_worktree_root: state.default_worktree_root.clone(),
    }))
}

#[cfg(test)]
mod tests {
    use agenthub_auth_domain::UserRole;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode, header};
    use tower::util::ServiceExt;
    use uuid::Uuid;

    use crate::api::teams::tests::{build_test_state, create_auth_token};
    use crate::state::AppState;

    use super::RuntimeDefaultsResponse;

    fn build_request(token: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder().method(Method::GET).uri("/defaults");
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        builder.body(Body::empty()).expect("build request")
    }

    async fn create_auth_token_with_role(state: &AppState, role: UserRole) -> String {
        let user_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO users (id, username, display_name, role, password_hash, created_at)
            VALUES (?, ?, ?, ?, NULL, ?)
            "#,
        )
        .bind(&user_id)
        .bind(format!("settings-{}-{}", role.as_str(), Uuid::new_v4()))
        .bind("Settings Test User")
        .bind(role.as_str())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert user");

        if role == UserRole::Device {
            sqlx::query(
                r#"
                INSERT INTO devices (id, user_id, name, user_agent, status, created_at)
                VALUES (?, ?, ?, ?, 'active', ?)
                "#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&user_id)
            .bind("Settings Test Device")
            .bind("settings-test")
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert device");
        }

        state
            .auth
            .create_session(&user_id)
            .await
            .expect("create session")
    }

    async fn decode_json_body<T: serde::de::DeserializeOwned>(
        response: axum::response::Response,
    ) -> T {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        serde_json::from_slice(&body).expect("decode response body")
    }

    #[tokio::test]
    async fn runtime_defaults_requires_authentication() {
        let state = build_test_state().await;
        let app = super::router(state);
        let response = app
            .oneshot(build_request(None))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn runtime_defaults_requires_runtime_inspect_capability() {
        let state = build_test_state().await;
        let device_token = create_auth_token_with_role(&state, UserRole::Device).await;
        let app = super::router(state);

        let response = app
            .oneshot(build_request(Some(&device_token)))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body: serde_json::Value = decode_json_body(response).await;
        assert_eq!(body["error"], "runtime:inspect required");
    }

    #[tokio::test]
    async fn runtime_defaults_allows_viewer_runtime_inspect() {
        let state = build_test_state().await;
        let expected_root = state.default_worktree_root.clone();
        let token = create_auth_token_with_role(&state, UserRole::Viewer).await;
        let app = super::router(state);

        let response = app
            .oneshot(build_request(Some(&token)))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::OK);

        let payload: RuntimeDefaultsResponse = decode_json_body(response).await;
        assert_eq!(payload.default_worktree_root, expected_root);
    }

    #[tokio::test]
    async fn runtime_defaults_returns_configured_worktree_root() {
        let state = build_test_state().await;
        let expected_root = state.default_worktree_root.clone();
        let token = create_auth_token(&state).await;
        let app = super::router(state);

        let response = app
            .oneshot(build_request(Some(&token)))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::OK);

        let payload: RuntimeDefaultsResponse = decode_json_body(response).await;
        assert_eq!(payload.default_worktree_root, expected_root);
    }
}
