use std::collections::BTreeSet;

use agenthub_agent_domain::app_events::APP_EVENT_MAX_CLASSES;
use agenthub_agent_domain::app_tools::{AppManifest, valid_name};
use serde::Serialize;
use sqlx::{Row, Sqlite, Transaction, sqlite::SqliteRow};

use super::{AppRegistry, AppStoreError, registration::next_revision};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AppEventRoute {
    pub app_id: String,
    pub team_id: String,
    pub actor_id: String,
    pub version: i64,
    pub classes: BTreeSet<String>,
    pub grant_epoch: i64,
    pub binding_epoch: i64,
    pub revision: i64,
    pub revoked_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct AppEventRouteUpdate<'a> {
    pub app_id: &'a str,
    pub team_id: &'a str,
    pub actor_id: &'a str,
    pub expected_revision: i64,
    pub classes: &'a BTreeSet<String>,
}

impl AppRegistry {
    pub async fn event_route(
        &self,
        app_id: &str,
        team_id: &str,
        actor_id: &str,
    ) -> anyhow::Result<Option<AppEventRoute>> {
        sqlx::query(
            "SELECT * FROM app_event_routes WHERE app_id = ? AND team_id = ? AND actor_id = ?",
        )
        .bind(app_id)
        .bind(team_id)
        .bind(actor_id)
        .fetch_optional(&self.pool)
        .await?
        .as_ref()
        .map(parse_route)
        .transpose()
    }

