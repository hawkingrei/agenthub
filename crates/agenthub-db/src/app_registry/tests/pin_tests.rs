use crate::loop_runtime::{LoopPolicyUpdate, LoopStore};
use agenthub_agent_domain::loop_runtime::{
    LoopAdmission, LoopCleanupDisposition, LoopLaunchSnapshot, LoopLimits, LoopPolicyState,
    LoopReservation, LoopSessionPolicy, LoopSourceReferences, LoopTriggerInput, LoopTriggerKind,
};

use super::*;

async fn starting(fixture: &Fixture) -> (LoopStore, LoopReservation) {
    let store = LoopStore::new(fixture.store.pool.clone());
    store
        .configure(
            LoopPolicyUpdate {
                actor_id: "worker",
                team_id: "team",
                expected_revision: 0,
                state: LoopPolicyState::Enabled,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits::default(),
            },
            100,
        )
        .await
        .unwrap();
    let receipt = store
        .accept_trigger(
            &LoopTriggerInput {
                actor_id: "worker".into(),
                team_id: "team".into(),
                kind: LoopTriggerKind::Operator,
                source_key: "operator".into(),
                due_at: None,
                references: LoopSourceReferences::default(),
            },
            100,
        )
        .await
        .unwrap();
    let LoopAdmission::Admitted(reservation) = store
        .admit("team", &receipt.activation_id, "daemon", 100)
        .await
        .unwrap()
    else {
        panic!("activation deferred")
    };
    sqlx::query("INSERT INTO team_runs(id, team_id, context_id, status, input_json, created_at) VALUES ('mailbox', 'team', 'loop', 'submitted', '{}', 100)")
        .execute(&fixture.store.pool).await.unwrap();
    sqlx::query("INSERT INTO loop_mailbox_partitions(run_id, team_id, created_at) VALUES ('mailbox', 'team', 100)")
        .execute(&fixture.store.pool).await.unwrap();
    store
        .bind_mailbox(&reservation, "mailbox", 100)
        .await
        .unwrap();
    (store, reservation)
}

async fn bootstrap(
    fixture: &Fixture,
    store: &LoopStore,
    reservation: &LoopReservation,
    now: i64,
) -> LoopReservation {
    store
        .record_launch(
            reservation,
            &LoopLaunchSnapshot {
                version: 1,
                provider_id: "fixture".into(),
                configuration_digest: "a".repeat(64),
                entry_prompt_version: "fixture-v1".into(),
                session_policy: LoopSessionPolicy::Fresh,
                workspace: "/tmp".into(),
                model: None,
                thinking_level: None,
            },
            now,
        )
        .await
        .unwrap();
    let session = format!("session-{}", reservation.generation);
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES (?, 'worker', 'running', ?)")
        .bind(&session).bind(now).execute(&fixture.store.pool).await.unwrap();
    store
        .bind_session(reservation, &session, now)
        .await
        .unwrap()
}

