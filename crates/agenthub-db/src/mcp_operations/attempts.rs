use agenthub_agent_domain::{
    loop_runtime::LoopReservation,
    mcp_operations::{McpAmbiguityReason, McpCompletion, McpOperationStatus},
};
use sqlx::{Connection, Row, Sqlite, Transaction};
use uuid::Uuid;

use crate::loop_runtime::LoopStore;

use super::{
    McpJournalError, McpOperationStore, McpSendPermit, parse_operation,
    prepare::reject_prior_effects, record_event,
};

impl McpOperationStore {
    /// Commit sent before writing any request bytes. Only the returned permit authorizes one send.
    /// expected_attempt_count prevents two concurrent callers from starting the same retry.
    pub async fn begin_send(
        &self,
        executor: &LoopReservation,
        operation_id: &str,
        expected_attempt_count: u32,
        now: i64,
    ) -> anyhow::Result<McpSendPermit> {
        let mut permits = self
            .begin_send_batch(executor, &[(operation_id, expected_attempt_count)], now)
            .await?;
        Ok(permits
            .pop()
            .expect("one operation returns one send permit"))
    }

    /// All send transitions commit together before a single batch POST. A rejected member rolls
    /// back every transition, so no earlier member is left falsely marked as sent.
    pub async fn begin_send_batch(
        &self,
        executor: &LoopReservation,
        operations: &[(&str, u32)],
        now: i64,
    ) -> anyhow::Result<Vec<McpSendPermit>> {
        anyhow::ensure!(now >= 0, "invalid MCP journal timestamp");
        anyhow::ensure!(
            (1..=256).contains(&operations.len()),
            "invalid MCP send batch size"
        );
        let mut unique = std::collections::HashSet::new();
        anyhow::ensure!(
            operations.iter().all(|(id, _)| unique.insert(*id)),
            McpJournalError::IdentityConflict
        );
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        self.require_current_daemon(&mut tx).await?;
        LoopStore::verify_executor_live_tx(&mut tx, executor, now).await?;
        let mut permits = Vec::with_capacity(operations.len());
        for (operation_id, expected_attempt_count) in operations {
            permits.push(
                self.begin_send_tx(
                    &mut tx,
                    executor,
                    operation_id,
                    *expected_attempt_count,
                    now,
                )
                .await?,
            );
        }
        tx.commit().await?;
        Ok(permits)
    }

