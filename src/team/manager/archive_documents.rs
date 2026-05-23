use serde_json::Value;

use super::archive_payloads::{
    message_archive_body_text, message_archive_payload_i64, message_archive_payload_string,
    message_archive_payload_string_any,
};
use super::redact_sensitive_json;
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
