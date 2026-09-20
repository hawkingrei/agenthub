use agenthub_agent_domain::{
    loop_metrics::LoopWaitMetricKind,
    loop_runtime::{
        LoopAdmission, LoopCleanupDisposition, LoopEventKind, LoopExitReason, LoopOutcome,
        LoopOutcomeKind, LoopWaitReason,
    },
    loop_scheduling::{LoopRegistrationInput, LoopSchedule},
};

use super::{
    lifecycle_tests::{running, task_note},
    *,
};

#[tokio::test]
async fn loop_metrics_duplicate_counts_are_atomic_bounded_and_do_not_expand_the_event_log() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let input = trigger("private-duplicate-key");
    let accepted = fixture.store.accept_trigger(&input, 100).await.unwrap();
    let mut joins = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let store = fixture.store.clone();
        let input = input.clone();
        joins.spawn(async move { store.accept_trigger(&input, 101).await.unwrap() });
    }
    while let Some(receipt) = joins.join_next().await {
        let receipt = receipt.unwrap();
        assert!(receipt.duplicate);
        assert_eq!(receipt.activation_id, accepted.activation_id);
    }
    let mut tx = fixture
        .store
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap();
    LoopStore::accept_in_transaction(&mut tx, &input, 102)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    let mut conflicting = input.clone();
    conflicting.due_at = Some(200);
    assert!(
        fixture
            .store
            .accept_trigger(&conflicting, 103)
            .await
            .is_err()
    );
    let metrics = fixture
        .store
        .metrics("team", "worker", 110, 100)
        .await
        .unwrap();
    assert_eq!(metrics.duplicates.suppressed_total, 8);
    assert_eq!(metrics.duplicates.sources_with_unknown_baseline, 0);
    assert_eq!(
        fixture
            .store
            .events("team", &accepted.activation_id, 0, 100)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        !serde_json::to_string(&metrics)
            .unwrap()
            .contains("private-duplicate-key")
    );
    sqlx::query("UPDATE loop_trigger_sources SET duplicate_count = ? WHERE id = ?")
        .bind(i64::MAX)
        .bind(&accepted.trigger_id)
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    fixture.store.accept_trigger(&input, 111).await.unwrap();
    assert_eq!(
        fixture
            .store
            .metrics("team", "worker", 120, 100)
            .await
            .unwrap()
            .duplicates
            .suppressed_total,
        i64::MAX
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_metrics_measure_scoped_queue_runs_progress_waits_retries_and_mem() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let first = running(&fixture, "first", 100).await;
    let note = task_note(&fixture, "worker", 101).await;
    fixture
        .store
        .finish(
            &first,
            &LoopOutcome {
                kind: LoopOutcomeKind::Progress,
                wait_reason: None,
                task_note_id: Some(note),
                continuation: None,
            },
            101,
        )
        .await
        .unwrap();
    fixture
        .store
        .cleanup_verified(&first, LoopCleanupDisposition::Exited, 105)
        .await
        .unwrap();
    let second = fixture
        .store
        .accept_trigger(&trigger("retry"), 110)
        .await
        .unwrap();
    let LoopAdmission::Admitted(starting) = fixture
        .store
        .admit("team", &second.activation_id, "daemon", 115)
        .await
        .unwrap()
    else {
        panic!("not admitted")
    };
    fixture
        .store
        .cleanup_verified(&starting, LoopCleanupDisposition::StartupFailed, 116)
        .await
        .unwrap();
    let due = fixture
        .store
        .activation("team", &second.activation_id)
        .await
        .unwrap()
        .unwrap()
        .next_admission_at;
    let LoopAdmission::Admitted(retry) = fixture
        .store
        .admit("team", &second.activation_id, "daemon", due)
        .await
        .unwrap()
    else {
        panic!("retry not admitted")
    };
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES ('retry-session', 'worker', 'running', ?)")
        .bind(due).execute(&fixture.store.pool).await.unwrap();
    let retry = fixture
        .store
        .bind_session(&retry, "retry-session", due)
        .await
        .unwrap();
    fixture.store.mark_running(&retry, due).await.unwrap();
    fixture
        .store
        .finish(
            &retry,
            &LoopOutcome {
                kind: LoopOutcomeKind::Waiting,
                wait_reason: Some(LoopWaitReason::Dependency),
                task_note_id: None,
                continuation: None,
            },
            due + 1,
        )
        .await
        .unwrap();
    fixture
        .store
        .cleanup_verified(&retry, LoopCleanupDisposition::Exited, due + 3)
        .await
        .unwrap();
    fixture
        .store
        .register_schedule(
            &LoopRegistrationInput {
                actor_id: "worker".into(),
                team_id: "team".into(),
                source_key: "private-wait".into(),
                schedule: LoopSchedule::Due { due_at: 350 },
                work_task_id: None,
                references: LoopSourceReferences::default(),
            },
            140,
        )
        .await
        .unwrap();
    let mut future = trigger("future");
    future.due_at = Some(1000);
    fixture.store.accept_trigger(&future, 180).await.unwrap();
    fixture
        .store
        .accept_trigger(&trigger("due"), 190)
        .await
        .unwrap();
    for (id, generation, kind, time) in [
        (
            first.activation_id.as_deref().unwrap(),
            first.generation,
            "mem_context_ready",
            100,
        ),
        (
            second.activation_id.as_str(),
            retry.generation,
            "mem_context_unavailable",
            due + 1,
        ),
    ] {
        sqlx::query("INSERT INTO loop_activation_events(activation_id, kind, generation, created_at) VALUES (?, ?, ?, ?)")
            .bind(id).bind(kind).bind(generation).bind(time).execute(&fixture.store.pool).await.unwrap();
    }
    let result = fixture
        .store
        .metrics("team", "worker", 200, 100)
        .await
        .unwrap();
    assert_eq!(result.pending.count, 2);
    assert_eq!(result.pending.oldest_age_seconds, Some(20));
    assert_eq!(result.pending.due_count, 1);
    assert_eq!(result.pending.oldest_due_age_seconds, Some(10));
    assert_eq!(result.admission_latency.samples, 2);
    assert_eq!(result.admission_latency.total_seconds, 5);
    assert_eq!(result.running_duration.samples, 2);
    assert_eq!(result.running_duration.total_seconds, 8);
    assert_eq!(result.running_duration.maximum_seconds, Some(5));
    assert_eq!(result.startup_failures, 1);
    assert_eq!(result.retries, 1);
    assert_eq!(result.progress.finalized_activations, 2);
    assert_eq!(result.progress.no_progress_activations, 1);
    assert_eq!(result.progress.current_no_progress_streak, Some(1));
    assert_eq!(
        result
            .exits
            .iter()
            .find(|entry| entry.kind == LoopExitReason::OutcomeRecorded)
            .unwrap()
            .count,
        2
    );
    assert_eq!(
        result
            .outcomes
            .iter()
            .find(|entry| entry.kind == LoopOutcomeKind::Progress)
            .unwrap()
            .count,
        1
    );
    assert_eq!(
        result.current_wait.as_ref().unwrap().age_seconds,
        Some(200 - due - 1)
    );
    assert_eq!(
        result.current_wait.as_ref().unwrap().reason,
        LoopWaitReason::Dependency
    );
    assert_eq!(
        result
            .waits
            .iter()
            .find(|entry| entry.kind == LoopWaitMetricKind::Due)
            .unwrap()
            .oldest_age_seconds,
        Some(60)
    );
    assert_eq!(
        result.mem.latest.as_ref().unwrap().kind,
        LoopEventKind::MemContextUnavailable
    );
    assert_eq!(result.mem.observations.len(), 2);
    assert_eq!(result.unsettled_run_count, 0);
    let recent = fixture
        .store
        .metrics("team", "worker", 200, 1)
        .await
        .unwrap();
    assert!(recent.outcomes.is_empty());
    assert_eq!(recent.pending, result.pending);
    assert_eq!(recent.current_wait, result.current_wait);
    for (team, actor) in [("elsewhere", "worker"), ("team", "other")] {
        let other = fixture.store.metrics(team, actor, 200, 100).await.unwrap();
        assert_eq!(other.pending.count, 0);
        assert!(
            other.outcomes.is_empty() && other.mem.latest.is_none() && other.current_wait.is_none()
        );
    }
    fixture.store.pool.close().await;
    let reopened = LoopStore::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    assert_eq!(
        reopened.metrics("team", "worker", 200, 100).await.unwrap(),
        result
    );
    reopened.pool.close().await;
    fixture.close().await;
}

