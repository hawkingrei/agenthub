use agenthub_agent_domain::app_events::AppEventDeclaration;

use super::*;

#[tokio::test]
async fn exhausted_key_installations_cannot_block_revocation_or_grow_tombstones() {
    let fixture = Fixture::new().await;
    let app = fixture.register().await;
    sqlx::query("WITH RECURSIVE versions(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM versions WHERE n < 1024) \
        INSERT INTO app_event_key_versions(app_id, version, credential_env, revoked_at, created_at) \
        SELECT ?, n, 'LIMITED_EVENT_KEY', CASE WHEN n < 1024 THEN 1 ELSE NULL END, 1 FROM versions")
        .bind(&app.id).execute(&fixture.store.pool).await.unwrap();
    let error = fixture
        .store
        .configure_event_key(&app.id, 1024, Some("NEW_EVENT_KEY"), 11)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<AppStoreError>(),
        Some(AppStoreError::Capacity)
    ));
    let revoked = fixture
        .store
        .configure_event_key(&app.id, 1024, None, 12)
        .await
        .unwrap();
    assert_eq!(revoked.version, 1025);
    assert!(
        fixture
            .store
            .event_signing_key(&app.id)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        fixture
            .store
            .configure_event_key(&app.id, 1025, None, 13)
            .await
            .unwrap(),
        revoked
    );
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM app_event_key_versions WHERE app_id = ?")
            .bind(&app.id)
            .fetch_one(&fixture.store.pool)
            .await
            .unwrap();
    assert_eq!(count, 1025);
    fixture.close().await;
}
#[tokio::test]
async fn event_configuration_migrates_an_existing_registry_and_survives_reopen() {
    let mut fixture = Fixture::new().await;
    let app = fixture.register().await;
    sqlx::raw_sql("DROP TABLE app_event_routes; DROP TABLE app_event_key_versions;")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    migrate_app_registry(&fixture.store.pool).await.unwrap();
    migrate_app_registry(&fixture.store.pool).await.unwrap();
    assert_eq!(fixture.store.app(&app.id).await.unwrap(), Some(app.clone()));
    assert!(
        fixture
            .store
            .event_route(&app.id, "team", "worker")
            .await
            .unwrap()
            .is_none()
    );
    let key = fixture
        .store
        .configure_event_key(&app.id, 0, Some("EVENT_MIGRATION_KEY"), 11)
        .await
        .unwrap();
    fixture.store.pool.close().await;
    fixture.store = AppRegistry::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    assert_eq!(fixture.store.event_key(&app.id).await.unwrap(), Some(key));
    assert_eq!(
        fixture
            .store
            .event_signing_key(&app.id)
            .await
            .unwrap()
            .unwrap()
            .credential_env,
        "EVENT_MIGRATION_KEY"
    );
    assert_eq!(
        fixture.store.credential_references().await.unwrap(),
        ["EVENT_MIGRATION_KEY", "PRIVATE_APP_TOKEN"]
    );
    fixture.close().await;
}

