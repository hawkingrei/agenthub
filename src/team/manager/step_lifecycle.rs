use serde_json::Value;

use super::context_artifacts::{ContextArtifactOwner, RuntimeStateSnapshotWritePlan};
use super::run_status_sync::{load_run_status_sync_meta_tx, sync_linked_task_status_tx};
use super::step_continuity::{
    build_continuity_snapshot, extract_continuity_mode_from_input, should_offload_continuity_output,
};
use super::step_helpers::{
    build_continuity_event_payload, build_step_runtime_handle_event_payload, load_step_record_tx,
    merge_step_input,
};
use super::step_reconcile::{
    ReconcileRoundArtifactSnapshot, build_reconcile_round_finished_input,
    build_reconcile_round_started_input, extract_reconcile_round_runtime,
    summarize_reconcile_output,
};
use super::{
    TeamManager, TeamStepRecord, TeamStepStatus, TeamTaskStatus,
    maybe_attach_context_artifact_pointer,
};
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

    #[allow(dead_code)]
    pub async fn resume_step(
        &self,
        step_id: &str,
        input: Option<Value>,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let current = load_step_record_tx(&mut tx, step_id).await?;
        let merged_input = merge_step_input(current.input.as_ref(), input);
        let started_input =
            build_reconcile_round_started_input(merged_input.as_ref().or(current.input.as_ref()));
        let input_json = started_input
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
                status = 'working',
                input_json = COALESCE(?1, input_json),
                error_text = NULL,
                started_at = COALESCE(started_at, ?2)
            WHERE id = ?3 AND status = 'input_required'
            "#,
        )
        .bind(input_json)
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
                WHERE id = ?2 AND status = 'input_required'
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
                let (team_id, run_input) =
                    load_run_status_sync_meta_tx(&mut tx, &step.run_id).await?;
                sync_linked_task_status_tx(
                    &mut tx,
                    &team_id,
                    &run_input,
                    TeamTaskStatus::InProgress,
                    now,
                    false,
                )
                .await?;
            }

            let step_payload = build_step_runtime_handle_event_payload(&step, "working");
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "step_resumed",
                now,
                &step_payload,
            )
            .await?;
            archive_events.push(event);

            if let Some((_, round)) = started_input.as_ref() {
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
                let round_payload = serde_json::json!({
                    "step_id": step.id,
                    "step_key": step.step_key,
                    "round": round,
                    "status": "failed",
                    "summary": step.error_text,
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
