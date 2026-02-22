mod codec;
mod mailbox;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

pub use agenthub_team_domain::TeamRunResumeError;
use chrono::Utc;
use serde_json::Value;
use sqlx::{QueryBuilder, Row, SqlitePool};
use uuid::Uuid;

pub use mailbox::{SendActorMessageInput, TeamRemoteRelayWorkerSettings};

use self::codec::{
    parse_run_event_row, parse_team_actor_message_row, parse_team_conversation_message_row,
    parse_team_conversation_row, parse_team_definition_row, parse_team_main_task_row,
    parse_team_run_row, parse_team_step_row, team_main_task_status_to_str, team_run_status_to_str,
    team_step_status_to_str,
};
use super::{
    TeamActorMessageRecord, TeamConversationMessageRecord, TeamConversationRecord,
    TeamDefinitionConfig, TeamDefinitionRecord, TeamMainTaskRecord, TeamMainTaskStatus,
    TeamRunEventRecord, TeamRunRecord, TeamRunStatus, TeamStepRecord, TeamStepStatus,
};

#[derive(Clone)]
pub struct TeamManager {
    db: SqlitePool,
}

impl TeamManager {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    #[cfg(test)]
    pub async fn create_team(
        &self,
        config: TeamDefinitionConfig,
    ) -> anyhow::Result<TeamDefinitionRecord> {
        self.create_team_with_owner(config, None).await
    }

