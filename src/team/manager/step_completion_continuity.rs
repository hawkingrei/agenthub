use serde_json::Value;
use sqlx::Sqlite;

use super::context_artifact_persistence::ContextArtifactOwner;
use super::context_artifacts::RuntimeStateSnapshotWritePlan;
use super::memory_flush_finalize::build_context_artifact_pointer_payload;
use super::run_task_status_sync::load_run_status_sync_meta_tx;
use super::step_continuity::{
    build_continuity_snapshot, extract_continuity_mode_from_input, should_offload_continuity_output,
};
use super::step_helpers::build_continuity_event_payload;
use super::{TeamManager, TeamRunEventRecord, TeamStepRecord};

pub(super) struct CompletedStepContinuityOutcome {
    pub(super) team_id: String,
    pub(super) run_input: Value,
    pub(super) runtime_state_snapshot: Option<RuntimeStateSnapshotWritePlan>,
}

impl TeamManager {
    pub(super) async fn apply_completed_step_continuity_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        step: &TeamStepRecord,
        now: i64,
        archive_events: &mut Vec<TeamRunEventRecord>,
    ) -> anyhow::Result<CompletedStepContinuityOutcome> {
        let (team_id, run_input) = load_run_status_sync_meta_tx(&mut *tx, &step.run_id).await?;
        let continuity_mode = extract_continuity_mode_from_input(&run_input);
        let mut continuity_snapshot = build_continuity_snapshot(step.output.as_ref());
        let mut artifact_pointer_for_event: Option<Value> = None;
        let mut artifact_offload_status = "inline";
        let mut artifact_offload_reason: Option<&str> = None;

        if should_offload_continuity_output(continuity_snapshot.redacted_output_text.as_str()) {
            match self
                .persist_continuity_artifact_tx(
                    &mut *tx,
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
                    let pointer_payload = build_context_artifact_pointer_payload(&pointer);
                    if let Some(history_obj) = continuity_snapshot.history_window.as_object_mut() {
                        history_obj.insert("artifact_pointer".to_string(), pointer_payload.clone());
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
        Self::upsert_member_continuity_state_tx(&mut *tx, &continuity_state).await?;
        let runtime_state_snapshot = self
            .prepare_runtime_state_snapshot_write_tx(
                &mut *tx,
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
            step,
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
            &mut *tx,
            &step.run_id,
            Some(&step.id),
            "continuity_state_updated",
            now,
            &continuity_payload,
        )
        .await?;
        archive_events.push(event);

        Ok(CompletedStepContinuityOutcome {
            team_id,
            run_input,
            runtime_state_snapshot,
        })
    }
}
