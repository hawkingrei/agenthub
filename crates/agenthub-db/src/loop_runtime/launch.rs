use agenthub_agent_domain::loop_runtime::{LoopLaunchSnapshot, LoopReservation};
use sqlx::{Row, Sqlite, Transaction};

use super::{LoopStore, LoopStoreError, reservation::require_live_reservation};

impl LoopStore {
    /// Validate the signed execution identity at request admission. The local runtime also
    /// holds an operation guard until the request ends, preventing cleanup from releasing it.
    pub async fn verify_executor_live(
        &self,
        expected: &LoopReservation,
        now: i64,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        Self::verify_executor_live_tx(&mut tx, expected, now).await?;
        tx.commit().await?;
        Ok(())
    }

    pub(crate) async fn verify_executor_live_tx(
        tx: &mut Transaction<'_, Sqlite>,
        expected: &LoopReservation,
        now: i64,
    ) -> anyhow::Result<()> {
        let current = require_live_reservation(tx, expected, now).await?;
        super::policy::require_member(tx, &current.team_id, &current.actor_id).await?;
        let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loop_activations a JOIN loop_mailbox_partitions p ON p.run_id = a.mailbox_run_id AND p.team_id = a.team_id WHERE a.id = ? AND p.active = 1 AND a.state = 'running')")
            .bind(&current.activation_id).fetch_one(&mut **tx).await?;
        anyhow::ensure!(active, LoopStoreError::InvalidState);
        Ok(())
    }

    pub async fn bind_mailbox(
        &self,
        expected: &LoopReservation,
        run_id: &str,
        now: i64,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = require_live_reservation(&mut tx, expected, now).await?;
        let id = current
            .activation_id
            .as_deref()
            .ok_or(LoopStoreError::InvalidState)?;
        let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loop_mailbox_partitions p \
            JOIN team_runs r ON r.id = p.run_id AND r.team_id = p.team_id WHERE p.run_id = ? AND p.team_id = ? AND p.active = 1)")
            .bind(run_id).bind(&current.team_id).fetch_one(&mut *tx).await?;
        anyhow::ensure!(valid, LoopStoreError::ScopeMismatch);
        let changed = sqlx::query("UPDATE loop_policies SET mailbox_run_id = ? WHERE actor_id = ? AND (mailbox_run_id IS NULL OR mailbox_run_id = ?)")
            .bind(run_id).bind(&current.actor_id).bind(run_id).execute(&mut *tx).await?.rows_affected();
        anyhow::ensure!(changed == 1, LoopStoreError::InvalidState);
        let changed = sqlx::query("UPDATE loop_activations SET mailbox_run_id = ?, updated_at = ? WHERE id = ? AND state = 'starting' AND (mailbox_run_id IS NULL OR mailbox_run_id = ?)")
            .bind(run_id).bind(now).bind(id).bind(run_id).execute(&mut *tx).await?.rows_affected();
        anyhow::ensure!(changed == 1, LoopStoreError::InvalidState);
        tx.commit().await?;
        Ok(())
    }

    pub async fn record_launch(
        &self,
        expected: &LoopReservation,
        snapshot: &LoopLaunchSnapshot,
        now: i64,
    ) -> anyhow::Result<()> {
        snapshot.validate()?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = require_live_reservation(&mut tx, expected, now).await?;
        let id = current
            .activation_id
            .as_deref()
            .ok_or(LoopStoreError::InvalidState)?;
        let row = sqlx::query(
            "SELECT state, mailbox_run_id, launch_json FROM loop_activations WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(
            row.try_get::<&str, _>("state")? == "starting"
                && row.try_get::<Option<&str>, _>("mailbox_run_id")?.is_some(),
            LoopStoreError::InvalidState
        );
        if let Some(existing) = row.try_get::<Option<&str>, _>("launch_json")? {
            anyhow::ensure!(
                serde_json::from_str::<LoopLaunchSnapshot>(existing)? == *snapshot,
                LoopStoreError::IdempotencyConflict
            );
        } else {
            sqlx::query("UPDATE loop_activations SET launch_json = ?, updated_at = ? WHERE id = ?")
                .bind(serde_json::to_string(snapshot)?)
                .bind(now)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO loop_activation_events(activation_id, kind, generation, created_at) VALUES (?, 'launch_resolved', ?, ?)")
                .bind(id).bind(current.generation).bind(now).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
