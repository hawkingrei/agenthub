use serde_json::Value;
use sqlx::Sqlite;

use super::run_task_status_sync::sync_linked_task_status_tx;
use super::{TeamManager, TeamRunEventRecord, TeamStepRecord, TeamTaskStatus};

impl TeamManager {
    pub(super) async fn finalize_completed_step_run_tx(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        step: &TeamStepRecord,
        team_id: &str,
        run_input: &Value,
        now: i64,
        archive_events: &mut Vec<TeamRunEventRecord>,
    ) -> anyhow::Result<()> {
        let non_completed_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM team_steps
            WHERE run_id = ?1 AND status <> 'completed'
            "#,
        )
        .bind(&step.run_id)
        .fetch_one(&mut **tx)
        .await?;

        if non_completed_count != 0 {
            return Ok(());
        }

        let run_update = sqlx::query(
            r#"
            UPDATE team_runs
            SET status = 'completed', ended_at = COALESCE(ended_at, ?1)
            WHERE id = ?2 AND status IN ('submitted', 'working', 'input_required')
            "#,
        )
        .bind(now)
        .bind(&step.run_id)
        .execute(&mut **tx)
        .await?;

        if run_update.rows_affected() == 0 {
            return Ok(());
        }

        let run_payload = serde_json::json!({
            "status": "completed",
        });
        let event =
            Self::append_run_event_tx(tx, &step.run_id, None, "run_completed", now, &run_payload)
                .await?;
        archive_events.push(event);
        sync_linked_task_status_tx(tx, team_id, run_input, TeamTaskStatus::InReview, now, true)
            .await?;
        Ok(())
    }
}
