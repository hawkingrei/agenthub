use std::collections::VecDeque;

use agenthub_agent_domain::loop_scheduling::LoopRegistration;
use sqlx::{Row, Sqlite, Transaction};

use super::{LoopStore, LoopStoreError, scheduling::parse_registration};

impl LoopStore {
    pub async fn revoke_schedule(
        &self,
        team_id: &str,
        id: &str,
        now: i64,
    ) -> anyhow::Result<LoopRegistration> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let registration = Self::revoke_schedule_tx(&mut tx, team_id, id, now).await?;
        tx.commit().await?;
        Ok(registration)
    }

    pub async fn revoke_schedule_tx(
        tx: &mut Transaction<'_, Sqlite>,
        team_id: &str,
        id: &str,
        now: i64,
    ) -> anyhow::Result<LoopRegistration> {
        anyhow::ensure!(now >= 0, "invalid revocation timestamp");
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM loop_registrations WHERE id = ? AND team_id = ?)",
        )
        .bind(id)
        .bind(team_id)
        .fetch_one(&mut **tx)
        .await?;
        anyhow::ensure!(exists, LoopStoreError::ScopeMismatch);
        revoke_registrations(tx, &[id.to_owned()], now).await?;
        let row = sqlx::query("SELECT * FROM loop_registrations WHERE id = ?")
            .bind(id)
            .fetch_one(&mut **tx)
            .await?;
        parse_registration(&row)
    }

    /// Call before deleting a task and its messages so thread dependencies remain resolvable.
    pub async fn revoke_task_schedules_tx(
        tx: &mut Transaction<'_, Sqlite>,
        team_id: &str,
        task_id: &str,
        now: i64,
    ) -> anyhow::Result<()> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM loop_registrations WHERE team_id = ? AND state != 'revoked' AND \
             (work_task_id = ? OR dependency_task_id = ? OR thread_root_message_id IN \
             (SELECT id FROM team_conversation_messages WHERE task_id = ?))",
        )
        .bind(team_id)
        .bind(task_id)
        .bind(task_id)
        .bind(task_id)
        .fetch_all(&mut **tx)
        .await?;
        revoke_registrations(tx, &ids, now).await
    }
}

pub(super) async fn revoke_origin_registrations(
    tx: &mut Transaction<'_, Sqlite>,
    team_id: &str,
    activation_id: &str,
    now: i64,
) -> anyhow::Result<()> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM loop_registrations WHERE team_id = ? AND origin_activation_id = ? AND state != 'revoked'",
    ).bind(team_id).bind(activation_id).fetch_all(&mut **tx).await?;
    revoke_registrations(tx, &ids, now).await
}

pub(super) async fn revoke_registrations(
    tx: &mut Transaction<'_, Sqlite>,
    ids: &[String],
    now: i64,
) -> anyhow::Result<()> {
    let mut pending: VecDeque<String> = ids.iter().cloned().collect();
    while let Some(id) = pending.pop_front() {
        let changed = sqlx::query(
            "UPDATE loop_registrations SET state = 'revoked', pending_cursor = NULL, pending_due_at = NULL, \
             next_due_at = NULL, updated_at = ? WHERE id = ? AND state != 'revoked'",
        ).bind(now).bind(&id).execute(&mut **tx).await?.rows_affected();
        if changed == 0 {
            continue;
        }
        sqlx::query(
            "INSERT OR IGNORE INTO loop_revoked_sources(trigger_id, created_at) \
             SELECT trigger_id, ? FROM loop_registration_firings WHERE registration_id = ?",
        )
        .bind(now)
        .bind(&id)
        .execute(&mut **tx)
        .await?;
        let canceled = sqlx::query(
            "UPDATE loop_activations SET state = 'canceled', updated_at = ?, finished_at = ? \
             WHERE state NOT IN ('finished', 'canceled') AND id IN \
             (SELECT s.activation_id FROM loop_trigger_sources s JOIN loop_registration_firings f ON f.trigger_id = s.id \
              WHERE f.registration_id = ?) AND NOT EXISTS \
             (SELECT 1 FROM loop_trigger_sources s WHERE s.activation_id = loop_activations.id \
              AND NOT EXISTS(SELECT 1 FROM loop_revoked_sources r WHERE r.trigger_id = s.id)) RETURNING id, generation",
        ).bind(now).bind(now).bind(&id).fetch_all(&mut **tx).await?;
        // Canceled generations reject actor tools immediately; reservations stay until cleanup.
        for row in canceled {
            let activation_id: String = row.try_get("id")?;
            sqlx::query(
                "INSERT INTO loop_activation_events(activation_id, kind, generation, created_at) VALUES (?, 'canceled', ?, ?)",
            ).bind(&activation_id).bind(row.try_get::<i64, _>("generation")?).bind(now).execute(&mut **tx).await?;
            let descendants: Vec<String> = sqlx::query_scalar(
                "SELECT id FROM loop_registrations WHERE origin_activation_id = ? AND state != 'revoked'",
            ).bind(&activation_id).fetch_all(&mut **tx).await?;
            pending.extend(descendants);
        }
    }
    Ok(())
}