    async fn begin_send_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        executor: &LoopReservation,
        operation_id: &str,
        expected_attempt_count: u32,
        now: i64,
    ) -> anyhow::Result<McpSendPermit> {
        let row = sqlx::query(
            "SELECT * FROM mcp_operations WHERE id = ? AND team_id = ? AND actor_id = ?",
        )
        .bind(operation_id)
        .bind(&executor.team_id)
        .bind(&executor.actor_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(McpJournalError::ScopeMismatch)?;
        let operation = parse_operation(&row)?;
        anyhow::ensure!(
            operation.attempt_count == expected_attempt_count,
            McpJournalError::StaleAttempt
        );
        match operation.status {
            McpOperationStatus::Prepared => {}
            McpOperationStatus::Sent => return Err(McpJournalError::InFlight.into()),
            McpOperationStatus::Succeeded => return Err(McpJournalError::AlreadyCompleted.into()),
            McpOperationStatus::Failed | McpOperationStatus::OutcomeUnknown => {
                anyhow::ensure!(
                    !matches!(operation.completion, Some(McpCompletion::Deferred { .. })),
                    McpJournalError::ContinuationRequired
                );
                anyhow::ensure!(
                    operation.intent.replay_safety.permits_retry(),
                    McpJournalError::UnsafeReplay
                );
            }
        }
        reject_prior_effects(tx, &executor.team_id, &operation.intent, Some(operation_id)).await?;
        let number = operation
            .attempt_count
            .checked_add(1)
            .ok_or(McpJournalError::StaleAttempt)?;
        let permit_id = Uuid::new_v4().to_string();
        let activation = executor
            .activation_id
            .as_deref()
            .ok_or(McpJournalError::ScopeMismatch)?;
        let sent_at = now.max(operation.updated_at);
        sqlx::query("INSERT INTO mcp_operation_attempts(operation_id, number, permit_id, activation_id, generation, \
            daemon_node_id, daemon_generation, daemon_owner_id, status, sent_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'sent', ?)")
            .bind(operation_id).bind(number).bind(&permit_id).bind(activation).bind(executor.generation)
            .bind(&self.daemon.node_id).bind(self.daemon.generation).bind(&self.daemon.owner_id)
            .bind(sent_at).execute(&mut **tx).await?;
        sqlx::query("UPDATE mcp_operations SET status = 'sent', attempt_count = ?, completion_json = NULL, updated_at = ? WHERE id = ?")
            .bind(number).bind(sent_at).bind(operation_id).execute(&mut **tx).await?;
        record_event(
            tx,
            operation_id,
            number,
            activation,
            McpOperationStatus::Sent,
            None,
            sent_at,
        )
        .await?;
        Ok(McpSendPermit {
            operation_id: operation_id.to_owned(),
            attempt_number: number,
            permit_id,
        })
    }

    /// Recording an observed result does not authorize further execution. The original send permit
    /// remains valid after activation cancellation/expiry, but cannot finish a replacement attempt.
    pub async fn complete(
        &self,
        permit: &McpSendPermit,
        completion: &McpCompletion,
        now: i64,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(now >= 0, "invalid MCP journal timestamp");
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        let row = sqlx::query(
            "SELECT a.*, o.updated_at FROM mcp_operation_attempts a \
            JOIN mcp_operations o ON o.id = a.operation_id AND o.attempt_count = a.number \
            WHERE a.operation_id = ? AND a.number = ? AND a.permit_id = ?",
        )
        .bind(&permit.operation_id)
        .bind(permit.attempt_number)
        .bind(&permit.permit_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(McpJournalError::StaleAttempt)?;
        let status: McpOperationStatus = row.try_get::<&str, _>("status")?.parse()?;
        let previous: Option<McpCompletion> = row
            .try_get::<Option<&str>, _>("completion_json")?
            .map(serde_json::from_str)
            .transpose()?;
        if previous.as_ref() == Some(completion)
            || (matches!(previous, Some(McpCompletion::OutcomeUnknown { .. }))
                && matches!(completion, McpCompletion::OutcomeUnknown { .. }))
            || (matches!(previous, Some(McpCompletion::Deferred { .. }))
                && matches!(completion, McpCompletion::OutcomeUnknown { .. }))
        {
            tx.commit().await?;
            return Ok(());
        }
        anyhow::ensure!(
            matches!(
                status,
                McpOperationStatus::Sent | McpOperationStatus::OutcomeUnknown
            ),
            McpJournalError::StaleAttempt
        );
        anyhow::ensure!(
            !matches!(previous, Some(McpCompletion::Deferred { .. }))
                || !matches!(completion, McpCompletion::Deferred { .. }),
            McpJournalError::StaleAttempt
        );
        // A late factual result may resolve an unknown attempt, while the event journal retains
        // the earlier uncertainty. This is not an unknown -> sent replay.
        let activation: &str = row.try_get("activation_id")?;
        let completed_at = now.max(row.try_get("updated_at")?);
        complete_attempt(
            &mut tx,
            &permit.operation_id,
            permit.attempt_number,
            activation,
            completion,
            completed_at,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Call after claiming this daemon generation, before accepting new proxy work. Migration and
    /// ordinary database opens do not reconcile sends owned by a still-current daemon.
    pub async fn recover_interrupted(&self, limit: u32, now: i64) -> anyhow::Result<u64> {
        anyhow::ensure!(now >= 0, "invalid MCP journal timestamp");
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        self.require_current_daemon(&mut tx).await?;
        let rows = sqlx::query("SELECT a.operation_id, a.number, a.activation_id, o.updated_at \
            FROM mcp_operation_attempts a JOIN mcp_operations o ON o.id = a.operation_id AND o.attempt_count = a.number \
            WHERE a.daemon_node_id = ? AND a.status = 'sent' AND (a.daemon_generation != ? OR a.daemon_owner_id != ?) \
            ORDER BY a.sent_at, a.operation_id LIMIT ?")
            .bind(&self.daemon.node_id).bind(self.daemon.generation).bind(&self.daemon.owner_id)
            .bind(limit.clamp(1, 100)).fetch_all(&mut *tx).await?;
        let completion = McpCompletion::OutcomeUnknown {
            reason: McpAmbiguityReason::DaemonRestart,
        };
        for row in &rows {
            complete_attempt(
                &mut tx,
                row.try_get("operation_id")?,
                row.try_get("number")?,
                row.try_get("activation_id")?,
                &completion,
                now.max(row.try_get("updated_at")?),
            )
            .await?;
        }
        tx.commit().await?;
        Ok(rows.len() as u64)
    }
}

async fn complete_attempt(
    tx: &mut Transaction<'_, Sqlite>,
    operation_id: &str,
    number: u32,
    activation_id: &str,
    completion: &McpCompletion,
    now: i64,
) -> anyhow::Result<()> {
    let json = serde_json::to_string(completion)?;
    sqlx::query("UPDATE mcp_operation_attempts SET status = ?, completion_json = ?, completed_at = ? WHERE operation_id = ? AND number = ?")
        .bind(completion.status().as_str()).bind(&json).bind(now).bind(operation_id).bind(number)
        .execute(&mut **tx).await?;
    sqlx::query("UPDATE mcp_operations SET status = ?, completion_json = ?, updated_at = ? WHERE id = ? AND attempt_count = ?")
        .bind(completion.status().as_str()).bind(&json).bind(now).bind(operation_id).bind(number)
        .execute(&mut **tx).await?;
    record_event(
        tx,
        operation_id,
        number,
        activation_id,
        completion.status(),
        Some(completion),
        now,
    )
    .await?;
    Ok(())
}
