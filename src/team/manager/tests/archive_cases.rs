use super::*;

#[tokio::test]
async fn append_task_conversation_message_dual_writes_created_rows_to_archive() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let team = manager
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "task-archive-team".to_string(),
                description: Some("team for task message archive dual-write".to_string()),
                spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
            },
            Some("user-task-archive"),
        )
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
            Some("coordinator"),
            "to_coordinator",
            json!({
                "type": "chat_message",
                "text": "archive this message",
                "correlation_id": "corr-archive-1"
            }),
            Some("task-archive-msg-1"),
        )
        .await
        .expect("append first message");
    assert!(first_created);

    let (retry, retry_created) = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "user",
            Some("coordinator"),
            "to_coordinator",
            json!({
                "type": "chat_message",
                "text": "archive this message",
                "correlation_id": "corr-archive-1"
            }),
            Some("task-archive-msg-1"),
        )
        .await
        .expect("append retry message");
    assert!(!retry_created);
    assert_eq!(retry.message_id, first.message_id);

    let documents = wait_for_archive_documents(&archive, 1).await;
    assert_eq!(
        documents.len(),
        1,
        "idempotent retries should not dual-write duplicate archive documents"
    );
    let document = &documents[0];
    assert_eq!(
        document.document_id,
        format!(
            "team_conversation_message:{}:{}",
            conversation.id, first.message_id
        )
    );
    assert_eq!(
        document.source_kind,
        MessageDocumentKind::TeamConversationMessage
    );
    assert_eq!(document.source_id, first.message_id.to_string());
    assert_eq!(document.authority_message_id, Some(first.message_id));
    assert_eq!(document.correlation_id.as_deref(), Some("corr-archive-1"));
    assert_eq!(document.group_id.as_deref(), Some("user-task-archive"));
    assert_eq!(document.team_id.as_deref(), Some(team.id.as_str()));
    assert_eq!(
        document.conversation_id.as_deref(),
        Some(conversation.id.as_str())
    );
    assert_eq!(document.task_id.as_deref(), Some(task.id.as_str()));
    assert_eq!(document.body_text, "archive this message");
    assert_eq!(document.created_at, first.created_at);
}

