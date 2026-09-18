use super::{event_intake::notification, event_scheduling::condition, *};
use crate::loop_runtime::{LoopPolicyUpdate, LoopStore, LoopStoreError};
use agenthub_agent_domain::{
    loop_runtime::{LoopLimits, LoopPolicyState, LoopSessionPolicy},
    loop_scheduling::LoopRegistrationState,
};

#[tokio::test]
async fn app_conditions_keep_task_and_origin_revocation_semantics() {
    for origin in [false, true] {
        let fixture = Fixture::new().await;
        let app = fixture.event_intake().await;
        let loops = LoopStore::new(fixture.store.pool.clone());
        let event = fixture
            .store
            .accept_signed_event(&app.id, 1, &notification(1), 100)
            .await
            .unwrap();
        let mut request = condition(&app.id, "owned", 0, true);
        if origin {
            request.references.scheduling_actor_id = Some("worker".into());
            request.references.scheduling_activation_id = Some(event.activation_id.clone());
        } else {
            sqlx::query("INSERT INTO team_tasks(id, team_id, title, status, created_by_actor_id, context_json, created_at, updated_at) VALUES ('work', 'team', 'Work', 'open', 'worker', '{}', 100, 100)")
                .execute(&fixture.store.pool).await.unwrap();
            request.work_task_id = Some("work".into());
        }
        let registered = loops.register_schedule(&request, 101).await.unwrap();
        let fired = loops.reconcile_schedules(101).await.unwrap();
        assert_eq!(fired.len(), 1);
        if origin {
            loops
                .cancel("team", &event.activation_id, 102)
                .await
                .unwrap();
        } else {
            let mut tx = fixture
                .store
                .pool
                .begin_with("BEGIN IMMEDIATE")
                .await
                .unwrap();
            sqlx::query(
                "UPDATE team_tasks SET status = 'completed', updated_at = 102 WHERE id = 'work'",
            )
            .execute(&mut *tx)
            .await
            .unwrap();
            LoopStore::observe_task_schedule_tx(&mut tx, "team", "work", 102)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }
        assert_eq!(
            loops
                .registration("team", &registered.registration.id)
                .await
                .unwrap()
                .unwrap()
                .state,
            LoopRegistrationState::Revoked
        );
        let sources = loops
            .triggers("team", &fired[0].receipt.activation_id)
            .await
            .unwrap();
        assert!(
            sources
                .iter()
                .find(|s| s.id == fired[0].receipt.trigger_id)
                .unwrap()
                .revoked
        );
        fixture
            .store
            .accept_signed_event(&app.id, 1, &notification(2), 103)
            .await
            .unwrap();
        assert!(loops.reconcile_schedules(103).await.unwrap().is_empty());
        fixture.close().await;
    }
}

