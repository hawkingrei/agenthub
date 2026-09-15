use agenthub_agent_domain::mcp_operations::{McpTaskInputRecord, McpTaskInputRequest};
use sqlx::{Row, Sqlite, Transaction};

use super::McpOperationStore;

impl McpOperationStore {
    pub(super) async fn record_task_inputs_tx(
        tx: &mut Transaction<'_, Sqlite>,
        operation_id: &str,
        attempt_number: u32,
        inputs: &[McpTaskInputRequest],
    ) -> anyhow::Result<bool> {
        if inputs.len() > 64 {
            return Ok(false);
        }
        let mut count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_task_inputs WHERE operation_id = ? AND attempt_number = ?")
            .bind(operation_id).bind(attempt_number).fetch_one(&mut **tx).await?;
        let mut valid = true;
        for input in inputs {
            let row = sqlx::query(
                "SELECT request_digest, conflicted FROM mcp_operation_task_inputs \
                WHERE operation_id = ? AND attempt_number = ? AND input_id_digest = ?",
            )
            .bind(operation_id)
            .bind(attempt_number)
            .bind(input.input_id_digest.as_str())
            .fetch_optional(&mut **tx)
            .await?;
            if let Some(row) = row {
                let changed =
                    row.try_get::<&str, _>("request_digest")? != input.request_digest.as_str();
                if changed {
                    // Retain equivocation even when this response cannot be delivered. A restart
                    // must not restore authority to answer a key whose meaning changed.
                    sqlx::query(
                        "UPDATE mcp_operation_task_inputs SET conflicted = 1 \
                        WHERE operation_id = ? AND attempt_number = ? AND input_id_digest = ?",
                    )
                    .bind(operation_id)
                    .bind(attempt_number)
                    .bind(input.input_id_digest.as_str())
                    .execute(&mut **tx)
                    .await?;
                }
                valid &= !changed && !row.try_get::<bool, _>("conflicted")?;
            } else if count < 4096 {
                sqlx::query("INSERT INTO mcp_operation_task_inputs(operation_id, attempt_number, input_id_digest, request_digest) VALUES (?, ?, ?, ?)")
                    .bind(operation_id).bind(attempt_number).bind(input.input_id_digest.as_str()).bind(input.request_digest.as_str()).execute(&mut **tx).await?;
                count += 1;
            } else {
                valid = false;
            }
        }
        Ok(valid)
    }

    pub async fn task_inputs(
        &self,
        team_id: &str,
        actor_id: &str,
        operation_id: &str,
        after: i64,
        limit: u32,
    ) -> anyhow::Result<Vec<McpTaskInputRecord>> {
        let rows = sqlx::query("SELECT i.* FROM mcp_operation_task_inputs i JOIN mcp_operations o ON o.id = i.operation_id \
            WHERE o.team_id = ? AND o.actor_id = ? AND o.id = ? AND i.sequence > ? ORDER BY i.sequence LIMIT ?")
            .bind(team_id).bind(actor_id).bind(operation_id).bind(after.max(0)).bind(limit.clamp(1, 100)).fetch_all(&self.pool).await?;
        rows.iter()
            .map(|row| {
                Ok(McpTaskInputRecord {
                    sequence: row.try_get("sequence")?,
                    attempt_number: row.try_get("attempt_number")?,
                    input_id_digest: row.try_get::<String, _>("input_id_digest")?.try_into()?,
                    request_digest: row.try_get::<String, _>("request_digest")?.try_into()?,
                    update_id: row.try_get("update_id")?,
                    conflicted: row.try_get("conflicted")?,
                })
            })
            .collect()
    }
}
