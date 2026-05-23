use serde_json::Value;
use sqlx::Sqlite;
use uuid::Uuid;

use super::step_helpers::load_step_record_tx;
use super::{TeamManager, TeamRunEventRecord, TeamStepStatus, team_step_status_to_str};

#[derive(Debug, Clone)]
pub(super) struct MaterializedRunStepTemplate {
    pub(super) step_key: String,
    pub(super) member_id: String,
    pub(super) depends_on: Vec<String>,
    pub(super) input: Option<Value>,
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
