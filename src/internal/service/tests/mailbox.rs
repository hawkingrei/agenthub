use super::*;

#[tokio::test]
async fn internal_grpc_time_trigger_rejects_past_fire_at() {
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
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let err = TeamInternalControl::create_time_trigger(
        &service,
        authenticated_request(
            CreateTimeTriggerRequest {
                actor_id: "reviewer".to_string(),
                message_text: "late trigger".to_string(),
                fire_at: chrono::Utc::now().timestamp() - 1,
            },
            &token,
        ),
    )
    .await
    .expect_err("past fire_at should be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);
    assert_eq!(err.message(), "fire_at must be in the future");
}

#[tokio::test]
async fn internal_grpc_time_trigger_rejects_unknown_agent() {
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
    let missing_actor_id = format!("missing-reviewer-{}", Uuid::new_v4());
    state
        .agents
        .get_agent(&missing_actor_id)
        .await
        .expect_err("missing actor should not be seeded in test state");
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Worker, Some(&missing_actor_id), None);
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let err = TeamInternalControl::create_time_trigger(
        &service,
        authenticated_request(
            CreateTimeTriggerRequest {
                actor_id: missing_actor_id,
                message_text: "missing agent".to_string(),
                fire_at: chrono::Utc::now().timestamp() + 120,
            },
            &token,
        ),
    )
    .await
    .expect_err("missing actor agent should fail");
    assert_eq!(err.code(), Code::NotFound);
    assert_eq!(err.message(), "agent not found");
}

