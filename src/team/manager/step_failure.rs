use super::step_helpers::load_step_record_tx;
use super::step_reconcile::{ReconcileRoundArtifactSnapshot, build_reconcile_round_finished_input};
use super::{TeamManager, TeamStepRecord, maybe_attach_context_artifact_pointer};
use chrono::Utc;

impl TeamManager {
    #[allow(dead_code)]
    pub async fn fail_step(
        &self,
        step_id: &str,
        error_text: &str,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let current = load_step_record_tx(&mut tx, step_id).await?;
        let reconcile_finished = build_reconcile_round_finished_input(
            current.input.as_ref(),
            "failed",
            Some(error_text),
        );
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                status = 'failed',
                input_json = COALESCE(?1, input_json),
                error_text = ?2,
                ended_at = COALESCE(ended_at, ?3)
            WHERE id = ?4 AND status IN ('submitted', 'working', 'input_required')
            "#,
        )
        .bind(
            reconcile_finished
                .as_ref()
                .map(|(value, _)| serde_json::to_string(value))
                .transpose()?,
        )
        .bind(error_text)
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;
        let step = load_step_record_tx(&mut tx, step_id).await?;
        let mut archive_events = Vec::new();

        if update.rows_affected() > 0 {
            let mut round_artifact_pointer = None;
            let mut round_artifact_offload_reason: Option<&str> = None;
            if let Some((_, round)) = reconcile_finished.as_ref() {
                match self
                    .persist_reconcile_round_artifact_tx(
                        &mut tx,
                        &step,
                        ReconcileRoundArtifactSnapshot {
                            round: *round,
                            status: "failed",
                            summary: step.error_text.as_deref(),
                            output: None,
                            input: step.input.as_ref(),
                            reason: None,
                            error_text: step.error_text.as_deref(),
                        },
                        now,
                    )
                    .await
                {
                    Ok(Some(pointer)) => round_artifact_pointer = Some(pointer),
                    Ok(None) => round_artifact_offload_reason = Some("agent_workdir_missing"),
                    Err(err) => {
                        tracing::warn!(
                            run_id = %step.run_id,
                            step_id = %step.id,
                            member_id = %step.member_id,
                            "team manager failed to persist reconcile round artifact: {}",
                            err
                        );
                        round_artifact_offload_reason = Some("artifact_write_failed");
                    }
                }
            }

            let payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "status": "failed",
                "error_text": step.error_text,
            });
            let mut payload = payload;
            maybe_attach_context_artifact_pointer(
                &mut payload,
                round_artifact_pointer.as_ref(),
                round_artifact_offload_reason,
            );
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "step_failed",
                now,
                &payload,
            )
            .await?;
            archive_events.push(event);

            if let Some((_, round)) = reconcile_finished.as_ref() {
                let mut round_payload = serde_json::json!({
                    "step_id": step.id,
                    "step_key": step.step_key,
                    "round": round,
                    "status": "failed",
                    "summary": step.error_text,
                });
                maybe_attach_context_artifact_pointer(
                    &mut round_payload,
                    round_artifact_pointer.as_ref(),
                    round_artifact_offload_reason,
                );
                let event = Self::append_run_event_tx(
                    &mut tx,
                    &step.run_id,
                    Some(&step.id),
                    "step_reconcile_round_finished",
                    now,
                    &round_payload,
                )
                .await?;
                archive_events.push(event);
            }

            let run_update = sqlx::query(
                r#"
                UPDATE team_runs
                SET status = 'failed', ended_at = COALESCE(ended_at, ?1)
                WHERE id = ?2 AND status IN ('submitted', 'working', 'input_required')
                "#,
            )
            .bind(now)
            .bind(&step.run_id)
            .execute(&mut *tx)
            .await?;

            if run_update.rows_affected() > 0 {
                let run_payload = serde_json::json!({
                    "status": "failed",
                });
                let event = Self::append_run_event_tx(
                    &mut tx,
                    &step.run_id,
                    None,
                    "run_failed",
                    now,
                    &run_payload,
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