#[tokio::test]
async fn every_app_authority_change_retires_idle_and_completed_conditions_immediately() {
    for change in [
        "app",
        "grant_revoke",
        "grant_scopes",
        "binding_revoke",
        "binding_scopes",
        "route_replace",
        "route_revoke",
    ] {
        let fixture = Fixture::new().await;
        let app = fixture.event_intake().await;
        let scopes = ["read".into(), "write".into()].into();
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
                80,
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
                80,
            )
            .await
            .unwrap();
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
                80,
            )
            .await
            .unwrap();
        let loops = LoopStore::new(fixture.store.pool.clone());
        let idle = loops
            .register_schedule(&condition(&app.id, "idle", 1000, true), 90)
            .await
            .unwrap();
        let once = loops
            .register_schedule(&condition(&app.id, "once", 0, false), 90)
            .await
            .unwrap();
        let direct = fixture
            .store
            .accept_signed_event(&app.id, 1, &notification(1), 100)
            .await
            .unwrap();
        let fired = loops.reconcile_schedules(100).await.unwrap();
        assert_eq!(fired.len(), 1);
        let read = ["read".into()].into();
        match change {
            "app" => {
                fixture
                    .store
                    .revoke_app(&app.id, "owner", 1, 101)
                    .await
                    .unwrap();
            }
            "grant_revoke" => {
                fixture
                    .store
                    .revoke_team_grant(&app.id, "team", 2, 101)
                    .await
                    .unwrap();
            }
            "grant_scopes" => {
                fixture
                    .store
                    .approve_team(
                        "owner",
                        AppGrantUpdate {
                            app_id: &app.id,
                            team_id: "team",
                            expected_revision: 2,
                            scopes: &read,
                        },
                        101,
                    )
                    .await
                    .unwrap();
            }
            "binding_revoke" => {
                fixture
                    .store
                    .revoke_member_binding(&app.id, "team", "worker", 2, 101)
                    .await
                    .unwrap();
            }
            "binding_scopes" => {
                fixture
                    .store
                    .bind_member(
                        AppBindingUpdate {
                            app_id: &app.id,
                            team_id: "team",
                            actor_id: "worker",
                            version: 1,
                            expected_revision: 2,
                            scopes: &read,
                        },
                        101,
                    )
                    .await
                    .unwrap();
            }
            "route_replace" => {
                fixture
                    .store
                    .configure_event_route(
                        AppEventRouteUpdate {
                            app_id: &app.id,
                            team_id: "team",
                            actor_id: "worker",
                            expected_revision: 2,
                            classes: &["changed".into()].into(),
                        },
                        101,
                    )
                    .await
                    .unwrap();
            }
            "route_revoke" => {
                fixture
                    .store
                    .revoke_event_route(&app.id, "team", "worker", 2, 101)
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        }
        for id in [&idle.registration.id, &once.registration.id] {
            let state = loops.registration("team", id).await.unwrap().unwrap();
            assert_eq!(state.state, LoopRegistrationState::Revoked, "{change}");
            assert!(state.pending_cursor.is_none());
        }
        let sources = loops.triggers("team", &direct.activation_id).await.unwrap();
        assert!(
            sources
                .iter()
                .find(|source| source.id == fired[0].receipt.trigger_id)
                .unwrap()
                .revoked,
            "{change}"
        );
        assert!(
            !sources
                .iter()
                .find(|source| source.id == direct.trigger_id)
                .unwrap()
                .revoked,
            "{change}"
        );
        assert!(loops.reconcile_schedules(102).await.unwrap().is_empty());
        if change != "app" {
            let grant = fixture
                .store
                .team_grant(&app.id, "team")
                .await
                .unwrap()
                .unwrap();
            fixture
                .store
                .approve_team(
                    "owner",
                    AppGrantUpdate {
                        app_id: &app.id,
                        team_id: "team",
                        expected_revision: grant.revision,
                        scopes: &scopes,
                    },
                    102,
                )
                .await
                .unwrap();
            let binding = fixture
                .store
                .member_binding(&app.id, "team", "worker")
                .await
                .unwrap()
                .unwrap();
            fixture
                .store
                .bind_member(
                    AppBindingUpdate {
                        app_id: &app.id,
                        team_id: "team",
                        actor_id: "worker",
                        version: 1,
                        expected_revision: binding.revision,
                        scopes: &scopes,
                    },
                    102,
                )
                .await
                .unwrap();
            let route = fixture
                .store
                .event_route(&app.id, "team", "worker")
                .await
                .unwrap()
                .unwrap();
            fixture
                .store
                .configure_event_route(
                    AppEventRouteUpdate {
                        app_id: &app.id,
                        team_id: "team",
                        actor_id: "worker",
                        expected_revision: route.revision,
                        classes: &["changed".into()].into(),
                    },
                    102,
                )
                .await
                .unwrap();
            fixture
                .store
                .accept_signed_event(&app.id, 1, &notification(1001), 103)
                .await
                .unwrap();
            assert!(
                loops.reconcile_schedules(103).await.unwrap().is_empty(),
                "{change}"
            );
            // Revocation frees standing capacity even when the old condition was never due.
            fixture
                .event_policy(
                    LoopPolicyState::Suspended,
                    1,
                    &LoopLimits {
                        standing_per_actor: 1,
                        ..LoopLimits::default()
                    },
                )
                .await;
            loops
                .register_schedule(&condition(&app.id, "fresh", 1001, true), 104)
                .await
                .unwrap();
        }
        fixture.close().await;
    }
}

