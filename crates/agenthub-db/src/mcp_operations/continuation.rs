use agenthub_agent_domain::{
    loop_runtime::LoopReservation,
    mcp_operations::{McpCompletion, McpContinuationInput, McpDeferralKind, McpOperationIntent},
};
use sqlx::{Connection, Row};

use super::{
    McpJournalError, McpOperationStore, McpSendPermit, parse_operation,
    prepare::reject_prior_effects,
};
use crate::loop_runtime::LoopStore;

const MAX_ROUNDS: i64 = 10;
const MAX_RETRIES_PER_ROUND: i64 = 3;

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
        let rows = sqlx::query("SELECT o.*, c.attempt_number AS retry_of_attempt_number, \
            p.completion_json AS parent_completion_json FROM mcp_operations o \
            LEFT JOIN mcp_operation_continuation_retries r ON r.operation_id = o.id AND r.attempt_number = o.attempt_count \
            LEFT JOIN mcp_operation_continuations c ON c.operation_id = o.id \
                AND c.attempt_number = COALESCE(r.continuation_attempt_number, o.attempt_count) \
            LEFT JOIN mcp_operation_attempts p ON p.operation_id = o.id AND p.number = c.parent_attempt_number \
            WHERE o.team_id = ? AND o.actor_id = ? \
            AND o.scope_digest = ? AND o.tool_name = ? AND o.arguments_digest = ? \
            AND ((o.status = 'outcome_unknown' AND json_extract(o.completion_json, '$.kind') = 'deferred' \
                AND json_extract(o.completion_json, '$.reason') = 'input_required' \
                AND json_extract(o.completion_json, '$.input_receipt.state_digest') IS ?) \
            OR (o.status IN ('outcome_unknown', 'failed') \
                AND json_extract(o.completion_json, '$.kind') IN ('outcome_unknown', 'failed') \
                AND c.request_digest = ?)) LIMIT 65")
            .bind(&executor.team_id).bind(&executor.actor_id).bind(intent.scope_digest.as_str())
            .bind(&intent.tool_name).bind(intent.arguments_digest.as_str())
            .bind(input.state_digest.as_ref().map(|value| value.as_str()))
            .bind(input.request_digest.as_str())
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
            let (completion, retry_of) = match &operation.completion {
                Some(McpCompletion::OutcomeUnknown { .. } | McpCompletion::Failed { .. }) => (
                    row.try_get::<Option<&str>, _>("parent_completion_json")?
                        .map(serde_json::from_str::<McpCompletion>)
                        .transpose()?,
                    row.try_get::<Option<u32>, _>("retry_of_attempt_number")?,
                ),
                completion => (completion.clone(), None),
            };
            if let Some(McpCompletion::Deferred {
                reason: McpDeferralKind::InputRequired,
                response_digest,
                input_receipt: Some(receipt),
                ..
            }) = completion
                && receipt.state_digest == input.state_digest
            {
                candidates.push((operation, response_digest, receipt, retry_of));
            }
        }
        // Without an opaque state token, parallel identical reads can still be distinguished by
        // their server-issued input IDs. Missing/extra inputs remain upstream validation concerns.
        if input.state_digest.is_none() {
            candidates.retain(|(_, _, receipt, _)| {
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
            candidates.retain(|(_, _, receipt, retry_of)| {
                // A recorded retry already matches the complete round, even if that round
                // supplied only extra inputs. Never discard it to select a different receipt.
                retry_of.is_some()
                    || input
                        .input_ids
                        .iter()
                        .any(|id| receipt.input_ids.contains(id))
            });
        }
        anyhow::ensure!(candidates.len() == 1, McpJournalError::ContinuationRequired);
        let (operation, parent_response_digest, receipt, retry_of) = candidates.pop().unwrap();
        anyhow::ensure!(
            receipt.request_id_digest != input.request_id_digest,
            McpJournalError::IdentityConflict
        );
        if let Some(round) = retry_of {
            anyhow::ensure!(
                operation.intent.replay_safety.permits_retry(),
                McpJournalError::UnsafeReplay
            );
            let retries: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM mcp_operation_continuation_retries WHERE operation_id = ? AND continuation_attempt_number = ?",
            )
            .bind(&operation.id).bind(round).fetch_one(&mut *tx).await?;
            anyhow::ensure!(
                retries < MAX_RETRIES_PER_ROUND,
                McpJournalError::ContinuationRequired
            );
        } else {
            let rounds: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM mcp_operation_continuations WHERE operation_id = ?",
            )
            .bind(&operation.id)
            .fetch_one(&mut *tx)
            .await?;
            anyhow::ensure!(rounds < MAX_ROUNDS, McpJournalError::ContinuationRequired);
        }
        let reused: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM mcp_operation_attempts \
            WHERE operation_id = ? AND json_extract(completion_json, '$.input_receipt.request_id_digest') = ?) \
            OR EXISTS(SELECT 1 FROM mcp_operation_continuations c JOIN mcp_operations o ON o.id = c.operation_id \
            WHERE o.team_id = ? AND o.actor_id = ? AND (c.request_key = ? OR (c.operation_id = ? AND c.request_id_digest = ?))) \
            OR EXISTS(SELECT 1 FROM mcp_operation_continuation_retries r JOIN mcp_operations o ON o.id = r.operation_id \
            WHERE o.team_id = ? AND o.actor_id = ? AND (r.request_key = ? OR (r.operation_id = ? AND r.request_id_digest = ?))) \
            OR EXISTS(SELECT 1 FROM mcp_operations WHERE team_id = ? AND actor_id = ? AND request_key = ?)")
            .bind(&operation.id).bind(input.request_id_digest.as_str())
            .bind(&executor.team_id).bind(&executor.actor_id).bind(intent.request_key.as_str())
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
        if let Some(round) = retry_of {
            // The receipt is consumed once. Retries refer to that round's original send and never
            // change its parameters or rewrite the attempt that lost its result.
            sqlx::query("INSERT INTO mcp_operation_continuation_retries(operation_id, attempt_number, \
                continuation_attempt_number, request_key, request_id_digest) VALUES (?, ?, ?, ?, ?)")
                .bind(&operation.id).bind(permit.attempt_number()).bind(round)
                .bind(intent.request_key.as_str()).bind(input.request_id_digest.as_str())
                .execute(&mut *tx).await?;
        } else {
            sqlx::query("INSERT INTO mcp_operation_continuations(operation_id, attempt_number, parent_attempt_number, \
            parent_response_digest, request_key, request_id_digest, request_digest) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&operation.id).bind(permit.attempt_number()).bind(operation.attempt_count)
            .bind(parent_response_digest.as_str()).bind(intent.request_key.as_str())
            .bind(input.request_id_digest.as_str()).bind(input.request_digest.as_str())
            .execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(permit)
    }
}
