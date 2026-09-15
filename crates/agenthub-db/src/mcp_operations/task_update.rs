use agenthub_agent_domain::{
    loop_runtime::LoopReservation,
    mcp_operations::{
        McpAmbiguityReason, McpCompletion, McpDeferralKind, McpTaskAuthority, McpTaskUpdateInput,
        McpTaskUpdateRecord, McpTaskVersion,
    },
};
use sqlx::{Connection, Row, Sqlite, Transaction};
use uuid::Uuid;

use super::{McpJournalError, McpOperationStore};

pub struct McpTaskUpdatePermit {
    id: String,
    operation_id: String,
    attempt_number: u32,
}

impl McpTaskUpdatePermit {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
    pub fn attempt_number(&self) -> u32 {
        self.attempt_number
    }
}

impl McpOperationStore {
    pub async fn begin_task_update(
        &self,
        executor: &LoopReservation,
        authority: &McpTaskAuthority,
        input: &McpTaskUpdateInput,
        now: i64,
    ) -> anyhow::Result<McpTaskUpdatePermit> {
        anyhow::ensure!(now >= 0, "invalid MCP journal timestamp");
        anyhow::ensure!(
            input.receipt.version == McpTaskVersion::July2026 && input.inputs.len() <= 64,
            McpJournalError::ContinuationRequired
        );
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
        let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM mcp_operation_task_inputs \
            WHERE operation_id = ? AND attempt_number = ? AND conflicted != 0) OR EXISTS(SELECT 1 FROM mcp_operation_task_cancellations \
            WHERE operation_id = ? AND attempt_number = ?)")
            .bind(&operation.id).bind(operation.attempt_count).bind(&operation.id).bind(operation.attempt_count).fetch_one(&mut *tx).await?;
        anyhow::ensure!(!blocked, McpJournalError::ContinuationRequired);
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM mcp_operation_task_updates WHERE operation_id = ?",
        )
        .bind(&operation.id)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(count < 4096, McpJournalError::ContinuationRequired);
        let id = Uuid::new_v4().to_string();
        let inserted = sqlx::query("INSERT INTO mcp_operation_task_updates(id, operation_id, attempt_number, request_key, request_digest, inputs_json, \
            activation_id, daemon_node_id, daemon_generation, daemon_owner_id, sent_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
            ON CONFLICT(operation_id, request_key) DO NOTHING")
            .bind(&id).bind(&operation.id).bind(operation.attempt_count).bind(input.request_key.as_str()).bind(input.request_digest.as_str())
            .bind(serde_json::to_string(&input.inputs)?).bind(&executor.activation_id).bind(&self.daemon.node_id)
            .bind(self.daemon.generation).bind(&self.daemon.owner_id).bind(now).execute(&mut *tx).await?;
        anyhow::ensure!(
            inserted.rows_affected() == 1,
            McpJournalError::IdentityConflict
        );
        for response in &input.inputs {
            let consumed = sqlx::query("UPDATE mcp_operation_task_inputs SET update_id = ? \
                WHERE operation_id = ? AND attempt_number = ? AND input_id_digest = ? AND update_id IS NULL AND conflicted = 0")
                .bind(&id).bind(&operation.id).bind(operation.attempt_count).bind(response.input_id_digest.as_str()).execute(&mut *tx).await?;
            anyhow::ensure!(
                consumed.rows_affected() == 1,
                McpJournalError::ContinuationRequired
            );
        }
        tx.commit().await?;
        Ok(McpTaskUpdatePermit {
            id,
            operation_id: operation.id,
            attempt_number: operation.attempt_count,
        })
    }

    pub async fn complete_task_update(
        &self,
        permit: &McpTaskUpdatePermit,
        completion: &McpCompletion,
        now: i64,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(now >= 0, "invalid MCP journal timestamp");
        anyhow::ensure!(
            !matches!(completion, McpCompletion::Deferred { .. }),
            McpJournalError::ContinuationRequired
        );
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        let row = sqlx::query("SELECT completion_json, sent_at FROM mcp_operation_task_updates WHERE id = ? AND operation_id = ? AND attempt_number = ?")
            .bind(&permit.id).bind(&permit.operation_id).bind(permit.attempt_number).fetch_optional(&mut *tx).await?.ok_or(McpJournalError::StaleAttempt)?;
        let previous: Option<McpCompletion> = row
            .try_get::<Option<&str>, _>("completion_json")?
            .map(serde_json::from_str)
            .transpose()?;
        if previous.as_ref() == Some(completion) {
            tx.commit().await?;
            return Ok(());
        }
        anyhow::ensure!(
            previous.is_none() || matches!(previous, Some(McpCompletion::OutcomeUnknown { .. })),
            McpJournalError::StaleAttempt
        );
        sqlx::query("UPDATE mcp_operation_task_updates SET completion_json = ?, completed_at = ? WHERE id = ?")
            .bind(serde_json::to_string(completion)?).bind(now.max(row.try_get("sent_at")?)).bind(&permit.id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn task_updates(
        &self,
        team_id: &str,
        actor_id: &str,
        operation_id: &str,
        after: i64,
        limit: u32,
    ) -> anyhow::Result<Vec<McpTaskUpdateRecord>> {
        let rows = sqlx::query("SELECT u.* FROM mcp_operation_task_updates u JOIN mcp_operations o ON o.id = u.operation_id \
            WHERE o.team_id = ? AND o.actor_id = ? AND o.id = ? AND u.sequence > ? ORDER BY u.sequence LIMIT ?")
            .bind(team_id).bind(actor_id).bind(operation_id).bind(after.max(0)).bind(limit.clamp(1, 100)).fetch_all(&self.pool).await?;
        rows.iter()
            .map(|row| {
                Ok(McpTaskUpdateRecord {
                    sequence: row.try_get("sequence")?,
                    id: row.try_get("id")?,
                    operation_id: row.try_get("operation_id")?,
                    attempt_number: row.try_get("attempt_number")?,
                    activation_id: row.try_get("activation_id")?,
                    inputs: serde_json::from_str(row.try_get("inputs_json")?)?,
                    completion: row
                        .try_get::<Option<&str>, _>("completion_json")?
                        .map(serde_json::from_str)
                        .transpose()?,
                })
            })
            .collect()
    }

    pub(super) async fn recover_task_updates_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        limit: u32,
        now: i64,
    ) -> anyhow::Result<u64> {
        let completion = McpCompletion::OutcomeUnknown {
            reason: McpAmbiguityReason::DaemonRestart,
        };
        Ok(sqlx::query("UPDATE mcp_operation_task_updates SET completion_json = ?, completed_at = MAX(sent_at, ?) \
            WHERE id IN (SELECT id FROM mcp_operation_task_updates WHERE daemon_node_id = ? AND completed_at IS NULL \
                AND (daemon_generation != ? OR daemon_owner_id != ?) ORDER BY sequence LIMIT ?)")
            .bind(serde_json::to_string(&completion)?).bind(now).bind(&self.daemon.node_id)
            .bind(self.daemon.generation).bind(&self.daemon.owner_id).bind(limit).execute(&mut **tx).await?.rows_affected())
    }
}
