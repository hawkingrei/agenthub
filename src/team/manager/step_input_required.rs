use serde_json::Value;

use super::run_task_status_sync::{load_run_status_sync_meta_tx, sync_linked_task_status_tx};
use super::step_helpers::{load_step_record_tx, merge_step_input};
use super::step_reconcile::{
    ReconcileRoundArtifactSnapshot, build_reconcile_round_finished_input,
    summarize_reconcile_output,
};
use super::{TeamManager, TeamStepRecord, TeamTaskStatus, maybe_attach_context_artifact_pointer};
use chrono::Utc;

impl TeamManager {
    #[allow(dead_code)]
    pub async fn set_step_input_required(
        &self,
        step_id: &str,
        reason: Option<&str>,
        input: Option<Value>,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let current = load_step_record_tx(&mut tx, step_id).await?;
        let summary = reason
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| summarize_reconcile_output(input.as_ref()));
        let merged_input = merge_step_input(current.input.as_ref(), input);
        let reconcile_finished = build_reconcile_round_finished_input(
            merged_input.as_ref().or(current.input.as_ref()),
            "input_required",
            summary.as_deref(),
        );
        let input_json = reconcile_finished
            .as_ref()
            .map(|(value, _)| serde_json::to_string(value))
            .transpose()?
            .or_else(|| {
                merged_input
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .ok()
                    .flatten()
            });
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                status = 'input_required',
                input_json = COALESCE(?1, input_json),
                error_text = COALESCE(?2, error_text),
                started_at = COALESCE(started_at, ?3)
            WHERE id = ?4 AND status IN ('submitted', 'working')
            "#,
        )
        .bind(input_json)
        .bind(reason)
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
                            status: "input_required",
                            summary: summary.as_deref(),
                            output: None,
                            input: step.input.as_ref(),
                            reason: step.error_text.as_deref(),
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

            let run_update = sqlx::query(
                r#"
                UPDATE team_runs
                SET status = 'input_required', started_at = COALESCE(started_at, ?1)
                WHERE id = ?2 AND status IN ('submitted', 'working')
                "#,
            )
            .bind(now)
            .bind(&step.run_id)
            .execute(&mut *tx)
            .await?;
            if run_update.rows_affected() > 0 {
                let run_payload = serde_json::json!({
                    "status": "input_required",
                    "step_id": step.id,
                    "step_key": step.step_key,
                });
                let event = Self::append_run_event_tx(
                    &mut tx,
                    &step.run_id,
                    None,
                    "run_input_required",
                    now,
                    &run_payload,
                )
                .await?;
                archive_events.push(event);
                let (team_id, run_input) =
                    load_run_status_sync_meta_tx(&mut tx, &step.run_id).await?;
                sync_linked_task_status_tx(
                    &mut tx,
                    &team_id,
                    &run_input,
                    TeamTaskStatus::Waiting,
                    now,
                    true,
                )
                .await?;
            }

            let step_payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "status": "input_required",
                "reason": step.error_text,
                "input": step.input,
            });
            let mut step_payload = step_payload;
            maybe_attach_context_artifact_pointer(
                &mut step_payload,
                round_artifact_pointer.as_ref(),
                round_artifact_offload_reason,
            );
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "step_input_required",
                now,
                &step_payload,
            )
            .await?;
            archive_events.push(event);

            if let Some((_, round)) = reconcile_finished.as_ref() {
                let round_payload = serde_json::json!({
                    "step_id": step.id,
                    "step_key": step.step_key,
                    "round": round,
                    "status": "input_required",
                    "summary": summary,
                });
                let mut round_payload = round_payload;
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
        }

        tx.commit().await?;
        self.spawn_archive_team_run_events(archive_events);
        Ok(step)
    }
}
