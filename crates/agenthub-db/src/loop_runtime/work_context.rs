use agenthub_agent_domain::loop_runtime::{
    LoopReservation, LoopTriggerRecord, LoopWorkPage, validate_loop_id,
};

use super::{
    LoopStore, LoopStoreError, parse_activation, parse_trigger, policy::require_member,
    reservation::require_live_reservation,
};

impl LoopStore {
    pub async fn work_source(
        &self,
        expected: &LoopReservation,
        source_id: &str,
        now: i64,
    ) -> anyhow::Result<LoopTriggerRecord> {
        validate_loop_id(source_id)?;
        let mut tx = self.pool.begin().await?;
        let current = require_live_reservation(&mut tx, expected, now).await?;
        require_member(&mut tx, &current.team_id, &current.actor_id).await?;
        let row = sqlx::query("SELECT s.*, EXISTS(SELECT 1 FROM loop_revoked_sources r WHERE r.trigger_id = s.id) AS revoked \
            FROM loop_trigger_sources s WHERE s.id = ? AND s.activation_id = ? AND s.actor_id = ? AND s.team_id = ?")
            .bind(source_id).bind(&current.activation_id).bind(&current.actor_id).bind(&current.team_id)
            .fetch_optional(&mut *tx).await?.ok_or(LoopStoreError::ScopeMismatch)?;
        let source = parse_trigger(&row)?;
        tx.commit().await?;
        Ok(source)
    }

    /// Read only the live caller's activation, with a bounded source page and stable ID cursor.
    pub async fn work_context(
        &self,
        expected: &LoopReservation,
        after: Option<&str>,
        limit: u32,
        now: i64,
    ) -> anyhow::Result<LoopWorkPage> {
        if let Some(cursor) = after {
            validate_loop_id(cursor)?;
        }
        let limit = limit.clamp(1, 256) as usize;
        let mut tx = self.pool.begin().await?;
        let current = require_live_reservation(&mut tx, expected, now).await?;
        require_member(&mut tx, &current.team_id, &current.actor_id).await?;
        let activation_id = current
            .activation_id
            .as_deref()
            .ok_or(LoopStoreError::InvalidState)?;
        let activation = sqlx::query(
            "SELECT * FROM loop_activations WHERE id = ? AND actor_id = ? AND team_id = ?",
        )
        .bind(activation_id)
        .bind(&current.actor_id)
        .bind(&current.team_id)
        .fetch_one(&mut *tx)
        .await?;
        let activation = parse_activation(&activation)?;
        let rows = sqlx::query(
            "SELECT s.*, EXISTS(SELECT 1 FROM loop_revoked_sources r WHERE r.trigger_id = s.id) AS revoked \
             FROM loop_trigger_sources s WHERE s.activation_id = ? AND s.actor_id = ? AND s.team_id = ? \
             AND (? IS NULL OR s.id > ?) ORDER BY s.id LIMIT ?",
        ).bind(activation_id).bind(&current.actor_id).bind(&current.team_id).bind(after).bind(after)
            .bind((limit + 1) as i64).fetch_all(&mut *tx).await?;
        let has_more = rows.len() > limit;
        let sources: Vec<LoopTriggerRecord> = rows
            .iter()
            .take(limit)
            .map(parse_trigger)
            .collect::<anyhow::Result<_>>()?;
        let next_cursor = has_more.then(|| sources.last().expect("positive page size").id.clone());
        tx.commit().await?;
        Ok(LoopWorkPage {
            activation,
            sources,
            next_cursor,
        })
    }
}
