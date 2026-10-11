//! Standalone native execution ownership, separate from provider conversation identity.

use agenthub_agent_domain::loop_runtime::validate_loop_id;
use sqlx::{Row, Sqlite, SqlitePool, Transaction, sqlite::SqliteRow};
use thiserror::Error;

mod binding;
mod owners;
mod schema;

pub(crate) use schema::migrate_in_transaction;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExecutionOwner {
    pub agent_id: String,
    pub local_session_id: String,
    pub owner_id: String,
    pub generation: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeExecutionState {
    Reserved,
    Guarded,
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExecutionRecord {
    pub owner: NativeExecutionOwner,
    pub state: NativeExecutionState,
}

#[derive(Debug, Error)]
pub enum NativeSessionError {
    #[error("native execution requires a local standalone actor")]
    ScopeMismatch,
    #[error("native execution owner changed")]
    StaleOwner,
    #[error("native execution still requires verified cleanup")]
    ReservationHeld,
    #[error("native execution is not in the required lifecycle state")]
    InvalidState,
    #[error("native conversation configuration changed; explicit fresh policy is required")]
    ConfigurationChanged,
    #[error("native conversation opening is uncertain and requires reconciliation")]
    OpeningUncertain,
}

#[derive(Clone)]
pub struct NativeSessionStore {
    pool: SqlitePool,
}

impl NativeSessionStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    async fn durable_connection(&self) -> anyhow::Result<sqlx::pool::PoolConnection<Sqlite>> {
        let mut connection = self.pool.acquire().await?;
        sqlx::query("PRAGMA synchronous = FULL")
            .execute(&mut *connection)
            .await?;
        Ok(connection)
    }
}

impl NativeExecutionOwner {
    fn validate(&self) -> anyhow::Result<()> {
        for id in [&self.agent_id, &self.local_session_id, &self.owner_id] {
            validate_loop_id(id)?;
        }
        anyhow::ensure!(self.generation > 0, NativeSessionError::StaleOwner);
        Ok(())
    }
}

fn parse_record(row: &SqliteRow) -> anyhow::Result<NativeExecutionRecord> {
    let state = match row.try_get::<&str, _>("state")? {
        "reserved" => NativeExecutionState::Reserved,
        "guarded" => NativeExecutionState::Guarded,
        "retired" => NativeExecutionState::Retired,
        _ => anyhow::bail!(NativeSessionError::InvalidState),
    };
    Ok(NativeExecutionRecord {
        owner: NativeExecutionOwner {
            agent_id: row.try_get("agent_id")?,
            local_session_id: row.try_get("local_session_id")?,
            owner_id: row.try_get("owner_id")?,
            generation: row.try_get("generation")?,
        },
        state,
    })
}

async fn require_standalone(tx: &mut Transaction<'_, Sqlite>, agent: &str) -> anyhow::Result<()> {
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agents a WHERE a.id = ? AND a.command = 'rara' \
         AND a.target_node_id IS NULL AND NOT EXISTS(SELECT 1 FROM loop_policies p WHERE p.actor_id = a.id) \
         AND NOT EXISTS(SELECT 1 FROM team_definitions t, json_each(t.spec_json, '$.members') m \
                        WHERE json_extract(m.value, '$.member_id') = a.id))",
    ).bind(agent).fetch_one(&mut **tx).await?;
    anyhow::ensure!(valid, NativeSessionError::ScopeMismatch);
    Ok(())
}

async fn require_owner(
    tx: &mut Transaction<'_, Sqlite>,
    expected: &NativeExecutionOwner,
) -> anyhow::Result<NativeExecutionState> {
    expected.validate()?;
    let row = sqlx::query(
        "SELECT * FROM native_execution_owners WHERE agent_id = ? AND generation = ? \
         AND local_session_id = ? AND owner_id = ?",
    )
    .bind(&expected.agent_id)
    .bind(expected.generation)
    .bind(&expected.local_session_id)
    .bind(&expected.owner_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(NativeSessionError::StaleOwner)?;
    Ok(parse_record(&row)?.state)
}

async fn require_guarded(
    tx: &mut Transaction<'_, Sqlite>,
    expected: &NativeExecutionOwner,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        require_owner(tx, expected).await? == NativeExecutionState::Guarded,
        NativeSessionError::InvalidState
    );
    require_standalone(tx, &expected.agent_id).await?;
    let session: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE id = ? AND agent_id = ? AND ended_at IS NULL)",
    ).bind(&expected.local_session_id).bind(&expected.agent_id).fetch_one(&mut **tx).await?;
    anyhow::ensure!(session, NativeSessionError::StaleOwner);
    Ok(())
}

pub(crate) async fn has_held_owner(
    tx: &mut Transaction<'_, Sqlite>,
    agent: &str,
) -> anyhow::Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM native_execution_owners WHERE agent_id = ? AND state != 'retired')",
    ).bind(agent).fetch_one(&mut **tx).await?)
}
