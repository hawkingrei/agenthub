use super::*;

#[tokio::test]
async fn create_team_task_and_run_persist_authority_group_id() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "group-authority-team".to_string(),
                description: Some("team with owner-backed group boundary".to_string()),
                spec: json!({
                    "entrypoint":"coordinator",
                    "members":[{"member_id":"coordinator","role":"coordinator"}]
                }),
            },
            Some("user-group-authority"),
        )
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "Group scoped task",
            "user",
            json!({"summary":"group boundary check"}),
            "group_chat",
            Some("all"),
        )
        .await
        .expect("create task");
    let run = manager
        .create_run(&team.id, Some(&task.id), json!({"task_id":task.id}))
        .await
        .expect("create run");

    let row = sqlx::query(
        r#"
        SELECT
            td.group_id AS team_group_id,
            tt.group_id AS task_group_id,
            tr.group_id AS run_group_id
        FROM team_definitions AS td
        JOIN team_tasks AS tt ON tt.team_id = td.id
        JOIN team_runs AS tr ON tr.team_id = td.id
        WHERE td.id = ?1
          AND tt.id = ?2
          AND tr.id = ?3
        "#,
    )
    .bind(&team.id)
    .bind(&task.id)
    .bind(&run.id)
    .fetch_one(&db)
    .await
    .expect("read authority group ids");
    assert_eq!(
        row.get::<Option<String>, _>("team_group_id"),
        Some("user-group-authority".to_string())
    );
    assert_eq!(
        row.get::<Option<String>, _>("task_group_id"),
        Some("user-group-authority".to_string())
    );
    assert_eq!(
        row.get::<Option<String>, _>("run_group_id"),
        Some("user-group-authority".to_string())
    );
}

#[tokio::test]
async fn append_task_conversation_message_persists_authority_group_id() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "message-group-authority-team".to_string(),
                description: Some("team with message group boundary".to_string()),
                spec: json!({
                    "entrypoint":"coordinator",
                    "members":[{"member_id":"coordinator","role":"coordinator"}]
                }),
            },
            Some("user-message-authority"),
        )
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "Message group task",
            "user",
            json!({"summary":"message group boundary check"}),
            "group_chat",
            Some("all"),
        )
        .await
        .expect("create task");

    let (message, created) = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({
                "type": "chat_message",
                "text": "message with group",
                "correlation_id": "corr-message-authority"
            }),
            Some("message-authority-group-1"),
        )
        .await
        .expect("append message");
    assert!(created);
    assert_eq!(message.group_id.as_deref(), Some("user-message-authority"));

    let stored_group_id: Option<String> =
        sqlx::query_scalar("SELECT group_id FROM team_conversation_messages WHERE id = ?1")
            .bind(message.message_id)
            .fetch_one(&db)
            .await
            .expect("read task message group_id");
    assert_eq!(stored_group_id, Some("user-message-authority".to_string()));
}

#[tokio::test]
async fn send_actor_message_persists_authority_group_id() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "actor-message-group-authority-team".to_string(),
                description: Some("team with actor message group boundary".to_string()),
                spec: json!({
                    "entrypoint":"coordinator",
                    "members":[
                        {"member_id":"coordinator","role":"coordinator"},
                        {"member_id":"worker-1","role":"worker"}
                    ]
                }),
            },
            Some("user-actor-message-authority"),
        )
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "Actor message group task",
            "user",
            json!({"summary":"actor message group boundary check"}),
            "group_chat",
            Some("all"),
        )
        .await
        .expect("create task");
    let run = manager
        .create_run(&team.id, Some(&task.id), json!({"task_id":task.id}))
        .await
        .expect("create run");

    let message = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "coordinator",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "worker-1",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "all",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({
                "type": "chat_message",
                "text": "actor message with group"
            }),
            idempotency_key: Some("actor-message-authority-group-1"),
            message_kind: None,
        })
        .await
        .expect("send actor message");

    let stored_group_id: Option<String> =
        sqlx::query_scalar("SELECT group_id FROM team_actor_messages WHERE id = ?1")
            .bind(message.message_id)
            .fetch_one(&db)
            .await
            .expect("read actor message group_id");
    assert_eq!(
        stored_group_id,
        Some("user-actor-message-authority".to_string())
    );
}

