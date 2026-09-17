use sqlx::SqlitePool;

pub async fn migrate_app_registry(pool: &SqlitePool) -> anyhow::Result<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::raw_sql(r#"
        CREATE TABLE IF NOT EXISTS registered_apps (
            id TEXT PRIMARY KEY,
            owner_user_id TEXT NOT NULL REFERENCES users(id),
            name TEXT NOT NULL,
            connection_json TEXT NOT NULL,
            authority TEXT NOT NULL,
            namespace TEXT NOT NULL,
            revision INTEGER NOT NULL CHECK(revision > 0),
            latest_version INTEGER NOT NULL CHECK(latest_version > 0),
            revoked_at INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            UNIQUE(authority, namespace)
        );
        CREATE INDEX IF NOT EXISTS idx_registered_app_owner ON registered_apps(owner_user_id, id);
        CREATE TABLE IF NOT EXISTS app_manifest_versions (
            app_id TEXT NOT NULL REFERENCES registered_apps(id),
            version INTEGER NOT NULL CHECK(version > 0),
            manifest_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            PRIMARY KEY(app_id, version)
        );
        CREATE TABLE IF NOT EXISTS app_team_grants (
            app_id TEXT NOT NULL REFERENCES registered_apps(id),
            team_id TEXT NOT NULL REFERENCES team_definitions(id),
            scopes_json TEXT NOT NULL,
            revision INTEGER NOT NULL CHECK(revision > 0),
            authorization_epoch INTEGER NOT NULL CHECK(authorization_epoch > 0),
            revoked_at INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY(app_id, team_id)
        );
        CREATE TABLE IF NOT EXISTS app_member_bindings (
            app_id TEXT NOT NULL,
            team_id TEXT NOT NULL,
            actor_id TEXT NOT NULL REFERENCES agents(id),
            version INTEGER NOT NULL,
            scopes_json TEXT NOT NULL,
            revision INTEGER NOT NULL CHECK(revision > 0),
            authorization_epoch INTEGER NOT NULL CHECK(authorization_epoch > 0),
            revoked_at INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY(app_id, team_id, actor_id),
            FOREIGN KEY(app_id, team_id) REFERENCES app_team_grants(app_id, team_id),
            FOREIGN KEY(app_id, version) REFERENCES app_manifest_versions(app_id, version)
        );
        CREATE INDEX IF NOT EXISTS idx_app_member_bindings ON app_member_bindings(team_id, actor_id, app_id);
        CREATE TABLE IF NOT EXISTS app_activation_snapshots (
            activation_id TEXT PRIMARY KEY REFERENCES loop_activations(id),
            pinned_generation INTEGER NOT NULL CHECK(pinned_generation > 0),
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS app_activation_pins (
            activation_id TEXT NOT NULL REFERENCES app_activation_snapshots(activation_id),
            app_id TEXT NOT NULL,
            team_id TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            version INTEGER NOT NULL,
            scopes_json TEXT NOT NULL,
            grant_revision INTEGER NOT NULL,
            binding_revision INTEGER NOT NULL,
            grant_epoch INTEGER NOT NULL,
            binding_epoch INTEGER NOT NULL,
            PRIMARY KEY(activation_id, app_id),
            FOREIGN KEY(app_id, team_id, actor_id) REFERENCES app_member_bindings(app_id, team_id, actor_id),
            FOREIGN KEY(app_id, version) REFERENCES app_manifest_versions(app_id, version)
        );
    "#).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
