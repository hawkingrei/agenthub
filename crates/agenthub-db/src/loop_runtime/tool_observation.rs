use std::time::Instant;

use agenthub_agent_domain::{
    loop_history::{LoopToolHistoryPage, LoopToolSummary},
    loop_runtime::{LoopReservation, LoopToolStatus, LoopToolSurface, validate_loop_id},
};
use sqlx::{Row, sqlite::SqliteRow};

use super::{
    LoopStore, LoopStoreError,
    history::{contains_activation, history_id, validate_scope},
    reservation::require_live_reservation,
};

/// A daemon-owned observation handle. Dropping it leaves an explicitly incomplete boundary.
pub struct LoopToolObservation {
    id: i64,
    activation_id: String,
    generation: i64,
    started: Instant,
}

impl LoopStore {
    /// Accept only names from the controller or its approved tool catalog. Request arguments,
    /// result bodies, error text, and caller-selected target descriptions do not belong here.
    pub async fn begin_tool_observation(
        &self,
        expected: &LoopReservation,
        surface: LoopToolSurface,
        tool_name: &str,
        target_ref: Option<&str>,
        now: i64,
    ) -> anyhow::Result<LoopToolObservation> {
        anyhow::ensure!(
            now >= 0
                && !tool_name.is_empty()
                && tool_name.len() <= 4096
                && !tool_name.chars().any(char::is_control),
            "invalid tool observation metadata"
        );
        if let Some(target) = target_ref {
            validate_loop_id(target)?;
        }
        let started = Instant::now();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = require_live_reservation(&mut tx, expected, now).await?;
        let activation_id = current.activation_id.ok_or(LoopStoreError::InvalidState)?;
        let id = sqlx::query_scalar(
            "INSERT INTO loop_tool_observations(activation_id, generation, surface, tool_name, \
             target_ref, status, started_at) VALUES (?, ?, ?, ?, ?, 'started', ?) RETURNING id",
        )
        .bind(&activation_id)
        .bind(current.generation)
        .bind(surface.as_str())
        .bind(tool_name)
        .bind(target_ref)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(LoopToolObservation {
            id,
            activation_id,
            generation: current.generation,
            started,
        })
    }

    /// Completion may arrive after cleanup or lease expiry. This updates only the already-issued
    /// observation, never execution authority, task state, or a newer generation's records.
    pub async fn complete_tool_observation(
        &self,
        observation: LoopToolObservation,
        status: LoopToolStatus,
        now: i64,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            now >= 0 && status != LoopToolStatus::Started,
            "invalid tool completion"
        );
        let duration_ms =
            i64::try_from(observation.started.elapsed().as_millis()).unwrap_or(i64::MAX);
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let changed = sqlx::query(
            "UPDATE loop_tool_observations SET status = ?, completed_at = ?, duration_ms = ? \
             WHERE id = ? AND activation_id = ? AND generation = ? AND status = 'started'",
        )
        .bind(status.as_str())
        .bind(now)
        .bind(duration_ms)
        .bind(observation.id)
        .bind(&observation.activation_id)
        .bind(observation.generation)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        anyhow::ensure!(changed == 1, LoopStoreError::InvalidState);
        sqlx::query("INSERT INTO loop_activation_events(activation_id, kind, generation, created_at) VALUES (?, 'tool_completed', ?, ?)")
            .bind(&observation.activation_id).bind(observation.generation).bind(now)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn activation_tool_history(
        &self,
        team_id: &str,
        actor_id: &str,
        activation_id: &str,
        after: Option<i64>,
        limit: u32,
    ) -> anyhow::Result<Option<LoopToolHistoryPage>> {
        validate_scope(team_id, actor_id, limit)?;
        history_id(activation_id)?;
        let mut tx = self.pool.begin().await?;
        if !contains_activation(&mut tx, team_id, actor_id, activation_id).await? {
            return Ok(None);
        }
        if let Some(cursor) = after {
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM loop_tool_observations WHERE activation_id = ? AND id = ?)",
            ).bind(activation_id).bind(cursor).fetch_one(&mut *tx).await?;
            anyhow::ensure!(valid, LoopStoreError::InvalidHistoryQuery);
        }
        let rows = sqlx::query(
            "SELECT t.id, t.activation_id, t.generation, t.surface, t.tool_name, t.target_ref, t.status, \
             t.started_at, t.completed_at, t.duration_ms, t.operation_id, t.attempt_number \
             FROM loop_tool_observations t \
             WHERE t.activation_id = ? AND (? IS NULL OR t.id > ?) ORDER BY t.id LIMIT ?",
        )
        .bind(activation_id)
        .bind(after)
        .bind(after)
        .bind(i64::from(limit) + 1)
        .fetch_all(&mut *tx)
        .await?;
        let tools = rows
            .iter()
            .take(limit as usize)
            .map(parse_tool)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let next_cursor =
            (rows.len() > limit as usize).then(|| tools.last().expect("positive page size").id);
        tx.commit().await?;
        Ok(Some(LoopToolHistoryPage { tools, next_cursor }))
    }
}

fn parse_tool(row: &SqliteRow) -> anyhow::Result<LoopToolSummary> {
    Ok(LoopToolSummary {
        id: row.try_get("id")?,
        activation_id: row.try_get("activation_id")?,
        generation: row.try_get("generation")?,
        surface: row.try_get::<&str, _>("surface")?.parse()?,
        tool_name: row.try_get("tool_name")?,
        target_ref: row.try_get("target_ref")?,
        operation_id: row.try_get("operation_id")?,
        attempt_number: row.try_get("attempt_number")?,
        status: row.try_get::<&str, _>("status")?.parse()?,
        started_at: row.try_get("started_at")?,
        completed_at: row.try_get("completed_at")?,
        duration_ms: row.try_get("duration_ms")?,
    })
}
