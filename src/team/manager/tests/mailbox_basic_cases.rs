use super::*;

#[tokio::test]
async fn actor_messages_support_inbox_and_ack_flow() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-message-inbox-team".to_string(),
            description: Some("team for actor inbox".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-msg-inbox"), json!({"payload":"start"}))
        .await
        .expect("create run");

    let sent = manager
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
        .expect("send actor message");
    assert_eq!(sent.status, TeamActorMessageStatus::Pending);

    let unread = manager
        .list_actor_inbox(&run.id, "reviewer", 50, None, false)
        .await
        .expect("list inbox");
    assert_eq!(unread.len(), 1);
    assert_eq!(unread[0].message_id, sent.message_id);
    assert_eq!(unread[0].status, TeamActorMessageStatus::Pending);

    let ack = manager
        .ack_actor_message(&run.id, "reviewer", sent.message_id)
        .await
        .expect("ack message");
    assert!(ack.status_changed);
    assert_eq!(ack.message.status, TeamActorMessageStatus::Delivered);

    let unread_after_ack = manager
        .list_actor_inbox(&run.id, "reviewer", 50, None, false)
        .await
        .expect("list inbox after ack");
    assert!(unread_after_ack.is_empty());

    let history = manager
        .list_actor_inbox(&run.id, "reviewer", 50, None, true)
        .await
        .expect("list inbox history");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, TeamActorMessageStatus::Delivered);
}

#[tokio::test]
async fn actor_messages_persist_canonical_inbound_envelope_for_text_payload() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-message-envelope-normalization-team".to_string(),
            description: Some("team for actor message envelope normalization".to_string()),
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
            Some("ctx-msg-envelope"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let sent = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!("please review"),
            idempotency_key: Some("msg-envelope-1"),
            message_kind: None,
        })
        .await
        .expect("send actor message");

    assert_eq!(sent.payload["type"], json!("chat_message"));
    assert_eq!(sent.payload["text"], json!("please review"));
    assert_eq!(sent.payload["source_kind"], json!("agent"));
    assert_eq!(sent.payload["source_surface"], json!("mailbox"));
    assert_eq!(sent.payload["requires_user_visible_reply"], json!(false));

    let stored_payload_json: String =
        sqlx::query_scalar("SELECT payload_json FROM team_actor_messages WHERE id = ?1")
            .bind(sent.message_id)
            .fetch_one(&db)
            .await
            .expect("load stored payload");
    let stored_payload: Value =
        serde_json::from_str(&stored_payload_json).expect("decode stored payload");
    assert_eq!(stored_payload, sent.payload);
}

#[tokio::test]
async fn summarize_open_reply_obligations_prefers_lightweight_snapshot_loader() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "reply-obligation-summary-team".to_string(),
            description: Some("team for reply obligation summary".to_string()),
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
            Some("ctx-reply-obligation"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "user:alice",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"Need update",
                "requires_user_visible_reply":true
            }),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .expect("send inbound obligation");
    manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "reviewer",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "user:alice",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: Value::String(
                r#"{"type":"chat_message","text":"Here is the update","correlation_id":"corr-summary"}"#
                    .to_string(),
            ),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .expect("send visible reply");

    let summary = manager
        .summarize_open_reply_obligations(&run.id)
        .await
        .expect("summarize reply obligations");

    assert_eq!(summary.open_total, 0);
    assert!(summary.open_by_actor.is_empty());
    assert!(summary.open_items.is_empty());
}