#[tokio::test]
async fn send_actor_message_dual_writes_created_rows_to_archive() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let team = manager
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "actor-message-archive-team".to_string(),
                description: Some("team for actor message archive dual-write".to_string()),
                spec: json!({
                    "entrypoint":"coordinator_plan",
                    "members":[
                        {"member_id":"coordinator"},
                        {"member_id":"worker-1","role":"worker"}
                    ]
                }),
            },
            Some("user-actor-archive"),
        )
        .await
        .expect("create team");
    let (task, conversation) = manager
        .create_task(
            &team.id,
            "Archive actor message",
            "user",
            json!({}),
            "group_chat",
            None,
        )
        .await
        .expect("create task");
    let source_message = manager
        .append_task_conversation_message(
            &task.id,
            "user",
            Some("coordinator"),
            "to_coordinator",
            json!({"type":"chat_message","text":"source actor archive message"}),
        )
        .await
        .expect("append source message");
    let run = manager
        .create_run(
            &team.id,
            Some(task.id.as_str()),
            json!({
                "task_id": task.id,
            }),
        )
        .await
        .expect("create run");

    let first = manager
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
                "text": "live actor archive message",
                "authority_message_id": source_message.message_id,
                "correlation_id": "corr-live-actor"
            }),
            idempotency_key: Some("actor-archive-msg-1"),
            message_kind: None,
        })
        .await
        .expect("send first actor message");
    let retry = manager
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
                "text": "live actor archive message",
                "authority_message_id": source_message.message_id,
                "correlation_id": "corr-live-actor"
            }),
            idempotency_key: Some("actor-archive-msg-1"),
            message_kind: None,
        })
        .await
        .expect("send retry actor message");
    assert_eq!(retry.message_id, first.message_id);

    let documents = wait_for_archive_documents(&archive, 4).await;
    let actor_documents: Vec<_> = documents
        .iter()
        .filter(|document| document.source_kind == MessageDocumentKind::TeamActorMessage)
        .collect();
    assert_eq!(
        actor_documents.len(),
        1,
        "idempotent retries should not dual-write duplicate actor archive documents"
    );
    let document = actor_documents[0];
    assert_eq!(
        document.document_id,
        format!("team_actor_message:{}:{}", run.id, first.message_id)
    );
    assert_eq!(document.source_id, first.message_id.to_string());
    assert_eq!(
        document.authority_message_id,
        Some(source_message.message_id)
    );
    assert_eq!(document.correlation_id.as_deref(), Some("corr-live-actor"));
    assert_eq!(document.group_id.as_deref(), Some("user-actor-archive"));
    assert_eq!(document.team_id.as_deref(), Some(team.id.as_str()));
    assert_eq!(document.run_id.as_deref(), Some(run.id.as_str()));
    assert_eq!(
        document.conversation_id.as_deref(),
        Some(conversation.id.as_str())
    );
    assert_eq!(document.task_id.as_deref(), Some(task.id.as_str()));
    assert_eq!(document.agent_id.as_deref(), Some("worker-1"));
    assert_eq!(document.body_text, "live actor archive message");
    assert_eq!(document.created_at, first.created_at);
    let sent_run_event_documents: Vec<_> = documents
        .iter()
        .filter(|document| {
            document.source_kind == MessageDocumentKind::TeamRunEvent
                && document.run_id.as_deref() == Some(run.id.as_str())
                && document.body_text == "actor_message_sent"
        })
        .collect();
    assert_eq!(
        sent_run_event_documents.len(),
        1,
        "idempotent retries should not dual-write duplicate actor_message_sent archive documents"
    );
    assert!(sent_run_event_documents.iter().any(|document| {
        document.source_kind == MessageDocumentKind::TeamRunEvent
            && document.run_id.as_deref() == Some(run.id.as_str())
            && document.body_text == "actor_message_sent"
            && document.conversation_id.as_deref() == Some(conversation.id.as_str())
            && document.task_id.as_deref() == Some(task.id.as_str())
    }));
}

#[tokio::test]
async fn append_run_event_skips_shared_thread_mailbox_runs_in_archive() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "shared-thread-run-event-archive-team".to_string(),
            description: Some("team for shared-thread run event archive skip".to_string()),
            spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
        })
        .await
        .expect("create team");
    let (task, conversation) = manager
        .create_task(
            &team.id,
            "Shared thread archive skip",
            "user",
            json!({}),
            "group_chat",
            None,
        )
        .await
        .expect("create task");
    let shared_run = manager
        .ensure_shared_thread_mailbox_run(&team.id, &task.id, &conversation.id)
        .await
        .expect("ensure shared thread mailbox run");

    manager
        .append_run_event(
            &shared_run.id,
            "shared_thread_live_event",
            json!({
                "text": "hidden shared thread event",
                "task_id": task.id,
                "conversation_id": conversation.id,
            }),
        )
        .await
        .expect("append shared thread run event");

    let events = manager
        .list_run_events(&shared_run.id, 10, None)
        .await
        .expect("list shared thread run events");
    let live_event = events
        .iter()
        .find(|event| event.event_type == "shared_thread_live_event")
        .expect("shared thread live event");
    assert!(
        manager
            .team_run_event_archive_document(live_event)
            .await
            .expect("build shared thread run event archive document")
            .is_none(),
        "shared-thread mailbox run events should not build archive documents"
    );

    assert_archive_documents_stay_empty(&archive).await;
}

