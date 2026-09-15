//! Durable MCP send boundaries. Only trusted daemon code may construct intents or send permits.

mod attempts;
mod continuation;
mod prepare;
mod schema;
mod task;

#[cfg(test)]
mod tests;

use agenthub_agent_domain::mcp_operations::{
    McpAttemptRecord, McpCompletion, McpContinuationRecord, McpOperationEvent, McpOperationRecord,
    McpOperationStatus,
};
use sqlx::{Row, Sqlite, SqlitePool, Transaction, pool::PoolConnection, sqlite::SqliteRow};
use thiserror::Error;

use crate::DaemonGeneration;

pub use schema::migrate_mcp_operations;
pub use task::McpTaskLookupPermit;

#[derive(Debug, Error)]
pub enum McpJournalError {
    #[error("MCP operation identity was reused with different intent")]
    IdentityConflict,
    #[error("MCP operation is already in flight")]
    InFlight,
    #[error("MCP operation has a recorded successful outcome")]
    AlreadyCompleted,
    #[error("MCP operation may have taken effect; replay requires the original stable identity")]
    UnsafeReplay,
    #[error("MCP operation requires a linked continuation or task result, not a replay")]
    ContinuationRequired,
    #[error("MCP send attempt is stale")]
    StaleAttempt,
    #[error("MCP journal daemon generation is stale")]
    StaleDaemon,
    #[error("MCP operation does not belong to this executor")]
    ScopeMismatch,
}

#[derive(Clone)]
pub struct McpOperationStore {
    pool: SqlitePool,
    daemon: DaemonGeneration,
}

/// A process-local capability returned exactly once after a durable sent transition.
/// It must never be serialized into provider state, logs, or RPC responses.
pub struct McpSendPermit {
    operation_id: String,
    attempt_number: u32,
    permit_id: String,
}

impl McpSendPermit {
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn attempt_number(&self) -> u32 {
        self.attempt_number
    }
}

impl McpOperationStore {
    pub fn new(pool: SqlitePool, daemon: DaemonGeneration) -> Self {
        Self { pool, daemon }
    }

    async fn durable_connection(&self) -> anyhow::Result<PoolConnection<Sqlite>> {
        let mut connection = self.pool.acquire().await?;
        // WAL/NORMAL can lose an acknowledged commit on power loss. A send boundary must be
        // flushed before its external effect. Keep the stronger setting when returning this
        // connection to the pool; cancellation must not leave an unsafe reset path.
        sqlx::query("PRAGMA synchronous = FULL")
            .execute(&mut *connection)
            .await?;
        Ok(connection)
    }

