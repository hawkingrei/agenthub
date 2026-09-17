use std::collections::BTreeSet;

use sqlx::{Row, sqlite::SqliteRow};

use super::{
    AppMemberBinding, AppRegistry, AppStoreError,
    grants::{authorization_epoch, manifest_tx, parse_grant, validate_scopes},
    registration::next_revision,
};

pub struct AppBindingUpdate<'a> {
    pub app_id: &'a str,
    pub team_id: &'a str,
    pub actor_id: &'a str,
    pub version: i64,
    pub expected_revision: i64,
    pub scopes: &'a BTreeSet<String>,
}

impl AppRegistry {
    pub async fn member_binding(
        &self,
        app_id: &str,
        team_id: &str,
        actor_id: &str,
    ) -> anyhow::Result<Option<AppMemberBinding>> {
        sqlx::query(
            "SELECT * FROM app_member_bindings WHERE app_id = ? AND team_id = ? AND actor_id = ?",
        )
        .bind(app_id)
        .bind(team_id)
        .bind(actor_id)
        .fetch_optional(&self.pool)
        .await?
        .as_ref()
        .map(parse_binding)
        .transpose()
    }

    pub async fn member_bindings(
        &self,
        team_id: &str,
        actor_id: &str,
        after: Option<&str>,
        limit: u32,
    ) -> anyhow::Result<Vec<AppMemberBinding>> {
        let rows = sqlx::query("SELECT * FROM app_member_bindings WHERE team_id = ? AND actor_id = ? AND (? IS NULL OR app_id > ?) ORDER BY app_id LIMIT ?")
            .bind(team_id).bind(actor_id).bind(after).bind(after).bind(limit.clamp(1, 100)).fetch_all(&self.pool).await?;
        rows.iter().map(parse_binding).collect()
    }

    /// Team owners select an explicit member and version within app-owner-approved Team scopes.
    pub async fn bind_member(
        &self,
        input: AppBindingUpdate<'_>,
        now: i64,
    ) -> anyhow::Result<AppMemberBinding> {
        validate_scopes(input.scopes)?;
        anyhow::ensure!(now >= 0, "invalid app timestamp");
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        crate::loop_runtime::require_member(&mut tx, input.team_id, input.actor_id).await?;
        let row = sqlx::query(
            "SELECT * FROM app_team_grants WHERE app_id = ? AND team_id = ? AND revoked_at IS NULL",
        )
        .bind(input.app_id)
        .bind(input.team_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppStoreError::Revoked)?;
        let grant = parse_grant(&row)?;
        let manifest = manifest_tx(&mut tx, input.app_id, input.version).await?;
        anyhow::ensure!(
            input.scopes.is_subset(&grant.scopes) && input.scopes.is_subset(&manifest.scopes),
            AppStoreError::ScopeMismatch
        );
        let previous = sqlx::query(
            "SELECT * FROM app_member_bindings WHERE app_id = ? AND team_id = ? AND actor_id = ?",
        )
        .bind(input.app_id)
        .bind(input.team_id)
        .bind(input.actor_id)
        .fetch_optional(&mut *tx)
        .await?
        .as_ref()
        .map(parse_binding)
        .transpose()?;
        anyhow::ensure!(
            previous.as_ref().map_or(0, |binding| binding.revision) == input.expected_revision,
            AppStoreError::RevisionConflict
        );
        if previous
            .as_ref()
            .is_none_or(|binding| binding.revoked_at.is_some())
        {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app_member_bindings WHERE team_id = ? AND actor_id = ? AND revoked_at IS NULL")
                .bind(input.team_id).bind(input.actor_id).fetch_one(&mut *tx).await?;
            anyhow::ensure!(count < 16, AppStoreError::Capacity);
        }
        // Version changes affect later launches. Permission changes invalidate old pinned grants.
        let epoch = authorization_epoch(previous.as_ref().map(|binding| {
            (
                binding.authorization_epoch,
                binding.revoked_at.is_some() || binding.scopes != *input.scopes,
            )
        }))?;
        let row = sqlx::query("INSERT INTO app_member_bindings(app_id, team_id, actor_id, version, scopes_json, revision, authorization_epoch, created_at, updated_at) \
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(app_id, team_id, actor_id) DO UPDATE SET version = excluded.version, \
            scopes_json = excluded.scopes_json, revision = excluded.revision, authorization_epoch = excluded.authorization_epoch, \
            revoked_at = NULL, updated_at = excluded.updated_at RETURNING *")
            .bind(input.app_id).bind(input.team_id).bind(input.actor_id).bind(input.version).bind(serde_json::to_string(input.scopes)?)
            .bind(next_revision(input.expected_revision)?).bind(epoch).bind(now).bind(now).fetch_one(&mut *tx).await?;
        let binding = parse_binding(&row)?;
        tx.commit().await?;
        Ok(binding)
    }

    pub async fn revoke_member_binding(
        &self,
        app_id: &str,
        team_id: &str,
        actor_id: &str,
        expected_revision: i64,
        now: i64,
    ) -> anyhow::Result<AppMemberBinding> {
        anyhow::ensure!(now >= 0, "invalid app timestamp");
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let row = sqlx::query(
            "SELECT * FROM app_member_bindings WHERE app_id = ? AND team_id = ? AND actor_id = ?",
        )
        .bind(app_id)
        .bind(team_id)
        .bind(actor_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppStoreError::NotFound)?;
        let previous = parse_binding(&row)?;
        anyhow::ensure!(
            previous.revision == expected_revision,
            AppStoreError::RevisionConflict
        );
        let row = sqlx::query("UPDATE app_member_bindings SET revoked_at = COALESCE(revoked_at, ?), revision = ?, authorization_epoch = ?, updated_at = ? WHERE app_id = ? AND team_id = ? AND actor_id = ? RETURNING *")
            .bind(now).bind(next_revision(previous.revision)?).bind(next_revision(previous.authorization_epoch)?).bind(now)
            .bind(app_id).bind(team_id).bind(actor_id).fetch_one(&mut *tx).await?;
        let binding = parse_binding(&row)?;
        tx.commit().await?;
        Ok(binding)
    }
}

pub(super) fn parse_binding(row: &SqliteRow) -> anyhow::Result<AppMemberBinding> {
    Ok(AppMemberBinding {
        app_id: row.try_get("app_id")?,
        team_id: row.try_get("team_id")?,
        actor_id: row.try_get("actor_id")?,
        version: row.try_get("version")?,
        scopes: serde_json::from_str(row.try_get("scopes_json")?)?,
        revision: row.try_get("revision")?,
        authorization_epoch: row.try_get("authorization_epoch")?,
        revoked_at: row.try_get("revoked_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