#[tokio::test]
async fn actor_messages_detect_pending_payload_type_by_actor_inbox() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-message-payload-type-team".to_string(),
            description: Some("team for payload type pending lookup".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-msg-type"), json!({"payload":"start"}))
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
            payload: json!({"type":"chat_message","text":"first"}),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .expect("send first message");
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
            payload: json!({"type":"chat_message","text":"second"}),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .expect("send second message");
    let _other_type = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"type":"  worker_status  ","status":"done"}),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .expect("send other type message");

    let has_other_chat_pending = manager
        .has_pending_actor_message_payload_type(
            &run.id,
            "reviewer",
            "chat_message",
            Some(second.message_id),
        )
        .await
        .expect("check chat pending excluding latest");
    assert!(has_other_chat_pending);

    let has_prior_for_first_chat = manager
        .has_pending_actor_message_payload_type(
            &run.id,
            "reviewer",
            "chat_message",
            Some(first.message_id),
        )
        .await
        .expect("check chat pending before first");
    assert!(!has_prior_for_first_chat);

    manager
        .ack_actor_message(&run.id, "reviewer", first.message_id)
        .await
        .expect("ack first");
    let still_has_other_chat_pending = manager
        .has_pending_actor_message_payload_type(
            &run.id,
            "reviewer",
            "chat_message",
            Some(second.message_id),
        )
        .await
        .expect("check chat pending after ack");
    assert!(!still_has_other_chat_pending);

    let has_worker_status_pending = manager
        .has_pending_actor_message_payload_type(&run.id, "reviewer", "worker_status", None)
        .await
        .expect("check worker_status pending");
    assert!(has_worker_status_pending);
}

#[tokio::test]
async fn actor_ack_reports_noop_when_message_is_already_delivered() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-ack-noop-team".to_string(),
            description: Some("team for duplicate ack diagnostics".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-ack-noop"), json!({"payload":"start"}))
        .await
        .expect("create run");
    let sent = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"type":"chat_message","text":"please review"}),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .expect("send message");

    let first = manager
        .ack_actor_message(&run.id, "reviewer", sent.message_id)
        .await
        .expect("first ack");
    assert!(first.status_changed);
    assert_eq!(first.message.status, TeamActorMessageStatus::Delivered);

    let second = manager
        .ack_actor_message(&run.id, "reviewer", sent.message_id)
        .await
        .expect("second ack");
    assert!(!second.status_changed);
    assert_eq!(second.message.status, TeamActorMessageStatus::Delivered);
}

#[tokio::test]
async fn actor_mailbox_service_returns_contract_responses() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-service-team".to_string(),
            description: Some("team for actor mailbox service contract".to_string()),
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
            Some("ctx-msg-service"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let sent = service
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
            payload: json!({"text":"please review"}),
            idempotency_key: Some("msg-service-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("actor send");
    assert_eq!(sent.state, TeamActorMessageStatus::Pending);
    assert!(!sent.deduped);

    let deduped = service
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
            payload: json!({"text":"please review"}),
            idempotency_key: Some("msg-service-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("actor send deduped");
    assert_eq!(sent.message_id, deduped.message_id);
    assert!(deduped.deduped);

    let inbox = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(50),
            states: None,
        })
        .await
        .expect("actor inbox");
    assert_eq!(inbox.messages.len(), 1);
    assert_eq!(inbox.messages[0].message_id, sent.message_id);
    assert_eq!(inbox.next_cursor, Some(sent.message_id));
    assert_eq!(inbox.pending_count, 1);

    let acked = service
        .actor_ack(ActorAckRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            ack_token: None,
            result: None,
        })
        .await
        .expect("actor ack");
    assert_eq!(acked.message_id, sent.message_id);
    assert_eq!(acked.state, TeamActorMessageStatus::Delivered);
}

#[tokio::test]
async fn actor_mailbox_service_cursor_can_hide_page_messages_without_resetting_pending_count() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-cursor-team".to_string(),
            description: Some("team for actor inbox cursor semantics".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-msg-cursor"), json!({"payload":"start"}))
        .await
        .expect("create run");

    let sent = service
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
            payload: json!({"text":"please review"}),
            idempotency_key: Some("msg-cursor-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("actor send");

    let inbox = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            cursor: Some(sent.message_id),
            limit: Some(50),
            states: None,
        })
        .await
        .expect("actor inbox with cursor");
    assert!(inbox.messages.is_empty());
    assert_eq!(inbox.next_cursor, None);
    assert_eq!(inbox.pending_count, 1);
}