async fn approve_and_bind(fixture: &Fixture, app: &RegisteredApp, bind: bool) {
    let scopes = ["read".into()].into();
    fixture
        .store
        .approve_team(
            "owner",
            AppGrantUpdate {
                app_id: &app.id,
                team_id: "team",
                expected_revision: 0,
                scopes: &scopes,
            },
            11,
        )
        .await
        .unwrap();
    if bind {
        fixture
            .store
            .bind_member(
                AppBindingUpdate {
                    app_id: &app.id,
                    team_id: "team",
                    actor_id: "worker",
                    version: 1,
                    expected_revision: 0,
                    scopes: &scopes,
                },
                12,
            )
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn activation_selection_including_empty_pins_survives_startup_retry_and_version_changes() {
    for initially_bound in [false, true] {
        let fixture = Fixture::new().await;
        fixture.prepare_members().await;
        let app = fixture.register().await;
        approve_and_bind(&fixture, &app, initially_bound).await;
        let (store, first) = starting(&fixture).await;
        assert!(
            fixture
                .store
                .activation_selection("team", first.activation_id.as_deref().unwrap())
                .await
                .unwrap()
                .is_none()
        );
        let pins = fixture.store.pin_activation(&first, 101).await.unwrap();
        assert_eq!(pins.len(), usize::from(initially_bound));
        assert_eq!(
            fixture
                .store
                .activation_selection("team", first.activation_id.as_deref().unwrap())
                .await
                .unwrap(),
            Some(pins.clone())
        );
        assert!(
            fixture
                .store
                .activation_selection("elsewhere", first.activation_id.as_deref().unwrap())
                .await
                .unwrap()
                .is_none()
        );
        fixture
            .store
            .publish_version(&app.id, "owner", 1, &manifest(), 101)
            .await
            .unwrap();
        let scopes = ["read".into()].into();
        fixture
            .store
            .bind_member(
                AppBindingUpdate {
                    app_id: &app.id,
                    team_id: "team",
                    actor_id: "worker",
                    version: 2,
                    expected_revision: i64::from(initially_bound),
                    scopes: &scopes,
                },
                101,
            )
            .await
            .unwrap();
        assert_eq!(
            fixture.store.pin_activation(&first, 101).await.unwrap(),
            pins
        );
        store
            .cleanup_verified(&first, LoopCleanupDisposition::StartupFailed, 102)
            .await
            .unwrap();
        let LoopAdmission::Admitted(second) = store
            .admit(
                "team",
                first.activation_id.as_deref().unwrap(),
                "daemon",
                104,
            )
            .await
            .unwrap()
        else {
            panic!("retry deferred")
        };
        assert!(second.generation > first.generation);
        assert!(fixture.store.pin_activation(&first, 104).await.is_err());
        assert_eq!(
            fixture.store.pin_activation(&second, 104).await.unwrap(),
            pins
        );
        let ready = bootstrap(&fixture, &store, &second, 104).await;
        if initially_bound {
            let authorized = fixture
                .store
                .authorize_pin(&ready, &app.id, true, 104)
                .await
                .unwrap();
            assert_eq!(authorized.version, 1);
            assert_eq!(authorized.pinned_generation, first.generation);
            assert!(
                fixture
                    .store
                    .authorize_pin(&ready, &app.id, false, 104)
                    .await
                    .is_err()
            );
            store.mark_running(&ready, 105).await.unwrap();
            assert_eq!(
                fixture
                    .store
                    .authorize_pin(&ready, &app.id, false, 105)
                    .await
                    .unwrap(),
                pins[0]
            );
            assert!(fixture.store.pin_activation(&ready, 105).await.is_err());
        } else {
            assert!(
                fixture
                    .store
                    .authorize_pin(&ready, &app.id, true, 104)
                    .await
                    .is_err()
            );
        }
        fixture.close().await;
    }
}

#[tokio::test]
async fn durable_call_authorization_rejects_each_revocation_and_never_revives_old_pins() {
    for boundary in ["binding", "team", "app"] {
        let mut fixture = Fixture::new().await;
        fixture.prepare_members().await;
        let app = fixture.register().await;
        approve_and_bind(&fixture, &app, true).await;
        let (store, reservation) = starting(&fixture).await;
        let pins = fixture
            .store
            .pin_activation(&reservation, 101)
            .await
            .unwrap();
        let ready = bootstrap(&fixture, &store, &reservation, 101).await;
        store.mark_running(&ready, 102).await.unwrap();
        assert_eq!(
            fixture
                .store
                .authorize_pin(&ready, &app.id, false, 102)
                .await
                .unwrap(),
            pins[0]
        );
        for mutate in [0, 1, 2] {
            let mut stale = ready.clone();
            match mutate {
                0 => stale.generation += 1,
                1 => stale.owner_id = "foreign".into(),
                _ => stale.team_id = "elsewhere".into(),
            }
            assert!(
                fixture
                    .store
                    .authorize_pin(&stale, &app.id, true, 102)
                    .await
                    .is_err()
            );
        }
        assert!(
            fixture
                .store
                .authorize_pin(&ready, &app.id, false, 200)
                .await
                .is_err()
        );
        match boundary {
            "binding" => {
                fixture
                    .store
                    .revoke_member_binding(&app.id, "team", "worker", 1, 103)
                    .await
                    .unwrap();
            }
            "team" => {
                fixture
                    .store
                    .revoke_team_grant(&app.id, "team", 1, 103)
                    .await
                    .unwrap();
            }
            _ => {
                fixture
                    .store
                    .revoke_app(&app.id, "owner", 1, 103)
                    .await
                    .unwrap();
            }
        }
        assert!(
            fixture
                .store
                .authorize_pin(&ready, &app.id, false, 104)
                .await
                .is_err()
        );
        let scopes = ["read".into()].into();
        match boundary {
            "binding" => {
                fixture
                    .store
                    .bind_member(
                        AppBindingUpdate {
                            app_id: &app.id,
                            team_id: "team",
                            actor_id: "worker",
                            version: 1,
                            expected_revision: 2,
                            scopes: &scopes,
                        },
                        105,
                    )
                    .await
                    .unwrap();
            }
            "team" => {
                fixture
                    .store
                    .approve_team(
                        "owner",
                        AppGrantUpdate {
                            app_id: &app.id,
                            team_id: "team",
                            expected_revision: 2,
                            scopes: &scopes,
                        },
                        105,
                    )
                    .await
                    .unwrap();
            }
            _ => {}
        }
        assert!(
            fixture
                .store
                .authorize_pin(&ready, &app.id, false, 105)
                .await
                .is_err()
        );
        fixture.store.pool.close().await;
        fixture.store = AppRegistry::new(crate::init_db_at_path(&fixture.path).await.unwrap());
        assert!(
            fixture
                .store
                .authorize_pin(&ready, &app.id, false, 106)
                .await
                .is_err()
        );
        assert_eq!(
            fixture
                .store
                .activation_pins("team", ready.activation_id.as_deref().unwrap())
                .await
                .unwrap(),
            pins
        );
        assert!(
            fixture
                .store
                .activation_pins("elsewhere", ready.activation_id.as_deref().unwrap())
                .await
                .unwrap()
                .is_empty()
        );
        fixture.close().await;
    }
}
