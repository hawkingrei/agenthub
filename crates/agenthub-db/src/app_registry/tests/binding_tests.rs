use super::*;

impl Fixture {
    pub(super) async fn prepare_members(&self) {
        for actor in ["worker", "other", "outsider"] {
            sqlx::query("INSERT INTO agents(id, name, workdir, command, args, worktree_mode, status, created_at, updated_at) \
                VALUES (?, ?, '/tmp', 'fixture', '[]', 'use_existing', 'created', 1, 1)")
                .bind(actor).bind(actor).execute(&self.store.pool).await.unwrap();
        }
        for (team, actors) in [
            ("team", vec!["worker", "other"]),
            ("elsewhere", vec!["outsider"]),
        ] {
            let spec = json!({"members": actors.into_iter().map(|actor| json!({"member_id":actor})).collect::<Vec<_>>()});
            sqlx::query("INSERT INTO team_definitions(id, name, spec_json, created_at, updated_at) VALUES (?, ?, ?, 1, 1)")
                .bind(team).bind(team).bind(spec.to_string()).execute(&self.store.pool).await.unwrap();
        }
    }
}

#[tokio::test]
async fn team_approval_and_manifest_scopes_bound_explicit_member_grants() {
    let fixture = Fixture::new().await;
    fixture.prepare_members().await;
    let app = fixture.register().await;
    let read = ["read".into()].into();
    let write = ["write".into()].into();
    let grant = AppGrantUpdate {
        app_id: &app.id,
        team_id: "team",
        expected_revision: 0,
        scopes: &read,
    };
    assert!(
        fixture
            .store
            .approve_team("other", grant, 11)
            .await
            .is_err()
    );
    let grant = fixture
        .store
        .approve_team(
            "owner",
            AppGrantUpdate {
                app_id: &app.id,
                team_id: "team",
                expected_revision: 0,
                scopes: &read,
            },
            11,
        )
        .await
        .unwrap();
    assert_eq!(grant.authorization_epoch, 1);
    assert!(
        fixture
            .store
            .member_bindings("team", "worker", None, 100)
            .await
            .unwrap()
            .is_empty()
    );
    for (actor, version, revision, scopes) in [
        ("outsider", 1, 0, &read),
        ("worker", 9, 0, &read),
        ("worker", 1, 1, &read),
        ("worker", 1, 0, &write),
    ] {
        assert!(
            fixture
                .store
                .bind_member(
                    AppBindingUpdate {
                        app_id: &app.id,
                        team_id: "team",
                        actor_id: actor,
                        version,
                        expected_revision: revision,
                        scopes
                    },
                    12
                )
                .await
                .is_err()
        );
    }
    let binding = fixture
        .store
        .bind_member(
            AppBindingUpdate {
                app_id: &app.id,
                team_id: "team",
                actor_id: "worker",
                version: 1,
                expected_revision: 0,
                scopes: &read,
            },
            13,
        )
        .await
        .unwrap();
    assert_eq!(binding.authorization_epoch, 1);
    assert_eq!(binding.scopes, read);
    assert_eq!(
        fixture
            .store
            .member_bindings("team", "worker", None, 100)
            .await
            .unwrap()
            .as_slice(),
        std::slice::from_ref(&binding)
    );
    assert!(
        fixture
            .store
            .member_bindings("team", "worker", Some(&app.id), 100)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .store
            .member_binding(&app.id, "team", "other")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        fixture
            .store
            .member_binding(&app.id, "elsewhere", "worker")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        fixture.store.team_grant(&app.id, "team").await.unwrap(),
        Some(grant)
    );
    assert!(
        fixture
            .store
            .team_grant(&app.id, "elsewhere")
            .await
            .unwrap()
            .is_none()
    );
    fixture.close().await;
}

