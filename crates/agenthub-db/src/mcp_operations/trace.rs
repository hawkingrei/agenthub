//! Redacted history projections share the canonical send/completion transaction.

use agenthub_agent_domain::{
    loop_runtime::{LoopReservation, LoopToolStatus},
    mcp_operations::{McpCompletion, McpDeferralKind, McpOperationRecord},
};
use sqlx::{Row, Sqlite, Transaction};

pub(super) async fn begin(
    tx: &mut Transaction<'_, Sqlite>,
    executor: &LoopReservation,
    operation: &McpOperationRecord,
    number: u32,
    now: i64,
) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "INSERT INTO loop_tool_observations(activation_id, generation, surface, tool_name, target_ref, status, started_at, operation_id, attempt_number) \
         VALUES (?, ?, 'mcp_tool', ?, ?, 'started', ?, ?, ?) RETURNING id",
    ).bind(&executor.activation_id).bind(executor.generation)
        .bind(&operation.intent.tool_name).bind(&operation.intent.server_id).bind(now).bind(&operation.id).bind(number)
        .fetch_one(&mut **tx).await?)
}

pub(super) async fn complete(
    tx: &mut Transaction<'_, Sqlite>,
    operation_id: &str,
    number: u32,
    completion: &McpCompletion,
    now: i64,
    duration_ms: Option<i64>,
) -> anyhow::Result<()> {
    // Recovery and asynchronous task completion have no surviving local clock. Clearing an older
    // duration is preferable to attributing the initial receipt's latency to a later outcome.
    let id: i64 = sqlx::query_scalar(
        "UPDATE loop_tool_observations SET status = ?, completed_at = ?, duration_ms = ? \
         WHERE id = (SELECT tool_observation_id FROM mcp_operation_attempts WHERE operation_id = ? AND number = ?) \
         RETURNING id",
    ).bind(status(completion).as_str()).bind(now).bind(duration_ms).bind(operation_id).bind(number)
        .fetch_one(&mut **tx).await?;
    sqlx::query("INSERT INTO loop_activation_events(activation_id, kind, generation, created_at) \
        SELECT activation_id, 'tool_completed', generation, ? FROM loop_tool_observations WHERE id = ?")
        .bind(now).bind(id).execute(&mut **tx).await?;
    Ok(())
}

fn status(completion: &McpCompletion) -> LoopToolStatus {
    match completion {
        McpCompletion::Succeeded { .. } => LoopToolStatus::Succeeded,
        McpCompletion::Failed { .. } => LoopToolStatus::Failed,
        McpCompletion::OutcomeUnknown { .. } => LoopToolStatus::OutcomeUnknown,
        McpCompletion::Deferred {
            reason: McpDeferralKind::InputRequired,
            ..
        } => LoopToolStatus::InputRequired,
        McpCompletion::Deferred {
            reason: McpDeferralKind::TaskAccepted,
            ..
        } => LoopToolStatus::TaskAccepted,
    }
}

/// This runs once when the nullable attempt link is added, inside the migration transaction.
/// Existing wall-clock evidence is preserved, but historical monotonic durations are unknowable.
pub(super) async fn backfill(tx: &mut Transaction<'_, Sqlite>) -> anyhow::Result<()> {
    let mut after = (String::new(), 0_i64);
    loop {
        let rows = sqlx::query(
            "SELECT a.operation_id, a.number, a.activation_id, a.generation, a.completion_json, \
             a.sent_at, a.completed_at, o.tool_name, o.server_id \
             FROM mcp_operation_attempts a JOIN mcp_operations o ON o.id = a.operation_id \
             WHERE (a.operation_id, a.number) > (?, ?) ORDER BY a.operation_id, a.number LIMIT 100",
        )
        .bind(&after.0)
        .bind(after.1)
        .fetch_all(&mut **tx)
        .await?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            let completion = row
                .try_get::<Option<&str>, _>("completion_json")?
                .map(serde_json::from_str::<McpCompletion>)
                .transpose()?;
            let observed = completion
                .as_ref()
                .map(status)
                .unwrap_or(LoopToolStatus::Started);
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO loop_tool_observations(activation_id, generation, surface, tool_name, target_ref, status, started_at, completed_at, operation_id, attempt_number) \
                 VALUES (?, ?, 'mcp_tool', ?, ?, ?, ?, ?, ?, ?) RETURNING id",
            ).bind(row.try_get::<&str, _>("activation_id")?).bind(row.try_get::<i64, _>("generation")?)
                .bind(row.try_get::<&str, _>("tool_name")?).bind(row.try_get::<&str, _>("server_id")?)
                .bind(observed.as_str()).bind(row.try_get::<i64, _>("sent_at")?)
                .bind(row.try_get::<Option<i64>, _>("completed_at")?)
                .bind(row.try_get::<&str, _>("operation_id")?).bind(row.try_get::<i64, _>("number")?)
                .fetch_one(&mut **tx).await?;
            after = (row.try_get("operation_id")?, row.try_get("number")?);
            sqlx::query("UPDATE mcp_operation_attempts SET tool_observation_id = ? WHERE operation_id = ? AND number = ?")
                .bind(id).bind(&after.0).bind(after.1).execute(&mut **tx).await?;
        }
    }
    Ok(())
}