#[tokio::test]
async fn task_and_conversation_messages_are_persisted_with_redaction() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-team".to_string(),
            description: Some("team for task persistence".to_string()),
            spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
        })
        .await
        .expect("create team");

    let (task, conversation) = manager
        .create_task(
            &team.id,
            "Investigate rollout plan",
            "user",
            json!({
                "source":"ui",
                "token":"should_not_persist",
                "nested":{"api_key":"xyz"}
            }),
            "group_chat",
            Some("kickoff"),
        )
        .await
        .expect("create task");
    assert_eq!(task.team_id, team.id);
    assert_eq!(task.status, TeamTaskStatus::Open);
    assert_eq!(task.assigned_member_id, None);
    assert_eq!(conversation.task_id, task.id);
    assert_eq!(task.context["token"], json!("[redacted]"));
    assert_eq!(task.context["nested"]["api_key"], json!("[redacted]"));

    let message = manager
        .append_task_conversation_message(
            &task.id,
            "coordinator",
            Some("worker-1"),
            "to_member",
            json!({
                "text":"draft changes",
                "authorization":"Bearer abc",
                "nested":{"secret":"top-secret"}
            }),
        )
        .await
        .expect("append message");
    assert_eq!(message.task_id, task.id);
    assert_eq!(message.payload["authorization"], json!("[redacted]"));
    assert_eq!(message.payload["nested"]["secret"], json!("[redacted]"));

    let listed = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: Some(team.id.clone()),
            limit: 20,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list tasks");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, task.id);

    let messages = manager
        .list_task_conversation_messages(&task.id, 50, None)
        .await
        .expect("list conversation messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].message_id, message.message_id);
    assert_eq!(messages[0].route, "to_member");
}

#[tokio::test]
async fn append_task_conversation_message_emits_stream_event() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let mut events = manager.subscribe_conversation_events();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-event-team".to_string(),
            description: Some("team for conversation stream events".to_string()),
            spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
        })
        .await
        .expect("create team");
    let (task, conversation) = manager
        .create_task(
            &team.id,
            "all",
            "user",
            json!({"bootstrap_kind":"shared_thread"}),
            "group_chat",
            Some("all"),
        )
        .await
        .expect("create task");

    let message = manager
        .append_task_conversation_message(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"type":"chat_message","text":"hello team"}),
        )
        .await
        .expect("append message");

    let event = tokio::time::timeout(std::time::Duration::from_millis(500), events.recv())
        .await
        .expect("receive stream event")
        .expect("stream event result");
    assert_eq!(event.team_id, team.id);
    assert_eq!(event.task_id, task.id);
    assert_eq!(event.conversation_id, conversation.id);
    assert_eq!(event.message_id, Some(message.message_id));
    assert_eq!(event.source, "conversation_message");
}

