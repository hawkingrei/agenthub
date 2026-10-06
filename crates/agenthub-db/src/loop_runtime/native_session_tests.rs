use agenthub_agent_domain::loop_runtime::{LoopAdmission, LoopCleanupDisposition, LoopReservation};

use super::*;

async fn starting(fixture: &Fixture, policy: LoopSessionPolicy, now: i64) -> LoopReservation {
    let key = format!("native-{now}");
    let receipt = fixture
        .store
        .accept_trigger(&trigger(&key), now)
        .await
        .unwrap();
    let LoopAdmission::Admitted(reservation) = fixture
        .store
        .admit("team", &receipt.activation_id, "daemon", now)
        .await
        .unwrap()
    else {
        panic!("not admitted");
    };
    bind_starting(fixture, reservation, policy, now).await
}

async fn bind_starting(
    fixture: &Fixture,
    reservation: LoopReservation,
    policy: LoopSessionPolicy,
    now: i64,
) -> LoopReservation {
    let key = format!("native-{now}");
    fixture
        .store
        .bind_mailbox(&reservation, "mailbox", now)
        .await
        .unwrap();
    let snapshot = agenthub_agent_domain::loop_runtime::LoopLaunchSnapshot {
        provider_id: "rara".into(),
        session_policy: policy,
        ..launch_tests::snapshot()
    };
    fixture
        .store
        .record_launch(&reservation, &snapshot, now)
        .await
        .unwrap();
    fixture
        .store
        .authorize_guarded_spawn(&reservation, now)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES (?, 'worker', 'running', ?)")
        .bind(&key).bind(now).execute(&fixture.store.pool).await.unwrap();
    fixture
        .store
        .bind_session(&reservation, &key, now)
        .await
        .unwrap()
}

#[tokio::test]
async fn native_session_startup_retry_uses_retired_generation_not_overwritten_activation_session() {
    let fixture = fixture().await;
    let digest = "a".repeat(64);
    let first = starting(&fixture, LoopSessionPolicy::Resume, 101).await;
    fixture
        .store
        .begin_native_session(&first, &digest, 102)
        .await
        .unwrap();
    fixture
        .store
        .bind_native_session(&first, "conversation", 102)
        .await
        .unwrap();
    fixture
        .store
        .cleanup_verified(&first, LoopCleanupDisposition::StartupFailed, 103)
        .await
        .unwrap();
    let LoopAdmission::Admitted(retry) = fixture
        .store
        .admit(
            "team",
            first.activation_id.as_deref().unwrap(),
            "replacement",
            106,
        )
        .await
        .unwrap()
    else {
        panic!("retry not admitted");
    };
    let retry = bind_starting(&fixture, retry, LoopSessionPolicy::Resume, 106).await;
    assert_eq!(retry.activation_id, first.activation_id);
    assert_ne!(retry.session_id, first.session_id);
    assert!(retry.generation > first.generation);
    assert_eq!(
        fixture
            .store
            .begin_native_session(&retry, &digest, 107)
            .await
            .unwrap()
            .as_deref(),
        Some("conversation")
    );
    fixture
        .store
        .bind_native_session(&retry, "conversation", 107)
        .await
        .unwrap();
    fixture.close().await;
}

async fn fixture() -> Fixture {
    let fixture = Fixture::new().await;
    fixture
        .enable(
            "worker",
            &LoopLimits {
                consecutive_no_progress: 20,
                ..LoopLimits::default()
            },
        )
        .await;
    launch_tests::partition(&fixture, "mailbox", "team").await;
    fixture
}

