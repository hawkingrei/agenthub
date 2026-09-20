use agenthub_agent_domain::loop_runtime::{
    LoopAdmission, LoopCleanupDisposition, LoopDeferralReason, LoopOutcome, LoopOutcomeKind,
    LoopReservation,
};
use agenthub_agent_domain::loop_scheduling::{
    LoopRegistrationInput, LoopRegistrationState, LoopSchedule,
};

use super::*;

fn registration(key: &str, schedule: LoopSchedule) -> LoopRegistrationInput {
    LoopRegistrationInput {
        actor_id: "worker".into(),
        team_id: "team".into(),
        source_key: key.into(),
        schedule,
        work_task_id: None,
        references: LoopSourceReferences::default(),
    }
}

fn dependency(key: &str, repeat: bool) -> LoopRegistrationInput {
    registration(
        key,
        LoopSchedule::TaskStatus {
            task_id: "dependency".into(),
            statuses: vec!["completed".parse().unwrap()],
            repeat,
        },
    )
}

async fn task(fixture: &Fixture, id: &str, status: &str) {
    sqlx::query("INSERT INTO team_tasks(id, team_id, title, status, created_by_actor_id, context_json, created_at, updated_at) VALUES (?, 'team', 'Work', ?, 'worker', '{}', 100, 100)")
        .bind(id).bind(status).execute(&fixture.store.pool).await.unwrap();
}

async fn set_status(fixture: &Fixture, id: &str, status: &str, now: i64) {
    let mut tx = fixture
        .store
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap();
    sqlx::query(
        "UPDATE team_tasks SET status = ?, updated_at = MAX(updated_at + 1, ?) WHERE id = ?",
    )
    .bind(status)
    .bind(now)
    .bind(id)
    .execute(&mut *tx)
    .await
    .unwrap();
    LoopStore::observe_task_schedule_tx(&mut tx, "team", id, now)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

async fn conversation(fixture: &Fixture) -> i64 {
    task(fixture, "discussion", "open").await;
    sqlx::query("INSERT INTO team_conversations(id, team_id, task_id, mode, created_at, updated_at) VALUES ('discussion', 'team', 'discussion', 'group', 100, 100)")
        .execute(&fixture.store.pool).await.unwrap();
    reply(fixture, None, "other", 100).await
}

async fn reply(fixture: &Fixture, root: Option<i64>, author: &str, now: i64) -> i64 {
    let mut tx = fixture
        .store
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap();
    let id: i64 = sqlx::query_scalar("INSERT INTO team_conversation_messages(conversation_id, task_id, from_actor_id, route, payload_json, thread_root_message_id, created_at) VALUES ('discussion', 'discussion', ?, 'group_chat', '{}', ?, ?) RETURNING id")
        .bind(author).bind(root).bind(now).fetch_one(&mut *tx).await.unwrap();
    LoopStore::observe_thread_schedule_tx(&mut tx, "team", id, now)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    id
}

async fn running(fixture: &Fixture, activation_id: &str, now: i64) -> LoopReservation {
    let LoopAdmission::Admitted(reservation) = fixture
        .store
        .admit("team", activation_id, "daemon", now)
        .await
        .unwrap()
    else {
        panic!("expected admission");
    };
    let session = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES (?, ?, 'running', ?)",
    )
    .bind(&session)
    .bind(&reservation.actor_id)
    .bind(now)
    .execute(&fixture.store.pool)
    .await
    .unwrap();
    let reservation = fixture
        .store
        .bind_session(&reservation, &session, now)
        .await
        .unwrap();
    fixture.store.mark_running(&reservation, now).await.unwrap();
    reservation
}