#[tokio::test]
async fn actor_mailbox_run_event_skips_shared_thread_mailbox_runs_in_archive() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "shared-thread-mailbox-archive-team".to_string(),
            description: Some("team for shared-thread mailbox archive skip".to_string()),
            spec: json!({
                "entrypoint":"coordinator_plan",
                "members":[
                    {"member_id":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let (task, conversation) = manager
        .create_task(
            &team.id,
            "Shared thread mailbox archive skip",
            "user",
            json!({}),
            "group_chat",
            None,
        )
        .await
        .expect("create task");
    let shared_run = manager
        .ensure_shared_thread_mailbox_run(&team.id, &task.id, &conversation.id)
        .await
        .expect("ensure shared thread mailbox run");

    manager
        .send_actor_message(SendActorMessageInput {
            run_id: &shared_run.id,
            from_actor_id: "coordinator",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "worker-1",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "all",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({
                "type": "chat_message",
                "text": "hidden shared thread mailbox event",
            }),
            idempotency_key: Some("shared-thread-mailbox-archive-msg-1"),
            message_kind: None,
        })
        .await
        .expect("send shared thread mailbox actor message");

    assert_archive_documents_stay_empty(&archive).await;
}

#[tokio::test]
async fn create_run_dual_writes_submission_event_to_archive() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "run-event-archive-team".to_string(),
            description: Some("team for run event archive dual-write".to_string()),
            spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
        })
        .await
        .expect("create team");
    let (task, conversation) = manager
        .create_task(
            &team.id,
            "Archive run event",
            "user",
            json!({}),
            "group_chat",
            None,
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some(task.id.as_str()),
            json!({
                "task_id": task.id,
            }),
        )
        .await
        .expect("create run");

    let documents = wait_for_archive_documents(&archive, 1).await;
    let document = documents
        .iter()
        .find(|document| document.source_kind == MessageDocumentKind::TeamRunEvent)
        .expect("run event archive document");
    assert_eq!(
        document.document_id,
        format!("team_run_event:{}:{}", run.id, document.source_id)
    );
    assert_eq!(document.team_id.as_deref(), Some(team.id.as_str()));
    assert_eq!(document.run_id.as_deref(), Some(run.id.as_str()));
    assert_eq!(
        document.conversation_id.as_deref(),
        Some(conversation.id.as_str())
    );
    assert_eq!(document.task_id.as_deref(), Some(task.id.as_str()));
    assert_eq!(document.body_text, "run_submitted");
    assert!(
        document
            .payload_json
            .as_deref()
            .is_some_and(|payload| payload.contains("inherit_recent"))
    );
}

#[tokio::test]
async fn append_run_event_dual_writes_created_event_to_archive() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let team = manager
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "append-run-event-archive-team".to_string(),
                description: Some("team for appended run event archive dual-write".to_string()),
                spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
            },
            Some("group-live-run-event"),
        )
        .await
        .expect("create team");
    let (task, conversation) = manager
        .create_task(
            &team.id,
            "Archive appended run event",
            "user",
            json!({}),
            "group_chat",
            None,
        )
        .await
        .expect("create task");
    let run = manager
        .create_run(
            &team.id,
            Some(task.id.as_str()),
            json!({
                "task_id": task.id,
            }),
        )
        .await
        .expect("create run");

    manager
        .append_run_event(
            &run.id,
            "custom_live_event",
            json!({
                "text": "custom live run event",
                "authority_message_id": 42,
                "correlation_id": "corr-live-run"
            }),
        )
        .await
        .expect("append run event");

    let documents = wait_for_archive_documents(&archive, 2).await;
    let document = documents
        .iter()
        .find(|document| document.body_text == "custom live run event")
        .expect("custom run event archive document");
    assert_eq!(document.source_kind, MessageDocumentKind::TeamRunEvent);
    assert_eq!(document.authority_message_id, Some(42));
    assert_eq!(document.correlation_id.as_deref(), Some("corr-live-run"));
    assert_eq!(document.group_id.as_deref(), Some("group-live-run-event"));
    assert_eq!(document.team_id.as_deref(), Some(team.id.as_str()));
    assert_eq!(document.run_id.as_deref(), Some(run.id.as_str()));
    assert_eq!(
        document.conversation_id.as_deref(),
        Some(conversation.id.as_str())
    );
    assert_eq!(document.task_id.as_deref(), Some(task.id.as_str()));
}

