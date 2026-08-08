use agenthub_auth_domain::UserRole;
use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use serde_json::Value;
use tower::util::ServiceExt;
use uuid::Uuid;

use crate::api::teams::tests::build_test_state;

#[tokio::test]
async fn openapi_json_requires_authorization() {
    let state = build_test_state().await;
    let app = super::router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/openapi.json")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("request openapi without auth");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn openapi_json_requires_runtime_inspect_capability() {
    let state = build_test_state().await;
    let token = create_role_auth_token(&state, UserRole::Device).await;
    let app = super::router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/openapi.json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .expect("build device request"),
        )
        .await
        .expect("request openapi with device auth");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let value: Value = serde_json::from_slice(&bytes).expect("decode openapi error");
    assert_eq!(value["error"], Value::from("runtime:inspect required"));
}

#[tokio::test]
async fn openapi_json_contains_team_runs_list_path() {
    let state = build_test_state().await;
    let token = crate::api::teams::tests::create_auth_token(&state).await;
    let app = super::router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/openapi.json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .expect("build authorized request"),
        )
        .await
        .expect("request openapi with auth");
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let value: Value = serde_json::from_slice(&bytes).expect("decode openapi json");
    assert_eq!(value["openapi"], Value::from("3.0.3"));
    assert!(value["paths"]["/api/agents/{id}/uploads"]["post"].is_object());
    assert!(value["paths"]["/api/agents/{id}/uploads/downloads"]["post"].is_object());
    assert!(value["paths"]["/api/agents/{id}/images"]["post"].is_object());
    assert!(value["paths"]["/api/teams/prompt_defaults"]["get"].is_object());
    assert!(value["paths"]["/api/teams/{id}"]["delete"].is_object());
    assert!(value["paths"]["/api/teams/{id}/channels"]["get"].is_object());
    assert!(value["paths"]["/api/teams/{id}/channels"]["post"].is_object());
    assert!(value["paths"]["/api/teams/{id}/channels/{channel_id}"]["delete"].is_object());
    assert!(value["paths"]["/api/teams/{id}/uploads"]["post"].is_object());
    assert!(value["paths"]["/api/teams/{id}/uploads/downloads"]["post"].is_object());
    assert!(value["paths"]["/api/teams/{id}/images"]["post"].is_object());
    assert!(value["paths"]["/api/teams/{id}/tasks/{task_id}/uploads"]["post"].is_object());
    assert!(
        value["paths"]["/api/teams/{id}/tasks/{task_id}/uploads/downloads"]["post"].is_object()
    );
    assert!(value["paths"]["/api/teams/{id}/tasks/{task_id}/images"]["post"].is_object());
    assert!(value["paths"]["/api/teams/{id}/tasks/{task_id}/forks"]["get"].is_object());
    assert!(value["paths"]["/api/teams/{id}/tasks/{task_id}/forks"]["post"].is_object());
    assert!(
        value["paths"]["/api/teams/{id}/tasks/{task_id}/forks/{fork_id}/complete"]["post"]
            .is_object()
    );
    assert!(value["paths"]["/api/teams/{id}/runs"].is_object());
    assert!(value["paths"]["/api/teams/runs/{run_id}/resume"].is_object());
    assert!(value["paths"]["/api/teams/runs/{run_id}/restart"].is_object());
    assert!(value["paths"]["/api/teams/runs/{run_id}/snapshot"].is_object());
    assert!(value["components"]["schemas"]["TeamChannelRecord"].is_object());
    assert!(value["components"]["schemas"]["CreateTeamChannelRequest"].is_object());
    assert!(value["components"]["schemas"]["TeamGoalForkRecord"].is_object());
    assert!(value["components"]["schemas"]["CreateGoalForkRequest"].is_object());
    assert!(value["components"]["schemas"]["CompleteGoalForkRequest"].is_object());
    assert!(value["components"]["schemas"]["TeamUploadRequest"].is_object());
    assert!(value["components"]["schemas"]["ObjectDownloadRequest"].is_object());
    assert!(value["components"]["schemas"]["ObjectUploadRecord"].is_object());
}

#[tokio::test]
async fn openapi_docs_returns_html_page() {
    let state = build_test_state().await;
    let app = super::router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/openapi/docs")
                .body(Body::empty())
                .expect("build docs request"),
        )
        .await
        .expect("request openapi docs");
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read docs body");
    let body = String::from_utf8(bytes.to_vec()).expect("decode docs body");
    assert!(body.contains("AgentHub OpenAPI"));
    assert!(body.contains("/api/openapi.json"));
}

async fn create_role_auth_token(state: &crate::state::AppState, role: UserRole) -> String {
    let user_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    sqlx::query(
        r#"
        INSERT INTO users (id, username, display_name, role, password_hash, created_at)
        VALUES (?1, ?2, ?3, ?4, NULL, ?5)
        "#,
    )
    .bind(&user_id)
    .bind(format!("{}-{}", role.as_str(), Uuid::new_v4()))
    .bind("OpenAPI Role User")
    .bind(role.as_str())
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert role user");

    if role == UserRole::Device {
        sqlx::query(
            r#"
            INSERT INTO devices (id, user_id, name, user_agent, status, created_at)
            VALUES (?1, ?2, ?3, ?4, 'active', ?5)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&user_id)
        .bind("OpenAPI Test Device")
        .bind("openapi-test")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert role device");
    }

    state
        .auth
        .create_session(&user_id)
        .await
        .expect("create role token")
}
