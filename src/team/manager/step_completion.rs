use serde_json::Value;

use super::context_artifact_persistence::ContextArtifactOwner;
use super::context_artifacts::RuntimeStateSnapshotWritePlan;
use super::run_status_sync::{load_run_status_sync_meta_tx, sync_linked_task_status_tx};
use super::step_continuity::{
    build_continuity_snapshot, extract_continuity_mode_from_input, should_offload_continuity_output,
};
use super::step_helpers::{build_continuity_event_payload, load_step_record_tx};
use super::step_reconcile::{
    ReconcileRoundArtifactSnapshot, build_reconcile_round_finished_input,
    summarize_reconcile_output,
};
use super::{
    TeamManager, TeamStepRecord, TeamTaskStatus, maybe_attach_context_artifact_pointer,
};
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

            let (team_id, run_input) = load_run_status_sync_meta_tx(&mut tx, &step.run_id).await?;
            let continuity_mode = extract_continuity_mode_from_input(&run_input);
            let mut continuity_snapshot = build_continuity_snapshot(step.output.as_ref());
            let mut artifact_pointer_for_event: Option<Value> = None;
            let mut artifact_offload_status = "inline";
            let mut artifact_offload_reason: Option<&str> = None;
            if should_offload_continuity_output(continuity_snapshot.redacted_output_text.as_str()) {
                match self
                    .persist_continuity_artifact_tx(
                        &mut tx,
                        ContextArtifactOwner {
                            team_id: &team_id,
                            run_id: &step.run_id,
                            member_id: &step.member_id,
                            session_id: step.runtime_handle_id.as_deref(),
                        },
                        &continuity_snapshot,
                        now,
                    )
                    .await
                {
                    Ok(Some(pointer)) => {
                        let pointer_payload = serde_json::json!({
                            "kind": pointer.artifact_kind,
                            "path": pointer.relative_path,
                            "size_bytes": pointer.artifact_size_bytes,
                            "checksum": pointer.content_checksum,
                        });
                        if let Some(history_obj) =
                            continuity_snapshot.history_window.as_object_mut()
                        {
                            history_obj
                                .insert("artifact_pointer".to_string(), pointer_payload.clone());
                        }
                        artifact_pointer_for_event = Some(pointer_payload);
                        artifact_offload_status = "persisted";
                    }
                    Ok(None) => {
                        artifact_offload_reason = Some("agent_workdir_missing");
                    }
                    Err(err) => {
                        tracing::warn!(
                            run_id = %step.run_id,
                            step_id = %step.id,
                            member_id = %step.member_id,
                            "team manager failed to persist continuity artifact: {}",
                            err
                        );
                        artifact_offload_reason = Some("artifact_write_failed");
                    }
                }
            }
            let continuity_state = crate::team::TeamMemberContinuityStateRecord {
                team_id: team_id.clone(),
                member_id: step.member_id.clone(),
                source_run_id: step.run_id.clone(),
                source_session_id: step.runtime_handle_id.clone(),
                summary_text: continuity_snapshot.summary_text,
                history_window: continuity_snapshot.history_window,
                updated_at: now,
            };
            Self::upsert_member_continuity_state_tx(&mut tx, &continuity_state).await?;
            runtime_state_snapshot = self
                .prepare_runtime_state_snapshot_write_tx(
                    &mut tx,
                    ContextArtifactOwner {
                        team_id: &team_id,
                        run_id: &step.run_id,
                        member_id: &step.member_id,
                        session_id: step.runtime_handle_id.as_deref(),
                    },
                    &continuity_mode,
                    &continuity_state,
                )
                .await?;

            let mut continuity_payload = build_continuity_event_payload(
                &continuity_state,
                &step,
                &continuity_mode,
                artifact_offload_status,
            );
            if let Some(payload_obj) = continuity_payload.as_object_mut() {
                if let Some(pointer_payload) = artifact_pointer_for_event.as_ref() {
                    payload_obj.insert("artifact_pointer".to_string(), pointer_payload.clone());
                }
                if let Some(reason) = artifact_offload_reason {
                    payload_obj.insert(
                        "artifact_offload_reason".to_string(),
                        Value::String(reason.to_string()),
                    );
                }
            }
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "continuity_state_updated",
                now,
                &continuity_payload,
            )
            .await?;
            archive_events.push(event);

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

            let non_completed_count: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM team_steps
                WHERE run_id = ?1 AND status <> 'completed'
                "#,
            )
            .bind(&step.run_id)
            .fetch_one(&mut *tx)
            .await?;

            if non_completed_count == 0 {
                let run_update = sqlx::query(
                    r#"
                    UPDATE team_runs
                    SET status = 'completed', ended_at = COALESCE(ended_at, ?1)
                    WHERE id = ?2 AND status IN ('submitted', 'working', 'input_required')
                    "#,
                )
                .bind(now)
                .bind(&step.run_id)
                .execute(&mut *tx)
                .await?;

                if run_update.rows_affected() > 0 {
                    let run_payload = serde_json::json!({
                        "status": "completed",
                    });
                    let event = Self::append_run_event_tx(
                        &mut tx,
                        &step.run_id,
                        None,
                        "run_completed",
                        now,
                        &run_payload,
                    )
                    .await?;
                    archive_events.push(event);
                    sync_linked_task_status_tx(
                        &mut tx,
                        &team_id,
                        &run_input,
                        TeamTaskStatus::InReview,
                        now,
                        true,
                    )
                    .await?;
                }
            }
        }

        tx.commit().await?;
        self.spawn_archive_team_run_events(archive_events);
        if let Some(plan) = runtime_state_snapshot {
            Self::write_runtime_state_snapshot_best_effort(plan).await?;
        }
        Ok(step)
    }

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
