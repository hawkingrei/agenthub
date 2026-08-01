use super::*;

#[tokio::test]
async fn actor_mailbox_service_direct_remote_send_requires_relay_route_and_remote_peer() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-direct-remote-validation-team".to_string(),
            description: Some("team for direct remote mailbox validation".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner"},
                    {"member_id":"reviewer"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-direct-remote-validation"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let missing_route = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            to_actor_id: Some("remote-reviewer".to_string()),
            channel_id: None,
            to_peer_id: Some(ACTOR_NODE_PEER_ID.to_string()),
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Remote),
            route: None,
            payload: json!({"text":"please review remotely"}),
            idempotency_key: Some("msg-direct-remote-missing-route".to_string()),
            message_kind: None,
        })
        .await
        .expect_err("remote direct send without route should fail");
    assert_eq!(missing_route.code, ActorServiceErrorCode::BadRequest);
    assert_eq!(
        missing_route.message,
        "route is required for remote transport"
    );

    let null_route = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            to_actor_id: Some("remote-reviewer".to_string()),
            channel_id: None,
            to_peer_id: Some(ACTOR_NODE_PEER_ID.to_string()),
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Remote),
            route: Some(Value::Null),
            payload: json!({"text":"please review remotely"}),
            idempotency_key: Some("msg-direct-remote-null-route".to_string()),
            message_kind: None,
        })
        .await
        .expect_err("remote direct send with null route should fail");
    assert_eq!(null_route.code, ActorServiceErrorCode::BadRequest);
    assert_eq!(
        null_route.message,
        "route must be a JSON object for remote transport"
    );

    let empty_route = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            to_actor_id: Some("remote-reviewer".to_string()),
            channel_id: None,
            to_peer_id: Some(ACTOR_NODE_PEER_ID.to_string()),
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Remote),
            route: Some(json!({})),
            payload: json!({"text":"please review remotely"}),
            idempotency_key: Some("msg-direct-remote-empty-route".to_string()),
            message_kind: None,
        })
        .await
        .expect_err("remote direct send with empty route should fail");
    assert_eq!(empty_route.code, ActorServiceErrorCode::BadRequest);
    assert_eq!(
        empty_route.message,
        "route must contain endpoint or grpc_target for remote transport"
    );

    let main_peer = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            to_actor_id: Some("remote-reviewer".to_string()),
            channel_id: None,
            to_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Remote),
            route: Some(json!({"endpoint":"https://remote.example/mailbox"})),
            payload: json!({"text":"please review remotely"}),
            idempotency_key: Some("msg-direct-remote-main-peer".to_string()),
            message_kind: None,
        })
        .await
        .expect_err("remote direct send to main peer should fail");
    assert_eq!(main_peer.code, ActorServiceErrorCode::BadRequest);
    assert_eq!(
        main_peer.message,
        "to_peer_id must not be 'main' for remote transport"
    );

    let valid = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            to_actor_id: Some("remote-reviewer".to_string()),
            channel_id: None,
            to_peer_id: Some(ACTOR_NODE_PEER_ID.to_string()),
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Remote),
            route: Some(json!({"endpoint":"https://remote.example/mailbox"})),
            payload: json!({"text":"please review remotely"}),
            idempotency_key: Some("msg-direct-remote-valid".to_string()),
            message_kind: None,
        })
        .await
        .expect("valid remote direct send");
    assert_eq!(valid.state, TeamActorMessageStatus::Pending);
    assert_eq!(valid.message.to_peer_id, ACTOR_NODE_PEER_ID);
    assert_eq!(valid.message.transport, TeamActorMessageTransport::Remote);

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
    .expect("count persisted mailbox rows");
    assert_eq!(mailbox_count, 1);
}

