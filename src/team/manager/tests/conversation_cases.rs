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

fn body_store() -> std::sync::Arc<agenthub_message_store::InMemoryBodyStore> {
    std::sync::Arc::new(agenthub_message_store::InMemoryBodyStore::new())
}

async fn body_store_team_and_task(manager: &TeamManager) -> crate::team::TeamTaskRecord {
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "body-store-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"coordinator","members":[{"member_id":"coordinator"}]}),
        })
        .await
        .expect("create team");
    let (task, _conversation) = manager
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
    task
}

#[tokio::test]
async fn conversation_body_dual_writes_to_store_while_retaining_sqlite_payload() {
    use agenthub_message_store::{AuthorityMessageId, MessageBodyStore};

    let db = setup_test_db().await;
    let store = body_store();
    let manager = TeamManager::new(db.clone()).with_body_store(Some(
        store.clone() as crate::message_body_store::SharedBodyStore
    ));
    let task = body_store_team_and_task(&manager).await;

    let payload =
        json!({"type":"chat_message","text":"a fairly chatty message body","extra":{"k":"v"}});
    let message = manager
        .append_task_conversation_message(&task.id, "user", None, "group_chat", payload.clone())
        .await
        .expect("append message");
    // The returned in-memory record keeps the full payload.
    assert_eq!(message.payload, payload);

    // Phase 1 retains the full authoritative compatibility body in SQLite.
    let stored: String =
        sqlx::query_scalar("SELECT payload_json FROM team_conversation_messages WHERE id = ?1")
            .bind(message.message_id)
            .fetch_one(&db)
            .await
            .expect("read stored payload");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stored).expect("stored payload is valid json"),
        payload
    );

    // Promoted columns stay populated so queries keep working without the body.
    let text_col: Option<String> =
        sqlx::query_scalar("SELECT text FROM team_conversation_messages WHERE id = ?1")
            .bind(message.message_id)
            .fetch_one(&db)
            .await
            .expect("read text column");
    assert_eq!(text_col.as_deref(), Some("a fairly chatty message body"));

    // The real body is staged in the durable outbox, not yet in the store.
    let key = AuthorityMessageId::new(format!("tcm:{}", message.message_id));
    assert_eq!(
        agenthub_db::message_body_outbox::pending_count(&db)
            .await
            .unwrap(),
        1
    );
    assert!(!store.contains(&key));

    // Reads remain available from SQLite while the body is staged.
    let messages = manager
        .list_task_conversation_messages(&task.id, 50, None)
        .await
        .expect("list before drain");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].payload, payload);

    // Draining writes the duplicate body into the store and clears the outbox.
    let drained = agenthub_db::message_body_outbox::drain_into(&db, store.as_ref(), 64)
        .await
        .unwrap();
    assert_eq!(drained, 1);
    assert_eq!(
        agenthub_db::message_body_outbox::pending_count(&db)
            .await
            .unwrap(),
        0
    );
    assert!(store.contains(&key));

    // The SQLite read remains unchanged after asynchronous compression.
    let messages = manager
        .list_task_conversation_messages(&task.id, 50, None)
        .await
        .expect("list after drain");
    assert_eq!(messages[0].payload, payload);
}

#[tokio::test]
async fn conversation_idempotency_replay_with_dual_written_body_is_not_a_false_conflict() {
    let db = setup_test_db().await;
    let store = body_store();
    let manager = TeamManager::new(db.clone()).with_body_store(Some(
        store.clone() as crate::message_body_store::SharedBodyStore
    ));
    let task = body_store_team_and_task(&manager).await;

    let payload = json!({"type":"chat_message","text":"idempotent body"});
    let (first, first_created) = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "user",
            None,
            "group_chat",
            payload.clone(),
            Some("idem-key-1"),
        )
        .await
        .expect("first append");
    assert!(first_created);

    // Drain the duplicate body before replaying the idempotent write.
    agenthub_db::message_body_outbox::drain_into(&db, store.as_ref(), 64)
        .await
        .unwrap();

    let (second, second_created) = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "user",
            None,
            "group_chat",
            payload.clone(),
            Some("idem-key-1"),
        )
        .await
        .expect("idempotent replay should not error");
    assert!(!second_created);
    assert_eq!(second.message_id, first.message_id);
}

