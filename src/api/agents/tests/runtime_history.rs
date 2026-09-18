use agenthub_db::runtime_events::{RuntimeEventStore, RuntimeRequestIntent, RuntimeRequestKind};

use super::*;

#[tokio::test]
async fn runtime_history_route_is_authorized_scoped_and_available_after_exit() {
    let state = build_test_state().await;
    // This shared fixture also tests legacy schemas without remote placement support.
    sqlx::query("ALTER TABLE agents ADD COLUMN target_node_id TEXT")
        .execute(&state.db)
        .await
        .unwrap();
    let token = create_role_auth_token(&state, UserRole::Viewer).await;
    sqlx::query("INSERT INTO agents (id, name, workdir, command, args, worktree_mode, status, created_at, updated_at) VALUES ('history-agent', 'history', '/tmp', 'rara', '[]', 'use_existing', 'exited', 1, 1)")
        .execute(&state.db).await.unwrap();
    sqlx::query("INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at) VALUES ('local-history', 'history-agent', 'exited', 1, 2)")
        .execute(&state.db).await.unwrap();
    let pool = state
        .agents
        .test_event_pool_for_agent("history-agent")
        .await
        .unwrap();
    let store = RuntimeEventStore::bind(pool, "local-history", "runtime-history")
        .await
        .unwrap();
    store.bind_stream("native-history").await.unwrap();
    for id in ["first", "second"] {
        store
            .prepare_request(
                RuntimeRequestIntent {
                    request_id: id,
                    kind: RuntimeRequestKind::Prompt,
                    target_session_id: Some("native-history"),
                    expected_turn_id: None,
                },
                1,
            )
            .await
            .unwrap();
    }
    store.mark_request_sent("first", 2).await.unwrap();
    store.close(3).await.unwrap();
    let app = router(state.clone());
    let route = "/history-agent/sessions/local-history/runtime";
    let response = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("{route}?limit=1"),
            Some(&token),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = decode_json_body(response).await;
    assert_eq!(body["closed"], true);
    assert_eq!(body["runtime_id"], "runtime-history");
    assert_eq!(body["receipts"][0]["status"], "not_sent");
    assert_eq!(body["next_before_request_id"], "second");
    let response = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("{route}?before_request_id=second"),
            Some(&token),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = decode_json_body(response).await;
    assert_eq!(body["receipts"][0]["status"], "outcome_unknown");
    assert!(body["next_before_request_id"].is_null());
    for route in [
        "/missing/sessions/local-history/runtime",
        "/history-agent/sessions/native-history/runtime",
        "/history-agent/sessions/missing/runtime",
    ] {
        let response = app
            .clone()
            .oneshot(build_json_request(Method::GET, route, Some(&token), None))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    let response = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("{route}?before_request_id=bad%20cursor"),
            Some(&token),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = app
        .oneshot(build_json_request(Method::GET, route, None, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