#[tokio::test]
async fn actor_mailbox_service_include_delivered_keeps_pending_visible_on_first_page() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-pending-first-team".to_string(),
            description: Some("team for delivered inbox pending-first behavior".to_string()),
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
            Some("ctx-msg-pending-first"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let mut latest_pending_id = None;
    for idx in 0..25 {
        let sent = service
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
                payload: json!({"text": format!("message-{idx}")}),
                idempotency_key: Some(format!("msg-pending-first-{idx}")),
                message_kind: None,
            })
            .await
            .expect("actor send");
        latest_pending_id = Some(sent.message_id);
        if idx < 24 {
            service
                .actor_ack(ActorAckRequest {
                    run_id: run.id.clone(),
                    actor_id: "reviewer".to_string(),
                    message_id: sent.message_id,
                    ack_token: None,
                    result: None,
                })
                .await
                .expect("ack historical message");
        }
    }

    let inbox = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id,
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![
                TeamActorMessageStatus::Pending,
                TeamActorMessageStatus::Delivered,
            ]),
        })
        .await
        .expect("actor inbox with delivered keeps unread visible");

    assert_eq!(inbox.pending_count, 1);
    assert_eq!(inbox.messages.len(), 1);
    assert_eq!(inbox.messages[0].status, TeamActorMessageStatus::Pending);
    assert_eq!(inbox.messages[0].message_id, latest_pending_id.unwrap());
}

#[tokio::test]
async fn actor_mailbox_service_include_delivered_returns_history_when_unread_is_empty() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-history-only-team".to_string(),
            description: Some("team for delivered-only inbox history".to_string()),
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
            Some("ctx-msg-history-only"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let mut first_delivered_id = None;
    for idx in 0..3 {
        let sent = service
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
                payload: json!({"text": format!("history-{idx}")}),
                idempotency_key: Some(format!("msg-history-only-{idx}")),
                message_kind: None,
            })
            .await
            .expect("actor send");
        if first_delivered_id.is_none() {
            first_delivered_id = Some(sent.message_id);
        }
        service
            .actor_ack(ActorAckRequest {
                run_id: run.id.clone(),
                actor_id: "reviewer".to_string(),
                message_id: sent.message_id,
                ack_token: None,
                result: None,
            })
            .await
            .expect("ack historical message");
    }

    let inbox = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id,
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![
                TeamActorMessageStatus::Pending,
                TeamActorMessageStatus::Delivered,
            ]),
        })
        .await
        .expect("actor inbox with delivered history only");

    assert_eq!(inbox.pending_count, 0);
    assert_eq!(inbox.messages.len(), 3);
    assert!(
        inbox
            .messages
            .iter()
            .all(|message| message.status == TeamActorMessageStatus::Delivered)
    );
    assert_eq!(inbox.messages[0].message_id, first_delivered_id.unwrap());
}

#[tokio::test]
async fn actor_mailbox_service_include_delivered_preserves_requested_mix_when_page_has_pending() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-mixed-first-page-team".to_string(),
            description: Some("team for delivered inbox mixed first page".to_string()),
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
            Some("ctx-msg-mixed-first-page"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    for idx in 0..3 {
        let sent = service
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
                payload: json!({"text": format!("mixed-{idx}")}),
                idempotency_key: Some(format!("msg-mixed-first-page-{idx}")),
                message_kind: None,
            })
            .await
            .expect("actor send");
        if idx < 2 {
            service
                .actor_ack(ActorAckRequest {
                    run_id: run.id.clone(),
                    actor_id: "reviewer".to_string(),
                    message_id: sent.message_id,
                    ack_token: None,
                    result: None,
                })
                .await
                .expect("ack historical message");
        }
    }

    let inbox = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id,
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![
                TeamActorMessageStatus::Pending,
                TeamActorMessageStatus::Delivered,
            ]),
        })
        .await
        .expect("actor inbox with delivered mixed page");

    assert_eq!(inbox.pending_count, 1);
    assert_eq!(inbox.messages.len(), 3);
    assert_eq!(inbox.messages[0].status, TeamActorMessageStatus::Delivered);
    assert_eq!(inbox.messages[1].status, TeamActorMessageStatus::Delivered);
    assert_eq!(inbox.messages[2].status, TeamActorMessageStatus::Pending);
}