#[tokio::test]
async fn loop_metrics_keep_clock_regressions_and_unfenced_runs_distinct_from_completed_work() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let first = running(&fixture, "clock", 100).await;
    fixture
        .store
        .cleanup_verified(&first, LoopCleanupDisposition::Exited, 99)
        .await
        .unwrap();
    let expired = running(&fixture, "expired", 120).await;
    fixture
        .store
        .interrupt_expired(expired.lease_expires_at)
        .await
        .unwrap();
    let result = fixture
        .store
        .metrics("team", "worker", expired.lease_expires_at, 1000)
        .await
        .unwrap();
    assert_eq!(result.running_duration.clock_regressions, 1);
    assert_eq!(result.running_duration.samples, 0);
    assert_eq!(result.running_duration.maximum_seconds, None);
    assert_eq!(result.unsettled_run_count, 1);
    assert_eq!(
        result.oldest_unsettled_run_age_seconds,
        Some(expired.lease_expires_at - 120)
    );
    assert_eq!(result.exits.iter().map(|entry| entry.count).sum::<i64>(), 1);
    assert_eq!(result.progress.finalized_activations, 1);
    assert_eq!(result.progress.no_progress_activations, 1);
    for (now, window) in [(-1, 10), (100, 0), (100, 604801)] {
        assert!(
            fixture
                .store
                .metrics("team", "worker", now, window)
                .await
                .unwrap_err()
                .downcast_ref::<LoopStoreError>()
                .is_some_and(|error| matches!(error, LoopStoreError::InvalidHistoryQuery))
        );
    }
    fixture.close().await;
}

