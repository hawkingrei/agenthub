use agenthub_agent_domain::loop_runtime::{
    LoopActivation, LoopCleanupDisposition, LoopExitReason, LoopReservation,
};
use sqlx::{Row, Sqlite, Transaction};

use super::{
    LoopStore, LoopStoreError,
    policy::parse_policy,
    reservation::{require_live_reservation, require_matching_reservation},
};

pub(super) async fn retire_inactive_task_sources(
    tx: &mut Transaction<'_, Sqlite>,
    activation: &LoopActivation,
    now: i64,
) -> anyhow::Result<bool> {
    sqlx::query("INSERT OR IGNORE INTO loop_revoked_sources(trigger_id, created_at) \
        SELECT s.id, ? FROM loop_trigger_sources s WHERE s.activation_id = ? \
        AND s.source_kind IN ('assignment', 'continuation') AND json_extract(s.input_json, '$.references.task_id') IS NOT NULL \
        AND NOT EXISTS(SELECT 1 FROM team_tasks t WHERE t.id = json_extract(s.input_json, '$.references.task_id') \
        AND t.team_id = s.team_id AND t.status NOT IN ('completed', 'canceled') \
        AND (s.source_kind != 'assignment' OR t.assigned_member_id = s.actor_id))")
        .bind(now).bind(&activation.id).execute(&mut **tx).await?;
    let actionable: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM loop_trigger_sources s WHERE s.activation_id = ? \
        AND NOT EXISTS(SELECT 1 FROM loop_revoked_sources r WHERE r.trigger_id = s.id))",
    )
    .bind(&activation.id)
    .fetch_one(&mut **tx)
    .await?;
    if !actionable {
        sqlx::query("UPDATE loop_activations SET state = 'canceled', updated_at = ?, finished_at = ? WHERE id = ?")
            .bind(now).bind(now).bind(&activation.id).execute(&mut **tx).await?;
        sqlx::query("INSERT INTO loop_activation_events(activation_id, kind, generation, created_at) VALUES (?, 'canceled', ?, ?)")
            .bind(&activation.id).bind(activation.generation).bind(now).execute(&mut **tx).await?;
    }
    Ok(!actionable)
}

