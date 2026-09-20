use serde_json::Value;
use sqlx::{Sqlite, Transaction};
use uuid::Uuid;

use super::run_task_status_sync::{
    extract_linked_task_id_from_run_input, sync_linked_task_status_tx,
};
use super::step_continuity::{extract_continuity_mode_from_input, normalize_run_input_continuity};
use super::step_materialization::insert_materialized_run_steps_tx;
use super::step_template_builders::{
    build_materialized_run_step_templates_from_task_execution_plan,
    extract_materialized_run_step_templates_from_input, validate_materialized_run_step_templates,
};
use super::{TeamManager, TeamRunRecord, TeamRunStatus, TeamTaskStatus, team_run_status_to_str};

impl TeamManager {
    async fn insert_submitted_run_tx(
        tx: &mut Transaction<'_, Sqlite>,
        run_id: &str,
        team_id: &str,
        context_id: &str,
        input_json: &str,
        now: i64,
    ) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO team_runs (id, team_id, group_id, context_id, status, input_json, created_at) \
            VALUES (?, ?, (SELECT group_id FROM team_definitions WHERE id = ?), ?, 'submitted', ?, ?)")
            .bind(run_id).bind(team_id).bind(team_id).bind(context_id).bind(input_json).bind(now)
            .execute(&mut **tx).await?;
        Ok(())
    }

    /// Allocate mailbox identity independently of a task attempt or provider session.
    pub(crate) async fn ensure_loop_mailbox_partition(
        &self,
        team_id: &str,
    ) -> anyhow::Result<TeamRunRecord> {
        self.get_team(team_id).await?;
        let mut tx = self.db.begin_with("BEGIN IMMEDIATE").await?;
        if let Some(run_id) = sqlx::query_scalar::<_, String>(
            "SELECT run_id FROM loop_mailbox_partitions WHERE team_id = ? AND active = 1",
        )
        .bind(team_id)
        .fetch_optional(&mut *tx)
        .await?
        {
            tx.commit().await?;
            let run = self.get_run(&run_id).await?;
            anyhow::ensure!(
                matches!(
                    run.status,
                    TeamRunStatus::Submitted
                        | TeamRunStatus::Working
                        | TeamRunStatus::InputRequired
                ),
                "loop mailbox partition is terminal; explicit scope reconciliation is required"
            );
            return Ok(run);
        }
        let run_id = Uuid::new_v4().to_string();
        let context_id = format!("loop:{team_id}");
        let input = serde_json::json!({"loop_mailbox_partition": {"version": 1}});
        let now = chrono::Utc::now().timestamp();
        Self::insert_submitted_run_tx(
            &mut tx,
            &run_id,
            team_id,
            &context_id,
            &input.to_string(),
            now,
        )
        .await?;
        sqlx::query(
            "INSERT INTO loop_mailbox_partitions(run_id, team_id, created_at) VALUES (?, ?, ?)",
        )
        .bind(&run_id)
        .bind(team_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        let event = Self::append_run_event_tx(
            &mut tx,
            &run_id,
            None,
            "loop_mailbox_created",
            now,
            &serde_json::json!({"team_id": team_id, "context_id": context_id}),
        )
        .await?;
        tx.commit().await?;
        self.spawn_archive_team_run_events(vec![event]);
        Ok(TeamRunRecord {
            id: run_id,
            team_id: team_id.into(),
            context_id,
            status: TeamRunStatus::Submitted,
            input,
            summary: None,
            created_at: now,
            started_at: None,
            ended_at: None,
        })
    }

    pub async fn create_run(
        &self,
        team_id: &str,
        context_id: Option<&str>,
        input: Value,
    ) -> anyhow::Result<TeamRunRecord> {
        let team = self.get_team(team_id).await?;
        let run_id = Uuid::new_v4().to_string();
        let resolved_context_id = context_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = chrono::Utc::now().timestamp();
        let status = TeamRunStatus::Submitted;
        let input = normalize_run_input_continuity(input);
        let linked_task = if let Some(task_id) = extract_linked_task_id_from_run_input(&input) {
            Some(self.get_task_for_team(team_id, task_id).await?)
        } else {
            None
        };
        let (materialized_steps, materialized_steps_scope) = {
            let from_input = extract_materialized_run_step_templates_from_input(&input)?;
            if !from_input.is_empty() {
                (from_input, "run input step_template")
            } else if let Some(task) = linked_task.as_ref() {
                (
                    build_materialized_run_step_templates_from_task_execution_plan(task)?,
                    "linked task execution_plan.steps",
                )
            } else {
                (Vec::new(), "run input step_template")
            }
        };
        validate_materialized_run_step_templates(
            &team.spec,
            &materialized_steps,
            materialized_steps_scope,
        )?;
        let input_json = serde_json::to_string(&input)?;
        let continuity_mode = extract_continuity_mode_from_input(&input);

        let mut tx = self.db.begin().await?;
        Self::insert_submitted_run_tx(
            &mut tx,
            &run_id,
            team_id,
            &resolved_context_id,
            &input_json,
            now,
        )
        .await?;

        let payload = serde_json::json!({
            "team_id": team_id,
            "context_id": &resolved_context_id,
            "status": team_run_status_to_str(&status),
            "continuity_mode": continuity_mode,
        });
        let submitted_event =
            Self::append_run_event_tx(&mut tx, &run_id, None, "run_submitted", now, &payload)
                .await?;
        let mut archive_events =
            insert_materialized_run_steps_tx(&mut tx, &run_id, &materialized_steps, now).await?;
        sync_linked_task_status_tx(
            &mut tx,
            team_id,
            &input,
            TeamTaskStatus::InProgress,
            now,
            true,
        )
        .await?;
        tx.commit().await?;
        archive_events.insert(0, submitted_event);
        self.spawn_archive_team_run_events(archive_events);

        Ok(TeamRunRecord {
            id: run_id,
            team_id: team_id.to_string(),
            context_id: resolved_context_id,
            status,
            input,
            summary: None,
            created_at: now,
            started_at: None,
            ended_at: None,
        })
    }
}