#[tokio::test]
async fn actor_mailbox_service_triage_hides_watching_messages_from_unread_snapshot() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-triage-team".to_string(),
            description: Some("team for mailbox triage unread semantics".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-msg-triage"), json!({"payload":"start"}))
        .await
        .expect("create run");

    let sent = service
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
            payload: json!({"text":"observe this request"}),
            idempotency_key: Some("msg-triage-watch-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("actor send");

    let triaged = service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            disposition: ActorMessageHandlingDisposition::Watching,
            reason: None,
        })
        .await
        .expect("triage watching");
    assert_eq!(
        triaged.disposition,
        ActorMessageHandlingDisposition::Watching
    );
    assert!(triaged.handling_changed);
    assert_eq!(
        triaged.message.handled_by_actor_id.as_deref(),
        Some("reviewer")
    );

    let unread = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: None,
        })
        .await
        .expect("unread inbox after watch triage");
    assert_eq!(unread.pending_count, 0);
    assert!(unread.messages.is_empty());

    let with_history = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id,
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![
                TeamActorMessageStatus::Pending,
                TeamActorMessageStatus::Delivered,
            ]),
        })
        .await
        .expect("history inbox after watch triage");
    assert_eq!(with_history.messages.len(), 1);
    assert_eq!(
        with_history.messages[0].handling_disposition,
        ActorMessageHandlingDisposition::Watching
    );
}

#[tokio::test]
async fn actor_mailbox_service_claims_topics_and_prevents_parallel_takeover() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-claim-team".to_string(),
            description: Some("team for mailbox claim semantics".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"},{"member_id":"observer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-msg-claim"), json!({"payload":"start"}))
        .await
        .expect("create run");

    let sent = service
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
                "text":"investigate incident",
                "topic":"incident/123"
            }),
            idempotency_key: Some("msg-claim-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("actor send");

    let claimed = service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            disposition: ActorMessageHandlingDisposition::Claimed,
            reason: None,
        })
        .await
        .expect("claim topic");
    assert_eq!(
        claimed.disposition,
        ActorMessageHandlingDisposition::Claimed
    );
    assert!(claimed.handling_changed);

    let conflict = service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "observer".to_string(),
            message_id: sent.message_id,
            disposition: ActorMessageHandlingDisposition::Claimed,
            reason: None,
        })
        .await
        .expect_err("parallel claim should fail");
    assert_eq!(conflict.code, ActorServiceErrorCode::Conflict);

    let history = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id,
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![
                TeamActorMessageStatus::Pending,
                TeamActorMessageStatus::Delivered,
            ]),
        })
        .await
        .expect("claim history");
    assert_eq!(history.messages.len(), 1);
    assert_eq!(
        history.messages[0].handling_disposition,
        ActorMessageHandlingDisposition::Claimed
    );
}

#[tokio::test]
async fn actor_mailbox_service_requires_active_owner_for_release_and_complete() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-owner-team".to_string(),
            description: Some("team for mailbox ownership semantics".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"},{"member_id":"observer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-msg-owner"), json!({"payload":"start"}))
        .await
        .expect("create run");

    let sent = service
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
                "text":"investigate incident",
                "topic":"incident/owner"
            }),
            idempotency_key: Some("msg-owner-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("actor send");

    service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            disposition: ActorMessageHandlingDisposition::Claimed,
            reason: None,
        })
        .await
        .expect("claim message");

    let release_err = service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "observer".to_string(),
            message_id: sent.message_id,
            disposition: ActorMessageHandlingDisposition::Watching,
            reason: None,
        })
        .await
        .expect_err("non-owner release should fail");
    assert_eq!(release_err.code, ActorServiceErrorCode::Conflict);

    let complete_err = service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "observer".to_string(),
            message_id: sent.message_id,
            disposition: ActorMessageHandlingDisposition::Completed,
            reason: None,
        })
        .await
        .expect_err("non-owner complete should fail");
    assert_eq!(complete_err.code, ActorServiceErrorCode::Conflict);
}

