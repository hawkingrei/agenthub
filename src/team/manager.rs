mod codec;
mod mailbox;
mod remote_relay;

#[cfg(test)]
mod tests;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

pub use agenthub_team_domain::TeamRunResumeError;
use agenthub_team_domain::{
    TEAM_CONTINUITY_NOTE_SCHEMA_FAMILY, TEAM_CONTINUITY_NOTE_SCHEMA_VERSION,
    TEAM_RUNTIME_STATE_SCHEMA_FAMILY, TEAM_RUNTIME_STATE_SCHEMA_VERSION,
    continuity_note_relative_path, extract_context_artifact_path,
};
use agenthub_text::truncate_chars;
use anyhow::Context;
use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};

fn hex_encode(data: &[u8]) -> String {
    const HEX_CHARS: &[u8] = b"0123456789abcdef";
    let mut result = String::with_capacity(data.len() * 2);
    for byte in data {
        result.push(HEX_CHARS[(byte >> 4) as usize] as char);
        result.push(HEX_CHARS[(byte & 0xf) as usize] as char);
    }
    result
}
use sqlx::{Error as SqlxError, Executor, QueryBuilder, Row, Sqlite, SqlitePool};
use tokio::sync::broadcast;
use uuid::Uuid;

pub use mailbox::{SendActorMessageInput, TeamRemoteRelayWorkerSettings};

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct TeamMessageArchiveMigrationReport {
    pub team_conversation_messages: usize,
    pub team_run_events: usize,
    pub team_actor_messages: usize,
    pub agent_events: usize,
    pub aggregated_acp_messages: usize,
}

impl TeamMessageArchiveMigrationReport {
    pub fn total_documents(&self) -> usize {
        self.team_conversation_messages
            + self.team_run_events
            + self.team_actor_messages
            + self.agent_events
            + self.aggregated_acp_messages
    }
}

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
    TeamStepRecord, TeamStepStatus, TeamTaskRecord, TeamTaskStatus, TeamTaskStepExecutionSpec,
    build_team_member_actor_context_for_role, normalize_optional_idempotency_key_input,
    parse_task_execution_plan, team_member_role_from_spec, validate_task_execution_plan,
    validate_task_execution_steps,
};
use crate::agent::event_message_codec::decode_message_from_storage;
use crate::agent::{WorktreeMode, derive_team_runtime_workdir};
use crate::internal::client::InternalGrpcPeerClientConfig;
use crate::internal::tls::InternalGrpcSecurityMode;
use agenthub_db::AgentEventDbRouter;
use agenthub_message_archive::{
    AcpAggregatedMessage, AcpChunkAccumulator, AcpEventRow, MessageArchiveStoreRef,
    MessageDocument, MessageDocumentKind, MessageSearchHit, MessageSearchQuery,
};
use agenthub_team_actor::{ACTOR_MAIN_PEER_ID, canonical_json};

#[derive(Clone)]
pub struct TeamManager {
    db: SqlitePool,
    event_dbs: AgentEventDbRouter,
    message_archive: Option<MessageArchiveStoreRef>,
    conversation_events: broadcast::Sender<TeamConversationStreamEvent>,
    remote_relay_adapter: Arc<TeamRemoteRelayAdapter>,
    agents_target_node_id_column: Arc<Mutex<Option<bool>>>,
}

const CONTINUITY_MODE_DEFAULT: &str = "inherit_recent";
const CONTINUITY_MODE_RESET: &str = "reset";
const CONTINUITY_MAX_SUMMARY_CHARS: usize = 2048;
const CONTINUITY_MAX_HISTORY_CHARS: usize = 4096;
const CONTINUITY_ARTIFACT_KIND_OUTPUT: &str = "continuity_output";
const RECONCILE_ROUND_ARTIFACT_KIND: &str = "reconcile_round_result";
const MEMORY_FLUSH_MAX_EVENTS_DEFAULT: i64 = 200;
const MEMORY_FLUSH_MAX_EVENTS_MAX: i64 = 1000;
const MEMORY_FLUSH_MAX_SUMMARY_CHARS: usize = 2048;
const MEMORY_FLUSH_MAX_EXCERPT_CHARS: usize = 700;
const MEMORY_FLUSH_ARTIFACT_KIND: &str = "memory_flush";
const TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND: &str = "shared_thread_mailbox";
const TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_SOURCE: &str = "teams_all";
const TEAM_CONVERSATION_STREAM_BUFFER_CAPACITY: usize = 256;
pub(crate) const TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX: i64 = 500;
pub(crate) const TEAM_SHARED_THREAD_TITLE: &str = "all";
pub(crate) const TEAM_SHARED_THREAD_BOOTSTRAP_KIND: &str = "shared_thread";
pub(crate) const TEAM_CHANNEL_BOOTSTRAP_KIND: &str = "team_channel";
const TEAM_CHANNEL_BOOTSTRAP_SOURCE: &str = "coordinator_created";
const SQLITE_CONSTRAINT_UNIQUE_CODE: &str = "2067";
const TEAM_CHANNEL_BOOTSTRAP_UNIQUE_INDEX: &str = "idx_team_channel_bootstrap_unique";
const MESSAGE_ARCHIVE_APPEND_TIMEOUT: Duration = Duration::from_secs(2);
const TEAM_CHANNEL_BOOTSTRAP_UNIQUE_CHANNEL_EXPR: &str =
    "lower(trim(COALESCE(json_extract(context_json, '$.channel_id'), '')))";
const TASK_CONVERSATION_MESSAGE_IDEMPOTENCY_UNIQUE_COLUMNS: &str = "team_conversation_messages.conversation_id, team_conversation_messages.from_actor_id, team_conversation_messages.idempotency_key";

fn is_row_not_found(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<SqlxError>(),
        Some(SqlxError::RowNotFound)
    )
}

fn normalize_team_channel_id(channel_id: &str) -> anyhow::Result<String> {
    let normalized = channel_id.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        anyhow::bail!("channel_id is required");
    }
    Ok(normalized)
}

