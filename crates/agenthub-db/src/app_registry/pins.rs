use std::collections::BTreeSet;

use agenthub_agent_domain::loop_runtime::LoopReservation;
use sqlx::{Row, Sqlite, Transaction, sqlite::SqliteRow};

use crate::loop_runtime::{LoopStore, LoopStoreError, require_live_reservation, require_member};

use super::{AppActivationPin, AppRegistry, AppStoreError, bindings::parse_binding};

impl AppRegistry {
    /// Freeze even an empty selection. Startup retries retain the activation's original versions.
    pub async fn pin_activation(
        &self,
        expected: &LoopReservation,
        now: i64,
    ) -> anyhow::Result<Vec<AppActivationPin>> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = require_live_reservation(&mut tx, expected, now).await?;
        require_member(&mut tx, &current.team_id, &current.actor_id).await?;
        let activation = current
            .activation_id
            .as_deref()
            .ok_or(LoopStoreError::InvalidState)?;
        let starting: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM loop_activations WHERE id = ? AND state = 'starting')",
        )
        .bind(activation)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(starting, LoopStoreError::InvalidState);
        let recorded: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM app_activation_snapshots WHERE activation_id = ?)",
        )
        .bind(activation)
        .fetch_one(&mut *tx)
        .await?;
        if !recorded {
            let rows = sqlx::query("SELECT b.*, g.scopes_json AS granted_scopes, g.revision AS grant_revision, g.authorization_epoch AS grant_epoch \
                FROM app_member_bindings b JOIN app_team_grants g ON g.app_id = b.app_id AND g.team_id = b.team_id \
                JOIN registered_apps a ON a.id = b.app_id \
                WHERE b.team_id = ? AND b.actor_id = ? AND b.revoked_at IS NULL AND g.revoked_at IS NULL AND a.revoked_at IS NULL ORDER BY b.app_id LIMIT 17")
                .bind(&current.team_id).bind(&current.actor_id).fetch_all(&mut *tx).await?;
            anyhow::ensure!(rows.len() <= 16, AppStoreError::Capacity);
            sqlx::query("INSERT INTO app_activation_snapshots(activation_id, pinned_generation, created_at) VALUES (?, ?, ?)")
                .bind(activation).bind(current.generation).bind(now).execute(&mut *tx).await?;
            for row in rows {
                let binding = parse_binding(&row)?;
                let granted: BTreeSet<String> =
                    serde_json::from_str(row.try_get("granted_scopes")?)?;
                let scopes: BTreeSet<String> =
                    binding.scopes.intersection(&granted).cloned().collect();
                if scopes.is_empty() {
                    continue;
                }
                sqlx::query("INSERT INTO app_activation_pins(activation_id, app_id, team_id, actor_id, version, scopes_json, grant_revision, binding_revision, grant_epoch, binding_epoch) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
                    .bind(activation).bind(&binding.app_id).bind(&binding.team_id).bind(&binding.actor_id).bind(binding.version)
                    .bind(serde_json::to_string(&scopes)?).bind(row.try_get::<i64, _>("grant_revision")?).bind(binding.revision)
                    .bind(row.try_get::<i64, _>("grant_epoch")?).bind(binding.authorization_epoch).execute(&mut *tx).await?;
            }
        }
        let pins = pins_tx(&mut tx, &current.team_id, activation).await?;
        tx.commit().await?;
        Ok(pins)
    }

    /// A historical projection remains readable after revocation; it confers no invocation authority.
    pub async fn activation_pins(
        &self,
        team_id: &str,
        activation_id: &str,
    ) -> anyhow::Result<Vec<AppActivationPin>> {
        let mut tx = self.pool.begin().await?;
        let pins = pins_tx(&mut tx, team_id, activation_id).await?;
        tx.commit().await?;
        Ok(pins)
    }

    /// Linearize each proxy admission against durable grants, including already-open sessions.
    /// A call admitted before revocation may finish; later admissions see the revoked epoch.
    pub async fn authorize_pin(
        &self,
        expected: &LoopReservation,
        app_id: &str,
        allow_bootstrap: bool,
        now: i64,
    ) -> anyhow::Result<AppActivationPin> {
        let mut tx = self.pool.begin().await?;
        LoopStore::verify_executor_phase_tx(&mut tx, expected, now, allow_bootstrap).await?;
        let row = sqlx::query("SELECT p.*, s.pinned_generation, g.authorization_epoch AS current_grant_epoch, b.authorization_epoch AS current_binding_epoch, \
            g.scopes_json AS current_granted_scopes, b.scopes_json AS current_bound_scopes \
            FROM app_activation_pins p JOIN app_activation_snapshots s ON s.activation_id = p.activation_id \
            JOIN app_team_grants g ON g.app_id = p.app_id AND g.team_id = p.team_id \
            JOIN app_member_bindings b ON b.app_id = p.app_id AND b.team_id = p.team_id AND b.actor_id = p.actor_id \
            JOIN registered_apps a ON a.id = p.app_id \
            WHERE p.activation_id = ? AND p.app_id = ? AND p.team_id = ? AND p.actor_id = ? \
            AND a.revoked_at IS NULL AND g.revoked_at IS NULL AND b.revoked_at IS NULL")
            .bind(&expected.activation_id).bind(app_id).bind(&expected.team_id).bind(&expected.actor_id)
            .fetch_optional(&mut *tx).await?.ok_or(AppStoreError::Revoked)?;
        let pin = parse_pin(&row)?;
        let granted: BTreeSet<String> =
            serde_json::from_str(row.try_get("current_granted_scopes")?)?;
        let bound: BTreeSet<String> = serde_json::from_str(row.try_get("current_bound_scopes")?)?;
        anyhow::ensure!(
            pin.grant_epoch == row.try_get::<i64, _>("current_grant_epoch")?
                && pin.binding_epoch == row.try_get::<i64, _>("current_binding_epoch")?
                && pin.scopes.is_subset(&granted)
                && pin.scopes.is_subset(&bound),
            AppStoreError::Revoked
        );
        tx.commit().await?;
        Ok(pin)
    }
}

async fn pins_tx(
    tx: &mut Transaction<'_, Sqlite>,
    team_id: &str,
    activation_id: &str,
) -> anyhow::Result<Vec<AppActivationPin>> {
    let rows = sqlx::query("SELECT p.*, s.pinned_generation FROM app_activation_pins p \
        JOIN app_activation_snapshots s ON s.activation_id = p.activation_id WHERE p.team_id = ? AND p.activation_id = ? ORDER BY p.app_id LIMIT 16")
        .bind(team_id).bind(activation_id).fetch_all(&mut **tx).await?;
    rows.iter().map(parse_pin).collect()
}

fn parse_pin(row: &SqliteRow) -> anyhow::Result<AppActivationPin> {
    Ok(AppActivationPin {
        activation_id: row.try_get("activation_id")?,
        app_id: row.try_get("app_id")?,
        team_id: row.try_get("team_id")?,
        actor_id: row.try_get("actor_id")?,
        pinned_generation: row.try_get("pinned_generation")?,
        version: row.try_get("version")?,
        scopes: serde_json::from_str(row.try_get("scopes_json")?)?,
        grant_revision: row.try_get("grant_revision")?,
        binding_revision: row.try_get("binding_revision")?,
        grant_epoch: row.try_get("grant_epoch")?,
        binding_epoch: row.try_get("binding_epoch")?,
    })
}
