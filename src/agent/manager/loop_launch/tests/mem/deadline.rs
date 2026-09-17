use crate::mcp_proxy::context::MemContext;
use agenthub_agent_domain::loop_runtime::{LoopCleanupDisposition, LoopEventKind};

use super::*;

pub(super) async fn consumer_timeout_keeps_sent_read_owned_until_factual_completion() {
    let (fixture, upstream, server) = fixture("deadline").await;
    let reservation = fixture.admit("context-deadline").await;
    let store = LoopStore::new(fixture.state.db.clone());
    let mailbox = fixture
        .state
        .teams
        .ensure_loop_mailbox_partition(&fixture.team_id)
        .await
        .unwrap();
    let now = Utc::now().timestamp();
    store
        .bind_mailbox(&reservation, &mailbox.id, now)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES ('context-test', 'worker', 'running', ?)")
        .bind(now).execute(&fixture.state.db).await.unwrap();
    let reservation = store
        .bind_session(&reservation, "context-test", now)
        .await
        .unwrap();
    fixture
        .state
        .agents
        .loop_reservations
        .lock()
        .await
        .insert("worker".into(), reservation.clone());
    store.mark_running(&reservation, now).await.unwrap();
    let binding = crate::mcp_proxy::configured::resolve_mem(
        &fixture.state.agents.loop_app_config,
        &fixture.team_id,
        "worker",
        |key| std::env::var(key).ok(),
    )
    .await
    .unwrap();
    let gate = fixture.state.agents.loop_operation_gate("worker").await;
    {
        let _guard = gate.clone().read_owned().await;
        fixture
            .state
            .agents
            .mcp_proxy()
            .unwrap()
            .mount(&reservation, binding.binding)
            .await
            .unwrap();
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    let read = fixture
        .state
        .agents
        .mem_context_until(&reservation, "space-a".into(), deadline);
    let arrived = tokio::time::timeout(Duration::from_secs(3), upstream.received.notified());
    let (context, arrived) = tokio::join!(read, arrived);
    arrived.expect("the admitted read reached the upstream before the consumer timeout");
    assert!(matches!(context.unwrap(), MemContext::Unavailable));
    assert!(
        gate.clone().try_write_owned().is_err(),
        "cleanup cannot overtake the admitted read"
    );
    let local_guard = gate
        .clone()
        .try_read_owned()
        .expect("independent execution remains admissible");
    store
        .record_mem_context(
            &reservation,
            LoopEventKind::MemContextUnavailable,
            Utc::now().timestamp(),
        )
        .await
        .unwrap();
    let task_id = std::fs::read_to_string(fixture.directory.join("local-task-id")).unwrap();
    fixture.state.teams.append_task_conversation_message(&task_id, "worker", None, "task_note",
        json!({"type":"task_note","kind":"result","text":"Progress survives the context deadline"})).await.unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM mcp_operations")
        .fetch_one(&fixture.state.db)
        .await
        .unwrap();
    assert_eq!(status, "sent");
    drop(local_guard);
    upstream.release.notify_one();
    let cleanup_guard = tokio::time::timeout(Duration::from_secs(5), gate.clone().write_owned())
        .await
        .unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM mcp_operations")
        .fetch_one(&fixture.state.db)
        .await
        .unwrap();
    assert_eq!(
        status, "succeeded",
        "late success is factual even though its consumer timed out"
    );
    let events = store
        .events(
            &fixture.team_id,
            reservation.activation_id.as_deref().unwrap(),
            0,
            100,
        )
        .await
        .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == LoopEventKind::MemContextUnavailable)
            .count(),
        1
    );
    assert!(
        !events
            .iter()
            .any(|event| event.kind == LoopEventKind::MemContextReady)
    );
    let notes = fixture
        .state
        .teams
        .list_task_notes(&task_id, 10)
        .await
        .unwrap();
    assert!(
        notes
            .iter()
            .any(|note| note.text == "Progress survives the context deadline")
    );
    drop(cleanup_guard);
    store
        .cancel(
            &fixture.team_id,
            reservation.activation_id.as_deref().unwrap(),
            Utc::now().timestamp(),
        )
        .await
        .unwrap();
    fixture
        .state
        .agents
        .release_loop_after_cleanup(
            "worker",
            Some("context-test"),
            LoopCleanupDisposition::Exited,
        )
        .await
        .unwrap();
    fixture.close().await;
    server.abort();
}