    pub async fn create_team_with_owner(
        &self,
        config: TeamDefinitionConfig,
        owner_user_id: Option<&str>,
    ) -> anyhow::Result<TeamDefinitionRecord> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        let spec_json = serde_json::to_string(&config.spec)?;
        sqlx::query(
            r#"
            INSERT INTO team_definitions (
                id,
                name,
                description,
                spec_json,
                owner_user_id,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(&id)
        .bind(&config.name)
        .bind(&config.description)
        .bind(spec_json)
        .bind(owner_user_id)
        .bind(now)
        .bind(now)
        .execute(&self.db)
        .await?;

        Ok(TeamDefinitionRecord {
            id,
            name: config.name,
            description: config.description,
            spec: config.spec,
            owner_user_id: owner_user_id.map(str::to_string),
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn list_teams(&self) -> anyhow::Result<Vec<TeamDefinitionRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, description, spec_json, owner_user_id, created_at, updated_at
            FROM team_definitions
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.db)
        .await?;

        let mut teams = Vec::with_capacity(rows.len());
        for row in rows {
            teams.push(parse_team_definition_row(&row)?);
        }
        Ok(teams)
    }

    pub async fn get_team(&self, team_id: &str) -> anyhow::Result<TeamDefinitionRecord> {
        let row = sqlx::query(
            r#"
            SELECT id, name, description, spec_json, owner_user_id, created_at, updated_at
            FROM team_definitions
            WHERE id = ?1
            "#,
        )
        .bind(team_id)
        .fetch_one(&self.db)
        .await?;
        parse_team_definition_row(&row)
    }

    pub async fn delete_team(&self, team_id: &str) -> anyhow::Result<TeamDefinitionRecord> {
        let mut tx = self.db.begin().await?;
        let team_row = sqlx::query(
            r#"
            SELECT id, name, description, spec_json, owner_user_id, created_at, updated_at
            FROM team_definitions
            WHERE id = ?1
            "#,
        )
        .bind(team_id)
        .fetch_one(&mut *tx)
        .await?;
        let team = parse_team_definition_row(&team_row)?;

        sqlx::query(
            r#"
            DELETE FROM team_actor_messages
            WHERE run_id IN (
                SELECT id FROM team_runs WHERE team_id = ?1
            )
            "#,
        )
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM team_conversation_messages
            WHERE conversation_id IN (
                SELECT id FROM team_conversations WHERE team_id = ?1
            )
            "#,
        )
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM team_conversations WHERE team_id = ?1")
            .bind(team_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM team_main_tasks WHERE team_id = ?1")
            .bind(team_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            r#"
            DELETE FROM team_run_events
            WHERE run_id IN (
                SELECT id FROM team_runs WHERE team_id = ?1
            )
            "#,
        )
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM team_steps
            WHERE run_id IN (
                SELECT id FROM team_runs WHERE team_id = ?1
            )
            "#,
        )
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM team_runs WHERE team_id = ?1")
            .bind(team_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM team_definitions WHERE id = ?1")
            .bind(team_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(team)
    }

    pub async fn create_main_task(
        &self,
        team_id: &str,
        title: &str,
        created_by_actor_id: &str,
        context: Value,
        conversation_mode: &str,
        topic: Option<&str>,
    ) -> anyhow::Result<(TeamMainTaskRecord, TeamConversationRecord)> {
        let now = Utc::now().timestamp();
        let task_id = Uuid::new_v4().to_string();
        let conversation_id = Uuid::new_v4().to_string();
        let status = TeamMainTaskStatus::Open;
        let context_json = redact_sensitive_json(&context).to_string();
        let topic = topic.map(str::trim).filter(|value| !value.is_empty());

        let mut tx = self.db.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO team_main_tasks (
                id, team_id, title, status, created_by_actor_id, context_json, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(&task_id)
        .bind(team_id)
        .bind(title)
        .bind(team_main_task_status_to_str(&status))
        .bind(created_by_actor_id)
        .bind(context_json)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO team_conversations (
                id, team_id, main_task_id, mode, topic, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(&conversation_id)
        .bind(team_id)
        .bind(&task_id)
        .bind(conversation_mode)
        .bind(topic)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        let task = self.get_main_task(&task_id).await?;
        let conversation = self.get_main_task_conversation(&task_id).await?;
        Ok((task, conversation))
    }

    pub async fn list_main_tasks(
        &self,
        team_id: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<TeamMainTaskRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                team_id,
                title,
                status,
                created_by_actor_id,
                context_json,
                created_at,
                updated_at
            FROM team_main_tasks
            WHERE team_id = ?1
            ORDER BY updated_at DESC, id DESC
            LIMIT ?2
            "#,
        )
        .bind(team_id)
        .bind(limit.max(1))
        .fetch_all(&self.db)
        .await?;
        let mut tasks = Vec::with_capacity(rows.len());
        for row in rows {
            tasks.push(parse_team_main_task_row(&row)?);
        }
        Ok(tasks)
    }

    pub async fn get_main_task(&self, main_task_id: &str) -> anyhow::Result<TeamMainTaskRecord> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                team_id,
                title,
                status,
                created_by_actor_id,
                context_json,
                created_at,
                updated_at
            FROM team_main_tasks
            WHERE id = ?1
            "#,
        )
        .bind(main_task_id)
        .fetch_one(&self.db)
        .await?;
        parse_team_main_task_row(&row)
    }

    pub async fn get_main_task_conversation(
        &self,
        main_task_id: &str,
    ) -> anyhow::Result<TeamConversationRecord> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                team_id,
                main_task_id,
                mode,
                topic,
                created_at,
                updated_at
            FROM team_conversations
            WHERE main_task_id = ?1
            "#,
        )
        .bind(main_task_id)
        .fetch_one(&self.db)
        .await?;
        parse_team_conversation_row(&row)
    }

