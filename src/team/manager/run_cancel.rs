use super::run_task_status_sync::{load_run_status_sync_meta_tx, sync_linked_task_status_tx};
use super::{TeamManager, TeamRunRecord, TeamTaskStatus};
use sqlx::Row;

impl TeamManager {
    pub async fn cancel_run(&self, run_id: &str) -> anyhow::Result<TeamRunRecord> {
        let now = chrono::Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let result = sqlx::query(
            r#"
            UPDATE team_runs
            SET status = 'canceled', ended_at = COALESCE(ended_at, ?1)
            WHERE id = ?2 AND status NOT IN ('completed', 'failed', 'canceled')
            "#,
        )
        .bind(now)
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
        let mut archive_events = Vec::new();

        if result.rows_affected() > 0 {
            let (team_id, run_input) = load_run_status_sync_meta_tx(&mut tx, run_id).await?;
            let active_steps = sqlx::query(
                r#"
                SELECT id, step_key
                FROM team_steps
                WHERE run_id = ?1 AND status NOT IN ('completed', 'failed', 'canceled')
                "#,
            )
            .bind(run_id)
            .fetch_all(&mut *tx)
            .await?;

            for step in active_steps {
                let step_id: String = Row::get(&step, "id");
                let step_key: String = Row::get(&step, "step_key");
                let step_update = sqlx::query(
                    r#"
                    UPDATE team_steps
                    SET status = 'canceled', ended_at = COALESCE(ended_at, ?1)
                    WHERE id = ?2 AND status NOT IN ('completed', 'failed', 'canceled')
                    "#,
                )
                .bind(now)
                .bind(&step_id)
                .execute(&mut *tx)
                .await?;
                if step_update.rows_affected() == 0 {
                    continue;
                }

                let step_payload = serde_json::json!({
                    "step_id": step_id,
                    "step_key": step_key,
                    "status": "canceled",
                });
                let event = Self::append_run_event_tx(
                    &mut tx,
                    run_id,
                    Some(&step_id),
                    "step_canceled",
                    now,
                    &step_payload,
                )
                .await?;
                archive_events.push(event);
            }

            let payload = serde_json::json!({ "status": "canceled" });
            let event =
                Self::append_run_event_tx(&mut tx, run_id, None, "run_canceled", now, &payload)
                    .await?;
            archive_events.push(event);
            sync_linked_task_status_tx(
                &mut tx,
                &team_id,
                &run_input,
                TeamTaskStatus::Canceled,
                now,
                true,
            )
            .await?;
        }
        tx.commit().await?;
        self.spawn_archive_team_run_events(archive_events);

        self.get_run(run_id).await
    }
}
