use agenthub_auth_domain::UserRole;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::{
    api::team_tests::{build_test_state, create_auth_token_with_role_and_user_id},
    state::AppState,
    team::TeamDefinitionConfig,
};

fn manifest() -> Value {
    json!({
        "schema_version": 1, "scopes": ["read", "write"],
        "tools": [{"name": "lookup", "input_schema": {"type": "object", "additionalProperties": false},
            "required_scopes": ["read"], "replay": {"kind": "read_only"}}]
    })
}

fn registration(owner: Option<&str>, namespace: &str) -> Value {
    json!({"name": "Example tools", "owner_user_id": owner, "manifest": manifest(),
        "connection": {"endpoint": "https://private.example.test/mcp", "credential_env": "PRIVATE_APP_TOKEN",
            "authority": "example", "namespace": namespace}})
}

async fn request(
    router: &Router,
    method: Method,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
    expected: StatusCode,
) -> Value {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let request = match body {
        Some(body) => request
            .header("content-type", "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1_048_576).await.unwrap();
    assert_eq!(
        status,
        expected,
        "{path}: {}",
        String::from_utf8_lossy(&bytes)
    );
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(!text.contains("PRIVATE_APP_TOKEN") && !text.contains("private.example.test"));
    serde_json::from_str(&text).unwrap()
}

async fn team(state: &AppState, owner: Option<&str>, actor: &str) -> String {
    state.teams.create_team_with_owner(TeamDefinitionConfig {
        name: format!("app-{actor}"), description: None,
        spec: json!({"execution_mode": "loop", "entrypoint": actor, "members": [{"member_id": actor, "role": "coordinator"}]}),
    }, owner).await.unwrap().id
}

mod bindings;
mod discovery;
mod registration;
mod validation;
