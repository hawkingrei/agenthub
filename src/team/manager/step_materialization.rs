use serde_json::Value;
use sqlx::Sqlite;
use uuid::Uuid;

use super::step_helpers::load_step_record_tx;
use super::{
    TeamManager, TeamRunEventRecord, TeamStepStatus, TeamTaskRecord, TeamTaskStepExecutionSpec,
    parse_task_execution_plan, team_step_status_to_str, validate_task_execution_steps,
};

#[derive(Debug, Clone)]
pub(super) struct MaterializedRunStepTemplate {
    pub(super) step_key: String,
    pub(super) member_id: String,
    pub(super) depends_on: Vec<String>,
    pub(super) input: Option<Value>,
}

fn build_materialized_step_input(
    goal: Option<String>,
    acceptance: Vec<String>,
    execution: TeamTaskStepExecutionSpec,
) -> Option<Value> {
    let goal = goal
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let acceptance = acceptance
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    if goal.is_none() && acceptance.is_empty() && execution == TeamTaskStepExecutionSpec::default()
    {
        return None;
    }

    Some(serde_json::json!({
        "task_execution_step": {
            "goal": goal,
            "acceptance": acceptance,
            "execution": execution,
            "round_state": {
                "current_round": 0_i64,
            },
        }
    }))
}

fn parse_task_execution_step_input(
    input: Option<&Value>,
) -> anyhow::Result<(Option<String>, Vec<String>, TeamTaskStepExecutionSpec)> {
    let Some(step) = input
        .and_then(|value| value.get("task_execution_step"))
        .and_then(Value::as_object)
    else {
        return Ok((None, Vec::new(), TeamTaskStepExecutionSpec::default()));
    };
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
    let execution = step
        .get("execution")
        .map(|value| serde_json::from_value::<TeamTaskStepExecutionSpec>(value.clone()))
        .transpose()?
        .unwrap_or_default();
    Ok((goal, acceptance, execution))
}

pub(super) fn validate_materialized_run_step_templates(
    team_spec: &Value,
    steps: &[MaterializedRunStepTemplate],
    scope: &str,
) -> anyhow::Result<()> {
    let execution_steps = steps
        .iter()
        .map(|step| {
            let (goal, acceptance, execution) =
                parse_task_execution_step_input(step.input.as_ref())?;
            Ok(agenthub_team_domain::TeamTaskExecutionStepSpec {
                step_key: step.step_key.clone(),
                member_id: step.member_id.clone(),
                depends_on: step.depends_on.clone(),
                goal,
                acceptance,
                execution,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    validate_task_execution_steps(team_spec, &execution_steps, scope)
}

pub(super) fn extract_materialized_run_step_templates_from_input(
    input: &Value,
) -> anyhow::Result<Vec<MaterializedRunStepTemplate>> {
    let Some(input_obj) = input.as_object() else {
        return Ok(Vec::new());
    };
    let Some(raw_steps) = input_obj.get("step_template") else {
        return Ok(Vec::new());
    };
    let steps = raw_steps
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("run input step_template must be an array"))?;
    let mut out = Vec::with_capacity(steps.len());
    for step in steps {
        let step_obj = step
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("run input step_template entries must be objects"))?;
        let step_key = step_obj
            .get("step_key")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("run input step_template[].step_key is required"))?;
        let member_id = step_obj
            .get("member_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("run input step_template[].member_id is required"))?;
        let depends_on = step_obj
            .get("depends_on")
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
        let goal = step_obj
            .get("goal")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let acceptance = step_obj
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
        let execution = step_obj
            .get("execution")
            .map(|value| serde_json::from_value::<TeamTaskStepExecutionSpec>(value.clone()))
            .transpose()?
            .unwrap_or_default();
        out.push(MaterializedRunStepTemplate {
            step_key: step_key.to_string(),
            member_id: member_id.to_string(),
            depends_on,
            input: build_materialized_step_input(goal, acceptance, execution),
        });
    }
    Ok(out)
}

pub(super) fn build_materialized_run_step_templates_from_task_execution_plan(
    task: &TeamTaskRecord,
) -> anyhow::Result<Vec<MaterializedRunStepTemplate>> {
    let Some(plan) = parse_task_execution_plan(&task.context)? else {
        return Ok(Vec::new());
    };
    Ok(plan
        .steps
        .into_iter()
        .map(|step| MaterializedRunStepTemplate {
            step_key: step.step_key.trim().to_string(),
            member_id: step.member_id.trim().to_string(),
            depends_on: step
                .depends_on
                .into_iter()
                .map(|value| value.trim().to_string())
                .collect(),
            input: build_materialized_step_input(step.goal, step.acceptance, step.execution),
        })
        .collect())
}

pub(super) async fn insert_materialized_run_steps_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
    steps: &[MaterializedRunStepTemplate],
    now: i64,
) -> anyhow::Result<Vec<TeamRunEventRecord>> {
    let mut events = Vec::with_capacity(steps.len());
    for step in steps {
        let step_id = Uuid::new_v4().to_string();
        let depends_on_json = serde_json::to_string(&step.depends_on)?;
        let input_json = step.input.as_ref().map(serde_json::to_string).transpose()?;
        sqlx::query(
            r#"
            INSERT INTO team_steps (
                id, run_id, step_key, member_id, remote_task_id, status, attempt, depends_on_json, input_json
            )
            VALUES (?1, ?2, ?3, ?4, NULL, ?5, 0, ?6, ?7)
            "#,
        )
        .bind(&step_id)
        .bind(run_id)
        .bind(&step.step_key)
        .bind(&step.member_id)
        .bind(team_step_status_to_str(&TeamStepStatus::Submitted))
        .bind(depends_on_json)
        .bind(input_json)
        .execute(&mut **tx)
        .await?;

        let payload = serde_json::json!({
            "step_id": step_id,
            "step_key": step.step_key,
            "member_id": step.member_id,
            "status": team_step_status_to_str(&TeamStepStatus::Submitted),
        });
        let event = TeamManager::append_run_event_tx(
            tx,
            run_id,
            Some(&step_id),
            "step_submitted",
            now,
            &payload,
        )
        .await?;
        events.push(event);
    }
    Ok(events)
}

#[allow(dead_code)]
pub(super) async fn load_step_record(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    step_id: &str,
) -> anyhow::Result<crate::team::TeamStepRecord> {
    load_step_record_tx(tx, step_id).await
}
