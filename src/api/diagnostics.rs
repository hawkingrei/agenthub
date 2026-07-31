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
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode, header};
    use sqlx::Row;
    use tower::util::ServiceExt;
    use uuid::Uuid;

    use crate::api::teams::tests::build_test_state;
    use crate::state::AppState;

    async fn create_auth_token_with_role(state: &AppState, role: &str) -> String {
        let user_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO users (id, username, display_name, role, password_hash, created_at)
            VALUES (?1, ?2, ?3, ?4, NULL, ?5)
            "#,
        )
        .bind(&user_id)
        .bind(format!("{role}-{}", Uuid::new_v4()))
        .bind("Diagnostics Test User")
        .bind(role)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert user");

        state
            .auth
            .create_session(&user_id)
            .await
            .expect("create session token")
    }

    fn build_agent_trace_request(token: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder()
            .method(Method::GET)
            .uri("/agent_trace?agent_id=missing-agent");
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        builder.body(Body::empty()).expect("build request")
    }

    #[tokio::test]
    async fn agent_trace_requires_diagnostics_read_capability() {
        let state = build_test_state().await;
        let app = super::router(state.clone());

        let missing_auth = app
            .clone()
            .oneshot(build_agent_trace_request(None))
            .await
            .expect("agent trace without auth");
        assert_eq!(missing_auth.status(), StatusCode::UNAUTHORIZED);

        let operator_token = create_auth_token_with_role(&state, "operator").await;
        let operator = app
            .clone()
            .oneshot(build_agent_trace_request(Some(&operator_token)))
            .await
            .expect("agent trace with operator role");
        assert_eq!(operator.status(), StatusCode::UNAUTHORIZED);

        let admin_token = create_auth_token_with_role(&state, "admin").await;
        let admin = app
            .oneshot(build_agent_trace_request(Some(&admin_token)))
            .await
            .expect("agent trace with admin role");
        let admin_status = admin.status();
        let body = axum::body::to_bytes(admin.into_body(), usize::MAX)
            .await
            .expect("read admin response");
        assert_eq!(
            admin_status,
            StatusCode::OK,
            "unexpected admin response: {}",
            String::from_utf8_lossy(&body)
        );
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("decode admin response");
        assert_eq!(payload["target"]["agent_id"], "missing-agent");
        assert_eq!(
            payload["verdict"]["layer"],
            serde_json::Value::from("target_not_found")
        );

        let count: i64 = sqlx::query("SELECT COUNT(*) AS count FROM auth_sessions")
            .fetch_one(&state.db)
            .await
            .expect("count auth sessions")
            .get("count");
        assert_eq!(count, 2);
    }
}
