use super::{
    TeamManager, TeamRunRecord, TeamRunStatus, TeamTaskStatus, filter_visible_team_runs,
    load_linked_task_ids_for_runs, parse_team_run_row,
};
use crate::team::TeamRunResumeError;

impl TeamManager {
    async fn fork_run_submission(&self, source: &TeamRunRecord) -> anyhow::Result<TeamRunRecord> {
        self.create_run(
            &source.team_id,
            Some(&source.context_id),
            source.input.clone(),
        )
        .await
    }

    pub async fn restart_run(&self, run_id: &str) -> anyhow::Result<TeamRunRecord> {
        let run = self.get_run(run_id).await?;
        self.fork_run_submission(&run).await
    }

    pub async fn resume_run(&self, run_id: &str) -> anyhow::Result<TeamRunRecord> {
        let run = self.get_run(run_id).await?;
        match run.status {
            TeamRunStatus::Submitted | TeamRunStatus::Working | TeamRunStatus::InputRequired => {
                Ok(run)
            }
            TeamRunStatus::Failed | TeamRunStatus::Canceled => self.fork_run_submission(&run).await,
            TeamRunStatus::Completed => Err(TeamRunResumeError::CompletedRun.into()),
        }
    }

    #[cfg(test)]
    pub async fn list_active_runs(&self, limit: i64) -> anyhow::Result<Vec<TeamRunRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id, team_id, context_id, status, input_json, created_at, started_at, ended_at
            FROM team_runs
            WHERE status IN ('submitted', 'working', 'input_required')
            ORDER BY created_at ASC, id ASC
            LIMIT ?1
            "#,
        )
        .bind(limit.max(1))
        .fetch_all(&self.db)
        .await?;

        let mut runs = Vec::with_capacity(rows.len());
        for row in rows {
            runs.push(parse_team_run_row(&row)?);
        }
        let runs = self.hydrate_run_summaries(runs).await?;
        Ok(filter_visible_team_runs(runs))
    }

    pub async fn list_active_runs_for_team(
        &self,
        team_id: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<TeamRunRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id, team_id, context_id, status, input_json, created_at, started_at, ended_at
            FROM team_runs
            WHERE team_id = ?1
              AND status IN ('submitted', 'working', 'input_required')
            ORDER BY created_at DESC, id DESC
            LIMIT ?2
            "#,
        )
        .bind(team_id)
        .bind(limit.max(1))
        .fetch_all(&self.db)
        .await?;

        let mut runs = Vec::with_capacity(rows.len());
        for row in rows {
            runs.push(parse_team_run_row(&row)?);
        }
        let runs = self.hydrate_run_summaries(runs).await?;
        Ok(filter_visible_team_runs(runs))
    }

    // Cancel all non-terminal runs left from a previous process lifetime.
    // This keeps startup deterministic and shifts resumption to explicit user action.
    pub async fn cancel_active_runs_on_startup(&self) -> anyhow::Result<usize> {
        let active_run_ids = sqlx::query_scalar::<_, String>(
            r#"
            SELECT id
            FROM team_runs
            WHERE status IN ('submitted', 'working', 'input_required')
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .fetch_all(&self.db)
        .await?;
        let linked_task_ids = load_linked_task_ids_for_runs(&self.db, &active_run_ids).await?;

        let mut canceled_count = 0usize;
        for run_id in active_run_ids {
            let linked_task_id = linked_task_ids.get(&run_id).map(String::as_str);
            let canceled = self.cancel_run(&run_id).await?;
            if canceled.status == TeamRunStatus::Canceled {
                canceled_count += 1;
                if let Err(err) = self
                    .append_run_event(
                        &run_id,
                        "run_startup_canceled",
                        serde_json::json!({
                            "status": "canceled",
                            "reason": "manual_start_required_after_service_restart",
                        }),
                    )
                    .await
                {
                    tracing::warn!(
                        run_id = %run_id,
                        error = %err,
                        "failed to append startup cancellation event"
                    );
                }
                if let Some(task_id) = linked_task_id {
                    match self.get_task(task_id).await {
                        Ok(task)
                            if matches!(
                                task.status,
                                TeamTaskStatus::InProgress | TeamTaskStatus::Canceled
                            ) =>
                        {
                            if let Err(err) =
                                self.update_task_status(task_id, TeamTaskStatus::Open).await
                            {
                                tracing::warn!(
                                    run_id = %run_id,
                                    task_id = %task_id,
                                    error = %err,
                                    "failed to reopen linked task after startup run cancellation"
                                );
                            }
                        }
                        Ok(_) => {}
                        Err(err) => {
                            tracing::warn!(
                                run_id = %run_id,
                                task_id = %task_id,
                                error = %err,
                                "failed to load linked task after startup run cancellation"
                            );
                        }
                    }
                }
            }
        }
        Ok(canceled_count)
    }
}
