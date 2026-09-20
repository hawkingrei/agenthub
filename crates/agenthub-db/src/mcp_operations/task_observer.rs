use agenthub_agent_domain::{
    loop_runtime::LoopReservation,
    mcp_operations::{McpTaskObservation, McpTaskObservationBinding, McpTaskReceipt},
};
use sqlx::{Connection, Row};

use super::{McpJournalError, McpOperationStore, McpTaskNotificationPermit};
use crate::loop_runtime::LoopStore;

/// Authenticated ownership of incoming facts. This survives executor exit but grants no send.
/// Only the trusted transport combines this owner with its private binding and HTTP context.
pub struct McpTaskObservationOwner {
    team_id: String,
    actor_id: String,
    activation_id: String,
}

impl McpOperationStore {
    pub async fn authorize_task_observation_owner(
        &self,
        executor: &LoopReservation,
        now: i64,
    ) -> anyhow::Result<McpTaskObservationOwner> {
        let mut tx = self.pool.begin().await?;
        self.require_current_daemon(&mut tx).await?;
        LoopStore::verify_executor_phase_tx(&mut tx, executor, now, true).await?;
        let owner = McpTaskObservationOwner {
            team_id: executor.team_id.clone(),
            actor_id: executor.actor_id.clone(),
            activation_id: executor
                .activation_id
                .clone()
                .ok_or(McpJournalError::ScopeMismatch)?,
        };
        tx.commit().await?;
        Ok(owner)
    }

    pub async fn record_scoped_task_notification(
        &self,
        owner: &McpTaskObservationOwner,
        binding: &McpTaskObservationBinding,
        observation: &McpTaskObservation,
        now: i64,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(now >= 0, "invalid MCP journal timestamp");
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        let rows = sqlx::query(
            "SELECT t.*, o.tool_name FROM mcp_operation_tasks t JOIN mcp_operations o ON o.id = t.operation_id \
            WHERE o.team_id = ? AND o.actor_id = ? AND o.server_id = ? AND o.scope_digest = ? \
            AND json_extract(o.intent_json, '$.binding_digest') = ? AND t.task_digest = ? LIMIT 65",
        )
        .bind(&owner.team_id)
        .bind(&owner.actor_id)
        .bind(&binding.server_id)
        .bind(binding.scope_digest.as_str())
        .bind(binding.binding_digest.as_str())
        .bind(observation.receipt.task_digest.as_str())
        .fetch_all(&mut *tx)
        .await?;
        anyhow::ensure!(rows.len() <= 64, McpJournalError::IdentityConflict);
        let mut matched = None;
        for row in rows {
            let receipt: McpTaskReceipt = serde_json::from_str(row.try_get("receipt_json")?)?;
            if receipt != observation.receipt {
                continue;
            }
            anyhow::ensure!(matched.is_none(), McpJournalError::IdentityConflict);
            matched = Some(McpTaskNotificationPermit {
                operation_id: row.try_get("operation_id")?,
                attempt_number: row.try_get("attempt_number")?,
                activation_id: owner.activation_id.clone(),
                tool_name: row.try_get("tool_name")?,
                receipt,
            });
        }
        let permit = matched.ok_or(McpJournalError::TaskReceiptMissing)?;
        // The original accepted attempt already records its admitted schema. A later catalog
        // invalidation may stop outgoing calls, but cannot erase an incoming fact for that task.
        let valid = Self::record_task_notification_tx(
            &mut tx,
            &permit,
            &observation.response_digest,
            observation.outcome.as_ref(),
            observation.inputs.as_deref(),
            now,
        )
        .await?;
        tx.commit().await?;
        anyhow::ensure!(valid, McpJournalError::TaskInputConflict);
        Ok(())
    }
}