#[tokio::test]
async fn internal_grpc_mailbox_send_list_ack_are_wire_compatible() {
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Coordinator, None, Some(&run.id));
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let send = TeamInternalControl::send_actor_message(
        &service,
        authenticated_request(
            SendActorMessageRequest {
                run_id: run.id.clone(),
                from_actor_id: "planner".to_string(),
                to_actor_id: "reviewer".to_string(),
                channel: "coordination".to_string(),
                transport: "local".to_string(),
                route_json: r#"{"topic":"review"}"#.to_string(),
                payload_json: r#"{"text":"please review"}"#.to_string(),
                idempotency_key: "internal-grpc-msg-1".to_string(),
                from_peer_id: "node-a".to_string(),
                to_peer_id: "main".to_string(),
                channel_id: String::new(),
                message_kind: String::new(),
            },
            &token,
        ),
    )
    .await
    .expect("send actor message")
    .into_inner();
    assert!(send.message_id > 0);
    assert_eq!(send.status, "pending");
    assert_eq!(send.idempotency_key, "internal-grpc-msg-1");
    let sent_message: crate::team::TeamActorMessageRecord =
        serde_json::from_str(&send.message_json).expect("decode sent message_json");
    assert_eq!(sent_message.message_id, send.message_id);
    assert_eq!(sent_message.to_actor_id, "reviewer");
    assert_eq!(sent_message.status, ActorMessageStatus::Pending);
    assert_eq!(
        sent_message.idempotency_key.as_deref(),
        Some("internal-grpc-msg-1")
    );

    let pending_inbox = TeamInternalControl::list_actor_inbox(
        &service,
        authenticated_request(
            ListActorInboxRequest {
                run_id: run.id.clone(),
                actor_id: "reviewer".to_string(),
                limit: 100,
                after_message_id: 0,
                include_delivered: false,
            },
            &token,
        ),
    )
    .await
    .expect("list pending inbox")
    .into_inner();
    assert_eq!(pending_inbox.messages.len(), 1);
    let pending = &pending_inbox.messages[0];
    assert_eq!(pending.message_id, send.message_id);
    assert_eq!(pending.run_id, run.id);
    assert_eq!(pending.from_actor_id, "planner");
    assert_eq!(pending.to_actor_id, "reviewer");
    assert_eq!(pending.channel, "coordination");
    assert_eq!(pending.transport, "local");
    assert_eq!(pending.route_json, r#"{"topic":"review"}"#);
    assert_eq!(pending.payload_json, r#"{"text":"please review"}"#);
    assert_eq!(pending.status, "pending");
    assert_eq!(pending.idempotency_key, "internal-grpc-msg-1");
    assert_eq!(pending.from_peer_id, "node-a");
    assert_eq!(pending.to_peer_id, "main");

    let acked = TeamInternalControl::ack_actor_message(
        &service,
        authenticated_request(
            AckActorMessageRequest {
                run_id: run.id.clone(),
                actor_id: "reviewer".to_string(),
                message_id: send.message_id,
            },
            &token,
        ),
    )
    .await
    .expect("ack actor message")
    .into_inner();
    assert!(acked.status_changed);
    let acked_message = acked.message.expect("acked message");
    assert_eq!(acked_message.message_id, send.message_id);
    assert_eq!(acked_message.status, "delivered");
    assert_eq!(acked_message.idempotency_key, "internal-grpc-msg-1");
    assert!(acked_message.delivered_at >= acked_message.created_at);
    assert_eq!(acked_message.from_peer_id, "node-a");
    assert_eq!(acked_message.to_peer_id, "main");

    let pending_after_ack = TeamInternalControl::list_actor_inbox(
        &service,
        authenticated_request(
            ListActorInboxRequest {
                run_id: run.id.clone(),
                actor_id: "reviewer".to_string(),
                limit: 100,
                after_message_id: 0,
                include_delivered: false,
            },
            &token,
        ),
    )
    .await
    .expect("list pending inbox after ack")
    .into_inner();
    assert!(pending_after_ack.messages.is_empty());

    let inbox_with_delivered = TeamInternalControl::list_actor_inbox(
        &service,
        authenticated_request(
            ListActorInboxRequest {
                run_id: run.id,
                actor_id: "reviewer".to_string(),
                limit: 100,
                after_message_id: 0,
                include_delivered: true,
            },
            &token,
        ),
    )
    .await
    .expect("list inbox including delivered")
    .into_inner();
    assert_eq!(inbox_with_delivered.messages.len(), 1);
    assert_eq!(inbox_with_delivered.messages[0].status, "delivered");
    assert_eq!(
        inbox_with_delivered.messages[0].idempotency_key,
        "internal-grpc-msg-1"
    );
}

#[tokio::test]
async fn internal_grpc_mailbox_triage_and_task_link_are_wire_compatible() {
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Coordinator, None, Some(&run.id));
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let send = TeamInternalControl::send_actor_message(
        &service,
        authenticated_request(
            SendActorMessageRequest {
                run_id: run.id.clone(),
                from_actor_id: "planner".to_string(),
                to_actor_id: "reviewer".to_string(),
                channel: "coordination".to_string(),
                transport: "local".to_string(),
                route_json: String::new(),
                payload_json:
                    r#"{"text":"investigate mailbox item","correlation_id":"wire-link-1"}"#
                        .to_string(),
                idempotency_key: "internal-grpc-triage-link-1".to_string(),
                from_peer_id: "main".to_string(),
                to_peer_id: "main".to_string(),
                channel_id: String::new(),
                message_kind: "coordination_request".to_string(),
            },
            &token,
        ),
    )
    .await
    .expect("send actor message")
    .into_inner();

    let triaged = TeamInternalControl::triage_actor_message(
        &service,
        authenticated_request(
            TriageActorMessageRequest {
                run_id: run.id.clone(),
                actor_id: "reviewer".to_string(),
                message_id: send.message_id,
                disposition: "claimed".to_string(),
            },
            &token,
        ),
    )
    .await
    .expect("triage actor message")
    .into_inner();
    assert!(triaged.handling_changed);
    let triaged_message = triaged.message.expect("triaged message");
    assert_eq!(triaged_message.message_id, send.message_id);
    assert_eq!(
        triaged_message.idempotency_key,
        "internal-grpc-triage-link-1"
    );
    assert_eq!(triaged_message.handling_disposition, "claimed");
    assert_eq!(triaged_message.thread_claim_status, "claimed");
    assert_eq!(triaged_message.thread_owner_actor_id, "reviewer");
    assert_eq!(triaged_message.thread_topic_key, "correlation:wire-link-1");
    assert!(triaged_message.thread_lease_expires_at >= triaged_message.created_at);

    let created = TeamInternalControl::create_team_task(
        &service,
        authenticated_request(
            CreateTeamTaskRequest {
                team_id: run.team_id.clone(),
                actor_id: "planner".to_string(),
                title: "Investigate linked mailbox message".to_string(),
                status: "open".to_string(),
                topic: "mailbox".to_string(),
                context_json: "{}".to_string(),
                priority: "medium".to_string(),
                assigned_member_id: "reviewer".to_string(),
            },
            &token,
        ),
    )
    .await
    .expect("create team task")
    .into_inner();
    let created_json: serde_json::Value =
        serde_json::from_str(&created.output_json).expect("decode created task output");
    let task_id = created_json["task"]["id"]
        .as_str()
        .expect("created task id")
        .to_string();

    let linked = TeamInternalControl::link_actor_message_task(
        &service,
        authenticated_request(
            LinkActorMessageTaskRequest {
                run_id: run.id.clone(),
                actor_id: "reviewer".to_string(),
                message_id: send.message_id,
                task_id: task_id.clone(),
                relation: "related_task".to_string(),
            },
            &token,
        ),
    )
    .await
    .expect("link actor message task")
    .into_inner();
    assert!(linked.created);
    assert_eq!(linked.task_id, task_id);
    assert_eq!(linked.relation, "related_task");
    let linked_message = linked.message.expect("linked message");
    assert_eq!(linked_message.message_id, send.message_id);
    assert_eq!(linked_message.linked_task_id, task_id);
    assert_eq!(linked_message.linked_task_relation, "related_task");
    assert_eq!(linked_message.thread_claim_status, "claimed");
    assert_eq!(linked_message.thread_owner_actor_id, "reviewer");
    assert_eq!(linked_message.thread_topic_key, "correlation:wire-link-1");
}