#[tokio::test]
async fn app_conditions_cannot_observe_another_members_events_or_unapproved_classes() {
    let fixture = Fixture::new().await;
    let app = fixture.event_intake().await;
    let loops = LoopStore::new(fixture.store.pool.clone());
    for (team, actor) in [("team", "other"), ("elsewhere", "outsider")] {
        loops
            .configure(
                LoopPolicyUpdate {
                    team_id: team,
                    actor_id: actor,
                    expected_revision: 0,
                    state: LoopPolicyState::Suspended,
                    session_policy: LoopSessionPolicy::Fresh,
                    limits: &LoopLimits::default(),
                },
                90,
            )
            .await
            .unwrap();
    }
    for (team, actor, schedule) in [
        (
            "team",
            "other",
            condition(&app.id, "unbound", 0, true).schedule,
        ),
        (
            "elsewhere",
            "outsider",
            condition(&app.id, "foreign", 0, true).schedule,
        ),
        (
            "team",
            "worker",
            condition("missing-app", "missing", 0, true).schedule,
        ),
        (
            "team",
            "worker",
            agenthub_agent_domain::loop_scheduling::LoopSchedule::AppEvent {
                app_id: app.id.clone(),
                event_class: "written".into(),
                after_cursor: 0,
                repeat: true,
            },
        ),
    ] {
        let mut request = condition(&app.id, "denied", 0, true);
        request.team_id = team.into();
        request.actor_id = actor.into();
        request.schedule = schedule;
        let error = loops.register_schedule(&request, 90).await.unwrap_err();
        assert!(matches!(
            error.downcast_ref::<LoopStoreError>(),
            Some(LoopStoreError::ScopeMismatch)
        ));
    }
    fixture
        .store
        .bind_member(
            AppBindingUpdate {
                app_id: &app.id,
                team_id: "team",
                actor_id: "other",
                version: 1,
                expected_revision: 0,
                scopes: &["read".into()].into(),
            },
            90,
        )
        .await
        .unwrap();
    fixture
        .store
        .configure_event_route(
            AppEventRouteUpdate {
                app_id: &app.id,
                team_id: "team",
                actor_id: "other",
                expected_revision: 0,
                classes: &["changed".into()].into(),
            },
            90,
        )
        .await
        .unwrap();
    let worker = loops
        .register_schedule(&condition(&app.id, "worker", 0, true), 90)
        .await
        .unwrap();
    let mut other = condition(&app.id, "other", 0, true);
    other.actor_id = "other".into();
    let other = loops.register_schedule(&other, 90).await.unwrap();
    fixture
        .store
        .accept_signed_event(&app.id, 1, &notification(1), 100)
        .await
        .unwrap();
    assert_eq!(
        loops
            .registration("team", &other.registration.id)
            .await
            .unwrap()
            .unwrap()
            .observed_cursor,
        0
    );
    let mut event = notification(2);
    event.actor_id = "other".into();
    fixture
        .store
        .accept_signed_event(&app.id, 1, &event, 101)
        .await
        .unwrap();
    assert_eq!(
        loops
            .registration("team", &worker.registration.id)
            .await
            .unwrap()
            .unwrap()
            .observed_cursor,
        1
    );
    let caught_up = loops
        .register_schedule(&condition(&app.id, "catchup", 0, false), 102)
        .await
        .unwrap();
    assert_eq!(
        (
            caught_up.registration.pending_cursor,
            caught_up.registration.observed_cursor
        ),
        (Some(1), 1)
    );
    let fired = loops.reconcile_schedules(102).await.unwrap();
    assert_eq!(fired.len(), 3);
    for firing in fired {
        let expected = if firing.registration_id == other.registration.id {
            2
        } else {
            1
        };
        assert_eq!(
            (firing.first_cursor, firing.through_cursor),
            (expected, expected)
        );
    }
    fixture.close().await;
}

#[tokio::test]
async fn app_condition_firing_rechecks_membership_and_watch_revision() {
    for change in ["membership", "revision", "missing_watch"] {
        let fixture = Fixture::new().await;
        let app = fixture.event_intake().await;
        let loops = LoopStore::new(fixture.store.pool.clone());
        let registered = loops
            .register_schedule(&condition(&app.id, "recheck", 0, true), 90)
            .await
            .unwrap();
        fixture
            .store
            .accept_signed_event(&app.id, 1, &notification(1), 100)
            .await
            .unwrap();
        // Simulate a stale/corrupt watch or a membership change independently of registry hooks.
        match change {
            "membership" => {
                sqlx::query(
                    "UPDATE team_definitions SET spec_json = '{\"members\":[]}' WHERE id = 'team'",
                )
                .execute(&fixture.store.pool)
                .await
                .unwrap();
            }
            "revision" => {
                sqlx::query("UPDATE app_event_watches SET route_revision = route_revision + 1")
                    .execute(&fixture.store.pool)
                    .await
                    .unwrap();
            }
            _ => {
                sqlx::query("DELETE FROM app_event_watches")
                    .execute(&fixture.store.pool)
                    .await
                    .unwrap();
            }
        }
        assert!(loops.reconcile_schedules(100).await.unwrap().is_empty());
        assert_eq!(
            loops
                .registration("team", &registered.registration.id)
                .await
                .unwrap()
                .unwrap()
                .state,
            LoopRegistrationState::Revoked
        );
        fixture.close().await;
    }
}
