use agenthub_agent_domain::loop_scheduling::{LoopRegistrationInput, LoopSchedule};
use sqlx::{Row, Sqlite, Transaction};

use super::{LoopStore, LoopStoreError, scheduling::parse_registration};

pub(super) async fn initial_observation(
    tx: &mut Transaction<'_, Sqlite>,
    input: &LoopRegistrationInput,
) -> anyhow::Result<(i64, bool, Option<i64>)> {
    match &input.schedule {
        LoopSchedule::TaskStatus {
            task_id, statuses, ..
        } => {
            let row = sqlx::query(
                "SELECT status, updated_at FROM team_tasks WHERE team_id = ? AND id = ?",
            )
            .bind(&input.team_id)
            .bind(task_id)
            .fetch_one(&mut **tx)
            .await?;
            let cursor: i64 = row.try_get("updated_at")?;
            let matches = statuses
                .iter()
                .any(|status| status.as_str() == row.get::<&str, _>("status"));
            Ok((cursor, matches, matches.then_some(cursor)))
        }
        LoopSchedule::ThreadReply {
            root_message_id,
            after_message_id,
            ..
        } => {
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM team_conversation_messages root \
                 JOIN team_conversations c ON c.id = root.conversation_id \
                 JOIN team_conversation_messages seen ON seen.conversation_id = c.id \
                 WHERE c.team_id = ? AND root.id = ? AND seen.id = ? \
                 AND COALESCE(root.thread_root_message_id, json_extract(root.payload_json, '$.thread_root_message_id'), root.id) = root.id \
                 AND (seen.id = root.id OR COALESCE(seen.thread_root_message_id, json_extract(seen.payload_json, '$.thread_root_message_id')) = root.id))",
            ).bind(&input.team_id).bind(root_message_id).bind(after_message_id)
                .fetch_one(&mut **tx).await?;
            anyhow::ensure!(valid, LoopStoreError::ScopeMismatch);
            let row = sqlx::query(
                "SELECT MIN(id) AS first, MAX(id) AS last FROM team_conversation_messages \
                 WHERE conversation_id = (SELECT conversation_id FROM team_conversation_messages WHERE id = ?) \
                 AND COALESCE(thread_root_message_id, json_extract(payload_json, '$.thread_root_message_id')) = ? \
                 AND id > ? AND from_actor_id != ?",
            ).bind(root_message_id).bind(root_message_id).bind(after_message_id).bind(&input.actor_id)
                .fetch_one(&mut **tx).await?;
            Ok((
                row.try_get::<Option<i64>, _>("last")?
                    .unwrap_or(*after_message_id),
                false,
                row.try_get("first")?,
            ))
        }
        _ => Ok((0, false, None)),
    }
}

impl LoopStore {
    /// Every runtime task status writer calls this before committing the canonical row.
    pub async fn observe_task_schedule_tx(
        tx: &mut Transaction<'_, Sqlite>,
        team_id: &str,
        task_id: &str,
        now: i64,
    ) -> anyhow::Result<()> {
        let row =
            sqlx::query("SELECT status, updated_at FROM team_tasks WHERE team_id = ? AND id = ?")
                .bind(team_id)
                .bind(task_id)
                .fetch_optional(&mut **tx)
                .await?;
        let Some(task) = row else {
            return Self::revoke_task_schedules_tx(tx, team_id, task_id, now).await;
        };
        let status: &str = task.try_get("status")?;
        let cursor: i64 = task.try_get("updated_at")?;
        if matches!(status, "completed" | "canceled") {
            let ids: Vec<String> = sqlx::query_scalar(
                "SELECT id FROM loop_registrations WHERE team_id = ? AND work_task_id = ? AND state != 'revoked'",
            ).bind(team_id).bind(task_id).fetch_all(&mut **tx).await?;
            super::scheduling_revocation::revoke_registrations(tx, &ids, now).await?;
        }
        let rows = sqlx::query(
            "SELECT * FROM loop_registrations WHERE team_id = ? AND dependency_task_id = ? AND state = 'active'",
        ).bind(team_id).bind(task_id).fetch_all(&mut **tx).await?;
        for row in rows {
            let registration = parse_registration(&row)?;
            if cursor <= registration.observed_cursor {
                continue;
            }
            let LoopSchedule::TaskStatus { statuses, .. } = &registration.input.schedule else {
                continue;
            };
            let matches = statuses
                .iter()
                .any(|candidate| candidate.as_str() == status);
            let fire = matches && !row.try_get::<bool, _>("condition_matches")?;
            sqlx::query(
                "UPDATE loop_registrations SET observed_cursor = ?, condition_matches = ?, \
                 next_check_at = CASE WHEN ? AND pending_cursor IS NULL THEN ? ELSE next_check_at END, \
                 pending_cursor = CASE WHEN ? THEN COALESCE(pending_cursor, ?) ELSE pending_cursor END, \
                 pending_due_at = CASE WHEN ? THEN COALESCE(pending_due_at, ?) ELSE pending_due_at END, \
                 updated_at = ? WHERE id = ?",
            ).bind(cursor).bind(matches).bind(fire).bind(now).bind(fire).bind(cursor)
                .bind(fire).bind(now).bind(now).bind(&registration.id).execute(&mut **tx).await?;
        }
        Ok(())
    }

    /// The message must have been inserted in this transaction; delivery replicas never call this.
    pub async fn observe_thread_schedule_tx(
        tx: &mut Transaction<'_, Sqlite>,
        team_id: &str,
        message_id: i64,
        now: i64,
    ) -> anyhow::Result<()> {
        let row = sqlx::query(
            "SELECT m.conversation_id, m.from_actor_id, \
             COALESCE(m.thread_root_message_id, json_extract(m.payload_json, '$.thread_root_message_id')) AS root_id \
             FROM team_conversation_messages m JOIN team_conversations c ON c.id = m.conversation_id \
             WHERE m.id = ? AND c.team_id = ?",
        ).bind(message_id).bind(team_id).fetch_optional(&mut **tx).await?;
        let Some(row) = row else {
            return Ok(());
        };
        let Some(root_id) = row.try_get::<Option<i64>, _>("root_id")? else {
            return Ok(());
        };
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM team_conversation_messages WHERE id = ? AND conversation_id = ?)",
        ).bind(root_id).bind(row.try_get::<&str, _>("conversation_id")?).fetch_one(&mut **tx).await?;
        if !valid {
            return Ok(());
        }
        sqlx::query(
            "UPDATE loop_registrations SET observed_cursor = ?, \
             next_check_at = CASE WHEN pending_cursor IS NULL THEN ? ELSE next_check_at END, \
             pending_cursor = COALESCE(pending_cursor, ?), pending_due_at = COALESCE(pending_due_at, ?), \
             updated_at = ? WHERE team_id = ? AND thread_root_message_id = ? AND state = 'active' \
             AND observed_cursor < ? AND actor_id != ?",
        ).bind(message_id).bind(now).bind(message_id).bind(now).bind(now).bind(team_id).bind(root_id)
            .bind(message_id).bind(row.try_get::<&str, _>("from_actor_id")?).execute(&mut **tx).await?;
        Ok(())
    }
}