#[tokio::test]
async fn loop_schedule_catches_up_once_after_reopen_and_preserves_firing_history() {
    let mut fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let input = registration(
        "clock",
        LoopSchedule::Recurring {
            first_at: 110,
            interval_seconds: 10,
        },
    );
    let created = fixture.store.register_schedule(&input, 100).await.unwrap();
    assert!(
        fixture
            .store
            .reconcile_schedules(109)
            .await
            .unwrap()
            .is_empty()
    );
    fixture.store.pool.close().await;
    fixture.store = LoopStore::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    let firings = fixture.store.reconcile_schedules(155).await.unwrap();
    assert_eq!(firings.len(), 1);
    assert_eq!(
        (firings[0].first_cursor, firings[0].through_cursor),
        (110, 150)
    );
    assert!(
        fixture
            .store
            .reconcile_schedules(155)
            .await
            .unwrap()
            .is_empty()
    );
    let stored = fixture
        .store
        .registration("team", &created.registration.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.next_due_at, Some(160));
    let duplicate = fixture.store.register_schedule(&input, 159).await.unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.registration, stored);
    let next = fixture.store.reconcile_schedules(160).await.unwrap();
    assert_eq!(next.len(), 1);
    assert_ne!(next[0].receipt.trigger_id, firings[0].receipt.trigger_id);
    assert_eq!(
        next[0].receipt.activation_id,
        firings[0].receipt.activation_id
    );
    let history = fixture
        .store
        .registration_firings("team", &stored.id, None, 1)
        .await
        .unwrap();
    assert_eq!(history, firings);
    assert_eq!(
        fixture
            .store
            .registration_firings("team", &stored.id, Some(110), 1)
            .await
            .unwrap(),
        next
    );
    assert!(
        fixture
            .store
            .registration_firings("elsewhere", &stored.id, None, 1)
            .await
            .unwrap()
            .is_empty()
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_schedule_capacity_preserves_the_latch_without_partial_intake() {
    let fixture = Fixture::new().await;
    let limits = LoopLimits {
        sources_per_activation: 1,
        pending_per_actor: 1,
        ..LoopLimits::default()
    };
    fixture.enable("worker", &limits).await;
    let occupied = fixture
        .store
        .accept_trigger(&trigger("occupied"), 100)
        .await
        .unwrap();
    let created = fixture
        .store
        .register_schedule(&registration("due", LoopSchedule::Due { due_at: 101 }), 100)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .reconcile_schedules(101)
            .await
            .unwrap()
            .is_empty()
    );
    let pending = fixture
        .store
        .registration("team", &created.registration.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending.pending_cursor, Some(101));
    assert_eq!(pending.next_check_at, 106);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM loop_trigger_sources")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    fixture
        .store
        .cancel("team", &occupied.activation_id, 102)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .reconcile_schedules(105)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        fixture.store.reconcile_schedules(106).await.unwrap().len(),
        1
    );
    assert_eq!(
        fixture
            .store
            .registration("team", &pending.id)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopRegistrationState::Completed
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_schedule_task_edges_survive_reversion_and_ignore_unchanged_conditions() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    task(&fixture, "dependency", "open").await;
    let created = fixture
        .store
        .register_schedule(&dependency("watch", true), 100)
        .await
        .unwrap();
    set_status(&fixture, "dependency", "completed", 101).await;
    set_status(&fixture, "dependency", "open", 102).await;
    let firings = fixture.store.reconcile_schedules(103).await.unwrap();
    assert_eq!(firings.len(), 1);
    assert_eq!(
        (firings[0].first_cursor, firings[0].through_cursor),
        (101, 102)
    );
    set_status(&fixture, "dependency", "completed", 104).await;
    let next = fixture.store.reconcile_schedules(104).await.unwrap();
    assert_eq!(next.len(), 1);
    set_status(&fixture, "dependency", "completed", 105).await;
    assert!(
        fixture
            .store
            .reconcile_schedules(105)
            .await
            .unwrap()
            .is_empty()
    );
    let one_shot = fixture
        .store
        .register_schedule(&dependency("already-done", false), 106)
        .await
        .unwrap();
    assert_eq!(
        fixture.store.reconcile_schedules(106).await.unwrap().len(),
        1
    );
    assert_eq!(
        fixture
            .store
            .registration("team", &one_shot.registration.id)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopRegistrationState::Completed
    );
    assert_eq!(
        fixture
            .store
            .registration("team", &created.registration.id)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopRegistrationState::Active
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_schedule_registration_serializes_with_an_in_flight_dependency_change() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    task(&fixture, "dependency", "open").await;
    let mut tx = fixture
        .store
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap();
    sqlx::query(
        "UPDATE team_tasks SET status = 'completed', updated_at = 101 WHERE id = 'dependency'",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    LoopStore::observe_task_schedule_tx(&mut tx, "team", "dependency", 101)
        .await
        .unwrap();
    let store = fixture.store.clone();
    let (started, observed) = tokio::sync::oneshot::channel();
    let mut registering = tokio::spawn(async move {
        started.send(()).unwrap();
        store
            .register_schedule(&dependency("race", false), 102)
            .await
    });
    observed.await.unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), &mut registering)
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    let registration = registering.await.unwrap().unwrap();
    assert_eq!(registration.registration.pending_cursor, Some(101));
    assert_eq!(
        fixture.store.reconcile_schedules(102).await.unwrap().len(),
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_schedule_thread_cursor_recovers_old_replies_and_excludes_self_authored_wakes() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let root = conversation(&fixture).await;
    reply(&fixture, Some(root), "worker", 101).await;
    let first = reply(&fixture, Some(root), "other", 102).await;
    let input = registration(
        "thread",
        LoopSchedule::ThreadReply {
            root_message_id: root,
            after_message_id: root,
            repeat: true,
        },
    );
    let created = fixture.store.register_schedule(&input, 103).await.unwrap();
    assert_eq!(created.registration.pending_cursor, Some(first));
    let firings = fixture.store.reconcile_schedules(103).await.unwrap();
    let source = fixture
        .store
        .triggers("team", &firings[0].receipt.activation_id)
        .await
        .unwrap();
    assert_eq!(
        source[0].input.references.conversation_message_id,
        Some(first)
    );
    reply(&fixture, Some(root), "worker", 104).await;
    assert!(
        fixture
            .store
            .reconcile_schedules(104)
            .await
            .unwrap()
            .is_empty()
    );
    let next = reply(&fixture, Some(root), "other", 105).await;
    let last = reply(&fixture, Some(root), "other", 106).await;
    let second = fixture.store.reconcile_schedules(107).await.unwrap();
    assert_eq!(second.len(), 1);
    assert_eq!(
        (second[0].first_cursor, second[0].through_cursor),
        (next, last)
    );
    let mut invalid = input.clone();
    invalid.source_key = "future-cursor".into();
    invalid.schedule = LoopSchedule::ThreadReply {
        root_message_id: root,
        after_message_id: last + 1,
        repeat: false,
    };
    assert!(
        fixture
            .store
            .register_schedule(&invalid, 108)
            .await
            .is_err()
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_schedule_revocation_keeps_independent_work_and_fences_an_admitted_firing() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let created = fixture
        .store
        .register_schedule(&registration("due", LoopSchedule::Due { due_at: 100 }), 100)
        .await
        .unwrap();
    let firing = fixture
        .store
        .reconcile_schedules(100)
        .await
        .unwrap()
        .remove(0);
    let independent = fixture
        .store
        .accept_trigger(&trigger("addressed"), 100)
        .await
        .unwrap();
    assert_eq!(independent.activation_id, firing.receipt.activation_id);
    fixture
        .store
        .revoke_schedule("team", &created.registration.id, 101)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .activation("team", &independent.activation_id)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopActivationState::Pending
    );
    fixture
        .store
        .cancel("team", &independent.activation_id, 101)
        .await
        .unwrap();
    let created = fixture
        .store
        .register_schedule(
            &registration("isolated", LoopSchedule::Due { due_at: 102 }),
            102,
        )
        .await
        .unwrap();
    let firing = fixture
        .store
        .reconcile_schedules(102)
        .await
        .unwrap()
        .remove(0);
    let reservation = running(&fixture, &firing.receipt.activation_id, 102).await;
    fixture
        .store
        .revoke_schedule("team", &created.registration.id, 103)
        .await
        .unwrap();
    assert!(fixture.store.renew(&reservation, 103).await.is_err());
    assert!(
        fixture
            .store
            .reservation("team", "worker")
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        fixture
            .store
            .activation("team", &firing.receipt.activation_id)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopActivationState::Canceled
    );
    fixture
        .store
        .cleanup_verified(&reservation, LoopCleanupDisposition::Exited, 104)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .reservation("team", "worker")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        fixture
            .store
            .reconcile_schedules(110)
            .await
            .unwrap()
            .is_empty()
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_schedule_task_cancellation_and_deletion_revoke_pending_and_dormant_work() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    task(&fixture, "work", "open").await;
    let mut input = registration("bound", LoopSchedule::Due { due_at: 101 });
    input.work_task_id = Some("work".into());
    let created = fixture.store.register_schedule(&input, 100).await.unwrap();
    let firing = fixture
        .store
        .reconcile_schedules(101)
        .await
        .unwrap()
        .remove(0);
    set_status(&fixture, "work", "canceled", 102).await;
    assert_eq!(
        fixture
            .store
            .registration("team", &created.registration.id)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopRegistrationState::Revoked
    );
    assert_eq!(
        fixture
            .store
            .activation("team", &firing.receipt.activation_id)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopActivationState::Canceled
    );
    let root = conversation(&fixture).await;
    let created = fixture
        .store
        .register_schedule(
            &registration(
                "thread",
                LoopSchedule::ThreadReply {
                    root_message_id: root,
                    after_message_id: root,
                    repeat: true,
                },
            ),
            103,
        )
        .await
        .unwrap();
    let mut tx = fixture
        .store
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap();
    LoopStore::revoke_task_schedules_tx(&mut tx, "team", "discussion", 104)
        .await
        .unwrap();
    sqlx::query("DELETE FROM team_conversation_messages WHERE task_id = 'discussion'")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        fixture
            .store
            .registration("team", &created.registration.id)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopRegistrationState::Revoked
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_schedule_origin_cancel_retires_descendants_but_normal_finish_preserves_them() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let source = fixture
        .store
        .accept_trigger(&trigger("origin"), 100)
        .await
        .unwrap();
    let reservation = running(&fixture, &source.activation_id, 100).await;
    let mut input = registration("follow-up", LoopSchedule::Due { due_at: 101 });
    input.references.scheduling_actor_id = Some("worker".into());
    input.references.scheduling_activation_id = Some(source.activation_id.clone());
    let created = fixture.store.register_schedule(&input, 100).await.unwrap();
    let outcome = LoopOutcome {
        kind: LoopOutcomeKind::NoActionableWork,
        wait_reason: None,
        task_note_id: None,
        continuation: None,
    };
    fixture
        .store
        .finish(&reservation, &outcome, 101)
        .await
        .unwrap();
    fixture
        .store
        .cleanup_verified(&reservation, LoopCleanupDisposition::Exited, 101)
        .await
        .unwrap();
    let firing = fixture
        .store
        .reconcile_schedules(102)
        .await
        .unwrap()
        .remove(0);
    let child = running(&fixture, &firing.receipt.activation_id, 102).await;
    let mut retry = input.clone();
    retry.references.scheduling_activation_id = child.activation_id.clone();
    assert!(
        fixture
            .store
            .register_schedule(&retry, 103)
            .await
            .unwrap()
            .duplicate
    );
    let mut descendant = retry.clone();
    descendant.source_key = "descendant".into();
    descendant.schedule = LoopSchedule::Due { due_at: 200 };
    let descendant = fixture
        .store
        .register_schedule(&descendant, 103)
        .await
        .unwrap();
    fixture
        .store
        .cancel("team", &source.activation_id, 104)
        .await
        .unwrap();
    for id in [created.registration.id, descendant.registration.id] {
        assert_eq!(
            fixture
                .store
                .registration("team", &id)
                .await
                .unwrap()
                .unwrap()
                .state,
            LoopRegistrationState::Revoked
        );
    }
    assert!(fixture.store.renew(&child, 104).await.is_err());
    fixture
        .store
        .cleanup_verified(&child, LoopCleanupDisposition::Exited, 105)
        .await
        .unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn loop_schedule_budgets_scope_and_pagination_cover_dormant_registrations() {
    let fixture = Fixture::new().await;
    let limits = LoopLimits {
        standing_per_actor: 2,
        standing_per_team: 2,
        ..LoopLimits::default()
    };
    fixture.enable("worker", &limits).await;
    fixture.enable("other", &LoopLimits::default()).await;
    let a = fixture
        .store
        .register_schedule(&registration("a", LoopSchedule::Due { due_at: 200 }), 100)
        .await
        .unwrap();
    fixture
        .store
        .register_schedule(&registration("b", LoopSchedule::Due { due_at: 200 }), 100)
        .await
        .unwrap();
    let mut c = registration("c", LoopSchedule::Due { due_at: 200 });
    assert!(matches!(
        fixture
            .store
            .register_schedule(&c, 100)
            .await
            .unwrap_err()
            .downcast_ref::<LoopStoreError>(),
        Some(LoopStoreError::Capacity)
    ));
    c.actor_id = "other".into();
    assert!(fixture.store.register_schedule(&c, 100).await.is_err());
    let first = fixture
        .store
        .registrations("team", "worker", None, 1)
        .await
        .unwrap();
    assert_eq!(first.registrations.len(), 1);
    let last = fixture
        .store
        .registrations("team", "worker", first.next_cursor.as_deref(), 1)
        .await
        .unwrap();
    assert_eq!(last.registrations.len(), 1);
    assert!(last.next_cursor.is_none());
    assert_ne!(first.registrations[0].id, last.registrations[0].id);
    fixture
        .store
        .configure(
            LoopPolicyUpdate {
                actor_id: "worker",
                team_id: "team",
                expected_revision: 1,
                state: LoopPolicyState::Suspended,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &limits,
            },
            101,
        )
        .await
        .unwrap();
    let mut tx = fixture
        .store
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap();
    assert!(
        LoopStore::require_scope_quiescent_tx(&mut tx, "worker")
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    fixture
        .store
        .revoke_schedule("team", &a.registration.id, 102)
        .await
        .unwrap();
    fixture
        .store
        .revoke_schedule("team", &last.registrations[0].id, 102)
        .await
        .unwrap();
    fixture
        .store
        .revoke_schedule("team", &first.registrations[0].id, 102)
        .await
        .unwrap();
    let mut tx = fixture
        .store
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap();
    LoopStore::require_scope_quiescent_tx(&mut tx, "worker")
        .await
        .unwrap();
    assert!(
        LoopStore::require_no_retained_history_tx(&mut tx, "worker")
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    assert!(
        fixture
            .store
            .revoke_schedule("elsewhere", &a.registration.id, 103)
            .await
            .is_err()
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_schedule_suspension_retains_work_and_repeated_cycles_hit_no_progress_limits() {
    let fixture = Fixture::new().await;
    let limits = LoopLimits {
        consecutive_no_progress: 2,
        ..LoopLimits::default()
    };
    fixture.enable("worker", &limits).await;
    let input = registration(
        "clock",
        LoopSchedule::Recurring {
            first_at: 100,
            interval_seconds: 10,
        },
    );
    fixture.store.register_schedule(&input, 100).await.unwrap();
    fixture
        .store
        .configure(
            LoopPolicyUpdate {
                actor_id: "worker",
                team_id: "team",
                expected_revision: 1,
                state: LoopPolicyState::Suspended,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &limits,
            },
            100,
        )
        .await
        .unwrap();
    let first = fixture
        .store
        .reconcile_schedules(100)
        .await
        .unwrap()
        .remove(0);
    assert_eq!(
        fixture
            .store
            .admit("team", &first.receipt.activation_id, "daemon", 100)
            .await
            .unwrap(),
        LoopAdmission::Deferred(LoopDeferralReason::Suspended)
    );
    fixture
        .store
        .configure(
            LoopPolicyUpdate {
                actor_id: "worker",
                team_id: "team",
                expected_revision: 2,
                state: LoopPolicyState::Enabled,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &limits,
            },
            100,
        )
        .await
        .unwrap();
    for (index, now) in [105, 110].into_iter().enumerate() {
        let activation_id = if index == 0 {
            first.receipt.activation_id.clone()
        } else {
            fixture
                .store
                .reconcile_schedules(now)
                .await
                .unwrap()
                .remove(0)
                .receipt
                .activation_id
        };
        let reservation = running(&fixture, &activation_id, now).await;
        fixture
            .store
            .finish(
                &reservation,
                &LoopOutcome {
                    kind: LoopOutcomeKind::NoActionableWork,
                    wait_reason: None,
                    task_note_id: None,
                    continuation: None,
                },
                now + 1,
            )
            .await
            .unwrap();
        fixture
            .store
            .cleanup_verified(&reservation, LoopCleanupDisposition::Exited, now + 1)
            .await
            .unwrap();
    }
    let next = fixture
        .store
        .reconcile_schedules(120)
        .await
        .unwrap()
        .remove(0);
    assert_eq!(
        fixture
            .store
            .admit("team", &next.receipt.activation_id, "daemon", 120)
            .await
            .unwrap(),
        LoopAdmission::Deferred(LoopDeferralReason::NoProgressLimit)
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_schedule_reconciliation_bounds_fanout_and_revocation_races_with_firing() {
    let fixture = Fixture::new().await;
    let limits = LoopLimits {
        standing_per_actor: 64,
        standing_per_team: 64,
        ..LoopLimits::default()
    };
    fixture.enable("worker", &limits).await;
    for index in 0..40 {
        fixture
            .store
            .register_schedule(
                &registration(&format!("due:{index}"), LoopSchedule::Due { due_at: 101 }),
                100,
            )
            .await
            .unwrap();
    }
    assert_eq!(
        fixture.store.reconcile_schedules(101).await.unwrap().len(),
        32
    );
    assert_eq!(
        fixture.store.reconcile_schedules(101).await.unwrap().len(),
        8
    );
    let created = fixture
        .store
        .register_schedule(
            &registration("race", LoopSchedule::Due { due_at: 102 }),
            101,
        )
        .await
        .unwrap();
    let (fired, revoked) = tokio::join!(
        fixture.store.reconcile_schedules(102),
        fixture
            .store
            .revoke_schedule("team", &created.registration.id, 102)
    );
    let fired = fired.unwrap();
    assert_eq!(revoked.unwrap().state, LoopRegistrationState::Revoked);
    assert!(fired.len() <= 1);
    for firing in fired {
        let sources = fixture
            .store
            .triggers("team", &firing.receipt.activation_id)
            .await
            .unwrap();
        assert!(
            sources
                .iter()
                .find(|source| source.id == firing.receipt.trigger_id)
                .unwrap()
                .revoked
        );
    }
    assert!(
        fixture
            .store
            .reconcile_schedules(103)
            .await
            .unwrap()
            .is_empty()
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_schedule_disabled_backlog_coalesces_without_discarding_due_work() {
    let fixture = Fixture::new().await;
    let limits = LoopLimits::default();
    fixture.enable("worker", &limits).await;
    let created = fixture
        .store
        .register_schedule(
            &registration(
                "clock",
                LoopSchedule::Recurring {
                    first_at: 100,
                    interval_seconds: 10,
                },
            ),
            100,
        )
        .await
        .unwrap();
    fixture
        .store
        .configure(
            LoopPolicyUpdate {
                actor_id: "worker",
                team_id: "team",
                expected_revision: 1,
                state: LoopPolicyState::Disabled,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &limits,
            },
            101,
        )
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .register_schedule(
                &registration("disabled", LoopSchedule::Due { due_at: 100 }),
                101
            )
            .await
            .is_err()
    );
    assert!(
        fixture
            .store
            .reconcile_schedules(1000)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        fixture
            .store
            .registration("team", &created.registration.id)
            .await
            .unwrap()
            .unwrap()
            .pending_cursor,
        Some(100)
    );
    fixture
        .store
        .configure(
            LoopPolicyUpdate {
                actor_id: "worker",
                team_id: "team",
                expected_revision: 2,
                state: LoopPolicyState::Enabled,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &limits,
            },
            1005,
        )
        .await
        .unwrap();
    let fired = fixture.store.reconcile_schedules(1005).await.unwrap();
    assert_eq!(fired.len(), 1);
    assert_eq!(
        (fired[0].first_cursor, fired[0].through_cursor),
        (100, 1000)
    );
    assert_eq!(
        fixture
            .store
            .registration("team", &created.registration.id)
            .await
            .unwrap()
            .unwrap()
            .next_due_at,
        Some(1010)
    );
    let mut changed = registration("clock", LoopSchedule::Due { due_at: 100 });
    assert!(matches!(
        fixture
            .store
            .register_schedule(&changed, 1006)
            .await
            .unwrap_err()
            .downcast_ref::<LoopStoreError>(),
        Some(LoopStoreError::IdempotencyConflict)
    ));
    changed.actor_id = "outsider".into();
    assert!(matches!(
        fixture
            .store
            .register_schedule(&changed, 1006)
            .await
            .unwrap_err()
            .downcast_ref::<LoopStoreError>(),
        Some(LoopStoreError::ScopeMismatch)
    ));
    fixture.close().await;
}