impl LoopStore {
    /// Fence further executor writes immediately; process cleanup remains the caller's duty.
    #[tracing::instrument(name = "loop.revoke_execution", skip_all, fields(
        team_id = %expected.team_id, actor_id = %expected.actor_id,
        activation_id = expected.activation_id.as_deref(), generation = expected.generation,
    ))]
    pub async fn revoke_execution(
        &self,
        expected: &LoopReservation,
        now: i64,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = require_matching_reservation(&mut tx, expected).await?;
        sqlx::query("UPDATE loop_execution_reservations SET lease_expires_at = MIN(lease_expires_at, ?) WHERE actor_id = ?")
            .bind(now).bind(&current.actor_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    /// Bind a runtime session after its row is inserted, before exposing actor tools.
    #[tracing::instrument(name = "loop.bind_session", skip_all, fields(
        team_id = %expected.team_id, actor_id = %expected.actor_id,
        activation_id = expected.activation_id.as_deref(), generation = expected.generation,
        session_id = %session_id,
    ))]
    pub async fn bind_session(
        &self,
        expected: &LoopReservation,
        session_id: &str,
        now: i64,
    ) -> anyhow::Result<LoopReservation> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let mut current = require_live_reservation(&mut tx, expected, now).await?;
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE id = ? AND agent_id = ?)",
        )
        .bind(session_id)
        .bind(&current.actor_id)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(valid, LoopStoreError::ScopeMismatch);
        anyhow::ensure!(
            current
                .session_id
                .as_deref()
                .is_none_or(|id| id == session_id),
            LoopStoreError::InvalidState
        );
        sqlx::query("UPDATE loop_execution_reservations SET session_id = ? WHERE actor_id = ?")
            .bind(session_id)
            .bind(&current.actor_id)
            .execute(&mut *tx)
            .await?;
        if let Some(id) = &current.activation_id {
            let changed = sqlx::query("UPDATE loop_activations SET session_id = ?, updated_at = ? WHERE id = ? AND state = 'starting'")
                .bind(session_id).bind(now).bind(id).execute(&mut *tx).await?.rows_affected();
            anyhow::ensure!(changed == 1, LoopStoreError::InvalidState);
        }
        current.session_id = Some(session_id.into());
        tx.commit().await?;
        Ok(current)
    }

    #[tracing::instrument(name = "loop.running", skip_all, fields(
        team_id = %expected.team_id, actor_id = %expected.actor_id,
        activation_id = expected.activation_id.as_deref(), generation = expected.generation,
        session_id = expected.session_id.as_deref(),
    ))]
    pub async fn mark_running(&self, expected: &LoopReservation, now: i64) -> anyhow::Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = require_live_reservation(&mut tx, expected, now).await?;
        anyhow::ensure!(current.session_id.is_some(), LoopStoreError::InvalidState);
        let id = current
            .activation_id
            .as_deref()
            .ok_or(LoopStoreError::InvalidState)?;
        let changed = sqlx::query("UPDATE loop_activations SET state = 'running', updated_at = ? WHERE id = ? AND state = 'starting'")
            .bind(now).bind(id).execute(&mut *tx).await?.rows_affected();
        anyhow::ensure!(changed == 1, LoopStoreError::InvalidState);
        sqlx::query("INSERT INTO loop_activation_events(activation_id, kind, generation, created_at) VALUES (?, 'running', ?, ?)")
            .bind(id).bind(current.generation).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// This is a trusted supervisor boundary, never an agent-supplied cleanup assertion.
    /// Lease expiry is allowed here, but the owner and generation must still match.
    #[tracing::instrument(name = "loop.cleanup", skip_all, fields(
        team_id = %expected.team_id, actor_id = %expected.actor_id,
        activation_id = expected.activation_id.as_deref(), generation = expected.generation,
        session_id = expected.session_id.as_deref(),
    ))]
    pub async fn cleanup_verified(
        &self,
        expected: &LoopReservation,
        disposition: LoopCleanupDisposition,
        now: i64,
    ) -> anyhow::Result<()> {
        self.cleanup(expected, disposition, now, false).await?;
        Ok(())
    }

    /// Atomically prove that no spawn was authorized and retire the expired reservation.
    pub async fn cleanup_unstarted(
        &self,
        expected: &LoopReservation,
        now: i64,
    ) -> anyhow::Result<bool> {
        self.cleanup(expected, LoopCleanupDisposition::Exited, now, true)
            .await
    }

    async fn cleanup(
        &self,
        expected: &LoopReservation,
        disposition: LoopCleanupDisposition,
        now: i64,
        require_unstarted: bool,
    ) -> anyhow::Result<bool> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = require_matching_reservation(&mut tx, expected).await?;
        if require_unstarted {
            let unstarted: bool = sqlx::query_scalar("SELECT executor_state = 'unstarted' AND session_id IS NULL AND lease_expires_at <= ? FROM loop_execution_reservations WHERE actor_id = ?")
                .bind(now).bind(&current.actor_id).fetch_one(&mut *tx).await?;
            if !unstarted {
                return Ok(false);
            }
        }
        if let Some(id) = &current.activation_id {
            let row = sqlx::query(
                "SELECT state, attempt_count, outcome_json FROM loop_activations WHERE id = ?",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            let state: &str = row.try_get("state")?;
            let finished = row.try_get::<Option<&str>, _>("outcome_json")?.is_some();
            let policy = parse_policy(
                &sqlx::query("SELECT * FROM loop_policies WHERE actor_id = ?")
                    .bind(&current.actor_id)
                    .fetch_one(&mut *tx)
                    .await?,
            )?;
            let attempts: u32 = row.try_get("attempt_count")?;
            let retry = disposition == LoopCleanupDisposition::StartupFailed
                && state == "starting"
                && attempts < policy.limits.startup_attempts;
            let next_state = if state == "canceled" {
                "canceled"
            } else if finished {
                "finished"
            } else if retry {
                "pending"
            } else {
                "interrupted"
            };
            let next_due = now
                .checked_add(i64::from(policy.limits.startup_retry_seconds(attempts)))
                .ok_or_else(|| anyhow::anyhow!("retry deadline overflow"))?;
            sqlx::query("UPDATE loop_activations SET state = ?, coalesce_key = NULL, next_admission_at = CASE WHEN ? THEN ? ELSE next_admission_at END, updated_at = ?, finished_at = ? WHERE id = ?")
                .bind(next_state).bind(retry).bind(next_due).bind(now).bind(if retry { None } else { Some(now) })
                .bind(id).execute(&mut *tx).await?;
            if !finished && !retry && state != "canceled" {
                sqlx::query("UPDATE loop_policies SET no_progress_count = MIN(no_progress_count + 1, 86400), updated_at = ? WHERE actor_id = ?")
                    .bind(now).bind(&current.actor_id).execute(&mut *tx).await?;
            }
            let exit_reason = if disposition == LoopCleanupDisposition::StartupFailed {
                LoopExitReason::StartupFailed
            } else if state == "canceled" {
                LoopExitReason::Canceled
            } else if finished {
                LoopExitReason::OutcomeRecorded
            } else {
                LoopExitReason::UnexpectedExit
            };
            sqlx::query("INSERT INTO loop_activation_events(activation_id, kind, generation, created_at, exit_reason_code) VALUES (?, 'cleanup_verified', ?, ?, ?)")
                .bind(id).bind(current.generation).bind(now).bind(exit_reason.as_str()).execute(&mut *tx).await?;
            if !finished && !retry && state != "canceled" && state != "interrupted" {
                sqlx::query("INSERT INTO loop_activation_events(activation_id, kind, generation, created_at) VALUES (?, 'interrupted', ?, ?)")
                    .bind(id).bind(current.generation).bind(now).execute(&mut *tx).await?;
            }
        }
        sqlx::query("DELETE FROM loop_execution_reservations WHERE actor_id = ? AND generation = ? AND owner_id = ?")
            .bind(&current.actor_id).bind(current.generation).bind(&current.owner_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Restart has no process handle proof. Retain expired reservations until explicit cleanup.
    pub async fn interrupt_expired(&self, now: i64) -> anyhow::Result<u64> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let rows = sqlx::query("UPDATE loop_activations SET state = 'interrupted', updated_at = ? WHERE state IN ('starting', 'running') AND id IN (SELECT activation_id FROM loop_execution_reservations WHERE lease_expires_at <= ?) RETURNING id, generation")
            .bind(now).bind(now).fetch_all(&mut *tx).await?;
        for row in &rows {
            sqlx::query("INSERT INTO loop_activation_events(activation_id, kind, generation, created_at) VALUES (?, 'interrupted', ?, ?)")
                .bind(row.try_get::<&str, _>("id")?).bind(row.try_get::<i64, _>("generation")?).bind(now).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(rows.len() as u64)
    }

    /// Cancel pending intent while retaining any active writer's reservation.
    #[tracing::instrument(name = "loop.cancel", skip_all, fields(team_id = %team_id, activation_id = %activation_id))]
    pub async fn cancel(&self, team_id: &str, activation_id: &str, now: i64) -> anyhow::Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM loop_activations WHERE id = ? AND team_id = ?)",
        )
        .bind(activation_id)
        .bind(team_id)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(exists, LoopStoreError::ScopeMismatch);
        super::scheduling_revocation::revoke_origin_registrations(
            &mut tx,
            team_id,
            activation_id,
            now,
        )
        .await?;
        // Retire only this continuation's source; a coalesced independent wake must survive.
        sqlx::query("INSERT OR IGNORE INTO loop_revoked_sources(trigger_id, created_at) SELECT id, ? FROM loop_trigger_sources WHERE team_id = ? AND source_kind = 'continuation' AND json_extract(input_json, '$.references.scheduling_activation_id') = ?")
            .bind(now).bind(team_id).bind(activation_id).execute(&mut *tx).await?;
        let rows = sqlx::query("UPDATE loop_activations SET state = 'canceled', updated_at = ?, finished_at = ? WHERE team_id = ? AND ((id = ? AND state NOT IN ('finished', 'canceled', 'interrupted')) OR (state = 'pending' AND NOT EXISTS(SELECT 1 FROM loop_trigger_sources s WHERE s.activation_id = loop_activations.id AND NOT EXISTS(SELECT 1 FROM loop_revoked_sources r WHERE r.trigger_id = s.id)))) RETURNING id, generation")
            .bind(now).bind(now).bind(team_id).bind(activation_id).fetch_all(&mut *tx).await?;
        for row in rows {
            sqlx::query("INSERT INTO loop_activation_events(activation_id, kind, generation, created_at) VALUES (?, 'canceled', ?, ?)")
                .bind(row.try_get::<&str, _>("id")?).bind(row.try_get::<i64, _>("generation")?).bind(now).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
