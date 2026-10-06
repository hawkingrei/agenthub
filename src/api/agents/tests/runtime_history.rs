use agenthub_db::runtime_events::{RuntimeEventStore, RuntimeRequestIntent, RuntimeRequestKind};

use super::*;

#[cfg(target_os = "linux")]
#[tokio::test]
async fn native_continuity_reset_selects_the_provider_and_requires_cleanup_and_authority() {
    let state = crate::api::team_tests::build_test_state().await;
    sqlx::query("INSERT INTO agents (id, name, workdir, command, args, worktree_mode, status, created_at, updated_at) VALUES ('reset-native', 'reset', '/tmp', 'rara', '[]', 'use_existing', 'stopped', 1, 1)")
        .execute(&state.db).await.unwrap();
    let store = agenthub_db::native_sessions::NativeSessionStore::new(state.db.clone());
    let owner = store
        .reserve("reset-native", "reset-local", "reset-daemon", 1)
        .await
        .unwrap();
    store.authorize_spawn(&owner, 2).await.unwrap();
    sqlx::query("INSERT INTO agent_sessions (id, agent_id, status, started_at) VALUES ('reset-local', 'reset-native', 'running', 2)")
        .execute(&state.db).await.unwrap();
    store
        .begin_conversation(
            &owner,
            &"a".repeat(64),
            agenthub_agent_domain::loop_runtime::LoopSessionPolicy::Fresh,
            3,
        )
        .await
        .unwrap();
    store
        .bind_conversation(&owner, "reset-conversation", 4)
        .await
        .unwrap();
    let viewer = create_role_auth_token(&state, UserRole::Viewer).await;
    let operator = create_role_auth_token(&state, UserRole::Operator).await;
    let app = router(state.clone());
    let route = "/reset-native/acp/session/clear";
    for (token, expected) in [
        (&viewer, StatusCode::UNAUTHORIZED),
        (&operator, StatusCode::CONFLICT),
    ] {
        let response = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                route,
                Some(token),
                Some(json!({})),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        if expected == StatusCode::UNAUTHORIZED {
            assert!(
                decode_json_body(response)
                    .await
                    .to_string()
                    .contains("runtime:operate required")
            );
        }
    }
    store.cleanup_verified(&owner, 5).await.unwrap();
    let response = app
        .oneshot(build_json_request(
            Method::POST,
            route,
            Some(&operator),
            Some(json!({})),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bindings: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM native_standalone_conversations WHERE agent_id = 'reset-native'",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    let owners: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM native_execution_owners WHERE agent_id = 'reset-native'",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!((bindings, owners), (0, 1));
}

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
    // Caller IDs deliberately sort in the opposite order to their creation time.
    for (id, created_at) in [("z-older", 1), ("a-newer", 2)] {
        store
            .prepare_request(
                RuntimeRequestIntent {
                    request_id: id,
                    kind: RuntimeRequestKind::Prompt,
                    target_session_id: Some("native-history"),
                    expected_turn_id: None,
                },
                created_at,
            )
            .await
            .unwrap();
    }
    store.mark_request_sent("z-older", 3).await.unwrap();
    store.close(4).await.unwrap();
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
    assert_eq!(body["receipts"][0]["request_id"], "a-newer");
    assert_eq!(body["next_before_request_id"], "a-newer");
    let response = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("{route}?before_request_id=a-newer"),
            Some(&token),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = decode_json_body(response).await;
    assert_eq!(body["receipts"][0]["status"], "outcome_unknown");
    assert_eq!(body["receipts"][0]["request_id"], "z-older");
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
    for cursor in ["bad%20cursor", "missing-request"] {
        let response = app
            .clone()
            .oneshot(build_json_request(
                Method::GET,
                &format!("{route}?before_request_id={cursor}"),
                Some(&token),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let response = app
        .oneshot(build_json_request(Method::GET, route, None, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
