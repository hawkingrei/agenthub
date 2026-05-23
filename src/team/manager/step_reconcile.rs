use serde_json::Value;

use super::TeamTaskStepExecutionSpec;
use agenthub_text::truncate_chars;

#[derive(Debug, Clone)]
pub(super) struct ReconcileRoundRuntime {
    pub(super) current_round: i64,
    pub(super) goal: Option<String>,
    pub(super) acceptance: Vec<String>,
    pub(super) execution: TeamTaskStepExecutionSpec,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ReconcileRoundArtifactSnapshot<'a> {
    pub(super) round: i64,
    pub(super) status: &'a str,
    pub(super) summary: Option<&'a str>,
    pub(super) output: Option<&'a Value>,
    pub(super) input: Option<&'a Value>,
    pub(super) reason: Option<&'a str>,
    pub(super) error_text: Option<&'a str>,
}

pub(super) fn extract_reconcile_round_runtime(
    input: Option<&Value>,
) -> Option<ReconcileRoundRuntime> {
    let root = input?.as_object()?;
    let step = root.get("task_execution_step")?.as_object()?;
    let execution = step
        .get("execution")
        .map(|value| serde_json::from_value::<TeamTaskStepExecutionSpec>(value.clone()))
        .transpose()
        .ok()?
        .unwrap_or_default();
    if execution.mode != super::super::TeamTaskStepExecutionMode::ReconcileLoop {
        return None;
    }
    let current_round = step
        .get("round_state")
        .and_then(Value::as_object)
        .and_then(|round_state| round_state.get("current_round"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let goal = step
        .get("goal")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let acceptance = step
        .get("acceptance")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some(ReconcileRoundRuntime {
        current_round,
        goal,
        acceptance,
        execution,
    })
}

pub(super) fn build_reconcile_round_started_input(input: Option<&Value>) -> Option<(Value, i64)> {
    let runtime = extract_reconcile_round_runtime(input)?;
    let next_round = runtime.current_round + 1;
    let mut next = input.cloned().unwrap_or_else(|| serde_json::json!({}));
    let root = next.as_object_mut()?;
    let step = root.get_mut("task_execution_step")?.as_object_mut()?;
    let round_state = step
        .entry("round_state".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let round_state = round_state.as_object_mut()?;
    round_state.insert("current_round".to_string(), Value::from(next_round));
    round_state.insert("latest_status".to_string(), Value::from("working"));
    round_state.remove("latest_outcome");
    round_state.remove("latest_summary");
    Some((next, next_round))
}

pub(super) fn build_reconcile_round_finished_input(
    input: Option<&Value>,
    outcome: &str,
    summary: Option<&str>,
) -> Option<(Value, i64)> {
    let runtime = extract_reconcile_round_runtime(input)?;
    let mut next = input.cloned().unwrap_or_else(|| serde_json::json!({}));
    let root = next.as_object_mut()?;
    let step = root.get_mut("task_execution_step")?.as_object_mut()?;
    let round_state = step
        .entry("round_state".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let round_state = round_state.as_object_mut()?;
    round_state.insert(
        "current_round".to_string(),
        Value::from(runtime.current_round.max(1)),
    );
    round_state.insert("latest_status".to_string(), Value::from(outcome));
    round_state.insert("latest_outcome".to_string(), Value::from(outcome));
    if let Some(summary) = summary.map(str::trim).filter(|value| !value.is_empty()) {
        round_state.insert("latest_summary".to_string(), Value::from(summary));
    } else {
        round_state.remove("latest_summary");
    }
    Some((next, runtime.current_round.max(1)))
}

pub(super) fn summarize_reconcile_output(output: Option<&Value>) -> Option<String> {
    let output = output?;
    output
        .as_object()
        .and_then(|obj| obj.get("summary"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            let text = output.to_string();
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| truncate_chars(trimmed, 240))
        })
}
