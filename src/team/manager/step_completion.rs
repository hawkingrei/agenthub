use serde_json::Value;

use super::context_artifacts::RuntimeStateSnapshotWritePlan;
use super::step_completion_continuity::CompletedStepContinuityOutcome;
use super::step_helpers::load_step_record_tx;
use super::step_reconcile::{
    ReconcileRoundArtifactSnapshot, build_reconcile_round_finished_input,
    summarize_reconcile_output,
};
use super::{TeamManager, TeamStepRecord, maybe_attach_context_artifact_pointer};
use chrono::Utc;

impl TeamManager {
    #[allow(dead_code)]
    pub async fn complete_step(
        &self,
        step_id: &str,
        output: Option<Value>,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let current = load_step_record_tx(&mut tx, step_id).await?;
        let summary = summarize_reconcile_output(output.as_ref());
        let reconcile_finished = build_reconcile_round_finished_input(
            current.input.as_ref(),
            "completed",
            summary.as_deref(),
        );
        let output_json = output.as_ref().map(serde_json::to_string).transpose()?;
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                status = 'completed',
                input_json = COALESCE(?1, input_json),
                output_json = ?2,
                ended_at = COALESCE(ended_at, ?3)
            WHERE id = ?4 AND status IN ('working', 'input_required')
            "#,
        )
        .bind(
            reconcile_finished
                .as_ref()
                .map(|(value, _)| serde_json::to_string(value))
                .transpose()?,
        )
        .bind(output_json)
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;
        let step = load_step_record_tx(&mut tx, step_id).await?;
        let mut runtime_state_snapshot: Option<RuntimeStateSnapshotWritePlan> = None;
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
                            status: "completed",
                            summary: summary.as_deref(),
                            output: step.output.as_ref(),
                            input: step.input.as_ref(),
                            reason: None,
                            error_text: None,
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
                "status": "completed",
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
                "step_completed",
                now,
                &payload,
            )
            .await?;
            archive_events.push(event);

            let CompletedStepContinuityOutcome {
                team_id,
                run_input,
                runtime_state_snapshot: continuity_runtime_state_snapshot,
            } = self
                .apply_completed_step_continuity_tx(&mut tx, &step, now, &mut archive_events)
                .await?;
            runtime_state_snapshot = continuity_runtime_state_snapshot;

            if let Some((_, round)) = reconcile_finished.as_ref() {
                let mut round_payload = serde_json::json!({
                    "step_id": step.id,
                    "step_key": step.step_key,
                    "round": round,
                    "status": "completed",
                    "summary": summary,
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

            Self::finalize_completed_step_run_tx(
                &mut tx,
                &step,
                &team_id,
                &run_input,
                now,
                &mut archive_events,
            )
            .await?;
        }

        tx.commit().await?;
        if step.status == crate::team::TeamStepStatus::Completed {
            self.release_step_execution(step_id).await?;
        }
        self.spawn_archive_team_run_events(archive_events);
        if let Some(plan) = runtime_state_snapshot {
            Self::write_runtime_state_snapshot_best_effort(plan).await?;
        }
        Ok(step)
    }
}
