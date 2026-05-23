use super::*;

#[tokio::test]
async fn actor_mailbox_service_channel_send_broadcasts_and_preserves_mentions() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-channel-team".to_string(),
            description: Some("team for channel mailbox broadcast".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner"},
                    {"member_id":"reviewer"},
                    {"member_id":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-channel-mailbox"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let sent = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: None,
            channel_id: Some("all".to_string()),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"@reviewer please validate api contract"
            }),
            idempotency_key: Some("msg-channel-mailbox-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("send channel mailbox message");
    assert_eq!(sent.state, TeamActorMessageStatus::Pending);

    let rows = sqlx::query(
        r#"
        SELECT to_actor_id, payload_json
        FROM team_actor_messages
        WHERE run_id = ?1
        ORDER BY id ASC
        "#,
    )
    .bind(&run.id)
    .fetch_all(&db)
    .await
    .expect("load channel mailbox rows");
    assert_eq!(rows.len(), 2);
    let recipients = rows
        .iter()
        .map(|row| row.get::<String, _>("to_actor_id"))
        .collect::<Vec<_>>();
    assert_eq!(
        recipients,
        vec!["reviewer".to_string(), "worker".to_string()]
    );
    for row in &rows {
        let payload: Value = serde_json::from_str(row.get::<String, _>("payload_json").as_str())
            .expect("decode forwarded channel payload");
        assert_eq!(payload["delivery_scope"], json!("channel_broadcast"));
        assert_eq!(payload["channel_id"], json!("all"));
        assert_eq!(payload["team_id"], json!(team.id));
        assert!(
            payload["authority_message_id"]
                .as_i64()
                .is_some_and(|value| value > 0),
            "missing authority_message_id: {payload}"
        );
        assert_eq!(payload["mention_actor_ids"], json!(["reviewer"]));
        assert_eq!(payload["mentioned_actor_ids"], json!(["reviewer"]));
        assert_eq!(
            payload["text"],
            json!("@reviewer please validate api contract")
        );
    }

    let shared_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_conversation_messages
        WHERE task_id IN (
            SELECT id
            FROM team_tasks
            WHERE team_id = ?1
              AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        )
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count shared thread messages");
    assert_eq!(shared_count, 1);

    let replica_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM team_channel_message_replicas")
            .fetch_one(&db)
            .await
            .expect("count channel replica rows");
    assert_eq!(replica_count, 0);
}

#[tokio::test]
async fn actor_mailbox_service_channel_send_honors_explicit_mentions_without_raw_text() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-explicit-mention-team".to_string(),
            description: Some("team for explicit channel mention payloads".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner"},
                    {"member_id":"reviewer"},
                    {"member_id":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-channel-explicit-mention"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: None,
            channel_id: Some("all".to_string()),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"please review the api contract",
                "mentioned_actor_ids":["reviewer"]
            }),
            idempotency_key: Some("msg-channel-explicit-mention-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("send explicit mention channel message");

    let rows = sqlx::query(
        r#"
        SELECT to_actor_id, payload_json
        FROM team_actor_messages
        WHERE run_id = ?1
        ORDER BY id ASC
        "#,
    )
    .bind(&run.id)
    .fetch_all(&db)
    .await
    .expect("load channel mailbox rows");
    assert_eq!(rows.len(), 2);

    for row in &rows {
        let payload: Value = serde_json::from_str(row.get::<String, _>("payload_json").as_str())
            .expect("decode forwarded payload");
        assert_eq!(payload["mention_actor_ids"], json!(["reviewer"]));
        assert_eq!(payload["mentioned_actor_ids"], json!(["reviewer"]));
    }
}