#[tokio::test]
async fn signing_key_rotation_preserves_secret_isolation_and_revision_fencing() {
    let fixture = Fixture::new().await;
    let app = fixture.register().await;
    assert!(fixture.store.event_key(&app.id).await.unwrap().is_none());
    assert!(
        fixture
            .store
            .event_signing_key(&app.id)
            .await
            .unwrap()
            .is_none()
    );
    for bad in ["", "HOME", "AGENTHUB_PRIVATE", "key=value"] {
        assert!(
            fixture
                .store
                .configure_event_key(&app.id, 0, Some(bad), 11)
                .await
                .is_err()
        );
    }
    let first = fixture
        .store
        .configure_event_key(&app.id, 0, Some("PRIVATE_EVENT_KEY"), 11)
        .await
        .unwrap();
    let raw = serde_json::to_string(&first).unwrap();
    assert!(!raw.contains("PRIVATE") && !raw.contains("credential"));
    let key = fixture
        .store
        .event_signing_key(&app.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(key.config, first);
    assert_eq!(key.credential_env, "PRIVATE_EVENT_KEY");
    let (a, b) = tokio::join!(
        fixture
            .store
            .configure_event_key(&app.id, 1, Some("ROTATED_EVENT_KEY"), 12),
        fixture
            .store
            .configure_event_key(&app.id, 1, Some("ROTATED_EVENT_KEY"), 12),
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(
        fixture
            .store
            .event_key(&app.id)
            .await
            .unwrap()
            .unwrap()
            .version,
        2
    );
    assert_eq!(
        fixture
            .store
            .event_signing_key(&app.id)
            .await
            .unwrap()
            .unwrap()
            .credential_env,
        "ROTATED_EVENT_KEY"
    );
    let revoked = fixture
        .store
        .configure_event_key(&app.id, 2, None, 13)
        .await
        .unwrap();
    assert_eq!(revoked.version, 3);
    assert_eq!(revoked.revoked_at, Some(13));
    assert!(
        fixture
            .store
            .event_signing_key(&app.id)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        fixture.store.credential_references().await.unwrap(),
        vec![
            "PRIVATE_APP_TOKEN",
            "PRIVATE_EVENT_KEY",
            "ROTATED_EVENT_KEY"
        ]
    );
    migrate_app_registry(&fixture.store.pool).await.unwrap();
    assert_eq!(
        fixture.store.event_key(&app.id).await.unwrap(),
        Some(revoked)
    );
    fixture
        .store
        .configure_event_key(&app.id, 3, Some("PRIVATE_EVENT_KEY"), 14)
        .await
        .unwrap();
    fixture
        .store
        .revoke_app(&app.id, "owner", 1, 15)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .event_signing_key(&app.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        fixture
            .store
            .configure_event_key(&app.id, 4, Some("PRIVATE_EVENT_KEY"), 16)
            .await
            .is_err()
    );
    fixture
        .store
        .configure_event_key(&app.id, 4, None, 16)
        .await
        .unwrap();
    fixture.close().await;
}

impl Fixture {
    async fn event_app(&self) -> RegisteredApp {
        self.prepare_members().await;
        let mut manifest = manifest();
        manifest.events = vec![
            AppEventDeclaration {
                name: "changed".into(),
                required_scopes: ["read".into()].into(),
            },
            AppEventDeclaration {
                name: "written".into(),
                required_scopes: ["write".into()].into(),
            },
        ];
        let app = self
            .store
            .register(
                RegisterApp {
                    owner_user_id: "owner",
                    name: "Events",
                    connection: &connection(),
                    manifest: &manifest,
                },
                10,
            )
            .await
            .unwrap();
        let scopes = ["read".into()].into();
        self.store
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
        self.store
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
        app
    }

    async fn event_authority(
        &self,
        app: &str,
        team: &str,
        actor: &str,
        class: &str,
    ) -> anyhow::Result<AppEventRoute> {
        let mut tx = self.store.pool.begin().await?;
        AppRegistry::authorize_event_route_tx(&mut tx, app, team, actor, class).await
    }
}

#[tokio::test]
async fn event_routes_require_explicit_class_target_and_scope_authority() {
    let fixture = Fixture::new().await;
    let app = fixture.event_app().await;
    assert!(
        fixture
            .event_authority(&app.id, "team", "worker", "changed")
            .await
            .is_err()
    );
    assert!(
        fixture
            .store
            .event_route(&app.id, "team", "worker")
            .await
            .unwrap()
            .is_none()
    );
    for (actor, classes) in [
        ("worker", BTreeSet::new()),
        ("worker", ["written".into()].into()),
        ("worker", ["undeclared".into()].into()),
        ("outsider", ["changed".into()].into()),
    ] {
        assert!(
            fixture
                .store
                .configure_event_route(
                    AppEventRouteUpdate {
                        app_id: &app.id,
                        team_id: "team",
                        actor_id: actor,
                        expected_revision: 0,
                        classes: &classes,
                    },
                    13
                )
                .await
                .is_err()
        );
    }
    let route = fixture
        .store
        .configure_event_route(
            AppEventRouteUpdate {
                app_id: &app.id,
                team_id: "team",
                actor_id: "worker",
                expected_revision: 0,
                classes: &["changed".into()].into(),
            },
            13,
        )
        .await
        .unwrap();
    assert_eq!(
        fixture
            .event_authority(&app.id, "team", "worker", "changed")
            .await
            .unwrap(),
        route
    );
    assert_eq!(
        fixture
            .store
            .event_route(&app.id, "team", "worker")
            .await
            .unwrap(),
        Some(route)
    );
    for (team, actor, class) in [
        ("team", "other", "changed"),
        ("elsewhere", "worker", "changed"),
        ("team", "worker", "written"),
    ] {
        assert!(
            fixture
                .event_authority(&app.id, team, actor, class)
                .await
                .is_err()
        );
    }
    fixture
        .store
        .revoke_event_route(&app.id, "team", "worker", 1, 14)
        .await
        .unwrap();
    assert!(
        fixture
            .event_authority(&app.id, "team", "worker", "changed")
            .await
            .is_err()
    );
    let classes = ["changed".into()].into();
    let make = || AppEventRouteUpdate {
        app_id: &app.id,
        team_id: "team",
        actor_id: "worker",
        expected_revision: 2,
        classes: &classes,
    };
    let (a, b) = tokio::join!(
        fixture.store.configure_event_route(make(), 15),
        fixture.store.configure_event_route(make(), 15)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(
        fixture
            .event_authority(&app.id, "team", "worker", "changed")
            .await
            .unwrap()
            .revision,
        3
    );
    fixture.close().await;
}

#[tokio::test]
async fn version_or_authority_changes_require_explicit_event_route_reapproval() {
    let fixture = Fixture::new().await;
    let app = fixture.event_app().await;
    let classes = ["changed".into()].into();
    fixture
        .store
        .configure_event_route(
            AppEventRouteUpdate {
                app_id: &app.id,
                team_id: "team",
                actor_id: "worker",
                expected_revision: 0,
                classes: &classes,
            },
            13,
        )
        .await
        .unwrap();
    let manifest = fixture
        .store
        .version(&app.id, 1)
        .await
        .unwrap()
        .unwrap()
        .manifest;
    fixture
        .store
        .publish_version(&app.id, "owner", 1, &manifest, 14)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .event_authority(&app.id, "team", "worker", "changed")
            .await
            .unwrap()
            .version,
        1
    );
    let scopes = ["read".into()].into();
    fixture
        .store
        .bind_member(
            AppBindingUpdate {
                app_id: &app.id,
                team_id: "team",
                actor_id: "worker",
                version: 2,
                expected_revision: 1,
                scopes: &scopes,
            },
            15,
        )
        .await
        .unwrap();
    assert!(
        fixture
            .event_authority(&app.id, "team", "worker", "changed")
            .await
            .is_err()
    );
    let route = fixture
        .store
        .configure_event_route(
            AppEventRouteUpdate {
                app_id: &app.id,
                team_id: "team",
                actor_id: "worker",
                expected_revision: 1,
                classes: &classes,
            },
            16,
        )
        .await
        .unwrap();
    assert_eq!(route.version, 2);
    fixture
        .store
        .revoke_member_binding(&app.id, "team", "worker", 2, 17)
        .await
        .unwrap();
    fixture
        .store
        .bind_member(
            AppBindingUpdate {
                app_id: &app.id,
                team_id: "team",
                actor_id: "worker",
                version: 2,
                expected_revision: 3,
                scopes: &scopes,
            },
            18,
        )
        .await
        .unwrap();
    assert!(
        fixture
            .event_authority(&app.id, "team", "worker", "changed")
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
                expected_revision: 2,
                classes: &classes,
            },
            19,
        )
        .await
        .unwrap();
    fixture
        .store
        .revoke_team_grant(&app.id, "team", 1, 20)
        .await
        .unwrap();
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
            21,
        )
        .await
        .unwrap();
    assert!(
        fixture
            .event_authority(&app.id, "team", "worker", "changed")
            .await
            .is_err()
    );
    fixture.close().await;
}
