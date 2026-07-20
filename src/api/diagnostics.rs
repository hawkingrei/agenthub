#[cfg(debug_assertions)]
use agenthub_auth_domain::UserCapability;
#[cfg(debug_assertions)]
use axum::{
    Json, Router,
    extract::{Query, State},
    http::HeaderMap,
    routing::get,
};
#[cfg(debug_assertions)]
use serde::Deserialize;

#[cfg(debug_assertions)]
use crate::api::{ApiError, authz};
#[cfg(debug_assertions)]
use crate::diagnostics::agent_trace::{
    AgentTraceReport, AgentTraceRequest, apply_live_overlay, collect_from_pool,
};
#[cfg(debug_assertions)]
use crate::state::AppState;

#[cfg(debug_assertions)]
#[derive(Debug, Deserialize)]
struct AgentTraceQuery {
    agent_id: Option<String>,
    team_id: Option<String>,
    member_id: Option<String>,
    session_id: Option<String>,
    event_limit: Option<i64>,
}

#[cfg(debug_assertions)]
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/agent_trace", get(agent_trace))
        .with_state(state)
}

#[cfg(debug_assertions)]
async fn agent_trace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AgentTraceQuery>,
) -> Result<Json<AgentTraceReport>, ApiError> {
    authz::require_capability(&headers, &state, UserCapability::DiagnosticsRead).await?;
    let request = AgentTraceRequest {
        agent_id: query.agent_id,
        team_id: query.team_id,
        member_id: query.member_id,
        session_id: query.session_id,
        event_limit: query.event_limit.unwrap_or(16),
    };
    let mut report = collect_from_pool(
        &state.db,
        state.agents.event_db_base_dir().to_path_buf(),
        request,
    )
    .await
    .map_err(ApiError::from)?;
    let overlay = state
        .agents
        .collect_agent_trace_live_overlay(&report.target.agent_id)
        .await;
    apply_live_overlay(&mut report, overlay);
    Ok(Json(report))
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use agenthub_auth_domain::UserRole;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode, header};
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::router;
    use crate::api::team_tests::build_test_state;
    use crate::state::AppState;

    async fn create_role_token(state: &AppState, role: UserRole) -> String {
        let user_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        let role_str = role.as_str();
        sqlx::query(
            r#"
            INSERT INTO users (id, username, display_name, role, password_hash, created_at)
            VALUES (?, ?, ?, ?, NULL, ?)
            "#,
        )
        .bind(&user_id)
        .bind(format!("{role_str}-{}", Uuid::new_v4()))
        .bind(role_str)
        .bind(role_str)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert role user");

        state
            .auth
            .create_session(&user_id)
            .await
            .expect("create role session")
    }

    fn build_request(path: &str, token: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder().method(Method::GET).uri(path);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        builder.body(Body::empty()).expect("build request")
    }

    async fn decode_json_body(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        serde_json::from_slice(&bytes).expect("decode response json")
    }

    #[tokio::test]
    async fn agent_trace_requires_diagnostics_read_capability() {
        let state = build_test_state().await;
        let viewer_token = create_role_token(&state, UserRole::Viewer).await;
        let admin_token = create_role_token(&state, UserRole::Admin).await;
        let app = router(state);

        let denied = app
            .clone()
            .oneshot(build_request(
                "/agent_trace?agent_id=missing",
                Some(&viewer_token),
            ))
            .await
            .expect("run viewer agent trace request");
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
        let denied_body = decode_json_body(denied).await;
        assert_eq!(denied_body["error"], json!("diagnostics:read required"));

        let allowed = app
            .oneshot(build_request(
                "/agent_trace?agent_id=missing",
                Some(&admin_token),
            ))
            .await
            .expect("run admin agent trace request");
        assert_ne!(allowed.status(), StatusCode::UNAUTHORIZED);
    }
}
