use agenthub_agent_domain::loop_runtime::{LoopAdmission, LoopDeferralReason, LoopReservation};

use super::*;

fn admitted(result: LoopAdmission) -> LoopReservation {
    match result {
        LoopAdmission::Admitted(reservation) => reservation,
        other => panic!("expected admission, got {other:?}"),
    }
}

#[tokio::test]
async fn loop_admission_applies_actor_and_team_rolling_limits() {
    let fixture = Fixture::new().await;
    let limits = LoopLimits {
        activations_per_actor: 1,
        activations_per_team: 2,
        ..LoopLimits::default()
    };
    fixture.enable("worker", &limits).await;
    fixture.enable("other", &limits).await;
    let first = fixture
        .store
        .accept_trigger(&trigger("first"), 100)
        .await
        .unwrap();
    admitted(
        fixture
            .store
            .admit("team", &first.activation_id, "daemon", 100)
            .await
            .unwrap(),
    );
    // Seed a completed historical episode. This test exercises admission, not process cleanup.
    sqlx::query("DELETE FROM loop_execution_reservations WHERE actor_id = 'worker'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE loop_activations SET state = 'finished', finished_at = 100 WHERE id = ?")
        .bind(&first.activation_id)
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    let next = fixture
        .store
        .accept_trigger(&trigger("next"), 100)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .admit("team", &next.activation_id, "daemon", 100)
            .await
            .unwrap(),
        LoopAdmission::Deferred(LoopDeferralReason::ActorRateLimit)
    );
    let mut other = trigger("other");
    other.actor_id = "other".into();
    let second = fixture.store.accept_trigger(&other, 100).await.unwrap();
    admitted(
        fixture
            .store
            .admit("team", &second.activation_id, "daemon", 100)
            .await
            .unwrap(),
    );
    let higher_limits = LoopLimits {
        activations_per_actor: 3,
        activations_per_team: 3,
        ..limits
    };
    fixture
        .store
        .configure(
            LoopPolicyUpdate {
                actor_id: "worker",
                team_id: "team",
                expected_revision: 1,
                state: LoopPolicyState::Enabled,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &higher_limits,
            },
            101,
        )
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .admit("team", &next.activation_id, "daemon", 110)
            .await
            .unwrap(),
        LoopAdmission::Deferred(LoopDeferralReason::TeamRateLimit)
    );
    admitted(
        fixture
            .store
            .admit("team", &next.activation_id, "daemon", 1000)
            .await
            .unwrap(),
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_bounded_admission_scan_does_not_starve_eligible_actors() {
    let fixture = Fixture::new().await;
    let limits = LoopLimits {
        pending_per_actor: 64,
        pending_per_team: 128,
        sources_per_activation: 1,
        ..LoopLimits::default()
    };
    fixture.enable("worker", &limits).await;
    fixture.enable("other", &LoopLimits::default()).await;
    for index in 0..64 {
        fixture
            .store
            .accept_trigger(&trigger(&format!("blocked:{index}")), 100 + index)
            .await
            .unwrap();
    }
    sqlx::query("UPDATE loop_policies SET no_progress_count = 3 WHERE actor_id = 'worker'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    let mut input = trigger("eligible");
    input.actor_id = "other".into();
    fixture.store.accept_trigger(&input, 164).await.unwrap();
    assert!(
        fixture
            .store
            .admit_next("daemon", 200)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        fixture
            .store
            .admit_next("daemon", 200)
            .await
            .unwrap()
            .is_none()
    );
    let lease = fixture
        .store
        .admit_next("daemon", 200)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(lease.actor_id, "other");
    fixture.close().await;
}

#[tokio::test]
async fn loop_admission_does_not_replace_another_task_owner() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    sqlx::raw_sql(
        "INSERT INTO team_tasks(id, team_id, title, status, created_by_actor_id, assigned_member_id, context_json, created_at, updated_at) \
         VALUES ('task', 'team', 'Fixture', 'in_progress', 'worker', 'worker', '{}', 1, 1); \
         INSERT INTO team_execution_claims(entity_kind, entity_id, team_id, owner_member_id, lease_generation, claimed_at, expires_at) \
         VALUES ('task', 'task', 'team', 'other', 7, 1, 10);",
    ).execute(&fixture.store.pool).await.unwrap();
    let mut input = trigger("assignment");
    input.kind = LoopTriggerKind::Assignment;
    input.references.task_id = Some("task".into());
    let receipt = fixture.store.accept_trigger(&input, 100).await.unwrap();
    assert_eq!(
        fixture
            .store
            .admit("team", &receipt.activation_id, "daemon", 100)
            .await
            .unwrap(),
        LoopAdmission::Deferred(LoopDeferralReason::TaskOwnedElsewhere)
    );
    sqlx::query("UPDATE team_execution_claims SET released_at = 101 WHERE entity_id = 'task'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    admitted(
        fixture
            .store
            .admit("team", &receipt.activation_id, "daemon", 105)
            .await
            .unwrap(),
    );
    let owner: String = sqlx::query_scalar(
        "SELECT owner_member_id FROM team_execution_claims WHERE entity_id = 'task'",
    )
    .fetch_one(&fixture.store.pool)
    .await
    .unwrap();
    assert_eq!(owner, "other");
    fixture.close().await;
}

#[tokio::test]
async fn loop_admission_rechecks_membership_and_preserves_pending_history() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let receipt = fixture
        .store
        .accept_trigger(&trigger("work"), 100)
        .await
        .unwrap();
    sqlx::query("UPDATE team_definitions SET spec_json = '{}' WHERE id = 'team'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .admit("team", &receipt.activation_id, "daemon", 100)
            .await
            .unwrap(),
        LoopAdmission::Deferred(LoopDeferralReason::MembershipChanged)
    );
    assert_eq!(
        fixture
            .store
            .activation("team", &receipt.activation_id)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopActivationState::Pending
    );
    assert!(
        fixture
            .store
            .reservation("team", "worker")
            .await
            .unwrap()
            .is_none()
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_admission_migration_backfills_old_pending_deadlines() {
    let mut fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let receipt = fixture
        .store
        .accept_trigger(&trigger("old-pending"), 100)
        .await
        .unwrap();
    sqlx::raw_sql(
        "DROP INDEX idx_loop_admission_due; \
         ALTER TABLE loop_activations DROP COLUMN next_admission_at; \
         ALTER TABLE loop_activation_events DROP COLUMN reason_code; \
         ALTER TABLE loop_execution_reservations DROP COLUMN lease_seconds;",
    )
    .execute(&fixture.store.pool)
    .await
    .unwrap();
    // Upgrade across daemon restart, without reusing statements prepared before the fixture DDL.
    fixture.store.pool.close().await;
    fixture.store = LoopStore::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    migrate_loop_runtime(&fixture.store.pool).await.unwrap();
    migrate_loop_runtime(&fixture.store.pool).await.unwrap();
    let activation = fixture
        .store
        .activation("team", &receipt.activation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(activation.next_admission_at, 100);
    assert_eq!(
        fixture
            .store
            .events("team", &receipt.activation_id, 0, 100)
            .await
            .unwrap()
            .len(),
        1
    );
    admitted(
        fixture
            .store
            .admit("team", &receipt.activation_id, "daemon", 100)
            .await
            .unwrap(),
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_admission_serializes_concurrent_claims() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let receipt = fixture
        .store
        .accept_trigger(&trigger("work"), 100)
        .await
        .unwrap();
    let (left, right) = tokio::join!(
        fixture
            .store
            .admit("team", &receipt.activation_id, "daemon-a", 100),
        fixture
            .store
            .admit("team", &receipt.activation_id, "daemon-b", 100),
    );
    let (left, right) = (left.unwrap(), right.unwrap());
    assert!(matches!(
        (&left, &right),
        (LoopAdmission::Admitted(_), LoopAdmission::NotPending)
            | (LoopAdmission::NotPending, LoopAdmission::Admitted(_))
    ));
    let reservation = fixture
        .store
        .reservation("team", "worker")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reservation.generation, 1);
    assert_eq!(reservation.lease_expires_at, 160);
    assert_eq!(
        fixture
            .store
            .activation("team", &receipt.activation_id)
            .await
            .unwrap()
            .unwrap()
            .attempt_count,
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_admission_and_manual_start_share_the_writer_reservation() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let receipt = fixture
        .store
        .accept_trigger(&trigger("work"), 100)
        .await
        .unwrap();
    let (manual, automatic) = tokio::join!(
        fixture
            .store
            .reserve_manual("team", "worker", "manual-owner", 100),
        fixture
            .store
            .admit("team", &receipt.activation_id, "loop-owner", 100),
    );
    match (manual, automatic.unwrap()) {
        (Ok(reservation), LoopAdmission::Deferred(LoopDeferralReason::Reserved)) => {
            assert!(reservation.activation_id.is_none())
        }
        (Err(error), LoopAdmission::Admitted(_)) => assert!(matches!(
            error.downcast_ref(),
            Some(LoopStoreError::ReservationHeld)
        )),
        other => panic!("invalid writer race: {other:?}"),
    }
    assert_eq!(
        fixture
            .store
            .policy("team", "worker")
            .await
            .unwrap()
            .unwrap()
            .generation,
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_expired_or_stale_lease_cannot_renew_or_allow_replacement() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let first = fixture
        .store
        .accept_trigger(&trigger("first"), 100)
        .await
        .unwrap();
    let lease = admitted(
        fixture
            .store
            .admit("team", &first.activation_id, "daemon", 100)
            .await
            .unwrap(),
    );
    let mut stale = lease.clone();
    stale.generation += 1;
    assert!(fixture.store.renew(&stale, 120).await.is_err());
    stale = lease.clone();
    stale.owner_id = "other-daemon".into();
    assert!(fixture.store.renew(&stale, 120).await.is_err());
    let renewed = fixture.store.renew(&lease, 120).await.unwrap();
    assert_eq!(renewed.lease_expires_at, 180);
    assert!(fixture.store.renew(&renewed, 180).await.is_err());
    let next = fixture
        .store
        .accept_trigger(&trigger("next"), 181)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .admit("team", &next.activation_id, "replacement", 181)
            .await
            .unwrap(),
        LoopAdmission::Deferred(LoopDeferralReason::LeaseExpiredUnfenced)
    );
    assert_eq!(
        fixture
            .store
            .reservation("team", "worker")
            .await
            .unwrap()
            .unwrap()
            .generation,
        1
    );
    let events = fixture
        .store
        .events("team", &next.activation_id, 0, 100)
        .await
        .unwrap();
    assert_eq!(
        events.last().unwrap().reason,
        Some(LoopDeferralReason::LeaseExpiredUnfenced)
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_admission_respects_due_time_suspension_and_active_lease_configuration() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let mut input = trigger("future");
    input.due_at = Some(200);
    let receipt = fixture.store.accept_trigger(&input, 100).await.unwrap();
    assert_eq!(
        fixture
            .store
            .admit("team", &receipt.activation_id, "daemon", 199)
            .await
            .unwrap(),
        LoopAdmission::Deferred(LoopDeferralReason::NotDue)
    );
    let lease = admitted(
        fixture
            .store
            .admit("team", &receipt.activation_id, "daemon", 200)
            .await
            .unwrap(),
    );
    let limits = LoopLimits {
        lease_seconds: 120,
        ..LoopLimits::default()
    };
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
            201,
        )
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .renew(&lease, 210)
            .await
            .unwrap()
            .lease_expires_at,
        270
    );
    let next = fixture
        .store
        .accept_trigger(&trigger("while-suspended"), 211)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .admit("team", &next.activation_id, "daemon", 211)
            .await
            .unwrap(),
        LoopAdmission::Deferred(LoopDeferralReason::Suspended)
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_admission_limits_and_deferral_history_survive_reopen() {
    let mut fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let receipt = fixture
        .store
        .accept_trigger(&trigger("work"), 100)
        .await
        .unwrap();
    sqlx::query("UPDATE loop_policies SET no_progress_count = 3 WHERE actor_id = 'worker'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    fixture.store.pool.close().await;
    fixture.store = LoopStore::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    for now in [100, 110, 120] {
        assert_eq!(
            fixture
                .store
                .admit("team", &receipt.activation_id, "daemon", now)
                .await
                .unwrap(),
            LoopAdmission::Deferred(LoopDeferralReason::NoProgressLimit)
        );
    }
    let events = fixture
        .store
        .events("team", &receipt.activation_id, 0, 100)
        .await
        .unwrap();
    assert_eq!(events.len(), 2);
    sqlx::query("UPDATE loop_policies SET no_progress_count = 0 WHERE actor_id = 'worker'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE loop_activations SET attempt_count = 5 WHERE id = ?")
        .bind(&receipt.activation_id)
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .admit("team", &receipt.activation_id, "daemon", 130)
            .await
            .unwrap(),
        LoopAdmission::Deferred(LoopDeferralReason::StartupLimit)
    );
    assert!(
        fixture
            .store
            .reservation("team", "worker")
            .await
            .unwrap()
            .is_none()
    );
    fixture.close().await;
}
