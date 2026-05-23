use serde_json::Value;

use super::step_helpers::load_step_record_tx;
use super::step_reconcile::{
    ReconcileRoundArtifactSnapshot, build_reconcile_round_finished_input,
    build_reconcile_round_started_input, extract_reconcile_round_runtime,
    summarize_reconcile_output,
};
use super::{TeamManager, TeamStepRecord, TeamStepStatus, maybe_attach_context_artifact_pointer};
use chrono::Utc;

impl TeamManager {
    #[allow(dead_code)]
    pub async fn continue_step(
        &self,
        step_id: &str,
        output: Option<Value>,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let current = load_step_record_tx(&mut tx, step_id).await?;
        if current.status != TeamStepStatus::Working {
            anyhow::bail!("continue_step requires a working reconcile step");
        }
        let runtime = extract_reconcile_round_runtime(current.input.as_ref())
            .ok_or_else(|| anyhow::anyhow!("continue_step requires reconcile_loop step input"))?;
        let current_round = runtime.current_round.max(1);
        if let Some(max_rounds) = runtime.execution.max_rounds
            && current_round >= max_rounds
        {
            anyhow::bail!(
                "reconcile_loop step reached max_rounds={max_rounds}; use complete, input_required, or fail instead"
            );
        }

        let summary = summarize_reconcile_output(output.as_ref());
        let finished_input = build_reconcile_round_finished_input(
            current.input.as_ref(),
            "continued",
            summary.as_deref(),
        )
        .ok_or_else(|| anyhow::anyhow!("continue_step requires reconcile_loop round state"))?;
        let started_input = build_reconcile_round_started_input(Some(&finished_input.0))
            .ok_or_else(|| anyhow::anyhow!("continue_step failed to start next reconcile round"))?;
        let output_json = output.as_ref().map(serde_json::to_string).transpose()?;
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                input_json = ?1,
                output_json = COALESCE(?2, output_json),
                error_text = NULL,
                started_at = COALESCE(started_at, ?3)
            WHERE id = ?4 AND status = 'working'
            "#,
        )
        .bind(serde_json::to_string(&started_input.0)?)
        .bind(output_json)
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;
        let step = load_step_record_tx(&mut tx, step_id).await?;
        let mut archive_events = Vec::new();

        if update.rows_affected() > 0 {
            let mut round_artifact_pointer = None;
            let mut round_artifact_offload_reason: Option<&str> = None;
            match self
                .persist_reconcile_round_artifact_tx(
                    &mut tx,
                    &step,
                    ReconcileRoundArtifactSnapshot {
                        round: finished_input.1,
                        status: "continued",
                        summary: summary.as_deref(),
                        output: output.as_ref(),
                        input: Some(&finished_input.0),
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

            let continue_payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "status": "working",
                "continued_from_round": finished_input.1,
                "continued_to_round": started_input.1,
                "summary": summary,
            });
            let mut continue_payload = continue_payload;
            maybe_attach_context_artifact_pointer(
                &mut continue_payload,
                round_artifact_pointer.as_ref(),
                round_artifact_offload_reason,
            );
            if round_artifact_pointer.is_none()
                && let (Some(output), Some(payload_obj)) =
                    (output.as_ref(), continue_payload.as_object_mut())
            {
                payload_obj.insert("output".to_string(), output.clone());
                payload_obj.insert(
                    "output_inlined_because".to_string(),
                    serde_json::json!(
                        round_artifact_offload_reason.unwrap_or("artifact_pointer_missing")
                    ),
                );
            }
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "step_continued",
                now,
                &continue_payload,
            )
            .await?;
            archive_events.push(event);

            let round_finished_payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "round": finished_input.1,
                "status": "continued",
                "summary": summary,
            });
            let mut round_finished_payload = round_finished_payload;
            maybe_attach_context_artifact_pointer(
                &mut round_finished_payload,
                round_artifact_pointer.as_ref(),
                round_artifact_offload_reason,
            );
            if round_artifact_pointer.is_none()
                && let (Some(output), Some(payload_obj)) =
                    (output.as_ref(), round_finished_payload.as_object_mut())
            {
                payload_obj.insert("output".to_string(), output.clone());
                payload_obj.insert(
                    "output_inlined_because".to_string(),
                    serde_json::json!(
                        round_artifact_offload_reason.unwrap_or("artifact_pointer_missing")
                    ),
                );
            }
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "step_reconcile_round_finished",
                now,
                &round_finished_payload,
            )
            .await?;
            archive_events.push(event);

            let round_started_payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "round": started_input.1,
                "status": "working",
                "goal": runtime.goal,
                "acceptance_count": runtime.acceptance.len(),
                "max_rounds": runtime.execution.max_rounds,
            });
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "step_reconcile_round_started",
                now,
                &round_started_payload,
            )
            .await?;
            archive_events.push(event);
        }

        tx.commit().await?;
        self.spawn_archive_team_run_events(archive_events);
        Ok(step)
    }
}
