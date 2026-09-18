use std::path::PathBuf;

use agenthub_agent_domain::OutputStream;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::*;

mod requests;

struct Fixture {
    directory: PathBuf,
    pool: SqlitePool,
    owner: RuntimeEventStore,
    stream: RuntimeEventStream,
}

impl Fixture {
    async fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("runtime-events-{}", Uuid::new_v4()));
        let pool = crate::AgentEventDbRouter::new(directory.clone())
            .pool_for_agent("actor")
            .await
            .unwrap();
        let owner = RuntimeEventStore::bind(pool.clone(), "local", "runtime")
            .await
            .unwrap();
        let stream = owner.bind_stream("native").await.unwrap();
        Self {
            directory,
            pool,
            owner,
            stream,
        }
    }

    async fn history_count(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM agent_events")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn event(id: &str, sequence: u64) -> RuntimeEventIdentity<'_> {
    RuntimeEventIdentity {
        event_id: id,
        sequence,
        fingerprint: &[7; 32],
    }
}

fn history(seq: &str) -> RuntimeHistoryEntry<'_> {
    RuntimeHistoryEntry {
        seq,
        ts: 100,
        stream: OutputStream::Acp,
        message: br#"{"type":"agent_message","text":"visible conversation"}"#,
    }
}

#[tokio::test]
async fn migrate_existing_event_database_preserves_history_and_reopens_idempotently() {
    let directory = std::env::temp_dir().join(format!("runtime-migration-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let pool = crate::connect_sqlite_with_defaults(&directory.join("actor.db"), 2)
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE agent_events(id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL, \
         seq TEXT NOT NULL, ts INTEGER NOT NULL, stream TEXT NOT NULL, message TEXT NOT NULL); \
         INSERT INTO agent_events(session_id, seq, ts, stream, message) \
         VALUES ('legacy', 'legacy-event', 1, 'stdout', 'existing output');",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
    let router = crate::AgentEventDbRouter::new(directory.clone());
    let pool = router.pool_for_agent("actor").await.unwrap();
    migrate(&pool).await.unwrap();
    let (session, message): (String, Vec<u8>) =
        sqlx::query_as("SELECT session_id, message FROM agent_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(session, "legacy");
    assert_eq!(message, b"existing output");
    RuntimeEventStore::bind(pool.clone(), "local", "runtime")
        .await
        .unwrap()
        .bind_stream("native")
        .await
        .unwrap();
    pool.close().await;
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn persistence_commits_history_receipt_and_cursor_together() {
    let fixture = Fixture::new().await;
    let outcome = fixture
        .stream
        .persist(
            event("first", 1),
            &[history("projection-a"), history("projection-b")],
        )
        .await
        .unwrap();
    let RuntimePersistResult::Persisted { history_ids } = outcome else {
        panic!("event was not persisted");
    };
    assert_eq!(history_ids.len(), 2);
    assert!(history_ids[0] < history_ids[1]);
    assert_eq!(fixture.stream.cursor().await.unwrap().sequence, 1);
    assert_eq!(fixture.history_count().await, 2);
    let sessions: Vec<String> = sqlx::query_scalar("SELECT session_id FROM agent_events")
        .fetch_all(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(sessions, ["local", "local"]);
    let links: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT history_id, runtime_id, native_session_id, event_id FROM runtime_event_history ORDER BY history_id",
    ).fetch_all(&fixture.pool).await.unwrap();
    assert_eq!(
        links,
        history_ids
            .into_iter()
            .map(|id| (id, "runtime".into(), "native".into(), "first".into()))
            .collect::<Vec<_>>()
    );
    // Metadata-only events still advance the stream without inventing conversation output.
    fixture
        .stream
        .persist(event("second", 2), &[])
        .await
        .unwrap();
    assert_eq!(fixture.stream.cursor().await.unwrap().sequence, 2);
    assert_eq!(fixture.history_count().await, 2);
}

#[tokio::test]
async fn concurrent_duplicate_and_reopened_replay_emit_history_once() {
    let fixture = Fixture::new().await;
    let mut pending = tokio::task::JoinSet::new();
    for _ in 0..12 {
        let stream = fixture.stream.clone();
        pending.spawn(async move {
            stream
                .persist(event("first", 1), &[history("projection")])
                .await
                .unwrap()
        });
    }
    let mut inserted = 0;
    while let Some(outcome) = pending.join_next().await {
        inserted += usize::from(matches!(
            outcome.unwrap(),
            RuntimePersistResult::Persisted { .. }
        ));
    }
    assert_eq!(inserted, 1);
    assert_eq!(fixture.history_count().await, 1);
    fixture.pool.close().await;
    let pool = crate::AgentEventDbRouter::new(fixture.directory.clone())
        .pool_for_agent("actor")
        .await
        .unwrap();
    let reopened = RuntimeEventStore::bind(pool.clone(), "local", "runtime")
        .await
        .unwrap()
        .stream("native")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reopened.cursor().await.unwrap().sequence, 1);
    assert_eq!(
        reopened
            .persist(event("first", 1), &[history("another-projection")])
            .await
            .unwrap(),
        RuntimePersistResult::Duplicate
    );
    reopened
        .persist(event("second", 2), &[history("new-projection")])
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
    pool.close().await;
}

#[tokio::test]
async fn failed_history_insert_rolls_back_receipt_and_cursor() {
    let fixture = Fixture::new().await;
    sqlx::raw_sql(
        "CREATE TRIGGER reject_projection BEFORE INSERT ON agent_events \
         BEGIN SELECT RAISE(ABORT, 'fixture storage failure'); END;",
    )
    .execute(&fixture.pool)
    .await
    .unwrap();
    assert!(
        fixture
            .stream
            .persist(event("first", 1), &[history("projection")])
            .await
            .is_err()
    );
    assert_eq!(fixture.stream.cursor().await.unwrap().sequence, 0);
    let receipts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_event_receipts")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(receipts, 0);
    sqlx::query("DROP TRIGGER reject_projection")
        .execute(&fixture.pool)
        .await
        .unwrap();
    assert!(matches!(
        fixture
            .stream
            .persist(event("first", 1), &[history("projection")])
            .await
            .unwrap(),
        RuntimePersistResult::Persisted { .. }
    ));
}

#[tokio::test]
async fn gaps_do_not_advance_history_and_replay_restores_order() {
    let fixture = Fixture::new().await;
    assert_eq!(
        fixture
            .stream
            .persist(event("second", 2), &[history("second")])
            .await
            .unwrap(),
        RuntimePersistResult::Gap {
            expected: 1,
            received: 2
        }
    );
    assert_eq!(fixture.stream.cursor().await.unwrap().sequence, 0);
    assert_eq!(fixture.history_count().await, 0);
    fixture
        .stream
        .persist(event("first", 1), &[history("first")])
        .await
        .unwrap();
    fixture
        .stream
        .persist(event("second", 2), &[history("second")])
        .await
        .unwrap();
    let order: Vec<String> = sqlx::query_scalar("SELECT seq FROM agent_events ORDER BY id")
        .fetch_all(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(order, ["first", "second"]);
    assert_eq!(fixture.stream.cursor().await.unwrap().sequence, 2);
}

#[tokio::test]
async fn event_and_sequence_conflicts_are_rejected_without_payload_errors() {
    let fixture = Fixture::new().await;
    fixture
        .stream
        .persist(event("first", 1), &[history("first")])
        .await
        .unwrap();
    for conflicting in [
        event("first", 2),
        event("other", 1),
        RuntimeEventIdentity {
            fingerprint: &[8; 32],
            ..event("first", 1)
        },
    ] {
        let error = fixture.stream.persist(conflicting, &[]).await.unwrap_err();
        assert!(matches!(
            error.downcast_ref(),
            Some(RuntimeEventError::EventConflict)
        ));
        assert!(!error.to_string().contains("visible conversation"));
    }
    assert_eq!(fixture.stream.cursor().await.unwrap().sequence, 1);
    assert_eq!(fixture.history_count().await, 1);
}

#[tokio::test]
async fn runtime_and_session_identity_isolation_and_close_survive_reopen() {
    let fixture = Fixture::new().await;
    for (local, runtime) in [("another", "runtime"), ("local", "replacement")] {
        assert!(matches!(
            RuntimeEventStore::bind(fixture.pool.clone(), local, runtime)
                .await
                .err()
                .unwrap()
                .downcast_ref(),
            Some(RuntimeEventError::OwnershipConflict)
        ));
    }
    assert!(fixture.owner.stream("unsolicited").await.unwrap().is_none());
    let other_session = fixture.owner.bind_stream("another-native").await.unwrap();
    let other_runtime = RuntimeEventStore::bind(fixture.pool.clone(), "another", "replacement")
        .await
        .unwrap()
        .bind_stream("native")
        .await
        .unwrap();
    for stream in [&fixture.stream, &other_session, &other_runtime] {
        stream
            .persist(event("same-id", 1), &[history("projection")])
            .await
            .unwrap();
    }
    assert_eq!(fixture.history_count().await, 3);
    fixture.owner.close(110).await.unwrap();
    let reopened = RuntimeEventStore::bind(fixture.pool.clone(), "local", "runtime")
        .await
        .unwrap();
    assert!(reopened.bind_stream("native").await.is_err());
    let stream = reopened.stream("native").await.unwrap().unwrap();
    assert_eq!(stream.cursor().await.unwrap().sequence, 1);
    assert!(matches!(
        stream
            .persist(event("second", 2), &[])
            .await
            .unwrap_err()
            .downcast_ref(),
        Some(RuntimeEventError::Closed)
    ));
    other_runtime
        .persist(event("second", 2), &[])
        .await
        .unwrap();
}

#[tokio::test]
async fn unavailable_replay_prefix_is_visible_and_never_skipped() {
    let fixture = Fixture::new().await;
    fixture
        .stream
        .persist(event("first", 1), &[])
        .await
        .unwrap();
    for gap in [
        RuntimeReplayGap {
            requested_after: 0,
            oldest_available: 4,
            latest: 8,
        },
        RuntimeReplayGap {
            requested_after: 1,
            oldest_available: 2,
            latest: 8,
        },
        RuntimeReplayGap {
            requested_after: 1,
            oldest_available: 10,
            latest: 8,
        },
    ] {
        assert!(fixture.stream.record_replay_gap(gap).await.is_err());
    }
    let gap = RuntimeReplayGap {
        requested_after: 1,
        oldest_available: 4,
        latest: 8,
    };
    fixture.stream.record_replay_gap(gap).await.unwrap();
    fixture.stream.record_replay_gap(gap).await.unwrap();
    assert_eq!(
        fixture.stream.cursor().await.unwrap(),
        RuntimeCursor {
            sequence: 1,
            gap: Some(gap)
        }
    );
    assert!(matches!(
        fixture
            .stream
            .persist(event("second", 2), &[])
            .await
            .unwrap_err()
            .downcast_ref(),
        Some(RuntimeEventError::ReplayGap)
    ));
    assert_eq!(
        fixture
            .stream
            .persist(event("first", 1), &[])
            .await
            .unwrap(),
        RuntimePersistResult::Duplicate
    );
}

#[tokio::test]
async fn retained_receipts_prevent_resurrection_after_history_retention() {
    let fixture = Fixture::new().await;
    fixture
        .stream
        .persist(event("first", 1), &[history("first")])
        .await
        .unwrap();
    crate::cleanup_agent_event_history(&fixture.pool, 1, false, 100)
        .await
        .unwrap();
    assert_eq!(fixture.history_count().await, 0);
    assert_eq!(
        fixture
            .stream
            .persist(event("first", 1), &[history("first")])
            .await
            .unwrap(),
        RuntimePersistResult::Duplicate
    );
    assert_eq!(fixture.history_count().await, 0);
    assert_eq!(fixture.stream.cursor().await.unwrap().sequence, 1);
    let links: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_event_history")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(links, 0);
}

#[tokio::test]
async fn invalid_sequence_identity_and_projection_are_rejected_before_storage() {
    let fixture = Fixture::new().await;
    for invalid in [
        event("", 1),
        event("credential\nbody", 1),
        event("zero", 0),
        event("overflow", u64::MAX),
    ] {
        assert!(fixture.stream.persist(invalid, &[]).await.is_err());
    }
    let too_many: Vec<_> = (0..17).map(|_| history("projection")).collect();
    assert!(
        fixture
            .stream
            .persist(event("first", 1), &too_many)
            .await
            .is_err()
    );
    let oversized = vec![0; 2 * 1024 * 1024 + 1];
    assert!(
        fixture
            .stream
            .persist(
                event("first", 1),
                &[RuntimeHistoryEntry {
                    message: &oversized,
                    ..history("projection")
                }]
            )
            .await
            .is_err()
    );
    assert_eq!(fixture.history_count().await, 0);
    assert_eq!(fixture.stream.cursor().await.unwrap().sequence, 0);
}

#[tokio::test]
async fn replay_rewind_is_recorded_without_rewinding_the_committed_cursor() {
    let fixture = Fixture::new().await;
    fixture
        .stream
        .persist(event("first", 1), &[])
        .await
        .unwrap();
    let gap = RuntimeReplayGap {
        requested_after: 1,
        oldest_available: 1,
        latest: 0,
    };
    fixture.stream.record_replay_gap(gap).await.unwrap();
    assert_eq!(
        fixture.stream.cursor().await.unwrap(),
        RuntimeCursor {
            sequence: 1,
            gap: Some(gap)
        }
    );
    assert!(
        fixture
            .stream
            .persist(event("second", 2), &[])
            .await
            .is_err()
    );
}