#[tokio::test]
async fn actor_mailbox_service_rejects_complete_after_terminal_ignore_without_visible_reply() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-terminal-ignore-complete-team".to_string(),
            description: Some("team for terminal ignore completion guard".to_string()),
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
            Some("ctx-msg-terminal-ignore-complete"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let sent = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "user:alice",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"Need update",
                "requires_user_visible_reply":true
            }),
            idempotency_key: Some("msg-terminal-ignore-complete-1"),
            message_kind: None,
        })
        .await
        .expect("send inbound obligation");

    service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            disposition: ActorMessageHandlingDisposition::Ignored,
            reason: Some("duplicate request".to_string()),
        })
        .await
        .expect("ignore with reason");

    let complete_err = service
        .actor_triage(ActorTriageRequest {
            run_id: run.id,
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            disposition: ActorMessageHandlingDisposition::Completed,
            reason: None,
        })
        .await
        .expect_err("terminal ignored work should still require visible reply evidence");
    assert_eq!(
        complete_err.code,
        ActorServiceErrorCode::UnprocessableEntity
    );
}

#[tokio::test]
async fn actor_mailbox_service_allows_complete_after_terminal_ignore_with_visible_reply() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-terminal-ignore-visible-reply-team".to_string(),
            description: Some("team for terminal ignore with visible reply".to_string()),
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
            Some("ctx-msg-terminal-ignore-visible-reply"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let sent = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "user:alice",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"Need update",
                "requires_user_visible_reply":true
            }),
            idempotency_key: Some("msg-terminal-ignore-visible-reply-1"),
            message_kind: None,
        })
        .await
        .expect("send inbound obligation");

    service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            disposition: ActorMessageHandlingDisposition::Ignored,
            reason: Some("duplicate request".to_string()),
        })
        .await
        .expect("ignore with reason");

    manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "reviewer",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "user:alice",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"Here is the update"
            }),
            idempotency_key: Some("msg-terminal-ignore-visible-reply-2"),
            message_kind: None,
        })
        .await
        .expect("send visible reply");

    let completed = service
        .actor_triage(ActorTriageRequest {
            run_id: run.id,
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            disposition: ActorMessageHandlingDisposition::Completed,
            reason: None,
        })
        .await
        .expect("visible reply should allow completion");
    assert_eq!(
        completed.disposition,
        ActorMessageHandlingDisposition::Completed
    );
}

#[tokio::test]
async fn actor_mailbox_service_completed_claim_remains_visible_in_history() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-completed-history-team".to_string(),
            description: Some("team for completed claim history".to_string()),
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
            Some("ctx-msg-completed-history"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let sent = service
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
                "text":"resolve incident",
                "topic":"incident/completed"
            }),
            idempotency_key: Some("msg-completed-history-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("actor send");

    service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            disposition: ActorMessageHandlingDisposition::Claimed,
            reason: None,
        })
        .await
        .expect("claim message");
    service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            disposition: ActorMessageHandlingDisposition::Completed,
            reason: None,
        })
        .await
        .expect("complete claim");

    let history = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id,
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![
                TeamActorMessageStatus::Pending,
                TeamActorMessageStatus::Delivered,
            ]),
        })
        .await
        .expect("history after complete");
    assert_eq!(history.messages.len(), 1);
    assert_eq!(
        history.messages[0].handling_disposition,
        ActorMessageHandlingDisposition::Completed
    );
}