#[tokio::test]
async fn native_session_binding_survives_migration_and_requires_exact_retirement() {
    let mut fixture = fixture().await;
    let digest = "a".repeat(64);
    let first = starting(&fixture, LoopSessionPolicy::Resume, 101).await;
    assert_eq!(
        fixture
            .store
            .begin_native_session(&first, &digest, 102)
            .await
            .unwrap(),
        None
    );
    assert!(
        fixture
            .store
            .begin_native_session(&first, &digest, 102)
            .await
            .is_err()
    );
    fixture
        .store
        .bind_native_session(&first, "conversation", 103)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .bind_native_session(&first, "foreign", 103)
            .await
            .is_err()
    );
    fixture
        .store
        .cleanup_verified(&first, LoopCleanupDisposition::Exited, 104)
        .await
        .unwrap();
    fixture.store.pool.close().await;
    fixture.store = LoopStore::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    migrate_loop_runtime(&fixture.store.pool).await.unwrap();
    let second = starting(&fixture, LoopSessionPolicy::Resume, 105).await;
    // Simulate incomplete historical retirement; expiry/status cannot fill this gap.
    sqlx::query("UPDATE loop_activation_events SET generation = generation + 100 WHERE kind = 'cleanup_verified'")
        .execute(&fixture.store.pool).await.unwrap();
    assert!(
        fixture
            .store
            .begin_native_session(&second, &digest, 106)
            .await
            .is_err()
    );
    sqlx::query("UPDATE loop_activation_events SET generation = generation - 100 WHERE kind = 'cleanup_verified'")
        .execute(&fixture.store.pool).await.unwrap();
    assert_eq!(
        fixture
            .store
            .begin_native_session(&second, &digest, 106)
            .await
            .unwrap()
            .as_deref(),
        Some("conversation")
    );
    assert!(
        fixture
            .store
            .bind_native_session(&first, "late", 106)
            .await
            .is_err()
    );
    fixture
        .store
        .bind_native_session(&second, "conversation", 106)
        .await
        .unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn native_session_resume_never_discards_config_mismatch_or_ambiguous_opening() {
    let fixture = fixture().await;
    let digest = "a".repeat(64);
    let first = starting(&fixture, LoopSessionPolicy::Fresh, 101).await;
    fixture
        .store
        .begin_native_session(&first, &digest, 102)
        .await
        .unwrap();
    // The response may have been lost; there is no proof that creation failed.
    fixture
        .store
        .cleanup_verified(&first, LoopCleanupDisposition::Exited, 104)
        .await
        .unwrap();
    let second = starting(&fixture, LoopSessionPolicy::Resume, 105).await;
    let error = fixture
        .store
        .begin_native_session(&second, &digest, 106)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref(),
        Some(LoopStoreError::NativeOpeningUncertain)
    ));
    let error = fixture
        .store
        .begin_native_session(&second, &"b".repeat(64), 106)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref(),
        Some(LoopStoreError::NativeConfigurationChanged)
    ));
    fixture
        .store
        .cleanup_verified(&second, LoopCleanupDisposition::Exited, 107)
        .await
        .unwrap();
    let third = starting(&fixture, LoopSessionPolicy::Fresh, 108).await;
    assert_eq!(
        fixture
            .store
            .begin_native_session(&third, &"b".repeat(64), 109)
            .await
            .unwrap(),
        None
    );
    fixture
        .store
        .bind_native_session(&third, "replacement", 109)
        .await
        .unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn native_session_binding_rejects_stale_or_unguarded_local_owners() {
    let fixture = fixture().await;
    let digest = "a".repeat(64);
    let owner = starting(&fixture, LoopSessionPolicy::Fresh, 101).await;
    for change in 0..5 {
        let mut stale = owner.clone();
        match change {
            0 => stale.generation += 1,
            1 => stale.team_id = "elsewhere".into(),
            2 => stale.owner_id = "other-daemon".into(),
            3 => stale.session_id = Some("other-launch".into()),
            _ => stale.activation_id = None,
        }
        assert!(
            fixture
                .store
                .begin_native_session(&stale, &digest, 102)
                .await
                .is_err()
        );
    }
    sqlx::query("UPDATE loop_execution_reservations SET executor_state = 'unknown'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .begin_native_session(&owner, &digest, 102)
            .await
            .is_err()
    );
    sqlx::query("UPDATE loop_execution_reservations SET executor_state = 'guarded'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    fixture
        .store
        .begin_native_session(&owner, &digest, 102)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .bind_native_session(&owner, "conversation", 200)
            .await
            .is_err()
    );
    sqlx::query("UPDATE agent_sessions SET ended_at = 103")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .bind_native_session(&owner, "conversation", 104)
            .await
            .is_err()
    );
    fixture.close().await;
}