#[tokio::test]
async fn conversation_body_stays_inline_without_body_store() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let task = body_store_team_and_task(&manager).await;

    let payload = json!({"type":"chat_message","text":"inline body"});
    let message = manager
        .append_task_conversation_message(&task.id, "user", None, "group_chat", payload.clone())
        .await
        .expect("append message");

    let stored: String =
        sqlx::query_scalar("SELECT payload_json FROM team_conversation_messages WHERE id = ?1")
            .bind(message.message_id)
            .fetch_one(&db)
            .await
            .expect("read stored payload");
    // Body stays inline; the legacy sentinel is never written.
    assert_ne!(stored, "::agenthub:tcm-body-moved::");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stored).expect("inline payload is valid json"),
        payload
    );
    assert_eq!(
        agenthub_db::message_body_outbox::pending_count(&db)
            .await
            .unwrap(),
        0
    );

    let messages = manager
        .list_task_conversation_messages(&task.id, 50, None)
        .await
        .expect("list");
    assert_eq!(messages[0].payload, payload);
}

#[tokio::test]
async fn migrate_backfills_inline_conversation_bodies_into_store() {
    use agenthub_message_store::{AuthorityMessageId, MessageBodyStore};

    let db = setup_test_db().await;
    // Write rows without a store, so the bodies are stored inline (the pre-migration shape).
    let writer = TeamManager::new(db.clone());
    let task = body_store_team_and_task(&writer).await;

    let p1 = json!({"type":"chat_message","text":"first inline body"});
    let p2 = json!({"type":"chat_message","text":"second inline body"});
    let m1 = writer
        .append_task_conversation_message(&task.id, "user", None, "group_chat", p1.clone())
        .await
        .expect("append 1");
    let m2 = writer
        .append_task_conversation_message(&task.id, "user", None, "group_chat", p2.clone())
        .await
        .expect("append 2");

    assert_eq!(
        crate::team::count_pending_conversation_body_migration(&db)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        agenthub_db::message_body_outbox::pending_count(&db)
            .await
            .unwrap(),
        0
    );

    // Migrate into a fresh store with batch_size 1 to exercise the batching loop.
    let store = body_store();
    let report = crate::team::migrate_conversation_bodies_into_store(
        &db,
        store.as_ref(),
        1,
        1_700_000_000,
        std::time::Duration::ZERO,
    )
    .await
    .expect("migrate");
    assert_eq!(report.restored, 0);
    assert_eq!(report.staged, 2);
    assert_eq!(report.drained, 2);

    // SQLite retains the bodies, the compressed copies live in the store, and the outbox is empty.
    assert_eq!(
        crate::team::count_pending_conversation_body_migration(&db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        agenthub_db::message_body_outbox::pending_count(&db)
            .await
            .unwrap(),
        0
    );
    assert!(store.contains(&AuthorityMessageId::new(format!("tcm:{}", m1.message_id))));
    assert!(store.contains(&AuthorityMessageId::new(format!("tcm:{}", m2.message_id))));

    // A reader with the store sees the original SQLite payloads.
    let reader = TeamManager::new(db.clone()).with_body_store(Some(
        store.clone() as crate::message_body_store::SharedBodyStore
    ));
    let messages = reader
        .list_task_conversation_messages(&task.id, 50, None)
        .await
        .expect("list after migrate");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].payload, p1);
    assert_eq!(messages[1].payload, p2);

    // Re-running the migration is a no-op.
    let report_again = crate::team::migrate_conversation_bodies_into_store(
        &db,
        store.as_ref(),
        16,
        1_700_000_000,
        std::time::Duration::ZERO,
    )
    .await
    .expect("migrate again");
    assert_eq!(report_again.restored, 0);
    assert_eq!(report_again.staged, 0);
}

