use std::collections::HashMap;

use serde_json::Value;
use sqlx::{Row, SqlitePool};

use super::{TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND, redact_sensitive_json};
use crate::team::{
    TeamActorMessageRecord, TeamConversationMessageRecord, TeamConversationRecord,
    TeamRunEventRecord,
};
use agenthub_message_archive::{AcpAggregatedMessage, MessageDocument, MessageDocumentKind};

pub(super) fn team_conversation_message_archive_document(
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

#[derive(Debug, Clone, Default)]
pub(super) struct MessageArchiveScopeFallback {
    pub(super) conversation_id: Option<String>,
    pub(super) task_id: Option<String>,
}

pub(super) struct TeamRunArchiveScope {
    pub(super) team_id: String,
    pub(super) group_id: Option<String>,
    pub(super) base_scope: MessageArchiveScopeFallback,
}

type TeamRunArchiveScopeCache = HashMap<String, Option<TeamRunArchiveScope>>;

impl MessageArchiveScopeFallback {
    pub(super) fn from_run_input(run_input: &Value) -> Self {
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

pub(super) async fn message_archive_scope_for_payload_db(
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

pub(super) async fn team_run_event_archive_document_for_db_cached(
    db: &SqlitePool,
    event: &TeamRunEventRecord,
    run_scope_cache: &mut TeamRunArchiveScopeCache,
    task_conversation_cache: &mut HashMap<(String, String), Option<String>>,
) -> anyhow::Result<Option<MessageDocument>> {
    if !run_scope_cache.contains_key(&event.run_id) {
        let resolved = team_run_event_archive_scope_for_db(db, &event.run_id).await?;
        run_scope_cache.insert(event.run_id.clone(), resolved);
    }
    let Some(Some(cached_scope)) = run_scope_cache.get(&event.run_id) else {
        return Ok(None);
    };
    let scope = message_archive_scope_for_payload_db(
        db,
        &cached_scope.team_id,
        &event.payload,
        &cached_scope.base_scope,
        task_conversation_cache,
    )
    .await?;
    Ok(Some(team_run_event_archive_document(
        &cached_scope.team_id,
        event,
        &scope,
        cached_scope.group_id.as_deref(),
    )))
}

async fn team_run_event_archive_scope_for_db(
    db: &SqlitePool,
    run_id: &str,
) -> anyhow::Result<Option<TeamRunArchiveScope>> {
    let row = sqlx::query(
        r#"
        SELECT team_id, group_id, input_json
        FROM team_runs
        WHERE id = ?1
        "#,
    )
    .bind(run_id)
    .fetch_one(db)
    .await?;
    let team_id: String = row.get("team_id");
    let group_id: Option<String> = row.get("group_id");
    let input_json: String = row.get("input_json");
    let run_input: Value = serde_json::from_str(&input_json)?;
    if message_archive_payload_string(&run_input, "bootstrap_kind").as_deref()
        == Some(TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND)
    {
        return Ok(None);
    }
    Ok(Some(TeamRunArchiveScope {
        team_id,
        group_id,
        base_scope: MessageArchiveScopeFallback::from_run_input(&run_input),
    }))
}

pub(super) fn team_run_event_archive_document(
    team_id: &str,
    event: &TeamRunEventRecord,
    scope: &MessageArchiveScopeFallback,
    group_id: Option<&str>,
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
        group_id: group_id.map(str::to_string),
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

pub(super) fn team_actor_message_archive_document(
    team_id: &str,
    message: &TeamActorMessageRecord,
    scope: &MessageArchiveScopeFallback,
    group_id: Option<String>,
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
        group_id,
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
pub(super) struct AgentEventArchiveRow {
    pub(super) event_id: i64,
    pub(super) agent_id: String,
    pub(super) session_id: String,
    pub(super) ts: i64,
    pub(super) stream: String,
    pub(super) message: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct AgentEventArchiveMigrationCounts {
    pub(super) agent_events: usize,
    pub(super) aggregated_acp_messages: usize,
}

impl AgentEventArchiveMigrationCounts {
    pub(super) fn add(&mut self, other: AgentEventArchiveMigrationCounts) {
        self.agent_events += other.agent_events;
        self.aggregated_acp_messages += other.aggregated_acp_messages;
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct AgentEventArchiveScope {
    pub(super) team_id: Option<String>,
    pub(super) run_id: Option<String>,
    pub(super) conversation_id: Option<String>,
    pub(super) task_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct AgentEventArchiveSnapshot {
    pub(super) main_max_id: i64,
    pub(super) per_agent_max_ids: Vec<(String, i64)>,
}

pub(super) fn agent_event_archive_document(
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

pub(super) fn aggregated_acp_message_archive_document(
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

pub(super) fn message_archive_payload_string_any(payload: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| message_archive_payload_string(payload, key))
}

pub(super) fn message_archive_payload_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn message_archive_payload_i64(payload: &Value, key: &str) -> Option<i64> {
    match payload.get(key)? {
        Value::Number(value) => value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|raw| i64::try_from(raw).ok())),
        Value::String(value) => value.trim().parse::<i64>().ok(),
        _ => None,
    }
}

pub(super) fn message_archive_body_text(payload: &Value) -> String {
    payload
        .as_str()
        .or_else(|| payload.get("text").and_then(Value::as_str))
        .or_else(|| payload.get("summary").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_default()
}
