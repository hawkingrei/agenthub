use serde_json::Value;
use sqlx::Sqlite;

use super::{TeamStepRecord, merge_json_value, parse_team_step_row};

pub(super) fn build_step_runtime_handle_event_payload(
    step: &TeamStepRecord,
    status: &'static str,
) -> Value {
    serde_json::json!({
        "step_id": step.id,
        "step_key": step.step_key,
        "status": status,
        "runtime_handle_id": step.runtime_handle_id,
    })
}

pub(super) fn build_continuity_event_payload(
    continuity_state: &crate::team::TeamMemberContinuityStateRecord,
    step: &TeamStepRecord,
    continuity_mode: &str,
    artifact_offload_status: &str,
) -> Value {
    serde_json::json!({
        "team_id": continuity_state.team_id,
        "member_id": continuity_state.member_id,
        "step_id": step.id,
        "step_key": step.step_key,
        "mode": continuity_mode,
        "source_run_id": continuity_state.source_run_id,
        "source_runtime_handle_id": continuity_state.source_session_id,
        "summary_chars": continuity_state.summary_text.chars().count(),
        "artifact_offload_status": artifact_offload_status,
    })
}

pub(super) fn merge_step_input(
    current_input: Option<&Value>,
    next_input: Option<Value>,
) -> Option<Value> {
    next_input.map(|next_input| {
        if let Some(current_input) = current_input
            && next_input.get("task_execution_step").is_none()
        {
            let mut merged = current_input.clone();
            merge_json_value(&mut merged, &next_input);
            return merged;
        }
        next_input
    })
}

pub(super) async fn load_step_record_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    step_id: &str,
) -> anyhow::Result<TeamStepRecord> {
    let step_row = sqlx::query(
        r#"
        SELECT
            id,
            run_id,
            step_key,
            member_id,
            remote_task_id,
            status,
            attempt,
            depends_on_json,
            input_json,
            output_json,
            error_text,
            started_at,
            ended_at
        FROM team_steps
        WHERE id = ?1
        "#,
    )
    .bind(step_id)
    .fetch_one(&mut **tx)
    .await?;
    parse_team_step_row(&step_row)
}
