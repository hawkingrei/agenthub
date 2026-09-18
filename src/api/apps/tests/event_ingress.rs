use agenthub_agent_domain::loop_runtime::{LoopLimits, LoopPolicyState, LoopSessionPolicy};
use agenthub_db::loop_runtime::{LoopPolicyUpdate, LoopStore};
use base64::{Engine, engine::general_purpose::STANDARD};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use super::*;

#[tokio::test]
async fn signed_event_http_workflow_uses_isolated_daemon_credentials() {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        tokio::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "api::apps::tests::event_ingress::signed_event_http_child",
                "--ignored",
                "--nocapture",
            ])
            .env("TEST_SIGNED_APP_KEY", STANDARD.encode([19; 32]))
            .env_remove("TEST_ABSENT_SIGNED_APP_KEY")
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn deliver(
    router: &Router,
    app: &str,
    version: i64,
    body: &str,
    key: &[u8; 32],
    expected: StatusCode,
) -> Value {
    let timestamp = chrono::Utc::now().timestamp();
    let mut mac = Hmac::<Sha256>::new_from_slice(key).unwrap();
    mac.update(format!("agenthub.app-event.v1\n{app}\n{version}\n{timestamp}\n").as_bytes());
    mac.update(body.as_bytes());
    let signature = STANDARD.encode(mac.finalize().into_bytes());
    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/apps/{app}/events"))
        .header("content-type", "application/json")
        .header("x-agenthub-app-key-version", version.to_string())
        .header("x-agenthub-app-timestamp", timestamp.to_string())
        .header("x-agenthub-app-signature", signature)
        .body(Body::from(body.to_owned()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 16384).await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(!text.contains("TEST_SIGNED_APP_KEY") && !text.contains(&STANDARD.encode([19; 32])));
    assert_eq!(status, expected, "{text}");
    serde_json::from_str(&text).unwrap()
}

#[tokio::test]
#[ignore = "Runs only through the parent with an isolated daemon environment"]
async fn signed_event_http_child() {
    let state = build_test_state().await;
    let (_, root) = create_auth_token_with_role_and_user_id(&state, UserRole::Root).await;
    let (owner_id, owner) =
        create_auth_token_with_role_and_user_id(&state, UserRole::Operator).await;
    let (_, other) = create_auth_token_with_role_and_user_id(&state, UserRole::Operator).await;
    let router = crate::api::router(state.clone());
    let mut input = registration(Some(&owner_id), "http-events");
    input["manifest"]["events"] = json!([{"name":"changed","required_scopes":["read"]}]);
    let app = request(
        &router,
        Method::POST,
        "/apps",
        Some(&root),
        Some(input),
        StatusCode::CREATED,
    )
    .await;
    let id = app["id"].as_str().unwrap();
    let team_id = team(&state, Some(&owner_id), "planner").await;
    let loops = LoopStore::new(state.db.clone());
    let revision = loops
        .policy(&team_id, "planner")
        .await
        .unwrap()
        .map_or(0, |policy| policy.revision);
    loops
        .configure(
            LoopPolicyUpdate {
                team_id: &team_id,
                actor_id: "planner",
                expected_revision: revision,
                state: LoopPolicyState::Suspended,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits::default(),
            },
            chrono::Utc::now().timestamp(),
        )
        .await
        .unwrap();
    let key_path = format!("/apps/{id}/event-key");
    let grant_path = format!("/teams/{team_id}/apps/{id}");
    let binding_path = format!("/teams/{team_id}/members/planner/apps/{id}");
    let route_path = format!("{binding_path}/events");
    request(
        &router,
        Method::PUT,
        &key_path,
        Some(&root),
        Some(json!({"expected_version":0,"credential_env":"TEST_SIGNED_APP_KEY"})),
        StatusCode::OK,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &grant_path,
        Some(&owner),
        Some(json!({"expected_revision":0,"scopes":["read"]})),
        StatusCode::OK,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &binding_path,
        Some(&owner),
        Some(json!({"expected_revision":0,"version":1,"scopes":["read"]})),
        StatusCode::OK,
    )
    .await;
    let first = json!({"schema_version":1,"event_id":"http-1","cursor":1,"team_id":team_id,"actor_id":"planner","event_class":"changed"});
    request(
        &router,
        Method::POST,
        &format!("/apps/{id}/events"),
        Some(&owner),
        Some(first.clone()),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    deliver(
        &router,
        id,
        1,
        &first.to_string(),
        &[20; 32],
        StatusCode::UNAUTHORIZED,
    )
    .await;
    deliver(
        &router,
        id,
        1,
        &first.to_string(),
        &[19; 32],
        StatusCode::FORBIDDEN,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &route_path,
        Some(&owner),
        Some(json!({"expected_revision":0,"classes":["changed"]})),
        StatusCode::OK,
    )
    .await;
    let accepted = deliver(
        &router,
        id,
        1,
        &first.to_string(),
        &[19; 32],
        StatusCode::ACCEPTED,
    )
    .await;
    assert_eq!(accepted["duplicate"], false);
    let duplicate = deliver(
        &router,
        id,
        1,
        &format!("{first}\n"),
        &[19; 32],
        StatusCode::OK,
    )
    .await;
    assert_eq!(duplicate["trigger_id"], accepted["trigger_id"]);
    let mut changed = first.clone();
    changed["cursor"] = json!(2);
    deliver(
        &router,
        id,
        1,
        &changed.to_string(),
        &[19; 32],
        StatusCode::CONFLICT,
    )
    .await;
    changed["event_id"] = json!("http-2");
    changed["event_class"] = json!("undeclared");
    deliver(
        &router,
        id,
        1,
        &changed.to_string(),
        &[19; 32],
        StatusCode::FORBIDDEN,
    )
    .await;
    changed["event_class"] = json!("changed");
    let mut injected = changed.clone();
    injected["command"] = json!("untrusted request");
    deliver(
        &router,
        id,
        1,
        &injected.to_string(),
        &[19; 32],
        StatusCode::BAD_REQUEST,
    )
    .await;
    deliver(
        &router,
        id,
        1,
        &"x".repeat(8193),
        &[19; 32],
        StatusCode::BAD_REQUEST,
    )
    .await;
    request(
        &router,
        Method::POST,
        &format!("{route_path}/revoke"),
        Some(&owner),
        Some(json!({"expected_revision":1})),
        StatusCode::OK,
    )
    .await;
    deliver(
        &router,
        id,
        1,
        &changed.to_string(),
        &[19; 32],
        StatusCode::FORBIDDEN,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &route_path,
        Some(&owner),
        Some(json!({"expected_revision":2,"classes":["changed"]})),
        StatusCode::OK,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &key_path,
        Some(&root),
        Some(json!({"expected_version":1,"credential_env":"TEST_ABSENT_SIGNED_APP_KEY"})),
        StatusCode::OK,
    )
    .await;
    deliver(
        &router,
        id,
        1,
        &changed.to_string(),
        &[19; 32],
        StatusCode::UNAUTHORIZED,
    )
    .await;
    deliver(
        &router,
        id,
        2,
        &changed.to_string(),
        &[19; 32],
        StatusCode::UNAUTHORIZED,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &key_path,
        Some(&root),
        Some(json!({"expected_version":2,"credential_env":"TEST_SIGNED_APP_KEY"})),
        StatusCode::OK,
    )
    .await;
    deliver(
        &router,
        id,
        3,
        &changed.to_string(),
        &[19; 32],
        StatusCode::ACCEPTED,
    )
    .await;
    sqlx::query("UPDATE app_event_budgets SET accepted_count = 30 WHERE scope_kind = 'actor' AND scope_id = 'planner'").execute(&state.db).await.unwrap();
    changed["event_id"] = json!("http-3");
    changed["cursor"] = json!(3);
    deliver(
        &router,
        id,
        3,
        &changed.to_string(),
        &[19; 32],
        StatusCode::TOO_MANY_REQUESTS,
    )
    .await;
    let history = loops
        .activation_source_history(
            &team_id,
            "planner",
            accepted["activation_id"].as_str().unwrap(),
            None,
            10,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(history.sources.len(), 2);
    assert!(
        history
            .sources
            .iter()
            .all(|source| source.references.app_event.as_ref().unwrap().version == 1)
    );
    let audit_path = format!("/apps/{id}/event-audit");
    let audit = request(
        &router,
        Method::GET,
        &audit_path,
        Some(&owner),
        None,
        StatusCode::OK,
    )
    .await;
    assert!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["code"] == "unauthorized")
    );
    assert!(audit.to_string().len() < 2048);
    request(
        &router,
        Method::GET,
        &audit_path,
        Some(&other),
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    request(
        &router,
        Method::POST,
        &format!("{key_path}/revoke"),
        Some(&owner),
        Some(json!({"expected_version":3})),
        StatusCode::OK,
    )
    .await;
    deliver(
        &router,
        id,
        3,
        &first.to_string(),
        &[19; 32],
        StatusCode::UNAUTHORIZED,
    )
    .await;
}
