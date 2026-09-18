use agenthub_agent_domain::{
    app_tools::{AppConnection, AppManifest},
    loop_runtime::validate_loop_id,
};
use sqlx::{Row, Sqlite, Transaction, sqlite::SqliteRow};

use super::{AppRegistry, AppStoreError, AppVersion, RegisteredApp};

pub struct RegisterApp<'a> {
    pub owner_user_id: &'a str,
    pub name: &'a str,
    pub connection: &'a AppConnection,
    pub manifest: &'a AppManifest,
}

impl AppRegistry {
    /// Only the instance-configuration capability may provision connection/credential references.
    pub async fn register(
        &self,
        input: RegisterApp<'_>,
        now: i64,
    ) -> anyhow::Result<RegisteredApp> {
        validate_loop_id(input.owner_user_id)?;
        anyhow::ensure!(
            !input.name.trim().is_empty() && input.name.len() <= 128,
            "invalid app display name"
        );
        anyhow::ensure!(now >= 0, "invalid app timestamp");
        input.connection.validate()?;
        input.manifest.compile()?;
        let id = uuid::Uuid::now_v7().to_string();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let row = sqlx::query("INSERT INTO registered_apps(id, owner_user_id, name, connection_json, authority, namespace, revision, latest_version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 1, 1, ?, ?) RETURNING *")
            .bind(&id).bind(input.owner_user_id).bind(input.name.trim()).bind(serde_json::to_string(input.connection)?)
            .bind(&input.connection.authority).bind(&input.connection.namespace).bind(now).bind(now)
            .fetch_one(&mut *tx).await?;
        sqlx::query("INSERT INTO app_manifest_versions(app_id, version, manifest_json, created_at) VALUES (?, 1, ?, ?)")
            .bind(&id).bind(serde_json::to_string(input.manifest)?).bind(now).execute(&mut *tx).await?;
        let app = parse_app(&row)?;
        tx.commit().await?;
        Ok(app)
    }

