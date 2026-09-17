use std::path::PathBuf;

use agenthub_agent_domain::app_tools::{AppConnection, AppManifest, AppReplayPolicy, AppTool};
use serde_json::json;

use super::*;

mod binding_tests;
mod pin_tests;

struct Fixture {
    path: PathBuf,
    store: AppRegistry,
}

impl Fixture {
    async fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("app-registry-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("control.sqlite");
        let pool = crate::init_db_at_path(&path).await.unwrap();
        for user in ["owner", "other"] {
            sqlx::query(
                "INSERT INTO users(id, username, display_name, role, created_at) VALUES (?, ?, ?, 'admin', 1)",
            )
            .bind(user)
            .bind(user)
            .bind(user)
            .execute(&pool)
            .await
            .unwrap();
        }
        Self {
            path,
            store: AppRegistry::new(pool),
        }
    }

    async fn register(&self) -> RegisteredApp {
        self.store
            .register(
                RegisterApp {
                    owner_user_id: "owner",
                    name: "Fixture tools",
                    connection: &connection(),
                    manifest: &manifest(),
                },
                10,
            )
            .await
            .unwrap()
    }

    async fn close(self) {
        self.store.pool.close().await;
        std::fs::remove_dir_all(self.path.parent().unwrap()).unwrap();
    }
}

fn connection() -> AppConnection {
    AppConnection {
        endpoint: "https://tools.example.test/mcp".into(),
        credential_env: Some("PRIVATE_APP_TOKEN".into()),
        authority: "tools.example.test".into(),
        namespace: "fixture".into(),
    }
}

fn manifest() -> AppManifest {
    AppManifest {
        schema_version: 1,
        scopes: ["read".into(), "write".into()].into(),
        tools: vec![AppTool {
            name: "lookup".into(),
            input_schema: json!({"type":"object","properties":{"key":{"type":"string"}},"required":["key"]}),
            output_schema: Some(json!({"type":"object"})),
            required_scopes: ["read".into()].into(),
            replay: AppReplayPolicy::ReadOnly,
        }],
    }
}

#[tokio::test]
async fn app_versions_are_immutable_owned_and_survive_migration_and_reopen() {
    let mut fixture = Fixture::new().await;
    migrate_app_registry(&fixture.store.pool).await.unwrap();
    assert!(
        fixture
            .store
            .list_owned("owner", None, 100)
            .await
            .unwrap()
            .is_empty()
    );
    let app = fixture.register().await;
    assert_eq!(
        fixture.store.credential_references().await.unwrap(),
        vec!["PRIVATE_APP_TOKEN"]
    );
    let original = fixture.store.version(&app.id, 1).await.unwrap().unwrap();
    let mut second = manifest();
    second.tools[0].input_schema["properties"]["key"]["minLength"] = json!(1);
    let published = fixture
        .store
        .publish_version(&app.id, "owner", 1, &second, 20)
        .await
        .unwrap();
    assert_eq!(published.version, 2);
    assert_eq!(
        fixture.store.version(&app.id, 1).await.unwrap().unwrap(),
        original
    );
    assert!(fixture.store.version(&app.id, 3).await.unwrap().is_none());
    for (user, revision, expected) in [("other", 2, "ownership"), ("owner", 1, "revision")] {
        let error = fixture
            .store
            .publish_version(&app.id, user, revision, &second, 30)
            .await
            .unwrap_err();
        assert!(error.to_string().contains(expected));
    }
    assert!(
        fixture
            .store
            .list_owned("other", None, 100)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        fixture
            .store
            .list_owned("owner", None, 1000)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        fixture
            .store
            .list_owned("owner", Some(&app.id), 100)
            .await
            .unwrap()
            .is_empty()
    );
    let safe = serde_json::to_string(&app).unwrap();
    assert!(
        !safe.contains("PRIVATE_APP_TOKEN")
            && !safe.contains("endpoint")
            && !safe.contains("namespace")
    );
    fixture.store.pool.close().await;
    fixture.store = AppRegistry::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    assert_eq!(
        fixture.store.version(&app.id, 1).await.unwrap().unwrap(),
        original
    );
    assert_eq!(
        fixture.store.version(&app.id, 2).await.unwrap().unwrap(),
        published
    );
    assert!(fixture.store.connection(&app.id).await.unwrap() == connection());
    assert_eq!(
        fixture.store.app(&app.id).await.unwrap().unwrap().revision,
        2
    );
    fixture.close().await;
}

#[tokio::test]
async fn app_revocation_keeps_history_and_prevents_authority_alias_re_registration() {
    let fixture = Fixture::new().await;
    let app = fixture.register().await;
    assert!(
        fixture
            .store
            .revoke_app(&app.id, "other", 1, 20)
            .await
            .is_err()
    );
    assert!(
        fixture
            .store
            .revoke_app(&app.id, "owner", 0, 20)
            .await
            .is_err()
    );
    let revoked = fixture
        .store
        .revoke_app(&app.id, "owner", 1, 20)
        .await
        .unwrap();
    assert_eq!(revoked.revoked_at, Some(20));
    assert_eq!(revoked.revision, 2);
    assert!(fixture.store.connection(&app.id).await.is_err());
    assert_eq!(
        fixture.store.credential_references().await.unwrap(),
        vec!["PRIVATE_APP_TOKEN"]
    );
    assert!(
        fixture
            .store
            .publish_version(&app.id, "owner", 2, &manifest(), 21)
            .await
            .is_err()
    );
    assert_eq!(
        fixture
            .store
            .version(&app.id, 1)
            .await
            .unwrap()
            .unwrap()
            .manifest,
        manifest()
    );
    // Changing display identity cannot create a new journal scope for the same external effects.
    assert!(
        fixture
            .store
            .register(
                RegisterApp {
                    owner_user_id: "owner",
                    name: "Another name",
                    connection: &connection(),
                    manifest: &manifest(),
                },
                30
            )
            .await
            .is_err()
    );
    assert_eq!(
        fixture
            .store
            .list_owned("owner", None, 100)
            .await
            .unwrap()
            .len(),
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn app_publication_is_atomic_under_concurrent_revision_updates() {
    let fixture = Fixture::new().await;
    let app = fixture.register().await;
    let input = manifest();
    let (first, second) = tokio::join!(
        fixture
            .store
            .publish_version(&app.id, "owner", 1, &input, 20),
        fixture
            .store
            .publish_version(&app.id, "owner", 1, &input, 20),
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let error = if let Err(error) = first {
        error
    } else {
        second.unwrap_err()
    };
    assert!(matches!(
        error.downcast_ref::<AppStoreError>(),
        Some(AppStoreError::RevisionConflict)
    ));
    assert_eq!(
        fixture
            .store
            .app(&app.id)
            .await
            .unwrap()
            .unwrap()
            .latest_version,
        2
    );
    assert!(fixture.store.version(&app.id, 3).await.unwrap().is_none());
    let mut invalid = manifest();
    invalid.tools[0].required_scopes.insert("undeclared".into());
    assert!(
        fixture
            .store
            .publish_version(&app.id, "owner", 2, &invalid, 21)
            .await
            .is_err()
    );
    assert_eq!(
        fixture.store.app(&app.id).await.unwrap().unwrap().revision,
        2
    );
    fixture.close().await;
}