#[tokio::test]
async fn actor_mailbox_service_channel_send_reuses_canonical_message_on_idempotent_retry() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-channel-idempotent-team".to_string(),
            description: Some("team for channel mailbox idempotency".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner"},
                    {"member_id":"reviewer"},
                    {"member_id":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-channel-mailbox-idempotent"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    for _ in 0..2 {
        service
            .actor_send(ActorSendRequest {
                run_id: run.id.clone(),
                from_actor_id: "planner".to_string(),
                from_peer_id: None,
                to_actor_id: None,
                channel_id: Some("all".to_string()),
                to_peer_id: None,
                channel: Some("coordination".to_string()),
                transport: Some(TeamActorMessageTransport::Local),
                route: None,
                payload: json!({
                    "type":"chat_message",
                    "text":"@reviewer please validate retry behavior"
                }),
                idempotency_key: Some("msg-channel-mailbox-idempotent-1".to_string()),
                message_kind: None,
            })
            .await
            .expect("send channel mailbox message");
    }

    let shared_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_conversation_messages
        WHERE task_id IN (
            SELECT id
            FROM team_tasks
            WHERE team_id = ?1
              AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        )
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count shared thread messages");
    assert_eq!(shared_count, 1);

    let mailbox_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_actor_messages
        WHERE run_id = ?1
        "#,
    )
    .bind(&run.id)
    .fetch_one(&db)
    .await
    .expect("count mailbox rows");
    assert_eq!(mailbox_count, 2);
}

#[tokio::test]
async fn actor_mailbox_service_persists_agent_reply_into_shared_thread() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();
    let mut events = manager.subscribe_conversation_events();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-shared-thread-team".to_string(),
            description: Some("team for canonical shared reply persistence".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-shared-thread"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let before_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM team_conversation_messages WHERE task_id IN (SELECT id FROM team_tasks WHERE team_id = ?1)",
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count conversation messages before send");
    assert_eq!(before_count, 0);

    service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("user".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"hello human",
                "current_phase":"planning",
                "correlation_id":"corr-1"
            }),
            idempotency_key: Some("msg-shared-thread-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("send shared thread reply");

    let shared_task_row = sqlx::query(
        r#"
        SELECT id, context_json
        FROM team_tasks
        WHERE team_id = ?1
          AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        LIMIT 1
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("load shared thread task");
    let shared_task_id: String = shared_task_row.get("id");
    let shared_task_context: String = shared_task_row.get("context_json");
    let shared_task_context_json: Value =
        serde_json::from_str(&shared_task_context).expect("decode shared task context");
    assert_eq!(
        shared_task_context_json["bootstrap_kind"],
        json!("shared_thread")
    );

    let row = sqlx::query(
        r#"
        SELECT
            from_actor_id,
            to_actor_id,
            route,
            payload_json
        FROM team_conversation_messages
        WHERE task_id = ?1
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(&shared_task_id)
    .fetch_one(&db)
    .await
    .expect("load canonical shared thread message");

    let from_actor_id: String = row.get("from_actor_id");
    let to_actor_id: Option<String> = row.get("to_actor_id");
    let route: String = row.get("route");
    let payload_json: String = row.get("payload_json");
    let payload: Value = serde_json::from_str(&payload_json).expect("decode canonical payload");
    assert_eq!(from_actor_id, "planner");
    assert_eq!(to_actor_id, None);
    assert_eq!(route, "group_chat");
    assert_eq!(payload["type"], json!("chat_message"));
    assert_eq!(payload["text"], json!("hello human"));
    assert_eq!(payload["correlation_id"], json!("corr-1"));
    assert!(payload.get("current_phase").is_none());

    let event = tokio::time::timeout(std::time::Duration::from_millis(500), events.recv())
        .await
        .expect("receive canonical shared thread event")
        .expect("canonical shared thread event result");
    assert_eq!(event.team_id, team.id);
    assert_eq!(event.task_id, shared_task_id);
    assert_eq!(event.message_id, None);
    assert_eq!(event.source, "canonical_chat_reply");
}

#[tokio::test]
async fn actor_mailbox_service_deduped_shared_thread_reply_does_not_duplicate_conversation() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-shared-thread-dedup-team".to_string(),
            description: Some("team for shared reply idempotency".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-shared-thread-dedup"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let first = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("user".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({"type":"chat_message","text":"hello human"}),
            idempotency_key: Some("msg-shared-thread-dedup".to_string()),
            message_kind: None,
        })
        .await
        .expect("first shared thread send");
    let second = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("user".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({"type":"chat_message","text":"hello human"}),
            idempotency_key: Some("msg-shared-thread-dedup".to_string()),
            message_kind: None,
        })
        .await
        .expect("deduped shared thread send");
    assert_eq!(first.message_id, second.message_id);

    let canonical_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_conversation_messages
        WHERE task_id IN (
            SELECT id
            FROM team_tasks
            WHERE team_id = ?1
              AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        )
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count canonical shared thread messages");
    assert_eq!(canonical_count, 1);
}

