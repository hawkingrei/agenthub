use agenthub_agent_domain::loop_runtime::{LoopAdmission, LoopCleanupDisposition, LoopReservation};

use super::*;

async fn tasks(fixture: &Fixture) {
    for (id, team) in [
        ("task", "team"),
        ("next-task", "team"),
        ("foreign", "elsewhere"),
    ] {
        sqlx::query("INSERT INTO team_tasks(id, team_id, title, status, created_by_actor_id, context_json, created_at, updated_at) VALUES (?, ?, 'Review   migration', 'open', 'worker', ?, 1, 1)")
            .bind(id).bind(team).bind(serde_json::json!({"summary":"Check\n rollback", "private":"not a source"}).to_string())
            .execute(&fixture.store.pool).await.unwrap();
    }
}

async fn admit(fixture: &Fixture, task: &str, key: &str, now: i64) -> LoopReservation {
    let mut input = trigger(key);
    input.references.task_id = Some(task.into());
    let receipt = fixture.store.accept_trigger(&input, now).await.unwrap();
    let LoopAdmission::Admitted(reservation) = fixture
        .store
        .admit("team", &receipt.activation_id, "daemon", now)
        .await
        .unwrap()
    else {
        panic!("not admitted");
    };
    reservation
}

#[tokio::test]
async fn loop_task_context_persists_prefix_across_reopen_follow_up_and_title_changes() {
    let mut fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    tasks(&fixture).await;
    let first = admit(&fixture, "task", "first", 100).await;
    let context = fixture
        .store
        .pin_task_context(&first, "task", 101)
        .await
        .unwrap();
    assert_eq!(context.title, "Review migration");
    assert_eq!(context.summary.as_deref(), Some("Check rollback"));
    assert!(context.memory_prefix.starts_with("task-memory-v1:"));
    assert!(
        !serde_json::to_string(&context)
            .unwrap()
            .contains("not a source")
    );
    fixture
        .store
        .cleanup_verified(&first, LoopCleanupDisposition::Exited, 102)
        .await
        .unwrap();
    fixture.store.pool.close().await;
    fixture.store = LoopStore::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    migrate_loop_runtime(&fixture.store.pool).await.unwrap();
    sqlx::query("UPDATE team_tasks SET title = 'Clarified migration', context_json = '{\"summary\":\"Use current schema\"}' WHERE id = 'task'")
        .execute(&fixture.store.pool).await.unwrap();
    let follow_up = admit(&fixture, "task", "reply", 103).await;
    let current = fixture
        .store
        .pin_task_context(&follow_up, "task", 104)
        .await
        .unwrap();
    assert_eq!(current.memory_prefix, context.memory_prefix);
    assert_eq!(current.title, "Clarified migration");
    assert_eq!(current.summary.as_deref(), Some("Use current schema"));
    fixture
        .store
        .cleanup_verified(&follow_up, LoopCleanupDisposition::Exited, 105)
        .await
        .unwrap();
    let next = admit(&fixture, "next-task", "new", 106).await;
    let distinct = fixture
        .store
        .pin_task_context(&next, "next-task", 107)
        .await
        .unwrap();
    assert_ne!(distinct.memory_prefix, context.memory_prefix);
    sqlx::query("DELETE FROM team_tasks WHERE id = 'next-task'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    let retained: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM loop_task_memory_prefixes")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(retained, 1);
    fixture.close().await;
}

#[tokio::test]
async fn loop_task_context_rejects_unaddressed_revoked_foreign_and_stale_work() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    tasks(&fixture).await;
    let reservation = admit(&fixture, "task", "first", 100).await;
    for task in ["foreign", "next-task", "missing"] {
        assert!(
            fixture
                .store
                .pin_task_context(&reservation, task, 101)
                .await
                .is_err()
        );
    }
    let mut stale = reservation.clone();
    stale.generation += 1;
    assert!(
        fixture
            .store
            .pin_task_context(&stale, "task", 101)
            .await
            .is_err()
    );
    assert!(
        fixture
            .store
            .pin_task_context(&reservation, "task", 200)
            .await
            .is_err()
    );
    sqlx::query("INSERT INTO loop_revoked_sources(trigger_id, created_at) SELECT id, 101 FROM loop_trigger_sources WHERE activation_id = ?")
        .bind(&reservation.activation_id).execute(&fixture.store.pool).await.unwrap();
    assert!(
        fixture
            .store
            .pin_task_context(&reservation, "task", 101)
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM loop_task_memory_prefixes")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    fixture.close().await;
}