    pub async fn app(&self, app_id: &str) -> anyhow::Result<Option<RegisteredApp>> {
        sqlx::query("SELECT * FROM registered_apps WHERE id = ?")
            .bind(app_id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(parse_app)
            .transpose()
    }

    pub async fn list_owned(
        &self,
        owner_user_id: &str,
        after: Option<&str>,
        limit: u32,
    ) -> anyhow::Result<Vec<RegisteredApp>> {
        let rows = sqlx::query("SELECT * FROM registered_apps WHERE owner_user_id = ? AND (? IS NULL OR id > ?) ORDER BY id LIMIT ?")
            .bind(owner_user_id).bind(after).bind(after).bind(limit.clamp(1, 100))
            .fetch_all(&self.pool).await?;
        rows.iter().map(parse_app).collect()
    }

    /// Not a user-facing projection. The launch service alone resolves this environment reference.
    pub async fn connection(&self, app_id: &str) -> anyhow::Result<AppConnection> {
        let raw: String = sqlx::query_scalar(
            "SELECT connection_json FROM registered_apps WHERE id = ? AND revoked_at IS NULL",
        )
        .bind(app_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AppStoreError::Revoked)?;
        let connection: AppConnection = serde_json::from_str(&raw)?;
        connection.validate()?;
        Ok(connection)
    }

    /// Strip all registered credentials from children, including unbound and revoked Apps.
    /// References stay daemon-private and must never be returned by management projections.
    pub async fn credential_references(&self) -> anyhow::Result<Vec<String>> {
        let names: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT json_extract(connection_json, '$.credential_env') AS name \
             FROM registered_apps WHERE json_type(connection_json, '$.credential_env') = 'text' \
             UNION SELECT credential_env AS name FROM app_event_key_versions WHERE credential_env IS NOT NULL ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        anyhow::ensure!(
            names
                .iter()
                .all(|name| agenthub_agent_domain::app_tools::valid_credential_reference(name)),
            "invalid app credential reference"
        );
        Ok(names)
    }

    pub async fn version(&self, app_id: &str, version: i64) -> anyhow::Result<Option<AppVersion>> {
        sqlx::query("SELECT * FROM app_manifest_versions WHERE app_id = ? AND version = ?")
            .bind(app_id)
            .bind(version)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(parse_version)
            .transpose()
    }

    pub async fn publish_version(
        &self,
        app_id: &str,
        owner_user_id: &str,
        expected_revision: i64,
        manifest: &AppManifest,
        now: i64,
    ) -> anyhow::Result<AppVersion> {
        anyhow::ensure!(now >= 0, "invalid app timestamp");
        manifest.compile()?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let app = require_owner(&mut tx, app_id, owner_user_id).await?;
        anyhow::ensure!(app.revoked_at.is_none(), AppStoreError::Revoked);
        anyhow::ensure!(
            app.revision == expected_revision,
            AppStoreError::RevisionConflict
        );
        anyhow::ensure!(app.latest_version < 1024, AppStoreError::Capacity);
        let next = app.latest_version + 1;
        let revision = next_revision(app.revision)?;
        let row = sqlx::query("INSERT INTO app_manifest_versions(app_id, version, manifest_json, created_at) VALUES (?, ?, ?, ?) RETURNING *")
            .bind(app_id).bind(next).bind(serde_json::to_string(manifest)?).bind(now)
            .fetch_one(&mut *tx).await?;
        sqlx::query("UPDATE registered_apps SET latest_version = ?, revision = ?, updated_at = ? WHERE id = ?")
            .bind(next).bind(revision).bind(now).bind(app_id).execute(&mut *tx).await?;
        let version = parse_version(&row)?;
        tx.commit().await?;
        Ok(version)
    }

    pub async fn revoke_app(
        &self,
        app_id: &str,
        owner_user_id: &str,
        expected_revision: i64,
        now: i64,
    ) -> anyhow::Result<RegisteredApp> {
        anyhow::ensure!(now >= 0, "invalid app timestamp");
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let app = require_owner(&mut tx, app_id, owner_user_id).await?;
        anyhow::ensure!(
            app.revision == expected_revision,
            AppStoreError::RevisionConflict
        );
        let row = sqlx::query("UPDATE registered_apps SET revoked_at = COALESCE(revoked_at, ?), revision = ?, updated_at = ? WHERE id = ? RETURNING *")
            .bind(now).bind(next_revision(app.revision)?).bind(now).bind(app_id).fetch_one(&mut *tx).await?;
        let app = parse_app(&row)?;
        crate::loop_runtime::LoopStore::revoke_app_schedules_tx(&mut tx, app_id, None, None, now)
            .await?;
        tx.commit().await?;
        Ok(app)
    }
}

pub(super) async fn require_owner(
    tx: &mut Transaction<'_, Sqlite>,
    app_id: &str,
    owner_user_id: &str,
) -> anyhow::Result<RegisteredApp> {
    let row = sqlx::query("SELECT * FROM registered_apps WHERE id = ?")
        .bind(app_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppStoreError::NotFound)?;
    let app = parse_app(&row)?;
    anyhow::ensure!(app.owner_user_id == owner_user_id, AppStoreError::Forbidden);
    Ok(app)
}

pub(super) fn next_revision(revision: i64) -> anyhow::Result<i64> {
    revision
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("app record revision exhausted"))
}

fn parse_app(row: &SqliteRow) -> anyhow::Result<RegisteredApp> {
    Ok(RegisteredApp {
        id: row.try_get("id")?,
        owner_user_id: row.try_get("owner_user_id")?,
        name: row.try_get("name")?,
        revision: row.try_get("revision")?,
        latest_version: row.try_get("latest_version")?,
        revoked_at: row.try_get("revoked_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

pub(super) fn parse_version(row: &SqliteRow) -> anyhow::Result<AppVersion> {
    Ok(AppVersion {
        app_id: row.try_get("app_id")?,
        version: row.try_get("version")?,
        manifest: serde_json::from_str(row.try_get("manifest_json")?)?,
        created_at: row.try_get("created_at")?,
    })
}
