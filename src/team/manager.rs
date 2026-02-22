mod codec;
mod mailbox;

#[cfg(test)]
mod tests;

use std::{collections::HashMap, path::PathBuf};

pub use agenthub_team_domain::TeamRunResumeError;
use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};
use uuid::Uuid;

pub use mailbox::{SendActorMessageInput, TeamRemoteRelayWorkerSettings};

use self::codec::{
    parse_run_event_row, parse_team_actor_message_row, parse_team_conversation_message_row,
    parse_team_conversation_row, parse_team_definition_row, parse_team_main_task_row,
    parse_team_member_continuity_state_row, parse_team_run_row, parse_team_step_row,
    team_main_task_status_to_str, team_run_status_to_str, team_step_status_to_str,
};
use super::{
    TEAM_RUN_CONTINUITY_MODE_VALUES, TeamActorMessageRecord, TeamConversationMessageRecord,
    TeamConversationRecord, TeamDefinitionConfig, TeamDefinitionRecord, TeamMainTaskRecord,
    TeamMainTaskStatus, TeamMemberContinuityStateRecord, TeamRunEventRecord, TeamRunRecord,
    TeamRunStatus, TeamStepRecord, TeamStepStatus,
};

#[derive(Clone)]
pub struct TeamManager {
    db: SqlitePool,
}

const CONTINUITY_MODE_DEFAULT: &str = "inherit_recent";
const CONTINUITY_MODE_RESET: &str = "reset";
const CONTINUITY_MAX_SUMMARY_CHARS: usize = 2048;
const CONTINUITY_MAX_HISTORY_CHARS: usize = 4096;
const CONTINUITY_ARTIFACT_KIND_OUTPUT: &str = "continuity_output";
const MEMORY_FLUSH_MAX_EVENTS_DEFAULT: i64 = 200;
const MEMORY_FLUSH_MAX_EVENTS_MAX: i64 = 1000;
const MEMORY_FLUSH_MAX_SUMMARY_CHARS: usize = 2048;
const MEMORY_FLUSH_MAX_EXCERPT_CHARS: usize = 700;
const MEMORY_FLUSH_ARTIFACT_KIND: &str = "memory_flush";