#[tokio::test]
async fn internal_grpc_mailbox_send_persists_channel_replica_history() {
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let (task_id, conversation_id) = state
        .teams
        .ensure_shared_thread_target_for_team(&run.team_id, "planner")
        .await
        .expect("ensure shared thread target");
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Coordinator, None, Some(&run.id));
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let authority_message_id = sqlx::query(
        r#"
        INSERT INTO team_conversation_messages (
            conversation_id,
            task_id,
            from_actor_id,
            to_actor_id,
            route,
            payload_json,
            idempotency_key,
            created_at
        )
        VALUES (?1, ?2, ?3, NULL, 'broadcast', ?4, NULL, ?5)
        "#,
    )
    .bind(&conversation_id)
    .bind(&task_id)
    .bind("planner")
    .bind(
        json!({
            "type": "chat_message",
            "text": "@reviewer please inspect p2p relay",
            "correlation_id": "corr-internal-grpc-replica-1"
        })
        .to_string(),
    )
    .bind(chrono::Utc::now().timestamp())
    .execute(&state.db)
    .await
    .expect("insert authority conversation message")
    .last_insert_rowid();
    let send = TeamInternalControl::send_actor_message(
        &service,
        authenticated_request(
            SendActorMessageRequest {
                run_id: run.id.clone(),
                from_actor_id: "planner".to_string(),
                to_actor_id: "reviewer".to_string(),
                channel: "coordination".to_string(),
                transport: "local".to_string(),
                route_json: String::new(),
                payload_json: json!({
                    "type": "chat_message",
                    "text": "@reviewer please inspect p2p relay",
                    "delivery_scope": "channel_broadcast",
                    "authority_message_id": authority_message_id,
                    "correlation_id": "corr-internal-grpc-replica-1",
                    "team_id": run.team_id,
                    "channel_conversation_id": conversation_id,
                    "task_id": task_id,
                    "channel_id": "all",
                    "mention_actor_ids": ["reviewer"],
                    "mentioned_actor_ids": ["reviewer"]
                })
                .to_string(),
                idempotency_key: "internal-grpc-channel-replica-1".to_string(),
                from_peer_id: "main".to_string(),
                to_peer_id: "main".to_string(),
                channel_id: String::new(),
                message_kind: String::new(),
            },
            &token,
        ),
    )
    .await
    .expect("send actor channel replica message")
    .into_inner();
    assert!(send.message_id > 0);

    let replica = sqlx::query(
            r#"
            SELECT authority_message_id, correlation_id, group_id, run_id, team_id, conversation_id, task_id, channel_id, from_actor_id, source_node_id, payload_json
            FROM team_channel_message_replicas
            WHERE authority_message_id = ?1
            "#,
        )
        .bind(authority_message_id)
        .fetch_one(&state.db)
        .await
        .expect("load channel replica row");
    assert_eq!(
        replica.get::<i64, _>("authority_message_id"),
        authority_message_id
    );
    assert_eq!(
        replica.get::<String, _>("correlation_id"),
        "corr-internal-grpc-replica-1"
    );
    assert_eq!(
        replica.get::<Option<String>, _>("group_id"),
        Some("internal-grpc-mailbox-group".to_string())
    );
    assert_eq!(replica.get::<String, _>("run_id"), run.id);
    assert_eq!(replica.get::<String, _>("channel_id"), "all");
    assert_eq!(replica.get::<String, _>("from_actor_id"), "planner");
    assert_eq!(replica.get::<String, _>("source_node_id"), "main");
    let payload: serde_json::Value =
        serde_json::from_str(replica.get::<String, _>("payload_json").as_str())
            .expect("decode replica payload");
    assert_eq!(payload["delivery_scope"], json!("channel_broadcast"));
    assert_eq!(
        payload["correlation_id"],
        json!("corr-internal-grpc-replica-1")
    );
    assert_eq!(payload["mention_actor_ids"], json!(["reviewer"]));
}

