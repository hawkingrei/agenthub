use agenthub_agent_domain::app_tools::valid_credential_reference;
use serde::Serialize;
use sqlx::{Row, sqlite::SqliteRow};

use super::{AppRegistry, AppStoreError, registration::next_revision};

const MAX_KEY_INSTALLATIONS: i64 = 1024;

/// Safe inspection data. Even environment variable names stay daemon-private.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AppEventKey {
    pub app_id: String,
    pub version: i64,
    pub revoked_at: Option<i64>,
    pub created_at: i64,
}

pub struct AppEventSigningKey {
    pub config: AppEventKey,
    pub credential_env: String,
}

impl AppRegistry {
    /// Instance configuration authority must be checked before selecting a daemon secret.
    /// None records an explicit revocation; all former references remain isolated from children.
    pub async fn configure_event_key(
        &self,
        app_id: &str,
        expected_version: i64,
        credential_env: Option<&str>,
        now: i64,
    ) -> anyhow::Result<AppEventKey> {
        anyhow::ensure!(now >= 0, "invalid app timestamp");
        anyhow::ensure!(
            credential_env.is_none_or(valid_credential_reference),
            "invalid app event key reference"
        );
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let revoked: Option<i64> =
            sqlx::query_scalar("SELECT revoked_at FROM registered_apps WHERE id = ?")
                .bind(app_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(AppStoreError::NotFound)?;
        anyhow::ensure!(
            credential_env.is_none() || revoked.is_none(),
            AppStoreError::Revoked
        );
        let current: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version), 0) FROM app_event_key_versions WHERE app_id = ?",
        )
        .bind(app_id)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(current == expected_version, AppStoreError::RevisionConflict);
        if credential_env.is_some() {
            let installed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app_event_key_versions WHERE app_id = ? AND credential_env IS NOT NULL")
                .bind(app_id).fetch_one(&mut *tx).await?;
            anyhow::ensure!(installed < MAX_KEY_INSTALLATIONS, AppStoreError::Capacity);
        } else if current > 0 {
            let row = sqlx::query(
                "SELECT * FROM app_event_key_versions WHERE app_id = ? AND version = ?",
            )
            .bind(app_id)
            .bind(current)
            .fetch_one(&mut *tx)
            .await?;
            let key = parse_key(&row)?;
            // Revocation is always available, and retries cannot grow unbounded tombstone history.
            if key.revoked_at.is_some() {
                tx.commit().await?;
                return Ok(key);
            }
        }
        let version = next_revision(current)?;
        sqlx::query("UPDATE app_event_key_versions SET revoked_at = ? WHERE app_id = ? AND revoked_at IS NULL")
            .bind(now).bind(app_id).execute(&mut *tx).await?;
        let row = sqlx::query("INSERT INTO app_event_key_versions(app_id, version, credential_env, revoked_at, created_at) VALUES (?, ?, ?, ?, ?) RETURNING *")
            .bind(app_id).bind(version).bind(credential_env).bind(credential_env.is_none().then_some(now)).bind(now)
            .fetch_one(&mut *tx).await?;
        let key = parse_key(&row)?;
        tx.commit().await?;
        Ok(key)
    }

    pub async fn event_key(&self, app_id: &str) -> anyhow::Result<Option<AppEventKey>> {
        sqlx::query(
            "SELECT * FROM app_event_key_versions WHERE app_id = ? ORDER BY version DESC LIMIT 1",
        )
        .bind(app_id)
        .fetch_optional(&self.pool)
        .await?
        .as_ref()
        .map(parse_key)
        .transpose()
    }

    /// Verification happens outside the write lock; intake must recheck this version atomically.
    pub async fn event_signing_key(
        &self,
        app_id: &str,
    ) -> anyhow::Result<Option<AppEventSigningKey>> {
        let row = sqlx::query(
            "SELECT k.* FROM app_event_key_versions k JOIN registered_apps a ON a.id = k.app_id \
            WHERE k.app_id = ? AND k.revoked_at IS NULL AND a.revoked_at IS NULL",
        )
        .bind(app_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            let credential_env: String = row.try_get("credential_env")?;
            anyhow::ensure!(
                valid_credential_reference(&credential_env),
                "invalid app event key reference"
            );
            Ok(AppEventSigningKey {
                config: parse_key(&row)?,
                credential_env,
            })
        })
        .transpose()
    }
}

fn parse_key(row: &SqliteRow) -> anyhow::Result<AppEventKey> {
    Ok(AppEventKey {
        app_id: row.try_get("app_id")?,
        version: row.try_get("version")?,
        revoked_at: row.try_get("revoked_at")?,
        created_at: row.try_get("created_at")?,
    })
}