#[tokio::test]
async fn migrate_team_messages_to_archive_covers_team_message_tables() {
    let db = setup_test_db().await;
    let seed_manager = TeamManager::new(db.clone());

    let team = seed_manager
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "team-message-archive-migration".to_string(),
                description: Some("team for archive migration".to_string()),
                spec: json!({
                    "entrypoint":"coordinator_plan",
                    "members":[
                        {"member_id":"coordinator"},
                        {"member_id":"worker-1","role":"worker"}
                    ]
                }),
            },
            Some("group-archive-migration"),
        )
        .await
        .expect("create team");
    let (task, conversation) = seed_manager
        .create_task(
            &team.id,
            "Archive migration",
            "user",
            json!({}),
            "group_chat",
            None,
        )
        .await
        .expect("create task");
    let conversation_message = seed_manager
        .append_task_conversation_message(
            &task.id,
            "user",
            Some("coordinator"),
            "to_coordinator",
            json!({
                "type": "chat_message",
                "text": "migrate conversation message",
                "correlation_id": "corr-migration-conversation"
            }),
        )
        .await
        .expect("append conversation message");
    let run = seed_manager
        .create_run(
            &team.id,
            Some(task.id.as_str()),
            json!({
                "task_id": task.id,
                "api_key": "run-input-secret",
            }),
        )
        .await
        .expect("create run");
    let actor_message = seed_manager
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
                "text": "migrate actor mailbox message",
                "authority_message_id": conversation_message.message_id,
                "correlation_id": "corr-migration-actor"
            }),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .expect("send actor message");
    sqlx::query(
        r#"
        INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
        VALUES (?1, NULL, ?2, ?3, ?4)
        "#,
    )
    .bind(&run.id)
    .bind("run_secret_event")
    .bind(Utc::now().timestamp())
    .bind(
        json!({
            "text": "raw run secret event",
            "authority_message_id": conversation_message.message_id,
            "api_key": "run-event-secret"
        })
        .to_string(),
    )
    .execute(&seed_manager.db)
    .await
    .expect("insert raw run event");
    let raw_actor_message_id = sqlx::query(
        r#"
        INSERT INTO team_actor_messages (
            run_id,
            from_actor_id,
            from_peer_id,
            to_actor_id,
            to_peer_id,
            channel,
            transport,
            route_json,
            payload_json,
            status,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9, ?10)
        "#,
    )
    .bind(&run.id)
    .bind("coordinator")
    .bind(ACTOR_MAIN_PEER_ID)
    .bind("worker-1")
    .bind(ACTOR_MAIN_PEER_ID)
    .bind("all")
    .bind("local")
    .bind(
        json!({
            "type": "chat_message",
            "text": "raw actor secret message",
            "api_key": "actor-message-secret"
        })
        .to_string(),
    )
    .bind("pending")
    .bind(Utc::now().timestamp())
    .execute(&seed_manager.db)
    .await
    .expect("insert raw actor message")
    .last_insert_rowid();
    let hidden_run = seed_manager
        .ensure_shared_thread_mailbox_run(&team.id, &task.id, &conversation.id)
        .await
        .expect("create hidden shared-thread mailbox run");
    seed_manager
        .send_actor_message(SendActorMessageInput {
            run_id: &hidden_run.id,
            from_actor_id: "coordinator",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "worker-1",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "all",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({
                "type": "chat_message",
                "text": "hidden shared-thread mailbox message",
                "task_id": task.id,
                "channel_conversation_id": conversation.id,
                "authority_message_id": conversation_message.message_id,
            }),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .expect("send hidden actor message");

    let archive = Arc::new(RecordingMessageArchive::default());
    let archive_manager = TeamManager::new_with_event_dbs_and_message_archive(
        db,
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );
    let report = archive_manager
        .migrate_team_messages_to_archive(2)
        .await
        .expect("migrate team messages");

    assert_eq!(report.team_conversation_messages, 1);
    assert_eq!(report.team_run_events, 3);
    assert_eq!(report.team_actor_messages, 2);
    assert_eq!(report.total_documents(), 6);

    let documents = archive.documents.lock().await.clone();
    assert_eq!(documents.len(), 6);
    assert!(documents.iter().any(|document| {
        document.document_id
            == format!(
                "team_conversation_message:{}:{}",
                conversation_message.conversation_id, conversation_message.message_id
            )
            && document.source_kind == MessageDocumentKind::TeamConversationMessage
            && document.team_id.as_deref() == Some(team.id.as_str())
            && document.correlation_id.as_deref() == Some("corr-migration-conversation")
            && document.body_text == "migrate conversation message"
    }));
    assert_eq!(
        documents
            .iter()
            .filter(|document| document.source_kind == MessageDocumentKind::TeamRunEvent)
            .count(),
        3
    );
    assert!(documents.iter().any(|document| {
        document
            .document_id
            .starts_with(format!("team_run_event:{}:", run.id).as_str())
            && document.source_kind == MessageDocumentKind::TeamRunEvent
            && document.team_id.as_deref() == Some(team.id.as_str())
            && document.run_id.as_deref() == Some(run.id.as_str())
            && document.authority_message_id.is_none()
            && document.conversation_id.as_deref() == Some(conversation.id.as_str())
            && document.task_id.as_deref() == Some(task.id.as_str())
            && document.body_text == "run_submitted"
            && document
                .payload_json
                .as_deref()
                .is_some_and(|payload| !payload.contains("run-input-secret"))
    }));
    assert!(documents.iter().any(|document| {
        document.body_text == "raw run secret event"
            && document.source_kind == MessageDocumentKind::TeamRunEvent
            && document.authority_message_id == Some(conversation_message.message_id)
            && document.group_id.as_deref() == Some("group-archive-migration")
            && document.payload_json.as_deref().is_some_and(|payload| {
                payload.contains("[redacted]") && !payload.contains("run-event-secret")
            })
    }));
    assert!(documents.iter().any(|document| {
        document.document_id
            == format!("team_actor_message:{}:{}", run.id, actor_message.message_id)
            && document.source_kind == MessageDocumentKind::TeamActorMessage
            && document.team_id.as_deref() == Some(team.id.as_str())
            && document.run_id.as_deref() == Some(run.id.as_str())
            && document.authority_message_id == Some(conversation_message.message_id)
            && document.conversation_id.as_deref() == Some(conversation.id.as_str())
            && document.task_id.as_deref() == Some(task.id.as_str())
            && document.agent_id.as_deref() == Some("worker-1")
            && document.correlation_id.as_deref() == Some("corr-migration-actor")
            && document.body_text == "migrate actor mailbox message"
    }));
    assert!(documents.iter().any(|document| {
        document.document_id == format!("team_actor_message:{}:{}", run.id, raw_actor_message_id)
            && document.source_kind == MessageDocumentKind::TeamActorMessage
            && document.authority_message_id.is_none()
            && document.conversation_id.as_deref() == Some(conversation.id.as_str())
            && document.task_id.as_deref() == Some(task.id.as_str())
            && document.body_text == "raw actor secret message"
            && document.payload_json.as_deref().is_some_and(|payload| {
                payload.contains("[redacted]") && !payload.contains("actor-message-secret")
            })
    }));
    assert!(
        documents
            .iter()
            .all(|document| document.run_id.as_deref() != Some(hidden_run.id.as_str()))
    );
}

#[tokio::test]
async fn migrate_team_messages_to_archive_skips_missing_per_agent_event_db() {
    let db = setup_test_db().await;
    let event_dbs = AgentEventDbRouter::new(std::env::temp_dir().join(format!(
        "agenthub-archive-missing-eventdb-{}",
        uuid::Uuid::new_v4()
    )));
    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("idle-agent")
    .bind("idle-agent")
    .bind(std::env::temp_dir().to_string_lossy().to_string())
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("stopped")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert idle agent");

    let idle_event_db_path = event_dbs.db_path_for_agent("idle-agent");
    assert!(
        !idle_event_db_path.exists(),
        "test must start without a per-agent event db"
    );

    let archive = Arc::new(RecordingMessageArchive::default());
    let archive_manager =
        TeamManager::new_with_event_dbs_and_message_archive(db, event_dbs, Some(archive.clone()));
    let report = archive_manager
        .migrate_team_messages_to_archive(2)
        .await
        .expect("migrate with missing per-agent event db");

    assert_eq!(report.agent_events, 0);
    assert_eq!(report.aggregated_acp_messages, 0);
    assert!(
        !idle_event_db_path.exists(),
        "migration should not create empty per-agent event db files"
    );
    let documents = archive.documents.lock().await.clone();
    assert!(documents.is_empty());
}

#[tokio::test]
async fn migrate_team_messages_to_archive_uses_start_snapshot_for_conversation_rows() {
    let db = setup_test_db().await;
    let seed_manager = TeamManager::new(db.clone());

    let team = seed_manager
        .create_team(TeamDefinitionConfig {
            name: "team-message-archive-snapshot".to_string(),
            description: Some("team for archive migration snapshot".to_string()),
            spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
        })
        .await
        .expect("create team");
    let (task, conversation) = seed_manager
        .create_task(
            &team.id,
            "Archive snapshot",
            "user",
            json!({}),
            "group_chat",
            None,
        )
        .await
        .expect("create task");
    seed_manager
        .append_task_conversation_message(
            &task.id,
            "user",
            Some("coordinator"),
            "to_coordinator",
            json!({"type":"chat_message","text":"snapshot message one"}),
        )
        .await
        .expect("append first message");
    seed_manager
        .append_task_conversation_message(
            &task.id,
            "user",
            Some("coordinator"),
            "to_coordinator",
            json!({"type":"chat_message","text":"snapshot message two"}),
        )
        .await
        .expect("append second message");

    let archive = Arc::new(TailAppendingMessageArchive {
        db: db.clone(),
        conversation_id: conversation.id.clone(),
        task_id: task.id.clone(),
        run_id: None,
        inserted: Mutex::new(false),
        documents: Mutex::new(Vec::new()),
    });
    let archive_manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let report = archive_manager
        .migrate_team_messages_to_archive(1)
        .await
        .expect("migrate team messages");

    assert_eq!(report.team_conversation_messages, 2);
    assert_eq!(report.total_documents(), 2);
    let documents = archive.documents.lock().await.clone();
    assert_eq!(documents.len(), 2);
    assert!(
        documents
            .iter()
            .all(|document| document.body_text != "live tail message")
    );
    let stored_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM team_conversation_messages WHERE conversation_id = ?1",
    )
    .bind(&conversation.id)
    .fetch_one(&db)
    .await
    .expect("count conversation messages");
    assert_eq!(stored_count, 3);
}

