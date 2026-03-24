mod codec;
mod mailbox;
mod remote_relay;

#[cfg(test)]
mod tests;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub use agenthub_team_domain::TeamRunResumeError;
use agenthub_text::truncate_chars;
use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};
use tokio::sync::broadcast;
use uuid::Uuid;

pub use mailbox::{SendActorMessageInput, TeamRemoteRelayWorkerSettings};

use self::codec::{
    parse_run_event_row, parse_team_actor_message_row, parse_team_conversation_message_row,
    parse_team_conversation_row, parse_team_definition_row, parse_team_member_continuity_state_row,
    parse_team_run_row, parse_team_step_row, parse_team_task_row, team_run_status_to_str,
    team_step_status_to_str, team_task_status_to_str,
};
use self::remote_relay::{GrpcRelayTlsDefaults, TeamRemoteRelayAdapter};
use super::{
    TEAM_RUN_CONTINUITY_MODE_VALUES, TeamActorMessageRecord, TeamConversationMessageRecord,
    TeamConversationRecord, TeamDefinitionConfig, TeamDefinitionRecord,
    TeamMemberContinuityStateRecord, TeamRunEventRecord, TeamRunRecord, TeamRunStatus,
    TeamStepRecord, TeamStepStatus, TeamTaskRecord, TeamTaskStatus,
};
use crate::agent::event_message_codec::decode_message_from_storage;
use crate::internal::client::InternalGrpcPeerClientConfig;
use crate::internal::tls::InternalGrpcSecurityMode;
use agenthub_db::AgentEventDbRouter;
use agenthub_team_actor::ACTOR_MAIN_PEER_ID;

