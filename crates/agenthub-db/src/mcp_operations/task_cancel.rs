use agenthub_agent_domain::{
    loop_runtime::LoopReservation,
    mcp_operations::{
        McpAmbiguityReason, McpCompletion, McpDeferralKind, McpFailureKind, McpTaskAuthority,
        McpTaskCancellationInput, McpTaskCancellationRecord,
    },
};
use sqlx::{Connection, Row, Sqlite, Transaction};
use uuid::Uuid;

use super::{McpJournalError, McpOperationStore, task::settle_task_tx};

/// A single cancellation intent; losing its acknowledgment does not permit another send.
pub struct McpTaskCancellationPermit {
    id: String,
    operation_id: String,
    attempt_number: u32,
}

impl McpTaskCancellationPermit {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn attempt_number(&self) -> u32 {
        self.attempt_number
    }
}

impl McpOperationStore {
    pub async fn begin_task_cancellation(
        &self,
        executor: &LoopReservation,
        authority: &McpTaskAuthority,
        input: &McpTaskCancellationInput,
        now: i64,
    ) -> anyhow::Result<McpTaskCancellationPermit> {
        anyhow::ensure!(now >= 0, "invalid MCP journal timestamp");
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        let operation = self
            .resolve_task_tx(&mut tx, executor, authority, &input.receipt, now)
            .await?;
        anyhow::ensure!(
            matches!(
                operation.completion,
                Some(McpCompletion::Deferred {
                    reason: McpDeferralKind::TaskAccepted,
                    ..
                })
            ),
            McpJournalError::AlreadyCompleted
        );
        let id = Uuid::new_v4().to_string();
        let inserted = sqlx::query("INSERT INTO mcp_operation_task_cancellations(operation_id, attempt_number, permit_id, \
            request_key, request_digest, activation_id, daemon_node_id, daemon_generation, daemon_owner_id, sent_at) \
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(operation_id, attempt_number) DO NOTHING")
            .bind(&operation.id).bind(operation.attempt_count).bind(&id).bind(input.request_key.as_str())
            .bind(input.request_digest.as_str()).bind(&executor.activation_id).bind(&self.daemon.node_id)
            .bind(self.daemon.generation).bind(&self.daemon.owner_id).bind(now).execute(&mut *tx).await?;
        anyhow::ensure!(inserted.rows_affected() == 1, McpJournalError::UnsafeReplay);
        tx.commit().await?;
        Ok(McpTaskCancellationPermit {
            id,
            operation_id: operation.id,
            attempt_number: operation.attempt_count,
        })
    }

    pub async fn complete_task_cancellation(
        &self,
        permit: &McpTaskCancellationPermit,
        completion: &McpCompletion,
        outcome: Option<&McpCompletion>,
        now: i64,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(now >= 0, "invalid MCP journal timestamp");
        anyhow::ensure!(
            outcome.is_none_or(|outcome| matches!(
                outcome,
                McpCompletion::Failed {
                    reason: McpFailureKind::TaskCancelled,
                    ..
                }
            )) && (outcome.is_none() || matches!(completion, McpCompletion::Succeeded { .. })),
            McpJournalError::ContinuationRequired
        );
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        let row = sqlx::query("SELECT * FROM mcp_operation_task_cancellations WHERE permit_id = ? AND operation_id = ? AND attempt_number = ?")
            .bind(&permit.id).bind(&permit.operation_id).bind(permit.attempt_number).fetch_optional(&mut *tx).await?
            .ok_or(McpJournalError::StaleAttempt)?;
        let previous: Option<McpCompletion> = row
            .try_get::<Option<&str>, _>("completion_json")?
            .map(serde_json::from_str)
            .transpose()?;
        let previous_outcome: Option<McpCompletion> = row
            .try_get::<Option<&str>, _>("outcome_json")?
            .map(serde_json::from_str)
            .transpose()?;
        if previous.as_ref() == Some(completion) && previous_outcome.as_ref() == outcome {
            tx.commit().await?;
            return Ok(());
        }
        anyhow::ensure!(
            previous.is_none() || matches!(previous, Some(McpCompletion::OutcomeUnknown { .. })),
            McpJournalError::StaleAttempt
        );
        let completed_at = now.max(row.try_get("sent_at")?);
        sqlx::query("UPDATE mcp_operation_task_cancellations SET completion_json = ?, outcome_json = ?, completed_at = ? WHERE permit_id = ?")
            .bind(serde_json::to_string(completion)?).bind(outcome.map(serde_json::to_string).transpose()?)
            .bind(completed_at).bind(&permit.id).execute(&mut *tx).await?;
        if let Some(outcome) = outcome {
            settle_task_tx(
                &mut tx,
                &permit.operation_id,
                permit.attempt_number,
                row.try_get("activation_id")?,
                outcome,
                completed_at,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn task_cancellation(
        &self,
        team_id: &str,
        actor_id: &str,
        operation_id: &str,
        attempt_number: u32,
    ) -> anyhow::Result<Option<McpTaskCancellationRecord>> {
        let row = sqlx::query("SELECT c.* FROM mcp_operation_task_cancellations c JOIN mcp_operations o ON o.id = c.operation_id \
            WHERE o.team_id = ? AND o.actor_id = ? AND o.id = ? AND c.attempt_number = ?")
            .bind(team_id).bind(actor_id).bind(operation_id).bind(attempt_number).fetch_optional(&self.pool).await?;
        row.map(|row| {
            Ok(McpTaskCancellationRecord {
                operation_id: row.try_get("operation_id")?,
                attempt_number: row.try_get("attempt_number")?,
                activation_id: row.try_get("activation_id")?,
                completion: row
                    .try_get::<Option<&str>, _>("completion_json")?
                    .map(serde_json::from_str)
                    .transpose()?,
                outcome: row
                    .try_get::<Option<&str>, _>("outcome_json")?
                    .map(serde_json::from_str)
                    .transpose()?,
            })
        })
        .transpose()
    }

    pub(super) async fn recover_task_cancellations_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        limit: u32,
        now: i64,
    ) -> anyhow::Result<u64> {
        let completion = McpCompletion::OutcomeUnknown {
            reason: McpAmbiguityReason::DaemonRestart,
        };
        Ok(sqlx::query("UPDATE mcp_operation_task_cancellations SET completion_json = ?, completed_at = MAX(sent_at, ?) \
            WHERE permit_id IN (SELECT permit_id FROM mcp_operation_task_cancellations WHERE daemon_node_id = ? AND completed_at IS NULL \
                AND (daemon_generation != ? OR daemon_owner_id != ?) ORDER BY sent_at, permit_id LIMIT ?)")
            .bind(serde_json::to_string(&completion)?).bind(now).bind(&self.daemon.node_id)
            .bind(self.daemon.generation).bind(&self.daemon.owner_id).bind(limit).execute(&mut **tx).await?.rows_affected())
    }
}