#[tokio::test]
async fn migrate_team_messages_to_archive_uses_start_snapshot_for_run_and_actor_rows() {
    let db = setup_test_db().await;
    let seed_manager = TeamManager::new(db.clone());

    let team = seed_manager
        .create_team(TeamDefinitionConfig {
            name: "team-message-archive-run-actor-snapshot".to_string(),
            description: Some("team for run and actor archive migration snapshot".to_string()),
            spec: json!({
                "entrypoint":"coordinator_plan",
                "members":[
                    {"member_id":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let (task, conversation) = seed_manager
        .create_task(
            &team.id,
            "Archive run and actor snapshot",
            "user",
            json!({}),
            "group_chat",
            None,
        )
        .await
        .expect("create task");
    let run = seed_manager
        .create_run(
            &team.id,
            Some(task.id.as_str()),
            json!({
                "task_id": task.id,
            }),
        )
        .await
        .expect("create run");
    sqlx::query(
        r#"
        INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
        VALUES (?1, NULL, ?2, ?3, ?4)
        "#,
    )
    .bind(&run.id)
    .bind("snapshot_run_event")
    .bind(Utc::now().timestamp())
    .bind(json!({"text":"snapshot run event"}).to_string())
    .execute(&seed_manager.db)
    .await
    .expect("insert snapshot run event");
    sqlx::query(
        r#"
        INSERT INTO team_actor_messages (
            run_id,
            from_actor_id,
            from_peer_id,
            to_actor_id,
            to_peer_id,
            channel,
            transport,
            route_json,
            payload_json,
            status,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9, ?10)
        "#,
    )
    .bind(&run.id)
    .bind("coordinator")
    .bind(ACTOR_MAIN_PEER_ID)
    .bind("worker-1")
    .bind(ACTOR_MAIN_PEER_ID)
    .bind("all")
    .bind("local")
    .bind(json!({"type":"chat_message","text":"snapshot actor message"}).to_string())
    .bind("pending")
    .bind(Utc::now().timestamp())
    .execute(&seed_manager.db)
    .await
    .expect("insert snapshot actor message");

    let archive = Arc::new(TailAppendingMessageArchive {
        db: db.clone(),
        conversation_id: conversation.id.clone(),
        task_id: task.id.clone(),
        run_id: Some(run.id.clone()),
        inserted: Mutex::new(false),
        documents: Mutex::new(Vec::new()),
    });
    let archive_manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let report = archive_manager
        .migrate_team_messages_to_archive(1)
        .await
        .expect("migrate team messages");

    assert_eq!(report.team_conversation_messages, 0);
    assert_eq!(report.team_run_events, 2);
    assert_eq!(report.team_actor_messages, 1);
    assert_eq!(report.total_documents(), 3);
    let documents = archive.documents.lock().await.clone();
    assert_eq!(documents.len(), 3);
    assert!(
        documents
            .iter()
            .all(|document| document.body_text != "live tail run event")
    );
    assert!(
        documents
            .iter()
            .all(|document| document.body_text != "live tail actor message")
    );
    let stored_run_event_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_run_events WHERE run_id = ?1")
            .bind(&run.id)
            .fetch_one(&db)
            .await
            .expect("count run events");
    let stored_actor_message_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_actor_messages WHERE run_id = ?1")
            .bind(&run.id)
            .fetch_one(&db)
            .await
            .expect("count actor messages");
    assert_eq!(stored_run_event_count, 3);
    assert_eq!(stored_actor_message_count, 2);
}

#[test]
fn message_archive_body_text_does_not_index_structured_payload_fallback() {
    assert_eq!(
        message_archive_body_text(&json!({"type": "event", "metadata": {"id": "meta-1"}})),
        ""
    );
    assert_eq!(
        message_archive_body_text(&json!({"text": "  searchable text  "})),
        "searchable text"
    );
    assert_eq!(
        message_archive_body_text(&json!({"summary": "  searchable summary  "})),
        "searchable summary"
    );
}

#[tokio::test]
async fn append_task_conversation_message_does_not_wait_for_slow_archive() {
    let db = setup_test_db().await;
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(Arc::new(PendingMessageArchive)),
    );

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-slow-archive-team".to_string(),
            description: Some("team for slow archive dual-write".to_string()),
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

    let result = timeout(
        Duration::from_millis(200),
        manager.append_task_conversation_message_with_created(
            &task.id,
            "user",
            Some("coordinator"),
            "to_coordinator",
            json!({"type": "chat_message", "text": "archive should not block"}),
            Some("task-slow-archive-msg-1"),
        ),
    )
    .await
    .expect("conversation append should not wait for archive append")
    .expect("append message");

    assert!(result.1);
}
