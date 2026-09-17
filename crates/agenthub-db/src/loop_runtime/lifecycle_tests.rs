use agenthub_agent_domain::loop_runtime::{
    LoopAdmission, LoopCleanupDisposition, LoopContinuation, LoopOutcome, LoopOutcomeKind,
    LoopReservation, LoopWaitReason,
};

use super::*;

async fn admitted(fixture: &Fixture, key: &str, now: i64) -> LoopReservation {
    let trigger = fixture
        .store
        .accept_trigger(&trigger(key), now)
        .await
        .unwrap();
    let LoopAdmission::Admitted(reservation) = fixture
        .store
        .admit("team", &trigger.activation_id, "daemon", now)
        .await
        .unwrap()
    else {
        panic!("not admitted");
    };
    reservation
}

pub(super) async fn running(fixture: &Fixture, key: &str, now: i64) -> LoopReservation {
    let reservation = admitted(fixture, key, now).await;
    let session = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES (?, 'worker', 'running', ?)")
        .bind(&session).bind(now).execute(&fixture.store.pool).await.unwrap();
    let reservation = fixture
        .store
        .bind_session(&reservation, &session, now)
        .await
        .unwrap();
    fixture.store.mark_running(&reservation, now).await.unwrap();
    reservation
}

fn outcome() -> LoopOutcome {
    LoopOutcome {
        kind: LoopOutcomeKind::NoActionableWork,
        wait_reason: None,
        task_note_id: None,
        continuation: None,
    }
}

