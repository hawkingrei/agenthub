use crate::loop_runtime::{LoopStore, LoopStoreError};
use agenthub_agent_domain::{
    loop_runtime::{LoopLimits, LoopPolicyState, LoopSourceReferences, LoopTriggerKind},
    loop_scheduling::{LoopRegistrationInput, LoopRegistrationState, LoopSchedule},
};

use super::{event_intake::notification, *};

pub(super) fn condition(
    app_id: &str,
    key: &str,
    after_cursor: i64,
    repeat: bool,
) -> LoopRegistrationInput {
    LoopRegistrationInput {
        actor_id: "worker".into(),
        team_id: "team".into(),
        source_key: key.into(),
        schedule: LoopSchedule::AppEvent {
            app_id: app_id.into(),
            event_class: "changed".into(),
            after_cursor,
            repeat,
        },
        work_task_id: None,
        references: LoopSourceReferences::default(),
    }
}

#[tokio::test]
async fn app_conditions_coalesce_distinct_cursors_without_replaying_once_or_repeat_firings() {
    let fixture = Fixture::new().await;
    let app = fixture.event_intake().await;
    let loops = LoopStore::new(fixture.store.pool.clone());
    let once = loops
        .register_schedule(&condition(&app.id, "once", 0, false), 90)
        .await
        .unwrap();
    let repeating = loops
        .register_schedule(&condition(&app.id, "repeat", 0, true), 90)
        .await
        .unwrap();
    let future = loops
        .register_schedule(&condition(&app.id, "future", 100, false), 90)
        .await
        .unwrap();
    assert_eq!(future.registration.observed_cursor, 100);
    assert_eq!(future.registration.next_check_at, i64::MAX);
    let metrics = loops.metrics("team", "worker", 90, 60).await.unwrap();
    let waits = metrics
        .waits
        .iter()
        .find(|wait| wait.kind == agenthub_agent_domain::loop_metrics::LoopWaitMetricKind::AppEvent)
        .unwrap();
    assert_eq!((waits.count, waits.oldest_age_seconds), (3, Some(0)));
    for cursor in [1, 3] {
        fixture
            .store
            .accept_signed_event(&app.id, 1, &notification(cursor), 100)
            .await
            .unwrap();
    }
    let fired = loops.reconcile_schedules(100).await.unwrap();
    assert_eq!(fired.len(), 2);
    for firing in &fired {
        assert_eq!((firing.first_cursor, firing.through_cursor), (1, 3));
        let sources = loops
            .triggers("team", &firing.receipt.activation_id)
            .await
            .unwrap();
        let source = sources
            .iter()
            .find(|s| s.id == firing.receipt.trigger_id)
            .unwrap();
        assert_eq!(source.input.kind, LoopTriggerKind::Dependency);
        assert_eq!(
            source.input.references.app_id.as_deref(),
            Some(app.id.as_str())
        );
        let event = source.input.references.app_event.as_ref().unwrap();
        assert_eq!(
            (
                &*event.event_id,
                &*event.event_class,
                event.cursor,
                event.version
            ),
            ("event-1", "changed", 1, 1)
        );
    }
    assert_eq!(
        loops
            .registration("team", &once.registration.id)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopRegistrationState::Completed
    );
    assert_eq!(
        loops
            .registration("team", &repeating.registration.id)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopRegistrationState::Active
    );
    for cursor in [1, 3] {
        assert!(
            fixture
                .store
                .accept_signed_event(&app.id, 1, &notification(cursor), 101)
                .await
                .unwrap()
                .duplicate
        );
    }
    assert!(loops.reconcile_schedules(101).await.unwrap().is_empty());
    fixture
        .store
        .accept_signed_event(&app.id, 1, &notification(101), 102)
        .await
        .unwrap();
    let fired = loops.reconcile_schedules(102).await.unwrap();
    assert_eq!(fired.len(), 2);
    assert!(
        fired
            .iter()
            .all(|f| (f.first_cursor, f.through_cursor) == (101, 101))
    );
    assert_eq!(
        loops
            .registration_firings("team", &once.registration.id, None, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn app_condition_registration_races_and_history_catchup_survive_reopen() {
    let mut fixture = Fixture::new().await;
    let app = fixture.event_intake().await;
    let loops = LoopStore::new(fixture.store.pool.clone());
    let request = condition(&app.id, "racing", 0, true);
    let event = notification(7);
    let (a, b, accepted) = tokio::join!(
        loops.register_schedule(&request, 100),
        loops.register_schedule(&request, 100),
        fixture.store.accept_signed_event(&app.id, 1, &event, 100),
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a.duplicate, b.duplicate);
    assert_eq!(a.registration.id, b.registration.id);
    let original = accepted.unwrap();
    let catchup = loops
        .register_schedule(&condition(&app.id, "catchup", 0, false), 101)
        .await
        .unwrap();
    assert_eq!(catchup.registration.pending_cursor, Some(7));
    fixture.store.pool.close().await;
    fixture.store = AppRegistry::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    let loops = LoopStore::new(fixture.store.pool.clone());
    let firings = loops.reconcile_schedules(102).await.unwrap();
    assert_eq!(firings.len(), 2);
    assert!(
        firings
            .iter()
            .all(|f| f.first_cursor == 7 && f.through_cursor == 7)
    );
    let duplicate = fixture
        .store
        .accept_signed_event(&app.id, 1, &event, 103)
        .await
        .unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.activation_id, original.activation_id);
    assert!(loops.reconcile_schedules(103).await.unwrap().is_empty());
    fixture.close().await;
}

#[tokio::test]
async fn app_condition_capacity_retries_preserve_latches_and_failed_observation_rolls_back_intake()
{
    let fixture = Fixture::new().await;
    let app = fixture.event_intake().await;
    let loops = LoopStore::new(fixture.store.pool.clone());
    let limits = LoopLimits {
        sources_per_activation: 1,
        pending_per_actor: 1,
        standing_per_actor: 1,
        ..LoopLimits::default()
    };
    fixture
        .event_policy(LoopPolicyState::Suspended, 1, &limits)
        .await;
    let registered = loops
        .register_schedule(&condition(&app.id, "retry", 0, true), 90)
        .await
        .unwrap();
    let error = loops
        .register_schedule(&condition(&app.id, "overflow", 0, false), 90)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<LoopStoreError>(),
        Some(LoopStoreError::Capacity)
    ));
    sqlx::raw_sql("CREATE TRIGGER fail_app_watch BEFORE UPDATE ON loop_registrations BEGIN SELECT RAISE(ABORT, 'fixture observation failure'); END;")
        .execute(&fixture.store.pool).await.unwrap();
    assert!(
        fixture
            .store
            .accept_signed_event(&app.id, 1, &notification(1), 100)
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app_event_receipts")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM loop_trigger_sources")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::raw_sql("DROP TRIGGER fail_app_watch")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    fixture
        .store
        .accept_signed_event(&app.id, 1, &notification(1), 100)
        .await
        .unwrap();
    assert!(loops.reconcile_schedules(100).await.unwrap().is_empty());
    let pending = loops
        .registration("team", &registered.registration.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending.pending_cursor, Some(1));
    assert_eq!(pending.next_check_at, 105);
    assert!(
        fixture
            .store
            .accept_signed_event(&app.id, 1, &notification(2), 101)
            .await
            .is_err()
    );
    assert_eq!(
        loops
            .registration("team", &registered.registration.id)
            .await
            .unwrap()
            .unwrap()
            .observed_cursor,
        1
    );
    let limits = LoopLimits {
        sources_per_activation: 4,
        ..limits
    };
    fixture
        .event_policy(LoopPolicyState::Suspended, 2, &limits)
        .await;
    fixture
        .store
        .accept_signed_event(&app.id, 1, &notification(2), 102)
        .await
        .unwrap();
    assert!(loops.reconcile_schedules(104).await.unwrap().is_empty());
    let fired = loops.reconcile_schedules(105).await.unwrap();
    assert_eq!(fired.len(), 1);
    assert_eq!((fired[0].first_cursor, fired[0].through_cursor), (1, 2));
    assert!(loops.reconcile_schedules(106).await.unwrap().is_empty());
    fixture.close().await;
}

