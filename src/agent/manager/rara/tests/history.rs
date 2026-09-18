use agenthub_db::runtime_events::{
    RuntimeEventStore, RuntimeRequestAck, RuntimeRequestIntent, RuntimeRequestKind,
    RuntimeRequestStatus,
};

use super::*;

#[tokio::test]
async fn startup_recovery_retires_transport_ownership_without_retry_or_task_completion() {
    let fixture = Fixture::new("normal").await;
    sqlx::query("INSERT INTO agent_sessions (id, agent_id, status, started_at) VALUES ('old-local', ?, 'running', 1)")
        .bind(&fixture.agent_id).execute(&fixture.manager.db).await.unwrap();
    let pool = fixture
        .manager
        .event_dbs
        .pool_for_agent(&fixture.agent_id)
        .await
        .unwrap();
    let store = RuntimeEventStore::bind(pool, "old-local", "old-runtime")
        .await
        .unwrap();
    store.bind_stream("old-native").await.unwrap();
    for id in ["prepared", "sent", "accepted"] {
        store
            .prepare_request(
                RuntimeRequestIntent {
                    request_id: id,
                    kind: RuntimeRequestKind::Prompt,
                    target_session_id: Some("old-native"),
                    expected_turn_id: None,
                },
                1,
            )
            .await
            .unwrap();
        if id != "prepared" {
            let permit = store.mark_request_sent(id, 2).await.unwrap();
            if id == "accepted" {
                store
                    .record_request_ack(
                        &permit,
                        RuntimeRequestAck::Accepted {
                            session_id: "old-native".into(),
                            turn_id: Some("old-turn".into()),
                            last_sequence: None,
                        },
                        3,
                    )
                    .await
                    .unwrap();
            }
        }
    }
    let lock_path = fixture.directory.join("daemon.db");
    let (permission_id, mut callback) = fixture
        .manager
        .permissions
        .create_request(
            &fixture.agent_id,
            "old-local",
            &agent_client_protocol::schema::v1::RequestPermissionRequest::new(
                "old-native",
                agent_client_protocol::schema::v1::ToolCallUpdate::new(
                    "old-tool",
                    agent_client_protocol::schema::v1::ToolCallUpdateFields::new(),
                ),
                Vec::new(),
            ),
            None,
        )
        .await
        .unwrap();
    let mut daemon =
        crate::daemon_instance::DaemonInstanceGuard::acquire(&lock_path, "main").unwrap();
    assert!(
        fixture
            .manager
            .recover_runtime_receipts_on_startup(&daemon)
            .await
            .is_err()
    );
    daemon.claim_generation(&fixture.manager.db).await.unwrap();
    fixture.manager.mark_exited_on_startup().await.unwrap();
    fixture
        .manager
        .recover_runtime_receipts_on_startup(&daemon)
        .await
        .unwrap();
    fixture
        .manager
        .recover_runtime_receipts_on_startup(&daemon)
        .await
        .unwrap();
    let history = fixture
        .manager
        .runtime_history(&fixture.agent_id, "old-local", 100, None)
        .await
        .unwrap()
        .unwrap();
    assert!(history.closed);
    assert_eq!(history.streams[0].cursor.sequence, 0);
    for (id, expected) in [
        ("prepared", RuntimeRequestStatus::NotSent),
        ("sent", RuntimeRequestStatus::OutcomeUnknown),
        ("accepted", RuntimeRequestStatus::Accepted),
    ] {
        assert_eq!(
            history
                .receipts
                .iter()
                .find(|receipt| receipt.request_id == id)
                .unwrap()
                .status,
            expected
        );
    }
    assert!(fixture.input_requests().is_empty());
    assert!(!fixture.directory.join("pid").exists());
    assert!(
        fixture
            .manager
            .runtime_history("another-agent", "old-local", 100, None)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        fixture
            .manager
            .runtime_history(&fixture.agent_id, "another-local", 100, None)
            .await
            .unwrap()
            .is_none()
    );
    assert!(store.bind_stream("resurrected").await.is_err());
    assert!(matches!(
        callback.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Closed)
    ));
    let permission_status: String =
        sqlx::query_scalar("SELECT status FROM acp_permission_requests WHERE id = ?")
            .bind(permission_id)
            .fetch_one(&fixture.manager.db)
            .await
            .unwrap();
    assert_eq!(permission_status, "timeout");
    let ended: bool = sqlx::query_scalar(
        "SELECT ended_at IS NOT NULL FROM agent_sessions WHERE id = 'old-local'",
    )
    .fetch_one(&fixture.manager.db)
    .await
    .unwrap();
    assert!(ended);
    drop(daemon);
    fixture.finish().await;
}

#[tokio::test]
async fn history_remains_readable_after_exit_and_cannot_resolve_foreign_sessions() {
    let fixture = Fixture::new("normal").await;
    let session = fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    fixture
        .manager
        .send_input(
            &fixture.agent_id,
            "private-input-body",
            Some("input"),
            Some(&session),
        )
        .await
        .unwrap();
    fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
    let history = fixture
        .manager
        .runtime_history(&fixture.agent_id, &session, 100, None)
        .await
        .unwrap()
        .unwrap();
    assert!(history.closed);
    assert_eq!(history.runtime_id, "managed-runtime");
    assert_eq!(history.local_session_id, session);
    assert!(history.streams[0].cursor.sequence >= 1);
    assert!(
        !serde_json::to_string(&history)
            .unwrap()
            .contains("private-input-body")
    );
    assert!(
        fixture
            .manager
            .runtime_history("foreign", &session, 100, None)
            .await
            .unwrap()
            .is_none()
    );
    fixture.finish().await;
}
