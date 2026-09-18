//! Durable loop control records. Runtime execution is owned by the daemon service.

mod admission;
mod admission_limits;
mod history;
mod intake;
mod launch;
mod lifecycle;
mod metrics;
mod outcome;
mod policy;
mod reservation;
mod scheduling;
mod scheduling_observation;
mod scheduling_reconcile;
mod scheduling_revocation;
mod schema;
mod scope;
mod tool_observation;
mod work_context;

#[cfg(test)]
mod tests;

use agenthub_agent_domain::loop_runtime::{LoopActivation, LoopEvent, LoopTriggerRecord};
use sqlx::{Row, SqlitePool, sqlite::SqliteRow};
use thiserror::Error;

pub use policy::LoopPolicyUpdate;
pub(crate) use policy::require_member;
pub(crate) use reservation::require_live_reservation;
pub use schema::migrate_loop_runtime;
pub use tool_observation::LoopToolObservation;

#[derive(Debug, Error)]
pub enum LoopStoreError {
    #[error("loop scope does not match current membership")]
    ScopeMismatch,
    #[error("loop policy revision changed")]
    RevisionConflict,
    #[error("loop execution is not enabled")]
    Disabled,
    #[error("loop pending work limit reached")]
    Capacity,
    #[error("loop source key reused with different work")]
    IdempotencyConflict,
    #[error("loop execution reservation is held")]
    ReservationHeld,
    #[error("loop execution lease is stale or expired")]
    StaleLease,
    #[error("loop activation is not in the required lifecycle state")]
    InvalidState,
    #[error("loop scope change requires quiescence: {0}")]
    ScopeBusy(&'static str),
    #[error("invalid loop history query or cursor")]
    InvalidHistoryQuery,
}

#[derive(Clone)]
pub struct LoopStore {
    pool: SqlitePool,
}

impl LoopStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn activation(
        &self,
        team_id: &str,
        id: &str,
    ) -> anyhow::Result<Option<LoopActivation>> {
        sqlx::query("SELECT * FROM loop_activations WHERE team_id = ? AND id = ?")
            .bind(team_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(parse_activation)
            .transpose()
    }

    pub async fn triggers(
        &self,
        team_id: &str,
        activation_id: &str,
    ) -> anyhow::Result<Vec<LoopTriggerRecord>> {
        let rows = sqlx::query(
            "SELECT s.*, EXISTS(SELECT 1 FROM loop_revoked_sources r WHERE r.trigger_id = s.id) AS revoked \
             FROM loop_trigger_sources s WHERE s.team_id = ? AND s.activation_id = ? ORDER BY s.created_at, s.id",
        )
        .bind(team_id)
        .bind(activation_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(parse_trigger).collect()
    }

    pub async fn events(
        &self,
        team_id: &str,
        activation_id: &str,
        after_id: i64,
        limit: u32,
    ) -> anyhow::Result<Vec<LoopEvent>> {
        let rows = sqlx::query(
            "SELECT e.* FROM loop_activation_events e JOIN loop_activations a ON a.id = e.activation_id \
             WHERE a.team_id = ? AND a.id = ? AND e.id > ? ORDER BY e.id LIMIT ?",
        )
        .bind(team_id)
        .bind(activation_id)
        .bind(after_id)
        .bind(limit.clamp(1, 100))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(parse_event).collect()
    }
}

fn parse_event(row: &SqliteRow) -> anyhow::Result<LoopEvent> {
    Ok(LoopEvent {
        id: row.try_get("id")?,
        activation_id: row.try_get("activation_id")?,
        kind: row.try_get::<&str, _>("kind")?.parse()?,
        generation: row.try_get("generation")?,
        trigger_id: row.try_get("trigger_id")?,
        reason: row
            .try_get::<Option<&str>, _>("reason_code")?
            .map(str::parse)
            .transpose()?,
        exit_reason: row
            .try_get::<Option<&str>, _>("exit_reason_code")?
            .map(str::parse)
            .transpose()?,
        created_at: row.try_get("created_at")?,
    })
}

fn parse_activation(row: &SqliteRow) -> anyhow::Result<LoopActivation> {
    Ok(LoopActivation {
        id: row.try_get("id")?,
        actor_id: row.try_get("actor_id")?,
        team_id: row.try_get("team_id")?,
        state: row.try_get::<&str, _>("state")?.parse()?,
        due_at: row.try_get("due_at")?,
        next_admission_at: row.try_get("next_admission_at")?,
        policy_revision: row.try_get("policy_revision")?,
        generation: row.try_get("generation")?,
        attempt_count: row.try_get("attempt_count")?,
        mailbox_run_id: row.try_get("mailbox_run_id")?,
        session_id: row.try_get("session_id")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        finished_at: row.try_get("finished_at")?,
        outcome: row
            .try_get::<Option<&str>, _>("outcome_json")?
            .map(serde_json::from_str)
            .transpose()?,
        launch: row
            .try_get::<Option<&str>, _>("launch_json")?
            .map(serde_json::from_str)
            .transpose()?,
    })
}

fn parse_trigger(row: &SqliteRow) -> anyhow::Result<LoopTriggerRecord> {
    Ok(LoopTriggerRecord {
        id: row.try_get("id")?,
        activation_id: row.try_get("activation_id")?,
        input: serde_json::from_str(row.try_get("input_json")?)?,
        created_at: row.try_get("created_at")?,
        revoked: row.try_get("revoked")?,
    })
}