#[tokio::test]
async fn internal_grpc_mailbox_send_rejects_channel_replica_payload_with_unknown_authority_message()
{
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let (task_id, conversation_id) = state
        .teams
        .ensure_shared_thread_target_for_team(&run.team_id, "planner")
        .await
        .expect("ensure shared thread target");
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Coordinator, None, Some(&run.id));
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let err = TeamInternalControl::send_actor_message(
        &service,
        authenticated_request(
            SendActorMessageRequest {
                run_id: run.id,
                from_actor_id: "planner".to_string(),
                to_actor_id: "reviewer".to_string(),
                channel: "coordination".to_string(),
                transport: "local".to_string(),
                route_json: String::new(),
                payload_json: json!({
                    "type": "chat_message",
                    "text": "@reviewer please inspect p2p relay",
                    "delivery_scope": "channel_broadcast",
                    "authority_message_id": 999_i64,
                    "correlation_id": "corr-internal-grpc-unknown-authority-1",
                    "team_id": run.team_id,
                    "channel_conversation_id": conversation_id,
                    "task_id": task_id,
                    "channel_id": "all"
                })
                .to_string(),
                idempotency_key: "internal-grpc-channel-replica-unknown-authority".to_string(),
                from_peer_id: "main".to_string(),
                to_peer_id: "main".to_string(),
                channel_id: String::new(),
                message_kind: String::new(),
            },
            &token,
        ),
    )
    .await
    .expect_err("unknown authority message should fail");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
    assert!(
        err.message().contains(
            "channel replica payload authority_message_id does not match canonical conversation context"
        ),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn internal_grpc_mailbox_send_rejects_channel_replica_payload_with_mismatched_correlation_id()
{
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let (task_id, conversation_id) = state
        .teams
        .ensure_shared_thread_target_for_team(&run.team_id, "planner")
        .await
        .expect("ensure shared thread target");
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Coordinator, None, Some(&run.id));
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let authority_message_id = sqlx::query(
        r#"
        INSERT INTO team_conversation_messages (
            conversation_id,
            task_id,
            from_actor_id,
            to_actor_id,
            route,
            payload_json,
            idempotency_key,
            created_at
        )
        VALUES (?1, ?2, ?3, NULL, 'broadcast', ?4, NULL, ?5)
        "#,
    )
    .bind(&conversation_id)
    .bind(&task_id)
    .bind("planner")
    .bind(
        json!({
            "type": "chat_message",
            "text": "@reviewer please inspect p2p relay",
            "correlation_id": "corr-authority-1"
        })
        .to_string(),
    )
    .bind(chrono::Utc::now().timestamp())
    .execute(&state.db)
    .await
    .expect("insert authority conversation message")
    .last_insert_rowid();

    let err = TeamInternalControl::send_actor_message(
        &service,
        authenticated_request(
            SendActorMessageRequest {
                run_id: run.id,
                from_actor_id: "planner".to_string(),
                to_actor_id: "reviewer".to_string(),
                channel: "coordination".to_string(),
                transport: "local".to_string(),
                route_json: String::new(),
                payload_json: json!({
                    "type": "chat_message",
                    "text": "@reviewer please inspect p2p relay",
                    "delivery_scope": "channel_broadcast",
                    "authority_message_id": authority_message_id,
                    "correlation_id": "corr-relay-1",
                    "team_id": run.team_id,
                    "channel_conversation_id": conversation_id,
                    "task_id": task_id,
                    "channel_id": "all"
                })
                .to_string(),
                idempotency_key: "internal-grpc-channel-replica-bad-correlation".to_string(),
                from_peer_id: "main".to_string(),
                to_peer_id: "main".to_string(),
                channel_id: String::new(),
                message_kind: String::new(),
            },
            &token,
        ),
    )
    .await
    .expect_err("mismatched correlation_id should fail");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
    assert!(
        err.message().contains(
            "channel replica payload correlation_id does not match canonical authority message"
        ),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn internal_grpc_mailbox_send_rejects_channel_replica_payload_with_mismatched_sender() {
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let (task_id, conversation_id) = state
        .teams
        .ensure_shared_thread_target_for_team(&run.team_id, "planner")
        .await
        .expect("ensure shared thread target");
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Coordinator, None, Some(&run.id));
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let authority_message_id = sqlx::query(
        r#"
        INSERT INTO team_conversation_messages (
            conversation_id,
            task_id,
            from_actor_id,
            to_actor_id,
            route,
            payload_json,
            idempotency_key,
            created_at
        )
        VALUES (?1, ?2, ?3, NULL, 'broadcast', ?4, NULL, ?5)
        "#,
    )
    .bind(&conversation_id)
    .bind(&task_id)
    .bind("planner")
    .bind(
        json!({
            "type": "chat_message",
            "text": "@reviewer please inspect p2p relay",
            "correlation_id": "corr-sender-1"
        })
        .to_string(),
    )
    .bind(chrono::Utc::now().timestamp())
    .execute(&state.db)
    .await
    .expect("insert authority conversation message")
    .last_insert_rowid();

    let err = TeamInternalControl::send_actor_message(
        &service,
        authenticated_request(
            SendActorMessageRequest {
                run_id: run.id,
                from_actor_id: "reviewer".to_string(),
                to_actor_id: "planner".to_string(),
                channel: "coordination".to_string(),
                transport: "local".to_string(),
                route_json: String::new(),
                payload_json: json!({
                    "type": "chat_message",
                    "text": "@planner please inspect p2p relay",
                    "delivery_scope": "channel_broadcast",
                    "authority_message_id": authority_message_id,
                    "correlation_id": "corr-sender-1",
                    "team_id": run.team_id,
                    "channel_conversation_id": conversation_id,
                    "task_id": task_id,
                    "channel_id": "all"
                })
                .to_string(),
                idempotency_key: "internal-grpc-channel-replica-bad-sender".to_string(),
                from_peer_id: "main".to_string(),
                to_peer_id: "main".to_string(),
                channel_id: String::new(),
                message_kind: String::new(),
            },
            &token,
        ),
    )
    .await
    .expect_err("mismatched sender should fail");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
    assert!(
        err.message()
            .contains("channel replica payload sender does not match canonical authority message"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn internal_grpc_mailbox_send_rejects_mismatched_channel_replica_context() {
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Coordinator, None, Some(&run.id));
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let err = TeamInternalControl::send_actor_message(
        &service,
        authenticated_request(
            SendActorMessageRequest {
                run_id: run.id,
                from_actor_id: "planner".to_string(),
                to_actor_id: "reviewer".to_string(),
                channel: "coordination".to_string(),
                transport: "local".to_string(),
                route_json: String::new(),
                payload_json: json!({
                    "type": "chat_message",
                    "text": "@reviewer please inspect p2p relay",
                    "delivery_scope": "channel_broadcast",
                    "authority_message_id": 999_i64,
                    "correlation_id": "corr-internal-grpc-bad-context-1",
                    "team_id": "wrong-team",
                    "channel_conversation_id": "conversation-all",
                    "task_id": "task-all",
                    "channel_id": "all"
                })
                .to_string(),
                idempotency_key: "internal-grpc-channel-replica-bad-1".to_string(),
                from_peer_id: "main".to_string(),
                to_peer_id: "main".to_string(),
                channel_id: String::new(),
                message_kind: String::new(),
            },
            &token,
        ),
    )
    .await
    .expect_err("mismatched replica payload should fail");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
    assert!(
        err.message()
            .contains("channel replica payload does not match run/team context"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn internal_grpc_mailbox_send_rejects_channel_replica_payload_without_correlation_id() {
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let (task_id, conversation_id) = state
        .teams
        .ensure_shared_thread_target_for_team(&run.team_id, "planner")
        .await
        .expect("ensure shared thread target");
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Coordinator, None, Some(&run.id));
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let err = TeamInternalControl::send_actor_message(
        &service,
        authenticated_request(
            SendActorMessageRequest {
                run_id: run.id,
                from_actor_id: "planner".to_string(),
                to_actor_id: "reviewer".to_string(),
                channel: "coordination".to_string(),
                transport: "local".to_string(),
                route_json: String::new(),
                payload_json: json!({
                    "type": "chat_message",
                    "text": "@reviewer please inspect p2p relay",
                    "delivery_scope": "channel_broadcast",
                    "authority_message_id": 999_i64,
                    "team_id": run.team_id,
                    "channel_conversation_id": conversation_id,
                    "task_id": task_id,
                    "channel_id": "all"
                })
                .to_string(),
                idempotency_key: "internal-grpc-channel-replica-missing-correlation".to_string(),
                from_peer_id: "main".to_string(),
                to_peer_id: "main".to_string(),
                channel_id: String::new(),
                message_kind: String::new(),
            },
            &token,
        ),
    )
    .await
    .expect_err("missing correlation id should fail");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
    assert!(
        err.message()
            .contains("payload_json must represent a valid channel_broadcast replica payload"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn ack_actor_message_rejects_non_positive_message_id() {
    let state = build_test_state().await;
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Coordinator, None, None);
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let err = TeamInternalControl::ack_actor_message(
        &service,
        authenticated_request(
            AckActorMessageRequest {
                run_id: "run-id".to_string(),
                actor_id: "actor-id".to_string(),
                message_id: 0,
            },
            &token,
        ),
    )
    .await
    .expect_err("message_id <= 0 should fail");
    assert_eq!(err.code(), Code::InvalidArgument);
    assert_eq!(err.message(), "message_id must be positive");
}