#[derive(Debug, Clone)]
pub struct TeamMemoryFlushRequest {
    pub member_id: String,
    pub session_id: Option<String>,
    pub trigger: String,
    pub max_events: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct TeamMemoryFlushResult {
    pub status: String,
    pub run_id: String,
    pub team_id: String,
    pub member_id: String,
    pub session_id: Option<String>,
    pub trigger: String,
    pub reason: Option<String>,
    pub artifact_pointer: Option<Value>,
    pub event_id_from: Option<i64>,
    pub event_id_to: Option<i64>,
    pub flushed_events: i64,
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

        sqlx::query("DELETE FROM team_member_continuity_state WHERE team_id = ?1")
            .bind(team_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM team_context_artifacts WHERE team_id = ?1")
            .bind(team_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM team_context_flush_checkpoint WHERE team_id = ?1")
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
        let input = normalize_run_input_continuity(input);
        let input_json = serde_json::to_string(&input)?;
        let continuity_mode = extract_continuity_mode_from_input(&input);

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
            "continuity_mode": continuity_mode,
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

    pub async fn get_member_continuity_state(
        &self,
        team_id: &str,
        member_id: &str,
    ) -> anyhow::Result<Option<TeamMemberContinuityStateRecord>> {
        let row = sqlx::query(
            r#"
            SELECT
                team_id,
                member_id,
                source_run_id,
                source_session_id,
                summary_text,
                history_window_json,
                updated_at
            FROM team_member_continuity_state
            WHERE team_id = ?1 AND member_id = ?2
            "#,
        )
        .bind(team_id)
        .bind(member_id)
        .fetch_optional(&self.db)
        .await?;
        row.as_ref()
            .map(parse_team_member_continuity_state_row)
            .transpose()
    }

    async fn upsert_member_continuity_state_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        continuity_state: &TeamMemberContinuityStateRecord,
    ) -> anyhow::Result<()> {
        let history_window_json = serde_json::to_string(&continuity_state.history_window)?;
        sqlx::query(
            r#"
            INSERT INTO team_member_continuity_state (
                team_id,
                member_id,
                source_run_id,
                source_session_id,
                summary_text,
                history_window_json,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(team_id, member_id)
            DO UPDATE SET
                source_run_id = excluded.source_run_id,
                source_session_id = excluded.source_session_id,
                summary_text = excluded.summary_text,
                history_window_json = excluded.history_window_json,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&continuity_state.team_id)
        .bind(&continuity_state.member_id)
        .bind(&continuity_state.source_run_id)
        .bind(continuity_state.source_session_id.as_deref())
        .bind(&continuity_state.summary_text)
        .bind(history_window_json)
        .bind(continuity_state.updated_at)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    async fn append_run_event_tx(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        run_id: &str,
        step_id: Option<&str>,
        event_type: &str,
        ts: i64,
        payload: &Value,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(run_id)
        .bind(step_id)
        .bind(event_type)
        .bind(ts)
        .bind(payload.to_string())
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    async fn persist_continuity_artifact_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        team_id: &str,
        run_id: &str,
        member_id: &str,
        session_id: Option<&str>,
        snapshot: &ContinuitySnapshot,
        now: i64,
    ) -> anyhow::Result<Option<ContextArtifactPointer>> {
        let artifact_payload = serde_json::json!({
            "schema_version": 1,
            "team_id": team_id,
            "run_id": run_id,
            "member_id": member_id,
            "session_id": session_id,
            "summary_text": snapshot.summary_text,
            "redacted_output": snapshot.redacted_output,
            "created_at": now,
        });
        self.persist_context_artifact_tx(
            tx,
            team_id,
            run_id,
            member_id,
            session_id,
            CONTINUITY_ARTIFACT_KIND_OUTPUT,
            artifact_payload,
            now,
        )
        .await
    }

    async fn persist_context_artifact_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        team_id: &str,
        run_id: &str,
        member_id: &str,
        session_id: Option<&str>,
        artifact_kind: &str,
        artifact_payload: Value,
        now: i64,
    ) -> anyhow::Result<Option<ContextArtifactPointer>> {
        let Some(workdir) = sqlx::query_scalar::<_, String>(
            r#"
            SELECT workdir
            FROM agents
            WHERE id = ?1
            "#,
        )
        .bind(member_id)
        .fetch_optional(&mut **tx)
        .await?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty()) else {
            return Ok(None);
        };

        let artifact_seq: i64 = sqlx::query_scalar(
            r#"
            SELECT COALESCE(MAX(artifact_seq), 0) + 1
            FROM team_context_artifacts
            WHERE run_id = ?1
            "#,
        )
        .bind(run_id)
        .fetch_one(&mut **tx)
        .await?;

        let run_context_dir = PathBuf::from(&workdir)
            .join(".cache")
            .join("context")
            .join("run")
            .join(run_id);
        std::fs::create_dir_all(&run_context_dir)?;

        let file_name = format!("artifact-{artifact_seq}-{artifact_kind}.json");
        let absolute_path = run_context_dir.join(&file_name);
        let relative_path = format!(".cache/context/run/{run_id}/{file_name}");
        let artifact_bytes = serde_json::to_vec(&artifact_payload)?;
        std::fs::write(&absolute_path, &artifact_bytes)?;
        let artifact_size_bytes = i64::try_from(artifact_bytes.len()).unwrap_or(i64::MAX);
        let content_checksum = format!("{:x}", Sha256::digest(&artifact_bytes));
        let absolute_path_string = absolute_path.to_string_lossy().to_string();

        sqlx::query(
            r#"
            INSERT INTO team_context_artifacts (
                team_id,
                run_id,
                member_id,
                session_id,
                artifact_seq,
                artifact_kind,
                artifact_path,
                artifact_size_bytes,
                content_checksum,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(team_id)
        .bind(run_id)
        .bind(member_id)
        .bind(session_id)
        .bind(artifact_seq)
        .bind(artifact_kind)
        .bind(absolute_path_string)
        .bind(artifact_size_bytes)
        .bind(&content_checksum)
        .bind(now)
        .execute(&mut **tx)
        .await?;

        Ok(Some(ContextArtifactPointer {
            artifact_kind: artifact_kind.to_string(),
            relative_path,
            artifact_size_bytes,
            content_checksum,
        }))
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

            let run_meta_row = sqlx::query(
                r#"
                SELECT team_id, input_json
                FROM team_runs
                WHERE id = ?1
                "#,
            )
            .bind(&step.run_id)
            .fetch_one(&mut *tx)
            .await?;
            let team_id: String = run_meta_row.get("team_id");
            let run_input_json: String = run_meta_row.get("input_json");
            let run_input: Value =
                serde_json::from_str(&run_input_json).unwrap_or_else(|_| serde_json::json!({}));
            let continuity_mode = extract_continuity_mode_from_input(&run_input);
            let mut continuity_snapshot = build_continuity_snapshot(step.output.as_ref());
            let mut artifact_pointer_for_event: Option<Value> = None;
            let mut artifact_offload_status = "inline";
            let mut artifact_offload_reason: Option<&str> = None;
            if should_offload_continuity_output(continuity_snapshot.redacted_output_text.as_str()) {
                match self
                    .persist_continuity_artifact_tx(
                        &mut tx,
                        &team_id,
                        &step.run_id,
                        &step.member_id,
                        step.remote_task_id.as_deref(),
                        &continuity_snapshot,
                        now,
                    )
                    .await
                {
                    Ok(Some(pointer)) => {
                        let pointer_payload = serde_json::json!({
                            "kind": pointer.artifact_kind,
                            "path": pointer.relative_path,
                            "size_bytes": pointer.artifact_size_bytes,
                            "checksum": pointer.content_checksum,
                        });
                        if let Some(history_obj) =
                            continuity_snapshot.history_window.as_object_mut()
                        {
                            history_obj
                                .insert("artifact_pointer".to_string(), pointer_payload.clone());
                        }
                        artifact_pointer_for_event = Some(pointer_payload);
                        artifact_offload_status = "persisted";
                    }
                    Ok(None) => {
                        artifact_offload_reason = Some("agent_workdir_missing");
                    }
                    Err(err) => {
                        tracing::warn!(
                            run_id = %step.run_id,
                            step_id = %step.id,
                            member_id = %step.member_id,
                            "team manager failed to persist continuity artifact: {}",
                            err
                        );
                        artifact_offload_reason = Some("artifact_write_failed");
                    }
                }
            }
            let continuity_state = TeamMemberContinuityStateRecord {
                team_id: team_id.clone(),
                member_id: step.member_id.clone(),
                source_run_id: step.run_id.clone(),
                source_session_id: step.remote_task_id.clone(),
                summary_text: continuity_snapshot.summary_text,
                history_window: continuity_snapshot.history_window,
                updated_at: now,
            };
            Self::upsert_member_continuity_state_tx(&mut tx, &continuity_state).await?;

            let mut continuity_payload = serde_json::json!({
                "team_id": continuity_state.team_id,
                "member_id": continuity_state.member_id,
                "step_id": step.id,
                "step_key": step.step_key,
                "mode": continuity_mode,
                "source_run_id": continuity_state.source_run_id,
                "source_session_id": continuity_state.source_session_id,
                "summary_chars": continuity_state.summary_text.chars().count(),
                "artifact_offload_status": artifact_offload_status,
            });
            if let Some(payload_obj) = continuity_payload.as_object_mut() {
                if let Some(pointer_payload) = artifact_pointer_for_event {
                    payload_obj.insert("artifact_pointer".to_string(), pointer_payload);
                }
                if let Some(reason) = artifact_offload_reason {
                    payload_obj.insert(
                        "artifact_offload_reason".to_string(),
                        Value::String(reason.to_string()),
                    );
                }
            }
            sqlx::query(
                r#"
                INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
            )
            .bind(&step.run_id)
            .bind(&step.id)
            .bind("continuity_state_updated")
            .bind(now)
            .bind(continuity_payload.to_string())
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

    pub async fn flush_run_context(
        &self,
        run_id: &str,
        request: TeamMemoryFlushRequest,
    ) -> anyhow::Result<TeamMemoryFlushResult> {
        let member_id = request.member_id.trim().to_string();
        if member_id.is_empty() {
            return Err(anyhow::anyhow!("member_id is required"));
        }
        let trigger = normalize_memory_flush_trigger(request.trigger.as_str()).to_string();
        let max_events = normalize_memory_flush_max_events(request.max_events);
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;

        let run_meta_row = sqlx::query(
            r#"
            SELECT team_id
            FROM team_runs
            WHERE id = ?1
            "#,
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await?;
        let team_id: String = run_meta_row.get("team_id");

        let session_id = resolve_memory_flush_session_id_tx(
            &mut tx,
            run_id,
            member_id.as_str(),
            request.session_id.as_deref(),
        )
        .await?;

        Self::append_run_event_tx(
            &mut tx,
            run_id,
            None,
            "memory_flush_started",
            now,
            &serde_json::json!({
                "team_id": team_id,
                "run_id": run_id,
                "member_id": member_id,
                "session_id": session_id,
                "trigger": trigger,
                "ts": now,
            }),
        )
        .await?;

        let Some(session_id) = session_id else {
            let reason = "session_mapping_missing";
            Self::append_run_event_tx(
                &mut tx,
                run_id,
                None,
                "memory_flush_failed",
                now,
                &serde_json::json!({
                    "team_id": team_id,
                    "run_id": run_id,
                    "member_id": member_id,
                    "trigger": trigger,
                    "reason_code": reason,
                    "ts": now,
                }),
            )
            .await?;
            tx.commit().await?;
            return Ok(TeamMemoryFlushResult {
                status: "failed".to_string(),
                run_id: run_id.to_string(),
                team_id,
                member_id,
                session_id: None,
                trigger,
                reason: Some(reason.to_string()),
                artifact_pointer: None,
                event_id_from: None,
                event_id_to: None,
                flushed_events: 0,
            });
        };

        let checkpoint_event_id = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT last_event_id
            FROM team_context_flush_checkpoint
            WHERE run_id = ?1
              AND member_id = ?2
              AND session_id = ?3
            "#,
        )
        .bind(run_id)
        .bind(member_id.as_str())
        .bind(session_id.as_str())
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);

        let event_rows = sqlx::query(
            r#"
            SELECT id, stream, message
            FROM agent_events
            WHERE agent_id = ?1
              AND session_id = ?2
              AND id > ?3
            ORDER BY id ASC
            LIMIT ?4
            "#,
        )
        .bind(member_id.as_str())
        .bind(session_id.as_str())
        .bind(checkpoint_event_id)
        .bind(max_events)
        .fetch_all(&mut *tx)
        .await?;

        if event_rows.is_empty() {
            Self::append_run_event_tx(
                &mut tx,
                run_id,
                None,
                "memory_flush_noop",
                now,
                &serde_json::json!({
                    "team_id": team_id,
                    "run_id": run_id,
                    "member_id": member_id,
                    "session_id": session_id,
                    "trigger": trigger,
                    "reason": "no_new_events",
                    "ts": now,
                }),
            )
            .await?;
            tx.commit().await?;
            return Ok(TeamMemoryFlushResult {
                status: "noop".to_string(),
                run_id: run_id.to_string(),
                team_id,
                member_id,
                session_id: Some(session_id),
                trigger,
                reason: Some("no_new_events".to_string()),
                artifact_pointer: None,
                event_id_from: None,
                event_id_to: None,
                flushed_events: 0,
            });
        }

        let observations = event_rows
            .iter()
            .map(build_memory_flush_observation)
            .collect::<Vec<_>>();
        let event_id_from = event_rows
            .first()
            .map(|row| row.get::<i64, _>("id"))
            .unwrap_or(0);
        let event_id_to = event_rows
            .last()
            .map(|row| row.get::<i64, _>("id"))
            .unwrap_or(0);
        let summary_text = build_memory_flush_summary(observations.as_slice());
        let flush_payload = serde_json::json!({
            "schema_version": 1,
            "team_id": team_id,
            "run_id": run_id,
            "member_id": member_id,
            "session_id": session_id,
            "trigger": trigger,
            "source_event_range": {
                "from_exclusive": checkpoint_event_id,
                "to_inclusive": event_id_to,
            },
            "summary_text": summary_text,
            "observations": observations,
            "created_at": now,
        });

        let pointer = match self
            .persist_context_artifact_tx(
                &mut tx,
                team_id.as_str(),
                run_id,
                member_id.as_str(),
                Some(session_id.as_str()),
                MEMORY_FLUSH_ARTIFACT_KIND,
                flush_payload,
                now,
            )
            .await
        {
            Ok(Some(pointer)) => pointer,
            Ok(None) => {
                let reason = "agent_workdir_missing";
                Self::append_run_event_tx(
                    &mut tx,
                    run_id,
                    None,
                    "memory_flush_failed",
                    now,
                    &serde_json::json!({
                        "team_id": team_id,
                        "run_id": run_id,
                        "member_id": member_id,
                        "session_id": session_id,
                        "trigger": trigger,
                        "reason_code": reason,
                        "ts": now,
                    }),
                )
                .await?;
                tx.commit().await?;
                return Ok(TeamMemoryFlushResult {
                    status: "failed".to_string(),
                    run_id: run_id.to_string(),
                    team_id,
                    member_id,
                    session_id: Some(session_id),
                    trigger,
                    reason: Some(reason.to_string()),
                    artifact_pointer: None,
                    event_id_from: Some(event_id_from),
                    event_id_to: Some(event_id_to),
                    flushed_events: i64::try_from(event_rows.len()).unwrap_or(i64::MAX),
                });
            }
            Err(err) => {
                let reason = "artifact_write_failed";
                Self::append_run_event_tx(
                    &mut tx,
                    run_id,
                    None,
                    "memory_flush_failed",
                    now,
                    &serde_json::json!({
                        "team_id": team_id,
                        "run_id": run_id,
                        "member_id": member_id,
                        "session_id": session_id,
                        "trigger": trigger,
                        "reason_code": reason,
                        "error_excerpt": truncate_continuity_text(err.to_string().as_str(), 400),
                        "ts": now,
                    }),
                )
                .await?;
                tx.commit().await?;
                return Ok(TeamMemoryFlushResult {
                    status: "failed".to_string(),
                    run_id: run_id.to_string(),
                    team_id,
                    member_id,
                    session_id: Some(session_id),
                    trigger,
                    reason: Some(reason.to_string()),
                    artifact_pointer: None,
                    event_id_from: Some(event_id_from),
                    event_id_to: Some(event_id_to),
                    flushed_events: i64::try_from(event_rows.len()).unwrap_or(i64::MAX),
                });
            }
        };

        sqlx::query(
            r#"
            INSERT INTO team_context_flush_checkpoint (
                team_id,
                run_id,
                member_id,
                session_id,
                last_event_id,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(run_id, member_id, session_id)
            DO UPDATE SET
                last_event_id = excluded.last_event_id,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(team_id.as_str())
        .bind(run_id)
        .bind(member_id.as_str())
        .bind(session_id.as_str())
        .bind(event_id_to)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        let pointer_payload = serde_json::json!({
            "kind": pointer.artifact_kind,
            "path": pointer.relative_path,
            "size_bytes": pointer.artifact_size_bytes,
            "checksum": pointer.content_checksum,
        });
        Self::append_run_event_tx(
            &mut tx,
            run_id,
            None,
            "memory_flush_persisted",
            now,
            &serde_json::json!({
                "team_id": team_id,
                "run_id": run_id,
                "member_id": member_id,
                "session_id": session_id,
                "trigger": trigger,
                "artifact_pointer": pointer_payload,
                "artifact_size_bytes": pointer.artifact_size_bytes,
                "event_id_from": event_id_from,
                "event_id_to": event_id_to,
                "ts": now,
            }),
        )
        .await?;

        tx.commit().await?;
        Ok(TeamMemoryFlushResult {
            status: "persisted".to_string(),
            run_id: run_id.to_string(),
            team_id,
            member_id,
            session_id: Some(session_id),
            trigger,
            reason: None,
            artifact_pointer: Some(pointer_payload),
            event_id_from: Some(event_id_from),
            event_id_to: Some(event_id_to),
            flushed_events: i64::try_from(event_rows.len()).unwrap_or(i64::MAX),
        })
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

async fn resolve_memory_flush_session_id_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
    member_id: &str,
    requested_session_id: Option<&str>,
) -> anyhow::Result<Option<String>> {
    if let Some(session_id) = requested_session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(Some(session_id.to_string()));
    }

    let row = sqlx::query(
        r#"
        SELECT remote_task_id
        FROM team_steps
        WHERE run_id = ?1
          AND member_id = ?2
          AND remote_task_id IS NOT NULL
        ORDER BY COALESCE(ended_at, started_at, 0) DESC, attempt DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(run_id)
    .bind(member_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.and_then(|entry| {
        entry
            .get::<Option<String>, _>("remote_task_id")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }))
}

fn normalize_memory_flush_trigger(raw: &str) -> &'static str {
    match raw.trim() {
        "soft_threshold" => "soft_threshold",
        "hard_error" => "hard_error",
        _ => "manual",
    }
}

fn normalize_memory_flush_max_events(raw: Option<i64>) -> i64 {
    raw.unwrap_or(MEMORY_FLUSH_MAX_EVENTS_DEFAULT)
        .clamp(1, MEMORY_FLUSH_MAX_EVENTS_MAX)
}

fn build_memory_flush_observation(row: &sqlx::sqlite::SqliteRow) -> Value {
    let event_id = row.get::<i64, _>("id");
    let stream = row.get::<String, _>("stream");
    let message = row.get::<String, _>("message");
    if let Ok(message_json) = serde_json::from_str::<Value>(&message) {
        let redacted = redact_sensitive_json(&message_json);
        let observation_type = message_json
            .as_object()
            .and_then(|obj| obj.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("json_message");
        let excerpt = truncate_continuity_text(
            redacted.to_string().as_str(),
            MEMORY_FLUSH_MAX_EXCERPT_CHARS,
        );
        return serde_json::json!({
            "event_id": event_id,
            "stream": stream,
            "type": observation_type,
            "excerpt": excerpt,
        });
    }

    serde_json::json!({
        "event_id": event_id,
        "stream": stream,
        "type": "text_message",
        "excerpt": truncate_continuity_text(message.as_str(), MEMORY_FLUSH_MAX_EXCERPT_CHARS),
    })
}

fn build_memory_flush_summary(observations: &[Value]) -> String {
    let mut lines = Vec::new();
    for observation in observations.iter().take(5) {
        let event_id = observation
            .get("event_id")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let observation_type = observation
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let excerpt = observation
            .get("excerpt")
            .and_then(Value::as_str)
            .unwrap_or_default();
        lines.push(format!("#{event_id} [{observation_type}] {excerpt}"));
    }
    truncate_continuity_text(lines.join("\n").as_str(), MEMORY_FLUSH_MAX_SUMMARY_CHARS)
}

#[derive(Debug, Clone)]
struct ContinuitySnapshot {
    summary_text: String,
    history_window: Value,
    redacted_output: Value,
    redacted_output_text: String,
}

#[derive(Debug, Clone)]
struct ContextArtifactPointer {
    artifact_kind: String,
    relative_path: String,
    artifact_size_bytes: i64,
    content_checksum: String,
}

fn normalize_run_input_continuity(mut input: Value) -> Value {
    let Some(input_obj) = input.as_object_mut() else {
        return input;
    };
    let continuity_value = input_obj
        .entry("continuity".to_string())
        .or_insert_with(|| serde_json::json!({ "mode": CONTINUITY_MODE_DEFAULT }));
    if !continuity_value.is_object() {
        *continuity_value = serde_json::json!({ "mode": CONTINUITY_MODE_DEFAULT });
        return input;
    }
    let continuity_obj = continuity_value
        .as_object_mut()
        .expect("continuity object must be object");
    let mode = continuity_obj
        .get("mode")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(CONTINUITY_MODE_DEFAULT);
    let normalized_mode = if TEAM_RUN_CONTINUITY_MODE_VALUES.contains(&mode) {
        mode
    } else {
        CONTINUITY_MODE_DEFAULT
    };
    continuity_obj.insert(
        "mode".to_string(),
        Value::String(normalized_mode.to_string()),
    );

    if let Some(raw) = continuity_obj
        .get("max_history_items")
        .and_then(Value::as_i64)
    {
        if !(1..=200).contains(&raw) {
            continuity_obj.remove("max_history_items");
        }
    } else {
        continuity_obj.remove("max_history_items");
    }

    if let Some(raw) = continuity_obj.get("max_chars").and_then(Value::as_i64) {
        if !(256..=20000).contains(&raw) {
            continuity_obj.remove("max_chars");
        }
    } else {
        continuity_obj.remove("max_chars");
    }

    input
}

fn extract_continuity_mode_from_input(input: &Value) -> String {
    let Some(input_obj) = input.as_object() else {
        return CONTINUITY_MODE_DEFAULT.to_string();
    };
    let mode = input_obj
        .get("continuity")
        .and_then(Value::as_object)
        .and_then(|continuity| continuity.get("mode"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(CONTINUITY_MODE_DEFAULT);
    if mode == CONTINUITY_MODE_RESET {
        CONTINUITY_MODE_RESET.to_string()
    } else if TEAM_RUN_CONTINUITY_MODE_VALUES.contains(&mode) {
        mode.to_string()
    } else {
        CONTINUITY_MODE_DEFAULT.to_string()
    }
}

fn build_continuity_snapshot(output: Option<&Value>) -> ContinuitySnapshot {
    let redacted_output = output
        .map(redact_sensitive_json)
        .unwrap_or_else(|| serde_json::json!({}));

    let summary_seed = redacted_output
        .as_object()
        .and_then(|obj| obj.get("summary"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| redacted_output.as_str().map(str::to_string))
        .unwrap_or_else(|| redacted_output.to_string());
    let summary_text =
        truncate_continuity_text(summary_seed.as_str(), CONTINUITY_MAX_SUMMARY_CHARS);

    let output_excerpt_seed = redacted_output.to_string();
    let history_window = serde_json::json!({
        "schema_version": 1,
        "output_excerpt": truncate_continuity_text(
            output_excerpt_seed.as_str(),
            CONTINUITY_MAX_HISTORY_CHARS
        ),
    });
    ContinuitySnapshot {
        summary_text,
        history_window,
        redacted_output,
        redacted_output_text: output_excerpt_seed,
    }
}

fn should_offload_continuity_output(raw_output: &str) -> bool {
    raw_output.chars().count() > CONTINUITY_MAX_HISTORY_CHARS
}

fn truncate_continuity_text(raw: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    raw.chars().take(max_chars).collect::<String>()
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