    pub async fn append_main_task_conversation_message(
        &self,
        main_task_id: &str,
        from_actor_id: &str,
        to_actor_id: Option<&str>,
        route: &str,
        payload: Value,
    ) -> anyhow::Result<TeamConversationMessageRecord> {
        let now = Utc::now().timestamp();
        let conversation = self.get_main_task_conversation(main_task_id).await?;
        let redacted_payload = redact_sensitive_json(&payload);
        let payload_json = redacted_payload.to_string();
        let to_actor_id = to_actor_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let result = sqlx::query(
            r#"
            INSERT INTO team_conversation_messages (
                conversation_id,
                main_task_id,
                from_actor_id,
                to_actor_id,
                route,
                payload_json,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(&conversation.id)
        .bind(main_task_id)
        .bind(from_actor_id)
        .bind(to_actor_id.as_deref())
        .bind(route)
        .bind(payload_json)
        .bind(now)
        .execute(&self.db)
        .await?;

        Ok(TeamConversationMessageRecord {
            message_id: result.last_insert_rowid(),
            conversation_id: conversation.id,
            main_task_id: main_task_id.to_string(),
            from_actor_id: from_actor_id.to_string(),
            to_actor_id,
            route: route.to_string(),
            payload: redacted_payload,
            created_at: now,
        })
    }

    pub async fn list_main_task_conversation_messages(
        &self,
        main_task_id: &str,
        limit: i64,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<TeamConversationMessageRecord>> {
        let conversation = self.get_main_task_conversation(main_task_id).await?;
        let mut builder = QueryBuilder::<sqlx::Sqlite>::new(
            r#"
            SELECT
                id,
                conversation_id,
                main_task_id,
                from_actor_id,
                to_actor_id,
                route,
                payload_json,
                created_at
            FROM team_conversation_messages
            WHERE conversation_id = "#,
        );
        builder.push_bind(&conversation.id);
        if let Some(before_id) = before_id {
            builder.push(" AND id < ");
            builder.push_bind(before_id);
        }
        builder.push(" ORDER BY id DESC LIMIT ");
        builder.push_bind(limit.max(1));

        let rows = builder.build().fetch_all(&self.db).await?;
        let mut messages = Vec::with_capacity(rows.len());
        for row in rows {
            messages.push(parse_team_conversation_message_row(&row)?);
        }
        messages.reverse();
        Ok(messages)
    }

    pub async fn create_run(
        &self,
        team_id: &str,
        context_id: Option<&str>,
        input: Value,
    ) -> anyhow::Result<TeamRunRecord> {
        let run_id = Uuid::new_v4().to_string();
        let resolved_context_id = context_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = Utc::now().timestamp();
        let status = TeamRunStatus::Submitted;
        let input_json = serde_json::to_string(&input)?;

        let mut tx = self.db.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO team_runs (id, team_id, context_id, status, input_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
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
        .bind(payload.to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(TeamRunRecord {
            id: run_id,
            team_id: team_id.to_string(),
            context_id: resolved_context_id,
            status,
            input,
            created_at: now,
            started_at: None,
            ended_at: None,
        })
    }

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

    #[allow(dead_code)]
    pub async fn submit_step(
        &self,
        run_id: &str,
        step_key: &str,
        member_id: &str,
        depends_on: Vec<String>,
        input: Option<Value>,
    ) -> anyhow::Result<TeamStepRecord> {
        let step_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        let status = TeamStepStatus::Submitted;
        let depends_on_json = serde_json::to_string(&depends_on)?;
        let input_json = input.as_ref().map(serde_json::to_string).transpose()?;

        let mut tx = self.db.begin().await?;
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
        .bind(step_key)
        .bind(member_id)
        .bind(team_step_status_to_str(&status))
        .bind(depends_on_json)
        .bind(input_json)
        .execute(&mut *tx)
        .await?;

        let payload = serde_json::json!({
            "step_id": step_id,
            "step_key": step_key,
            "member_id": member_id,
            "status": team_step_status_to_str(&status),
        });
        sqlx::query(
            r#"
            INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(run_id)
        .bind(&step_id)
        .bind("step_submitted")
        .bind(now)
        .bind(payload.to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        self.get_step(&step_id).await
    }

    #[allow(dead_code)]
    pub async fn get_step(&self, step_id: &str) -> anyhow::Result<TeamStepRecord> {
        let row = sqlx::query(
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
        .fetch_one(&self.db)
        .await?;
        parse_team_step_row(&row)
    }

    pub async fn list_steps(&self, run_id: &str) -> anyhow::Result<Vec<TeamStepRecord>> {
        let rows = sqlx::query(
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
            WHERE run_id = ?1
            ORDER BY attempt ASC, step_key ASC, id ASC
            "#,
        )
        .bind(run_id)
        .fetch_all(&self.db)
        .await?;

        let mut steps = Vec::with_capacity(rows.len());
        for row in rows {
            steps.push(parse_team_step_row(&row)?);
        }
        Ok(steps)
    }

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
        Ok(runs)
    }

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
        Ok(runs)
    }

    pub async fn get_agent_session_status(
        &self,
        session_id: &str,
    ) -> anyhow::Result<Option<String>> {
        let row = sqlx::query(
            r#"
            SELECT status
            FROM agent_sessions
            WHERE id = ?1
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(|row| row.get("status")))
    }

    #[allow(dead_code)]
    pub async fn start_step(
        &self,
        step_id: &str,
        remote_task_id: Option<&str>,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                status = 'working',
                remote_task_id = COALESCE(?1, remote_task_id),
                started_at = COALESCE(started_at, ?2)
            WHERE id = ?3 AND status IN ('submitted', 'input_required')
            "#,
        )
        .bind(remote_task_id)
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;

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
        .fetch_one(&mut *tx)
        .await?;
        let step = parse_team_step_row(&step_row)?;

        if update.rows_affected() > 0 {
            let run_update = sqlx::query(
                r#"
                UPDATE team_runs
                SET status = 'working', started_at = COALESCE(started_at, ?1)
                WHERE id = ?2 AND status IN ('submitted', 'input_required')
                "#,
            )
            .bind(now)
            .bind(&step.run_id)
            .execute(&mut *tx)
            .await?;
            if run_update.rows_affected() > 0 {
                let run_payload = serde_json::json!({
                    "status": "working",
                });
                sqlx::query(
                    r#"
                    INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                    VALUES (?1, NULL, ?2, ?3, ?4)
                    "#,
                )
                .bind(&step.run_id)
                .bind("run_working")
                .bind(now)
                .bind(run_payload.to_string())
                .execute(&mut *tx)
                .await?;
            }

            let step_payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "status": "working",
                "remote_task_id": step.remote_task_id,
            });
            sqlx::query(
                r#"
                INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
            )
            .bind(&step.run_id)
            .bind(&step.id)
            .bind("step_working")
            .bind(now)
            .bind(step_payload.to_string())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(step)
    }

    #[allow(dead_code)]
    pub async fn set_step_input_required(
        &self,
        step_id: &str,
        reason: Option<&str>,
        input: Option<Value>,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let input_json = input.as_ref().map(serde_json::to_string).transpose()?;
        let mut tx = self.db.begin().await?;
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                status = 'input_required',
                input_json = COALESCE(?1, input_json),
                error_text = COALESCE(?2, error_text),
                started_at = COALESCE(started_at, ?3)
            WHERE id = ?4 AND status IN ('submitted', 'working')
            "#,
        )
        .bind(input_json)
        .bind(reason)
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;

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
        .fetch_one(&mut *tx)
        .await?;
        let step = parse_team_step_row(&step_row)?;

        if update.rows_affected() > 0 {
            let run_update = sqlx::query(
                r#"
                UPDATE team_runs
                SET status = 'input_required', started_at = COALESCE(started_at, ?1)
                WHERE id = ?2 AND status IN ('submitted', 'working')
                "#,
            )
            .bind(now)
            .bind(&step.run_id)
            .execute(&mut *tx)
            .await?;
            if run_update.rows_affected() > 0 {
                let run_payload = serde_json::json!({
                    "status": "input_required",
                    "step_id": step.id,
                    "step_key": step.step_key,
                });
                sqlx::query(
                    r#"
                    INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                    VALUES (?1, NULL, ?2, ?3, ?4)
                    "#,
                )
                .bind(&step.run_id)
                .bind("run_input_required")
                .bind(now)
                .bind(run_payload.to_string())
                .execute(&mut *tx)
                .await?;
            }

            let step_payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "status": "input_required",
                "reason": step.error_text,
                "input": step.input,
            });
            sqlx::query(
                r#"
                INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
            )
            .bind(&step.run_id)
            .bind(&step.id)
            .bind("step_input_required")
            .bind(now)
            .bind(step_payload.to_string())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(step)
    }

    #[allow(dead_code)]
    pub async fn resume_step(
        &self,
        step_id: &str,
        input: Option<Value>,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let input_json = input.as_ref().map(serde_json::to_string).transpose()?;
        let mut tx = self.db.begin().await?;
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                status = 'working',
                input_json = COALESCE(?1, input_json),
                error_text = NULL,
                started_at = COALESCE(started_at, ?2)
            WHERE id = ?3 AND status = 'input_required'
            "#,
        )
        .bind(input_json)
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;

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
        .fetch_one(&mut *tx)
        .await?;
        let step = parse_team_step_row(&step_row)?;

        if update.rows_affected() > 0 {
            let run_update = sqlx::query(
                r#"
                UPDATE team_runs
                SET status = 'working', started_at = COALESCE(started_at, ?1)
                WHERE id = ?2 AND status = 'input_required'
                "#,
            )
            .bind(now)
            .bind(&step.run_id)
            .execute(&mut *tx)
            .await?;
            if run_update.rows_affected() > 0 {
                let run_payload = serde_json::json!({
                    "status": "working",
                });
                sqlx::query(
                    r#"
                    INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                    VALUES (?1, NULL, ?2, ?3, ?4)
                    "#,
                )
                .bind(&step.run_id)
                .bind("run_working")
                .bind(now)
                .bind(run_payload.to_string())
                .execute(&mut *tx)
                .await?;
            }

            let step_payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "status": "working",
                "remote_task_id": step.remote_task_id,
            });
            sqlx::query(
                r#"
                INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
            )
            .bind(&step.run_id)
            .bind(&step.id)
            .bind("step_resumed")
            .bind(now)
            .bind(step_payload.to_string())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(step)
    }