#[tokio::test]
async fn actor_mailbox_service_channel_send_auto_routes_remote_recipients_over_p2p() {
    let db = setup_test_db().await;
    sqlx::query("ALTER TABLE agents ADD COLUMN target_node_id TEXT")
        .execute(&db)
        .await
        .expect("add target_node_id");
    sqlx::query(
        r#"
        CREATE TABLE agent_nodes (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            grpc_target TEXT NOT NULL,
            tls_server_name TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(&db)
    .await
    .expect("create agent_nodes");

    let manager = TeamManager::new(db.clone());
    manager.configure_internal_grpc_peer_client(Some(InternalGrpcPeerClientConfig {
        shared_secret: "team-channel-p2p-secret".to_string(),
        expected_issuer: Some("agenthub".to_string()),
        expected_audience: Some("agenthub-internal".to_string()),
        source_node_id: "main".to_string(),
        cert_dir: std::env::temp_dir()
            .join(format!("team-channel-p2p-{}", Uuid::new_v4()))
            .to_string_lossy()
            .to_string(),
        security_mode: InternalGrpcSecurityMode::Mtls,
    }));
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-channel-p2p-team".to_string(),
            description: Some("team for channel mailbox p2p broadcast".to_string()),
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
            Some("ctx-channel-mailbox-p2p"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let now = Utc::now().timestamp();
    for (agent_id, target_node_id) in [
        ("planner", None),
        ("reviewer", None),
        ("worker", Some("node-east")),
    ] {
        sqlx::query(
            r#"
            INSERT INTO agents (
                id,
                name,
                workdir,
                command,
                args,
                worktree_mode,
                code_mode,
                agent_loop_enabled,
                source,
                status,
                created_at,
                updated_at,
                target_node_id
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, 'manual', 'running', ?7, ?8, ?9)
            "#,
        )
        .bind(agent_id)
        .bind(format!("Agent {agent_id}"))
        .bind(format!("/tmp/{agent_id}"))
        .bind("agenthub")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .bind(target_node_id)
        .execute(&db)
        .await
        .expect("insert agent");
    }

    sqlx::query(
        r#"
        INSERT INTO agent_nodes (id, name, grpc_target, tls_server_name, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind("node-east")
    .bind("Node East")
    .bind("https://node-east.internal:50051")
    .bind("node-east.internal")
    .bind(now)
    .bind(now)
    .execute(&db)
    .await
    .expect("insert remote node");

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
                "text":"@worker please validate remote relay"
            }),
            idempotency_key: Some("msg-channel-mailbox-p2p-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("send p2p channel mailbox message");
    assert_eq!(sent.state, TeamActorMessageStatus::Pending);

    let canonical_row = sqlx::query(
        r#"
        SELECT from_actor_id, to_actor_id, route, payload_json
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
    .expect("load canonical channel conversation message");
    let canonical_payload: Value =
        serde_json::from_str(canonical_row.get::<String, _>("payload_json").as_str())
            .expect("decode canonical channel payload");
    assert_eq!(canonical_row.get::<String, _>("from_actor_id"), "planner");
    assert!(
        canonical_row
            .try_get::<Option<String>, _>("to_actor_id")
            .ok()
            .flatten()
            .is_none()
    );
    assert_eq!(canonical_row.get::<String, _>("route"), "group_chat");
    assert_eq!(
        canonical_payload["text"],
        json!("@worker please validate remote relay")
    );

    let rows = sqlx::query(
        r#"
        SELECT to_actor_id, to_peer_id, transport, route_json, payload_json
        FROM team_actor_messages
        WHERE run_id = ?1
        ORDER BY to_actor_id ASC
        "#,
    )
    .bind(&run.id)
    .fetch_all(&db)
    .await
    .expect("load p2p mailbox rows");
    assert_eq!(rows.len(), 2);

    let reviewer = rows
        .iter()
        .find(|row| row.get::<String, _>("to_actor_id") == "reviewer")
        .expect("reviewer row");
    assert_eq!(reviewer.get::<String, _>("to_peer_id"), ACTOR_MAIN_PEER_ID);
    assert_eq!(reviewer.get::<String, _>("transport"), "local");
    assert!(
        reviewer
            .try_get::<Option<String>, _>("route_json")
            .ok()
            .flatten()
            .is_none()
    );

    let worker = rows
        .iter()
        .find(|row| row.get::<String, _>("to_actor_id") == "worker")
        .expect("worker row");
    assert_eq!(worker.get::<String, _>("to_peer_id"), ACTOR_NODE_PEER_ID);
    assert_eq!(worker.get::<String, _>("transport"), "remote");
    let route: Value =
        serde_json::from_str(worker.get::<String, _>("route_json").as_str()).expect("route");
    assert_eq!(route["kind"], json!("grpc"));
    assert_eq!(
        route["grpc_target"],
        json!("https://node-east.internal:50051")
    );
    assert_eq!(route["tls_server_name"], json!("node-east.internal"));
    assert_eq!(route["target_node_id"], json!("node-east"));
    assert!(
        route.get("access_token").is_none(),
        "persisted route should stay stable and omit access_token: {route}"
    );
    assert!(
        route.get("issued_at").is_none() && route.get("expires_at").is_none(),
        "persisted route should omit transient credential metadata: {route}"
    );

    let worker_payload: Value =
        serde_json::from_str(worker.get::<String, _>("payload_json").as_str())
            .expect("worker payload");
    assert_eq!(worker_payload["delivery_scope"], json!("channel_broadcast"));
    assert_eq!(worker_payload["team_id"], json!(team.id));
    assert!(
        worker_payload["authority_message_id"]
            .as_i64()
            .is_some_and(|value| value > 0),
        "missing authority_message_id: {worker_payload}"
    );
    assert_eq!(worker_payload["mention_actor_ids"], json!(["worker"]));
    assert_eq!(worker_payload["mentioned_actor_ids"], json!(["worker"]));
}

#[tokio::test]
async fn remote_actor_messages_relay_success_preserves_payload_metadata() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let (endpoint, captures, server_handle) = spawn_relay_http_server(StatusCode::OK).await;

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-relay-success-team".to_string(),
            description: Some("team for relay success flow".to_string()),
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
            Some("ctx-relay-success"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let sent = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "remote-reviewer",
            to_peer_id: ACTOR_NODE_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Remote,
            route: Some(json!({
                "endpoint": endpoint,
                "method": "POST",
                "headers": {
                    "x-agenthub-relay-test": "success"
                },
                "auth": {
                    "type": "bearer",
                    "token": "relay-token"
                },
                "signing": {
                    "type": "hmac_sha256",
                    "secret": "relay-signing-secret",
                    "header": "x-agenthub-signature",
                    "timestamp_header": "x-agenthub-timestamp"
                }
            })),
            payload: json!({
                "text": "@remote-reviewer review this",
                "summary": "Review handoff is ready",
                "detail_ref": {
                    "uri": "artifact://remote-review/full-review-1",
                    "label": "Full review",
                    "kind": "artifact",
                    "content_type": "text/markdown"
                },
                "mention_actor_ids": ["remote-reviewer"],
                "mentioned_actor_ids": ["remote-reviewer"]
            }),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .expect("send remote message");
    assert_eq!(sent.status, TeamActorMessageStatus::Pending);

    let relay_result = manager
        .relay_remote_messages_once(100, 3, 30)
        .await
        .expect("relay remote messages");
    assert_eq!(relay_result.scanned, 1);
    assert_eq!(relay_result.delivered, 1);
    assert_eq!(relay_result.retried, 0);
    assert_eq!(relay_result.dead_lettered, 0);

    let relayed_row = sqlx::query(
        r#"
        SELECT status, delivered_at
        FROM team_actor_messages
        WHERE id = ?1
        "#,
    )
    .bind(sent.message_id)
    .fetch_one(&db)
    .await
    .expect("fetch relayed message row");
    let relayed_status: String = relayed_row.get("status");
    let relayed_at: Option<i64> = relayed_row.try_get("delivered_at").ok();
    assert_eq!(relayed_status, "delivered");
    assert!(relayed_at.is_some());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let delivered_count = events
        .iter()
        .filter(|event| event.event_type == "actor_message_delivered")
        .count();
    assert_eq!(delivered_count, 1);

    let captured = captures.lock().await;
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].method, "POST");
    assert_eq!(
        captured[0].headers.get("x-agenthub-relay-test"),
        Some(&"success".to_string())
    );
    assert_eq!(
        captured[0].headers.get("authorization"),
        Some(&"Bearer relay-token".to_string())
    );
    assert!(
        captured[0].headers.contains_key("x-agenthub-signature"),
        "missing signature header"
    );
    assert!(
        captured[0].headers.contains_key("x-agenthub-timestamp"),
        "missing signature timestamp header"
    );
    assert_eq!(
        captured[0].headers.get("x-agenthub-message-id"),
        Some(&sent.message_id.to_string())
    );
    assert_eq!(captured[0].body["run_id"], run.id);
    assert_eq!(captured[0].body["source_node_id"], "main");
    assert_eq!(captured[0].body["target_node_id"], "node");
    assert_eq!(captured[0].body["from_actor_id"], "planner");
    assert_eq!(captured[0].body["from_actor_kind"], "agent");
    assert_eq!(captured[0].body["to_actor_id"], "remote-reviewer");
    assert_eq!(captured[0].body["to_actor_kind"], "agent");
    assert_eq!(captured[0].body["message_kind"], "coordination_request");
    assert_eq!(captured[0].body["scope"], json!(["node:p2p"]));
    assert_eq!(captured[0].body["kid"], "phase1-shared-key");
    assert!(captured[0].body["payload_digest"].is_string());
    assert_eq!(
        captured[0].body["payload"]["text"],
        "@remote-reviewer review this"
    );
    assert_eq!(
        captured[0].body["payload"]["summary"],
        "Review handoff is ready"
    );
    assert_eq!(
        captured[0].body["payload"]["detail_ref"],
        json!({
            "uri": "artifact://remote-review/full-review-1",
            "label": "Full review",
            "kind": "artifact",
            "content_type": "text/markdown"
        })
    );
    assert_eq!(
        captured[0].body["payload"]["mention_actor_ids"],
        json!(["remote-reviewer"])
    );
    assert_eq!(
        captured[0].body["payload"]["mentioned_actor_ids"],
        json!(["remote-reviewer"])
    );
    drop(captured);
    server_handle.abort();
}

#[tokio::test]
async fn remote_actor_messages_relay_supports_retry_and_dead_letter() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let (retry_endpoint, retry_captures, retry_server_handle) =
        spawn_relay_http_server(StatusCode::SERVICE_UNAVAILABLE).await;
    let (dead_endpoint, dead_captures, dead_server_handle) =
        spawn_relay_http_server(StatusCode::BAD_REQUEST).await;

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-relay-policy-team".to_string(),
            description: Some("team for relay retry/dead-letter policy".to_string()),
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
            Some("ctx-relay-policy"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let retry_message = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "remote-retry",
            to_peer_id: ACTOR_NODE_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Remote,
            route: Some(json!({
                "endpoint": retry_endpoint,
                "method": "POST",
                "auth": {
                    "type": "header",
                    "name": "x-agenthub-auth",
                    "value": "retry-secret"
                }
            })),
            payload: json!({"text":"retry this"}),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .expect("send retry remote message");
    let dead_message = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "remote-dead",
            to_peer_id: ACTOR_NODE_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Remote,
            route: Some(json!({
                "endpoint": dead_endpoint,
                "method": "POST"
            })),
            payload: json!({"text":"dead-letter this"}),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .expect("send dead remote message");

    let relay_result = manager
        .relay_remote_messages_once(100, 3, 60)
        .await
        .expect("relay remote messages");
    assert_eq!(relay_result.scanned, 2);
    assert_eq!(relay_result.delivered, 0);
    assert_eq!(relay_result.retried, 1);
    assert_eq!(relay_result.dead_lettered, 1);

    let retry_row = sqlx::query(
        r#"
        SELECT status, relay_attempt, relay_next_retry_at, dead_letter_at
        FROM team_actor_messages
        WHERE id = ?1
        "#,
    )
    .bind(retry_message.message_id)
    .fetch_one(&db)
    .await
    .expect("fetch retry message row");
    let retry_status: String = retry_row.get("status");
    let retry_attempt: i64 = retry_row.get("relay_attempt");
    let retry_next: Option<i64> = retry_row
        .try_get("relay_next_retry_at")
        .expect("retry next retry at");
    let retry_dead_letter_at: Option<i64> = retry_row
        .try_get("dead_letter_at")
        .expect("retry dead letter at");
    assert_eq!(retry_status, "pending");
    assert_eq!(retry_attempt, 1);
    assert!(retry_next.is_some());
    assert!(retry_dead_letter_at.is_none());

    let dead_row = sqlx::query(
        r#"
        SELECT status, relay_attempt, relay_next_retry_at, dead_letter_at
        FROM team_actor_messages
        WHERE id = ?1
        "#,
    )
    .bind(dead_message.message_id)
    .fetch_one(&db)
    .await
    .expect("fetch dead-letter message row");
    let dead_status: String = dead_row.get("status");
    let dead_attempt: i64 = dead_row.get("relay_attempt");
    let dead_next: Option<i64> = dead_row
        .try_get("relay_next_retry_at")
        .expect("dead next retry at");
    let dead_dead_letter_at: Option<i64> = dead_row
        .try_get("dead_letter_at")
        .expect("dead dead letter at");
    assert_eq!(dead_status, "dead_letter");
    assert_eq!(dead_attempt, 1);
    assert!(dead_next.is_none());
    assert!(dead_dead_letter_at.is_some());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let retry_event_count = events
        .iter()
        .filter(|event| event.event_type == "actor_message_relay_retry")
        .count();
    let dead_letter_event_count = events
        .iter()
        .filter(|event| event.event_type == "actor_message_dead_letter")
        .count();
    assert_eq!(retry_event_count, 1);
    assert_eq!(dead_letter_event_count, 1);

    let retry_captured = retry_captures.lock().await;
    assert_eq!(retry_captured.len(), 1);
    assert_eq!(
        retry_captured[0].headers.get("x-agenthub-auth"),
        Some(&"retry-secret".to_string())
    );
    assert_eq!(retry_captured[0].body["to_actor_id"], "remote-retry");
    drop(retry_captured);

    let dead_captured = dead_captures.lock().await;
    assert_eq!(dead_captured.len(), 1);
    assert_eq!(dead_captured[0].body["to_actor_id"], "remote-dead");
    drop(dead_captured);

    retry_server_handle.abort();
    dead_server_handle.abort();
}

#[tokio::test]
async fn remote_actor_messages_relay_rejects_invalid_header_values() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let (endpoint, captures, server_handle) = spawn_relay_http_server(StatusCode::OK).await;

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-relay-invalid-header-team".to_string(),
            description: Some("team for invalid relay header validation".to_string()),
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
            Some("ctx-relay-invalid-header"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let sent = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "remote-reviewer",
            to_peer_id: ACTOR_NODE_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Remote,
            route: Some(json!({
                "endpoint": endpoint,
                "method": "POST",
                "headers": {
                    "x-agenthub-relay-test": "bad\nvalue"
                }
            })),
            payload: json!({"text":"review this"}),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .expect("send remote message");
    assert_eq!(sent.status, TeamActorMessageStatus::Pending);

    let relay_result = manager
        .relay_remote_messages_once(100, 3, 30)
        .await
        .expect("relay remote messages");
    assert_eq!(relay_result.scanned, 1);
    assert_eq!(relay_result.delivered, 0);
    assert_eq!(relay_result.retried, 0);
    assert_eq!(relay_result.dead_lettered, 1);

    let rows = sqlx::query(
        r#"
        SELECT status, relay_last_error
        FROM team_actor_messages
        WHERE id = ?1
        "#,
    )
    .bind(sent.message_id)
    .fetch_one(&db)
    .await
    .expect("fetch relay row");
    let status: String = rows.get("status");
    let relay_last_error: Option<String> = rows.try_get("relay_last_error").ok();
    assert_eq!(status, "dead_letter");
    assert!(
        relay_last_error
            .as_deref()
            .is_some_and(|text| text.contains("invalid")),
        "unexpected relay error: {:?}",
        relay_last_error
    );

    let captured = captures.lock().await;
    assert!(captured.is_empty());
    drop(captured);
    server_handle.abort();
}
