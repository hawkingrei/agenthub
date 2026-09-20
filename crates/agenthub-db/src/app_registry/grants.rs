use std::collections::BTreeSet;

use agenthub_agent_domain::app_tools::{AppManifest, valid_name};
use sqlx::{Row, Sqlite, Transaction, sqlite::SqliteRow};

use super::{
    AppRegistry, AppStoreError, AppTeamGrant,
    registration::{next_revision, require_owner},
};

pub struct AppGrantUpdate<'a> {
    pub app_id: &'a str,
    pub team_id: &'a str,
    pub expected_revision: i64,
    pub scopes: &'a BTreeSet<String>,
}

impl AppRegistry {
    pub async fn team_grant(
        &self,
        app_id: &str,
        team_id: &str,
    ) -> anyhow::Result<Option<AppTeamGrant>> {
        sqlx::query("SELECT * FROM app_team_grants WHERE app_id = ? AND team_id = ?")
            .bind(app_id)
            .bind(team_id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(parse_grant)
            .transpose()
    }

    /// Team ownership is checked by the API; app ownership is also checked under this write lock.
    pub async fn approve_team(
        &self,
        owner_user_id: &str,
        input: AppGrantUpdate<'_>,
        now: i64,
    ) -> anyhow::Result<AppTeamGrant> {
        validate_scopes(input.scopes)?;
        anyhow::ensure!(now >= 0, "invalid app timestamp");
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let app = require_owner(&mut tx, input.app_id, owner_user_id).await?;
        anyhow::ensure!(app.revoked_at.is_none(), AppStoreError::Revoked);
        let manifest = manifest_tx(&mut tx, input.app_id, app.latest_version).await?;
        anyhow::ensure!(
            input.scopes.is_subset(&manifest.scopes),
            AppStoreError::ScopeMismatch
        );
        let existing =
            sqlx::query("SELECT * FROM app_team_grants WHERE app_id = ? AND team_id = ?")
                .bind(input.app_id)
                .bind(input.team_id)
                .fetch_optional(&mut *tx)
                .await?
                .as_ref()
                .map(parse_grant)
                .transpose()?;
        anyhow::ensure!(
            existing.as_ref().map_or(0, |grant| grant.revision) == input.expected_revision,
            AppStoreError::RevisionConflict
        );
        if existing
            .as_ref()
            .is_none_or(|grant| grant.revoked_at.is_some())
        {
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM app_team_grants WHERE team_id = ? AND revoked_at IS NULL",
            )
            .bind(input.team_id)
            .fetch_one(&mut *tx)
            .await?;
            anyhow::ensure!(count < 64, AppStoreError::Capacity);
        }
        let epoch = authorization_epoch(existing.as_ref().map(|grant| {
            (
                grant.authorization_epoch,
                grant.revoked_at.is_some() || grant.scopes != *input.scopes,
            )
        }))?;
        let row = sqlx::query("INSERT INTO app_team_grants(app_id, team_id, scopes_json, revision, authorization_epoch, created_at, updated_at) \
            VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(app_id, team_id) DO UPDATE SET scopes_json = excluded.scopes_json, \
            revision = excluded.revision, authorization_epoch = excluded.authorization_epoch, revoked_at = NULL, updated_at = excluded.updated_at RETURNING *")
            .bind(input.app_id).bind(input.team_id).bind(serde_json::to_string(input.scopes)?)
            .bind(next_revision(input.expected_revision)?).bind(epoch).bind(now).bind(now)
            .fetch_one(&mut *tx).await?;
        let grant = parse_grant(&row)?;
        if existing
            .as_ref()
            .is_some_and(|previous| previous.authorization_epoch != grant.authorization_epoch)
        {
            crate::loop_runtime::LoopStore::revoke_app_schedules_tx(
                &mut tx,
                input.app_id,
                Some(input.team_id),
                None,
                now,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(grant)
    }

    /// Revocation may be requested by the Team owner even after the app itself was revoked.
    pub async fn revoke_team_grant(
        &self,
        app_id: &str,
        team_id: &str,
        expected_revision: i64,
        now: i64,
    ) -> anyhow::Result<AppTeamGrant> {
        anyhow::ensure!(now >= 0, "invalid app timestamp");
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let row = sqlx::query("SELECT * FROM app_team_grants WHERE app_id = ? AND team_id = ?")
            .bind(app_id)
            .bind(team_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppStoreError::NotFound)?;
        let previous = parse_grant(&row)?;
        anyhow::ensure!(
            previous.revision == expected_revision,
            AppStoreError::RevisionConflict
        );
        let row = sqlx::query("UPDATE app_team_grants SET revoked_at = COALESCE(revoked_at, ?), revision = ?, authorization_epoch = ?, updated_at = ? WHERE app_id = ? AND team_id = ? RETURNING *")
            .bind(now).bind(next_revision(previous.revision)?).bind(next_revision(previous.authorization_epoch)?)
            .bind(now).bind(app_id).bind(team_id).fetch_one(&mut *tx).await?;
        let grant = parse_grant(&row)?;
        crate::loop_runtime::LoopStore::revoke_app_schedules_tx(
            &mut tx,
            app_id,
            Some(team_id),
            None,
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(grant)
    }
}

pub(super) fn authorization_epoch(previous: Option<(i64, bool)>) -> anyhow::Result<i64> {
    match previous {
        Some((epoch, true)) => next_revision(epoch),
        Some((epoch, false)) => Ok(epoch),
        None => Ok(1),
    }
}

pub(super) fn validate_scopes(scopes: &BTreeSet<String>) -> anyhow::Result<()> {
    anyhow::ensure!(
        !scopes.is_empty() && scopes.len() <= 64 && scopes.iter().all(|scope| valid_name(scope)),
        "invalid app grant scopes"
    );
    Ok(())
}

pub(super) async fn manifest_tx(
    tx: &mut Transaction<'_, Sqlite>,
    app_id: &str,
    version: i64,
) -> anyhow::Result<AppManifest> {
    let raw: String = sqlx::query_scalar("SELECT v.manifest_json FROM app_manifest_versions v JOIN registered_apps a ON a.id = v.app_id WHERE v.app_id = ? AND v.version = ? AND a.revoked_at IS NULL")
        .bind(app_id).bind(version).fetch_optional(&mut **tx).await?.ok_or(AppStoreError::NotFound)?;
    Ok(serde_json::from_str(&raw)?)
}

pub(super) fn parse_grant(row: &SqliteRow) -> anyhow::Result<AppTeamGrant> {
    Ok(AppTeamGrant {
        app_id: row.try_get("app_id")?,
        team_id: row.try_get("team_id")?,
        scopes: serde_json::from_str(row.try_get("scopes_json")?)?,
        revision: row.try_get("revision")?,
        authorization_epoch: row.try_get("authorization_epoch")?,
        revoked_at: row.try_get("revoked_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
