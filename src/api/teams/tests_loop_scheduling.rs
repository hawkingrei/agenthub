#[tokio::test]
async fn loop_schedule_http_owner_controls_offline_registration_and_revocation() {
    use agenthub_agent_domain::loop_runtime::{LoopLimits, LoopPolicyState, LoopSessionPolicy};
    use agenthub_db::loop_runtime::{LoopPolicyUpdate, LoopStore};
    let state = build_test_state().await;
    let token = create_auth_token(&state).await;
    let headers = build_json_request(Method::GET, "/", Some(&token), None)
        .headers()
        .clone();
    let Json(team) = create_team(State(state.clone()), headers, Json(CreateTeamRequest {
        name: "owner-schedules".into(), description: None,
        spec: json!({"execution_mode":"loop","entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
    })).await.unwrap();
    let store = LoopStore::new(state.db.clone());
    let now = chrono::Utc::now().timestamp();
    store
        .configure(
            LoopPolicyUpdate {
                actor_id: "planner",
                team_id: &team.id,
                expected_revision: 1,
                state: LoopPolicyState::Suspended,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits {
                    standing_per_actor: 1,
                    ..Default::default()
                },
            },
            now,
        )
        .await
        .unwrap();
    let app = super::router(state.clone());
    let uri = format!("/{}/members/planner/loop/schedules", team.id);
    let input = json!({"source_key":"operator-clock","schedule":{"kind":"recurring","first_at":now,"interval_seconds":60}});
    let response = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &uri,
            Some(&token),
            Some(input.clone()),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let receipt: Value = decode_json_body(response).await;
    assert!(receipt["registration"]["input"]["references"]["scheduling_user_id"].is_string());
    assert!(receipt["registration"]["input"]["references"]["scheduling_actor_id"].is_null());
    let response = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &uri,
            Some(&token),
            Some(input),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(decode_json_body(response).await["duplicate"], true);
    let response = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &uri,
            Some(&token),
            Some(json!({"source_key":"extra","schedule":{"kind":"due","due_at":now}})),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let response = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("{uri}?limit=1"),
            Some(&token),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        decode_json_body(response).await["registrations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(store.reconcile_schedules(now).await.unwrap().len(), 1);
    let detail_uri = format!("{uri}/{}", receipt["registration"]["id"].as_str().unwrap());
    let response = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &detail_uri,
            Some(&token),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let detail: Value = decode_json_body(response).await;
    assert_eq!(detail["firings"].as_array().unwrap().len(), 1);
    let viewer = create_auth_token_with_role(&state, UserRole::Viewer).await;
    for method in [Method::POST, Method::DELETE] {
        let target = if method == Method::DELETE {
            &detail_uri
        } else {
            &uri
        };
        let response = app
            .clone()
            .oneshot(build_json_request(
                method,
                target,
                Some(&viewer),
                Some(json!({"source_key":"denied","schedule":{"kind":"due","due_at":now}})),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let response = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("{uri}?limit=0"),
            Some(&token),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = app.clone().oneshot(build_json_request(Method::POST, &uri, Some(&token), Some(json!({"source_key":"forged","actor_id":"planner","schedule":{"kind":"due","due_at":now}})))).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(build_json_request(
                Method::DELETE,
                &detail_uri,
                Some(&token),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(decode_json_body(response).await["state"], "revoked");
    }
    assert!(
        store
            .reconcile_schedules(now + 120)
            .await
            .unwrap()
            .is_empty()
    );
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_sessions")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(sessions, 0);
}
