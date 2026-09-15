#[tokio::test]
async fn loop_work_operator_activation_is_owner_scoped_idempotent_and_offline() {
    use agenthub_agent_domain::loop_runtime::{
        LoopLimits, LoopPolicyState, LoopSessionPolicy, LoopTriggerKind,
    };
    use agenthub_db::loop_runtime::{LoopPolicyUpdate, LoopStore};
    let state = build_test_state().await;
    let token = create_auth_token(&state).await;
    let headers = build_json_request(Method::GET, "/", Some(&token), None)
        .headers()
        .clone();
    let Json(team)=create_team(State(state.clone()),headers,Json(CreateTeamRequest {
        name:"operator-loop-work".into(),description:None,
        spec:json!({"execution_mode":"loop","entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
    })).await.unwrap();
    let store = LoopStore::new(state.db.clone());
    store
        .configure(
            LoopPolicyUpdate {
                actor_id: "planner",
                team_id: &team.id,
                expected_revision: 1,
                state: LoopPolicyState::Suspended,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits {
                    pending_per_actor: 1,
                    sources_per_activation: 1,
                    ..Default::default()
                },
            },
            chrono::Utc::now().timestamp(),
        )
        .await
        .unwrap();
    let app = super::router(state.clone());
    let uri = format!("/{}/members/planner/loop/activate", team.id);
    let input = json!({"source_key":"operator:one"});
    let first = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &uri,
            Some(&token),
            Some(input.clone()),
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first: Value = decode_json_body(first).await;
    let again = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &uri,
            Some(&token),
            Some(input),
        ))
        .await
        .unwrap();
    assert_eq!(again.status(), StatusCode::OK);
    let again: Value = decode_json_body(again).await;
    assert_eq!(again["trigger_id"], first["trigger_id"]);
    assert_eq!(again["duplicate"], true);
    let sources = store
        .triggers(&team.id, first["activation_id"].as_str().unwrap())
        .await
        .unwrap();
    assert_eq!(sources[0].input.kind, LoopTriggerKind::Operator);
    assert!(sources[0].input.references.scheduling_user_id.is_some());
    assert!(sources[0].input.references.scheduling_actor_id.is_none());
    let overflow = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &uri,
            Some(&token),
            Some(json!({"source_key":"operator:two"})),
        ))
        .await
        .unwrap();
    assert_eq!(overflow.status(), StatusCode::TOO_MANY_REQUESTS);
    let viewer = create_auth_token_with_role(&state, UserRole::Viewer).await;
    let denied = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &uri,
            Some(&viewer),
            Some(json!({"source_key":"viewer"})),
        ))
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    let forged = app
        .oneshot(build_json_request(
            Method::POST,
            &uri,
            Some(&token),
            Some(json!({"source_key":"forged","scheduling_actor_id":"planner"})),
        ))
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_sessions")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(sessions, 0);
}