    #[allow(dead_code)]
    pub async fn complete_step(
        &self,
        step_id: &str,
        output: Option<Value>,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let output_json = output.as_ref().map(serde_json::to_string).transpose()?;
        let mut tx = self.db.begin().await?;
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                status = 'completed',
                output_json = ?1,
                ended_at = COALESCE(ended_at, ?2)
            WHERE id = ?3 AND status IN ('working', 'input_required')
            "#,
        )
        .bind(output_json)
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;

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
        .fetch_one(&mut *tx)
        .await?;
        let step = parse_team_step_row(&step_row)?;

        if update.rows_affected() > 0 {
            let payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "status": "completed",
            });
            sqlx::query(
                r#"
                INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
            )
            .bind(&step.run_id)
            .bind(&step.id)
            .bind("step_completed")
            .bind(now)
            .bind(payload.to_string())
            .execute(&mut *tx)
            .await?;

            let non_completed_count: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM team_steps
                WHERE run_id = ?1 AND status <> 'completed'
                "#,
            )
            .bind(&step.run_id)
            .fetch_one(&mut *tx)
            .await?;

            if non_completed_count == 0 {
                let run_update = sqlx::query(
                    r#"
                    UPDATE team_runs
                    SET status = 'completed', ended_at = COALESCE(ended_at, ?1)
                    WHERE id = ?2 AND status IN ('submitted', 'working', 'input_required')
                    "#,
                )
                .bind(now)
                .bind(&step.run_id)
                .execute(&mut *tx)
                .await?;

                if run_update.rows_affected() > 0 {
                    let run_payload = serde_json::json!({
                        "status": "completed",
                    });
                    sqlx::query(
                        r#"
                        INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                        VALUES (?1, NULL, ?2, ?3, ?4)
                        "#,
                    )
                    .bind(&step.run_id)
                    .bind("run_completed")
                    .bind(now)
                    .bind(run_payload.to_string())
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }

        tx.commit().await?;
        Ok(step)
    }

    #[allow(dead_code)]
    pub async fn fail_step(
        &self,
        step_id: &str,
        error_text: &str,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                status = 'failed',
                error_text = ?1,
                ended_at = COALESCE(ended_at, ?2)
            WHERE id = ?3 AND status IN ('submitted', 'working', 'input_required')
            "#,
        )
        .bind(error_text)
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;

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
        .fetch_one(&mut *tx)
        .await?;
        let step = parse_team_step_row(&step_row)?;

        if update.rows_affected() > 0 {
            let payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "status": "failed",
                "error_text": step.error_text,
            });
            sqlx::query(
                r#"
                INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
            )
            .bind(&step.run_id)
            .bind(&step.id)
            .bind("step_failed")
            .bind(now)
            .bind(payload.to_string())
            .execute(&mut *tx)
            .await?;

            let run_update = sqlx::query(
                r#"
                UPDATE team_runs
                SET status = 'failed', ended_at = COALESCE(ended_at, ?1)
                WHERE id = ?2 AND status IN ('submitted', 'working', 'input_required')
                "#,
            )
            .bind(now)
            .bind(&step.run_id)
            .execute(&mut *tx)
            .await?;

            if run_update.rows_affected() > 0 {
                let run_payload = serde_json::json!({
                    "status": "failed",
                });
                sqlx::query(
                    r#"
                    INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                    VALUES (?1, NULL, ?2, ?3, ?4)
                    "#,
                )
                .bind(&step.run_id)
                .bind("run_failed")
                .bind(now)
                .bind(run_payload.to_string())
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;
        Ok(step)
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
        parse_team_run_row(&row)
    }

    pub async fn update_team_spec_if_unchanged(
        &self,
        team_id: &str,
        expected_updated_at: i64,
        spec: Value,
    ) -> anyhow::Result<Option<TeamDefinitionRecord>> {
        let now = Utc::now().timestamp();
        let spec_json = serde_json::to_string(&spec)?;
        let update = sqlx::query(
            r#"
            UPDATE team_definitions
            SET spec_json = ?1, updated_at = ?2
            WHERE id = ?3 AND updated_at = ?4
            "#,
        )
        .bind(spec_json)
        .bind(now)
        .bind(team_id)
        .bind(expected_updated_at)
        .execute(&self.db)
        .await?;
        if update.rows_affected() == 0 {
            let exists: Option<i64> = sqlx::query_scalar(
                r#"
                SELECT 1
                FROM team_definitions
                WHERE id = ?1
                "#,
            )
            .bind(team_id)
            .fetch_optional(&self.db)
            .await?;
            if exists.is_none() {
                return Err(sqlx::Error::RowNotFound.into());
            }
            return Ok(None);
        }
        self.get_team(team_id).await.map(Some)
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
        let ts = Utc::now().timestamp();
        sqlx::query(
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
        Ok(())
    }

    pub async fn cancel_run(&self, run_id: &str) -> anyhow::Result<TeamRunRecord> {
        let now = Utc::now().timestamp();
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

        if result.rows_affected() > 0 {
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
                let step_id: String = step.get("id");
                let step_key: String = step.get("step_key");
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
                sqlx::query(
                    r#"
                    INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    "#,
                )
                .bind(run_id)
                .bind(&step_id)
                .bind("step_canceled")
                .bind(now)
                .bind(step_payload.to_string())
                .execute(&mut *tx)
                .await?;
            }

            let payload = serde_json::json!({ "status": "canceled" });
            sqlx::query(
                r#"
                INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                VALUES (?1, NULL, ?2, ?3, ?4)
                "#,
            )
            .bind(run_id)
            .bind("run_canceled")
            .bind(now)
            .bind(payload.to_string())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

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
                to_actor_id,
                channel,
                transport,
                route_json,
                payload_json,
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
        Ok(messages)
    }

    pub async fn list_actor_message_status_counts(
        &self,
        run_id: &str,
    ) -> anyhow::Result<HashMap<String, i64>> {
        let rows = sqlx::query(
            r#"
            SELECT status, COUNT(*) AS cnt
            FROM team_actor_messages
            WHERE run_id = ?1
            GROUP BY status
            "#,
        )
        .bind(run_id)
        .fetch_all(&self.db)
        .await?;

        let mut counts = HashMap::with_capacity(rows.len());
        for row in rows {
            let status: String = row.get("status");
            let count: i64 = row.get("cnt");
            counts.insert(status, count);
        }
        Ok(counts)
    }

    pub async fn list_actor_pending_counts_by_actor(
        &self,
        run_id: &str,
    ) -> anyhow::Result<HashMap<String, i64>> {
        let rows = sqlx::query(
            r#"
            SELECT to_actor_id, COUNT(*) AS cnt
            FROM team_actor_messages
            WHERE run_id = ?1
              AND status = 'pending'
            GROUP BY to_actor_id
            "#,
        )
        .bind(run_id)
        .fetch_all(&self.db)
        .await?;

        let mut counts = HashMap::with_capacity(rows.len());
        for row in rows {
            let actor_id: String = row.get("to_actor_id");
            let count: i64 = row.get("cnt");
            counts.insert(actor_id, count);
        }
        Ok(counts)
    }
}

fn redact_sensitive_json(value: &Value) -> Value {
    const REDACTED: &str = "[redacted]";
    match value {
        Value::Object(map) => {
            let mut redacted = serde_json::Map::with_capacity(map.len());
            for (key, child) in map {
                if is_sensitive_key(key) {
                    redacted.insert(key.clone(), Value::String(REDACTED.to_string()));
                } else {
                    redacted.insert(key.clone(), redact_sensitive_json(child));
                }
            }
            Value::Object(redacted)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_sensitive_json).collect()),
        _ => value.clone(),
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    normalized.contains("token")
        || normalized.contains("secret")
        || normalized.contains("password")
        || normalized.contains("authorization")
        || normalized.contains("api_key")
        || normalized.contains("apikey")
}
