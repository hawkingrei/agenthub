use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use serde_json::Value;
use tower::util::ServiceExt;

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
    assert!(value["paths"]["/api/agents/{id}/uploads/sessions"]["post"].is_object());
    assert!(
        value["paths"]["/api/agents/{id}/uploads/sessions/{session_id}/cancel"]["post"].is_object()
    );
    assert!(
        value["paths"]["/api/agents/{id}/uploads/sessions/{session_id}/complete"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]["/api/agents/{id}/uploads/sessions/{session_id}/direct-write"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]["/api/agents/{id}/uploads/sessions/{session_id}/complete-direct"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]["/api/agents/{id}/uploads/sessions/{session_id}/multipart"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]
            ["/api/agents/{id}/uploads/sessions/{session_id}/multipart/parts/{part_number}"]
            ["post"]
            .is_object()
    );
    assert!(
        value["paths"]["/api/agents/{id}/uploads/sessions/{session_id}/multipart/complete"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]["/api/agents/{id}/uploads/sessions/{session_id}/multipart/abort"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]["/api/agents/{id}/uploads/sessions/{session_id}/parts/{part_number}"]
            ["post"]
            .is_object()
    );
    assert!(
        value["paths"]["/api/agents/{id}/uploads/sessions/{session_id}/complete-parts"]["post"]
            .is_object()
    );
    assert!(value["paths"]["/api/agents/{id}/images"]["post"].is_object());
    assert!(value["paths"]["/api/teams/prompt_defaults"]["get"].is_object());
    assert!(value["paths"]["/api/teams/{id}"]["delete"].is_object());
    assert!(value["paths"]["/api/teams/{id}/channels"]["get"].is_object());
    assert!(value["paths"]["/api/teams/{id}/channels"]["post"].is_object());
    assert!(value["paths"]["/api/teams/{id}/channels/{channel_id}"]["delete"].is_object());
    assert!(value["paths"]["/api/teams/{id}/uploads"]["post"].is_object());
    assert!(value["paths"]["/api/teams/{id}/uploads/sessions"]["post"].is_object());
    assert!(
        value["paths"]["/api/teams/{id}/uploads/sessions/{session_id}/cancel"]["post"].is_object()
    );
    assert!(
        value["paths"]["/api/teams/{id}/uploads/sessions/{session_id}/complete"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]["/api/teams/{id}/uploads/sessions/{session_id}/direct-write"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]["/api/teams/{id}/uploads/sessions/{session_id}/complete-direct"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]["/api/teams/{id}/uploads/sessions/{session_id}/multipart"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]
            ["/api/teams/{id}/uploads/sessions/{session_id}/multipart/parts/{part_number}"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]["/api/teams/{id}/uploads/sessions/{session_id}/multipart/complete"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]["/api/teams/{id}/uploads/sessions/{session_id}/multipart/abort"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]["/api/teams/{id}/uploads/sessions/{session_id}/parts/{part_number}"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]["/api/teams/{id}/uploads/sessions/{session_id}/complete-parts"]["post"]
            .is_object()
    );
    assert!(value["paths"]["/api/teams/{id}/images"]["post"].is_object());
    assert!(value["paths"]["/api/teams/{id}/tasks/{task_id}/uploads"]["post"].is_object());
    assert!(value["paths"]["/api/teams/{id}/tasks/{task_id}/uploads/sessions"]["post"].is_object());
    assert!(
        value["paths"]
            ["/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/cancel"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]
            ["/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/complete"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]
            ["/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/direct-write"]["post"]
            .is_object()
    );
    assert!(
        value["paths"]
            ["/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/complete-direct"]
            ["post"]
            .is_object()
    );
    assert!(
        value["paths"]["/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/multipart"]
            ["post"]
            .is_object()
    );
    assert!(
        value["paths"]
            ["/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/multipart/parts/{part_number}"]
            ["post"]
            .is_object()
    );
    assert!(
        value["paths"]
            ["/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/multipart/complete"]
            ["post"]
            .is_object()
    );
    assert!(
        value["paths"]
            ["/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/multipart/abort"]
            ["post"]
            .is_object()
    );
    assert!(
        value["paths"]
            ["/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/parts/{part_number}"]
            ["post"]
            .is_object()
    );
    assert!(
        value["paths"]
            ["/api/teams/{id}/tasks/{task_id}/uploads/sessions/{session_id}/complete-parts"]
            ["post"]
            .is_object()
    );
    assert!(value["paths"]["/api/teams/{id}/tasks/{task_id}/images"]["post"].is_object());
    assert!(value["paths"]["/api/teams/{id}/runs"].is_object());
    assert!(value["paths"]["/api/teams/runs/{run_id}/resume"].is_object());
    assert!(value["paths"]["/api/teams/runs/{run_id}/restart"].is_object());
    assert!(value["paths"]["/api/teams/runs/{run_id}/snapshot"].is_object());
    assert!(value["components"]["schemas"]["TeamChannelRecord"].is_object());
    assert!(value["components"]["schemas"]["CreateTeamChannelRequest"].is_object());
    assert!(value["components"]["schemas"]["TeamUploadRequest"].is_object());
    assert!(value["components"]["schemas"]["UploadSessionPrepareRequest"].is_object());
    assert!(value["components"]["schemas"]["UploadSessionDirectWriteRequest"].is_object());
    assert!(value["components"]["schemas"]["UploadSessionDirectWriteResponse"].is_object());
    assert!(value["components"]["schemas"]["UploadSessionMultipartUploadResponse"].is_object());
    assert!(value["components"]["schemas"]["UploadSessionMultipartPartWriteRequest"].is_object());
    assert!(value["components"]["schemas"]["UploadSessionMultipartPartWriteResponse"].is_object());
    assert!(value["components"]["schemas"]["UploadSessionMultipartCompleteRequest"].is_object());
    assert!(value["components"]["schemas"]["UploadSessionMultipartAbortRequest"].is_object());
    assert!(value["components"]["schemas"]["ObjectUploadRecord"].is_object());
    assert!(value["components"]["schemas"]["ObjectUploadSessionRecord"].is_object());
    assert!(value["components"]["schemas"]["ObjectUploadSessionPartRecord"].is_object());
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
