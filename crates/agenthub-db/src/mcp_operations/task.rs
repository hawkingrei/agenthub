use agenthub_agent_domain::{
    loop_runtime::LoopReservation,
    mcp_operations::{
        McpCompletion, McpDeferralKind, McpOperationRecord, McpTaskAuthority, McpTaskInputRequest,
        McpTaskLookupInput, McpTaskLookupMethod, McpTaskLookupRecord, McpTaskReceipt,
        McpTaskVersion,
    },
};
use sqlx::{Connection, Row, Sqlite, Transaction};
use uuid::Uuid;

use super::{McpJournalError, McpOperationStore, attempts::complete_attempt, parse_operation};
use crate::loop_runtime::LoopStore;

/// Authorizes a single read of a previously received task handle. It does not authorize a tool send.
pub struct McpTaskLookupPermit {
    id: String,
    operation_id: String,
    attempt_number: u32,
}

impl McpTaskLookupPermit {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
    pub fn attempt_number(&self) -> u32 {
        self.attempt_number
    }
}

impl McpOperationStore {
    pub async fn begin_task_lookup(
        &self,
        executor: &LoopReservation,
        authority: &McpTaskAuthority,
        input: &McpTaskLookupInput,
        now: i64,
    ) -> anyhow::Result<McpTaskLookupPermit> {
        anyhow::ensure!(now >= 0, "invalid MCP journal timestamp");
        anyhow::ensure!(
            input.method != McpTaskLookupMethod::Result
                || input.receipt.version == McpTaskVersion::November2025,
            McpJournalError::ContinuationRequired
        );
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        let operation = self
            .resolve_task_tx(&mut tx, executor, authority, &input.receipt, now)
            .await?;
        let attempt_number = operation.attempt_count;
        let reused: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM mcp_operation_task_lookups \
            WHERE operation_id = ? AND request_key = ?)",
        )
        .bind(&operation.id)
        .bind(input.request_key.as_str())
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(!reused, McpJournalError::IdentityConflict);
        // Bound durable query history independently of the number of tool/continuation sends.
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM mcp_operation_task_lookups WHERE operation_id = ?",
        )
        .bind(&operation.id)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(count < 4096, McpJournalError::ContinuationRequired);
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO mcp_operation_task_lookups(id, operation_id, attempt_number, request_key, request_digest, \
            method_json, activation_id, generation, daemon_node_id, daemon_generation, daemon_owner_id, sent_at) \
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id).bind(&operation.id).bind(attempt_number).bind(input.request_key.as_str()).bind(input.request_digest.as_str())
            .bind(serde_json::to_string(&input.method)?).bind(&executor.activation_id).bind(executor.generation)
            .bind(&self.daemon.node_id).bind(self.daemon.generation).bind(&self.daemon.owner_id).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(McpTaskLookupPermit {
            id,
            operation_id: operation.id,
            attempt_number,
        })
    }

    pub(super) async fn resolve_task_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        executor: &LoopReservation,
        authority: &McpTaskAuthority,
        receipt: &McpTaskReceipt,
        now: i64,
    ) -> anyhow::Result<McpOperationRecord> {
        self.require_current_daemon(tx).await?;
        LoopStore::verify_executor_live_tx(tx, executor, now).await?;
        let rows = sqlx::query("SELECT o.*, t.attempt_number AS task_attempt_number, t.receipt_json \
            FROM mcp_operation_tasks t JOIN mcp_operations o ON o.id = t.operation_id AND o.attempt_count = t.attempt_number \
            WHERE o.team_id = ? AND o.actor_id = ? AND o.server_id = ? AND o.scope_digest = ? \
            AND json_extract(o.intent_json, '$.binding_digest') = ? AND t.task_digest = ? LIMIT 65")
            .bind(&executor.team_id).bind(&executor.actor_id).bind(&authority.server_id)
            .bind(authority.scope_digest.as_str()).bind(authority.binding_digest.as_str())
            .bind(receipt.task_digest.as_str()).fetch_all(&mut **tx).await?;
        anyhow::ensure!(rows.len() <= 64, McpJournalError::ContinuationRequired);
        let mut candidates = Vec::new();
        for row in rows {
            let recorded: McpTaskReceipt = serde_json::from_str(row.try_get("receipt_json")?)?;
            if recorded == *receipt {
                let operation = parse_operation(&row)?;
                // Task authority cannot outlive the originating tool's discovered binding.
                anyhow::ensure!(
                    authority.tools.get(&operation.intent.tool_name)
                        == Some(&operation.intent.schema_digest),
                    McpJournalError::ScopeMismatch
                );
                candidates.push(operation);
            }
        }
        anyhow::ensure!(candidates.len() == 1, McpJournalError::ContinuationRequired);
        Ok(candidates.pop().unwrap())
    }

    /// Lookup failures never change the tool outcome. A factual terminal task result can settle
    /// the original deferred attempt even after the querying executor has stopped.
    pub async fn complete_task_lookup(
        &self,
        permit: &McpTaskLookupPermit,
        completion: &McpCompletion,
        outcome: Option<&McpCompletion>,
        now: i64,
    ) -> anyhow::Result<()> {
        self.complete_task_lookup_with_inputs(permit, completion, outcome, None, now)
            .await
    }

    pub async fn complete_task_lookup_with_inputs(
        &self,
        permit: &McpTaskLookupPermit,
        completion: &McpCompletion,
        outcome: Option<&McpCompletion>,
        inputs: Option<&[McpTaskInputRequest]>,
        now: i64,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(now >= 0, "invalid MCP journal timestamp");
        anyhow::ensure!(
            outcome.is_none_or(|outcome| matches!(
                outcome,
                McpCompletion::Succeeded { .. } | McpCompletion::Failed { .. }
            )),
            McpJournalError::ContinuationRequired
        );
        anyhow::ensure!(
            outcome.is_none() || matches!(completion, McpCompletion::Succeeded { .. }),
            McpJournalError::ContinuationRequired
        );
        anyhow::ensure!(
            inputs.is_none()
                || outcome.is_none() && matches!(completion, McpCompletion::Succeeded { .. }),
            McpJournalError::ContinuationRequired
        );
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        let row = sqlx::query("SELECT * FROM mcp_operation_task_lookups WHERE id = ? AND operation_id = ? AND attempt_number = ?")
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
        let inputs_valid = if let Some(inputs) = inputs {
            let task: String = sqlx::query_scalar("SELECT receipt_json FROM mcp_operation_tasks WHERE operation_id = ? AND attempt_number = ?")
                .bind(&permit.operation_id).bind(permit.attempt_number).fetch_one(&mut *tx).await?;
            anyhow::ensure!(
                serde_json::from_str::<McpTaskReceipt>(&task)?.version == McpTaskVersion::July2026
                    && serde_json::from_str::<McpTaskLookupMethod>(row.try_get("method_json")?)?
                        == McpTaskLookupMethod::Get,
                McpJournalError::ContinuationRequired
            );
            Self::record_task_inputs_tx(
                &mut tx,
                &permit.operation_id,
                permit.attempt_number,
                inputs,
            )
            .await?
        } else {
            true
        };
        let invalid_inputs = McpCompletion::OutcomeUnknown {
            reason: agenthub_agent_domain::mcp_operations::McpAmbiguityReason::InvalidResponse,
        };
        let completion = if inputs_valid {
            completion
        } else {
            &invalid_inputs
        };
        sqlx::query("UPDATE mcp_operation_task_lookups SET completion_json = ?, outcome_json = ?, completed_at = ? WHERE id = ?")
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
        anyhow::ensure!(inputs_valid, McpJournalError::TaskInputConflict);
        Ok(())
    }

    pub async fn task_lookups(
        &self,
        team_id: &str,
        actor_id: &str,
        operation_id: &str,
        after: i64,
        limit: u32,
    ) -> anyhow::Result<Vec<McpTaskLookupRecord>> {
        let rows = sqlx::query("SELECT q.* FROM mcp_operation_task_lookups q JOIN mcp_operations o ON o.id = q.operation_id \
            WHERE o.team_id = ? AND o.actor_id = ? AND o.id = ? AND q.sequence > ? ORDER BY q.sequence LIMIT ?")
            .bind(team_id).bind(actor_id).bind(operation_id).bind(after.max(0)).bind(limit.clamp(1, 100)).fetch_all(&self.pool).await?;
        rows.iter()
            .map(|row| {
                Ok(McpTaskLookupRecord {
                    sequence: row.try_get("sequence")?,
                    id: row.try_get("id")?,
                    operation_id: row.try_get("operation_id")?,
                    attempt_number: row.try_get("attempt_number")?,
                    activation_id: row.try_get("activation_id")?,
                    method: serde_json::from_str(row.try_get("method_json")?)?,
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
            .collect()
    }

    pub(super) async fn recover_task_lookups_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        limit: u32,
        now: i64,
    ) -> anyhow::Result<u64> {
        let completion = McpCompletion::OutcomeUnknown {
            reason: agenthub_agent_domain::mcp_operations::McpAmbiguityReason::DaemonRestart,
        };
        Ok(sqlx::query("UPDATE mcp_operation_task_lookups SET completion_json = ?, completed_at = MAX(sent_at, ?) \
            WHERE id IN (SELECT id FROM mcp_operation_task_lookups WHERE daemon_node_id = ? AND completed_at IS NULL \
                AND (daemon_generation != ? OR daemon_owner_id != ?) ORDER BY sequence LIMIT ?)")
            .bind(serde_json::to_string(&completion)?).bind(now).bind(&self.daemon.node_id)
            .bind(self.daemon.generation).bind(&self.daemon.owner_id).bind(limit).execute(&mut **tx).await?.rows_affected())
    }
}

/// An admitted task exchange may settle its original attempt after executor shutdown.
pub(super) async fn settle_task_tx(
    tx: &mut Transaction<'_, Sqlite>,
    operation_id: &str,
    attempt_number: u32,
    activation_id: &str,
    outcome: &McpCompletion,
    now: i64,
) -> anyhow::Result<()> {
    let operation = sqlx::query("SELECT * FROM mcp_operations WHERE id = ?")
        .bind(operation_id)
        .fetch_one(&mut **tx)
        .await?;
    let operation = parse_operation(&operation)?;
    // First terminal fact wins; later conflicting exchanges retain their own receipts.
    if operation.attempt_count == attempt_number
        && matches!(
            operation.completion,
            Some(McpCompletion::Deferred {
                reason: McpDeferralKind::TaskAccepted,
                ..
            })
        )
    {
        complete_attempt(
            tx,
            operation_id,
            attempt_number,
            activation_id,
            outcome,
            now.max(operation.updated_at),
        )
        .await?;
    }
    Ok(())
}