pub(super) async fn task_note(fixture: &Fixture, actor: &str, now: i64) -> i64 {
    sqlx::query("INSERT OR IGNORE INTO team_tasks(id, team_id, title, status, created_by_actor_id, context_json, created_at, updated_at) VALUES ('task', 'team', 'Work', 'in_progress', 'worker', '{}', 100, 100)")
        .execute(&fixture.store.pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO team_conversations(id, team_id, task_id, mode, created_at, updated_at) VALUES ('conversation', 'team', 'task', 'group', 100, 100)")
        .execute(&fixture.store.pool).await.unwrap();
    sqlx::query_scalar("INSERT INTO team_conversation_messages(conversation_id, task_id, from_actor_id, route, payload_json, created_at) VALUES ('conversation', 'task', ?, 'task_note', '{}', ?) RETURNING id")
        .bind(actor).bind(now).fetch_one(&fixture.store.pool).await.unwrap()
}

#[tokio::test]
async fn loop_progress_requires_new_canonical_evidence_without_accepting_the_task() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let current = running(&fixture, "first", 100).await;
    let note = task_note(&fixture, "worker", 100).await;
    let foreign = task_note(&fixture, "other", 100).await;
    let mut finish = outcome();
    finish.kind = LoopOutcomeKind::CompletionProposed;
    finish.task_note_id = Some(foreign);
    assert!(fixture.store.finish(&current, &finish, 101).await.is_err());
    finish.task_note_id = Some(note);
    fixture.store.finish(&current, &finish, 101).await.unwrap();
    assert_eq!(
        fixture
            .store
            .policy("team", "worker")
            .await
            .unwrap()
            .unwrap()
            .no_progress_count,
        0
    );
    let status: String = sqlx::query_scalar("SELECT status FROM team_tasks WHERE id = 'task'")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(status, "in_progress");
    fixture
        .store
        .cleanup_verified(&current, LoopCleanupDisposition::Exited, 101)
        .await
        .unwrap();
    // Same-second starts still cannot repeatedly credit the same note.
    let next = running(&fixture, "next", 100).await;
    fixture.store.finish(&next, &finish, 102).await.unwrap();
    assert_eq!(
        fixture
            .store
            .policy("team", "worker")
            .await
            .unwrap()
            .unwrap()
            .no_progress_count,
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_canceled_task_continuation_rolls_back_the_finish() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let current = running(&fixture, "first", 100).await;
    let note = task_note(&fixture, "worker", 101).await;
    sqlx::query("UPDATE team_tasks SET status = 'canceled' WHERE id = 'task'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    let mut finish = outcome();
    finish.task_note_id = Some(note);
    finish.continuation = Some(LoopContinuation {
        due_at: 105,
        task_id: Some("task".into()),
    });
    assert!(fixture.store.finish(&current, &finish, 102).await.is_err());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM loop_progress_receipts")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    finish.continuation = None;
    fixture.store.finish(&current, &finish, 103).await.unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn loop_admission_retires_task_continuation_canceled_after_finish() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let current = running(&fixture, "first", 100).await;
    task_note(&fixture, "worker", 101).await;
    let mut finish = outcome();
    finish.continuation = Some(LoopContinuation {
        due_at: 105,
        task_id: Some("task".into()),
    });
    let next = fixture
        .store
        .finish(&current, &finish, 102)
        .await
        .unwrap()
        .continuation
        .unwrap()
        .activation_id;
    fixture
        .store
        .cleanup_verified(&current, LoopCleanupDisposition::Exited, 103)
        .await
        .unwrap();
    sqlx::query("UPDATE team_tasks SET status = 'canceled' WHERE id = 'task'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .admit("team", &next, "daemon", 105)
            .await
            .unwrap(),
        LoopAdmission::NotPending
    );
    assert!(fixture.store.triggers("team", &next).await.unwrap()[0].revoked);
    assert_eq!(
        fixture
            .store
            .activation("team", &next)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopActivationState::Canceled
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_cancel_revokes_a_sole_continuation_without_deleting_its_trace() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let current = running(&fixture, "first", 100).await;
    let mut finish = outcome();
    finish.continuation = Some(LoopContinuation {
        due_at: 110,
        task_id: None,
    });
    let receipt = fixture.store.finish(&current, &finish, 101).await.unwrap();
    let next = receipt.continuation.unwrap().activation_id;
    fixture
        .store
        .cancel("team", current.activation_id.as_ref().unwrap(), 102)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .activation("team", &next)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopActivationState::Canceled
    );
    assert!(fixture.store.triggers("team", &next).await.unwrap()[0].revoked);
    assert!(
        !fixture
            .store
            .events("team", &next, 0, 100)
            .await
            .unwrap()
            .is_empty()
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_finish_keeps_writer_until_cleanup_and_replays_after_reopen() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let current = running(&fixture, "first", 100).await;
    let mut finish = outcome();
    finish.continuation = Some(LoopContinuation {
        due_at: 110,
        task_id: None,
    });
    let receipt = fixture.store.finish(&current, &finish, 101).await.unwrap();
    let next = &receipt.continuation.as_ref().unwrap().activation_id;
    assert_eq!(
        fixture
            .store
            .activation("team", current.activation_id.as_ref().unwrap())
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopActivationState::Finalizing
    );
    assert!(matches!(
        fixture
            .store
            .admit("team", next, "daemon", 110)
            .await
            .unwrap(),
        LoopAdmission::Deferred(_)
    ));
    assert_eq!(
        fixture.store.finish(&current, &finish, 102).await.unwrap(),
        receipt
    );
    fixture
        .store
        .cleanup_verified(&current, LoopCleanupDisposition::Exited, 103)
        .await
        .unwrap();
    fixture.store.pool.close().await;
    let reopened = LoopStore::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    assert_eq!(
        reopened.finish(&current, &finish, 1000).await.unwrap(),
        receipt
    );
    assert_eq!(
        reopened
            .policy("team", "worker")
            .await
            .unwrap()
            .unwrap()
            .no_progress_count,
        1
    );
    assert!(matches!(
        reopened
            .admit("team", next, "new-daemon", 1000)
            .await
            .unwrap(),
        LoopAdmission::Admitted(_)
    ));
    assert!(
        reopened
            .cleanup_verified(&current, LoopCleanupDisposition::Exited, 1001)
            .await
            .is_err()
    );
    reopened.pool.close().await;
    fixture.close().await;
}

#[tokio::test]
async fn loop_finish_rolls_back_outcome_and_progress_when_continuation_is_rejected() {
    let fixture = Fixture::new().await;
    fixture
        .enable(
            "worker",
            &LoopLimits {
                pending_per_actor: 1,
                ..LoopLimits::default()
            },
        )
        .await;
    let current = running(&fixture, "first", 100).await;
    fixture
        .store
        .accept_trigger(&trigger("pending"), 101)
        .await
        .unwrap();
    let mut finish = outcome();
    finish.continuation = Some(LoopContinuation {
        due_at: 200,
        task_id: None,
    });
    assert!(matches!(
        fixture
            .store
            .finish(&current, &finish, 102)
            .await
            .unwrap_err()
            .downcast_ref(),
        Some(LoopStoreError::Capacity)
    ));
    assert_eq!(
        fixture
            .store
            .activation("team", current.activation_id.as_ref().unwrap())
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopActivationState::Running
    );
    assert_eq!(
        fixture
            .store
            .policy("team", "worker")
            .await
            .unwrap()
            .unwrap()
            .no_progress_count,
        0
    );
    let receipts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM loop_finish_receipts")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(receipts, 0);
    fixture.close().await;
}

#[tokio::test]
async fn loop_restart_and_expiry_retain_uncertain_writer_and_reject_finish() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let current = running(&fixture, "first", 100).await;
    assert_eq!(fixture.store.interrupt_expired(159).await.unwrap(), 0);
    assert_eq!(fixture.store.interrupt_expired(160).await.unwrap(), 1);
    assert_eq!(fixture.store.interrupt_expired(161).await.unwrap(), 0);
    assert!(
        fixture
            .store
            .reservation("team", "worker")
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        fixture
            .store
            .finish(&current, &outcome(), 161)
            .await
            .is_err()
    );
    assert!(
        fixture
            .store
            .reserve_manual("team", "worker", "replacement", 161)
            .await
            .is_err()
    );
    fixture
        .store
        .cleanup_verified(&current, LoopCleanupDisposition::Exited, 162)
        .await
        .unwrap();
    let next = fixture
        .store
        .reserve_manual("team", "worker", "replacement", 162)
        .await
        .unwrap();
    assert!(next.generation > current.generation);
    assert!(
        fixture
            .store
            .cleanup_verified(&current, LoopCleanupDisposition::Exited, 163)
            .await
            .is_err()
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_startup_failure_retries_only_after_cleanup_with_durable_backoff() {
    let fixture = Fixture::new().await;
    fixture
        .enable(
            "worker",
            &LoopLimits {
                startup_attempts: 2,
                ..LoopLimits::default()
            },
        )
        .await;
    let first = admitted(&fixture, "first", 100).await;
    fixture
        .store
        .cleanup_verified(&first, LoopCleanupDisposition::StartupFailed, 101)
        .await
        .unwrap();
    let id = first.activation_id.as_ref().unwrap();
    assert_eq!(
        fixture
            .store
            .activation("team", id)
            .await
            .unwrap()
            .unwrap()
            .next_admission_at,
        102
    );
    assert!(matches!(
        fixture
            .store
            .admit("team", id, "daemon", 101)
            .await
            .unwrap(),
        LoopAdmission::Deferred(_)
    ));
    let LoopAdmission::Admitted(second) = fixture
        .store
        .admit("team", id, "daemon", 102)
        .await
        .unwrap()
    else {
        panic!("not admitted");
    };
    fixture
        .store
        .cleanup_verified(&second, LoopCleanupDisposition::StartupFailed, 103)
        .await
        .unwrap();
    let result = fixture.store.activation("team", id).await.unwrap().unwrap();
    assert_eq!(result.attempt_count, 2);
    assert_eq!(result.state, LoopActivationState::Interrupted);
    assert_eq!(fixture.store.triggers("team", id).await.unwrap().len(), 1);
    fixture.close().await;
}

#[tokio::test]
async fn loop_cancel_preserves_history_and_independent_coalesced_wake() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let current = running(&fixture, "first", 100).await;
    let mut finish = outcome();
    finish.continuation = Some(LoopContinuation {
        due_at: 110,
        task_id: None,
    });
    let receipt = fixture.store.finish(&current, &finish, 101).await.unwrap();
    let mut independent = trigger("independent");
    independent.due_at = Some(110);
    let wake = fixture
        .store
        .accept_trigger(&independent, 102)
        .await
        .unwrap();
    assert_eq!(
        wake.activation_id,
        receipt.continuation.unwrap().activation_id
    );
    fixture
        .store
        .cancel("team", current.activation_id.as_ref().unwrap(), 103)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .reservation("team", "worker")
            .await
            .unwrap()
            .is_some()
    );
    assert!(fixture.store.renew(&current, 104).await.is_err());
    assert_eq!(
        fixture
            .store
            .triggers("team", &wake.activation_id)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        fixture
            .store
            .activation("team", &wake.activation_id)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopActivationState::Pending
    );
    fixture
        .store
        .cleanup_verified(&current, LoopCleanupDisposition::Exited, 105)
        .await
        .unwrap();
    assert!(matches!(
        fixture
            .store
            .admit("team", &wake.activation_id, "daemon", 110)
            .await
            .unwrap(),
        LoopAdmission::Admitted(_)
    ));
    fixture.close().await;
}

#[tokio::test]
async fn loop_finish_rejects_conflicting_or_wrong_owner_replay_and_invalid_wait() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let current = running(&fixture, "first", 100).await;
    let mut waiting = outcome();
    waiting.kind = LoopOutcomeKind::Waiting;
    assert!(fixture.store.finish(&current, &waiting, 101).await.is_err());
    waiting.wait_reason = Some(LoopWaitReason::Knowledge);
    fixture.store.finish(&current, &waiting, 101).await.unwrap();
    assert!(
        fixture
            .store
            .finish(&current, &outcome(), 102)
            .await
            .is_err()
    );
    let mut stale = current.clone();
    stale.owner_id = "someone-else".into();
    assert!(fixture.store.finish(&stale, &waiting, 102).await.is_err());
    fixture.close().await;
}

#[tokio::test]
async fn loop_finish_and_concurrent_wakes_preserve_every_source() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let current = running(&fixture, "first", 100).await;
    let before = fixture
        .store
        .accept_trigger(&trigger("before"), 101)
        .await
        .unwrap();
    let finish = outcome();
    let during = trigger("during");
    let (finished, wake) = tokio::join!(
        fixture.store.finish(&current, &finish, 102),
        fixture.store.accept_trigger(&during, 102)
    );
    finished.unwrap();
    assert_eq!(wake.unwrap().activation_id, before.activation_id);
    fixture
        .store
        .cleanup_verified(&current, LoopCleanupDisposition::Exited, 103)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .accept_trigger(&trigger("after"), 104)
            .await
            .unwrap()
            .activation_id,
        before.activation_id
    );
    assert_eq!(
        fixture
            .store
            .triggers("team", &before.activation_id)
            .await
            .unwrap()
            .len(),
        3
    );
    fixture.close().await;
}