#[tokio::test]
async fn loop_metrics_migration_preserves_unknown_historical_coverage() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let first = running(&fixture, "legacy", 100).await;
    fixture
        .store
        .cleanup_verified(&first, LoopCleanupDisposition::Exited, 101)
        .await
        .unwrap();
    for statement in [
        "ALTER TABLE loop_trigger_sources DROP COLUMN duplicate_count",
        "ALTER TABLE loop_trigger_sources DROP COLUMN duplicate_observation_started_at",
        "ALTER TABLE loop_activation_events DROP COLUMN exit_reason_code",
    ] {
        sqlx::query(statement)
            .execute(&fixture.store.pool)
            .await
            .unwrap();
    }
    migrate_loop_runtime(&fixture.store.pool).await.unwrap();
    migrate_loop_runtime(&fixture.store.pool).await.unwrap();
    fixture
        .store
        .accept_trigger(&trigger("legacy"), 102)
        .await
        .unwrap();
    fixture
        .store
        .accept_trigger(&trigger("new"), 103)
        .await
        .unwrap();
    let result = fixture
        .store
        .metrics("team", "worker", 200, 200)
        .await
        .unwrap();
    assert_eq!(result.duplicates.suppressed_total, 1);
    assert_eq!(result.duplicates.sources_with_unknown_baseline, 1);
    assert_eq!(result.exits_without_reason, 1);
    assert!(result.exits.is_empty());
    let events = fixture
        .store
        .events("team", first.activation_id.as_deref().unwrap(), 0, 100)
        .await
        .unwrap();
    assert!(events.iter().all(|event| event.exit_reason.is_none()));
    fixture.close().await;
}

#[tokio::test]
async fn loop_metrics_wait_age_restarts_after_a_recurring_condition_fires() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    fixture
        .store
        .register_schedule(
            &LoopRegistrationInput {
                actor_id: "worker".into(),
                team_id: "team".into(),
                source_key: "recurring".into(),
                schedule: LoopSchedule::Recurring {
                    first_at: 110,
                    interval_seconds: 10,
                },
                work_task_id: None,
                references: LoopSourceReferences::default(),
            },
            100,
        )
        .await
        .unwrap();
    fixture.store.reconcile_schedules(110).await.unwrap();
    let metrics = fixture
        .store
        .metrics("team", "worker", 115, 100)
        .await
        .unwrap();
    let wait = metrics
        .waits
        .iter()
        .find(|entry| entry.kind == LoopWaitMetricKind::Recurring)
        .unwrap();
    assert_eq!(wait.count, 1);
    assert_eq!(wait.oldest_age_seconds, Some(5));
    fixture.close().await;
}
