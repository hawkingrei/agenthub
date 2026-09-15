use agenthub_agent_domain::loop_runtime::{
    LoopFinishReceipt, LoopOutcome, LoopReservation, LoopSourceReferences, LoopTriggerInput,
    LoopTriggerKind,
};
use sqlx::Row;

use super::{
    LoopStore, LoopStoreError, policy::require_member, reservation::require_live_reservation,
};

impl LoopStore {
    /// Resolve exclusively from an authenticated actor/run/activation fence. Historical receipts
    /// remain available after cleanup; an old fence never resolves the actor's newer execution.
    pub async fn executor_reservation(
        &self,
        actor_id: &str,
        run_id: &str,
        activation_id: &str,
        generation: i64,
    ) -> anyhow::Result<LoopReservation> {
        let row = sqlx::query("SELECT a.*, COALESCE(r.owner_id, f.owner_id) AS executor_owner, \
            COALESCE(r.lease_expires_at, 0) AS lease_expires_at, COALESCE(r.lease_seconds, 60) AS lease_seconds, \
            COALESCE(r.renewal_seconds, 15) AS renewal_seconds \
            FROM loop_activations a \
            JOIN loop_mailbox_partitions p ON p.run_id = a.mailbox_run_id AND p.team_id = a.team_id \
            JOIN team_runs tr ON tr.id = p.run_id AND tr.team_id = a.team_id \
            LEFT JOIN loop_execution_reservations r ON r.activation_id = a.id AND r.generation = a.generation \
            LEFT JOIN loop_finish_receipts f ON f.activation_id = a.id AND f.generation = a.generation \
            WHERE a.actor_id = ? AND a.mailbox_run_id = ? AND a.id = ? AND a.generation = ? AND a.session_id IS NOT NULL")
            .bind(actor_id).bind(run_id).bind(activation_id).bind(generation).fetch_optional(&self.pool).await?
            .ok_or(LoopStoreError::ScopeMismatch)?;
        Ok(LoopReservation {
            actor_id: actor_id.into(),
            team_id: row.try_get("team_id")?,
            activation_id: Some(activation_id.into()),
            generation,
            owner_id: row
                .try_get::<Option<String>, _>("executor_owner")?
                .ok_or(LoopStoreError::StaleLease)?,
            lease_expires_at: row.try_get("lease_expires_at")?,
            lease_seconds: row.try_get("lease_seconds")?,
            renewal_seconds: row.try_get("renewal_seconds")?,
            session_id: row.try_get("session_id")?,
            created_at: row.try_get("created_at")?,
        })
    }

    /// Persist an outcome and its self-continuation in one control-store transaction.
    /// The caller authenticates the executor; this method checks its durable fence.
    pub async fn finish(
        &self,
        reservation: &LoopReservation,
        outcome: &LoopOutcome,
        now: i64,
    ) -> anyhow::Result<LoopFinishReceipt> {
        outcome.validate()?;
        anyhow::ensure!(now >= 0, "invalid finish time");
        let activation_id = reservation
            .activation_id
            .as_deref()
            .ok_or(LoopStoreError::InvalidState)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let row = sqlx::query(
            "SELECT a.*, f.owner_id AS finish_owner, f.receipt_json FROM loop_activations a \
             LEFT JOIN loop_finish_receipts f ON f.activation_id = a.id \
             WHERE a.id = ? AND a.actor_id = ? AND a.team_id = ? AND a.generation = ?",
        )
        .bind(activation_id)
        .bind(&reservation.actor_id)
        .bind(&reservation.team_id)
        .bind(reservation.generation)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(LoopStoreError::StaleLease)?;
        if let Some(recorded) = row.try_get::<Option<&str>, _>("outcome_json")? {
            anyhow::ensure!(
                row.try_get::<Option<&str>, _>("finish_owner")? == Some(&reservation.owner_id),
                LoopStoreError::StaleLease
            );
            anyhow::ensure!(
                serde_json::from_str::<LoopOutcome>(recorded)? == *outcome,
                LoopStoreError::IdempotencyConflict
            );
            let receipt = serde_json::from_str(row.try_get("receipt_json")?)?;
            tx.commit().await?;
            return Ok(receipt);
        }
        let current = require_live_reservation(&mut tx, reservation, now).await?;
        require_member(&mut tx, &current.team_id, &current.actor_id).await?;
        anyhow::ensure!(
            row.try_get::<&str, _>("state")? == "running",
            LoopStoreError::InvalidState
        );

        let mut recorded_progress = false;
        if let Some(note_id) = outcome.task_note_id {
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM team_conversation_messages m \
                 JOIN team_tasks t ON t.id = m.task_id WHERE m.id = ? AND t.team_id = ? \
                 AND m.from_actor_id = ? AND m.route = 'task_note' AND m.created_at >= ? AND m.created_at <= ?)",
            )
            .bind(note_id)
            .bind(&current.team_id)
            .bind(&current.actor_id)
            .bind(current.created_at)
            .bind(now)
            .fetch_one(&mut *tx)
            .await?;
            anyhow::ensure!(valid, LoopStoreError::ScopeMismatch);
            recorded_progress = sqlx::query(
                "INSERT OR IGNORE INTO loop_progress_receipts(task_note_id, activation_id) VALUES (?, ?)",
            ).bind(note_id).bind(activation_id).execute(&mut *tx).await?.rows_affected() == 1;
        }

        let continuation = if let Some(next) = &outcome.continuation {
            if let Some(task_id) = &next.task_id {
                let actionable: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM team_tasks WHERE id = ? AND team_id = ? AND status NOT IN ('completed', 'canceled'))",
                ).bind(task_id).bind(&current.team_id).fetch_one(&mut *tx).await?;
                anyhow::ensure!(actionable, LoopStoreError::InvalidState);
            }
            let input = LoopTriggerInput {
                actor_id: current.actor_id.clone(),
                team_id: current.team_id.clone(),
                kind: LoopTriggerKind::Continuation,
                source_key: format!("finish:{activation_id}:{}", current.generation),
                due_at: Some(next.due_at),
                references: LoopSourceReferences {
                    task_id: next.task_id.clone(),
                    scheduling_actor_id: Some(current.actor_id.clone()),
                    scheduling_activation_id: Some(activation_id.into()),
                    ..LoopSourceReferences::default()
                },
            };
            Some(Self::accept_in_transaction(&mut tx, &input, now).await?)
        } else {
            None
        };
        let receipt = LoopFinishReceipt {
            activation_id: activation_id.into(),
            generation: current.generation,
            continuation,
        };
        sqlx::query("UPDATE loop_activations SET state = 'finalizing', outcome_json = ?, updated_at = ? WHERE id = ?")
            .bind(serde_json::to_string(outcome)?).bind(now).bind(activation_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO loop_finish_receipts(activation_id, generation, owner_id, receipt_json) VALUES (?, ?, ?, ?)")
            .bind(activation_id).bind(current.generation).bind(&current.owner_id)
            .bind(serde_json::to_string(&receipt)?).execute(&mut *tx).await?;
        sqlx::query("UPDATE loop_policies SET no_progress_count = CASE WHEN ? THEN 0 ELSE MIN(no_progress_count + 1, 86400) END, updated_at = ? WHERE actor_id = ?")
            .bind(recorded_progress).bind(now).bind(&current.actor_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO loop_activation_events(activation_id, kind, generation, created_at) VALUES (?, 'outcome_recorded', ?, ?)")
            .bind(activation_id).bind(current.generation).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(receipt)
    }
}