    /// Team-owner authorization precedes this transaction. Tools alone never grant wakeup authority.
    pub async fn configure_event_route(
        &self,
        input: AppEventRouteUpdate<'_>,
        now: i64,
    ) -> anyhow::Result<AppEventRoute> {
        anyhow::ensure!(now >= 0, "invalid app timestamp");
        anyhow::ensure!(
            !input.classes.is_empty()
                && input.classes.len() <= APP_EVENT_MAX_CLASSES
                && input.classes.iter().all(|name| valid_name(name)),
            "invalid app event classes"
        );
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let binding = current_binding(&mut tx, input.app_id, input.team_id, input.actor_id).await?;
        let manifest: AppManifest = serde_json::from_str(binding.try_get("manifest_json")?)?;
        let scopes: BTreeSet<String> = serde_json::from_str(binding.try_get("scopes_json")?)?;
        let grant_scopes: BTreeSet<String> =
            serde_json::from_str(binding.try_get("granted_scopes")?)?;
        let allowed = manifest
            .compile()?
            .allowed_events(&scopes.intersection(&grant_scopes).cloned().collect())?;
        anyhow::ensure!(
            input.classes.is_subset(&allowed),
            AppStoreError::ScopeMismatch
        );
        let revision: i64 = sqlx::query_scalar("SELECT revision FROM app_event_routes WHERE app_id = ? AND team_id = ? AND actor_id = ?")
            .bind(input.app_id).bind(input.team_id).bind(input.actor_id).fetch_optional(&mut *tx).await?.unwrap_or(0);
        anyhow::ensure!(
            revision == input.expected_revision,
            AppStoreError::RevisionConflict
        );
        let row = sqlx::query("INSERT INTO app_event_routes(app_id, team_id, actor_id, version, classes_json, grant_epoch, binding_epoch, revision, created_at, updated_at) \
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(app_id, team_id, actor_id) DO UPDATE SET \
            version = excluded.version, classes_json = excluded.classes_json, grant_epoch = excluded.grant_epoch, binding_epoch = excluded.binding_epoch, \
            revision = excluded.revision, revoked_at = NULL, updated_at = excluded.updated_at RETURNING *")
            .bind(input.app_id).bind(input.team_id).bind(input.actor_id).bind(binding.try_get::<i64, _>("version")?)
            .bind(serde_json::to_string(input.classes)?).bind(binding.try_get::<i64, _>("grant_epoch")?)
            .bind(binding.try_get::<i64, _>("authorization_epoch")?).bind(next_revision(revision)?).bind(now).bind(now)
            .fetch_one(&mut *tx).await?;
        let route = parse_route(&row)?;
        crate::loop_runtime::LoopStore::revoke_app_schedules_tx(
            &mut tx,
            input.app_id,
            Some(input.team_id),
            Some(input.actor_id),
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(route)
    }

    pub async fn revoke_event_route(
        &self,
        app_id: &str,
        team_id: &str,
        actor_id: &str,
        expected_revision: i64,
        now: i64,
    ) -> anyhow::Result<AppEventRoute> {
        anyhow::ensure!(now >= 0, "invalid app timestamp");
        let revision = next_revision(expected_revision)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let row = sqlx::query("UPDATE app_event_routes SET revoked_at = COALESCE(revoked_at, ?), revision = ?, updated_at = ? \
            WHERE app_id = ? AND team_id = ? AND actor_id = ? AND revision = ? RETURNING *")
            .bind(now).bind(revision).bind(now).bind(app_id).bind(team_id).bind(actor_id).bind(expected_revision)
            .fetch_optional(&mut *tx).await?.ok_or(AppStoreError::RevisionConflict)?;
        let route = parse_route(&row)?;
        crate::loop_runtime::LoopStore::revoke_app_schedules_tx(
            &mut tx,
            app_id,
            Some(team_id),
            Some(actor_id),
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(route)
    }

    /// Also used by standing conditions, under their existing canonical transaction.
    pub async fn authorize_event_route_tx(
        tx: &mut Transaction<'_, Sqlite>,
        app_id: &str,
        team_id: &str,
        actor_id: &str,
        class: &str,
    ) -> anyhow::Result<AppEventRoute> {
        let binding = current_binding(tx, app_id, team_id, actor_id).await?;
        let row = sqlx::query("SELECT * FROM app_event_routes WHERE app_id = ? AND team_id = ? AND actor_id = ? AND revoked_at IS NULL")
            .bind(app_id).bind(team_id).bind(actor_id).fetch_optional(&mut **tx).await?.ok_or(AppStoreError::Revoked)?;
        let route = parse_route(&row)?;
        anyhow::ensure!(
            route.version == binding.try_get::<i64, _>("version")?
                && route.grant_epoch == binding.try_get::<i64, _>("grant_epoch")?
                && route.binding_epoch == binding.try_get::<i64, _>("authorization_epoch")?,
            AppStoreError::Revoked
        );
        anyhow::ensure!(route.classes.contains(class), AppStoreError::ScopeMismatch);
        // Stored manifests were validated at publication; avoid recompiling all tool schemas at ingress.
        let manifest: AppManifest = serde_json::from_str(binding.try_get("manifest_json")?)?;
        let declaration = manifest
            .events
            .iter()
            .find(|event| event.name == class)
            .ok_or(AppStoreError::ScopeMismatch)?;
        let scopes: BTreeSet<String> = serde_json::from_str(binding.try_get("scopes_json")?)?;
        let granted: BTreeSet<String> = serde_json::from_str(binding.try_get("granted_scopes")?)?;
        anyhow::ensure!(
            declaration.required_scopes.is_subset(&scopes)
                && declaration.required_scopes.is_subset(&granted),
            AppStoreError::ScopeMismatch
        );
        Ok(route)
    }
}

async fn current_binding(
    tx: &mut Transaction<'_, Sqlite>,
    app_id: &str,
    team_id: &str,
    actor_id: &str,
) -> anyhow::Result<SqliteRow> {
    crate::loop_runtime::require_member(tx, team_id, actor_id).await?;
    sqlx::query("SELECT b.*, g.scopes_json AS granted_scopes, g.authorization_epoch AS grant_epoch, v.manifest_json \
        FROM app_member_bindings b JOIN app_team_grants g ON g.app_id = b.app_id AND g.team_id = b.team_id \
        JOIN registered_apps a ON a.id = b.app_id JOIN app_manifest_versions v ON v.app_id = b.app_id AND v.version = b.version \
        WHERE b.app_id = ? AND b.team_id = ? AND b.actor_id = ? AND b.revoked_at IS NULL AND g.revoked_at IS NULL AND a.revoked_at IS NULL")
        .bind(app_id).bind(team_id).bind(actor_id).fetch_optional(&mut **tx).await?.ok_or_else(|| AppStoreError::Revoked.into())
}

fn parse_route(row: &SqliteRow) -> anyhow::Result<AppEventRoute> {
    Ok(AppEventRoute {
        app_id: row.try_get("app_id")?,
        team_id: row.try_get("team_id")?,
        actor_id: row.try_get("actor_id")?,
        version: row.try_get("version")?,
        classes: serde_json::from_str(row.try_get("classes_json")?)?,
        grant_epoch: row.try_get("grant_epoch")?,
        binding_epoch: row.try_get("binding_epoch")?,
        revision: row.try_get("revision")?,
        revoked_at: row.try_get("revoked_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
