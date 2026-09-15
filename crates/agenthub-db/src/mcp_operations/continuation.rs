use agenthub_agent_domain::{
    loop_runtime::LoopReservation,
    mcp_operations::{McpCompletion, McpContinuationInput, McpDeferralKind, McpOperationIntent},
};
use sqlx::Connection;

use super::{
    McpJournalError, McpOperationStore, McpSendPermit, parse_operation,
    prepare::reject_prior_effects,
};
use crate::loop_runtime::LoopStore;

const MAX_ROUNDS: i64 = 10;

impl McpOperationStore {
    /// Resolve one current upstream receipt and commit exactly one linked send. No raw state or
    /// input enters the journal; the caller retains the already bound HTTP request until commit.
    pub async fn begin_continuation(
        &self,
        executor: &LoopReservation,
        intent: &McpOperationIntent,
        input: &McpContinuationInput,
        now: i64,
    ) -> anyhow::Result<McpSendPermit> {
        intent.validate()?;
        anyhow::ensure!(now >= 0, "invalid MCP journal timestamp");
        anyhow::ensure!(
            input.input_ids.len() <= 64 && intent.request_digest.is_some(),
            McpJournalError::ContinuationRequired
        );
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        self.require_current_daemon(&mut tx).await?;
        LoopStore::verify_executor_live_tx(&mut tx, executor, now).await?;
        let rows = sqlx::query("SELECT * FROM mcp_operations WHERE team_id = ? AND actor_id = ? \
            AND scope_digest = ? AND tool_name = ? AND arguments_digest = ? AND status = 'outcome_unknown' \
            AND json_extract(completion_json, '$.kind') = 'deferred' \
            AND json_extract(completion_json, '$.reason') = 'input_required' \
            AND json_extract(completion_json, '$.input_receipt.state_digest') IS ? LIMIT 65")
            .bind(&executor.team_id).bind(&executor.actor_id).bind(intent.scope_digest.as_str())
            .bind(&intent.tool_name).bind(intent.arguments_digest.as_str())
            .bind(input.state_digest.as_ref().map(|value| value.as_str()))
            .fetch_all(&mut *tx).await?;
        anyhow::ensure!(rows.len() <= 64, McpJournalError::ContinuationRequired);
        let mut candidates = Vec::new();
        for row in rows {
            let operation = parse_operation(&row)?;
            let mut bound = intent.clone();
            bound.request_key = operation.intent.request_key.clone();
            if bound != operation.intent {
                continue;
            }
            if let Some(McpCompletion::Deferred {
                reason: McpDeferralKind::InputRequired,
                response_digest,
                input_receipt: Some(receipt),
            }) = &operation.completion
                && receipt.state_digest == input.state_digest
            {
                candidates.push((operation.clone(), response_digest.clone(), receipt.clone()));
            }
        }
        // Without an opaque state token, parallel identical reads can still be distinguished by
        // their server-issued input IDs. Missing/extra inputs remain upstream validation concerns.
        if input.state_digest.is_none() {
            candidates.retain(|(_, _, receipt)| {
                if receipt.input_ids.is_empty() {
                    input.input_ids.is_empty()
                } else {
                    input
                        .input_ids
                        .iter()
                        .any(|id| receipt.input_ids.contains(id))
                }
            });
        } else if candidates.len() > 1 && !input.input_ids.is_empty() {
            candidates.retain(|(_, _, receipt)| {
                input
                    .input_ids
                    .iter()
                    .any(|id| receipt.input_ids.contains(id))
            });
        }
        anyhow::ensure!(candidates.len() == 1, McpJournalError::ContinuationRequired);
        let (operation, parent_response_digest, receipt) = candidates.pop().unwrap();
        anyhow::ensure!(
            receipt.request_id_digest != input.request_id_digest,
            McpJournalError::IdentityConflict
        );
        let rounds: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM mcp_operation_continuations WHERE operation_id = ?",
        )
        .bind(&operation.id)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(rounds < MAX_ROUNDS, McpJournalError::ContinuationRequired);
        let reused: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM mcp_operation_attempts \
            WHERE operation_id = ? AND json_extract(completion_json, '$.input_receipt.request_id_digest') = ?) \
            OR EXISTS(SELECT 1 FROM mcp_operation_continuations c JOIN mcp_operations o ON o.id = c.operation_id \
            WHERE o.team_id = ? AND o.actor_id = ? AND (c.request_key = ? OR (c.operation_id = ? AND c.request_id_digest = ?))) \
            OR EXISTS(SELECT 1 FROM mcp_operations WHERE team_id = ? AND actor_id = ? AND request_key = ?)")
            .bind(&operation.id).bind(input.request_id_digest.as_str())
            .bind(&executor.team_id).bind(&executor.actor_id).bind(intent.request_key.as_str())
            .bind(&operation.id).bind(input.request_id_digest.as_str())
            .bind(&executor.team_id).bind(&executor.actor_id).bind(intent.request_key.as_str())
            .fetch_one(&mut *tx).await?;
        anyhow::ensure!(!reused, McpJournalError::IdentityConflict);
        reject_prior_effects(
            &mut tx,
            &executor.team_id,
            &operation.intent,
            Some(&operation.id),
        )
        .await?;
        let permit = self
            .insert_send_tx(&mut tx, executor, &operation, now)
            .await?;
        sqlx::query("INSERT INTO mcp_operation_continuations(operation_id, attempt_number, parent_attempt_number, \
            parent_response_digest, request_key, request_id_digest, request_digest) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&operation.id).bind(permit.attempt_number()).bind(operation.attempt_count)
            .bind(parent_response_digest.as_str()).bind(intent.request_key.as_str())
            .bind(input.request_id_digest.as_str()).bind(input.request_digest.as_str())
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(permit)
    }
}
