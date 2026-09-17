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
        CREATE TABLE IF NOT EXISTS app_event_key_versions (
            app_id TEXT NOT NULL REFERENCES registered_apps(id),
            version INTEGER NOT NULL CHECK(version > 0),
            credential_env TEXT,
            revoked_at INTEGER,
            created_at INTEGER NOT NULL,
            PRIMARY KEY(app_id, version),
            CHECK(credential_env IS NOT NULL OR revoked_at IS NOT NULL)
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_app_event_active_key
            ON app_event_key_versions(app_id) WHERE revoked_at IS NULL;
        CREATE TABLE IF NOT EXISTS app_event_routes (
            app_id TEXT NOT NULL,
            team_id TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            version INTEGER NOT NULL,
            classes_json TEXT NOT NULL,
            grant_epoch INTEGER NOT NULL CHECK(grant_epoch > 0),
            binding_epoch INTEGER NOT NULL CHECK(binding_epoch > 0),
            revision INTEGER NOT NULL CHECK(revision > 0),
            revoked_at INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY(app_id, team_id, actor_id),
            FOREIGN KEY(app_id, team_id, actor_id) REFERENCES app_member_bindings(app_id, team_id, actor_id),
            FOREIGN KEY(app_id, version) REFERENCES app_manifest_versions(app_id, version)
        );
        CREATE TABLE IF NOT EXISTS app_event_cursors (
            app_id TEXT PRIMARY KEY REFERENCES registered_apps(id),
            cursor INTEGER NOT NULL CHECK(cursor > 0)
        );
        CREATE TABLE IF NOT EXISTS app_event_receipts (
            app_id TEXT NOT NULL REFERENCES registered_apps(id),
            event_id TEXT NOT NULL,
            cursor INTEGER NOT NULL CHECK(cursor > 0),
            team_id TEXT NOT NULL REFERENCES team_definitions(id),
            actor_id TEXT NOT NULL REFERENCES agents(id),
            event_class TEXT NOT NULL,
            notification_json TEXT NOT NULL,
            version INTEGER NOT NULL,
            signing_version INTEGER NOT NULL,
            route_revision INTEGER NOT NULL,
            trigger_id TEXT NOT NULL REFERENCES loop_trigger_sources(id),
            created_at INTEGER NOT NULL,
            PRIMARY KEY(app_id, event_id),
            UNIQUE(app_id, cursor),
            FOREIGN KEY(app_id, version) REFERENCES app_manifest_versions(app_id, version),
            FOREIGN KEY(app_id, signing_version) REFERENCES app_event_key_versions(app_id, version)
        );
        CREATE INDEX IF NOT EXISTS idx_app_event_receipt_route
            ON app_event_receipts(app_id, team_id, actor_id, event_class, cursor);
        CREATE TABLE IF NOT EXISTS app_event_budgets (
            scope_kind TEXT NOT NULL CHECK(scope_kind IN ('app', 'actor', 'team')),
            scope_id TEXT NOT NULL,
            window_started_at INTEGER NOT NULL,
            accepted_count INTEGER NOT NULL CHECK(accepted_count > 0),
            PRIMARY KEY(scope_kind, scope_id)
        );
        CREATE TABLE IF NOT EXISTS app_event_denials (
            app_id TEXT NOT NULL REFERENCES registered_apps(id),
            code TEXT NOT NULL CHECK(code IN ('unauthorized', 'id_conflict', 'cursor_replay', 'capacity', 'disabled')),
            count INTEGER NOT NULL CHECK(count > 0),
            last_event_id TEXT NOT NULL,
            last_seen_at INTEGER NOT NULL,
            PRIMARY KEY(app_id, code)
        );
    "#).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
