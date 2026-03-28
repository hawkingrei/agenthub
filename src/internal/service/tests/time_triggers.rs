use super::*;

#[tokio::test]
async fn internal_grpc_describe_team_context_defaults_to_scoped_run() {
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Leader, Some("planner"), Some(&run.id));
    let service = TeamInternalControlService::new(
        state,
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let context = TeamInternalControl::describe_team_context(
        &service,
        authenticated_request(
            DescribeTeamContextRequest {
                team_id: String::new(),
                run_id: String::new(),
                actor_id: "planner".to_string(),
            },
            &token,
        ),
    )
    .await
    .expect("scoped run should default describe_team_context target")
    .into_inner();
    let context_json: serde_json::Value =
        serde_json::from_str(&context.context_json).expect("decode scoped run context");
    assert_eq!(context_json["team_id"], json!(run.team_id));
    assert_eq!(context_json["run"]["run_id"], json!(run.id));
}

#[tokio::test]
async fn internal_grpc_time_trigger_controls_are_wire_compatible() {
    let state = build_test_state().await;
    sqlx::query(
        r#"
            CREATE TABLE agent_time_triggers (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                created_by_actor_id TEXT NOT NULL,
                message_text TEXT NOT NULL,
                fire_at INTEGER NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                fired_at INTEGER,
                last_error TEXT,
                FOREIGN KEY(agent_id) REFERENCES agents(id)
            )
            "#,
    )
    .execute(&state.db)
    .await
    .expect("create agent_time_triggers");
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Worker, Some("reviewer"), None);
    let service = TeamInternalControlService::new(
        state,
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );
    let fire_at = chrono::Utc::now().timestamp() + 120;

    let created = TeamInternalControl::create_time_trigger(
        &service,
        authenticated_request(
            CreateTimeTriggerRequest {
                actor_id: "reviewer".to_string(),
                message_text: "Ping the reviewer inbox".to_string(),
                fire_at,
            },
            &token,
        ),
    )
    .await
    .expect("create time trigger")
    .into_inner();
    let trigger: AgentTimeTriggerRecord =
        serde_json::from_str(&created.trigger_json).expect("decode trigger");
    assert_eq!(trigger.agent_id, "reviewer");
    assert_eq!(trigger.created_by_actor_id, "reviewer");
    assert_eq!(trigger.message_text, "Ping the reviewer inbox");
    assert_eq!(trigger.fire_at, fire_at);

    let listed = TeamInternalControl::list_time_triggers(
        &service,
        authenticated_request(
            ListTimeTriggersRequest {
                actor_id: "reviewer".to_string(),
                limit: 20,
            },
            &token,
        ),
    )
    .await
    .expect("list time triggers")
    .into_inner();
    let triggers: Vec<AgentTimeTriggerRecord> =
        serde_json::from_str(&listed.triggers_json).expect("decode trigger list");
    assert!(triggers.iter().any(|item| item.id == trigger.id));

    let canceled = TeamInternalControl::cancel_time_trigger(
        &service,
        authenticated_request(
            CancelTimeTriggerRequest {
                actor_id: "reviewer".to_string(),
                trigger_id: trigger.id.clone(),
            },
            &token,
        ),
    )
    .await
    .expect("cancel time trigger")
    .into_inner();
    let canceled_json: serde_json::Value =
        serde_json::from_str(&canceled.output_json).expect("decode cancel output");
    assert_eq!(canceled_json["status"], json!("ok"));
    assert_eq!(canceled_json["trigger_id"], json!(trigger.id.clone()));

    let listed_after_cancel = TeamInternalControl::list_time_triggers(
        &service,
        authenticated_request(
            ListTimeTriggersRequest {
                actor_id: "reviewer".to_string(),
                limit: 20,
            },
            &token,
        ),
    )
    .await
    .expect("list time triggers after cancel")
    .into_inner();
    let triggers_after_cancel: Vec<AgentTimeTriggerRecord> =
        serde_json::from_str(&listed_after_cancel.triggers_json)
            .expect("decode trigger list after cancel");
    let canceled_trigger = triggers_after_cancel
        .iter()
        .find(|item| item.id == trigger.id)
        .expect("canceled trigger remains queryable");
    assert_eq!(
        serde_json::to_value(&canceled_trigger.status).expect("serialize canceled status"),
        json!("canceled")
    );
}
