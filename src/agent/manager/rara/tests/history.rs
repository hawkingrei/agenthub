use agenthub_db::runtime_events::{
    RuntimeEventStore, RuntimeHistoryEntry, RuntimeRequestAck, RuntimeRequestIntent,
    RuntimeRequestKind, RuntimeRequestStatus,
};

use super::*;

#[cfg(target_os = "linux")]
fn event_database_handles(database: &std::path::Path) -> usize {
    let paths: Vec<_> = ["", "-wal", "-shm"]
        .into_iter()
        .map(|suffix| {
            let mut path = database.as_os_str().to_os_string();
            path.push(suffix);
            PathBuf::from(path)
        })
        .collect();
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|entry| std::fs::read_link(entry.path()).ok())
        .filter(|path| paths.contains(path))
        .count()
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn startup_recovery_releases_temporary_pools_on_success_and_failure() {
    let fixture = Fixture::new("normal").await;
    let directory = fixture.manager.event_dbs.base_dir().to_owned();
    let seed = agenthub_db::AgentEventDbRouter::new(directory.clone());
    for agent_id in [&fixture.agent_id, "historical-a", "historical-z"] {
        if agent_id != fixture.agent_id {
            sqlx::query("INSERT INTO agents (id, name, workdir, command, args, worktree_mode, status, created_at, updated_at) VALUES (?, ?, ?, 'codex', '[]', 'use_existing', 'stopped', 1, 1)")
                .bind(agent_id).bind(agent_id).bind(fixture.directory.to_str().unwrap())
                .execute(&fixture.manager.db).await.unwrap();
        }
        let pool = seed.pool_for_agent(agent_id).await.unwrap();
        if agent_id == fixture.agent_id {
            sqlx::query("INSERT INTO agent_sessions (id, agent_id, status, started_at) VALUES ('old-local', ?, 'running', 1)")
                .bind(agent_id).execute(&fixture.manager.db).await.unwrap();
            RuntimeEventStore::bind(pool.clone(), "old-local", "old-runtime")
                .await
                .unwrap();
        }
        pool.close().await;
    }
    drop(seed);
    let cached = fixture
        .manager
        .event_dbs
        .pool_for_agent("historical-z")
        .await
        .unwrap();
    // SQLite can retain shared WAL handles while an existing reader remains open.
    // Count only cold databases; the cached reader's lifetime is checked separately.
    let cold_handles = || {
        [&fixture.agent_id, "historical-a"].map(|agent_id| {
            event_database_handles(&fixture.manager.event_dbs.db_path_for_agent(agent_id))
        })
    };
    let baseline = cold_handles();
    assert_eq!(baseline, [0, 0]);
    let mut daemon = crate::daemon_instance::DaemonInstanceGuard::acquire(
        &fixture.directory.join("pool-recovery.db"),
        "main",
    )
    .unwrap();
    daemon.claim_generation(&fixture.manager.db).await.unwrap();
    sqlx::query("CREATE TRIGGER fail_recovery BEFORE UPDATE ON agent_sessions WHEN OLD.id = 'old-local' BEGIN SELECT RAISE(ABORT, 'fixture recovery failure'); END")
        .execute(&fixture.manager.db).await.unwrap();
    let failed = fixture
        .manager
        .recover_runtime_receipts_on_startup(&daemon)
        .await;
    let after_failure = cold_handles();
    sqlx::query("DROP TRIGGER fail_recovery")
        .execute(&fixture.manager.db)
        .await
        .unwrap();
    fixture
        .manager
        .recover_runtime_receipts_on_startup(&daemon)
        .await
        .unwrap();
    let after_success = cold_handles();
    fixture
        .manager
        .recover_runtime_receipts_on_startup(&daemon)
        .await
        .unwrap();
    let after_repeat = cold_handles();
    let cached_open = !cached.is_closed();
    let cached_query = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runtime_event_owners")
        .fetch_one(&cached)
        .await;
    cached.close().await;
    drop(daemon);
    fixture.finish().await;

    assert!(failed.is_err());
    assert_eq!(
        after_failure, baseline,
        "failed recovery must close its temporary pool"
    );
    assert_eq!(
        after_success, baseline,
        "historical databases must not retain new pools"
    );
    assert_eq!(after_repeat, baseline);
    assert!(
        cached_open,
        "recovery must not close an existing cached reader"
    );
    cached_query.unwrap();
}

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
        let message = serde_json::json!({
            "type":"user_message", "text":"Recover this input", "message_id":id,
            "meta":{"delivery":"pending", "provider_runtime":{
                "provider":"rara", "runtime_id":"old-runtime",
                "native_session_id":"old-native", "request_id":id
            }}
        });
        let encoded = agenthub_agent_event_codec::encode_message_for_storage(
            &crate::agent::OutputStream::Acp,
            &message.to_string(),
        );
        store
            .prepare_input_request(
                RuntimeRequestIntent {
                    request_id: id,
                    kind: RuntimeRequestKind::Prompt,
                    target_session_id: Some("old-native"),
                    expected_turn_id: None,
                },
                1,
                RuntimeHistoryEntry {
                    seq: id,
                    ts: 1,
                    stream: crate::agent::OutputStream::Acp,
                    message: &encoded,
                },
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
    sqlx::raw_sql("CREATE TRIGGER fail_permission_cleanup BEFORE UPDATE ON acp_permission_requests WHEN NEW.status = 'timeout' BEGIN SELECT RAISE(ABORT, 'fixture cleanup interruption'); END;")
        .execute(&fixture.manager.db).await.unwrap();
    assert!(
        fixture
            .manager
            .recover_runtime_receipts_on_startup(&daemon)
            .await
            .is_err()
    );
    assert!(
        !store.history(1, None).await.unwrap().closed,
        "failed cleanup must remain recoverable"
    );
    sqlx::query("DROP TRIGGER fail_permission_cleanup")
        .execute(&fixture.manager.db)
        .await
        .unwrap();
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
    let events = fixture
        .manager
        .list_events(&fixture.agent_id, 100, None)
        .await
        .unwrap();
    assert_eq!(
        events.len(),
        3,
        "recovery must not append duplicate history"
    );
    for (event, expected) in events
        .iter()
        .zip(["not_sent", "outcome_unknown", "accepted"])
    {
        let message: serde_json::Value = serde_json::from_str(&event.message).unwrap();
        assert_eq!(message["meta"]["delivery"], expected);
        let reloaded = fixture
            .manager
            .get_event(&fixture.agent_id, event.event_id)
            .await
            .unwrap();
        assert_eq!(reloaded.message, event.message);
        let page = fixture
            .manager
            .list_events_for_session(&fixture.agent_id, "old-local", 1, Some(event.event_id + 1))
            .await
            .unwrap();
        assert_eq!(page[0].message, event.message);
    }
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

#[tokio::test]
async fn startup_recovery_skips_closed_owners_and_drains_multiple_open_pages() {
    let fixture = Fixture::new("normal").await;
    let pool = fixture
        .manager
        .event_dbs
        .pool_for_agent(&fixture.agent_id)
        .await
        .unwrap();
    for index in 0..205 {
        let session = format!("history-{index:03}");
        sqlx::query("INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at) VALUES (?, ?, 'exited', 1, 2)")
            .bind(&session).bind(&fixture.agent_id).execute(&fixture.manager.db).await.unwrap();
        sqlx::query("INSERT INTO runtime_event_owners(runtime_id, local_session_id, closed) VALUES (?, ?, ?)")
            .bind(format!("runtime-{index:03}")).bind(session).bind(index < 100)
            .execute(&pool).await.unwrap();
    }
    // Repeated recovery must not write owners or control-plane rows already retired.
    sqlx::raw_sql("CREATE TRIGGER reject_closed_owner_update BEFORE UPDATE ON runtime_event_owners WHEN OLD.closed = 1 BEGIN SELECT RAISE(ABORT, 'closed owner revisited'); END;")
        .execute(&pool).await.unwrap();
    sqlx::raw_sql("CREATE TRIGGER reject_ended_session_update BEFORE UPDATE ON agent_sessions WHEN OLD.ended_at IS NOT NULL BEGIN SELECT RAISE(ABORT, 'ended session revisited'); END;")
        .execute(&fixture.manager.db).await.unwrap();
    let mut daemon = crate::daemon_instance::DaemonInstanceGuard::acquire(
        &fixture.directory.join("recovery.db"),
        "main",
    )
    .unwrap();
    daemon.claim_generation(&fixture.manager.db).await.unwrap();
    let orphan = RuntimeEventStore::bind(pool.clone(), "foreign-local", "foreign-runtime")
        .await
        .unwrap();
    for _ in 0..2 {
        fixture
            .manager
            .recover_runtime_receipts_on_startup(&daemon)
            .await
            .unwrap();
    }
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM runtime_event_owners WHERE closed = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 1);
    assert!(
        !orphan.history(1, None).await.unwrap().closed,
        "unassociated owners cannot mutate another session"
    );
    drop(daemon);
    fixture.finish().await;
}
