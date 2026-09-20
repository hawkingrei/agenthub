use super::*;
use agenthub_db::{AgentEventDbRouter, runtime_events::RuntimeEventStore};
use agenthub_rara::RuntimeEvent;
use serde_json::{Value, json};
use sqlx::SqlitePool;

struct Fixture {
    directory: std::path::PathBuf,
    pool: SqlitePool,
    stream: RuntimeEventStream,
    consumer: DurableEvents,
}

impl Fixture {
    async fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("direct-consumer-{}", Uuid::now_v7()));
        let pool = AgentEventDbRouter::new(directory.clone())
            .pool_for_agent("actor")
            .await
            .unwrap();
        let store = RuntimeEventStore::bind(pool.clone(), "local", "runtime")
            .await
            .unwrap();
        let stream = store.bind_stream("native").await.unwrap();
        let consumer = DurableEvents::new(stream.clone(), "runtime", "actor", "local")
            .await
            .unwrap();
        Self {
            directory,
            pool,
            stream,
            consumer,
        }
    }

    async fn count(&self) -> i64 {
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

fn frame(sequence: u64, text: &str) -> EventFrame {
    EventFrame {
        runtime_id: "runtime".into(),
        session_id: "native".into(),
        event: RuntimeEvent {
            event_id: format!("event-{sequence}"),
            sequence,
            turn_id: Some("turn".into()),
            provenance: json!({}),
            event: json!({"type":"assistant","payload":{"type":"text_delta","payload":text}}),
        },
    }
}

#[tokio::test]
async fn reordered_replay_commits_contiguous_history_and_suppresses_duplicates() {
    let mut fixture = Fixture::new().await;
    fixture.consumer.enqueue(frame(3, "third")).await.unwrap();
    assert!(fixture.consumer.commit_next().await.unwrap().is_none());
    assert!(fixture.consumer.missing_prefix());
    assert_eq!(fixture.count().await, 0);
    for (sequence, text) in [(1, "first"), (2, "second")] {
        fixture
            .consumer
            .enqueue(frame(sequence, text))
            .await
            .unwrap();
        let committed = fixture.consumer.commit_next().await.unwrap().unwrap();
        assert_eq!(committed.sequence, sequence);
        assert_eq!(committed.output.len(), 1);
        assert!(committed.output[0].event_id > 0);
        let history: Value = serde_json::from_str(&committed.output[0].message).unwrap();
        assert_eq!(history["chunk_index"], sequence - 1);
        assert_eq!(fixture.count().await, sequence as i64);
    }
    let third = fixture.consumer.commit_next().await.unwrap().unwrap();
    let history: Value = serde_json::from_str(&third.output[0].message).unwrap();
    assert_eq!(history["chunk_index"], 2);
    fixture.consumer.enqueue(frame(2, "second")).await.unwrap();
    assert!(fixture.consumer.commit_next().await.unwrap().is_none());
    assert_eq!(fixture.count().await, 3);
    assert_eq!(fixture.stream.cursor().await.unwrap().sequence, 3);
    assert!(fixture.consumer.enqueue(frame(2, "changed")).await.is_err());
}

#[tokio::test]
async fn failed_transaction_retains_event_and_projection_for_retry() {
    let mut fixture = Fixture::new().await;
    fixture.consumer.enqueue(frame(1, "first")).await.unwrap();
    fixture.consumer.commit_next().await.unwrap().unwrap();
    sqlx::query("CREATE TRIGGER fail_history BEFORE INSERT ON agent_events BEGIN SELECT RAISE(ABORT, 'fixture failure'); END")
        .execute(&fixture.pool).await.unwrap();
    fixture.consumer.enqueue(frame(2, "retry")).await.unwrap();
    assert!(fixture.consumer.commit_next().await.is_err());
    assert_eq!(fixture.consumer.sequence(), 1);
    assert_eq!(fixture.count().await, 1);
    assert_eq!(fixture.stream.cursor().await.unwrap().sequence, 1);
    sqlx::query("DROP TRIGGER fail_history")
        .execute(&fixture.pool)
        .await
        .unwrap();
    let committed = fixture.consumer.commit_next().await.unwrap().unwrap();
    let history: Value = serde_json::from_str(&committed.output[0].message).unwrap();
    assert_eq!(history["chunk_index"], 1);
    assert_eq!(fixture.count().await, 2);
}

#[tokio::test]
async fn event_scope_buffer_conflicts_and_reopen_do_not_allocate_authority() {
    let mut fixture = Fixture::new().await;
    let mut foreign = frame(1, "foreign");
    foreign.session_id = "foreign".into();
    assert!(fixture.consumer.enqueue(foreign).await.is_err());
    fixture
        .consumer
        .enqueue(frame(2, "buffered"))
        .await
        .unwrap();
    assert!(fixture.consumer.enqueue(frame(2, "changed")).await.is_err());
    fixture.consumer.enqueue(frame(1, "first")).await.unwrap();
    fixture.consumer.commit_next().await.unwrap().unwrap();
    assert!(
        DurableEvents::new(fixture.stream.clone(), "runtime", "actor", "local")
            .await
            .is_err()
    );
    assert_eq!(fixture.count().await, 1);
}

#[tokio::test]
async fn reorder_capacity_is_bounded_without_advancing_past_missing_history() {
    let mut fixture = Fixture::new().await;
    for sequence in 2..MAX_REORDER_EVENTS as u64 + 2 {
        fixture
            .consumer
            .enqueue(frame(sequence, "waiting"))
            .await
            .unwrap();
    }
    assert!(
        fixture
            .consumer
            .enqueue(frame(999, "overflow"))
            .await
            .is_err()
    );
    assert!(fixture.consumer.commit_next().await.unwrap().is_none());
    assert_eq!(fixture.stream.cursor().await.unwrap().sequence, 0);
    assert_eq!(fixture.count().await, 0);
}

#[tokio::test]
async fn tool_replay_keeps_one_card_and_one_result_across_approval_handoff() {
    let mut fixture = Fixture::new().await;
    let payloads = [
        (
            "tool",
            "use",
            json!({"call_id":"call", "name":"bash", "input":{}}),
        ),
        (
            "input",
            "requested",
            json!({"pending":{"turn_id":"turn", "kind":{"type":"shell","payload":{"approval_id":"call","request":{"command":"echo fixture"}}}}}),
        ),
        ("input", "answered", json!({"waiting_turn":"turn"})),
        ("session", "turn_started", Value::Null),
        (
            "approval",
            "answered",
            json!({"approval_id":"call", "approved":true}),
        ),
        (
            "tool",
            "use",
            json!({"call_id":"call", "name":"bash", "input":{}}),
        ),
        (
            "tool",
            "result",
            json!({"call_id":"call", "name":"bash", "content":"done", "is_error":false}),
        ),
    ];
    let mut committed = Vec::new();
    let mut frames = Vec::new();
    for (index, (family, kind, payload)) in payloads.into_iter().enumerate() {
        let mut event = frame(index as u64 + 1, "");
        event.event.event = json!({"type":family,"payload":{"type":kind,"payload":payload}});
        if index >= 2 {
            event.event.turn_id = Some("answer-turn".into());
        }
        fixture.consumer.enqueue(event.clone()).await.unwrap();
        committed.extend(
            fixture
                .consumer
                .commit_next()
                .await
                .unwrap()
                .unwrap()
                .output,
        );
        frames.push(event);
    }
    let count = fixture.count().await;
    for event in frames.into_iter().rev() {
        fixture.consumer.enqueue(event).await.unwrap();
        assert!(fixture.consumer.commit_next().await.unwrap().is_none());
    }
    assert_eq!(fixture.count().await, count);
    assert_eq!(fixture.stream.cursor().await.unwrap().sequence, 7);
    let history: Vec<Value> = committed
        .iter()
        .map(|entry| serde_json::from_str(&entry.message).unwrap())
        .collect();
    let calls: Vec<_> = history
        .iter()
        .filter(|entry| entry["type"] == "tool_call")
        .collect();
    assert_eq!(calls.len(), 1);
    let completed: Vec<_> = history
        .iter()
        .filter(|entry| entry["type"] == "tool_call_update" && entry["status"] == "completed")
        .collect();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0]["id"], calls[0]["id"]);
}