#[tokio::test]
async fn migrate_does_not_head_of_line_block_on_a_persistently_failing_body() {
    use agenthub_message_store::{AuthorityMessageId, MessageBodyStore};

    let db = setup_test_db().await;
    let writer = TeamManager::new(db.clone());
    let task = body_store_team_and_task(&writer).await;

    let mut ids = Vec::new();
    for i in 0..3 {
        let message = writer
            .append_task_conversation_message(
                &task.id,
                "user",
                None,
                "group_chat",
                json!({"type":"chat_message","text":format!("body {i}")}),
            )
            .await
            .expect("append");
        ids.push(message.message_id);
    }

    // Persistently reject the middle row's body, so it stays stuck in the outbox.
    let store = FaultInjectingBodyStore::new();
    store.reject_put_key(format!("tcm:{}", ids[1]));

    // batch_size 1: a naive drain keyed off the row batch would let the stuck oldest body block the
    // rest. The migration must still stage every other body into the store and only leave the stuck one.
    let report = crate::team::migrate_conversation_bodies_into_store(
        &db,
        &store,
        1,
        1_700_000_000,
        std::time::Duration::ZERO,
    )
    .await
    .expect("migrate");
    assert_eq!(report.staged, 3);
    assert_eq!(report.drained, 2);

    assert!(store.contains(&AuthorityMessageId::new(format!("tcm:{}", ids[0]))));
    assert!(!store.contains(&AuthorityMessageId::new(format!("tcm:{}", ids[1]))));
    assert!(store.contains(&AuthorityMessageId::new(format!("tcm:{}", ids[2]))));
    // Only the rejected body is still pending; every compatibility body remains in SQLite.
    assert_eq!(
        agenthub_db::message_body_outbox::pending_count(&db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        crate::team::count_pending_conversation_body_migration(&db)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn drain_retries_after_transient_put_failure_without_losing_body() {
    use std::sync::Arc;

    let db = setup_test_db().await;
    let store = Arc::new(FaultInjectingBodyStore::new());
    let manager = TeamManager::new(db.clone()).with_body_store(Some(
        store.clone() as crate::message_body_store::SharedBodyStore
    ));
    let task = body_store_team_and_task(&manager).await;

    let payload = json!({"type":"chat_message","text":"resilient body"});
    let message = manager
        .append_task_conversation_message(&task.id, "user", None, "group_chat", payload.clone())
        .await
        .expect("append");

    // The first drain's store write fails: nothing is confirmed and the body stays in the outbox.
    store.fail_next_puts(1);
    let drained = agenthub_db::message_body_outbox::drain_into(&db, store.as_ref(), 64)
        .await
        .unwrap();
    assert_eq!(drained, 0);
    assert_eq!(
        agenthub_db::message_body_outbox::pending_count(&db)
            .await
            .unwrap(),
        1
    );

    // The Phase 1 read path remains available from SQLite while the store does not have the body.
    let messages = manager
        .list_task_conversation_messages(&task.id, 50, None)
        .await
        .expect("list during store outage");
    assert_eq!(messages[0].payload, payload);

    // A later drain (e.g. the background drainer retrying) succeeds and clears the outbox.
    let drained = agenthub_db::message_body_outbox::drain_into(&db, store.as_ref(), 64)
        .await
        .unwrap();
    assert_eq!(drained, 1);
    assert_eq!(
        agenthub_db::message_body_outbox::pending_count(&db)
            .await
            .unwrap(),
        0
    );

    let messages = manager
        .list_task_conversation_messages(&task.id, 50, None)
        .await
        .expect("list after recovery");
    assert_eq!(messages[0].payload, payload);
    assert_eq!(message.payload, payload);
    // One failed write plus one successful retry — the body was re-attempted, not dropped.
    assert_eq!(store.put_calls(), 2);
}

#[tokio::test]
async fn phase_one_reads_do_not_depend_on_body_store_gets() {
    use std::sync::Arc;

    let db = setup_test_db().await;
    let store = Arc::new(FaultInjectingBodyStore::new());
    let manager = TeamManager::new(db.clone()).with_body_store(Some(
        store.clone() as crate::message_body_store::SharedBodyStore
    ));
    let task = body_store_team_and_task(&manager).await;

    let payload = json!({"type":"chat_message","text":"body behind a flaky store"});
    manager
        .append_task_conversation_message(&task.id, "user", None, "group_chat", payload.clone())
        .await
        .expect("append");
    // Drain the duplicate body, then make a future store read fail.
    agenthub_db::message_body_outbox::drain_into(&db, store.as_ref(), 64)
        .await
        .unwrap();

    // SQLite remains the Phase 1 source of truth, so a body-store outage cannot affect a read.
    store.fail_next_gets(1);
    let result = manager
        .list_task_conversation_messages(&task.id, 50, None)
        .await;
    assert_eq!(
        result.expect("SQLite compatibility read")[0].payload,
        payload
    );
    assert_eq!(
        store.get_calls(),
        0,
        "normal Phase 1 reads do not query the store"
    );
}

#[tokio::test]
async fn phase_one_reads_ignore_a_corrupt_compressed_copy() {
    use std::sync::Arc;

    let db = setup_test_db().await;
    let store = Arc::new(FaultInjectingBodyStore::new());
    let manager = TeamManager::new(db.clone()).with_body_store(Some(
        store.clone() as crate::message_body_store::SharedBodyStore
    ));
    let task = body_store_team_and_task(&manager).await;

    let message = manager
        .append_task_conversation_message(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"type":"chat_message","text":"body that gets corrupted at rest"}),
        )
        .await
        .expect("append");
    agenthub_db::message_body_outbox::drain_into(&db, store.as_ref(), 64)
        .await
        .unwrap();

    // A corrupt compressed copy cannot affect the retained SQLite compatibility body.
    store.corrupt_get_key(format!("tcm:{}", message.message_id));
    let result = manager
        .list_task_conversation_messages(&task.id, 50, None)
        .await;
    assert!(
        result.is_ok(),
        "SQLite compatibility read must succeed: {result:?}"
    );
}

#[tokio::test]
async fn migrate_restores_legacy_sentinel_before_dual_write_backfill() {
    use agenthub_message_store::{AuthorityMessageId, MessageBodyStore};

    let db = setup_test_db().await;
    let store = body_store();
    let manager = TeamManager::new(db.clone()).with_body_store(Some(
        store.clone() as crate::message_body_store::SharedBodyStore
    ));
    let task = body_store_team_and_task(&manager).await;
    let payload = json!({"type":"chat_message","text":"legacy sentinel body"});
    let message = manager
        .append_task_conversation_message(&task.id, "user", None, "group_chat", payload.clone())
        .await
        .expect("append");
    agenthub_db::message_body_outbox::drain_into(&db, store.as_ref(), 64)
        .await
        .expect("drain");
    sqlx::query("UPDATE team_conversation_messages SET payload_json = ?1 WHERE id = ?2")
        .bind("::agenthub:tcm-body-moved::")
        .bind(message.message_id)
        .execute(&db)
        .await
        .expect("simulate legacy row");

    let report = crate::team::migrate_conversation_bodies_into_store(
        &db,
        store.as_ref(),
        16,
        1_700_000_000,
        std::time::Duration::ZERO,
    )
    .await
    .expect("restore and backfill");
    assert_eq!(report.restored, 1);
    assert_eq!(report.staged, 1);
    let stored: String =
        sqlx::query_scalar("SELECT payload_json FROM team_conversation_messages WHERE id = ?1")
            .bind(message.message_id)
            .fetch_one(&db)
            .await
            .expect("read restored payload");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stored).unwrap(),
        payload
    );
    assert!(store.contains(&AuthorityMessageId::new(format!(
        "tcm:{}",
        message.message_id
    ))));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_append_drain_and_read_keep_sqlite_compatibility_bodies() {
    use agenthub_message_store::{InMemoryBodyStore, MessageBodyStore};
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    // A real file-backed WAL pool so the write path, the outbox drainer, and the read path actually
    // interleave (the shared :memory: pool serializes everything).
    let (db, dir) = setup_concurrent_conversation_db().await;

    // Remove the temp database directory even if the test panics. Cleanup is spawned so the blocking
    // filesystem work stays off the async drop path.
    struct CleanupGuard(std::path::PathBuf);
    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            let path = std::mem::take(&mut self.0);
            tokio::spawn(async move {
                let _ = tokio::fs::remove_dir_all(path).await;
            });
        }
    }
    let _cleanup = CleanupGuard(dir);

    let store = Arc::new(InMemoryBodyStore::new());
    let manager = Arc::new(TeamManager::new(db.clone()).with_body_store(Some(
        store.clone() as crate::message_body_store::SharedBodyStore
    )));

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "race-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"coordinator","members":[{"member_id":"coordinator"}]}),
        })
        .await
        .expect("create team");
    let (task, _conversation) = manager
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
    let task_id = task.id;

    const MESSAGE_COUNT: usize = 40;

    // The drainer and readers run for the whole duration of the appends (driven by this flag) rather
    // than a fixed iteration count that might finish before the writes do.
    let appenders_done = Arc::new(AtomicBool::new(false));

    // Appenders write concurrently: each retains its SQLite body and stages a compressed copy.
    let mut appenders = Vec::new();
    for i in 0..MESSAGE_COUNT {
        let manager = manager.clone();
        let task_id = task_id.clone();
        appenders.push(tokio::spawn(async move {
            manager
                .append_task_conversation_message(
                    &task_id,
                    "user",
                    None,
                    "group_chat",
                    json!({"type":"chat_message","seq":i,"text":format!("message {i}")}),
                )
                .await
                .expect("append");
        }));
    }

    // A drainer moves staged bodies into the store concurrently with the writes, exercising the
    // store↔outbox handoff the read path must tolerate.
    let drainer = {
        let db = db.clone();
        let store = store.clone();
        let appenders_done = appenders_done.clone();
        tokio::spawn(async move {
            while !appenders_done.load(Ordering::Relaxed) {
                agenthub_db::message_body_outbox::drain_into(&db, store.as_ref(), 8)
                    .await
                    .expect("drain");
                tokio::task::yield_now().await;
            }
        })
    };

    // Readers continuously list while writes/drains race. The retained SQLite body must always be a
    // real object payload; asynchronous store writes must never surface a placeholder or read error.
    let mut readers = Vec::new();
    for _ in 0..3 {
        let manager = manager.clone();
        let task_id = task_id.clone();
        let appenders_done = appenders_done.clone();
        readers.push(tokio::spawn(async move {
            while !appenders_done.load(Ordering::Relaxed) {
                let messages = manager
                    .list_task_conversation_messages(&task_id, 100, None)
                    .await
                    .expect("list must not error mid-race");
                for message in &messages {
                    assert!(
                        message
                            .payload
                            .get("seq")
                            .and_then(serde_json::Value::as_u64)
                            .is_some(),
                        "a SQLite compatibility body must retain its real payload, got {:?}",
                        message.payload
                    );
                }
                tokio::task::yield_now().await;
            }
        }));
    }

    for appender in appenders {
        appender.await.expect("appender task");
    }
    appenders_done.store(true, Ordering::Relaxed);
    drainer.await.expect("drainer task");
    for reader in readers {
        reader.await.expect("reader task");
    }

    // Drain whatever is left, then assert the final state is fully consistent: every message is present
    // exactly once with its real body, the outbox is empty, and the store holds one body per message.
    loop {
        let drained = agenthub_db::message_body_outbox::drain_into(&db, store.as_ref(), 64)
            .await
            .expect("final drain");
        if drained == 0 {
            break;
        }
    }

    let messages = manager
        .list_task_conversation_messages(&task_id, 200, None)
        .await
        .expect("final list");
    assert_eq!(messages.len(), MESSAGE_COUNT);
    let mut seqs = HashSet::new();
    for message in &messages {
        let seq = message
            .payload
            .get("seq")
            .and_then(serde_json::Value::as_u64)
            .expect("SQLite compatibility payload keeps seq");
        assert_eq!(message.payload["text"], json!(format!("message {seq}")));
        seqs.insert(seq);
    }
    assert_eq!(seqs.len(), MESSAGE_COUNT, "every distinct message survived");
    assert_eq!(
        agenthub_db::message_body_outbox::pending_count(&db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(store.len(), MESSAGE_COUNT);
}
