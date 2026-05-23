use super::step_helpers::{build_step_runtime_handle_event_payload, load_step_record_tx};
use super::step_reconcile::{build_reconcile_round_started_input, extract_reconcile_round_runtime};
use super::{TeamManager, TeamStepRecord};
use chrono::Utc;

impl TeamManager {
    #[allow(dead_code)]
    pub async fn start_step(
        &self,
        step_id: &str,
        runtime_handle_id: Option<&str>,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let current = load_step_record_tx(&mut tx, step_id).await?;
        let reconcile_started = build_reconcile_round_started_input(current.input.as_ref()).map(
            |(next_input, round)| {
                (
                    serde_json::to_string(&next_input)
                        .expect("reconcile round input should serialize"),
                    round,
                )
            },
        );
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                status = 'working',
                remote_task_id = COALESCE(?1, remote_task_id),
                input_json = COALESCE(?2, input_json),
                started_at = COALESCE(started_at, ?3)
            WHERE id = ?4 AND status IN ('submitted', 'input_required')
            "#,
        )
        .bind(runtime_handle_id)
        .bind(
            reconcile_started
                .as_ref()
                .map(|(input_json, _)| input_json.as_str()),
        )
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;
        let step = load_step_record_tx(&mut tx, step_id).await?;
        let mut archive_events = Vec::new();

        if update.rows_affected() > 0 {
            let run_update = sqlx::query(
                r#"
                UPDATE team_runs
                SET status = 'working', started_at = COALESCE(started_at, ?1)
                WHERE id = ?2 AND status IN ('submitted', 'input_required')
                "#,
            )
            .bind(now)
            .bind(&step.run_id)
            .execute(&mut *tx)
            .await?;
            if run_update.rows_affected() > 0 {
                let run_payload = serde_json::json!({
                    "status": "working",
                });
                let event = Self::append_run_event_tx(
                    &mut tx,
                    &step.run_id,
                    None,
                    "run_working",
                    now,
                    &run_payload,
                )
                .await?;
                archive_events.push(event);
            }

            let step_payload = build_step_runtime_handle_event_payload(&step, "working");
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "step_working",
                now,
                &step_payload,
            )
            .await?;
            archive_events.push(event);

            if let Some((_, round)) = reconcile_started.as_ref() {
                let runtime = extract_reconcile_round_runtime(step.input.as_ref());
                let round_payload = serde_json::json!({
                    "step_id": step.id,
                    "step_key": step.step_key,
                    "round": round,
                    "status": "working",
                    "goal": runtime.as_ref().and_then(|item| item.goal.clone()),
                    "acceptance_count": runtime.as_ref().map(|item| item.acceptance.len()).unwrap_or(0),
                    "max_rounds": runtime.and_then(|item| item.execution.max_rounds),
                });
                let event = Self::append_run_event_tx(
                    &mut tx,
                    &step.run_id,
                    Some(&step.id),
                    "step_reconcile_round_started",
                    now,
                    &round_payload,
                )
                .await?;
                archive_events.push(event);
            }
        }

        tx.commit().await?;
        self.spawn_archive_team_run_events(archive_events);
        Ok(step)
    }
}
