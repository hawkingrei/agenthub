use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};

use crate::api::authz::require_user;
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
    let _user = require_user(&headers, &state).await?;
    Ok(Json(RuntimeDefaultsResponse {
        default_worktree_root: state.default_worktree_root.clone(),
    }))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode, header};
    use tower::util::ServiceExt;

    use crate::api::teams::tests::{build_test_state, create_auth_token};

    use super::RuntimeDefaultsResponse;

    fn build_request(token: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder().method(Method::GET).uri("/defaults");
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        builder.body(Body::empty()).expect("build request")
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

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let payload: RuntimeDefaultsResponse =
            serde_json::from_slice(&body).expect("decode response body");
        assert_eq!(payload.default_worktree_root, expected_root);
    }
}