#[tokio::test]
async fn actor_mailbox_service_task_link_surfaces_durable_task_association() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-task-link-team".to_string(),
            description: Some("team for task-link mailbox semantics".to_string()),
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
            Some("ctx-msg-task-link"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");
    let (task, _) = manager
        .create_task(
            &team.id,
            "Linked task",
            "planner",
            json!({"source":"mailbox"}),
            "group_chat",
            Some("mailbox-linked-task"),
        )
        .await
        .expect("create linked task");

    let sent = service
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
            payload: json!({"text":"please work on linked task"}),
            idempotency_key: Some("msg-task-link-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("actor send");

    let linked = service
        .actor_task_link(ActorTaskLinkRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            relation: ActorMessageTaskRelation::RelatedTask,
            task_id: task.id.clone(),
        })
        .await
        .expect("link task");
    assert_eq!(linked.task_id, task.id);

    let history = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id,
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![
                TeamActorMessageStatus::Pending,
                TeamActorMessageStatus::Delivered,
            ]),
        })
        .await
        .expect("linked task history");
    assert_eq!(history.messages.len(), 1);
    assert_eq!(
        history.messages[0].linked_task_id.as_deref(),
        Some(task.id.as_str())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_transfers_of_same_reply_required_message_do_not_both_apply() {
    // A real file-backed WAL pool so the two reassignment transactions actually interleave (the
    // shared :memory: pool serializes everything).
    let (db, dir) = setup_concurrent_mailbox_db().await;

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

    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "reassign-race-team".to_string(),
            description: Some("team for concurrent reassignment races".to_string()),
            spec: json!({
                "entrypoint":"worker-1",
                "members":[
                    {"member_id":"worker-1","role":"worker"},
                    {"member_id":"worker-2","role":"worker"},
                    {"member_id":"worker-3","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-reassign-race"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    for iteration in 0..60 {
        let payload_json = json!({
            "type":"chat_message",
            "text":"please handle this",
            "source_kind":"human",
            "source_surface":"conversation",
            "requires_user_visible_reply": true,
        })
        .to_string();
        let message_id = sqlx::query(
            r#"
            INSERT INTO team_actor_messages (
                run_id,
                from_actor_id,
                to_actor_id,
                channel,
                transport,
                route_json,
                payload_json,
                idempotency_key,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL, 'pending', ?7)
            "#,
        )
        .bind(&run.id)
        .bind("user")
        .bind("worker-1")
        .bind("default")
        .bind("local")
        .bind(&payload_json)
        .bind(Utc::now().timestamp())
        .execute(&db)
        .await
        .expect("insert human reply-required message")
        .last_insert_rowid();

        let service_a = manager.actor_mailbox_service();
        let service_b = manager.actor_mailbox_service();
        let run_id_for_a = run.id.clone();
        let run_id_for_b = run.id.clone();
        let (result_a, result_b) = tokio::join!(
            tokio::spawn(async move {
                service_a
                    .transfer_reply_required_message(
                        &run_id_for_a,
                        "worker-1",
                        ACTOR_MAIN_PEER_ID,
                        message_id,
                        "worker-2",
                    )
                    .await
            }),
            tokio::spawn(async move {
                service_b
                    .transfer_reply_required_message(
                        &run_id_for_b,
                        "worker-1",
                        ACTOR_MAIN_PEER_ID,
                        message_id,
                        "worker-3",
                    )
                    .await
            })
        );
        let result_a = result_a.expect("transfer-to-worker-2 task did not panic");
        let result_b = result_b.expect("transfer-to-worker-3 task did not panic");

        // A losing side is rejected one of three legitimate ways: sequentially, by re-reading the
        // already-terminal message fresh (BadRequest, a pre-existing check unrelated to this fix);
        // concurrently, by the CAS guard on the release UPDATE finding the row already changed
        // (Conflict); or, if its transaction's snapshot went stale mid-flight under genuine WAL
        // concurrency, a raw SQLite busy/locked error (Internal, the codebase's existing, separately-
        // tested contract for that case -- empirically the one WAL's own snapshot-conflict detection
        // always produces here, ahead of the CAS guard, but the guard stays as explicit, self-
        // documenting defense-in-depth consistent with `triage_message_impl`). It must never be an
        // unrelated error, and it must never be a duplicate success.
        for result in [&result_a, &result_b] {
            if let Err(err) = result {
                assert!(
                    matches!(
                        err.code,
                        ActorServiceErrorCode::Conflict
                            | ActorServiceErrorCode::Internal
                            | ActorServiceErrorCode::BadRequest
                    ),
                    "iteration {iteration}: unexpected error code on losing transfer: {err:?}"
                );
            }
        }
        let successes = [result_a.is_ok(), result_b.is_ok()]
            .into_iter()
            .filter(|ok| *ok)
            .count();
        assert!(
            successes <= 1,
            "iteration {iteration}: at most one of two concurrent transfers of the same message should apply, got a={result_a:?} b={result_b:?}"
        );

        let reassigned_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM team_actor_messages
            WHERE run_id = ?1 AND idempotency_key = ?2
            "#,
        )
        .bind(&run.id)
        .bind(format!("mailbox-reassign:{message_id}"))
        .fetch_one(&db)
        .await
        .expect("count reassigned rows");
        assert_eq!(
            reassigned_count as usize, successes,
            "iteration {iteration}: the number of persisted reassigned rows must match the number of transfers that actually succeeded"
        );
    }
}

#[tokio::test]
async fn transfer_reply_required_message_reuses_existing_row_on_idempotency_key_collision() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "reassign-idempotency-team".to_string(),
            description: Some("team for reassignment idempotency fallback".to_string()),
            spec: json!({
                "entrypoint":"worker-1",
                "members":[
                    {"member_id":"worker-1","role":"worker"},
                    {"member_id":"worker-2","role":"worker"},
                    {"member_id":"worker-3","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-reassign-idempotency"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let payload_json = json!({
        "type":"chat_message",
        "text":"please handle this",
        "source_kind":"human",
        "source_surface":"conversation",
        "requires_user_visible_reply": true,
    })
    .to_string();
    let message_id = sqlx::query(
        r#"
        INSERT INTO team_actor_messages (
            run_id,
            from_actor_id,
            to_actor_id,
            channel,
            transport,
            route_json,
            payload_json,
            idempotency_key,
            status,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL, 'pending', ?7)
        "#,
    )
    .bind(&run.id)
    .bind("user")
    .bind("worker-1")
    .bind("default")
    .bind("local")
    .bind(&payload_json)
    .bind(Utc::now().timestamp())
    .execute(&db)
    .await
    .expect("insert human reply-required message")
    .last_insert_rowid();

    // Pre-seed a row occupying the idempotency key this reassignment would compute, standing in for
    // a duplicate insert that already landed from an earlier, concurrently-committed attempt. It is
    // addressed to a third target so it is unmistakably distinct from the transfer request below.
    let phantom_idempotency_key = format!("mailbox-reassign:{message_id}");
    let phantom_message_id = sqlx::query(
        r#"
        INSERT INTO team_actor_messages (
            run_id,
            from_actor_id,
            from_peer_id,
            to_actor_id,
            channel,
            transport,
            route_json,
            payload_json,
            idempotency_key,
            status,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, 'pending', ?9)
        "#,
    )
    .bind(&run.id)
    .bind("user")
    .bind(ACTOR_MAIN_PEER_ID)
    .bind("worker-3")
    .bind("default")
    .bind("local")
    .bind(&payload_json)
    .bind(&phantom_idempotency_key)
    .bind(Utc::now().timestamp())
    .execute(&db)
    .await
    .expect("insert phantom idempotency row")
    .last_insert_rowid();

    let service = manager.actor_mailbox_service();
    let transferred = service
        .transfer_reply_required_message(
            &run.id,
            "worker-1",
            ACTOR_MAIN_PEER_ID,
            message_id,
            "worker-2",
        )
        .await
        .expect("transfer reuses the pre-existing idempotency row instead of erroring");
    assert_eq!(
        transferred.message_id, phantom_message_id,
        "transfer should resolve to the pre-existing row occupying the idempotency key, not insert a new one"
    );
    assert_eq!(
        transferred.to_actor_id, "worker-3",
        "the reused row keeps its own original recipient, not the requested transfer target"
    );

    let row_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_actor_messages
        WHERE run_id = ?1 AND idempotency_key = ?2
        "#,
    )
    .bind(&run.id)
    .bind(&phantom_idempotency_key)
    .fetch_one(&db)
    .await
    .expect("count rows for idempotency key");
    assert_eq!(
        row_count, 1,
        "no second row should be inserted when the idempotency key already exists"
    );
}