#[tokio::test]
async fn app_condition_route_replacement_fences_old_intent_and_retains_original_event_version() {
    let fixture = Fixture::new().await;
    let app = fixture.event_intake().await;
    let loops = LoopStore::new(fixture.store.pool.clone());
    let request = condition(&app.id, "old", 0, true);
    let old = loops.register_schedule(&request, 90).await.unwrap();
    let direct = fixture
        .store
        .accept_signed_event(&app.id, 1, &notification(1), 100)
        .await
        .unwrap();
    let first = loops.reconcile_schedules(100).await.unwrap();
    assert_eq!(first.len(), 1);
    let manifest = fixture
        .store
        .version(&app.id, 1)
        .await
        .unwrap()
        .unwrap()
        .manifest;
    fixture
        .store
        .publish_version(&app.id, "owner", 1, &manifest, 101)
        .await
        .unwrap();
    let scopes = ["read".into()].into();
    fixture
        .store
        .approve_team(
            "owner",
            AppGrantUpdate {
                app_id: &app.id,
                team_id: "team",
                expected_revision: 1,
                scopes: &scopes,
            },
            101,
        )
        .await
        .unwrap();
    fixture
        .store
        .bind_member(
            AppBindingUpdate {
                app_id: &app.id,
                team_id: "team",
                actor_id: "worker",
                version: 1,
                expected_revision: 1,
                scopes: &scopes,
            },
            101,
        )
        .await
        .unwrap();
    fixture
        .store
        .configure_event_key(&app.id, 1, Some("ROTATED_KEY"), 101)
        .await
        .unwrap();
    assert_eq!(
        loops
            .registration("team", &old.registration.id)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopRegistrationState::Active
    );
    fixture
        .store
        .bind_member(
            AppBindingUpdate {
                app_id: &app.id,
                team_id: "team",
                actor_id: "worker",
                version: 2,
                expected_revision: 2,
                scopes: &scopes,
            },
            102,
        )
        .await
        .unwrap();
    assert_eq!(
        loops
            .registration("team", &old.registration.id)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopRegistrationState::Revoked
    );
    assert!(
        loops
            .register_schedule(&condition(&app.id, "unapproved", 0, true), 102)
            .await
            .is_err()
    );
    fixture
        .store
        .configure_event_route(
            AppEventRouteUpdate {
                app_id: &app.id,
                team_id: "team",
                actor_id: "worker",
                expected_revision: 1,
                classes: &["changed".into()].into(),
            },
            103,
        )
        .await
        .unwrap();
    let retry = loops.register_schedule(&request, 104).await.unwrap();
    assert!(retry.duplicate);
    assert_eq!(retry.registration.state, LoopRegistrationState::Revoked);
    loops
        .register_schedule(&condition(&app.id, "new", 0, true), 104)
        .await
        .unwrap();
    let second = loops.reconcile_schedules(104).await.unwrap();
    assert_eq!(second.len(), 1);
    let sources = loops
        .triggers("team", &second[0].receipt.activation_id)
        .await
        .unwrap();
    let source = sources
        .iter()
        .find(|s| s.id == second[0].receipt.trigger_id)
        .unwrap();
    assert_eq!(
        source.input.references.app_event.as_ref().unwrap().version,
        1
    );
    let sources = loops.triggers("team", &direct.activation_id).await.unwrap();
    assert!(
        sources
            .iter()
            .find(|s| s.id == first[0].receipt.trigger_id)
            .unwrap()
            .revoked
    );
    assert!(
        !sources
            .iter()
            .find(|s| s.id == direct.trigger_id)
            .unwrap()
            .revoked
    );
    fixture.close().await;
}
