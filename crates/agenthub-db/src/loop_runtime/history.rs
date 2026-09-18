use agenthub_agent_domain::loop_history::{
    LoopEventHistoryPage, LoopHistoryPage, LoopSourceHistoryPage, LoopSourceSummary,
};
use agenthub_agent_domain::loop_runtime::validate_loop_id;
use sqlx::{Sqlite, Transaction};

use super::{LoopStore, LoopStoreError, parse_activation, parse_event, parse_trigger};

impl LoopStore {
    /// Caller authorization is separate; every query additionally binds both durable scope IDs.
    pub async fn activation_history(
        &self,
        team_id: &str,
        actor_id: &str,
        before: Option<&str>,
        limit: u32,
    ) -> anyhow::Result<LoopHistoryPage> {
        validate_scope(team_id, actor_id, limit)?;
        let mut tx = self.pool.begin().await?;
        let cursor = if let Some(before) = before {
            history_id(before)?;
            Some(sqlx::query_as::<_, (i64, String)>(
                "SELECT created_at, id FROM loop_activations WHERE team_id = ? AND actor_id = ? AND id = ?",
            ).bind(team_id).bind(actor_id).bind(before).fetch_optional(&mut *tx).await?
                .ok_or(LoopStoreError::InvalidHistoryQuery)?)
        } else {
            None
        };
        let rows = sqlx::query(
            "SELECT * FROM loop_activations WHERE team_id = ? AND actor_id = ? \
             AND (? IS NULL OR (created_at, id) < (?, ?)) \
             ORDER BY created_at DESC, id DESC LIMIT ?",
        )
        .bind(team_id)
        .bind(actor_id)
        .bind(cursor.as_ref().map(|value| value.0))
        .bind(cursor.as_ref().map(|value| value.0))
        .bind(cursor.as_ref().map(|value| value.1.as_str()))
        .bind(i64::from(limit) + 1)
        .fetch_all(&mut *tx)
        .await?;
        let activations = rows
            .iter()
            .take(limit as usize)
            .map(parse_activation)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let next_cursor = (rows.len() > limit as usize)
            .then(|| activations.last().expect("positive page size").id.clone());
        tx.commit().await?;
        Ok(LoopHistoryPage {
            activations,
            next_cursor,
        })
    }

    pub async fn activation_source_history(
        &self,
        team_id: &str,
        actor_id: &str,
        activation_id: &str,
        after: Option<&str>,
        limit: u32,
    ) -> anyhow::Result<Option<LoopSourceHistoryPage>> {
        validate_scope(team_id, actor_id, limit)?;
        history_id(activation_id)?;
        let mut tx = self.pool.begin().await?;
        if !contains_activation(&mut tx, team_id, actor_id, activation_id).await? {
            return Ok(None);
        }
        if let Some(after) = after {
            history_id(after)?;
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM loop_trigger_sources WHERE activation_id = ? AND id = ?)",
            ).bind(activation_id).bind(after).fetch_one(&mut *tx).await?;
            anyhow::ensure!(valid, LoopStoreError::InvalidHistoryQuery);
        }
        let rows = sqlx::query(
            "SELECT s.*, EXISTS(SELECT 1 FROM loop_revoked_sources r WHERE r.trigger_id = s.id) AS revoked \
             FROM loop_trigger_sources s WHERE s.activation_id = ? \
             AND (? IS NULL OR s.id > ?) ORDER BY s.id LIMIT ?",
        ).bind(activation_id).bind(after).bind(after).bind(i64::from(limit) + 1)
            .fetch_all(&mut *tx).await?;
        let sources = rows
            .iter()
            .take(limit as usize)
            .map(|row| {
                let source = parse_trigger(row)?;
                Ok(LoopSourceSummary {
                    id: source.id,
                    kind: source.input.kind,
                    references: source.input.references,
                    due_at: source.input.due_at,
                    created_at: source.created_at,
                    revoked: source.revoked,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let next_cursor = (rows.len() > limit as usize)
            .then(|| sources.last().expect("positive page size").id.clone());
        tx.commit().await?;
        Ok(Some(LoopSourceHistoryPage {
            sources,
            next_cursor,
        }))
    }

    pub async fn activation_event_history(
        &self,
        team_id: &str,
        actor_id: &str,
        activation_id: &str,
        after: Option<i64>,
        limit: u32,
    ) -> anyhow::Result<Option<LoopEventHistoryPage>> {
        validate_scope(team_id, actor_id, limit)?;
        history_id(activation_id)?;
        anyhow::ensure!(
            after.is_none_or(|id| id > 0),
            LoopStoreError::InvalidHistoryQuery
        );
        let mut tx = self.pool.begin().await?;
        if !contains_activation(&mut tx, team_id, actor_id, activation_id).await? {
            return Ok(None);
        }
        if let Some(after) = after {
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM loop_activation_events WHERE activation_id = ? AND id = ?)",
            ).bind(activation_id).bind(after).fetch_one(&mut *tx).await?;
            anyhow::ensure!(valid, LoopStoreError::InvalidHistoryQuery);
        }
        let rows = sqlx::query(
            "SELECT * FROM loop_activation_events WHERE activation_id = ? AND id > ? ORDER BY id LIMIT ?",
        ).bind(activation_id).bind(after.unwrap_or(0)).bind(i64::from(limit) + 1)
            .fetch_all(&mut *tx).await?;
        let events = rows
            .iter()
            .take(limit as usize)
            .map(parse_event)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let next_cursor =
            (rows.len() > limit as usize).then(|| events.last().expect("positive page size").id);
        tx.commit().await?;
        Ok(Some(LoopEventHistoryPage {
            events,
            next_cursor,
        }))
    }
}

pub(super) fn validate_scope(team_id: &str, actor_id: &str, limit: u32) -> anyhow::Result<()> {
    history_id(team_id)?;
    history_id(actor_id)?;
    anyhow::ensure!(
        (1..=100).contains(&limit),
        LoopStoreError::InvalidHistoryQuery
    );
    Ok(())
}

pub(super) fn history_id(value: &str) -> anyhow::Result<()> {
    validate_loop_id(value).map_err(|_| LoopStoreError::InvalidHistoryQuery.into())
}

pub(super) async fn contains_activation(
    tx: &mut Transaction<'_, Sqlite>,
    team_id: &str,
    actor_id: &str,
    activation_id: &str,
) -> anyhow::Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM loop_activations WHERE team_id = ? AND actor_id = ? AND id = ?)",
    ).bind(team_id).bind(actor_id).bind(activation_id).fetch_one(&mut **tx).await?)
}