#[derive(Clone)]
pub struct TeamManager {
    db: SqlitePool,
    event_dbs: AgentEventDbRouter,
    conversation_events: broadcast::Sender<TeamConversationStreamEvent>,
    remote_relay_adapter: Arc<TeamRemoteRelayAdapter>,
    agents_target_node_id_column: Arc<Mutex<Option<bool>>>,
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
const TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND: &str = "shared_thread_mailbox";
const TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_SOURCE: &str = "teams_all";
const TEAM_CONVERSATION_STREAM_BUFFER_CAPACITY: usize = 256;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TeamConversationStreamEvent {
    pub team_id: String,
    pub task_id: String,
    pub conversation_id: String,
    pub message_id: Option<i64>,
    pub source: String,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamPendingActorUnreadRecord {
    pub run_id: String,
    pub actor_id: String,
    pub unread_count: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamRunMembersRecord {
    pub team_id: String,
    pub team_name: String,
    pub run_id: String,
    pub members: Vec<TeamRunMemberRecord>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamContextRunOverlayRecord {
    pub run_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamRuntimeStatus {
    Running,
    Stopped,
    Degraded,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamContextRecord {
    pub team_id: String,
    pub team_name: String,
    pub runtime: TeamRuntimeSummaryRecord,
    pub members: Vec<TeamRunMemberRecord>,
    pub run: Option<TeamContextRunOverlayRecord>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamRuntimeRecord {
    pub team_id: String,
    pub team_name: String,
    pub status: TeamRuntimeStatus,
    pub members: Vec<TeamRuntimeMemberRecord>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamRuntimeSummaryRecord {
    pub status: TeamRuntimeStatus,
    pub online_count: usize,
    pub member_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamRuntimeMemberRecord {
    pub member_id: String,
    pub display_name: String,
    pub role: String,
    pub description: Option<String>,
    pub pending_inbox_count: i64,
    pub agent_status: Option<String>,
    pub session_id: Option<String>,
    pub session_status: Option<String>,
    pub card: TeamMemberCardRecord,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamRunMemberRecord {
    pub member_id: String,
    pub display_name: String,
    pub role: String,
    pub description: Option<String>,
    pub pending_inbox_count: i64,
    pub agent_status: Option<String>,
    pub session_id: Option<String>,
    pub session_status: Option<String>,
    pub card: TeamMemberCardRecord,
    pub steps: Vec<TeamRunMemberStepRecord>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamMemberCardRecord {
    pub card_id: String,
    pub schema_version: String,
    pub description: String,
    pub capability_tags: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamRunMemberStepRecord {
    pub step_id: String,
    pub step_key: String,
    pub status: TeamStepStatus,
    pub attempt: i64,
    pub session_id: Option<String>,
    pub session_status: Option<String>,
}

#[derive(Debug, Clone)]
struct AgentRunningSessionRow {
    session_id: String,
    session_status: String,
}

impl TeamManager {
    #[cfg(test)]
    pub fn new(db: SqlitePool) -> Self {
        Self::new_with_event_dbs(db, AgentEventDbRouter::with_default_base_dir())
    }

    pub fn new_with_event_dbs(db: SqlitePool, event_dbs: AgentEventDbRouter) -> Self {
        let (conversation_events, _) = broadcast::channel(TEAM_CONVERSATION_STREAM_BUFFER_CAPACITY);
        let remote_relay_adapter = Arc::new(TeamRemoteRelayAdapter::new(db.clone()));
        let agents_target_node_id_column = Arc::new(Mutex::new(None));
        Self {
            db,
            event_dbs,
            conversation_events,
            remote_relay_adapter,
            agents_target_node_id_column,
        }
    }

    pub fn subscribe_conversation_events(
        &self,
    ) -> broadcast::Receiver<TeamConversationStreamEvent> {
        self.conversation_events.subscribe()
    }

    pub fn configure_internal_grpc_relay(&self, cert_dir: &Path, mode: InternalGrpcSecurityMode) {
        self.remote_relay_adapter
            .configure_grpc_tls_defaults(Some(GrpcRelayTlsDefaults::from_cert_dir(cert_dir, mode)));
    }

    pub fn configure_internal_grpc_peer_client(
        &self,
        config: Option<InternalGrpcPeerClientConfig>,
    ) {
        self.remote_relay_adapter.configure_grpc_peer_client(config);
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

    pub async fn delete_team(
        &self,
        team_id: &str,
        member_ids: &HashSet<String>,
    ) -> anyhow::Result<TeamDefinitionRecord> {
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

        for member_id in member_ids {
            sqlx::query("DELETE FROM acp_permission_requests WHERE agent_id = ?1")
                .bind(member_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM agent_sessions WHERE agent_id = ?1")
                .bind(member_id)
                .execute(&mut *tx)
                .await?;
        }

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

        sqlx::query("DELETE FROM team_tasks WHERE team_id = ?1")
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
        for member_id in member_ids {
            self.event_dbs.remove_agent_db(member_id).await?;
        }
        Ok(team)
    }

    pub async fn create_task(
        &self,
        team_id: &str,
        title: &str,
        created_by_actor_id: &str,
        context: Value,
        conversation_mode: &str,
        topic: Option<&str>,
    ) -> anyhow::Result<(TeamTaskRecord, TeamConversationRecord)> {
        let now = Utc::now().timestamp();
        let task_id = Uuid::new_v4().to_string();
        let conversation_id = Uuid::new_v4().to_string();
        let status = TeamTaskStatus::Open;
        let context_json = redact_sensitive_json(&context).to_string();
        let topic = topic.map(str::trim).filter(|value| !value.is_empty());

        let mut tx = self.db.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO team_tasks (
                id, team_id, title, status, created_by_actor_id, assigned_member_id, context_json, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?8)
            "#,
        )
        .bind(&task_id)
        .bind(team_id)
        .bind(title)
        .bind(team_task_status_to_str(&status))
        .bind(created_by_actor_id)
        .bind(context_json)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO team_conversations (
                id, team_id, task_id, mode, topic, created_at, updated_at
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

        let task = self.get_task(&task_id).await?;
        let conversation = self.get_task_conversation(&task_id).await?;
        Ok((task, conversation))
    }

    pub async fn list_tasks(
        &self,
        team_id: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<TeamTaskRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                team_id,
                title,
                status,
                created_by_actor_id,
                assigned_member_id,
                context_json,
                created_at,
                updated_at
            FROM team_tasks
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
            tasks.push(parse_team_task_row(&row)?);
        }
        Ok(tasks)
    }

    pub async fn get_task(&self, task_id: &str) -> anyhow::Result<TeamTaskRecord> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                team_id,
                title,
                status,
                created_by_actor_id,
                assigned_member_id,
                context_json,
                created_at,
                updated_at
            FROM team_tasks
            WHERE id = ?1
            "#,
        )
        .bind(task_id)
        .fetch_one(&self.db)
        .await?;
        parse_team_task_row(&row)
    }

    pub async fn update_task_status(
        &self,
        task_id: &str,
        status: TeamTaskStatus,
    ) -> anyhow::Result<TeamTaskRecord> {
        let current = self.get_task(task_id).await?;
        if current.status == status {
            return Ok(current);
        }

        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            UPDATE team_tasks
            SET status = ?2, updated_at = ?3
            WHERE id = ?1
            "#,
        )
        .bind(task_id)
        .bind(team_task_status_to_str(&status))
        .bind(now)
        .execute(&self.db)
        .await?;

        self.get_task(task_id).await
    }

    pub async fn get_task_conversation(
        &self,
        task_id: &str,
    ) -> anyhow::Result<TeamConversationRecord> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                team_id,
                task_id,
                mode,
                topic,
                created_at,
                updated_at
            FROM team_conversations
            WHERE task_id = ?1
            "#,
        )
        .bind(task_id)
        .fetch_one(&self.db)
        .await?;
        parse_team_conversation_row(&row)
    }

    pub async fn append_task_conversation_message(
        &self,
        task_id: &str,
        from_actor_id: &str,
        to_actor_id: Option<&str>,
        route: &str,
        payload: Value,
    ) -> anyhow::Result<TeamConversationMessageRecord> {
        let now = Utc::now().timestamp();
        let conversation = self.get_task_conversation(task_id).await?;
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
                task_id,
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
        .bind(task_id)
        .bind(from_actor_id)
        .bind(to_actor_id.as_deref())
        .bind(route)
        .bind(payload_json)
        .bind(now)
        .execute(&self.db)
        .await?;
        let message_id = result.last_insert_rowid();
        self.emit_conversation_event(TeamConversationStreamEvent {
            team_id: conversation.team_id.clone(),
            task_id: task_id.to_string(),
            conversation_id: conversation.id.clone(),
            message_id: Some(message_id),
            source: "conversation_message".to_string(),
        });

        Ok(TeamConversationMessageRecord {
            message_id,
            conversation_id: conversation.id,
            task_id: task_id.to_string(),
            from_actor_id: from_actor_id.to_string(),
            to_actor_id,
            route: route.to_string(),
            payload: redacted_payload,
            created_at: now,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn append_channel_replica_message(
        &self,
        authority_message_id: i64,
        run_id: &str,
        team_id: &str,
        conversation_id: &str,
        task_id: &str,
        channel_id: &str,
        from_actor_id: &str,
        source_node_id: &str,
        payload: &Value,
    ) -> anyhow::Result<bool> {
        let stored_at = Utc::now().timestamp();
        let payload_json = redact_sensitive_json(payload).to_string();
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO team_channel_message_replicas (
                authority_message_id,
                run_id,
                team_id,
                conversation_id,
                task_id,
                channel_id,
                from_actor_id,
                source_node_id,
                payload_json,
                stored_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(authority_message_id)
        .bind(run_id)
        .bind(team_id)
        .bind(conversation_id)
        .bind(task_id)
        .bind(channel_id)
        .bind(from_actor_id)
        .bind(source_node_id)
        .bind(payload_json)
        .bind(stored_at)
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn list_task_conversation_messages(
        &self,
        task_id: &str,
        limit: i64,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<TeamConversationMessageRecord>> {
        let conversation = self.get_task_conversation(task_id).await?;
        let mut builder = QueryBuilder::<sqlx::Sqlite>::new(
            r#"
            SELECT
                id,
                conversation_id,
                task_id,
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
        sync_linked_task_status_tx(&mut tx, team_id, &input, TeamTaskStatus::InProgress, now)
            .await?;
        tx.commit().await?;

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

    pub async fn describe_run_members(&self, run_id: &str) -> anyhow::Result<TeamRunMembersRecord> {
        let run = self.get_run(run_id).await?;
        let team = self.get_team(&run.team_id).await?;
        let members = parse_team_member_specs(&team.spec)?;
        let steps = self.list_steps(run_id).await?;
        let pending_inbox_counts = self.list_actor_pending_counts_by_actor(run_id).await?;

        let mut steps_by_member = HashMap::<String, Vec<TeamStepRecord>>::new();
        let mut session_ids = Vec::new();
        for step in steps {
            if let Some(session_id) = step
                .runtime_handle_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                session_ids.push(session_id.to_string());
            }
            steps_by_member
                .entry(step.member_id.clone())
                .or_default()
                .push(step);
        }

        let agent_runtime_by_id = load_agent_runtime_rows(&self.db, &members).await?;
        let running_session_by_agent =
            load_running_session_rows_by_agent(&self.db, &members).await?;
        let session_status_by_id = load_session_status_rows(&self.db, &session_ids).await?;

        let mut out = Vec::with_capacity(members.len());
        for member in members {
            let pending_inbox_count = pending_inbox_counts
                .get(member.member_id.as_str())
                .copied()
                .unwrap_or(0);
            let display_name = agent_runtime_by_id
                .get(member.member_id.as_str())
                .map(|agent| agent.name.clone())
                .unwrap_or_else(|| member.member_id.clone());
            let agent_status = agent_runtime_by_id
                .get(member.member_id.as_str())
                .and_then(|agent| agent.status.clone());
            let running_session = running_session_by_agent.get(member.member_id.as_str());
            let session_id = running_session.map(|session| session.session_id.clone());
            let session_status = running_session.map(|session| session.session_status.clone());
            let steps = steps_by_member
                .remove(member.member_id.as_str())
                .unwrap_or_default()
                .into_iter()
                .map(|step| TeamRunMemberStepRecord {
                    step_id: step.id,
                    step_key: step.step_key,
                    status: step.status,
                    attempt: step.attempt,
                    session_id: step.runtime_handle_id.clone(),
                    session_status: step
                        .runtime_handle_id
                        .as_deref()
                        .and_then(|session_id| session_status_by_id.get(session_id))
                        .cloned(),
                })
                .collect::<Vec<_>>();
            let card = build_team_member_card(
                &member,
                agent_runtime_by_id.get(member.member_id.as_str()),
                &display_name,
            );
            out.push(TeamRunMemberRecord {
                member_id: member.member_id,
                display_name,
                role: member.role,
                description: member.description,
                pending_inbox_count,
                agent_status,
                session_id,
                session_status,
                card,
                steps,
            });
        }

        Ok(TeamRunMembersRecord {
            team_id: team.id,
            team_name: team.name,
            run_id: run.id,
            members: out,
        })
    }

    pub async fn describe_team_runtime(&self, team_id: &str) -> anyhow::Result<TeamRuntimeRecord> {
        let team = self.get_team(team_id).await?;
        let members = parse_team_member_specs(&team.spec)?;
        let agent_runtime_by_id = load_agent_runtime_rows(&self.db, &members).await?;
        let running_session_by_agent =
            load_running_session_rows_by_agent(&self.db, &members).await?;

        let mut online = 0_usize;
        let mut out = Vec::with_capacity(members.len());
        for member in members {
            let display_name = agent_runtime_by_id
                .get(member.member_id.as_str())
                .map(|agent| agent.name.clone())
                .unwrap_or_else(|| member.member_id.clone());
            let agent_status = agent_runtime_by_id
                .get(member.member_id.as_str())
                .and_then(|agent| agent.status.clone());
            let running_session = running_session_by_agent.get(member.member_id.as_str());
            let session_id = running_session.map(|session| session.session_id.clone());
            let session_status = running_session.map(|session| session.session_status.clone());
            if session_id.is_some() {
                online += 1;
            }
            let card = build_team_member_card(
                &member,
                agent_runtime_by_id.get(member.member_id.as_str()),
                &display_name,
            );
            out.push(TeamRuntimeMemberRecord {
                member_id: member.member_id,
                display_name,
                role: member.role,
                description: member.description,
                pending_inbox_count: 0,
                agent_status,
                session_id,
                session_status,
                card,
            });
        }

        let status = if out.is_empty() || online == 0 {
            TeamRuntimeStatus::Stopped
        } else if online == out.len() {
            TeamRuntimeStatus::Running
        } else {
            TeamRuntimeStatus::Degraded
        };

        Ok(TeamRuntimeRecord {
            team_id: team.id,
            team_name: team.name,
            status,
            members: out,
        })
    }

    pub async fn describe_team_context(
        &self,
        team_id: Option<&str>,
        run_id: Option<&str>,
    ) -> anyhow::Result<TeamContextRecord> {
        let normalized_team_id = team_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let normalized_run_id = run_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        if let Some(run_id) = normalized_run_id.as_deref() {
            let roster = self.describe_run_members(run_id).await?;
            if let Some(explicit_team_id) = normalized_team_id.as_deref() {
                anyhow::ensure!(
                    explicit_team_id == roster.team_id,
                    "run_id {} belongs to team {}, not {}",
                    run_id,
                    roster.team_id,
                    explicit_team_id
                );
            }
            let runtime = self.describe_team_runtime(&roster.team_id).await?;
            return Ok(TeamContextRecord {
                team_id: roster.team_id,
                team_name: roster.team_name,
                runtime: build_team_runtime_summary(&runtime),
                members: roster.members,
                run: Some(TeamContextRunOverlayRecord {
                    run_id: roster.run_id,
                }),
            });
        }

        let team_id =
            normalized_team_id.ok_or_else(|| anyhow::anyhow!("team_id or run_id is required"))?;
        let runtime = self.describe_team_runtime(&team_id).await?;
        let runtime_summary = build_team_runtime_summary(&runtime);
        let members = runtime
            .members
            .into_iter()
            .map(team_run_member_from_runtime_member)
            .collect::<Vec<_>>();
        Ok(TeamContextRecord {
            team_id: runtime.team_id,
            team_name: runtime.team_name,
            runtime: runtime_summary,
            members,
            run: None,
        })
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
                // Best-effort audit trail: cancellation already committed by cancel_run.
                // We should not fail startup because of a follow-up event write error.
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

    #[allow(dead_code)]
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

    pub async fn get_live_member_session(
        &self,
        member_id: &str,
    ) -> anyhow::Result<Option<(String, String)>> {
        let row = sqlx::query(
            r#"
            SELECT id, status
            FROM agent_sessions
            WHERE agent_id = ?1
              AND ended_at IS NULL
            ORDER BY started_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(member_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(|row| (row.get("id"), row.get("status"))))
    }

    #[allow(dead_code)]
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
        owner: ContextArtifactOwner<'_>,
        snapshot: &ContinuitySnapshot,
        now: i64,
    ) -> anyhow::Result<Option<ContextArtifactPointer>> {
        let artifact_payload = serde_json::json!({
            "schema_version": 1,
            "team_id": owner.team_id,
            "run_id": owner.run_id,
            "member_id": owner.member_id,
            "session_id": owner.session_id,
            "summary_text": snapshot.summary_text,
            "redacted_output": snapshot.redacted_output,
            "created_at": now,
        });
        self.persist_context_artifact_tx(
            tx,
            owner,
            CONTINUITY_ARTIFACT_KIND_OUTPUT,
            artifact_payload,
            now,
        )
        .await
    }

    async fn persist_context_artifact_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        owner: ContextArtifactOwner<'_>,
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
        .bind(owner.member_id)
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
        .bind(owner.run_id)
        .fetch_one(&mut **tx)
        .await?;

        let run_context_dir = PathBuf::from(&workdir)
            .join(".cache")
            .join("context")
            .join("run")
            .join(owner.run_id);
        std::fs::create_dir_all(&run_context_dir)?;

        let file_name = format!("artifact-{artifact_seq}-{artifact_kind}.json");
        let absolute_path = run_context_dir.join(&file_name);
        let relative_path = format!(".cache/context/run/{}/{file_name}", owner.run_id);
        let artifact_bytes = serde_json::to_vec(&artifact_payload)?;
        std::fs::write(&absolute_path, &artifact_bytes)?;
        let artifact_size_bytes = i64::try_from(artifact_bytes.len()).ok().unwrap_or(i64::MAX);
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
        .bind(owner.team_id)
        .bind(owner.run_id)
        .bind(owner.member_id)
        .bind(owner.session_id)
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
        runtime_handle_id: Option<&str>,
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
        .bind(runtime_handle_id)
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
                "runtime_handle_id": step.runtime_handle_id,
                "remote_task_id": step.runtime_handle_id,
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
                "runtime_handle_id": step.runtime_handle_id,
                "remote_task_id": step.runtime_handle_id,
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
                        ContextArtifactOwner {
                            team_id: &team_id,
                            run_id: &step.run_id,
                            member_id: &step.member_id,
                            session_id: step.runtime_handle_id.as_deref(),
                        },
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
                source_session_id: step.runtime_handle_id.clone(),
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
                    sync_linked_task_status_tx(
                        &mut tx,
                        &team_id,
                        &run_input,
                        TeamTaskStatus::InReview,
                        now,
                    )
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
                context_id,
                status,
                input_json,
                created_at,
                started_at,
                ended_at
            )
            VALUES (?1, ?2, ?3, 'completed', ?4, ?5, ?6, ?7)
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

    async fn hydrate_run_summary(&self, mut run: TeamRunRecord) -> anyhow::Result<TeamRunRecord> {
        run.summary = load_run_summary(&self.db, &run.id, &run.status).await?;
        Ok(run)
    }

    async fn hydrate_run_summaries(
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
            let run_meta_row = sqlx::query(
                r#"
                SELECT team_id, input_json
                FROM team_runs
                WHERE id = ?1
                "#,
            )
            .bind(run_id)
            .fetch_one(&mut *tx)
            .await?;
            let team_id: String = run_meta_row.get("team_id");
            let run_input_json: String = run_meta_row.get("input_json");
            let run_input: Value =
                serde_json::from_str(&run_input_json).unwrap_or_else(|_| serde_json::json!({}));
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
            sync_linked_task_status_tx(
                &mut tx,
                &team_id,
                &run_input,
                TeamTaskStatus::Canceled,
                now,
            )
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
                from_peer_id,
                to_actor_id,
                to_peer_id,
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
        let normalized = normalize_memory_flush_request(request)?;
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let team_id = load_memory_flush_team_id_tx(&mut tx, run_id).await?;

        let session_id = resolve_memory_flush_session_id_tx(
            &mut tx,
            run_id,
            normalized.member_id.as_str(),
            normalized.session_id.as_deref(),
        )
        .await?;

        Self::append_run_event_tx(
            &mut tx,
            run_id,
            None,
            "memory_flush_started",
            now,
            &serde_json::json!({
                "team_id": team_id.as_str(),
                "run_id": run_id,
                "member_id": normalized.member_id.as_str(),
                "session_id": session_id.as_deref(),
                "trigger": normalized.trigger.as_str(),
                "ts": now,
            }),
        )
        .await?;

        let Some(session_id) = session_id else {
            let context = MemoryFlushFinalizeContext {
                run_id,
                team_id: team_id.as_str(),
                member_id: normalized.member_id.as_str(),
                session_id: None,
                trigger: normalized.trigger.as_str(),
                now,
            };
            let result = finalize_memory_flush_failed_tx(
                &mut tx,
                &context,
                "session_mapping_missing",
                None,
                0,
                None,
            )
            .await?;
            tx.commit().await?;
            return Ok(result);
        };

        let checkpoint_event_id = load_memory_flush_checkpoint_event_id_tx(
            &mut tx,
            run_id,
            normalized.member_id.as_str(),
            session_id.as_str(),
        )
        .await?;
        let event_rows = load_memory_flush_event_rows(
            &self.event_dbs,
            normalized.member_id.as_str(),
            session_id.as_str(),
            checkpoint_event_id,
            normalized.max_events,
        )
        .await?;

        if event_rows.is_empty() {
            let context = MemoryFlushFinalizeContext {
                run_id,
                team_id: team_id.as_str(),
                member_id: normalized.member_id.as_str(),
                session_id: Some(session_id.as_str()),
                trigger: normalized.trigger.as_str(),
                now,
            };
            let result = finalize_memory_flush_noop_tx(&mut tx, &context).await?;
            tx.commit().await?;
            return Ok(result);
        }

        let observations = event_rows
            .iter()
            .map(build_memory_flush_observation)
            .collect::<Vec<_>>();
        let event_id_from = event_rows.first().map(|row| row.id).unwrap_or(0);
        let event_id_to = event_rows.last().map(|row| row.id).unwrap_or(0);
        let flushed_events = safe_i64_len(event_rows.len());
        let summary_text = build_memory_flush_summary(observations.as_slice());
        let flush_payload = serde_json::json!({
            "schema_version": 1,
            "team_id": team_id.as_str(),
            "run_id": run_id,
            "member_id": normalized.member_id.as_str(),
            "session_id": session_id.as_str(),
            "trigger": normalized.trigger.as_str(),
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
                ContextArtifactOwner {
                    team_id: team_id.as_str(),
                    run_id,
                    member_id: normalized.member_id.as_str(),
                    session_id: Some(session_id.as_str()),
                },
                MEMORY_FLUSH_ARTIFACT_KIND,
                flush_payload,
                now,
            )
            .await
        {
            Ok(Some(pointer)) => pointer,
            Ok(None) => {
                let context = MemoryFlushFinalizeContext {
                    run_id,
                    team_id: team_id.as_str(),
                    member_id: normalized.member_id.as_str(),
                    session_id: Some(session_id.as_str()),
                    trigger: normalized.trigger.as_str(),
                    now,
                };
                let result = finalize_memory_flush_failed_tx(
                    &mut tx,
                    &context,
                    "agent_workdir_missing",
                    Some((event_id_from, event_id_to)),
                    flushed_events,
                    None,
                )
                .await?;
                tx.commit().await?;
                return Ok(result);
            }
            Err(err) => {
                let context = MemoryFlushFinalizeContext {
                    run_id,
                    team_id: team_id.as_str(),
                    member_id: normalized.member_id.as_str(),
                    session_id: Some(session_id.as_str()),
                    trigger: normalized.trigger.as_str(),
                    now,
                };
                let result = finalize_memory_flush_failed_tx(
                    &mut tx,
                    &context,
                    "artifact_write_failed",
                    Some((event_id_from, event_id_to)),
                    flushed_events,
                    Some(truncate_chars(err.to_string().as_str(), 400)),
                )
                .await?;
                tx.commit().await?;
                return Ok(result);
            }
        };

        upsert_memory_flush_checkpoint_tx(
            &mut tx,
            team_id.as_str(),
            run_id,
            normalized.member_id.as_str(),
            session_id.as_str(),
            event_id_to,
            now,
        )
        .await?;

        let pointer_payload = build_context_artifact_pointer_payload(&pointer);
        Self::append_run_event_tx(
            &mut tx,
            run_id,
            None,
            "memory_flush_persisted",
            now,
            &serde_json::json!({
                "team_id": team_id.as_str(),
                "run_id": run_id,
                "member_id": normalized.member_id.as_str(),
                "session_id": session_id.as_str(),
                "trigger": normalized.trigger.as_str(),
                "artifact_pointer": pointer_payload.clone(),
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
            member_id: normalized.member_id,
            session_id: Some(session_id),
            trigger: normalized.trigger,
            reason: None,
            artifact_pointer: Some(pointer_payload),
            event_id_from: Some(event_id_from),
            event_id_to: Some(event_id_to),
            flushed_events,
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
              AND to_peer_id = ?2
            GROUP BY to_actor_id
            "#,
        )
        .bind(run_id)
        .bind(ACTOR_MAIN_PEER_ID)
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

    pub async fn count_actor_pending_inbox(
        &self,
        run_id: &str,
        actor_id: &str,
    ) -> anyhow::Result<i64> {
        let count = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM team_actor_messages
            WHERE run_id = ?1
              AND to_actor_id = ?2
              AND status = 'pending'
              AND to_peer_id = ?3
            "#,
        )
        .bind(run_id)
        .bind(actor_id)
        .bind(ACTOR_MAIN_PEER_ID)
        .fetch_one(&self.db)
        .await?;
        Ok(count)
    }

    pub async fn list_pending_actor_unread_counts(
        &self,
    ) -> anyhow::Result<Vec<TeamPendingActorUnreadRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT run_id, to_actor_id, COUNT(*) AS cnt
            FROM team_actor_messages
            WHERE status = 'pending'
              AND to_peer_id = ?1
            GROUP BY run_id, to_actor_id
            ORDER BY run_id ASC, to_actor_id ASC
            "#,
        )
        .bind(ACTOR_MAIN_PEER_ID)
        .fetch_all(&self.db)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(TeamPendingActorUnreadRecord {
                run_id: row.get("run_id"),
                actor_id: row.get("to_actor_id"),
                unread_count: row.get("cnt"),
            });
        }
        Ok(out)
    }

    pub async fn member_role_for_run(
        &self,
        run_id: &str,
        member_id: &str,
    ) -> anyhow::Result<Option<String>> {
        let team_id =
            sqlx::query_scalar::<_, String>("SELECT team_id FROM team_runs WHERE id = ?1")
                .bind(run_id)
                .fetch_optional(&self.db)
                .await?;
        let Some(team_id) = team_id else {
            return Ok(None);
        };
        let team = self.get_team(&team_id).await?;
        let role = parse_team_member_specs(&team.spec)?
            .into_iter()
            .find(|member| member.member_id == member_id)
            .map(|member| member.role);
        Ok(role)
    }
}

#[derive(Debug, Clone)]
struct NormalizedMemoryFlushRequest {
    member_id: String,
    session_id: Option<String>,
    trigger: String,
    max_events: i64,
}

#[derive(Debug, Clone, Copy)]
struct MemoryFlushFinalizeContext<'a> {
    run_id: &'a str,
    team_id: &'a str,
    member_id: &'a str,
    session_id: Option<&'a str>,
    trigger: &'a str,
    now: i64,
}

#[derive(Debug, Clone)]
struct MemoryFlushEventRow {
    id: i64,
    stream: String,
    message: Vec<u8>,
}

fn normalize_memory_flush_request(
    request: TeamMemoryFlushRequest,
) -> anyhow::Result<NormalizedMemoryFlushRequest> {
    let member_id = request.member_id.trim().to_string();
    if member_id.is_empty() {
        return Err(anyhow::anyhow!("member_id is required"));
    }
    Ok(NormalizedMemoryFlushRequest {
        member_id,
        session_id: request.session_id,
        trigger: normalize_memory_flush_trigger(request.trigger.as_str()).to_string(),
        max_events: normalize_memory_flush_max_events(request.max_events),
    })
}

fn safe_i64_len(len: usize) -> i64 {
    i64::try_from(len).unwrap_or(i64::MAX)
}

fn build_context_artifact_pointer_payload(pointer: &ContextArtifactPointer) -> Value {
    serde_json::json!({
        "kind": pointer.artifact_kind.as_str(),
        "path": pointer.relative_path.as_str(),
        "size_bytes": pointer.artifact_size_bytes,
        "checksum": pointer.content_checksum.as_str(),
    })
}

async fn load_memory_flush_team_id_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
) -> anyhow::Result<String> {
    let run_meta_row = sqlx::query(
        r#"
        SELECT team_id
        FROM team_runs
        WHERE id = ?1
        "#,
    )
    .bind(run_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(run_meta_row.get("team_id"))
}

async fn load_memory_flush_checkpoint_event_id_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
    member_id: &str,
    session_id: &str,
) -> anyhow::Result<i64> {
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
    .bind(member_id)
    .bind(session_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or(0);
    Ok(checkpoint_event_id)
}

async fn load_memory_flush_event_rows(
    event_dbs: &AgentEventDbRouter,
    member_id: &str,
    session_id: &str,
    checkpoint_event_id: i64,
    max_events: i64,
) -> anyhow::Result<Vec<MemoryFlushEventRow>> {
    let event_db = event_dbs.pool_for_agent(member_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT id, stream, message
        FROM agent_events
        WHERE session_id = ?1
          AND id > ?2
        ORDER BY id ASC
        LIMIT ?3
        "#,
    )
    .bind(session_id)
    .bind(checkpoint_event_id)
    .bind(max_events)
    .fetch_all(&event_db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| MemoryFlushEventRow {
            id: row.get("id"),
            stream: row.get("stream"),
            message: row.get("message"),
        })
        .collect())
}

async fn upsert_memory_flush_checkpoint_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    team_id: &str,
    run_id: &str,
    member_id: &str,
    session_id: &str,
    event_id_to: i64,
    now: i64,
) -> anyhow::Result<()> {
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
    .bind(team_id)
    .bind(run_id)
    .bind(member_id)
    .bind(session_id)
    .bind(event_id_to)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn finalize_memory_flush_failed_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    context: &MemoryFlushFinalizeContext<'_>,
    reason: &str,
    event_range: Option<(i64, i64)>,
    flushed_events: i64,
    error_excerpt: Option<String>,
) -> anyhow::Result<TeamMemoryFlushResult> {
    let mut payload = serde_json::Map::new();
    payload.insert(
        "team_id".to_string(),
        Value::String(context.team_id.to_string()),
    );
    payload.insert(
        "run_id".to_string(),
        Value::String(context.run_id.to_string()),
    );
    payload.insert(
        "member_id".to_string(),
        Value::String(context.member_id.to_string()),
    );
    payload.insert(
        "trigger".to_string(),
        Value::String(context.trigger.to_string()),
    );
    payload.insert("reason_code".to_string(), Value::String(reason.to_string()));
    payload.insert("ts".to_string(), Value::from(context.now));
    if let Some(session_id) = context.session_id {
        payload.insert(
            "session_id".to_string(),
            Value::String(session_id.to_string()),
        );
    }
    if let Some(error_excerpt) = error_excerpt {
        payload.insert("error_excerpt".to_string(), Value::String(error_excerpt));
    }
    TeamManager::append_run_event_tx(
        tx,
        context.run_id,
        None,
        "memory_flush_failed",
        context.now,
        &Value::Object(payload),
    )
    .await?;
    Ok(TeamMemoryFlushResult {
        status: "failed".to_string(),
        run_id: context.run_id.to_string(),
        team_id: context.team_id.to_string(),
        member_id: context.member_id.to_string(),
        session_id: context.session_id.map(str::to_string),
        trigger: context.trigger.to_string(),
        reason: Some(reason.to_string()),
        artifact_pointer: None,
        event_id_from: event_range.map(|(event_id_from, _)| event_id_from),
        event_id_to: event_range.map(|(_, event_id_to)| event_id_to),
        flushed_events,
    })
}

async fn finalize_memory_flush_noop_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    context: &MemoryFlushFinalizeContext<'_>,
) -> anyhow::Result<TeamMemoryFlushResult> {
    let session_id = context
        .session_id
        .ok_or_else(|| anyhow::anyhow!("session_id is required for noop flush"))?;
    TeamManager::append_run_event_tx(
        tx,
        context.run_id,
        None,
        "memory_flush_noop",
        context.now,
        &serde_json::json!({
            "team_id": context.team_id,
            "run_id": context.run_id,
            "member_id": context.member_id,
            "session_id": session_id,
            "trigger": context.trigger,
            "reason": "no_new_events",
            "ts": context.now,
        }),
    )
    .await?;
    Ok(TeamMemoryFlushResult {
        status: "noop".to_string(),
        run_id: context.run_id.to_string(),
        team_id: context.team_id.to_string(),
        member_id: context.member_id.to_string(),
        session_id: Some(session_id.to_string()),
        trigger: context.trigger.to_string(),
        reason: Some("no_new_events".to_string()),
        artifact_pointer: None,
        event_id_from: None,
        event_id_to: None,
        flushed_events: 0,
    })
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

async fn load_run_summary(
    db: &SqlitePool,
    run_id: &str,
    status: &TeamRunStatus,
) -> anyhow::Result<Option<String>> {
    let row = sqlx::query(
        r#"
        SELECT output_json, error_text
        FROM team_steps
        WHERE run_id = ?1
          AND (output_json IS NOT NULL OR error_text IS NOT NULL)
        ORDER BY COALESCE(ended_at, started_at, 0) DESC, attempt DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(run_id)
    .fetch_optional(db)
    .await?;

    if let Some(row) = row {
        let output_json = row.try_get::<Option<String>, _>("output_json")?;
        let error_text = row.try_get::<Option<String>, _>("error_text")?;
        if let Some(summary) =
            summarize_run_summary_fields(output_json.as_deref(), error_text.as_deref())
        {
            return Ok(Some(summary));
        }
    }

    Ok(fallback_run_summary(status))
}

async fn load_run_summaries(
    db: &SqlitePool,
    runs: &[TeamRunRecord],
) -> anyhow::Result<HashMap<String, Option<String>>> {
    let mut summaries = HashMap::with_capacity(runs.len());
    if runs.is_empty() {
        return Ok(summaries);
    }

    let mut builder = QueryBuilder::<Sqlite>::new(
        r#"
        SELECT run_id, output_json, error_text
        FROM team_steps
        WHERE run_id IN (
        "#,
    );
    {
        let mut separated = builder.separated(", ");
        for run in runs {
            separated.push_bind(&run.id);
        }
    }
    builder.push(
        r#")
          AND (output_json IS NOT NULL OR error_text IS NOT NULL)
        ORDER BY run_id ASC, COALESCE(ended_at, started_at, 0) DESC, attempt DESC, id DESC
        "#,
    );

    let rows = builder.build().fetch_all(db).await?;
    for row in rows {
        let run_id = row.try_get::<String, _>("run_id")?;
        let output_json = row.try_get::<Option<String>, _>("output_json")?;
        let error_text = row.try_get::<Option<String>, _>("error_text")?;
        summaries.entry(run_id).or_insert_with(|| {
            summarize_run_summary_fields(output_json.as_deref(), error_text.as_deref())
        });
    }

    for run in runs {
        summaries
            .entry(run.id.clone())
            .or_insert_with(|| fallback_run_summary(&run.status));
    }
    Ok(summaries)
}

fn summarize_run_summary_fields(
    output_json: Option<&str>,
    error_text: Option<&str>,
) -> Option<String> {
    if let Some(output_json) = output_json
        && let Ok(output) = serde_json::from_str::<Value>(output_json)
    {
        let summary = build_continuity_snapshot(Some(&output)).summary_text;
        if !summary.trim().is_empty() {
            return Some(summary);
        }
    }
    if let Some(error_text) = error_text {
        let trimmed = error_text.trim();
        if !trimmed.is_empty() {
            return Some(truncate_chars(trimmed, CONTINUITY_MAX_SUMMARY_CHARS));
        }
    }
    None
}

fn fallback_run_summary(status: &TeamRunStatus) -> Option<String> {
    let fallback = match status {
        TeamRunStatus::Completed => Some("Completed without a structured summary."),
        TeamRunStatus::Failed => Some("Run failed before a structured summary was recorded."),
        TeamRunStatus::Canceled => Some("Run was canceled before completion."),
        TeamRunStatus::Submitted | TeamRunStatus::Working | TeamRunStatus::InputRequired => None,
    };
    fallback.map(str::to_string)
}

async fn load_linked_task_ids_for_runs(
    db: &SqlitePool,
    run_ids: &[String],
) -> anyhow::Result<HashMap<String, String>> {
    let mut linked_task_ids = HashMap::new();
    if run_ids.is_empty() {
        return Ok(linked_task_ids);
    }

    let mut builder = QueryBuilder::<Sqlite>::new(
        r#"
        SELECT id, trim(COALESCE(json_extract(input_json, '$.task_id'), '')) AS task_id
        FROM team_runs
        WHERE id IN (
        "#,
    );
    {
        let mut separated = builder.separated(", ");
        for run_id in run_ids {
            separated.push_bind(run_id);
        }
    }
    builder.push(")");

    let rows = builder.build().fetch_all(db).await?;
    for row in rows {
        let run_id = row.try_get::<String, _>("id")?;
        let task_id = row.try_get::<String, _>("task_id")?;
        let task_id = task_id.trim();
        if !task_id.is_empty() {
            linked_task_ids.insert(run_id, task_id.to_string());
        }
    }
    Ok(linked_task_ids)
}

fn extract_linked_task_id_from_run_input(input: &Value) -> Option<&str> {
    input
        .as_object()
        .and_then(|obj| obj.get("task_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

async fn sync_linked_task_status_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    team_id: &str,
    input: &Value,
    status: TeamTaskStatus,
    now: i64,
) -> anyhow::Result<()> {
    let Some(task_id) = extract_linked_task_id_from_run_input(input) else {
        return Ok(());
    };

    sqlx::query(
        r#"
        UPDATE team_tasks
        SET status = ?3, updated_at = ?4
        WHERE id = ?1 AND team_id = ?2 AND status <> ?3
        "#,
    )
    .bind(task_id)
    .bind(team_id)
    .bind(team_task_status_to_str(&status))
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
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

fn build_memory_flush_observation(row: &MemoryFlushEventRow) -> Value {
    let event_id = row.id;
    let stream = row.stream.as_str();
    let message = decode_message_from_storage(row.message.as_slice());
    if let Ok(message_json) = serde_json::from_str::<Value>(&message) {
        let redacted = redact_sensitive_json(&message_json);
        let observation_type = message_json
            .as_object()
            .and_then(|obj| obj.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("json_message");
        let excerpt = truncate_chars(
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
        "excerpt": truncate_chars(message.as_str(), MEMORY_FLUSH_MAX_EXCERPT_CHARS),
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
    truncate_chars(lines.join("\n").as_str(), MEMORY_FLUSH_MAX_SUMMARY_CHARS)
}

#[derive(Debug, Clone)]
struct ContinuitySnapshot {
    summary_text: String,
    history_window: Value,
    redacted_output: Value,
    redacted_output_text: String,
}

#[derive(Debug, Clone, Copy)]
struct ContextArtifactOwner<'a> {
    team_id: &'a str,
    run_id: &'a str,
    member_id: &'a str,
    session_id: Option<&'a str>,
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
    let summary_text = truncate_chars(summary_seed.as_str(), CONTINUITY_MAX_SUMMARY_CHARS);

    let output_excerpt_seed = redacted_output.to_string();
    let history_window = serde_json::json!({
        "schema_version": 1,
        "output_excerpt": truncate_chars(
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

pub(super) fn redact_sensitive_json(value: &Value) -> Value {
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

#[derive(Debug, Clone)]
struct TeamMemberSpecView {
    member_id: String,
    role: String,
    description: Option<String>,
}

#[derive(Debug, Clone)]
struct AgentRuntimeRow {
    name: String,
    status: Option<String>,
    code_mode: bool,
    worktree_mode: Option<String>,
}

fn parse_team_member_specs(spec: &Value) -> anyhow::Result<Vec<TeamMemberSpecView>> {
    let members = spec
        .get("members")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("spec.members must be an array"))?;
    let mut out = Vec::with_capacity(members.len());
    for member in members {
        let member_obj = member
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("spec.members entries must be objects"))?;
        let member_id = member_obj
            .get("member_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("spec.members[].member_id is required"))?;
        let role = member_obj
            .get("role")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("worker");
        let description = member_obj
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        out.push(TeamMemberSpecView {
            member_id: member_id.to_string(),
            role: role.to_string(),
            description,
        });
    }
    Ok(out)
}

async fn load_agent_runtime_rows(
    db: &SqlitePool,
    members: &[TeamMemberSpecView],
) -> anyhow::Result<HashMap<String, AgentRuntimeRow>> {
    if members.is_empty() {
        return Ok(HashMap::new());
    }
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT id, name, status, code_mode, worktree_mode FROM agents WHERE id IN (",
    );
    let mut separated = builder.separated(", ");
    for member in members {
        separated.push_bind(member.member_id.as_str());
    }
    separated.push_unseparated(")");
    let rows = builder.build().fetch_all(db).await?;

    let mut out = HashMap::with_capacity(rows.len());
    for row in rows {
        let id: String = row.get("id");
        let code_mode_raw: i64 = row.get("code_mode");
        out.insert(
            id,
            AgentRuntimeRow {
                name: row.get("name"),
                status: row.get::<Option<String>, _>("status"),
                code_mode: code_mode_raw != 0,
                worktree_mode: row.get::<Option<String>, _>("worktree_mode"),
            },
        );
    }
    Ok(out)
}

async fn load_session_status_rows(
    db: &SqlitePool,
    session_ids: &[String],
) -> anyhow::Result<HashMap<String, String>> {
    if session_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut builder =
        QueryBuilder::<Sqlite>::new("SELECT id, status FROM agent_sessions WHERE id IN (");
    let mut separated = builder.separated(", ");
    for session_id in session_ids {
        separated.push_bind(session_id.as_str());
    }
    separated.push_unseparated(")");
    let rows = builder.build().fetch_all(db).await?;
    let mut out = HashMap::with_capacity(rows.len());
    for row in rows {
        let id: String = row.get("id");
        let status: String = row.get("status");
        out.insert(id, status);
    }
    Ok(out)
}

async fn load_running_session_rows_by_agent(
    db: &SqlitePool,
    members: &[TeamMemberSpecView],
) -> anyhow::Result<HashMap<String, AgentRunningSessionRow>> {
    if members.is_empty() {
        return Ok(HashMap::new());
    }
    let mut builder = QueryBuilder::<Sqlite>::new(
        r#"
        SELECT agent_id, id, status
        FROM agent_sessions
        WHERE ended_at IS NULL
          AND agent_id IN (
        "#,
    );
    let mut separated = builder.separated(", ");
    for member in members {
        separated.push_bind(member.member_id.as_str());
    }
    separated.push_unseparated(
        r#")
        ORDER BY started_at DESC, id DESC
        "#,
    );
    let rows = builder.build().fetch_all(db).await?;

    let mut out = HashMap::with_capacity(rows.len());
    for row in rows {
        let member_id: String = row.get("agent_id");
        let session_id = row.get::<String, _>("id").trim().to_string();
        if session_id.is_empty() {
            continue;
        }
        if out.contains_key(member_id.as_str()) {
            continue;
        }
        out.insert(
            member_id,
            AgentRunningSessionRow {
                session_id,
                session_status: row.get("status"),
            },
        );
    }
    Ok(out)
}

impl TeamManager {
    pub async fn ensure_shared_thread_target_for_team(
        &self,
        team_id: &str,
        created_by_actor_id: &str,
    ) -> anyhow::Result<(String, String)> {
        let mut tx = self.db.begin().await?;
        let existing = sqlx::query(
            r#"
            SELECT
                t.id AS task_id,
                c.id AS conversation_id
            FROM team_tasks t
            INNER JOIN team_conversations c ON c.task_id = t.id
            WHERE t.team_id = ?1
              AND (
                lower(trim(t.title)) = 'all'
                OR trim(COALESCE(json_extract(t.context_json, '$.bootstrap_kind'), '')) = 'shared_thread'
              )
            ORDER BY t.updated_at DESC, t.created_at DESC, t.id DESC
            LIMIT 1
            "#,
        )
        .bind(team_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(row) = existing {
            tx.commit().await?;
            return Ok((
                row.get::<String, _>("task_id"),
                row.get::<String, _>("conversation_id"),
            ));
        }

        let now = Utc::now().timestamp();
        let task_id = Uuid::new_v4().to_string();
        let conversation_id = Uuid::new_v4().to_string();
        let context_json = serde_json::json!({
            "bootstrap_kind": "shared_thread",
            "bootstrap_source": "server_canonical_reply",
        })
        .to_string();

        sqlx::query(
            r#"
            INSERT INTO team_tasks (
                id,
                team_id,
                title,
                status,
                created_by_actor_id,
                assigned_member_id,
                context_json,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, 'all', 'open', ?3, NULL, ?4, ?5, ?6)
            "#,
        )
        .bind(&task_id)
        .bind(team_id)
        .bind(created_by_actor_id)
        .bind(context_json)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO team_conversations (
                id,
                team_id,
                task_id,
                mode,
                topic,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, 'group_chat', 'all', ?4, ?5)
            "#,
        )
        .bind(&conversation_id)
        .bind(team_id)
        .bind(&task_id)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((task_id, conversation_id))
    }

    pub async fn team_has_member(&self, team_id: &str, member_id: &str) -> anyhow::Result<bool> {
        let team = self.get_team(team_id).await?;
        let members = team
            .spec
            .get("members")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(members.iter().any(|member| {
            member
                .get("member_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .is_some_and(|value| value == member_id)
        }))
    }

    pub(crate) async fn shared_thread_target_for_run(
        &self,
        run_id: &str,
    ) -> anyhow::Result<Option<(String, String, String)>> {
        let team_id =
            sqlx::query_scalar::<_, String>("SELECT team_id FROM team_runs WHERE id = ?1")
                .bind(run_id)
                .fetch_optional(&self.db)
                .await?;
        let Some(team_id) = team_id else {
            return Ok(None);
        };
        let row = sqlx::query(
            r#"
            SELECT
                t.id AS task_id,
                c.id AS conversation_id
            FROM team_tasks t
            INNER JOIN team_conversations c ON c.task_id = t.id
            WHERE t.team_id = ?1
              AND (
                lower(trim(t.title)) = 'all'
                OR trim(COALESCE(json_extract(t.context_json, '$.bootstrap_kind'), '')) = 'shared_thread'
              )
            ORDER BY t.updated_at DESC, t.created_at DESC, t.id DESC
            LIMIT 1
            "#,
        )
        .bind(&team_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.map(|row| {
            (
                team_id,
                row.get::<String, _>("task_id"),
                row.get::<String, _>("conversation_id"),
            )
        }))
    }

    pub(crate) fn emit_conversation_event(&self, event: TeamConversationStreamEvent) {
        let _ = self.conversation_events.send(event);
    }
}

fn build_team_member_card(
    member: &TeamMemberSpecView,
    agent: Option<&AgentRuntimeRow>,
    display_name: &str,
) -> TeamMemberCardRecord {
    let mut capability_tags = vec![
        "team_mailbox_v1".to_string(),
        "team_step_execution_v1".to_string(),
    ];
    if let Some(agent) = agent {
        if agent.code_mode {
            capability_tags.push("code_mode".to_string());
        }
        if let Some(worktree_mode) = agent
            .worktree_mode
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            && matches!(worktree_mode, "create_worktree" | "reuse_worktree")
        {
            capability_tags.push("git_worktree".to_string());
        }
    }
    let description = member.description.clone().unwrap_or_else(|| {
        format!(
            "AgentHub team member {} ({}) supports {}",
            display_name,
            member.role,
            capability_tags.join(", ")
        )
    });
    TeamMemberCardRecord {
        card_id: format!("agenthub://team-members/{}", member.member_id),
        schema_version: "agenthub.a2a.discovery_card.v1".to_string(),
        description,
        capability_tags,
    }
}

fn build_team_runtime_summary(runtime: &TeamRuntimeRecord) -> TeamRuntimeSummaryRecord {
    TeamRuntimeSummaryRecord {
        status: runtime.status,
        online_count: runtime
            .members
            .iter()
            .filter(|member| member.session_id.is_some())
            .count(),
        member_count: runtime.members.len(),
    }
}

#[allow(dead_code)]
fn filter_visible_team_runs(runs: Vec<TeamRunRecord>) -> Vec<TeamRunRecord> {
    runs.into_iter()
        .filter(|run| !is_shared_thread_mailbox_run_input(&run.input))
        .collect()
}

fn shared_thread_mailbox_run_id(team_id: &str, task_id: &str) -> String {
    format!("shared-thread-mailbox:{team_id}:{task_id}")
}

#[allow(dead_code)]
fn is_shared_thread_mailbox_run_input(input: &Value) -> bool {
    input
        .as_object()
        .and_then(|obj| obj.get("bootstrap_kind"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| value == TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND)
}

fn team_run_member_from_runtime_member(member: TeamRuntimeMemberRecord) -> TeamRunMemberRecord {
    TeamRunMemberRecord {
        member_id: member.member_id,
        display_name: member.display_name,
        role: member.role,
        description: member.description,
        pending_inbox_count: member.pending_inbox_count,
        agent_status: member.agent_status,
        session_id: member.session_id,
        session_status: member.session_status,
        card: member.card,
        steps: Vec::new(),
    }
}
