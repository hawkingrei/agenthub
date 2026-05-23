use serde_json::Value;
use sqlx::QueryBuilder;

use super::run_status_sync::{fallback_run_summary, load_run_summaries, load_run_summary};
use super::{
    TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND, TeamManager, TeamRunContextFingerprint,
    TeamRunEventRecord, TeamRunRecord, parse_team_run_row,
};

impl TeamManager {
    pub async fn list_runs(
        &self,
        team_id: &str,
        limit: i64,
        status: Option<&str>,
        before_created_at: Option<i64>,
    ) -> anyhow::Result<Vec<TeamRunRecord>> {
        let limit = limit.max(1);
        let mut builder = QueryBuilder::<sqlx::Sqlite>::new(
            r#"
            SELECT id, team_id, context_id, status, input_json, created_at, started_at, ended_at
            FROM team_runs
            WHERE team_id = "#,
        );
        builder.push_bind(team_id);
        builder.push(" AND trim(COALESCE(json_extract(input_json, '$.bootstrap_kind'), '')) != ");
        builder.push_bind(TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND);
        if let Some(status) = status {
            builder.push(" AND status = ");
            builder.push_bind(status);
        }
        if let Some(before_created_at) = before_created_at {
            builder.push(" AND created_at < ");
            builder.push_bind(before_created_at);
        }
        builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        builder.push_bind(limit);

        let rows = builder.build().fetch_all(&self.db).await?;

        let mut runs = Vec::with_capacity(rows.len());
        for row in rows {
            runs.push(parse_team_run_row(&row)?);
        }
        let runs = self.hydrate_run_summaries(runs).await?;
        Ok(runs)
    }

    pub async fn get_run(&self, run_id: &str) -> anyhow::Result<TeamRunRecord> {
        let row = sqlx::query(
            r#"
            SELECT id, team_id, context_id, status, input_json, created_at, started_at, ended_at
            FROM team_runs
            WHERE id = ?1
            "#,
        )
        .bind(run_id)
        .fetch_one(&self.db)
        .await?;
        self.hydrate_run_summary(parse_team_run_row(&row)?).await
    }

    pub async fn get_latest_run_for_task(
        &self,
        team_id: &str,
        task_id: &str,
    ) -> anyhow::Result<Option<TeamRunRecord>> {
        let row = sqlx::query(
            r#"
            SELECT id, team_id, context_id, status, input_json, created_at, started_at, ended_at
            FROM team_runs
            WHERE team_id = ?1
              AND trim(COALESCE(json_extract(input_json, '$.task_id'), '')) = ?2
            ORDER BY created_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(team_id)
        .bind(task_id)
        .fetch_optional(&self.db)
        .await?;
        match row {
            Some(row) => Ok(Some(
                self.hydrate_run_summary(parse_team_run_row(&row)?).await?,
            )),
            None => Ok(None),
        }
    }

    pub(super) async fn hydrate_run_summary(
        &self,
        mut run: TeamRunRecord,
    ) -> anyhow::Result<TeamRunRecord> {
        run.summary = load_run_summary(&self.db, &run.id, &run.status).await?;
        Ok(run)
    }

    pub(super) async fn hydrate_run_summaries(
        &self,
        mut runs: Vec<TeamRunRecord>,
    ) -> anyhow::Result<Vec<TeamRunRecord>> {
        let summaries = load_run_summaries(&self.db, &runs).await?;
        for run in &mut runs {
            run.summary = summaries
                .get(&run.id)
                .cloned()
                .unwrap_or_else(|| fallback_run_summary(&run.status));
        }
        Ok(runs)
    }

    pub async fn update_run_input(
        &self,
        run_id: &str,
        input: Value,
    ) -> anyhow::Result<TeamRunRecord> {
        let input_json = serde_json::to_string(&input)?;
        let update = sqlx::query(
            r#"
            UPDATE team_runs
            SET input_json = ?1
            WHERE id = ?2
            "#,
        )
        .bind(input_json)
        .bind(run_id)
        .execute(&self.db)
        .await?;
        if update.rows_affected() == 0 {
            return Err(sqlx::Error::RowNotFound.into());
        }
        self.get_run(run_id).await
    }

    pub async fn append_run_event(
        &self,
        run_id: &str,
        event_type: &str,
        payload: Value,
    ) -> anyhow::Result<()> {
        let ts = chrono::Utc::now().timestamp();
        let result = sqlx::query(
            r#"
            INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
            VALUES (?1, NULL, ?2, ?3, ?4)
            "#,
        )
        .bind(run_id)
        .bind(event_type)
        .bind(ts)
        .bind(payload.to_string())
        .execute(&self.db)
        .await?;
        let event = TeamRunEventRecord {
            event_id: result.last_insert_rowid(),
            run_id: run_id.to_string(),
            step_id: None,
            event_type: event_type.to_string(),
            ts,
            payload,
        };
        self.spawn_archive_team_run_event(&event);
        Ok(())
    }

    pub async fn read_run_context_fingerprint(
        &self,
        run_id: &str,
    ) -> anyhow::Result<TeamRunContextFingerprint> {
        let run_row = sqlx::query(
            r#"
            SELECT team_id, status
            FROM team_runs
            WHERE id = ?1
            "#,
        )
        .bind(run_id)
        .fetch_one(&self.db)
        .await?;
        let team_id: String = sqlx::Row::get(&run_row, "team_id");
        let run_status: String = sqlx::Row::get(&run_row, "status");
        let latest_event_id = sqlx::query_scalar::<_, Option<i64>>(
            r#"
            SELECT MAX(id)
            FROM team_run_events
            WHERE run_id = ?1
            "#,
        )
        .bind(run_id)
        .fetch_one(&self.db)
        .await?
        .unwrap_or(0);
        let latest_mailbox_message_id = sqlx::query_scalar::<_, Option<i64>>(
            r#"
            SELECT MAX(id)
            FROM team_actor_messages
            WHERE run_id = ?1
            "#,
        )
        .bind(run_id)
        .fetch_one(&self.db)
        .await?
        .unwrap_or(0);
        let status_counts = self.list_actor_message_status_counts(run_id).await?;
        Ok(TeamRunContextFingerprint {
            team_id,
            run_id: run_id.to_string(),
            run_status,
            latest_event_id,
            latest_mailbox_message_id,
            mailbox_pending: status_counts.get("pending").copied().unwrap_or(0),
            mailbox_delivered: status_counts.get("delivered").copied().unwrap_or(0),
            mailbox_dead_letter: status_counts.get("dead_letter").copied().unwrap_or(0),
        })
    }
}