#[tokio::test]
async fn append_task_conversation_message_honors_idempotency_key() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let mut events = manager.subscribe_conversation_events();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-idempotency-team".to_string(),
            description: Some("team for task message idempotency".to_string()),
            spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
        })
        .await
        .expect("create team");
    let (task, conversation) = manager
        .create_task(
            &team.id,
            "all",
            "user",
            json!({"bootstrap_kind":"shared_thread"}),
            "group_chat",
            Some("all"),
        )
        .await
        .expect("create task");

    let (first, first_created) = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"type":"chat_message","text":"hello team"}),
            Some("task-msg-1"),
        )
        .await
        .expect("append first message");
    assert!(first_created);

    let (retry, retry_created) = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"type":"chat_message","text":"hello team"}),
            Some("task-msg-1"),
        )
        .await
        .expect("append retry message");
    assert!(!retry_created);
    assert_eq!(first.message_id, retry.message_id);

    let event = tokio::time::timeout(std::time::Duration::from_millis(500), events.recv())
        .await
        .expect("receive first stream event")
        .expect("stream event result");
    assert_eq!(event.team_id, team.id);
    assert_eq!(event.task_id, task.id);
    assert_eq!(event.conversation_id, conversation.id);
    assert_eq!(event.message_id, Some(first.message_id));
    assert_eq!(event.source, "conversation_message");
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), events.recv())
            .await
            .is_err(),
        "retry should not emit a second stream event"
    );

    let err = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"type":"chat_message","text":"changed payload"}),
            Some("task-msg-1"),
        )
        .await
        .expect_err("mismatched payload should conflict");
    assert!(
        TeamManager::is_task_message_idempotency_conflict(&err),
        "expected idempotency conflict, got: {err:?}"
    );

    let messages = manager
        .list_task_conversation_messages(&task.id, 50, None)
        .await
        .expect("list conversation messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].message_id, first.message_id);
}

#[tokio::test]
async fn append_task_conversation_message_persists_correlation_id_column() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-correlation-team".to_string(),
            description: Some("team for task message correlation".to_string()),
            spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
        })
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "all",
            "user",
            json!({"bootstrap_kind":"shared_thread"}),
            "group_chat",
            Some("all"),
        )
        .await
        .expect("create task");

    let (message, created) = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"type":"chat_message","text":"hello team","correlation_id":"corr-task-authority-1"}),
            Some("task-corr-1"),
        )
        .await
        .expect("append message");
    assert!(created);

    let correlation_id: String =
        sqlx::query_scalar("SELECT correlation_id FROM team_conversation_messages WHERE id = ?1")
            .bind(message.message_id)
            .fetch_one(&db)
            .await
            .expect("read task message correlation_id");
    assert_eq!(correlation_id, "corr-task-authority-1");

    let direct_message = manager
        .append_task_conversation_message(
            &task.id,
            "user",
            Some("  "),
            "group_chat",
            json!({
                "type":"chat_message",
                "text":"hello without idempotency",
                "correlation_id":" corr-task-direct-1 "
            }),
        )
        .await
        .expect("append direct message");
    let direct_correlation_id: String =
        sqlx::query_scalar("SELECT correlation_id FROM team_conversation_messages WHERE id = ?1")
            .bind(direct_message.message_id)
            .fetch_one(&db)
            .await
            .expect("read direct task message correlation_id");
    assert_eq!(direct_correlation_id, "corr-task-direct-1");
}

#[tokio::test]
async fn append_task_conversation_message_propagates_non_idempotency_insert_failures() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-idempotency-insert-failure-team".to_string(),
            description: Some("team for task message insert failure".to_string()),
            spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
        })
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "all",
            "user",
            json!({"bootstrap_kind":"shared_thread"}),
            "group_chat",
            Some("all"),
        )
        .await
        .expect("create task");

    sqlx::query(
        r#"
        CREATE TRIGGER fail_task_message_insert
        BEFORE INSERT ON team_conversation_messages
        WHEN NEW.idempotency_key IS NOT NULL
        BEGIN
            SELECT RAISE(FAIL, 'forced task message insert failure');
        END;
        "#,
    )
    .execute(&db)
    .await
    .expect("create failing trigger");

    let err = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"type":"chat_message","text":"hello team"}),
            Some("task-msg-trigger-fail"),
        )
        .await
        .expect_err("trigger failure should propagate");
    assert!(
        err.to_string()
            .contains("forced task message insert failure"),
        "expected insert failure to propagate, got: {err:?}"
    );
    assert!(
        !TeamManager::is_task_message_idempotency_conflict(&err),
        "expected non-idempotency error, got: {err:?}"
    );
}
