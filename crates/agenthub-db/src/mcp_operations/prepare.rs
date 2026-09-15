use agenthub_agent_domain::{
    loop_runtime::LoopReservation,
    mcp_operations::{McpOperationIntent, McpOperationRecord, McpOperationStatus},
};
use sqlx::{Connection, Sqlite, Transaction};
use uuid::Uuid;

use crate::loop_runtime::LoopStore;

use super::{McpJournalError, McpOperationStore, parse_operation, record_event};

impl McpOperationStore {
    /// This transaction never sends an upstream request. A prepared operation remains safe to send
    /// after a crash, but every later send must independently revalidate execution authority.
    pub async fn prepare(
        &self,
        executor: &LoopReservation,
        intent: &McpOperationIntent,
        now: i64,
    ) -> anyhow::Result<McpOperationRecord> {
        intent.validate()?;
        anyhow::ensure!(now >= 0, "invalid MCP journal timestamp");
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        self.require_current_daemon(&mut tx).await?;
        LoopStore::verify_executor_live_tx(&mut tx, executor, now).await?;
        let existing = sqlx::query(
            "SELECT * FROM mcp_operations WHERE team_id = ? AND actor_id = ? AND request_key = ?",
        )
        .bind(&executor.team_id)
        .bind(&executor.actor_id)
        .bind(intent.request_key.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(row) = existing {
            let record = parse_operation(&row)?;
            anyhow::ensure!(record.intent == *intent, McpJournalError::IdentityConflict);
            tx.commit().await?;
            return Ok(record);
        }
        reject_prior_effects(&mut tx, &executor.team_id, intent, None).await?;
        let id = Uuid::new_v4().to_string();
        let activation = executor
            .activation_id
            .as_deref()
            .ok_or(McpJournalError::ScopeMismatch)?;
        sqlx::query("INSERT INTO mcp_operations(id, actor_id, team_id, origin_activation_id, request_key, \
            server_id, scope_digest, tool_name, arguments_digest, identity_digest, intent_json, status, created_at, updated_at) \
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'prepared', ?, ?)")
            .bind(&id).bind(&executor.actor_id).bind(&executor.team_id).bind(activation)
            .bind(intent.request_key.as_str()).bind(&intent.server_id).bind(intent.scope_digest.as_str())
            .bind(&intent.tool_name).bind(intent.arguments_digest.as_str())
            .bind(intent.replay_safety.identity_digest().map(|value| value.as_str()))
            .bind(serde_json::to_string(intent)?).bind(now).bind(now).execute(&mut *tx).await?;
        record_event(
            &mut tx,
            &id,
            0,
            activation,
            McpOperationStatus::Prepared,
            None,
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(McpOperationRecord {
            id,
            actor_id: executor.actor_id.clone(),
            team_id: executor.team_id.clone(),
            origin_activation_id: activation.to_owned(),
            intent: intent.clone(),
            status: McpOperationStatus::Prepared,
            attempt_count: 0,
            completion: None,
            created_at: now,
            updated_at: now,
        })
    }
}

/// A fresh correlation ID is not evidence that an unresolved side effect is safe to repeat.
/// The predicate intentionally excludes profile names and binding/schema revisions so changing
/// configuration cannot hide an earlier attempt against the same upstream scope and arguments.
/// Across the authority upgrade, an older digest cannot establish a different namespace. Match
/// it conservatively within the same stable integration ID, in both prepare/send directions.
/// This expands replay rejection only; task and continuation authority still require exact intent.
pub(super) async fn reject_prior_effects(
    tx: &mut Transaction<'_, Sqlite>,
    team_id: &str,
    intent: &McpOperationIntent,
    except_id: Option<&str>,
) -> anyhow::Result<()> {
    if let Some(identity) = intent.replay_safety.identity_digest() {
        let reused: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM mcp_operations WHERE team_id = ? \
            AND (scope_digest = ? OR (server_id = ? AND \
                COALESCE(json_extract(intent_json, '$.scope_identity') = 'verified_authority', 0) != ?)) \
            AND tool_name = ? AND identity_digest = ? AND id IS NOT ?)",
        )
        .bind(team_id)
        .bind(intent.scope_digest.as_str())
        .bind(&intent.server_id)
        .bind(!intent.scope_identity.is_legacy())
        .bind(&intent.tool_name)
        .bind(identity.as_str())
        .bind(except_id)
        .fetch_one(&mut **tx)
        .await?;
        // Stable identities resolve to the original request key in trusted proxy policy.
        anyhow::ensure!(!reused, McpJournalError::IdentityConflict);
    }
    let previous: Option<String> = sqlx::query_scalar(
        "SELECT status FROM mcp_operations WHERE team_id = ? \
        AND (scope_digest = ? OR (server_id = ? AND \
            COALESCE(json_extract(intent_json, '$.scope_identity') = 'verified_authority', 0) != ?)) \
        AND tool_name = ? AND arguments_digest = ? AND id IS NOT ? \
        AND status IN ('sent', 'outcome_unknown', 'failed') \
        AND json_extract(intent_json, '$.replay_safety.kind') != 'read_only' LIMIT 1",
    )
    .bind(team_id)
    .bind(intent.scope_digest.as_str())
    .bind(&intent.server_id)
    .bind(!intent.scope_identity.is_legacy())
    .bind(&intent.tool_name)
    .bind(intent.arguments_digest.as_str())
    .bind(except_id)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(status) = previous {
        if status == McpOperationStatus::Sent.as_str() {
            return Err(McpJournalError::InFlight.into());
        }
        return Err(McpJournalError::UnsafeReplay.into());
    }
    Ok(())
}
