use agenthub_agent_domain::{
    loop_runtime::LoopReservation,
    mcp_operations::{
        McpCompletion, McpDigest, McpTaskAuthority, McpTaskInputRequest, McpTaskNotificationRecord,
        McpTaskReceipt, McpTaskVersion,
    },
};
use sqlx::{Connection, Row, Sqlite, Transaction};

use super::{McpJournalError, McpOperationStore, task::settle_task_tx};

/// An admitted observation of one immutable task receipt. This grants no outgoing tool action.
/// Keep it private to the daemon stream that passed live executor and binding checks.
pub struct McpTaskNotificationPermit {
    pub(super) operation_id: String,
    pub(super) attempt_number: u32,
    pub(super) activation_id: String,
    pub(super) receipt: McpTaskReceipt,
}

impl McpTaskNotificationPermit {
    pub fn receipt(&self) -> &McpTaskReceipt {
        &self.receipt
    }
}

impl McpOperationStore {
    pub async fn authorize_task_notifications(
        &self,
        executor: &LoopReservation,
        authority: &McpTaskAuthority,
        receipt: &McpTaskReceipt,
        now: i64,
    ) -> anyhow::Result<McpTaskNotificationPermit> {
        anyhow::ensure!(now >= 0, "invalid MCP journal timestamp");
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let operation = self
            .resolve_task_tx(&mut tx, executor, authority, receipt, now)
            .await?;
        let activation_id = executor
            .activation_id
            .clone()
            .ok_or(McpJournalError::ScopeMismatch)?;
        tx.commit().await?;
        Ok(McpTaskNotificationPermit {
            operation_id: operation.id,
            attempt_number: operation.attempt_count,
            activation_id,
            receipt: receipt.clone(),
        })
    }

    /// Persist each distinct observation before delivery. A received fact may land after executor
    /// shutdown; only the original attempt can settle, and a later terminal fact cannot replace it.
    pub async fn record_task_notification(
        &self,
        permit: &McpTaskNotificationPermit,
        response_digest: &McpDigest,
        outcome: Option<&McpCompletion>,
        inputs: Option<&[McpTaskInputRequest]>,
        now: i64,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(now >= 0, "invalid MCP journal timestamp");
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        let valid = Self::record_task_notification_tx(
            &mut tx,
            permit,
            response_digest,
            outcome,
            inputs,
            now,
        )
        .await?;
        tx.commit().await?;
        anyhow::ensure!(valid, McpJournalError::TaskInputConflict);
        Ok(())
    }

    pub(super) async fn record_task_notification_tx(
        tx: &mut Transaction<'_, Sqlite>,
        permit: &McpTaskNotificationPermit,
        response_digest: &McpDigest,
        outcome: Option<&McpCompletion>,
        inputs: Option<&[McpTaskInputRequest]>,
        now: i64,
    ) -> anyhow::Result<bool> {
        anyhow::ensure!(
            outcome.is_none_or(|value| matches!(
                value,
                McpCompletion::Succeeded { .. } | McpCompletion::Failed { .. }
            )),
            McpJournalError::ContinuationRequired
        );
        anyhow::ensure!(
            inputs.is_none()
                || outcome.is_none() && permit.receipt.version == McpTaskVersion::July2026,
            McpJournalError::ContinuationRequired
        );
        let previous: Option<bool> = sqlx::query_scalar(
            "SELECT inputs_valid FROM mcp_operation_task_notifications \
            WHERE operation_id = ? AND attempt_number = ? AND response_digest = ?",
        )
        .bind(&permit.operation_id)
        .bind(permit.attempt_number)
        .bind(response_digest.as_str())
        .fetch_optional(&mut **tx)
        .await?;
        if let Some(valid) = previous {
            return Ok(valid);
        }
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM mcp_operation_task_notifications \
            WHERE operation_id = ? AND attempt_number = ?",
        )
        .bind(&permit.operation_id)
        .bind(permit.attempt_number)
        .fetch_one(&mut **tx)
        .await?;
        anyhow::ensure!(count < 4096, McpJournalError::ContinuationRequired);
        let inputs_valid = if let Some(inputs) = inputs {
            Self::record_task_inputs_tx(tx, &permit.operation_id, permit.attempt_number, inputs)
                .await?
        } else {
            true
        };
        sqlx::query("INSERT INTO mcp_operation_task_notifications(operation_id, attempt_number, activation_id, \
            response_digest, outcome_json, inputs_valid, observed_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&permit.operation_id).bind(permit.attempt_number).bind(&permit.activation_id)
            .bind(response_digest.as_str()).bind(outcome.map(serde_json::to_string).transpose()?)
            .bind(inputs_valid).bind(now).execute(&mut **tx).await?;
        if let Some(outcome) = outcome {
            settle_task_tx(
                tx,
                &permit.operation_id,
                permit.attempt_number,
                &permit.activation_id,
                outcome,
                now,
            )
            .await?;
        }
        Ok(inputs_valid)
    }

    pub async fn task_notifications(
        &self,
        team_id: &str,
        actor_id: &str,
        operation_id: &str,
        after: i64,
        limit: u32,
    ) -> anyhow::Result<Vec<McpTaskNotificationRecord>> {
        let rows = sqlx::query("SELECT n.* FROM mcp_operation_task_notifications n JOIN mcp_operations o ON o.id = n.operation_id \
            WHERE o.team_id = ? AND o.actor_id = ? AND o.id = ? AND n.sequence > ? ORDER BY n.sequence LIMIT ?")
            .bind(team_id).bind(actor_id).bind(operation_id).bind(after.max(0)).bind(limit.clamp(1, 100))
            .fetch_all(&self.pool).await?;
        rows.iter()
            .map(|row| {
                Ok(McpTaskNotificationRecord {
                    sequence: row.try_get("sequence")?,
                    attempt_number: row.try_get("attempt_number")?,
                    activation_id: row.try_get("activation_id")?,
                    response_digest: row.try_get::<String, _>("response_digest")?.try_into()?,
                    outcome: row
                        .try_get::<Option<&str>, _>("outcome_json")?
                        .map(serde_json::from_str)
                        .transpose()?,
                    inputs_valid: row.try_get("inputs_valid")?,
                })
            })
            .collect()
    }
}