#[tokio::test]
async fn version_selection_preserves_epochs_but_permission_changes_and_rebinding_do_not() {
    let fixture = Fixture::new().await;
    fixture.prepare_members().await;
    let app = fixture.register().await;
    let all = ["read".into(), "write".into()].into();
    let read = ["read".into()].into();
    fixture
        .store
        .approve_team(
            "owner",
            AppGrantUpdate {
                app_id: &app.id,
                team_id: "team",
                expected_revision: 0,
                scopes: &all,
            },
            11,
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
                expected_revision: 0,
                scopes: &all,
            },
            12,
        )
        .await
        .unwrap();
    fixture
        .store
        .publish_version(&app.id, "owner", 1, &manifest(), 13)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .member_binding(&app.id, "team", "worker")
            .await
            .unwrap()
            .unwrap()
            .version,
        1
    );
    let selected = fixture
        .store
        .bind_member(
            AppBindingUpdate {
                app_id: &app.id,
                team_id: "team",
                actor_id: "worker",
                version: 2,
                expected_revision: 1,
                scopes: &all,
            },
            14,
        )
        .await
        .unwrap();
    assert_eq!((selected.revision, selected.authorization_epoch), (2, 1));
    let narrowed = fixture
        .store
        .bind_member(
            AppBindingUpdate {
                app_id: &app.id,
                team_id: "team",
                actor_id: "worker",
                version: 2,
                expected_revision: 2,
                scopes: &read,
            },
            15,
        )
        .await
        .unwrap();
    assert_eq!((narrowed.revision, narrowed.authorization_epoch), (3, 2));
    assert!(
        fixture
            .store
            .revoke_member_binding(&app.id, "team", "worker", 2, 16)
            .await
            .is_err()
    );
    let revoked = fixture
        .store
        .revoke_member_binding(&app.id, "team", "worker", 3, 16)
        .await
        .unwrap();
    assert_eq!(revoked.revoked_at, Some(16));
    assert_eq!(revoked.authorization_epoch, 3);
    let rebound = fixture
        .store
        .bind_member(
            AppBindingUpdate {
                app_id: &app.id,
                team_id: "team",
                actor_id: "worker",
                version: 1,
                expected_revision: 4,
                scopes: &read,
            },
            17,
        )
        .await
        .unwrap();
    assert_eq!(rebound.authorization_epoch, 4);
    assert_eq!(rebound.revoked_at, None);
    fixture.close().await;
}

#[tokio::test]
async fn revoked_team_grants_require_fresh_approval_and_keep_a_distinct_epoch() {
    let fixture = Fixture::new().await;
    fixture.prepare_members().await;
    let app = fixture.register().await;
    let read = ["read".into()].into();
    fixture
        .store
        .approve_team(
            "owner",
            AppGrantUpdate {
                app_id: &app.id,
                team_id: "team",
                expected_revision: 0,
                scopes: &read,
            },
            11,
        )
        .await
        .unwrap();
    let same = fixture
        .store
        .approve_team(
            "owner",
            AppGrantUpdate {
                app_id: &app.id,
                team_id: "team",
                expected_revision: 1,
                scopes: &read,
            },
            12,
        )
        .await
        .unwrap();
    assert_eq!((same.revision, same.authorization_epoch), (2, 1));
    assert!(
        fixture
            .store
            .revoke_team_grant(&app.id, "team", 1, 13)
            .await
            .is_err()
    );
    let revoked = fixture
        .store
        .revoke_team_grant(&app.id, "team", 2, 13)
        .await
        .unwrap();
    assert_eq!(revoked.authorization_epoch, 2);
    assert!(
        fixture
            .store
            .bind_member(
                AppBindingUpdate {
                    app_id: &app.id,
                    team_id: "team",
                    actor_id: "worker",
                    version: 1,
                    expected_revision: 0,
                    scopes: &read
                },
                14
            )
            .await
            .is_err()
    );
    let reapproved = fixture
        .store
        .approve_team(
            "owner",
            AppGrantUpdate {
                app_id: &app.id,
                team_id: "team",
                expected_revision: 3,
                scopes: &read,
            },
            15,
        )
        .await
        .unwrap();
    assert_eq!(reapproved.authorization_epoch, 3);
    assert_eq!(reapproved.revoked_at, None);
    fixture
        .store
        .revoke_app(&app.id, "owner", 1, 16)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .approve_team(
                "owner",
                AppGrantUpdate {
                    app_id: &app.id,
                    team_id: "team",
                    expected_revision: 4,
                    scopes: &read
                },
                17
            )
            .await
            .is_err()
    );
    fixture
        .store
        .revoke_team_grant(&app.id, "team", 4, 17)
        .await
        .unwrap();
    fixture.close().await;
}