    pub async fn operation(
        &self,
        team_id: &str,
        actor_id: &str,
        id: &str,
    ) -> anyhow::Result<Option<McpOperationRecord>> {
        sqlx::query("SELECT * FROM mcp_operations WHERE team_id = ? AND actor_id = ? AND id = ?")
            .bind(team_id)
            .bind(actor_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(parse_operation)
            .transpose()
    }

    pub async fn attempts(
        &self,
        team_id: &str,
        actor_id: &str,
        id: &str,
        after: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<McpAttemptRecord>> {
        let rows = sqlx::query("SELECT a.*, c.parent_attempt_number, c.parent_response_digest, \
            COALESCE(r.request_id_digest, c.request_id_digest) AS request_id_digest, c.request_digest, \
            r.continuation_attempt_number AS retry_of_attempt_number \
            FROM mcp_operation_attempts a JOIN mcp_operations o ON o.id = a.operation_id \
            LEFT JOIN mcp_operation_continuation_retries r ON r.operation_id = a.operation_id AND r.attempt_number = a.number \
            LEFT JOIN mcp_operation_continuations c ON c.operation_id = a.operation_id \
                AND c.attempt_number = COALESCE(r.continuation_attempt_number, a.number) \
            WHERE o.team_id = ? AND o.actor_id = ? AND o.id = ? AND a.number > ? ORDER BY a.number LIMIT ?")
            .bind(team_id).bind(actor_id).bind(id).bind(after).bind(limit.clamp(1, 100))
            .fetch_all(&self.pool).await?;
        rows.iter()
            .map(|row| {
                Ok(McpAttemptRecord {
                    operation_id: row.try_get("operation_id")?,
                    number: row.try_get("number")?,
                    activation_id: row.try_get("activation_id")?,
                    generation: row.try_get("generation")?,
                    status: row.try_get::<&str, _>("status")?.parse()?,
                    completion: row
                        .try_get::<Option<&str>, _>("completion_json")?
                        .map(serde_json::from_str)
                        .transpose()?,
                    sent_at: row.try_get("sent_at")?,
                    completed_at: row.try_get("completed_at")?,
                    continuation: row
                        .try_get::<Option<u32>, _>("parent_attempt_number")?
                        .map(|parent_attempt_number| -> anyhow::Result<_> {
                            Ok(McpContinuationRecord {
                                parent_attempt_number,
                                parent_response_digest: row
                                    .try_get::<String, _>("parent_response_digest")?
                                    .try_into()?,
                                request_id_digest: row
                                    .try_get::<String, _>("request_id_digest")?
                                    .try_into()?,
                                request_digest: row
                                    .try_get::<String, _>("request_digest")?
                                    .try_into()?,
                                retry_of_attempt_number: row.try_get("retry_of_attempt_number")?,
                            })
                        })
                        .transpose()?,
                })
            })
            .collect()
    }

    pub async fn events(
        &self,
        team_id: &str,
        actor_id: &str,
        activation_id: &str,
        after: i64,
        limit: u32,
    ) -> anyhow::Result<Vec<McpOperationEvent>> {
        let rows = sqlx::query("SELECT e.* FROM mcp_operation_events e JOIN mcp_operations o ON o.id = e.operation_id \
            WHERE o.team_id = ? AND o.actor_id = ? AND e.activation_id = ? AND e.id > ? ORDER BY e.id LIMIT ?")
            .bind(team_id).bind(actor_id).bind(activation_id).bind(after.max(0)).bind(limit.clamp(1, 100))
            .fetch_all(&self.pool).await?;
        rows.iter()
            .map(|row| {
                Ok(McpOperationEvent {
                    id: row.try_get("id")?,
                    operation_id: row.try_get("operation_id")?,
                    attempt_number: row.try_get("attempt_number")?,
                    activation_id: row.try_get("activation_id")?,
                    status: row.try_get::<&str, _>("status")?.parse()?,
                    completion: row
                        .try_get::<Option<&str>, _>("completion_json")?
                        .map(serde_json::from_str)
                        .transpose()?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect()
    }

    async fn require_current_daemon(&self, tx: &mut Transaction<'_, Sqlite>) -> anyhow::Result<()> {
        let current: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM daemon_generations \
            WHERE node_id = ? AND generation = ? AND owner_id = ?)",
        )
        .bind(&self.daemon.node_id)
        .bind(self.daemon.generation)
        .bind(&self.daemon.owner_id)
        .fetch_one(&mut **tx)
        .await?;
        anyhow::ensure!(current, McpJournalError::StaleDaemon);
        Ok(())
    }
}

fn parse_operation(row: &SqliteRow) -> anyhow::Result<McpOperationRecord> {
    Ok(McpOperationRecord {
        id: row.try_get("id")?,
        actor_id: row.try_get("actor_id")?,
        team_id: row.try_get("team_id")?,
        origin_activation_id: row.try_get("origin_activation_id")?,
        intent: serde_json::from_str(row.try_get("intent_json")?)?,
        status: row.try_get::<&str, _>("status")?.parse()?,
        attempt_count: row.try_get("attempt_count")?,
        completion: row
            .try_get::<Option<&str>, _>("completion_json")?
            .map(serde_json::from_str)
            .transpose()?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

async fn record_event(
    tx: &mut Transaction<'_, Sqlite>,
    operation_id: &str,
    attempt: u32,
    activation_id: &str,
    status: McpOperationStatus,
    completion: Option<&McpCompletion>,
    now: i64,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO mcp_operation_events(operation_id, attempt_number, activation_id, status, completion_json, created_at) \
        VALUES (?, ?, ?, ?, ?, ?)")
        .bind(operation_id).bind(attempt).bind(activation_id).bind(status.as_str())
        .bind(completion.map(serde_json::to_string).transpose()?).bind(now)
        .execute(&mut **tx).await?;
    Ok(())
}