#[tokio::test]
async fn actor_mailbox_service_does_not_persist_agent_to_agent_chat_into_shared_thread() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-private-chat-team".to_string(),
            description: Some("team for private mailbox reply routing".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-private-chat"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("reviewer".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"internal review request"
            }),
            idempotency_key: Some("msg-private-chat-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("send internal mailbox reply");

    let shared_task_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_tasks
        WHERE team_id = ?1
          AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count shared thread tasks");
    assert_eq!(shared_task_count, 0);

    let conversation_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM team_conversation_messages WHERE task_id IN (SELECT id FROM team_tasks WHERE team_id = ?1)",
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count conversation messages after private send");
    assert_eq!(conversation_count, 0);
}

#[tokio::test]
async fn actor_mailbox_service_canonicalizes_stringified_json_reply_into_shared_thread() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-stringified-reply-team".to_string(),
            description: Some("team for stringified shared reply canonicalization".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-stringified-reply"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("user".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!("{\"type\":\"chat_message\",\"text\":\"hello from string\",\"current_phase\":\"planning\",\"correlation_id\":\"corr-string\"}"),
            idempotency_key: Some("msg-stringified-chat-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("send stringified shared reply");

    let payload_json: String = sqlx::query_scalar(
        r#"
        SELECT payload_json
        FROM team_conversation_messages
        WHERE task_id IN (
            SELECT id
            FROM team_tasks
            WHERE team_id = ?1
              AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        )
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("load canonical payload for stringified reply");
    let payload: Value =
        serde_json::from_str(&payload_json).expect("decode canonical stringified payload");
    assert_eq!(payload["type"], json!("chat_message"));
    assert_eq!(payload["text"], json!("hello from string"));
    assert_eq!(payload["correlation_id"], json!("corr-string"));
    assert!(payload.get("current_phase").is_none());
}

#[tokio::test]
async fn actor_mailbox_service_reuses_existing_shared_thread_for_canonical_reply() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-existing-shared-thread-team".to_string(),
            description: Some("team for shared thread reuse".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-existing-shared-thread"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let (shared_task, _conversation) = manager
        .create_task(
            &team.id,
            "all",
            "user",
            json!({
                "bootstrap_kind":"shared_thread",
                "bootstrap_source":"test"
            }),
            "group_chat",
            Some("shared"),
        )
        .await
        .expect("create existing shared thread");

    service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("user".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"reuse existing thread"
            }),
            idempotency_key: Some("msg-existing-shared-thread-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("send shared thread reply into existing thread");

    let shared_task_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_tasks
        WHERE team_id = ?1
          AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count shared thread tasks after reuse");
    assert_eq!(shared_task_count, 1);

    let message_task_id: String = sqlx::query_scalar(
        r#"
        SELECT task_id
        FROM team_conversation_messages
        WHERE task_id IN (
            SELECT id
            FROM team_tasks
            WHERE team_id = ?1
              AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        )
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("load canonical message task id");
    assert_eq!(message_task_id, shared_task.id);
}

#[tokio::test]
async fn actor_mailbox_service_prefers_shared_thread_with_latest_message_when_duplicates_exist() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-canonical-shared-thread-team".to_string(),
            description: Some("team for canonical shared thread selection".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-canonical-shared-thread"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let (preferred_task, _preferred_conversation) = manager
        .create_task(
            &team.id,
            "all",
            "user",
            json!({
                "bootstrap_kind":"shared_thread",
                "bootstrap_source":"test"
            }),
            "group_chat",
            Some("shared"),
        )
        .await
        .expect("create preferred shared thread");
    let (older_task, _older_conversation) = manager
        .create_task(
            &team.id,
            "all",
            "user",
            json!({
                "bootstrap_kind":"shared_thread",
                "bootstrap_source":"test"
            }),
            "group_chat",
            Some("shared"),
        )
        .await
        .expect("create older shared thread");

    manager
        .append_task_conversation_message(
            &older_task.id,
            "user",
            None,
            "group_chat",
            json!({
                "type":"chat_message",
                "text":"older duplicate thread"
            }),
        )
        .await
        .expect("append older duplicate thread message");
    manager
        .append_task_conversation_message(
            &preferred_task.id,
            "user",
            None,
            "group_chat",
            json!({
                "type":"chat_message",
                "text":"newest canonical thread"
            }),
        )
        .await
        .expect("append newest canonical thread message");

    service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("user".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"persist into canonical duplicate thread"
            }),
            idempotency_key: Some("msg-canonical-shared-thread-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("send shared thread reply into canonical duplicate thread");

    let message_task_id: String = sqlx::query_scalar(
        r#"
        SELECT task_id
        FROM team_conversation_messages
        WHERE task_id IN (
            SELECT id
            FROM team_tasks
            WHERE team_id = ?1
              AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        )
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("load canonical duplicate shared thread message task id");
    assert_eq!(message_task_id, preferred_task.id);
}

#[tokio::test]
async fn actor_mailbox_service_validates_required_fields() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);
    let service = manager.actor_mailbox_service();

    let err = service
        .actor_send(ActorSendRequest {
            run_id: " ".to_string(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("reviewer".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: None,
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({"text":"invalid"}),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .expect_err("blank run_id should fail");
    assert_eq!(err.code, ActorServiceErrorCode::BadRequest);
}

#[tokio::test]
async fn actor_message_send_is_idempotent_by_key() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-message-idempotent-team".to_string(),
            description: Some("team for idempotent send flow".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-msg-idempotent"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let first = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"text":"please review"}),
            idempotency_key: Some("msg-1"),
            message_kind: None,
        })
        .await
        .expect("first send");
    let second = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"text":"please review"}),
            idempotency_key: Some("msg-1"),
            message_kind: None,
        })
        .await
        .expect("retry send");
    assert_eq!(first.message_id, second.message_id);

    let deduped_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_actor_messages
        WHERE run_id = ?1 AND from_actor_id = ?2 AND idempotency_key = ?3
        "#,
    )
    .bind(&run.id)
    .bind("planner")
    .bind("msg-1")
    .fetch_one(&db)
    .await
    .expect("count deduped messages");
    assert_eq!(deduped_count, 1);

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let sent_count = events
        .iter()
        .filter(|event| event.event_type == "actor_message_sent")
        .count();
    assert_eq!(sent_count, 1);
}

#[tokio::test]
async fn actor_message_send_rejects_mismatched_payload_for_same_idempotency_key() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-message-idempotency-conflict-team".to_string(),
            description: Some("team for idempotency conflict flow".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-msg-idempotency-conflict"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let _ = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"text":"please review"}),
            idempotency_key: Some("msg-1"),
            message_kind: None,
        })
        .await
        .expect("first send");
    let err = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"text":"changed payload"}),
            idempotency_key: Some("msg-1"),
            message_kind: None,
        })
        .await
        .expect_err("mismatched payload should conflict");
    assert!(
        TeamManager::is_actor_message_idempotency_conflict(&err),
        "expected idempotency conflict error, got: {err}"
    );

    let message_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM team_actor_messages WHERE run_id = ?1 AND from_actor_id = ?2",
    )
    .bind(&run.id)
    .bind("planner")
    .fetch_one(&db)
    .await
    .expect("count actor messages");
    assert_eq!(message_count, 1);
}
