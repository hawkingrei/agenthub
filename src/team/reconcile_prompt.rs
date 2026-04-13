use crate::agent::AgentManager;
use crate::team::{
    TeamManager, TeamStepRecord, TeamTaskStepExecutionMode, TeamTaskStepExecutionSpec,
};
use serde_json::Value;

#[derive(Debug, Clone)]
struct ReconcilePromptContext {
    round: i64,
    goal: Option<String>,
    acceptance: Vec<String>,
    execution: TeamTaskStepExecutionSpec,
}

fn extract_reconcile_prompt_context(step: &TeamStepRecord) -> Option<ReconcilePromptContext> {
    let input = step.input.as_ref()?.as_object()?;
    let task_execution_step = input.get("task_execution_step")?.as_object()?;
    let execution = task_execution_step
        .get("execution")
        .map(|value| serde_json::from_value::<TeamTaskStepExecutionSpec>(value.clone()))
        .transpose()
        .ok()?
        .unwrap_or_default();
    if execution.mode != TeamTaskStepExecutionMode::ReconcileLoop {
        return None;
    }
    let round = task_execution_step
        .get("round_state")
        .and_then(Value::as_object)
        .and_then(|round_state| round_state.get("current_round"))
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(1);
    let goal = task_execution_step
        .get("goal")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let acceptance = task_execution_step
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
    Some(ReconcilePromptContext {
        round,
        goal,
        acceptance,
        execution,
    })
}

pub(crate) fn build_reconcile_step_prompt(step: &TeamStepRecord) -> Option<String> {
    let context = extract_reconcile_prompt_context(step)?;
    let mut lines = vec![
        "AgentHub reconcile step follow-up:".to_string(),
        format!("- run_id: {}", step.run_id),
        format!("- step_id: {}", step.id),
        format!("- step_key: {}", step.step_key),
        format!("- round: {}", context.round),
    ];
    if let Some(max_rounds) = context.execution.max_rounds {
        lines.push(format!("- max_rounds: {max_rounds}"));
    }
    if let Some(goal) = context.goal.as_deref() {
        lines.push(format!("- goal: {goal}"));
    }
    if !context.acceptance.is_empty() {
        lines.push("- acceptance:".to_string());
        lines.extend(context.acceptance.iter().map(|item| format!("  - {item}")));
    }
    lines.push(
        "Continue this assigned step now using the current workspace state and prior evidence."
            .to_string(),
    );
    lines.push(
        "At the end of this round, write `.agenthubmemory/step-decision.json`, then run `agenthub actor team-step-decision --step-id <step_id>`. Use `action=\"continue\"` to begin the next round, or choose `complete`, `input_required`, or `fail` with concrete evidence."
            .to_string(),
    );
    lines.push("Decision file template:".to_string());
    lines.push("  {".to_string());
    lines.push("    \"action\": \"continue|complete|input_required|fail\",".to_string());
    lines.push("    \"output\": {".to_string());
    lines.push("      \"summary\": \"what changed this round\",".to_string());
    lines.push("      \"artifacts\": [\"relative/path/to/evidence\"]".to_string());
    lines.push("    },".to_string());
    lines.push("    \"input\": {\"question\": \"only when more input is required\"},".to_string());
    lines.push("    \"reason\": \"only when handing off or requesting input\",".to_string());
    lines.push("    \"error_text\": \"only when action=fail\"".to_string());
    lines.push("  }".to_string());
    Some(lines.join("\n"))
}

pub(crate) async fn maybe_nudge_reconcile_step_prompt(
    teams: &TeamManager,
    agents: &AgentManager,
    step: &TeamStepRecord,
) -> anyhow::Result<bool> {
    let Some(runtime_handle_id) = step
        .runtime_handle_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(false);
    };
    let Some(prompt) = build_reconcile_step_prompt(step) else {
        return Ok(false);
    };

    let payload = serde_json::json!({
        "step_id": step.id,
        "step_key": step.step_key,
        "member_id": step.member_id,
        "runtime_handle_id": runtime_handle_id,
    });
    teams
        .append_run_event(&step.run_id, "step_reconcile_prompt_requested", payload)
        .await?;

    if let Err(err) = agents
        .send_input(&step.member_id, &prompt, None, Some(runtime_handle_id))
        .await
    {
        teams
            .append_run_event(
                &step.run_id,
                "step_reconcile_prompt_failed",
                serde_json::json!({
                    "step_id": step.id,
                    "step_key": step.step_key,
                    "member_id": step.member_id,
                    "runtime_handle_id": runtime_handle_id,
                    "error": err.to_string(),
                }),
            )
            .await?;
        return Err(err);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::build_reconcile_step_prompt;
    use crate::team::{TeamStepRecord, TeamStepStatus};
    use serde_json::json;

    #[test]
    fn reconcile_prompt_includes_round_goal_and_acceptance() {
        let step = TeamStepRecord {
            id: "step-1".to_string(),
            run_id: "run-1".to_string(),
            step_key: "worker-implement".to_string(),
            member_id: "worker".to_string(),
            runtime_handle_id: Some("session-1".to_string()),
            status: TeamStepStatus::Working,
            attempt: 0,
            depends_on: Vec::new(),
            input: Some(json!({
                "task_execution_step": {
                    "goal":"finish the patch",
                    "acceptance":["tests pass","review ready"],
                    "execution":{"mode":"reconcile_loop","max_rounds":4},
                    "round_state":{"current_round":2}
                }
            })),
            output: None,
            error_text: None,
            started_at: None,
            ended_at: None,
        };
        let prompt = build_reconcile_step_prompt(&step).expect("build reconcile prompt");
        assert!(prompt.contains("run_id: run-1"));
        assert!(prompt.contains("step_key: worker-implement"));
        assert!(prompt.contains("round: 2"));
        assert!(prompt.contains("goal: finish the patch"));
        assert!(prompt.contains("tests pass"));
        assert!(prompt.contains("review ready"));
        assert!(prompt.contains("Use `action=\"continue\"`"));
        assert!(prompt.contains(".agenthubmemory/step-decision.json"));
        assert!(prompt.contains("\"action\": \"continue|complete|input_required|fail\""));
    }
}
