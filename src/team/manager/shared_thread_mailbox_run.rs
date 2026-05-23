use chrono::Utc;

use super::{
    TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND, TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_SOURCE,
    TeamManager, parse_team_run_row,
};
use crate::team::TeamRunRecord;

impl TeamManager {
    async fn get_latest_shared_thread_mailbox_run(
        &self,
        team_id: &str,
        task_id: &str,
    ) -> anyhow::Result<Option<TeamRunRecord>> {
        let row = sqlx::query(
            r#"
            SELECT id, team_id, context_id, status, input_json, created_at, started_at, ended_at
            FROM team_runs
            WHERE team_id = ?1
              AND trim(COALESCE(json_extract(input_json, '$.bootstrap_kind'), '')) = ?2
              AND trim(COALESCE(json_extract(input_json, '$.task_id'), '')) = ?3
            ORDER BY created_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(team_id)
        .bind(TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND)
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

    pub async fn ensure_shared_thread_mailbox_run(
        &self,
        team_id: &str,
        task_id: &str,
        conversation_id: &str,
    ) -> anyhow::Result<TeamRunRecord> {
        if let Some(existing) = self
            .get_latest_shared_thread_mailbox_run(team_id, task_id)
            .await?
        {
            return Ok(existing);
        }

        let run_id = shared_thread_mailbox_run_id(team_id, task_id);
        let context_id = format!("shared-thread-mailbox:{task_id}");
        let now = Utc::now().timestamp();
        let input = serde_json::json!({
            "bootstrap_kind": TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND,
            "bootstrap_source": TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_SOURCE,
            "task_id": task_id,
            "conversation_id": conversation_id,
            "channel": "all",
        });
        let input_json = serde_json::to_string(&input)?;

        let mut tx = self.db.begin().await?;
        let insert_result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO team_runs (
                id,
                team_id,
                group_id,
                context_id,
                status,
                input_json,
                created_at,
                started_at,
                ended_at
            )
            VALUES (?1, ?2, (SELECT group_id FROM team_definitions WHERE id = ?2), ?3, 'completed', ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(&run_id)
        .bind(team_id)
        .bind(&context_id)
        .bind(input_json)
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        if insert_result.rows_affected() > 0 {
            let submitted_payload = serde_json::json!({
                "team_id": team_id,
                "context_id": &context_id,
                "status": "completed",
                "bootstrap_kind": TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND,
            });
            sqlx::query(
                r#"
                INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                VALUES (?1, NULL, ?2, ?3, ?4)
                "#,
            )
            .bind(&run_id)
            .bind("run_submitted")
            .bind(now)
            .bind(submitted_payload.to_string())
            .execute(&mut *tx)
            .await?;

            let completed_payload = serde_json::json!({
                "status": "completed",
                "bootstrap_kind": TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND,
            });
            sqlx::query(
                r#"
                INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                VALUES (?1, NULL, ?2, ?3, ?4)
                "#,
            )
            .bind(&run_id)
            .bind("run_completed")
            .bind(now)
            .bind(completed_payload.to_string())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        self.get_run(&run_id).await
    }
}

pub(super) fn shared_thread_mailbox_run_id(team_id: &str, task_id: &str) -> String {
    format!("shared-thread-mailbox:{team_id}:{task_id}")
}
