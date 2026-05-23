use serde_json::Value;
use uuid::Uuid;

use super::run_status_sync::{
    extract_linked_task_id_from_run_input, load_run_status_sync_meta_tx, sync_linked_task_status_tx,
};
use super::step_lifecycle::{
    build_materialized_run_step_templates_from_task_execution_plan,
    extract_continuity_mode_from_input, extract_materialized_run_step_templates_from_input,
    insert_materialized_run_steps_tx, normalize_run_input_continuity,
    validate_materialized_run_step_templates,
};
use super::{
    TeamManager, TeamRunEventRecord, TeamRunRecord, TeamRunStatus, TeamTaskStatus, mailbox,
    parse_run_event_row, parse_team_actor_message_row, team_run_status_to_str,
};
use crate::team::TeamActorMessageRecord;

impl TeamManager {
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
        sqlx::query(
            r#"
            INSERT INTO team_runs (id, team_id, group_id, context_id, status, input_json, created_at)
            VALUES (?1, ?2, (SELECT group_id FROM team_definitions WHERE id = ?2), ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&run_id)
        .bind(team_id)
        .bind(&resolved_context_id)
        .bind(team_run_status_to_str(&status))
        .bind(input_json)
        .bind(now)
        .execute(&mut *tx)
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

    pub async fn cancel_run(&self, run_id: &str) -> anyhow::Result<TeamRunRecord> {
        let now = chrono::Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let result = sqlx::query(
            r#"
            UPDATE team_runs
            SET status = 'canceled', ended_at = COALESCE(ended_at, ?1)
            WHERE id = ?2 AND status NOT IN ('completed', 'failed', 'canceled')
            "#,
        )
        .bind(now)
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
        let mut archive_events = Vec::new();

        if result.rows_affected() > 0 {
            let (team_id, run_input) = load_run_status_sync_meta_tx(&mut tx, run_id).await?;
            let active_steps = sqlx::query(
                r#"
                SELECT id, step_key
                FROM team_steps
                WHERE run_id = ?1 AND status NOT IN ('completed', 'failed', 'canceled')
                "#,
            )
            .bind(run_id)
            .fetch_all(&mut *tx)
            .await?;

            for step in active_steps {
                let step_id: String = sqlx::Row::get(&step, "id");
                let step_key: String = sqlx::Row::get(&step, "step_key");
                let step_update = sqlx::query(
                    r#"
                    UPDATE team_steps
                    SET status = 'canceled', ended_at = COALESCE(ended_at, ?1)
                    WHERE id = ?2 AND status NOT IN ('completed', 'failed', 'canceled')
                    "#,
                )
                .bind(now)
                .bind(&step_id)
                .execute(&mut *tx)
                .await?;
                if step_update.rows_affected() == 0 {
                    continue;
                }

                let step_payload = serde_json::json!({
                    "step_id": step_id,
                    "step_key": step_key,
                    "status": "canceled",
                });
                let event = Self::append_run_event_tx(
                    &mut tx,
                    run_id,
                    Some(&step_id),
                    "step_canceled",
                    now,
                    &step_payload,
                )
                .await?;
                archive_events.push(event);
            }

            let payload = serde_json::json!({ "status": "canceled" });
            let event =
                Self::append_run_event_tx(&mut tx, run_id, None, "run_canceled", now, &payload)
                    .await?;
            archive_events.push(event);
            sync_linked_task_status_tx(
                &mut tx,
                &team_id,
                &run_input,
                TeamTaskStatus::Canceled,
                now,
                true,
            )
            .await?;
        }
        tx.commit().await?;
        self.spawn_archive_team_run_events(archive_events);

        self.get_run(run_id).await
    }

    pub async fn list_run_events(
        &self,
        run_id: &str,
        limit: i64,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<TeamRunEventRecord>> {
        let rows = if let Some(before_id) = before_id {
            sqlx::query(
                r#"
                SELECT id, run_id, step_id, event_type, ts, payload_json
                FROM team_run_events
                WHERE run_id = ?1 AND id < ?2
                ORDER BY id DESC
                LIMIT ?3
                "#,
            )
            .bind(run_id)
            .bind(before_id)
            .bind(limit)
            .fetch_all(&self.db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, run_id, step_id, event_type, ts, payload_json
                FROM team_run_events
                WHERE run_id = ?1
                ORDER BY id DESC
                LIMIT ?2
                "#,
            )
            .bind(run_id)
            .bind(limit)
            .fetch_all(&self.db)
            .await?
        };

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            events.push(parse_run_event_row(&row)?);
        }
        events.reverse();
        Ok(events)
    }

    pub async fn list_actor_messages_for_run(
        &self,
        run_id: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<TeamActorMessageRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                run_id,
                from_actor_id,
                from_peer_id,
                to_actor_id,
                to_peer_id,
                channel,
                transport,
                route_json,
                payload_json,
                message_kind,
                handling_disposition,
                handled_by_actor_id,
                handled_at,
                status,
                created_at,
                delivered_at
            FROM team_actor_messages
            WHERE run_id = ?1
            ORDER BY id DESC
            LIMIT ?2
            "#,
        )
        .bind(run_id)
        .bind(limit.max(1))
        .fetch_all(&self.db)
        .await?;
        let mut messages = Vec::with_capacity(rows.len());
        for row in rows {
            messages.push(parse_team_actor_message_row(&row)?);
        }
        mailbox::enrich_actor_messages(&self.db, &mut messages).await?;
        Ok(messages)
    }
}