fn team_conversation_message_archive_document(
    conversation: &TeamConversationRecord,
    message: &TeamConversationMessageRecord,
) -> MessageDocument {
    MessageDocument {
        document_id: format!(
            "team_conversation_message:{}:{}",
            message.conversation_id, message.message_id
        ),
        source_kind: MessageDocumentKind::TeamConversationMessage,
        source_id: message.message_id.to_string(),
        logical_message_id: None,
        authority_message_id: Some(message.message_id),
        correlation_id: message
            .payload
            .get("correlation_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        group_id: message.group_id.clone(),
        team_id: Some(conversation.team_id.clone()),
        run_id: None,
        conversation_id: Some(message.conversation_id.clone()),
        task_id: Some(message.task_id.clone()),
        agent_id: None,
        session_id: None,
        body_text: message_archive_body_text(&message.payload),
        payload_json: Some(message.payload.to_string()),
        created_at: message.created_at,
        event_id_from: None,
        event_id_to: None,
        chunk_count: None,
    }
}

fn task_conversation_payload_correlation_id(payload: &Value) -> String {
    payload
        .get("correlation_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

#[derive(Debug, Clone, Default)]
struct MessageArchiveScopeFallback {
    conversation_id: Option<String>,
    task_id: Option<String>,
}

impl MessageArchiveScopeFallback {
    fn from_run_input(run_input: &Value) -> Self {
        Self {
            conversation_id: message_archive_payload_string(run_input, "conversation_id"),
            task_id: message_archive_payload_string(run_input, "task_id"),
        }
    }
}

async fn message_archive_task_conversation_id_db(
    db: &SqlitePool,
    team_id: &str,
    task_id: &str,
) -> anyhow::Result<Option<String>> {
    Ok(sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM team_conversations
        WHERE team_id = ?1 AND task_id = ?2
        LIMIT 1
        "#,
    )
    .bind(team_id)
    .bind(task_id)
    .fetch_optional(db)
    .await?)
}

async fn message_archive_scope_for_payload_db(
    db: &SqlitePool,
    team_id: &str,
    payload: &Value,
    base_scope: &MessageArchiveScopeFallback,
    task_conversation_cache: &mut HashMap<(String, String), Option<String>>,
) -> anyhow::Result<MessageArchiveScopeFallback> {
    let task_id =
        message_archive_payload_string(payload, "task_id").or_else(|| base_scope.task_id.clone());
    let mut conversation_id = base_scope.conversation_id.clone();
    if conversation_id.is_none()
        && let Some(task_id) = task_id.as_deref()
    {
        let cache_key = (team_id.to_string(), task_id.to_string());
        if !task_conversation_cache.contains_key(&cache_key) {
            let resolved = message_archive_task_conversation_id_db(db, team_id, task_id).await?;
            task_conversation_cache.insert(cache_key.clone(), resolved);
        }
        conversation_id = task_conversation_cache.get(&cache_key).cloned().flatten();
    }
    Ok(MessageArchiveScopeFallback {
        conversation_id,
        task_id,
    })
}

pub(super) async fn team_run_event_archive_document_for_db(
    db: &SqlitePool,
    event: &TeamRunEventRecord,
) -> anyhow::Result<Option<MessageDocument>> {
    let mut run_scope_cache = HashMap::new();
    let mut task_conversation_cache = HashMap::new();
    team_run_event_archive_document_for_db_cached(
        db,
        event,
        &mut run_scope_cache,
        &mut task_conversation_cache,
    )
    .await
}

async fn team_run_event_archive_document_for_db_cached(
    db: &SqlitePool,
    event: &TeamRunEventRecord,
    run_scope_cache: &mut HashMap<String, Option<(String, MessageArchiveScopeFallback)>>,
    task_conversation_cache: &mut HashMap<(String, String), Option<String>>,
) -> anyhow::Result<Option<MessageDocument>> {
    if !run_scope_cache.contains_key(&event.run_id) {
        let resolved = team_run_event_archive_scope_for_db(db, &event.run_id).await?;
        run_scope_cache.insert(event.run_id.clone(), resolved);
    }
    let Some(Some((team_id, base_scope))) = run_scope_cache.get(&event.run_id) else {
        return Ok(None);
    };
    let scope = message_archive_scope_for_payload_db(
        db,
        team_id,
        &event.payload,
        base_scope,
        task_conversation_cache,
    )
    .await?;
    Ok(Some(team_run_event_archive_document(
        team_id, event, &scope,
    )))
}

async fn team_run_event_archive_scope_for_db(
    db: &SqlitePool,
    run_id: &str,
) -> anyhow::Result<Option<(String, MessageArchiveScopeFallback)>> {
    let row = sqlx::query(
        r#"
        SELECT team_id, input_json
        FROM team_runs
        WHERE id = ?1
        "#,
    )
    .bind(run_id)
    .fetch_one(db)
    .await?;
    let team_id: String = row.get("team_id");
    let input_json: String = row.get("input_json");
    let run_input: Value = serde_json::from_str(&input_json)?;
    if message_archive_payload_string(&run_input, "bootstrap_kind").as_deref()
        == Some(TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND)
    {
        return Ok(None);
    }
    Ok(Some((
        team_id,
        MessageArchiveScopeFallback::from_run_input(&run_input),
    )))
}

fn team_run_event_archive_document(
    team_id: &str,
    event: &TeamRunEventRecord,
    scope: &MessageArchiveScopeFallback,
) -> MessageDocument {
    let body_text = message_archive_body_text(&event.payload);
    let redacted_payload = redact_sensitive_json(&event.payload);
    MessageDocument {
        document_id: format!("team_run_event:{}:{}", event.run_id, event.event_id),
        source_kind: MessageDocumentKind::TeamRunEvent,
        source_id: event.event_id.to_string(),
        logical_message_id: None,
        authority_message_id: message_archive_payload_i64(&event.payload, "authority_message_id"),
        correlation_id: message_archive_payload_string(&event.payload, "correlation_id"),
        group_id: None,
        team_id: Some(team_id.to_string()),
        run_id: Some(event.run_id.clone()),
        conversation_id: message_archive_payload_string(&event.payload, "conversation_id")
            .or_else(|| scope.conversation_id.clone()),
        task_id: message_archive_payload_string(&event.payload, "task_id")
            .or_else(|| scope.task_id.clone()),
        agent_id: None,
        session_id: None,
        body_text: if body_text.is_empty() {
            event.event_type.clone()
        } else {
            body_text
        },
        payload_json: Some(redacted_payload.to_string()),
        created_at: event.ts,
        event_id_from: Some(event.event_id),
        event_id_to: Some(event.event_id),
        chunk_count: None,
    }
}

fn team_actor_message_archive_document(
    team_id: &str,
    message: &TeamActorMessageRecord,
    scope: &MessageArchiveScopeFallback,
) -> MessageDocument {
    let redacted_payload = redact_sensitive_json(&message.payload);
    MessageDocument {
        document_id: format!(
            "team_actor_message:{}:{}",
            message.run_id, message.message_id
        ),
        source_kind: MessageDocumentKind::TeamActorMessage,
        source_id: message.message_id.to_string(),
        logical_message_id: None,
        authority_message_id: message_archive_payload_i64(&message.payload, "authority_message_id"),
        correlation_id: message_archive_payload_string(&message.payload, "correlation_id"),
        group_id: None,
        team_id: Some(team_id.to_string()),
        run_id: Some(message.run_id.clone()),
        conversation_id: message_archive_payload_string_any(
            &message.payload,
            &[
                "task_conversation_id",
                "channel_conversation_id",
                "conversation_id",
            ],
        )
        .or_else(|| scope.conversation_id.clone()),
        task_id: message_archive_payload_string(&message.payload, "task_id")
            .or_else(|| scope.task_id.clone()),
        agent_id: Some(message.to_actor_id.clone()),
        session_id: None,
        body_text: message_archive_body_text(&message.payload),
        payload_json: Some(redacted_payload.to_string()),
        created_at: message.created_at,
        event_id_from: None,
        event_id_to: None,
        chunk_count: None,
    }
}

#[derive(Debug, Clone)]
struct AgentEventArchiveRow {
    event_id: i64,
    agent_id: String,
    session_id: String,
    ts: i64,
    stream: String,
    message: String,
}

#[derive(Debug, Clone, Copy, Default)]
struct AgentEventArchiveMigrationCounts {
    agent_events: usize,
    aggregated_acp_messages: usize,
}

impl AgentEventArchiveMigrationCounts {
    fn add(&mut self, other: AgentEventArchiveMigrationCounts) {
        self.agent_events += other.agent_events;
        self.aggregated_acp_messages += other.aggregated_acp_messages;
    }
}

#[derive(Debug, Clone, Default)]
struct AgentEventArchiveScope {
    team_id: Option<String>,
    run_id: Option<String>,
    conversation_id: Option<String>,
    task_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct AgentEventArchiveSnapshot {
    main_max_id: i64,
    per_agent_max_ids: Vec<(String, i64)>,
}

fn agent_event_archive_document(
    row: &AgentEventArchiveRow,
    scope: Option<&AgentEventArchiveScope>,
) -> MessageDocument {
    let parsed_message = serde_json::from_str::<Value>(row.message.as_str()).ok();
    let body_text = parsed_message
        .as_ref()
        .map(message_archive_body_text)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            parsed_message
                .as_ref()
                .and_then(|payload| payload.get("type"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| row.message.trim().to_string());
    let payload_json = parsed_message
        .as_ref()
        .map(redact_sensitive_json)
        .unwrap_or_else(|| {
            serde_json::json!({
                "stream": row.stream.as_str(),
                "message": row.message.as_str(),
            })
        })
        .to_string();

    MessageDocument {
        document_id: format!(
            "agent_event:{}:{}:{}",
            row.agent_id, row.session_id, row.event_id
        ),
        source_kind: MessageDocumentKind::AgentEvent,
        source_id: row.event_id.to_string(),
        logical_message_id: None,
        authority_message_id: None,
        correlation_id: parsed_message
            .as_ref()
            .and_then(|payload| message_archive_payload_string(payload, "correlation_id")),
        group_id: None,
        team_id: scope.and_then(|scope| scope.team_id.clone()),
        run_id: scope.and_then(|scope| scope.run_id.clone()),
        conversation_id: scope.and_then(|scope| scope.conversation_id.clone()),
        task_id: scope.and_then(|scope| scope.task_id.clone()),
        agent_id: Some(row.agent_id.clone()),
        session_id: Some(row.session_id.clone()),
        body_text,
        payload_json: Some(payload_json),
        created_at: row.ts,
        event_id_from: Some(row.event_id),
        event_id_to: Some(row.event_id),
        chunk_count: None,
    }
}

fn aggregated_acp_message_archive_document(
    message: &AcpAggregatedMessage,
    scope: Option<&AgentEventArchiveScope>,
) -> MessageDocument {
    MessageDocument {
        document_id: format!(
            "aggregated_acp_message:{}:{}:{}:{}",
            message.agent_id, message.session_id, message.logical_message_id, message.message_kind
        ),
        source_kind: MessageDocumentKind::AggregatedAcpMessage,
        source_id: format!(
            "{}:{}:{}:{}",
            message.agent_id, message.session_id, message.logical_message_id, message.message_kind
        ),
        logical_message_id: Some(message.logical_message_id.clone()),
        authority_message_id: None,
        correlation_id: None,
        group_id: None,
        team_id: scope.and_then(|scope| scope.team_id.clone()),
        run_id: scope.and_then(|scope| scope.run_id.clone()),
        conversation_id: scope.and_then(|scope| scope.conversation_id.clone()),
        task_id: scope.and_then(|scope| scope.task_id.clone()),
        agent_id: Some(message.agent_id.clone()),
        session_id: Some(message.session_id.clone()),
        body_text: message.text.clone(),
        payload_json: Some(
            serde_json::json!({
                "type": message.message_kind.as_str(),
                "message_id": message.logical_message_id.as_str(),
                "text": message.text.as_str(),
                "event_id_from": message.first_event_id,
                "event_id_to": message.last_event_id,
                "chunk_count": message.chunk_count,
            })
            .to_string(),
        ),
        created_at: message.created_at,
        event_id_from: Some(message.first_event_id),
        event_id_to: Some(message.last_event_id),
        chunk_count: Some(message.chunk_count),
    }
}

fn message_archive_payload_string_any(payload: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| message_archive_payload_string(payload, key))
}

fn message_archive_payload_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn message_archive_payload_i64(payload: &Value, key: &str) -> Option<i64> {
    match payload.get(key)? {
        Value::Number(value) => value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|raw| i64::try_from(raw).ok())),
        Value::String(value) => value.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn message_archive_body_text(payload: &Value) -> String {
    payload
        .as_str()
        .or_else(|| payload.get("text").and_then(Value::as_str))
        .or_else(|| payload.get("summary").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_default()
}

fn build_step_runtime_handle_event_payload(step: &TeamStepRecord, status: &'static str) -> Value {
    serde_json::json!({
        "step_id": step.id,
        "step_key": step.step_key,
        "status": status,
        "runtime_handle_id": step.runtime_handle_id,
    })
}

fn build_continuity_event_payload(
    continuity_state: &TeamMemberContinuityStateRecord,
    step: &TeamStepRecord,
    continuity_mode: &str,
    artifact_offload_status: &str,
) -> Value {
    serde_json::json!({
        "team_id": continuity_state.team_id,
        "member_id": continuity_state.member_id,
        "step_id": step.id,
        "step_key": step.step_key,
        "mode": continuity_mode,
        "source_run_id": continuity_state.source_run_id,
        "source_runtime_handle_id": continuity_state.source_session_id,
        "summary_chars": continuity_state.summary_text.chars().count(),
        "artifact_offload_status": artifact_offload_status,
    })
}

#[derive(Debug, thiserror::Error)]
enum TaskConversationMessageStoreError {
    #[error("idempotency_key conflicts with an existing task conversation message payload")]
    IdempotencyConflict,
}

#[derive(Debug, Clone)]
struct TeamMemberContextWorkspace {
    runtime_workdir: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TeamConversationStreamEvent {
    pub team_id: String,
    pub task_id: String,
    pub conversation_id: String,
    pub message_id: Option<i64>,
    pub source: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TeamRunContextStreamEvent {
    pub team_id: String,
    pub run_id: String,
    pub refresh_run: bool,
    pub refresh_events: bool,
    pub refresh_snapshot: bool,
    pub refresh_mailbox: bool,
    pub latest_event_id: Option<i64>,
    pub latest_mailbox_message_id: Option<i64>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamRunContextFingerprint {
    pub team_id: String,
    pub run_id: String,
    pub run_status: String,
    pub latest_event_id: i64,
    pub latest_mailbox_message_id: i64,
    pub mailbox_pending: i64,
    pub mailbox_delivered: i64,
    pub mailbox_dead_letter: i64,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharedThreadTargetRecord {
    pub task_id: String,
    pub conversation_id: String,
}

#[derive(Debug, Clone)]
struct MaterializedRunStepTemplate {
    step_key: String,
    member_id: String,
    depends_on: Vec<String>,
    input: Option<Value>,
}

#[derive(Debug, Clone)]
struct ReconcileRoundRuntime {
    current_round: i64,
    goal: Option<String>,
    acceptance: Vec<String>,
    execution: TeamTaskStepExecutionSpec,
}

pub(crate) async fn fetch_canonical_shared_thread_target<'e, E>(
    executor: E,
    team_id: &str,
) -> Result<Option<SharedThreadTargetRecord>, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query(
        r#"
        SELECT
            t.id AS task_id,
            c.id AS conversation_id,
            (
                SELECT MAX(m.id)
                FROM team_conversation_messages m
                WHERE m.conversation_id = c.id
            ) AS latest_message_id
        FROM team_tasks t
        INNER JOIN team_conversations c ON c.task_id = t.id
        WHERE t.team_id = ?1
          AND (
            lower(trim(t.title)) = ?2
            OR lower(trim(COALESCE(json_extract(t.context_json, '$.bootstrap_kind'), ''))) = ?3
          )
        ORDER BY
            CASE WHEN latest_message_id IS NULL THEN 1 ELSE 0 END ASC,
            latest_message_id DESC,
            c.created_at ASC,
            t.created_at ASC,
            t.id ASC
        LIMIT 1
        "#,
    )
    .bind(team_id)
    .bind(TEAM_SHARED_THREAD_TITLE)
    .bind(TEAM_SHARED_THREAD_BOOTSTRAP_KIND)
    .fetch_optional(executor)
    .await?;

    Ok(row.map(|row| SharedThreadTargetRecord {
        task_id: row.get("task_id"),
        conversation_id: row.get("conversation_id"),
    }))
}

#[derive(Debug, Clone)]
struct TeamChannelTargetRecord {
    task_id: String,
    conversation_id: String,
    channel_id: String,
    description: Option<String>,
    created_by_actor_id: String,
    created_at: i64,
    updated_at: i64,
}

async fn fetch_team_channel_target<'e, E>(
    executor: E,
    team_id: &str,
    channel_id: &str,
) -> Result<Option<TeamChannelTargetRecord>, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query(
        r#"
        SELECT
            t.id AS task_id,
            c.id AS conversation_id,
            lower(trim(COALESCE(json_extract(t.context_json, '$.channel_id'), ''))) AS channel_id,
            json_extract(t.context_json, '$.description') AS description,
            t.created_by_actor_id,
            t.created_at,
            t.updated_at
        FROM team_tasks t
        INNER JOIN team_conversations c ON c.task_id = t.id
        WHERE t.team_id = ?1
          AND c.team_id = ?1
          AND c.mode = 'group_chat'
          AND lower(trim(COALESCE(json_extract(t.context_json, '$.bootstrap_kind'), ''))) = ?2
          AND lower(trim(COALESCE(json_extract(t.context_json, '$.channel_id'), ''))) = ?3
        ORDER BY c.created_at ASC, t.created_at ASC, t.id ASC
        LIMIT 1
        "#,
    )
    .bind(team_id)
    .bind(TEAM_CHANNEL_BOOTSTRAP_KIND)
    .bind(channel_id.trim().to_ascii_lowercase())
    .fetch_optional(executor)
    .await?;

    Ok(row.map(|row| TeamChannelTargetRecord {
        task_id: row.get("task_id"),
        conversation_id: row.get("conversation_id"),
        channel_id: row.get("channel_id"),
        description: row
            .try_get::<Option<String>, _>("description")
            .ok()
            .flatten(),
        created_by_actor_id: row.get("created_by_actor_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }))
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

#[derive(Debug, thiserror::Error)]
pub enum TeamContextLookupError {
    #[error("team_id or run_id is required")]
    MissingSelector,
    #[error("run_id {run_id} belongs to team {actual_team_id}, not {requested_team_id}")]
    RunTeamMismatch {
        run_id: String,
        actual_team_id: String,
        requested_team_id: String,
    },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamTaskAssignmentUpdate {
    Unchanged,
    Unassigned,
    Assigned(String),
}

#[derive(Debug, Clone, Default)]
pub struct TeamTaskListQuery {
    pub team_id: Option<String>,
    pub run_id: Option<String>,
    pub limit: i64,
    pub status: Option<TeamTaskStatus>,
    pub task_id: Option<String>,
    pub assigned_member_id: Option<String>,
    pub topic: Option<String>,
    pub include_shared_thread: bool,
}

#[derive(Debug, Clone)]
pub enum TeamTaskContextPatch {
    Replace(Value),
    Merge(Value),
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
    pub role: String,
    pub skills: Vec<String>,
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
    pub(crate) fn task_message_idempotency_conflict_error() -> anyhow::Error {
        TaskConversationMessageStoreError::IdempotencyConflict.into()
    }

    pub fn is_task_message_idempotency_conflict(err: &anyhow::Error) -> bool {
        err.downcast_ref::<TaskConversationMessageStoreError>()
            .is_some_and(|cause| {
                matches!(
                    cause,
                    TaskConversationMessageStoreError::IdempotencyConflict
                )
            })
    }

    #[cfg(test)]
    pub fn new(db: SqlitePool) -> Self {
        Self::new_with_event_dbs(db, AgentEventDbRouter::with_default_base_dir())
    }

    #[cfg(test)]
    pub fn new_with_event_dbs(db: SqlitePool, event_dbs: AgentEventDbRouter) -> Self {
        Self::new_with_event_dbs_and_message_archive(db, event_dbs, None)
    }

    pub fn new_with_event_dbs_and_message_archive(
        db: SqlitePool,
        event_dbs: AgentEventDbRouter,
        message_archive: Option<MessageArchiveStoreRef>,
    ) -> Self {
        let (conversation_events, _) = broadcast::channel(TEAM_CONVERSATION_STREAM_BUFFER_CAPACITY);
        let remote_relay_adapter = Arc::new(TeamRemoteRelayAdapter::new(db.clone()));
        let agents_target_node_id_column = Arc::new(Mutex::new(None));
        Self {
            db,
            event_dbs,
            message_archive,
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
        let group_id = owner_user_id
            .map(str::trim)
            .filter(|value| !value.is_empty());
        sqlx::query(
            r#"
            INSERT INTO team_definitions (
                id,
                name,
                description,
                spec_json,
                owner_user_id,
                group_id,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(&id)
        .bind(&config.name)
        .bind(&config.description)
        .bind(spec_json)
        .bind(owner_user_id)
        .bind(group_id)
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

    pub async fn list_teams_referencing_member(
        &self,
        member_id: &str,
    ) -> anyhow::Result<Vec<TeamDefinitionRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT td.id, td.name, td.description, td.spec_json, td.owner_user_id, td.created_at, td.updated_at
            FROM team_definitions AS td,
                 json_each(td.spec_json, '$.members') AS member
            WHERE json_extract(member.value, '$.member_id') = ?1
            ORDER BY td.created_at DESC
            "#,
        )
        .bind(member_id)
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
        let team = self.get_team(team_id).await?;
        validate_task_execution_plan(&team.spec, &context)?;
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
                id, team_id, group_id, title, status, created_by_actor_id, assigned_member_id, context_json, created_at, updated_at
            )
            VALUES (?1, ?2, (SELECT group_id FROM team_definitions WHERE id = ?2), ?3, ?4, ?5, NULL, ?6, ?7, ?8)
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

    pub async fn list_tasks_with_query(
        &self,
        query: TeamTaskListQuery,
    ) -> anyhow::Result<Vec<TeamTaskRecord>> {
        let team_id = self
            .resolve_team_scope(query.team_id.as_deref(), query.run_id.as_deref())
            .await?;
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                t.id,
                t.team_id,
                t.title,
                t.status,
                t.created_by_actor_id,
                t.assigned_member_id,
                t.context_json,
                t.created_at,
                t.updated_at
            FROM team_tasks AS t
            LEFT JOIN team_conversations AS c ON c.task_id = t.id
            WHERE t.team_id = "#,
        );
        builder.push_bind(&team_id);
        if let Some(status) = query.status {
            builder.push(" AND t.status = ");
            builder.push_bind(team_task_status_to_str(&status));
        }
        if let Some(task_id) = query.task_id.as_deref() {
            builder.push(" AND t.id = ");
            builder.push_bind(task_id);
        }
        if let Some(assigned_member_id) = query.assigned_member_id.as_deref() {
            builder.push(" AND t.assigned_member_id = ");
            builder.push_bind(assigned_member_id);
        }
        if let Some(topic) = query.topic.as_deref() {
            builder.push(" AND trim(COALESCE(c.topic, '')) = ");
            builder.push_bind(topic);
        }
        if !query.include_shared_thread {
            builder.push(
                " AND lower(trim(t.title)) != 'all' \
                 AND lower(trim(COALESCE(json_extract(t.context_json, '$.bootstrap_kind'), ''))) != ",
            );
            builder.push_bind("shared_thread");
        }
        builder.push(
            " AND lower(trim(COALESCE(json_extract(t.context_json, '$.bootstrap_kind'), ''))) != ",
        );
        builder.push_bind(TEAM_CHANNEL_BOOTSTRAP_KIND);
        builder.push(" ORDER BY t.updated_at DESC, t.id DESC LIMIT ");
        builder.push_bind(query.limit.max(1));
        let rows = builder.build().fetch_all(&self.db).await?;
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

    async fn get_task_for_team(
        &self,
        team_id: &str,
        task_id: &str,
    ) -> anyhow::Result<TeamTaskRecord> {
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
              AND team_id = ?2
            "#,
        )
        .bind(task_id)
        .bind(team_id)
        .fetch_optional(&self.db)
        .await?;
        let row = row
            .ok_or_else(|| anyhow::anyhow!("linked task does not belong to the requested team"))?;
        parse_team_task_row(&row)
    }

    pub async fn get_task_detail(
        &self,
        task_id: &str,
        message_limit: i64,
    ) -> anyhow::Result<super::TeamTaskDetailRecord> {
        let task = self.get_task(task_id).await?;
        let conversation = self.get_task_conversation(task_id).await?;
        let latest_run = self.get_latest_run_for_task(&task.team_id, task_id).await?;
        let recent_messages = self
            .list_task_conversation_messages(
                task_id,
                message_limit.clamp(1, TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX),
                None,
            )
            .await?;
        Ok(super::TeamTaskDetailRecord {
            task,
            conversation,
            latest_run,
            recent_messages,
        })
    }

    pub async fn list_channels(
        &self,
        team_id: &str,
    ) -> anyhow::Result<Vec<super::TeamChannelRecord>> {
        let normalized_team_id = team_id.trim();
        if normalized_team_id.is_empty() {
            anyhow::bail!("team_id is required");
        }

        self.get_team(normalized_team_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT
                t.id AS task_id,
                c.id AS conversation_id,
                lower(trim(COALESCE(json_extract(t.context_json, '$.channel_id'), ''))) AS channel_id,
                json_extract(t.context_json, '$.description') AS description,
                t.created_by_actor_id,
                t.created_at,
                t.updated_at
            FROM team_tasks t
            INNER JOIN team_conversations c ON c.task_id = t.id
            WHERE t.team_id = ?1
              AND c.team_id = ?1
              AND c.mode = 'group_chat'
              AND lower(trim(COALESCE(json_extract(t.context_json, '$.bootstrap_kind'), ''))) = ?2
              AND lower(trim(COALESCE(json_extract(t.context_json, '$.channel_id'), ''))) <> ''
            ORDER BY c.created_at ASC, c.rowid ASC, t.created_at ASC, t.rowid ASC
            "#,
        )
        .bind(normalized_team_id)
        .bind(TEAM_CHANNEL_BOOTSTRAP_KIND)
        .fetch_all(&self.db)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| super::TeamChannelRecord {
                team_id: normalized_team_id.to_string(),
                task_id: row.get("task_id"),
                conversation_id: row.get("conversation_id"),
                channel_id: row.get("channel_id"),
                description: row
                    .try_get::<Option<String>, _>("description")
                    .ok()
                    .flatten(),
                created_by_actor_id: row.get("created_by_actor_id"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn open_thread(
        &self,
        team_id: &str,
        channel_id: &str,
        root_message_id: i64,
    ) -> anyhow::Result<super::TeamThreadOpenRecord> {
        let normalized_team_id = team_id.trim();
        if normalized_team_id.is_empty() {
            anyhow::bail!("team_id is required");
        }
        let normalized_channel_id = normalize_team_channel_id(channel_id)?;
        if root_message_id <= 0 {
            anyhow::bail!("root_message_id must be positive");
        }

        let (task_id, conversation_id, resolved_channel_id) =
            if normalized_channel_id.eq_ignore_ascii_case(TEAM_SHARED_THREAD_TITLE) {
                let target = fetch_canonical_shared_thread_target(&self.db, normalized_team_id)
                    .await?
                    .ok_or_else(|| {
                        anyhow::anyhow!("shared thread is missing for team {}", normalized_team_id)
                    })?;
                (
                    target.task_id,
                    target.conversation_id,
                    TEAM_SHARED_THREAD_TITLE.to_string(),
                )
            } else {
                let row =
                    fetch_team_channel_target(&self.db, normalized_team_id, &normalized_channel_id)
                        .await?
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "channel '{}' not found for team {}",
                                normalized_channel_id,
                                normalized_team_id
                            )
                        })?;
                (row.task_id, row.conversation_id, row.channel_id)
            };

        let root_exists = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT 1
            FROM team_conversation_messages
            WHERE id = ?1 AND conversation_id = ?2 AND task_id = ?3
            LIMIT 1
            "#,
        )
        .bind(root_message_id)
        .bind(&conversation_id)
        .bind(&task_id)
        .fetch_optional(&self.db)
        .await?;
        if root_exists.is_none() {
            anyhow::bail!(
                "root_message_id {} was not found in channel '{}'",
                root_message_id,
                resolved_channel_id
            );
        }

        Ok(super::TeamThreadOpenRecord {
            team_id: normalized_team_id.to_string(),
            channel_id: resolved_channel_id,
            task_id,
            conversation_id,
            root_message_id,
            thread_id: root_message_id.to_string(),
        })
    }

    pub async fn reply_thread(
        &self,
        team_id: &str,
        channel_id: &str,
        root_message_id: i64,
        from_actor_id: &str,
        text: &str,
        mention_actor_ids: &[String],
    ) -> anyhow::Result<super::TeamThreadReplyRecord> {
        let normalized_actor_id = from_actor_id.trim();
        if normalized_actor_id.is_empty() {
            anyhow::bail!("from_actor_id is required");
        }
        let normalized_text = text.trim();
        if normalized_text.is_empty() {
            anyhow::bail!("text is required");
        }

        let normalized_mentions = mention_actor_ids
            .iter()
            .map(|actor_id| actor_id.trim())
            .filter(|actor_id| !actor_id.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let thread = self
            .open_thread(team_id, channel_id, root_message_id)
            .await?;
        let message = self
            .append_task_conversation_message(
                &thread.task_id,
                normalized_actor_id,
                None,
                "team_thread_reply",
                serde_json::json!({
                    "type": "chat_message",
                    "text": normalized_text,
                    "mention_actor_ids": normalized_mentions,
                    "thread_root_message_id": root_message_id,
                }),
            )
            .await?;

        Ok(super::TeamThreadReplyRecord { thread, message })
    }

    pub async fn create_channel(
        &self,
        team_id: &str,
        channel_id: &str,
        description: Option<&str>,
        created_by_actor_id: &str,
    ) -> anyhow::Result<super::TeamChannelRecord> {
        let normalized_team_id = team_id.trim();
        if normalized_team_id.is_empty() {
            anyhow::bail!("team_id is required");
        }
        let normalized_created_by_actor_id = created_by_actor_id.trim();
        if normalized_created_by_actor_id.is_empty() {
            anyhow::bail!("created_by_actor_id is required");
        }
        let normalized_channel_id = normalize_team_channel_id(channel_id)?;
        if normalized_channel_id.eq_ignore_ascii_case(TEAM_SHARED_THREAD_TITLE) {
            anyhow::bail!("channel_id 'all' is reserved");
        }
        let normalized_description = description
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        self.get_team(normalized_team_id).await?;
        let now = Utc::now().timestamp();
        let task_id = Uuid::new_v4().to_string();
        let conversation_id = Uuid::new_v4().to_string();
        let context_json = serde_json::json!({
            "bootstrap_kind": TEAM_CHANNEL_BOOTSTRAP_KIND,
            "bootstrap_source": TEAM_CHANNEL_BOOTSTRAP_SOURCE,
            "channel_id": normalized_channel_id,
            "description": normalized_description,
        })
        .to_string();

        let mut tx = self.db.begin().await?;
        let existing =
            fetch_team_channel_target(&mut *tx, normalized_team_id, &normalized_channel_id).await?;
        if existing.is_some() {
            anyhow::bail!(
                "channel '{}' already exists for team {}",
                normalized_channel_id,
                normalized_team_id
            );
        }

        sqlx::query(
            r#"
            INSERT INTO team_tasks (
                id, team_id, group_id, title, status, created_by_actor_id, assigned_member_id, context_json, created_at, updated_at
            )
            VALUES (?1, ?2, (SELECT group_id FROM team_definitions WHERE id = ?2), ?3, 'open', ?4, NULL, ?5, ?6, ?7)
            "#,
        )
        .bind(&task_id)
        .bind(normalized_team_id)
        .bind(&normalized_channel_id)
        .bind(normalized_created_by_actor_id)
        .bind(context_json)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|err| {
            if is_team_channel_bootstrap_unique_violation(&err) {
                anyhow::anyhow!(
                    "channel '{}' already exists for team {}",
                    normalized_channel_id,
                    normalized_team_id
                )
            } else {
                err.into()
            }
        })?;

        sqlx::query(
            r#"
            INSERT INTO team_conversations (
                id, team_id, task_id, mode, topic, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, 'group_chat', ?4, ?5, ?6)
            "#,
        )
        .bind(&conversation_id)
        .bind(normalized_team_id)
        .bind(&task_id)
        .bind(&normalized_channel_id)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(super::TeamChannelRecord {
            team_id: normalized_team_id.to_string(),
            channel_id: normalized_channel_id,
            task_id,
            conversation_id,
            description: normalized_description,
            created_by_actor_id: normalized_created_by_actor_id.to_string(),
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn delete_channel(
        &self,
        team_id: &str,
        channel_id: &str,
    ) -> anyhow::Result<super::TeamChannelRecord> {
        let normalized_team_id = team_id.trim();
        if normalized_team_id.is_empty() {
            anyhow::bail!("team_id is required");
        }
        let normalized_channel_id = normalize_team_channel_id(channel_id)?;
        if normalized_channel_id.eq_ignore_ascii_case(TEAM_SHARED_THREAD_TITLE) {
            anyhow::bail!("channel_id 'all' cannot be deleted");
        }

        let mut tx = self.db.begin().await?;
        let row = fetch_team_channel_target(&mut *tx, normalized_team_id, &normalized_channel_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "channel '{}' not found for team {}",
                    normalized_channel_id,
                    normalized_team_id
                )
            })?;

        let task_id = row.task_id;
        let conversation_id = row.conversation_id;
        let channel_id = row.channel_id;
        let description = row.description;
        let created_at = row.created_at;
        let updated_at = row.updated_at;
        let created_by_actor_id = row.created_by_actor_id;

        sqlx::query(
            "DELETE FROM team_channel_message_replicas WHERE conversation_id = ?1 OR task_id = ?2",
        )
        .bind(&conversation_id)
        .bind(&task_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM team_conversation_messages WHERE conversation_id = ?1")
            .bind(&conversation_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM team_conversations WHERE id = ?1")
            .bind(&conversation_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM team_tasks WHERE id = ?1")
            .bind(&task_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        Ok(super::TeamChannelRecord {
            team_id: normalized_team_id.to_string(),
            channel_id,
            task_id,
            conversation_id,
            description,
            created_by_actor_id,
            created_at,
            updated_at,
        })
    }

    pub(crate) async fn resolve_team_scope(
        &self,
        team_id: Option<&str>,
        run_id: Option<&str>,
    ) -> anyhow::Result<String> {
        let normalized_team_id = team_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let normalized_run_id = run_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if let Some(run_id) = normalized_run_id.as_deref() {
            let run = self.get_run(run_id).await?;
            if let Some(explicit_team_id) = normalized_team_id.as_deref()
                && explicit_team_id != run.team_id
            {
                return Err(TeamContextLookupError::RunTeamMismatch {
                    run_id: run_id.to_string(),
                    actual_team_id: run.team_id,
                    requested_team_id: explicit_team_id.to_string(),
                }
                .into());
            }
            return Ok(run.team_id);
        }
        normalized_team_id.ok_or(TeamContextLookupError::MissingSelector.into())
    }

    pub async fn update_task_status(
        &self,
        task_id: &str,
        status: TeamTaskStatus,
    ) -> anyhow::Result<TeamTaskRecord> {
        self.update_task(task_id, Some(status), TeamTaskAssignmentUpdate::Unchanged)
            .await
    }

    pub async fn update_task(
        &self,
        task_id: &str,
        status: Option<TeamTaskStatus>,
        assignment: TeamTaskAssignmentUpdate,
    ) -> anyhow::Result<TeamTaskRecord> {
        self.update_task_with_context(task_id, status, assignment, None)
            .await
    }

    pub async fn update_task_with_context(
        &self,
        task_id: &str,
        status: Option<TeamTaskStatus>,
        assignment: TeamTaskAssignmentUpdate,
        context_patch: Option<TeamTaskContextPatch>,
    ) -> anyhow::Result<TeamTaskRecord> {
        let current = self.get_task(task_id).await?;
        let team = self.get_team(&current.team_id).await?;
        let status_patch = status.filter(|candidate| *candidate != current.status);
        let assignment_patch = match assignment {
            TeamTaskAssignmentUpdate::Unchanged => None,
            TeamTaskAssignmentUpdate::Unassigned => {
                if current.assigned_member_id.is_none() {
                    None
                } else {
                    Some(None)
                }
            }
            TeamTaskAssignmentUpdate::Assigned(member_id) => {
                if current.assigned_member_id.as_deref() == Some(member_id.as_str()) {
                    None
                } else {
                    Some(Some(member_id))
                }
            }
        };
        let context_patch = context_patch
            .and_then(|patch| resolve_task_context_patch(&current.context, patch))
            .map(|next_context| {
                validate_task_execution_plan(&team.spec, &next_context)?;
                Ok::<_, anyhow::Error>(next_context)
            })
            .transpose()?;
        if status_patch.is_none() && assignment_patch.is_none() && context_patch.is_none() {
            return Ok(current);
        }

        let now = Utc::now().timestamp();
        let mut builder = QueryBuilder::<Sqlite>::new("UPDATE team_tasks SET ");
        let mut first = true;
        if let Some(next_status) = status_patch.as_ref() {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push("status = ");
            builder.push_bind(team_task_status_to_str(next_status));
        }
        if let Some(next_assignment) = assignment_patch.as_ref() {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push("assigned_member_id = ");
            builder.push_bind(next_assignment);
        }
        if let Some(next_context) = context_patch.as_ref() {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push("context_json = ");
            builder.push_bind(next_context.to_string());
        }
        if !first {
            builder.push(", ");
        }
        builder.push("updated_at = ");
        builder.push_bind(now);
        builder.push(" WHERE id = ");
        builder.push_bind(task_id);
        builder.build().execute(&self.db).await?;

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
        let (message, _created) = self
            .append_task_conversation_message_with_created(
                task_id,
                from_actor_id,
                to_actor_id,
                route,
                payload,
                None,
            )
            .await?;
        Ok(message)
    }

    pub async fn append_task_conversation_message_with_created(
        &self,
        task_id: &str,
        from_actor_id: &str,
        to_actor_id: Option<&str>,
        route: &str,
        payload: Value,
        idempotency_key: Option<&str>,
    ) -> anyhow::Result<(TeamConversationMessageRecord, bool)> {
        let now = Utc::now().timestamp();
        let conversation = self.get_task_conversation(task_id).await?;
        let redacted_payload = redact_sensitive_json(&payload);
        let payload_json = redacted_payload.to_string();
        let correlation_id = task_conversation_payload_correlation_id(&redacted_payload);
        let group_id = sqlx::query_scalar::<_, Option<String>>(
            "SELECT group_id FROM team_tasks WHERE id = ?1",
        )
        .bind(task_id)
        .fetch_optional(&self.db)
        .await?
        .flatten();
        let to_actor_id = to_actor_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let idempotency_key = normalize_optional_idempotency_key_input(idempotency_key);

        let (message, created) = if let Some(idempotency_key) = idempotency_key.as_deref() {
            let mut tx = self.db.begin().await?;
            let outcome = match sqlx::query(
                r#"
                INSERT INTO team_conversation_messages (
                    conversation_id,
                    task_id,
                    from_actor_id,
                    to_actor_id,
                    route,
                    correlation_id,
                    group_id,
                    payload_json,
                    idempotency_key,
                    created_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                "#,
            )
            .bind(&conversation.id)
            .bind(task_id)
            .bind(from_actor_id)
            .bind(to_actor_id.as_deref())
            .bind(route)
            .bind(&correlation_id)
            .bind(group_id.as_deref())
            .bind(&payload_json)
            .bind(idempotency_key)
            .bind(now)
            .execute(&mut *tx)
            .await
            {
                Ok(result) => (
                    TeamConversationMessageRecord {
                        message_id: result.last_insert_rowid(),
                        conversation_id: conversation.id.clone(),
                        task_id: task_id.to_string(),
                        group_id: group_id.clone(),
                        from_actor_id: from_actor_id.to_string(),
                        to_actor_id: to_actor_id.clone(),
                        route: route.to_string(),
                        payload: redacted_payload.clone(),
                        created_at: now,
                    },
                    true,
                ),
                Err(err) if is_task_conversation_message_idempotency_unique_violation(&err) => {
                    let existing = fetch_task_conversation_message_by_idempotency(
                        &mut tx,
                        &conversation.id,
                        from_actor_id,
                        idempotency_key,
                    )
                    .await?;
                    ensure_task_conversation_message_idempotency_compatible(
                        task_id,
                        from_actor_id,
                        to_actor_id.as_deref(),
                        route,
                        &redacted_payload,
                        &existing,
                    )?;
                    (existing, false)
                }
                Err(err) => return Err(err.into()),
            };
            tx.commit().await?;
            outcome
        } else {
            let result = sqlx::query(
                r#"
                INSERT INTO team_conversation_messages (
                    conversation_id,
                    task_id,
                    from_actor_id,
                    to_actor_id,
                    route,
                    correlation_id,
                    group_id,
                    payload_json,
                    created_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
            )
            .bind(&conversation.id)
            .bind(task_id)
            .bind(from_actor_id)
            .bind(to_actor_id.as_deref())
            .bind(route)
            .bind(&correlation_id)
            .bind(group_id.as_deref())
            .bind(&payload_json)
            .bind(now)
            .execute(&self.db)
            .await?;
            (
                TeamConversationMessageRecord {
                    message_id: result.last_insert_rowid(),
                    conversation_id: conversation.id.clone(),
                    task_id: task_id.to_string(),
                    group_id: group_id.clone(),
                    from_actor_id: from_actor_id.to_string(),
                    to_actor_id: to_actor_id.clone(),
                    route: route.to_string(),
                    payload: redacted_payload.clone(),
                    created_at: now,
                },
                true,
            )
        };

        if created {
            self.spawn_archive_task_conversation_message(&conversation, &message);
            self.emit_conversation_event(TeamConversationStreamEvent {
                team_id: conversation.team_id.clone(),
                task_id: task_id.to_string(),
                conversation_id: conversation.id.clone(),
                message_id: Some(message.message_id),
                source: "conversation_message".to_string(),
            });
        }

        Ok((message, created))
    }

    fn spawn_archive_task_conversation_message(
        &self,
        conversation: &TeamConversationRecord,
        message: &TeamConversationMessageRecord,
    ) {
        let Some(archive) = self.message_archive.as_ref() else {
            return;
        };
        let archive = archive.clone();
        let document = team_conversation_message_archive_document(conversation, message);
        let conversation_id = message.conversation_id.clone();
        let message_id = message.message_id;

        tokio::spawn(async move {
            match tokio::time::timeout(
                MESSAGE_ARCHIVE_APPEND_TIMEOUT,
                archive.append_documents(&[document]),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::warn!(
                        error = ?error,
                        conversation_id = %conversation_id,
                        message_id,
                        "failed to dual-write team conversation message to archive"
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        conversation_id = %conversation_id,
                        message_id,
                        timeout_ms = MESSAGE_ARCHIVE_APPEND_TIMEOUT.as_millis(),
                        "timed out dual-writing team conversation message to archive"
                    );
                }
            }
        });
    }

    pub(super) fn spawn_archive_team_actor_message(&self, message: &TeamActorMessageRecord) {
        if self.message_archive.is_none() {
            return;
        }
        let manager = self.clone();
        let message = message.clone();
        let run_id = message.run_id.clone();
        let message_id = message.message_id;

        tokio::spawn(async move {
            match manager.team_actor_message_archive_document(&message).await {
                Ok(Some(document)) => {
                    let Some(archive) = manager.message_archive.as_ref().cloned() else {
                        return;
                    };
                    match tokio::time::timeout(
                        MESSAGE_ARCHIVE_APPEND_TIMEOUT,
                        archive.append_documents(&[document]),
                    )
                    .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => {
                            tracing::warn!(
                                error = ?error,
                                run_id = %run_id,
                                message_id,
                                "failed to dual-write team actor message to archive"
                            );
                        }
                        Err(_) => {
                            tracing::warn!(
                                run_id = %run_id,
                                message_id,
                                timeout_ms = MESSAGE_ARCHIVE_APPEND_TIMEOUT.as_millis(),
                                "timed out dual-writing team actor message to archive"
                            );
                        }
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(
                        error = ?error,
                        run_id = %run_id,
                        message_id,
                        "failed to build team actor message archive document"
                    );
                }
            }
        });
    }

    async fn team_actor_message_archive_document(
        &self,
        message: &TeamActorMessageRecord,
    ) -> anyhow::Result<Option<MessageDocument>> {
        let row = sqlx::query(
            r#"
            SELECT team_id, input_json
            FROM team_runs
            WHERE id = ?1
            "#,
        )
        .bind(&message.run_id)
        .fetch_one(&self.db)
        .await?;
        let team_id: String = row.get("team_id");
        let input_json: String = row.get("input_json");
        let run_input: Value = serde_json::from_str(&input_json)?;
        if message_archive_payload_string(&run_input, "bootstrap_kind").as_deref()
            == Some(TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND)
        {
            return Ok(None);
        }
        let base_scope = MessageArchiveScopeFallback::from_run_input(&run_input);
        let scope = message_archive_scope_for_payload_db(
            &self.db,
            &team_id,
            &message.payload,
            &base_scope,
            &mut HashMap::new(),
        )
        .await?;
        Ok(Some(team_actor_message_archive_document(
            &team_id, message, &scope,
        )))
    }

    pub(super) fn spawn_archive_team_run_event(&self, event: &TeamRunEventRecord) {
        self.spawn_archive_team_run_events(vec![event.clone()]);
    }

    #[cfg(test)]
    pub(super) async fn team_run_event_archive_document(
        &self,
        event: &TeamRunEventRecord,
    ) -> anyhow::Result<Option<MessageDocument>> {
        team_run_event_archive_document_for_db(&self.db, event).await
    }

    fn spawn_archive_team_run_events(&self, events: Vec<TeamRunEventRecord>) {
        let Some(archive) = self.message_archive.as_ref().cloned() else {
            return;
        };
        if events.is_empty() {
            return;
        }
        let manager = self.clone();

        tokio::spawn(async move {
            let mut documents = Vec::with_capacity(events.len());
            let mut run_scope_cache = HashMap::new();
            let mut task_conversation_cache = HashMap::new();
            for event in events {
                match team_run_event_archive_document_for_db_cached(
                    &manager.db,
                    &event,
                    &mut run_scope_cache,
                    &mut task_conversation_cache,
                )
                .await
                {
                    Ok(Some(document)) => documents.push(document),
                    Ok(None) => {}
                    Err(error) => {
                        tracing::warn!(
                            error = ?error,
                            run_id = %event.run_id,
                            event_id = event.event_id,
                            "failed to build team run event archive document"
                        );
                    }
                }
            }
            if documents.is_empty() {
                return;
            }

            let document_count = documents.len();
            match tokio::time::timeout(
                MESSAGE_ARCHIVE_APPEND_TIMEOUT,
                archive.append_documents(&documents),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::warn!(
                        error = ?error,
                        document_count,
                        "failed to dual-write team run events to archive"
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        document_count,
                        timeout_ms = MESSAGE_ARCHIVE_APPEND_TIMEOUT.as_millis(),
                        "timed out dual-writing team run events to archive"
                    );
                }
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn append_channel_replica_message(
        &self,
        authority_message_id: i64,
        correlation_id: &str,
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
                correlation_id,
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
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
        )
        .bind(authority_message_id)
        .bind(correlation_id)
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
                group_id,
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

    pub async fn search_message_archive(
        &self,
        query: &MessageSearchQuery,
    ) -> anyhow::Result<Vec<MessageSearchHit>> {
        let Some(archive) = self.message_archive.as_ref() else {
            return Ok(Vec::new());
        };
        archive.search(query).await
    }

    async fn message_archive_table_max_id(&self, table_name: &str) -> anyhow::Result<i64> {
        let sql = match table_name {
            "team_conversation_messages" => {
                "SELECT COALESCE(MAX(id), 0) FROM team_conversation_messages"
            }
            "team_run_events" => "SELECT COALESCE(MAX(id), 0) FROM team_run_events",
            "team_actor_messages" => "SELECT COALESCE(MAX(id), 0) FROM team_actor_messages",
            _ => anyhow::bail!("unsupported archive migration table: {table_name}"),
        };
        Ok(sqlx::query_scalar::<_, i64>(sql)
            .fetch_one(&self.db)
            .await?)
    }

    async fn migrate_agent_events_to_archive(
        &self,
        archive: &MessageArchiveStoreRef,
        batch_size: usize,
        snapshot: &AgentEventArchiveSnapshot,
    ) -> anyhow::Result<AgentEventArchiveMigrationCounts> {
        let mut counts = AgentEventArchiveMigrationCounts::default();
        counts.add(
            migrate_main_agent_events_to_archive(
                &self.db,
                archive,
                batch_size,
                snapshot.main_max_id,
            )
            .await
            .context("migrate main agent_events to message archive")?,
        );

        for (agent_id, max_id) in &snapshot.per_agent_max_ids {
            let pool = self.event_dbs.pool_for_agent(agent_id).await?;
            counts.add(
                migrate_per_agent_events_to_archive(
                    &self.db, &pool, archive, agent_id, batch_size, *max_id,
                )
                .await
                .with_context(|| {
                    format!("migrate per-agent agent_events for agent_id={agent_id}")
                })?,
            );
        }
        Ok(counts)
    }

    async fn message_archive_agent_event_snapshot(
        &self,
    ) -> anyhow::Result<AgentEventArchiveSnapshot> {
        let main_max_id =
            sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(id), 0) FROM agent_events")
                .fetch_one(&self.db)
                .await?;
        let agent_ids = sqlx::query_scalar::<_, String>("SELECT id FROM agents ORDER BY id ASC")
            .fetch_all(&self.db)
            .await?;
        let mut per_agent_max_ids = Vec::new();
        for agent_id in agent_ids {
            let db_path = self.event_dbs.db_path_for_agent(&agent_id);
            if !tokio::fs::try_exists(&db_path).await? {
                continue;
            }
            let pool = self.event_dbs.pool_for_agent(&agent_id).await?;
            let max_id =
                sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(id), 0) FROM agent_events")
                    .fetch_one(&pool)
                    .await?;
            per_agent_max_ids.push((agent_id, max_id));
        }
        Ok(AgentEventArchiveSnapshot {
            main_max_id,
            per_agent_max_ids,
        })
    }

    pub async fn migrate_team_messages_to_archive(
        &self,
        batch_size: usize,
    ) -> anyhow::Result<TeamMessageArchiveMigrationReport> {
        let Some(archive) = self.message_archive.as_ref() else {
            anyhow::bail!("message archive is not configured");
        };
        let mut report = TeamMessageArchiveMigrationReport::default();
        let batch_size = batch_size.clamp(1, 1000);
        let batch_limit = i64::try_from(batch_size)?;
        let max_conversation_message_id = self
            .message_archive_table_max_id("team_conversation_messages")
            .await?;
        let max_run_event_id = self.message_archive_table_max_id("team_run_events").await?;
        let max_actor_message_id = self
            .message_archive_table_max_id("team_actor_messages")
            .await?;
        let agent_event_snapshot = self.message_archive_agent_event_snapshot().await?;
        let mut run_scope_cache: HashMap<String, MessageArchiveScopeFallback> = HashMap::new();
        let mut task_conversation_cache: HashMap<(String, String), Option<String>> = HashMap::new();

        let mut last_id = 0_i64;
        loop {
            let rows = sqlx::query(
                r#"
                SELECT
                    m.id,
                    m.conversation_id,
                    m.task_id,
                    m.group_id,
                    m.from_actor_id,
                    m.to_actor_id,
                    m.route,
                    m.payload_json,
                    m.created_at,
                    c.team_id
                FROM team_conversation_messages m
                INNER JOIN team_conversations c ON c.id = m.conversation_id
                WHERE m.id > ?1 AND m.id <= ?2
                ORDER BY m.id ASC
                LIMIT ?3
                "#,
            )
            .bind(last_id)
            .bind(max_conversation_message_id)
            .bind(batch_limit)
            .fetch_all(&self.db)
            .await?;
            if rows.is_empty() {
                break;
            }
            let mut documents = Vec::with_capacity(rows.len());
            for row in rows {
                last_id = row.get("id");
                let conversation = TeamConversationRecord {
                    id: row.get("conversation_id"),
                    team_id: row.get("team_id"),
                    task_id: row.get("task_id"),
                    mode: String::new(),
                    topic: None,
                    created_at: 0,
                    updated_at: 0,
                };
                let message = parse_team_conversation_message_row(&row)?;
                documents.push(team_conversation_message_archive_document(
                    &conversation,
                    &message,
                ));
                report.team_conversation_messages += 1;
            }
            archive.append_documents(&documents).await?;
        }

        let mut last_id = 0_i64;
        loop {
            let rows = sqlx::query(
                r#"
                SELECT
                    e.id,
                    e.run_id,
                    e.step_id,
                    e.event_type,
                    e.ts,
                    e.payload_json,
                    r.team_id,
                    r.input_json
                FROM team_run_events e
                INNER JOIN team_runs r ON r.id = e.run_id
                WHERE e.id > ?1
                  AND e.id <= ?2
                  AND trim(COALESCE(json_extract(r.input_json, '$.bootstrap_kind'), '')) != ?3
                ORDER BY e.id ASC
                LIMIT ?4
                "#,
            )
            .bind(last_id)
            .bind(max_run_event_id)
            .bind(TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND)
            .bind(batch_limit)
            .fetch_all(&self.db)
            .await?;
            if rows.is_empty() {
                break;
            }
            let mut documents = Vec::with_capacity(rows.len());
            for row in rows {
                last_id = row.get("id");
                let run_id: String = row.get("run_id");
                let team_id: String = row.get("team_id");
                if !run_scope_cache.contains_key(&run_id) {
                    let input_json: String = row.get("input_json");
                    let run_input: Value = serde_json::from_str(&input_json)?;
                    run_scope_cache.insert(
                        run_id.clone(),
                        MessageArchiveScopeFallback::from_run_input(&run_input),
                    );
                }
                let base_scope = run_scope_cache
                    .get(&run_id)
                    .expect("run scope is cached before use")
                    .clone();
                let event = parse_run_event_row(&row)?;
                let scope = message_archive_scope_for_payload_db(
                    &self.db,
                    &team_id,
                    &event.payload,
                    &base_scope,
                    &mut task_conversation_cache,
                )
                .await?;
                documents.push(team_run_event_archive_document(&team_id, &event, &scope));
                report.team_run_events += 1;
            }
            archive.append_documents(&documents).await?;
        }

        let mut last_id = 0_i64;
        loop {
            let rows = sqlx::query(
                r#"
                SELECT
                    m.id,
                    m.run_id,
                    m.from_actor_id,
                    m.from_peer_id,
                    m.to_actor_id,
                    m.to_peer_id,
                    m.channel,
                    m.transport,
                    m.route_json,
                    m.payload_json,
                    m.status,
                    m.created_at,
                    m.delivered_at,
                    r.team_id,
                    r.input_json
                FROM team_actor_messages m
                INNER JOIN team_runs r ON r.id = m.run_id
                WHERE m.id > ?1
                  AND m.id <= ?2
                  AND trim(COALESCE(json_extract(r.input_json, '$.bootstrap_kind'), '')) != ?3
                ORDER BY m.id ASC
                LIMIT ?4
                "#,
            )
            .bind(last_id)
            .bind(max_actor_message_id)
            .bind(TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND)
            .bind(batch_limit)
            .fetch_all(&self.db)
            .await?;
            if rows.is_empty() {
                break;
            }
            let mut documents = Vec::with_capacity(rows.len());
            for row in rows {
                last_id = row.get("id");
                let run_id: String = row.get("run_id");
                let team_id: String = row.get("team_id");
                if !run_scope_cache.contains_key(&run_id) {
                    let input_json: String = row.get("input_json");
                    let run_input: Value = serde_json::from_str(&input_json)?;
                    run_scope_cache.insert(
                        run_id.clone(),
                        MessageArchiveScopeFallback::from_run_input(&run_input),
                    );
                }
                let base_scope = run_scope_cache
                    .get(&run_id)
                    .expect("run scope is cached before use")
                    .clone();
                let message = parse_team_actor_message_row(&row)?;
                let scope = message_archive_scope_for_payload_db(
                    &self.db,
                    &team_id,
                    &message.payload,
                    &base_scope,
                    &mut task_conversation_cache,
                )
                .await?;
                documents.push(team_actor_message_archive_document(
                    &team_id, &message, &scope,
                ));
                report.team_actor_messages += 1;
            }
            archive.append_documents(&documents).await?;
        }

        let agent_event_counts = self
            .migrate_agent_events_to_archive(archive, batch_size, &agent_event_snapshot)
            .await?;
        report.agent_events = agent_event_counts.agent_events;
        report.aggregated_acp_messages = agent_event_counts.aggregated_acp_messages;
        Ok(report)
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
        let now = Utc::now().timestamp();
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
        let submitted_event = Self::append_run_event_tx(
            &mut tx,
            run_id,
            Some(&step_id),
            "step_submitted",
            now,
            &payload,
        )
        .await?;
        tx.commit().await?;
        self.spawn_archive_team_run_event(&submitted_event);

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
            if let Some(explicit_team_id) = normalized_team_id.as_deref()
                && explicit_team_id != roster.team_id
            {
                return Err(TeamContextLookupError::RunTeamMismatch {
                    run_id: run_id.to_string(),
                    actual_team_id: roster.team_id.clone(),
                    requested_team_id: explicit_team_id.to_string(),
                }
                .into());
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

        let team_id = normalized_team_id.ok_or(TeamContextLookupError::MissingSelector)?;
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
    ) -> anyhow::Result<TeamRunEventRecord> {
        let result = sqlx::query(
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
        Ok(TeamRunEventRecord {
            event_id: result.last_insert_rowid(),
            run_id: run_id.to_string(),
            step_id: step_id.map(str::to_string),
            event_type: event_type.to_string(),
            ts,
            payload: payload.clone(),
        })
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

    async fn persist_reconcile_round_artifact_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        step: &TeamStepRecord,
        snapshot: ReconcileRoundArtifactSnapshot<'_>,
        now: i64,
    ) -> anyhow::Result<Option<ContextArtifactPointer>> {
        let team_id: String = sqlx::query_scalar(
            r#"
            SELECT team_id
            FROM team_runs
            WHERE id = ?1
            "#,
        )
        .bind(&step.run_id)
        .fetch_one(&mut **tx)
        .await?;
        let team_id_for_payload = team_id.clone();

        let artifact_payload = serde_json::json!({
            "schema_version": 1,
            "team_id": team_id_for_payload,
            "run_id": step.run_id,
            "step_id": step.id,
            "step_key": step.step_key,
            "member_id": step.member_id,
            "session_id": step.runtime_handle_id,
            "round": snapshot.round,
            "status": snapshot.status,
            "summary": snapshot.summary,
            "output": snapshot.output,
            "input": snapshot.input,
            "reason": snapshot.reason,
            "error_text": snapshot.error_text,
            "created_at": now,
        });
        self.persist_context_artifact_tx(
            tx,
            ContextArtifactOwner {
                team_id: &team_id,
                run_id: &step.run_id,
                member_id: &step.member_id,
                session_id: step.runtime_handle_id.as_deref(),
            },
            RECONCILE_ROUND_ARTIFACT_KIND,
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
        let Some(workspace) =
            load_team_member_context_workspace_tx(tx, owner.team_id, owner.member_id).await?
        else {
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

        let run_context_dir = PathBuf::from(&workspace.runtime_workdir)
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
        let content_checksum = hex_encode(&Sha256::digest(&artifact_bytes));
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

    async fn prepare_runtime_state_snapshot_write_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        owner: ContextArtifactOwner<'_>,
        continuity_mode: &str,
        continuity_state: &TeamMemberContinuityStateRecord,
    ) -> anyhow::Result<Option<RuntimeStateSnapshotWritePlan>> {
        let Some(workspace) =
            load_team_member_context_workspace_tx(tx, owner.team_id, owner.member_id).await?
        else {
            return Ok(None);
        };

        let workspace_root = PathBuf::from(&workspace.runtime_workdir);
        let state_path = workspace_root
            .join(".cache")
            .join("context")
            .join("state.md");
        let continuity_note = continuity_note_relative_path(&continuity_state.source_run_id).map(
            |relative_note_path| {
                let note_path = workspace_root.join(relative_note_path);
                let note_text =
                    build_runtime_continuity_note_text(owner, continuity_mode, continuity_state);
                (note_path, note_text)
            },
        );
        let state_text =
            build_runtime_state_snapshot_text(owner, continuity_mode, continuity_state);
        Ok(Some(RuntimeStateSnapshotWritePlan {
            team_id: owner.team_id.to_string(),
            run_id: owner.run_id.to_string(),
            member_id: owner.member_id.to_string(),
            state_path,
            state_text,
            continuity_note,
        }))
    }

    async fn write_runtime_state_snapshot_best_effort(
        plan: RuntimeStateSnapshotWritePlan,
    ) -> anyhow::Result<()> {
        if let Some((note_path, note_text)) = plan.continuity_note.as_ref() {
            if let Some(parent) = note_path.parent()
                && let Err(err) = tokio::fs::create_dir_all(parent).await
            {
                tracing::warn!(
                    team_id = plan.team_id,
                    run_id = plan.run_id,
                    member_id = plan.member_id,
                    path = %note_path.display(),
                    "team manager failed to create runtime continuity note dir: {}",
                    err
                );
            } else if let Err(err) = tokio::fs::write(note_path, note_text).await {
                tracing::warn!(
                    team_id = plan.team_id,
                    run_id = plan.run_id,
                    member_id = plan.member_id,
                    path = %note_path.display(),
                    "team manager failed to write runtime continuity note: {}",
                    err
                );
            }
        }
        if let Some(parent) = plan.state_path.parent()
            && let Err(err) = tokio::fs::create_dir_all(parent).await
        {
            tracing::warn!(
                team_id = plan.team_id,
                run_id = plan.run_id,
                member_id = plan.member_id,
                path = %plan.state_path.display(),
                "team manager failed to create runtime state snapshot dir: {}",
                err
            );
            return Ok(());
        }
        if let Err(err) = tokio::fs::write(&plan.state_path, plan.state_text).await {
            tracing::warn!(
                team_id = plan.team_id,
                run_id = plan.run_id,
                member_id = plan.member_id,
                path = %plan.state_path.display(),
                "team manager failed to write runtime state snapshot: {}",
                err
            );
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn start_step(
        &self,
        step_id: &str,
        runtime_handle_id: Option<&str>,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let current = load_step_record_tx(&mut tx, step_id).await?;
        let reconcile_started = build_reconcile_round_started_input(current.input.as_ref()).map(
            |(next_input, round)| {
                (
                    serde_json::to_string(&next_input)
                        .expect("reconcile round input should serialize"),
                    round,
                )
            },
        );
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                status = 'working',
                remote_task_id = COALESCE(?1, remote_task_id),
                input_json = COALESCE(?2, input_json),
                started_at = COALESCE(started_at, ?3)
            WHERE id = ?4 AND status IN ('submitted', 'input_required')
            "#,
        )
        .bind(runtime_handle_id)
        .bind(
            reconcile_started
                .as_ref()
                .map(|(input_json, _)| input_json.as_str()),
        )
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;
        let step = load_step_record_tx(&mut tx, step_id).await?;
        let mut archive_events = Vec::new();

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
                let event = Self::append_run_event_tx(
                    &mut tx,
                    &step.run_id,
                    None,
                    "run_working",
                    now,
                    &run_payload,
                )
                .await?;
                archive_events.push(event);
            }

            let step_payload = build_step_runtime_handle_event_payload(&step, "working");
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "step_working",
                now,
                &step_payload,
            )
            .await?;
            archive_events.push(event);

            if let Some((_, round)) = reconcile_started.as_ref() {
                let runtime = extract_reconcile_round_runtime(step.input.as_ref());
                let round_payload = serde_json::json!({
                    "step_id": step.id,
                    "step_key": step.step_key,
                    "round": round,
                    "status": "working",
                    "goal": runtime.as_ref().and_then(|item| item.goal.clone()),
                    "acceptance_count": runtime.as_ref().map(|item| item.acceptance.len()).unwrap_or(0),
                    "max_rounds": runtime.and_then(|item| item.execution.max_rounds),
                });
                let event = Self::append_run_event_tx(
                    &mut tx,
                    &step.run_id,
                    Some(&step.id),
                    "step_reconcile_round_started",
                    now,
                    &round_payload,
                )
                .await?;
                archive_events.push(event);
            }
        }

        tx.commit().await?;
        self.spawn_archive_team_run_events(archive_events);
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
        let mut tx = self.db.begin().await?;
        let current = load_step_record_tx(&mut tx, step_id).await?;
        let summary = reason
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| summarize_reconcile_output(input.as_ref()));
        let merged_input = merge_step_input(current.input.as_ref(), input);
        let reconcile_finished = build_reconcile_round_finished_input(
            merged_input.as_ref().or(current.input.as_ref()),
            "input_required",
            summary.as_deref(),
        );
        let input_json = reconcile_finished
            .as_ref()
            .map(|(value, _)| serde_json::to_string(value))
            .transpose()?
            .or_else(|| {
                merged_input
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .ok()
                    .flatten()
            });
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
        let step = load_step_record_tx(&mut tx, step_id).await?;
        let mut archive_events = Vec::new();

        if update.rows_affected() > 0 {
            let mut round_artifact_pointer = None;
            let mut round_artifact_offload_reason: Option<&str> = None;
            if let Some((_, round)) = reconcile_finished.as_ref() {
                match self
                    .persist_reconcile_round_artifact_tx(
                        &mut tx,
                        &step,
                        ReconcileRoundArtifactSnapshot {
                            round: *round,
                            status: "input_required",
                            summary: summary.as_deref(),
                            output: None,
                            input: step.input.as_ref(),
                            reason: step.error_text.as_deref(),
                            error_text: None,
                        },
                        now,
                    )
                    .await
                {
                    Ok(Some(pointer)) => round_artifact_pointer = Some(pointer),
                    Ok(None) => round_artifact_offload_reason = Some("agent_workdir_missing"),
                    Err(err) => {
                        tracing::warn!(
                            run_id = %step.run_id,
                            step_id = %step.id,
                            member_id = %step.member_id,
                            "team manager failed to persist reconcile round artifact: {}",
                            err
                        );
                        round_artifact_offload_reason = Some("artifact_write_failed");
                    }
                }
            }

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
                let event = Self::append_run_event_tx(
                    &mut tx,
                    &step.run_id,
                    None,
                    "run_input_required",
                    now,
                    &run_payload,
                )
                .await?;
                archive_events.push(event);
                let (team_id, run_input) =
                    load_run_status_sync_meta_tx(&mut tx, &step.run_id).await?;
                sync_linked_task_status_tx(
                    &mut tx,
                    &team_id,
                    &run_input,
                    TeamTaskStatus::Waiting,
                    now,
                    true,
                )
                .await?;
            }

            let step_payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "status": "input_required",
                "reason": step.error_text,
                "input": step.input,
            });
            let mut step_payload = step_payload;
            maybe_attach_context_artifact_pointer(
                &mut step_payload,
                round_artifact_pointer.as_ref(),
                round_artifact_offload_reason,
            );
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "step_input_required",
                now,
                &step_payload,
            )
            .await?;
            archive_events.push(event);

            if let Some((_, round)) = reconcile_finished.as_ref() {
                let round_payload = serde_json::json!({
                    "step_id": step.id,
                    "step_key": step.step_key,
                    "round": round,
                    "status": "input_required",
                    "summary": summary,
                });
                let mut round_payload = round_payload;
                maybe_attach_context_artifact_pointer(
                    &mut round_payload,
                    round_artifact_pointer.as_ref(),
                    round_artifact_offload_reason,
                );
                let event = Self::append_run_event_tx(
                    &mut tx,
                    &step.run_id,
                    Some(&step.id),
                    "step_reconcile_round_finished",
                    now,
                    &round_payload,
                )
                .await?;
                archive_events.push(event);
            }
        }

        tx.commit().await?;
        self.spawn_archive_team_run_events(archive_events);
        Ok(step)
    }

    #[allow(dead_code)]
    pub async fn resume_step(
        &self,
        step_id: &str,
        input: Option<Value>,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let current = load_step_record_tx(&mut tx, step_id).await?;
        let merged_input = merge_step_input(current.input.as_ref(), input);
        let started_input =
            build_reconcile_round_started_input(merged_input.as_ref().or(current.input.as_ref()));
        let input_json = started_input
            .as_ref()
            .map(|(value, _)| serde_json::to_string(value))
            .transpose()?
            .or_else(|| {
                merged_input
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .ok()
                    .flatten()
            });
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
        let step = load_step_record_tx(&mut tx, step_id).await?;
        let mut archive_events = Vec::new();

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
                let event = Self::append_run_event_tx(
                    &mut tx,
                    &step.run_id,
                    None,
                    "run_working",
                    now,
                    &run_payload,
                )
                .await?;
                archive_events.push(event);
                let (team_id, run_input) =
                    load_run_status_sync_meta_tx(&mut tx, &step.run_id).await?;
                sync_linked_task_status_tx(
                    &mut tx,
                    &team_id,
                    &run_input,
                    TeamTaskStatus::InProgress,
                    now,
                    false,
                )
                .await?;
            }

            let step_payload = build_step_runtime_handle_event_payload(&step, "working");
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "step_resumed",
                now,
                &step_payload,
            )
            .await?;
            archive_events.push(event);

            if let Some((_, round)) = started_input.as_ref() {
                let runtime = extract_reconcile_round_runtime(step.input.as_ref());
                let round_payload = serde_json::json!({
                    "step_id": step.id,
                    "step_key": step.step_key,
                    "round": round,
                    "status": "working",
                    "goal": runtime.as_ref().and_then(|item| item.goal.clone()),
                    "acceptance_count": runtime.as_ref().map(|item| item.acceptance.len()).unwrap_or(0),
                    "max_rounds": runtime.and_then(|item| item.execution.max_rounds),
                });
                let event = Self::append_run_event_tx(
                    &mut tx,
                    &step.run_id,
                    Some(&step.id),
                    "step_reconcile_round_started",
                    now,
                    &round_payload,
                )
                .await?;
                archive_events.push(event);
            }
        }

        tx.commit().await?;
        self.spawn_archive_team_run_events(archive_events);
        Ok(step)
    }

    #[allow(dead_code)]
    pub async fn continue_step(
        &self,
        step_id: &str,
        output: Option<Value>,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let current = load_step_record_tx(&mut tx, step_id).await?;
        if current.status != TeamStepStatus::Working {
            anyhow::bail!("continue_step requires a working reconcile step");
        }
        let runtime = extract_reconcile_round_runtime(current.input.as_ref())
            .ok_or_else(|| anyhow::anyhow!("continue_step requires reconcile_loop step input"))?;
        let current_round = runtime.current_round.max(1);
        if let Some(max_rounds) = runtime.execution.max_rounds
            && current_round >= max_rounds
        {
            anyhow::bail!(
                "reconcile_loop step reached max_rounds={max_rounds}; use complete, input_required, or fail instead"
            );
        }

        let summary = summarize_reconcile_output(output.as_ref());
        let finished_input = build_reconcile_round_finished_input(
            current.input.as_ref(),
            "continued",
            summary.as_deref(),
        )
        .ok_or_else(|| anyhow::anyhow!("continue_step requires reconcile_loop round state"))?;
        let started_input = build_reconcile_round_started_input(Some(&finished_input.0))
            .ok_or_else(|| anyhow::anyhow!("continue_step failed to start next reconcile round"))?;
        let output_json = output.as_ref().map(serde_json::to_string).transpose()?;
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                input_json = ?1,
                output_json = COALESCE(?2, output_json),
                error_text = NULL,
                started_at = COALESCE(started_at, ?3)
            WHERE id = ?4 AND status = 'working'
            "#,
        )
        .bind(serde_json::to_string(&started_input.0)?)
        .bind(output_json)
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;
        let step = load_step_record_tx(&mut tx, step_id).await?;
        let mut archive_events = Vec::new();

        if update.rows_affected() > 0 {
            let mut round_artifact_pointer = None;
            let mut round_artifact_offload_reason: Option<&str> = None;
            match self
                .persist_reconcile_round_artifact_tx(
                    &mut tx,
                    &step,
                    ReconcileRoundArtifactSnapshot {
                        round: finished_input.1,
                        status: "continued",
                        summary: summary.as_deref(),
                        output: output.as_ref(),
                        input: Some(&finished_input.0),
                        reason: None,
                        error_text: None,
                    },
                    now,
                )
                .await
            {
                Ok(Some(pointer)) => round_artifact_pointer = Some(pointer),
                Ok(None) => round_artifact_offload_reason = Some("agent_workdir_missing"),
                Err(err) => {
                    tracing::warn!(
                        run_id = %step.run_id,
                        step_id = %step.id,
                        member_id = %step.member_id,
                        "team manager failed to persist reconcile round artifact: {}",
                        err
                    );
                    round_artifact_offload_reason = Some("artifact_write_failed");
                }
            }

            let continue_payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "status": "working",
                "continued_from_round": finished_input.1,
                "continued_to_round": started_input.1,
                "summary": summary,
            });
            let mut continue_payload = continue_payload;
            maybe_attach_context_artifact_pointer(
                &mut continue_payload,
                round_artifact_pointer.as_ref(),
                round_artifact_offload_reason,
            );
            if round_artifact_pointer.is_none()
                && let (Some(output), Some(payload_obj)) =
                    (output.as_ref(), continue_payload.as_object_mut())
            {
                payload_obj.insert("output".to_string(), output.clone());
                payload_obj.insert(
                    "output_inlined_because".to_string(),
                    serde_json::json!(
                        round_artifact_offload_reason.unwrap_or("artifact_pointer_missing")
                    ),
                );
            }
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "step_continued",
                now,
                &continue_payload,
            )
            .await?;
            archive_events.push(event);

            let round_finished_payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "round": finished_input.1,
                "status": "continued",
                "summary": summary,
            });
            let mut round_finished_payload = round_finished_payload;
            maybe_attach_context_artifact_pointer(
                &mut round_finished_payload,
                round_artifact_pointer.as_ref(),
                round_artifact_offload_reason,
            );
            if round_artifact_pointer.is_none()
                && let (Some(output), Some(payload_obj)) =
                    (output.as_ref(), round_finished_payload.as_object_mut())
            {
                payload_obj.insert("output".to_string(), output.clone());
                payload_obj.insert(
                    "output_inlined_because".to_string(),
                    serde_json::json!(
                        round_artifact_offload_reason.unwrap_or("artifact_pointer_missing")
                    ),
                );
            }
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "step_reconcile_round_finished",
                now,
                &round_finished_payload,
            )
            .await?;
            archive_events.push(event);

            let round_started_payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "round": started_input.1,
                "status": "working",
                "goal": runtime.goal,
                "acceptance_count": runtime.acceptance.len(),
                "max_rounds": runtime.execution.max_rounds,
            });
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "step_reconcile_round_started",
                now,
                &round_started_payload,
            )
            .await?;
            archive_events.push(event);
        }

        tx.commit().await?;
        self.spawn_archive_team_run_events(archive_events);
        Ok(step)
    }

    #[allow(dead_code)]
    pub async fn complete_step(
        &self,
        step_id: &str,
        output: Option<Value>,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let current = load_step_record_tx(&mut tx, step_id).await?;
        let summary = summarize_reconcile_output(output.as_ref());
        let reconcile_finished = build_reconcile_round_finished_input(
            current.input.as_ref(),
            "completed",
            summary.as_deref(),
        );
        let output_json = output.as_ref().map(serde_json::to_string).transpose()?;
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                status = 'completed',
                input_json = COALESCE(?1, input_json),
                output_json = ?2,
                ended_at = COALESCE(ended_at, ?3)
            WHERE id = ?4 AND status IN ('working', 'input_required')
            "#,
        )
        .bind(
            reconcile_finished
                .as_ref()
                .map(|(value, _)| serde_json::to_string(value))
                .transpose()?,
        )
        .bind(output_json)
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;
        let step = load_step_record_tx(&mut tx, step_id).await?;
        let mut runtime_state_snapshot: Option<RuntimeStateSnapshotWritePlan> = None;
        let mut archive_events = Vec::new();

        if update.rows_affected() > 0 {
            let mut round_artifact_pointer = None;
            let mut round_artifact_offload_reason: Option<&str> = None;
            if let Some((_, round)) = reconcile_finished.as_ref() {
                match self
                    .persist_reconcile_round_artifact_tx(
                        &mut tx,
                        &step,
                        ReconcileRoundArtifactSnapshot {
                            round: *round,
                            status: "completed",
                            summary: summary.as_deref(),
                            output: step.output.as_ref(),
                            input: step.input.as_ref(),
                            reason: None,
                            error_text: None,
                        },
                        now,
                    )
                    .await
                {
                    Ok(Some(pointer)) => round_artifact_pointer = Some(pointer),
                    Ok(None) => round_artifact_offload_reason = Some("agent_workdir_missing"),
                    Err(err) => {
                        tracing::warn!(
                            run_id = %step.run_id,
                            step_id = %step.id,
                            member_id = %step.member_id,
                            "team manager failed to persist reconcile round artifact: {}",
                            err
                        );
                        round_artifact_offload_reason = Some("artifact_write_failed");
                    }
                }
            }

            let payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "status": "completed",
            });
            let mut payload = payload;
            maybe_attach_context_artifact_pointer(
                &mut payload,
                round_artifact_pointer.as_ref(),
                round_artifact_offload_reason,
            );
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "step_completed",
                now,
                &payload,
            )
            .await?;
            archive_events.push(event);

            let (team_id, run_input) = load_run_status_sync_meta_tx(&mut tx, &step.run_id).await?;
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
            runtime_state_snapshot = self
                .prepare_runtime_state_snapshot_write_tx(
                    &mut tx,
                    ContextArtifactOwner {
                        team_id: &team_id,
                        run_id: &step.run_id,
                        member_id: &step.member_id,
                        session_id: step.runtime_handle_id.as_deref(),
                    },
                    &continuity_mode,
                    &continuity_state,
                )
                .await?;

            let mut continuity_payload = build_continuity_event_payload(
                &continuity_state,
                &step,
                &continuity_mode,
                artifact_offload_status,
            );
            if let Some(payload_obj) = continuity_payload.as_object_mut() {
                if let Some(pointer_payload) = artifact_pointer_for_event.as_ref() {
                    payload_obj.insert("artifact_pointer".to_string(), pointer_payload.clone());
                }
                if let Some(reason) = artifact_offload_reason {
                    payload_obj.insert(
                        "artifact_offload_reason".to_string(),
                        Value::String(reason.to_string()),
                    );
                }
            }
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "continuity_state_updated",
                now,
                &continuity_payload,
            )
            .await?;
            archive_events.push(event);

            if let Some((_, round)) = reconcile_finished.as_ref() {
                let mut round_payload = serde_json::json!({
                    "step_id": step.id,
                    "step_key": step.step_key,
                    "round": round,
                    "status": "completed",
                    "summary": summary,
                });
                maybe_attach_context_artifact_pointer(
                    &mut round_payload,
                    round_artifact_pointer.as_ref(),
                    round_artifact_offload_reason,
                );
                let event = Self::append_run_event_tx(
                    &mut tx,
                    &step.run_id,
                    Some(&step.id),
                    "step_reconcile_round_finished",
                    now,
                    &round_payload,
                )
                .await?;
                archive_events.push(event);
            }

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
                    let event = Self::append_run_event_tx(
                        &mut tx,
                        &step.run_id,
                        None,
                        "run_completed",
                        now,
                        &run_payload,
                    )
                    .await?;
                    archive_events.push(event);
                    sync_linked_task_status_tx(
                        &mut tx,
                        &team_id,
                        &run_input,
                        TeamTaskStatus::InReview,
                        now,
                        true,
                    )
                    .await?;
                }
            }
        }

        tx.commit().await?;
        self.spawn_archive_team_run_events(archive_events);
        if let Some(plan) = runtime_state_snapshot {
            Self::write_runtime_state_snapshot_best_effort(plan).await?;
        }
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
        let current = load_step_record_tx(&mut tx, step_id).await?;
        let reconcile_finished = build_reconcile_round_finished_input(
            current.input.as_ref(),
            "failed",
            Some(error_text),
        );
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                status = 'failed',
                input_json = COALESCE(?1, input_json),
                error_text = ?2,
                ended_at = COALESCE(ended_at, ?3)
            WHERE id = ?4 AND status IN ('submitted', 'working', 'input_required')
            "#,
        )
        .bind(
            reconcile_finished
                .as_ref()
                .map(|(value, _)| serde_json::to_string(value))
                .transpose()?,
        )
        .bind(error_text)
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;
        let step = load_step_record_tx(&mut tx, step_id).await?;
        let mut archive_events = Vec::new();

        if update.rows_affected() > 0 {
            let mut round_artifact_pointer = None;
            let mut round_artifact_offload_reason: Option<&str> = None;
            if let Some((_, round)) = reconcile_finished.as_ref() {
                match self
                    .persist_reconcile_round_artifact_tx(
                        &mut tx,
                        &step,
                        ReconcileRoundArtifactSnapshot {
                            round: *round,
                            status: "failed",
                            summary: step.error_text.as_deref(),
                            output: None,
                            input: step.input.as_ref(),
                            reason: None,
                            error_text: step.error_text.as_deref(),
                        },
                        now,
                    )
                    .await
                {
                    Ok(Some(pointer)) => round_artifact_pointer = Some(pointer),
                    Ok(None) => round_artifact_offload_reason = Some("agent_workdir_missing"),
                    Err(err) => {
                        tracing::warn!(
                            run_id = %step.run_id,
                            step_id = %step.id,
                            member_id = %step.member_id,
                            "team manager failed to persist reconcile round artifact: {}",
                            err
                        );
                        round_artifact_offload_reason = Some("artifact_write_failed");
                    }
                }
            }

            let payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "status": "failed",
                "error_text": step.error_text,
            });
            let mut payload = payload;
            maybe_attach_context_artifact_pointer(
                &mut payload,
                round_artifact_pointer.as_ref(),
                round_artifact_offload_reason,
            );
            let event = Self::append_run_event_tx(
                &mut tx,
                &step.run_id,
                Some(&step.id),
                "step_failed",
                now,
                &payload,
            )
            .await?;
            archive_events.push(event);

            if let Some((_, round)) = reconcile_finished.as_ref() {
                let round_payload = serde_json::json!({
                    "step_id": step.id,
                    "step_key": step.step_key,
                    "round": round,
                    "status": "failed",
                    "summary": step.error_text,
                });
                let mut round_payload = round_payload;
                maybe_attach_context_artifact_pointer(
                    &mut round_payload,
                    round_artifact_pointer.as_ref(),
                    round_artifact_offload_reason,
                );
                let event = Self::append_run_event_tx(
                    &mut tx,
                    &step.run_id,
                    Some(&step.id),
                    "step_reconcile_round_finished",
                    now,
                    &round_payload,
                )
                .await?;
                archive_events.push(event);
            }

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
                let event = Self::append_run_event_tx(
                    &mut tx,
                    &step.run_id,
                    None,
                    "run_failed",
                    now,
                    &run_payload,
                )
                .await?;
                archive_events.push(event);
            }
        }

        tx.commit().await?;
        self.spawn_archive_team_run_events(archive_events);
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
        let team_id: String = run_row.get("team_id");
        let run_status: String = run_row.get("status");
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

        let mut archive_events = Vec::new();
        let started_event = Self::append_run_event_tx(
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
        archive_events.push(started_event);

        let Some(session_id) = session_id else {
            let context = MemoryFlushFinalizeContext {
                run_id,
                team_id: team_id.as_str(),
                member_id: normalized.member_id.as_str(),
                session_id: None,
                trigger: normalized.trigger.as_str(),
                now,
            };
            let (result, event) = finalize_memory_flush_failed_tx(
                &mut tx,
                &context,
                "session_mapping_missing",
                None,
                0,
                None,
            )
            .await?;
            tx.commit().await?;
            archive_events.push(event);
            self.spawn_archive_team_run_events(archive_events);
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
            let (result, event) = finalize_memory_flush_noop_tx(&mut tx, &context).await?;
            tx.commit().await?;
            archive_events.push(event);
            self.spawn_archive_team_run_events(archive_events);
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
                let (result, event) = finalize_memory_flush_failed_tx(
                    &mut tx,
                    &context,
                    "agent_workdir_missing",
                    Some((event_id_from, event_id_to)),
                    flushed_events,
                    None,
                )
                .await?;
                tx.commit().await?;
                archive_events.push(event);
                self.spawn_archive_team_run_events(archive_events);
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
                let (result, event) = finalize_memory_flush_failed_tx(
                    &mut tx,
                    &context,
                    "artifact_write_failed",
                    Some((event_id_from, event_id_to)),
                    flushed_events,
                    Some(truncate_chars(err.to_string().as_str(), 400)),
                )
                .await?;
                tx.commit().await?;
                archive_events.push(event);
                self.spawn_archive_team_run_events(archive_events);
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
        let persisted_event = Self::append_run_event_tx(
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
        archive_events.push(persisted_event);

        tx.commit().await?;
        self.spawn_archive_team_run_events(archive_events);
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

async fn migrate_main_agent_events_to_archive(
    db: &SqlitePool,
    archive: &MessageArchiveStoreRef,
    batch_size: usize,
    max_id: i64,
) -> anyhow::Result<AgentEventArchiveMigrationCounts> {
    migrate_agent_event_rows_to_archive(
        db,
        db,
        archive,
        batch_size,
        max_id,
        AgentEventArchiveSource::Main,
    )
    .await
}

async fn migrate_per_agent_events_to_archive(
    main_db: &SqlitePool,
    event_db: &SqlitePool,
    archive: &MessageArchiveStoreRef,
    agent_id: &str,
    batch_size: usize,
    max_id: i64,
) -> anyhow::Result<AgentEventArchiveMigrationCounts> {
    migrate_agent_event_rows_to_archive(
        main_db,
        event_db,
        archive,
        batch_size,
        max_id,
        AgentEventArchiveSource::PerAgent { agent_id },
    )
    .await
}

#[derive(Debug, Clone, Copy)]
enum AgentEventArchiveSource<'a> {
    Main,
    PerAgent { agent_id: &'a str },
}

async fn migrate_agent_event_rows_to_archive(
    main_db: &SqlitePool,
    event_db: &SqlitePool,
    archive: &MessageArchiveStoreRef,
    batch_size: usize,
    max_id: i64,
    source: AgentEventArchiveSource<'_>,
) -> anyhow::Result<AgentEventArchiveMigrationCounts> {
    let mut last_id = 0_i64;
    let mut chunk_accumulator = AcpChunkAccumulator::default();
    let mut counts = AgentEventArchiveMigrationCounts::default();
    let batch_limit = i64::try_from(batch_size.clamp(1, 1000))?;
    let mut scope_cache = HashMap::<(String, String, i64), Option<AgentEventArchiveScope>>::new();
    let mut task_conversation_cache = HashMap::<(String, String), Option<String>>::new();

    loop {
        let rows =
            fetch_agent_event_archive_rows(event_db, source, last_id, max_id, batch_limit).await?;
        if rows.is_empty() {
            break;
        }
        let mut documents = Vec::new();
        for row in rows {
            last_id = row.event_id;
            let scope = agent_event_archive_scope_for_row(
                main_db,
                &row,
                &mut scope_cache,
                &mut task_conversation_cache,
            )
            .await?;
            collect_agent_event_archive_row(
                row,
                scope.as_ref(),
                &mut documents,
                &mut chunk_accumulator,
                &mut counts,
            );
        }
        if !documents.is_empty() {
            archive.append_documents(&documents).await?;
        }
    }

    append_aggregated_acp_documents(
        main_db,
        archive,
        chunk_accumulator,
        batch_size,
        &mut scope_cache,
        &mut task_conversation_cache,
        &mut counts,
    )
    .await?;
    Ok(counts)
}

async fn fetch_agent_event_archive_rows(
    db: &SqlitePool,
    source: AgentEventArchiveSource<'_>,
    last_id: i64,
    max_id: i64,
    batch_limit: i64,
) -> anyhow::Result<Vec<AgentEventArchiveRow>> {
    let rows = match source {
        AgentEventArchiveSource::Main => sqlx::query(
            r#"
                SELECT id, agent_id, session_id, ts, stream, message
                FROM agent_events
                WHERE id > ?1 AND id <= ?2
                ORDER BY id ASC
                LIMIT ?3
                "#,
        )
        .bind(last_id)
        .bind(max_id)
        .bind(batch_limit)
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|row| AgentEventArchiveRow {
            event_id: row.get("id"),
            agent_id: row.get("agent_id"),
            session_id: row.get("session_id"),
            ts: row.get("ts"),
            stream: row.get("stream"),
            message: decode_message_from_storage(row.get::<Vec<u8>, _>("message").as_slice()),
        })
        .collect(),
        AgentEventArchiveSource::PerAgent { agent_id } => sqlx::query(
            r#"
                SELECT id, session_id, ts, stream, message
                FROM agent_events
                WHERE id > ?1 AND id <= ?2
                ORDER BY id ASC
                LIMIT ?3
                "#,
        )
        .bind(last_id)
        .bind(max_id)
        .bind(batch_limit)
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|row| AgentEventArchiveRow {
            event_id: row.get("id"),
            agent_id: agent_id.to_string(),
            session_id: row.get("session_id"),
            ts: row.get("ts"),
            stream: row.get("stream"),
            message: decode_message_from_storage(row.get::<Vec<u8>, _>("message").as_slice()),
        })
        .collect(),
    };
    Ok(rows)
}

fn collect_agent_event_archive_row(
    row: AgentEventArchiveRow,
    scope: Option<&AgentEventArchiveScope>,
    documents: &mut Vec<MessageDocument>,
    chunk_accumulator: &mut AcpChunkAccumulator,
    counts: &mut AgentEventArchiveMigrationCounts,
) {
    if row.stream == "acp"
        && chunk_accumulator.push_row(AcpEventRow {
            event_id: row.event_id,
            agent_id: row.agent_id.clone(),
            session_id: row.session_id.clone(),
            ts: row.ts,
            message: row.message.clone(),
        })
    {
        return;
    }

    documents.push(agent_event_archive_document(&row, scope));
    counts.agent_events += 1;
}

async fn agent_event_archive_scope_for_row(
    db: &SqlitePool,
    row: &AgentEventArchiveRow,
    scope_cache: &mut HashMap<(String, String, i64), Option<AgentEventArchiveScope>>,
    task_conversation_cache: &mut HashMap<(String, String), Option<String>>,
) -> anyhow::Result<Option<AgentEventArchiveScope>> {
    let cache_key = (row.agent_id.clone(), row.session_id.clone(), row.event_id);
    if let Some(scope) = scope_cache.get(&cache_key) {
        return Ok(scope.clone());
    }
    let scope = agent_event_archive_scope_for_session(
        db,
        &row.agent_id,
        &row.session_id,
        row.ts,
        task_conversation_cache,
    )
    .await?;
    scope_cache.insert(cache_key, scope.clone());
    Ok(scope)
}

async fn agent_event_archive_scope_for_session(
    db: &SqlitePool,
    agent_id: &str,
    session_id: &str,
    event_ts: i64,
    task_conversation_cache: &mut HashMap<(String, String), Option<String>>,
) -> anyhow::Result<Option<AgentEventArchiveScope>> {
    let row = sqlx::query(
        r#"
        SELECT r.team_id, r.id AS run_id, r.input_json
        FROM team_steps s
        INNER JOIN team_runs r ON r.id = s.run_id
        WHERE s.member_id = ?1
          AND s.remote_task_id = ?2
          AND COALESCE(s.started_at, r.started_at, r.created_at, 0) <= ?3
          AND (s.ended_at IS NULL OR s.ended_at >= ?3)
          AND trim(COALESCE(json_extract(r.input_json, '$.bootstrap_kind'), '')) != ?4
        ORDER BY COALESCE(s.started_at, 0) DESC, COALESCE(s.ended_at, 0) DESC, s.attempt DESC, s.id DESC
        LIMIT 1
        "#,
    )
    .bind(agent_id)
    .bind(session_id)
    .bind(event_ts)
    .bind(TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND)
    .fetch_optional(db)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let team_id: String = row.get("team_id");
    let run_id: String = row.get("run_id");
    let input_json: String = row.get("input_json");
    let run_input: Value = serde_json::from_str(&input_json)?;
    let base_scope = MessageArchiveScopeFallback::from_run_input(&run_input);
    let scope = message_archive_scope_for_payload_db(
        db,
        &team_id,
        &Value::Null,
        &base_scope,
        task_conversation_cache,
    )
    .await?;
    Ok(Some(AgentEventArchiveScope {
        team_id: Some(team_id),
        run_id: Some(run_id),
        conversation_id: scope.conversation_id,
        task_id: scope.task_id,
    }))
}

async fn append_aggregated_acp_documents(
    db: &SqlitePool,
    archive: &MessageArchiveStoreRef,
    accumulator: AcpChunkAccumulator,
    batch_size: usize,
    scope_cache: &mut HashMap<(String, String, i64), Option<AgentEventArchiveScope>>,
    task_conversation_cache: &mut HashMap<(String, String), Option<String>>,
    counts: &mut AgentEventArchiveMigrationCounts,
) -> anyhow::Result<()> {
    let mut documents = Vec::new();
    for message in accumulator.finish() {
        let row = AgentEventArchiveRow {
            event_id: message.first_event_id,
            agent_id: message.agent_id.clone(),
            session_id: message.session_id.clone(),
            ts: message.created_at,
            stream: "acp".to_string(),
            message: String::new(),
        };
        let scope =
            agent_event_archive_scope_for_row(db, &row, scope_cache, task_conversation_cache)
                .await?;
        documents.push(aggregated_acp_message_archive_document(
            &message,
            scope.as_ref(),
        ));
    }
    counts.aggregated_acp_messages += documents.len();
    for chunk in documents.chunks(batch_size.clamp(1, 1000)) {
        archive.append_documents(chunk).await?;
    }
    Ok(())
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

fn maybe_attach_context_artifact_pointer(
    payload: &mut Value,
    pointer: Option<&ContextArtifactPointer>,
    offload_reason: Option<&str>,
) {
    let Some(payload_obj) = payload.as_object_mut() else {
        return;
    };
    if let Some(pointer) = pointer {
        payload_obj.insert(
            "artifact_pointer".to_string(),
            build_context_artifact_pointer_payload(pointer),
        );
        payload_obj.insert(
            "artifact_offload_status".to_string(),
            Value::String("persisted".to_string()),
        );
    } else if let Some(reason) = offload_reason {
        payload_obj.insert(
            "artifact_offload_status".to_string(),
            Value::String("skipped".to_string()),
        );
        payload_obj.insert(
            "artifact_offload_reason".to_string(),
            Value::String(reason.to_string()),
        );
    }
}

async fn load_team_member_context_workspace_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    team_id: &str,
    member_id: &str,
) -> anyhow::Result<Option<TeamMemberContextWorkspace>> {
    let row = sqlx::query(
        r#"
        SELECT a.workdir, a.worktree_mode, td.spec_json
        FROM agents a, team_definitions td
        WHERE a.id = ?2
          AND td.id = ?1
        "#,
    )
    .bind(team_id)
    .bind(member_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };

    let workdir = row.get::<String, _>("workdir").trim().to_string();
    if workdir.is_empty() {
        return Ok(None);
    }
    let worktree_mode = match row.get::<String, _>("worktree_mode").trim() {
        "create_worktree" => WorktreeMode::CreateWorktree,
        "reuse_worktree" => WorktreeMode::ReuseWorktree,
        _ => WorktreeMode::UseExisting,
    };
    let spec_json = row.get::<String, _>("spec_json");

    let runtime_workdir = if let Some(member_role) = serde_json::from_str::<Value>(&spec_json)
        .ok()
        .and_then(|spec| team_member_role_from_spec(&spec, member_id))
    {
        let actor_context =
            build_team_member_actor_context_for_role(team_id, None, member_id, &member_role);
        derive_team_runtime_workdir(&workdir, &actor_context, &worktree_mode)
    } else {
        workdir
    };

    Ok(Some(TeamMemberContextWorkspace { runtime_workdir }))
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
) -> anyhow::Result<(TeamMemoryFlushResult, TeamRunEventRecord)> {
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
    let event = TeamManager::append_run_event_tx(
        tx,
        context.run_id,
        None,
        "memory_flush_failed",
        context.now,
        &Value::Object(payload),
    )
    .await?;
    Ok((
        TeamMemoryFlushResult {
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
        },
        event,
    ))
}

async fn finalize_memory_flush_noop_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    context: &MemoryFlushFinalizeContext<'_>,
) -> anyhow::Result<(TeamMemoryFlushResult, TeamRunEventRecord)> {
    let session_id = context
        .session_id
        .ok_or_else(|| anyhow::anyhow!("session_id is required for noop flush"))?;
    let event = TeamManager::append_run_event_tx(
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
    Ok((
        TeamMemoryFlushResult {
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
        },
        event,
    ))
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

fn compute_next_task_execution_context(
    current_status: &TeamTaskStatus,
    current_context: &Value,
    next_status: &TeamTaskStatus,
) -> Option<Value> {
    if *next_status != TeamTaskStatus::InProgress || *current_status == TeamTaskStatus::InProgress {
        return None;
    }

    let mut next_context = current_context.clone();
    let next_attempt_number = current_context
        .pointer("/execution/attempt_number")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        + 1;

    let context_obj = next_context
        .as_object_mut()
        .expect("team task context should always be a JSON object");
    let execution = context_obj
        .entry("execution".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let execution_obj = execution
        .as_object_mut()
        .expect("task execution context should always be a JSON object");
    execution_obj.insert(
        "attempt_number".to_string(),
        Value::Number(next_attempt_number.into()),
    );
    Some(next_context)
}

async fn load_run_status_sync_meta_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
) -> anyhow::Result<(String, Value)> {
    let run_meta_row = sqlx::query(
        r#"
        SELECT team_id, input_json
        FROM team_runs
        WHERE id = ?1
        "#,
    )
    .bind(run_id)
    .fetch_one(&mut **tx)
    .await?;
    let team_id: String = run_meta_row.get("team_id");
    let run_input_json: String = run_meta_row.get("input_json");
    let run_input: Value =
        serde_json::from_str(&run_input_json).unwrap_or_else(|_| serde_json::json!({}));
    Ok((team_id, run_input))
}

async fn sync_linked_task_status_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    team_id: &str,
    input: &Value,
    status: TeamTaskStatus,
    now: i64,
    preserve_waiting: bool,
) -> anyhow::Result<()> {
    let Some(task_id) = extract_linked_task_id_from_run_input(input) else {
        return Ok(());
    };

    let current_row = sqlx::query(
        r#"
        SELECT status, context_json
        FROM team_tasks
        WHERE id = ?1 AND team_id = ?2
        "#,
    )
    .bind(task_id)
    .bind(team_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(current_row) = current_row else {
        return Ok(());
    };

    let current_status_raw: String = current_row.get("status");
    let current_status = codec::team_task_status_from_str(&current_status_raw);
    let current_context_json: String = current_row.get("context_json");
    let current_context: Value =
        serde_json::from_str(&current_context_json).unwrap_or_else(|_| serde_json::json!({}));

    let effective_status = if preserve_waiting && current_status == TeamTaskStatus::Waiting {
        current_status.clone()
    } else {
        status
    };
    let next_context =
        compute_next_task_execution_context(&current_status, &current_context, &effective_status);
    if current_status == effective_status && next_context.is_none() {
        return Ok(());
    }

    let mut builder = QueryBuilder::<Sqlite>::new("UPDATE team_tasks SET ");
    let mut first = true;
    if current_status != effective_status {
        builder.push("status = ");
        builder.push_bind(team_task_status_to_str(&effective_status));
        first = false;
    }
    if let Some(next_context) = next_context.as_ref() {
        builder.push(", ");
        builder.push("context_json = ");
        builder.push_bind(next_context.to_string());
    }
    if !first {
        builder.push(", ");
    }
    builder.push("updated_at = ");
    builder.push_bind(now);
    builder.push(" WHERE id = ");
    builder.push_bind(task_id);
    builder.push(" AND team_id = ");
    builder.push_bind(team_id);
    builder.build().execute(&mut **tx).await?;
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
struct RuntimeStateSnapshotWritePlan {
    team_id: String,
    run_id: String,
    member_id: String,
    state_path: PathBuf,
    state_text: String,
    continuity_note: Option<(PathBuf, String)>,
}

fn build_runtime_state_snapshot_text(
    owner: ContextArtifactOwner<'_>,
    continuity_mode: &str,
    continuity_state: &TeamMemberContinuityStateRecord,
) -> String {
    let mut lines = vec![
        "# Team Runtime State".to_string(),
        String::new(),
        format!("- schema_family: {TEAM_RUNTIME_STATE_SCHEMA_FAMILY}"),
        format!("- schema_version: {TEAM_RUNTIME_STATE_SCHEMA_VERSION}"),
        format!("- updated_at: {}", continuity_state.updated_at),
        format!("- team_id: {}", owner.team_id),
        format!("- member_id: {}", owner.member_id),
        format!("- current_execution_run_id: {}", owner.run_id),
        format!("- continuity_mode: {continuity_mode}"),
        format!(
            "- continuity_source_execution_run_id: {}",
            continuity_state.source_run_id
        ),
    ];
    if let Some(note_path) = continuity_note_relative_path(&continuity_state.source_run_id) {
        lines.push(format!("- continuity_note_path: {note_path}"));
    }
    if let Some(artifact_path) = continuity_state
        .history_window
        .get("artifact_pointer")
        .and_then(extract_context_artifact_path)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("- continuity_artifact_path: {artifact_path}"));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn build_runtime_continuity_note_text(
    owner: ContextArtifactOwner<'_>,
    continuity_mode: &str,
    continuity_state: &TeamMemberContinuityStateRecord,
) -> String {
    let history_window = serde_json::to_string_pretty(&continuity_state.history_window)
        .unwrap_or_else(|_| continuity_state.history_window.to_string());
    let mut lines = vec![
        "# Team Continuity Note".to_string(),
        String::new(),
        format!("- schema_family: {TEAM_CONTINUITY_NOTE_SCHEMA_FAMILY}"),
        format!("- schema_version: {TEAM_CONTINUITY_NOTE_SCHEMA_VERSION}"),
        format!("- updated_at: {}", continuity_state.updated_at),
        format!("- team_id: {}", owner.team_id),
        format!("- member_id: {}", owner.member_id),
        format!("- current_execution_run_id: {}", owner.run_id),
        format!(
            "- continuity_source_execution_run_id: {}",
            continuity_state.source_run_id
        ),
        format!("- continuity_mode: {continuity_mode}"),
    ];
    if let Some(artifact_path) = continuity_state
        .history_window
        .get("artifact_pointer")
        .and_then(extract_context_artifact_path)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("- continuity_artifact_path: {artifact_path}"));
    }
    lines.extend([
        String::new(),
        "## Summary".to_string(),
        continuity_state.summary_text.clone(),
        String::new(),
        "## History Window".to_string(),
        "````json".to_string(),
        history_window,
        "````".to_string(),
        String::new(),
    ]);
    lines.join("\n")
}

#[derive(Debug, Clone)]
struct ContextArtifactPointer {
    artifact_kind: String,
    relative_path: String,
    artifact_size_bytes: i64,
    content_checksum: String,
}

#[derive(Debug, Clone, Copy)]
struct ReconcileRoundArtifactSnapshot<'a> {
    round: i64,
    status: &'a str,
    summary: Option<&'a str>,
    output: Option<&'a Value>,
    input: Option<&'a Value>,
    reason: Option<&'a str>,
    error_text: Option<&'a str>,
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

fn validate_materialized_run_step_templates(
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

fn merge_step_input(current_input: Option<&Value>, next_input: Option<Value>) -> Option<Value> {
    next_input.map(|next_input| {
        if let Some(current_input) = current_input
            && next_input.get("task_execution_step").is_none()
        {
            let mut merged = current_input.clone();
            merge_json_value(&mut merged, &next_input);
            return merged;
        }
        next_input
    })
}

fn extract_reconcile_round_runtime(input: Option<&Value>) -> Option<ReconcileRoundRuntime> {
    let root = input?.as_object()?;
    let step = root.get("task_execution_step")?.as_object()?;
    let execution = step
        .get("execution")
        .map(|value| serde_json::from_value::<TeamTaskStepExecutionSpec>(value.clone()))
        .transpose()
        .ok()?
        .unwrap_or_default();
    if execution.mode != super::TeamTaskStepExecutionMode::ReconcileLoop {
        return None;
    }
    let current_round = step
        .get("round_state")
        .and_then(Value::as_object)
        .and_then(|round_state| round_state.get("current_round"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
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
    Some(ReconcileRoundRuntime {
        current_round,
        goal,
        acceptance,
        execution,
    })
}

fn build_reconcile_round_started_input(input: Option<&Value>) -> Option<(Value, i64)> {
    let runtime = extract_reconcile_round_runtime(input)?;
    let next_round = runtime.current_round + 1;
    let mut next = input.cloned().unwrap_or_else(|| serde_json::json!({}));
    let root = next.as_object_mut()?;
    let step = root.get_mut("task_execution_step")?.as_object_mut()?;
    let round_state = step
        .entry("round_state".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let round_state = round_state.as_object_mut()?;
    round_state.insert("current_round".to_string(), Value::from(next_round));
    round_state.insert("latest_status".to_string(), Value::from("working"));
    round_state.remove("latest_outcome");
    round_state.remove("latest_summary");
    Some((next, next_round))
}

fn build_reconcile_round_finished_input(
    input: Option<&Value>,
    outcome: &str,
    summary: Option<&str>,
) -> Option<(Value, i64)> {
    let runtime = extract_reconcile_round_runtime(input)?;
    let mut next = input.cloned().unwrap_or_else(|| serde_json::json!({}));
    let root = next.as_object_mut()?;
    let step = root.get_mut("task_execution_step")?.as_object_mut()?;
    let round_state = step
        .entry("round_state".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let round_state = round_state.as_object_mut()?;
    round_state.insert(
        "current_round".to_string(),
        Value::from(runtime.current_round.max(1)),
    );
    round_state.insert("latest_status".to_string(), Value::from(outcome));
    round_state.insert("latest_outcome".to_string(), Value::from(outcome));
    if let Some(summary) = summary.map(str::trim).filter(|value| !value.is_empty()) {
        round_state.insert("latest_summary".to_string(), Value::from(summary));
    } else {
        round_state.remove("latest_summary");
    }
    Some((next, runtime.current_round.max(1)))
}

fn summarize_reconcile_output(output: Option<&Value>) -> Option<String> {
    let output = output?;
    output
        .as_object()
        .and_then(|obj| obj.get("summary"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            let text = output.to_string();
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| truncate_chars(trimmed, 240))
        })
}

fn extract_materialized_run_step_templates_from_input(
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

fn build_materialized_run_step_templates_from_task_execution_plan(
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

async fn insert_materialized_run_steps_tx(
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

async fn load_step_record_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    step_id: &str,
) -> anyhow::Result<TeamStepRecord> {
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
    .fetch_one(&mut **tx)
    .await?;
    parse_team_step_row(&step_row)
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

fn resolve_task_context_patch(current: &Value, patch: TeamTaskContextPatch) -> Option<Value> {
    let next = match patch {
        TeamTaskContextPatch::Replace(value) => redact_sensitive_json(&value),
        TeamTaskContextPatch::Merge(value) => {
            let mut merged = current.clone();
            merge_json_value(&mut merged, &value);
            redact_sensitive_json(&merged)
        }
    };
    (next != *current).then_some(next)
}

fn merge_json_value(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(target_obj), Value::Object(patch_obj)) => {
            for (key, patch_value) in patch_obj {
                match target_obj.get_mut(key) {
                    Some(target_value) => merge_json_value(target_value, patch_value),
                    None => {
                        target_obj.insert(key.clone(), patch_value.clone());
                    }
                }
            }
        }
        (target, patch) => *target = patch.clone(),
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
    async fn load_task_detail_for_team(
        &self,
        team_id: &str,
        task_id: &str,
    ) -> anyhow::Result<(
        TeamTaskRecord,
        TeamConversationRecord,
        Option<TeamRunRecord>,
    )> {
        let task = self.get_task(task_id).await?;
        if task.team_id != team_id {
            anyhow::bail!("task not found for team");
        }
        let conversation = self.get_task_conversation(task_id).await?;
        let latest_run = self.get_latest_run_for_task(team_id, task_id).await?;
        Ok((task, conversation, latest_run))
    }

    pub async fn get_shared_thread_detail_for_team(
        &self,
        team_id: &str,
    ) -> anyhow::Result<
        Option<(
            TeamTaskRecord,
            TeamConversationRecord,
            Option<TeamRunRecord>,
        )>,
    > {
        let Some(target) = fetch_canonical_shared_thread_target(&self.db, team_id).await? else {
            return Ok(None);
        };
        match self
            .load_task_detail_for_team(team_id, &target.task_id)
            .await
        {
            Ok(detail) => Ok(Some(detail)),
            Err(err) if is_row_not_found(&err) => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub async fn ensure_shared_thread_detail_for_team(
        &self,
        team_id: &str,
        created_by_actor_id: &str,
    ) -> anyhow::Result<(
        TeamTaskRecord,
        TeamConversationRecord,
        Option<TeamRunRecord>,
    )> {
        let (task_id, _) = self
            .ensure_shared_thread_target_for_team(team_id, created_by_actor_id)
            .await?;
        self.load_task_detail_for_team(team_id, &task_id).await
    }

    pub async fn ensure_shared_thread_target_for_team(
        &self,
        team_id: &str,
        created_by_actor_id: &str,
    ) -> anyhow::Result<(String, String)> {
        let mut tx = self.db.begin().await?;
        if let Some(existing) = fetch_canonical_shared_thread_target(&mut *tx, team_id).await? {
            tx.commit().await?;
            return Ok((existing.task_id, existing.conversation_id));
        }

        let now = Utc::now().timestamp();
        let task_id = Uuid::new_v4().to_string();
        let conversation_id = Uuid::new_v4().to_string();
        let context_json = serde_json::json!({
            "bootstrap_kind": TEAM_SHARED_THREAD_BOOTSTRAP_KIND,
            "bootstrap_source": "server_canonical_reply",
        })
        .to_string();

        sqlx::query(
            r#"
            INSERT INTO team_tasks (
                id,
                team_id,
                group_id,
                title,
                status,
                created_by_actor_id,
                assigned_member_id,
                context_json,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, (SELECT group_id FROM team_definitions WHERE id = ?2), ?3, 'open', ?4, NULL, ?5, ?6, ?7)
            "#,
        )
        .bind(&task_id)
        .bind(team_id)
        .bind(TEAM_SHARED_THREAD_TITLE)
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
            VALUES (?1, ?2, ?3, 'group_chat', ?4, ?5, ?6)
            "#,
        )
        .bind(&conversation_id)
        .bind(team_id)
        .bind(&task_id)
        .bind(TEAM_SHARED_THREAD_TITLE)
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
        let target = fetch_canonical_shared_thread_target(&self.db, &team_id).await?;
        Ok(target.map(|target| (team_id, target.task_id, target.conversation_id)))
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
        role: member.role.clone(),
        skills: crate::team::effective_team_member_skills(&member.role),
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

fn push_fingerprint_component(hasher: &mut Sha256, value: &str) {
    hasher.update(value.as_bytes());
    hasher.update([0_u8]);
}

fn task_conversation_message_fingerprint(
    task_id: &str,
    from_actor_id: &str,
    to_actor_id: Option<&str>,
    route: &str,
    payload: &Value,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update("task-conversation-message-fingerprint:v1");
    push_fingerprint_component(&mut hasher, task_id);
    push_fingerprint_component(&mut hasher, from_actor_id);
    push_fingerprint_component(&mut hasher, to_actor_id.unwrap_or(""));
    push_fingerprint_component(&mut hasher, route);
    push_fingerprint_component(&mut hasher, canonical_json(payload).as_str());
    hex_encode(&hasher.finalize())
}

async fn fetch_task_conversation_message_by_idempotency(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    conversation_id: &str,
    from_actor_id: &str,
    idempotency_key: &str,
) -> Result<TeamConversationMessageRecord, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            id,
            conversation_id,
            task_id,
            group_id,
            from_actor_id,
            to_actor_id,
            route,
            payload_json,
            created_at
        FROM team_conversation_messages
        WHERE conversation_id = ?1
          AND from_actor_id = ?2
          AND idempotency_key = ?3
        LIMIT 1
        "#,
    )
    .bind(conversation_id)
    .bind(from_actor_id)
    .bind(idempotency_key)
    .fetch_one(&mut **tx)
    .await?;
    parse_team_conversation_message_row(&row).map_err(|err| sqlx::Error::Protocol(err.to_string()))
}

fn ensure_task_conversation_message_idempotency_compatible(
    task_id: &str,
    from_actor_id: &str,
    to_actor_id: Option<&str>,
    route: &str,
    payload: &Value,
    existing: &TeamConversationMessageRecord,
) -> Result<(), TaskConversationMessageStoreError> {
    let incoming_fp =
        task_conversation_message_fingerprint(task_id, from_actor_id, to_actor_id, route, payload);
    let existing_fp = task_conversation_message_fingerprint(
        &existing.task_id,
        &existing.from_actor_id,
        existing.to_actor_id.as_deref(),
        &existing.route,
        &existing.payload,
    );
    if incoming_fp != existing_fp {
        return Err(TaskConversationMessageStoreError::IdempotencyConflict);
    }
    Ok(())
}

fn is_task_conversation_message_idempotency_unique_violation(err: &SqlxError) -> bool {
    match err {
        SqlxError::Database(db_err) => {
            db_err.code().as_deref() == Some(SQLITE_CONSTRAINT_UNIQUE_CODE)
                && db_err
                    .message()
                    .contains(TASK_CONVERSATION_MESSAGE_IDEMPOTENCY_UNIQUE_COLUMNS)
        }
        _ => false,
    }
}

fn is_team_channel_bootstrap_unique_violation(err: &SqlxError) -> bool {
    match err {
        SqlxError::Database(db_err) => {
            let message = db_err.message();
            db_err.code().as_deref() == Some(SQLITE_CONSTRAINT_UNIQUE_CODE)
                && (message.contains(TEAM_CHANNEL_BOOTSTRAP_UNIQUE_INDEX)
                    || (message.contains("team_tasks.team_id")
                        && message.contains(TEAM_CHANNEL_BOOTSTRAP_UNIQUE_CHANNEL_EXPR)))
        }
        _ => false,
    }
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
