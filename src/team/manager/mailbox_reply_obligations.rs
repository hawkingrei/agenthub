use std::collections::HashMap;

use serde_json::{Map, Value};
use sqlx::sqlite::SqliteRow;
use sqlx::{Executor, Row, Sqlite};

use super::TeamReplyObligationSummary;
use super::mailbox::{
    MAILBOX_RESOLUTION_ESCALATED, ReplyActorPairKey, normalize_optional_sqlite_string,
    parse_optional_sqlite_json_value,
};
use super::mailbox_payloads::{
    CanonicalChatReply, is_human_actor_id, parse_stringified_json_payload,
    resolve_canonical_chat_reply,
};
use crate::team::{
    TeamActorMessageRecord, TeamActorMessageStatus, TeamActorMessageTransport,
    TeamReplyObligationRecord,
};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ActorIdentityKind, ActorMessageHandlingDisposition, ActorMessageKind,
    parse_actor_message_handling_disposition, parse_actor_message_kind,
};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ReplyObligationMessageSnapshot {
    pub(super) message_id: i64,
    pub(super) from_actor_id: String,
    pub(super) to_actor_id: String,
    pub(super) to_peer_id: String,
    pub(super) transport: TeamActorMessageTransport,
    pub(super) status: TeamActorMessageStatus,
    pub(super) handling_disposition: ActorMessageHandlingDisposition,
    pub(super) message_kind: ActorMessageKind,
    pub(super) source_surface: Option<String>,
    pub(super) reply_target: Option<Value>,
    pub(super) conversation_id: Option<String>,
    pub(super) thread_root_message_id: Option<i64>,
    pub(super) requires_user_visible_reply: bool,
    pub(super) mailbox_resolution_kind: Option<String>,
    pub(super) reply_payload_type: Option<String>,
    pub(super) reply_text: Option<String>,
    pub(super) reply_correlation_id: Option<String>,
    pub(super) reply_string_payload: Option<String>,
    pub(super) created_at: i64,
}

fn parse_reply_obligation_message_snapshot_row(
    row: &SqliteRow,
) -> Result<ReplyObligationMessageSnapshot, sqlx::Error> {
    let transport_raw: String = row.get("transport");
    let status_raw: String = row.get("status");
    let handling_disposition_raw: String = row
        .try_get("handling_disposition")
        .unwrap_or_else(|_| "untriaged".to_string());
    let message_kind_raw: String = row.try_get("message_kind").unwrap_or_default();
    let requires_user_visible_reply = row
        .try_get::<Option<i64>, _>("requires_user_visible_reply")
        .ok()
        .flatten()
        .unwrap_or_default()
        != 0;
    Ok(ReplyObligationMessageSnapshot {
        message_id: row.get("id"),
        from_actor_id: row.get("from_actor_id"),
        to_actor_id: row.get("to_actor_id"),
        to_peer_id: row
            .try_get("to_peer_id")
            .unwrap_or_else(|_| ACTOR_MAIN_PEER_ID.to_string()),
        transport: super::codec::team_actor_message_transport_from_str(&transport_raw),
        status: super::codec::team_actor_message_status_from_str(&status_raw),
        handling_disposition: parse_actor_message_handling_disposition(&handling_disposition_raw),
        message_kind: parse_actor_message_kind(&message_kind_raw),
        source_surface: normalize_optional_sqlite_string(row.try_get("source_surface")?),
        reply_target: parse_optional_sqlite_json_value(row.try_get("reply_target_json")?)?,
        conversation_id: normalize_optional_sqlite_string(row.try_get("conversation_id")?),
        thread_root_message_id: row.try_get("thread_root_message_id").ok().flatten(),
        requires_user_visible_reply,
        mailbox_resolution_kind: normalize_optional_sqlite_string(
            row.try_get("mailbox_resolution_kind")?,
        ),
        reply_payload_type: normalize_optional_sqlite_string(row.try_get("reply_payload_type")?),
        reply_text: normalize_optional_sqlite_string(row.try_get("reply_text")?),
        reply_correlation_id: normalize_optional_sqlite_string(
            row.try_get("reply_correlation_id")?,
        ),
        reply_string_payload: normalize_optional_sqlite_string(
            row.try_get("reply_string_payload")?,
        ),
        created_at: row.get("created_at"),
    })
}

pub(super) async fn load_reply_obligation_message_snapshots_on_executor<'e, E>(
    executor: E,
    run_id: &str,
) -> Result<Vec<ReplyObligationMessageSnapshot>, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query(
        r#"
        SELECT
            id,
            from_actor_id,
            to_actor_id,
            to_peer_id,
            transport,
            status,
            message_kind,
            handling_disposition,
            trim(COALESCE(json_extract(payload_json, '$.source_surface'), '')) AS source_surface,
            json_extract(payload_json, '$.reply_target') AS reply_target_json,
            trim(COALESCE(
                json_extract(payload_json, '$.task_conversation_id'),
                json_extract(payload_json, '$.channel_conversation_id'),
                json_extract(payload_json, '$.conversation_id'),
                ''
            )) AS conversation_id,
            CAST(json_extract(payload_json, '$.thread_root_message_id') AS INTEGER) AS thread_root_message_id,
            COALESCE(CAST(json_extract(payload_json, '$.requires_user_visible_reply') AS INTEGER), 0) AS requires_user_visible_reply,
            trim(COALESCE(json_extract(payload_json, '$.mailbox_resolution.kind'), '')) AS mailbox_resolution_kind,
            trim(COALESCE(json_extract(payload_json, '$.type'), '')) AS reply_payload_type,
            trim(COALESCE(json_extract(payload_json, '$.text'), '')) AS reply_text,
            trim(COALESCE(json_extract(payload_json, '$.correlation_id'), '')) AS reply_correlation_id,
            CASE
                WHEN json_type(payload_json) = 'text' THEN trim(COALESCE(json_extract(payload_json, '$'), ''))
                ELSE NULL
            END AS reply_string_payload,
            created_at
        FROM team_actor_messages
        WHERE run_id = ?1
          AND (
              COALESCE(CAST(json_extract(payload_json, '$.requires_user_visible_reply') AS INTEGER), 0) != 0
              OR message_kind = 'human_request'
              OR (
                  transport = 'local'
                  AND to_peer_id = 'main'
                  AND (
                      to_actor_id = 'user'
                      OR to_actor_id = 'human'
                      OR to_actor_id LIKE 'user:%'
                      OR to_actor_id LIKE 'human:%'
                  )
              )
          )
        ORDER BY id ASC
        "#,
    )
    .bind(run_id)
    .fetch_all(executor)
    .await?;
    let mut snapshots = Vec::with_capacity(rows.len());
    for row in rows {
        snapshots.push(parse_reply_obligation_message_snapshot_row(&row)?);
    }
    Ok(snapshots)
}

pub(super) fn summarize_open_reply_obligations_from_snapshots(
    messages: &[ReplyObligationMessageSnapshot],
) -> TeamReplyObligationSummary {
    let mut visible_reply_credits = HashMap::<ReplyActorPairKey, i64>::new();
    let mut summary = TeamReplyObligationSummary::default();

    for message in messages.iter().rev() {
        if let Some(pair_key) = reply_actor_pair_for_visible_reply_snapshot(message) {
            *visible_reply_credits.entry(pair_key).or_default() += 1;
            continue;
        }
        let Some(pair_key) = reply_actor_pair_for_inbound_obligation_snapshot(message) else {
            continue;
        };
        if reply_obligation_snapshot_is_terminal(message) {
            continue;
        }
        if let Some(credits) = visible_reply_credits.get_mut(&pair_key)
            && *credits > 0
        {
            *credits -= 1;
            continue;
        }
        *summary
            .open_by_actor
            .entry(pair_key.agent_actor_id.clone())
            .or_default() += 1;
        summary.open_total += 1;
        summary
            .open_items
            .push(build_reply_obligation_record_from_snapshot(
                message, pair_key,
            ));
    }

    summary
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn summarize_open_reply_obligations_from_messages(
    messages: &[TeamActorMessageRecord],
) -> TeamReplyObligationSummary {
    let snapshots = messages
        .iter()
        .map(reply_obligation_snapshot_from_message)
        .collect::<Vec<_>>();
    summarize_open_reply_obligations_from_snapshots(snapshots.as_slice())
}

pub(super) fn reply_actor_pair_for_inbound_obligation(
    message: &TeamActorMessageRecord,
) -> Option<ReplyActorPairKey> {
    let envelope = message.inbound_envelope();
    if !envelope.requires_user_visible_reply {
        return None;
    }
    if message.from_actor_kind != ActorIdentityKind::Human
        || message.to_actor_kind != ActorIdentityKind::Agent
    {
        return None;
    }
    Some(ReplyActorPairKey {
        agent_actor_id: message.to_actor_id.clone(),
        human_actor_id: message.from_actor_id.clone(),
    })
}

pub(super) fn reply_obligation_is_terminal(message: &TeamActorMessageRecord) -> bool {
    if matches!(
        message.handling_disposition,
        ActorMessageHandlingDisposition::Ignored | ActorMessageHandlingDisposition::Completed
    ) {
        return true;
    }
    matches!(
        (
            message.handling_disposition.clone(),
            mailbox_resolution_kind(&message.payload),
        ),
        (
            ActorMessageHandlingDisposition::Released,
            Some(MAILBOX_RESOLUTION_ESCALATED)
        )
    )
}

pub(super) fn build_mailbox_resolution_payload(
    source_payload: &Value,
    kind: &str,
    resolved_by_actor_id: &str,
    target_actor_id: &str,
    resolved_at: i64,
) -> Value {
    let mut payload_obj = match source_payload {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    payload_obj.insert(
        "mailbox_resolution".to_string(),
        serde_json::json!({
            "kind": kind,
            "resolved_by_actor_id": resolved_by_actor_id,
            "target_actor_id": target_actor_id,
            "resolved_at": resolved_at
        }),
    );
    Value::Object(payload_obj)
}

pub(super) fn build_escalated_mailbox_payload(
    source_payload: &Value,
    source_message_id: i64,
    source_actor_id: &str,
    target_actor_id: &str,
    escalated_by_actor_id: &str,
    escalated_at: i64,
) -> Value {
    let mut payload_obj = match source_payload {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    payload_obj.insert(
        "mailbox_escalation".to_string(),
        serde_json::json!({
            "kind": MAILBOX_RESOLUTION_ESCALATED,
            "source_message_id": source_message_id,
            "source_actor_id": source_actor_id,
            "target_actor_id": target_actor_id,
            "escalated_by_actor_id": escalated_by_actor_id,
            "escalated_at": escalated_at
        }),
    );
    Value::Object(payload_obj)
}

pub(super) fn has_visible_reply_credit_for_message(
    messages: &[ReplyObligationMessageSnapshot],
    target_message_id: i64,
) -> bool {
    let mut visible_reply_credits = HashMap::<ReplyActorPairKey, i64>::new();
    for message in messages.iter().rev() {
        if let Some(pair_key) = reply_actor_pair_for_visible_reply_snapshot(message) {
            *visible_reply_credits.entry(pair_key).or_default() += 1;
            continue;
        }
        let Some(pair_key) = reply_actor_pair_for_inbound_obligation_snapshot(message) else {
            continue;
        };
        if reply_obligation_snapshot_is_terminal(message) {
            continue;
        }
        if let Some(credits) = visible_reply_credits.get_mut(&pair_key)
            && *credits > 0
        {
            if message.message_id == target_message_id {
                return true;
            }
            *credits -= 1;
            continue;
        }
        if message.message_id == target_message_id {
            return false;
        }
    }
    true
}

fn reply_actor_pair_for_inbound_obligation_snapshot(
    message: &ReplyObligationMessageSnapshot,
) -> Option<ReplyActorPairKey> {
    if !(message.requires_user_visible_reply
        || (message.message_kind == ActorMessageKind::HumanRequest
            && is_human_actor_id(&message.from_actor_id)
            && !is_human_actor_id(&message.to_actor_id)))
    {
        return None;
    }
    if !is_human_actor_id(&message.from_actor_id) || is_human_actor_id(&message.to_actor_id) {
        return None;
    }
    Some(ReplyActorPairKey {
        agent_actor_id: message.to_actor_id.clone(),
        human_actor_id: message.from_actor_id.clone(),
    })
}

fn reply_actor_pair_for_visible_reply_snapshot(
    message: &ReplyObligationMessageSnapshot,
) -> Option<ReplyActorPairKey> {
    if message.status == TeamActorMessageStatus::DeadLetter
        || message.transport != TeamActorMessageTransport::Local
        || message.to_peer_id != ACTOR_MAIN_PEER_ID
        || !is_human_actor_id(&message.to_actor_id)
        || is_human_actor_id(&message.from_actor_id)
        || resolve_canonical_chat_reply_from_snapshot(message).is_none()
    {
        return None;
    }
    Some(ReplyActorPairKey {
        agent_actor_id: message.from_actor_id.clone(),
        human_actor_id: message.to_actor_id.clone(),
    })
}

fn build_reply_obligation_record_from_snapshot(
    message: &ReplyObligationMessageSnapshot,
    pair_key: ReplyActorPairKey,
) -> TeamReplyObligationRecord {
    TeamReplyObligationRecord {
        message_id: message.message_id,
        agent_actor_id: pair_key.agent_actor_id,
        human_actor_id: pair_key.human_actor_id,
        source_surface: reply_obligation_source_surface(message),
        reply_target: message.reply_target.clone(),
        conversation_id: message.conversation_id.clone(),
        thread_root_message_id: message.thread_root_message_id,
        text_excerpt: resolve_canonical_chat_reply_from_snapshot(message).map(|reply| reply.text),
        created_at: message.created_at,
    }
}

fn mailbox_resolution_kind(payload: &Value) -> Option<&str> {
    payload
        .as_object()
        .and_then(|map| map.get("mailbox_resolution"))
        .and_then(Value::as_object)
        .and_then(|map| map.get("kind"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn reply_obligation_snapshot_is_terminal(message: &ReplyObligationMessageSnapshot) -> bool {
    if matches!(
        message.handling_disposition,
        ActorMessageHandlingDisposition::Ignored | ActorMessageHandlingDisposition::Completed
    ) {
        return true;
    }
    matches!(
        (
            message.handling_disposition.clone(),
            message.mailbox_resolution_kind.as_deref(),
        ),
        (
            ActorMessageHandlingDisposition::Released,
            Some(MAILBOX_RESOLUTION_ESCALATED)
        )
    )
}

fn reply_obligation_source_surface(message: &ReplyObligationMessageSnapshot) -> String {
    if let Some(source_surface) = message.source_surface.as_deref() {
        return source_surface.to_string();
    }
    if message.thread_root_message_id.is_some() {
        return "thread".to_string();
    }
    if message.conversation_id.is_some() {
        return "conversation".to_string();
    }
    match message.message_kind {
        ActorMessageKind::TriggerEvent => "trigger".to_string(),
        ActorMessageKind::SystemNotice => "system".to_string(),
        _ => "mailbox".to_string(),
    }
}

fn resolve_canonical_chat_reply_from_snapshot(
    message: &ReplyObligationMessageSnapshot,
) -> Option<CanonicalChatReply> {
    if let Some(raw_text) = message.reply_string_payload.as_deref() {
        if let Some(parsed) = parse_stringified_json_payload(raw_text)
            && let Some(reply) = resolve_canonical_chat_reply(&parsed)
        {
            return Some(reply);
        }
        let trimmed = raw_text.trim();
        if trimmed.is_empty() {
            return None;
        }
        return Some(CanonicalChatReply {
            text: raw_text.to_string(),
            correlation_id: None,
        });
    }
    let payload_type = message
        .reply_payload_type
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    if !payload_type.is_empty() && payload_type != "chat_message" {
        return None;
    }
    let text = message
        .reply_text
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    Some(CanonicalChatReply {
        text,
        correlation_id: message.reply_correlation_id.clone(),
    })
}

#[cfg_attr(not(test), allow(dead_code))]
fn reply_obligation_snapshot_from_message(
    message: &TeamActorMessageRecord,
) -> ReplyObligationMessageSnapshot {
    let envelope = message.inbound_envelope();
    let reply = resolve_canonical_chat_reply(&message.payload);
    ReplyObligationMessageSnapshot {
        message_id: message.message_id,
        from_actor_id: message.from_actor_id.clone(),
        to_actor_id: message.to_actor_id.clone(),
        to_peer_id: message.to_peer_id.clone(),
        transport: message.transport.clone(),
        status: message.status.clone(),
        handling_disposition: message.handling_disposition.clone(),
        message_kind: message.message_kind.clone(),
        source_surface: Some(envelope.source_surface),
        reply_target: envelope.reply_target,
        conversation_id: envelope.conversation_id,
        thread_root_message_id: envelope.thread_root_message_id,
        requires_user_visible_reply: envelope.requires_user_visible_reply,
        mailbox_resolution_kind: mailbox_resolution_kind(&message.payload).map(str::to_string),
        reply_payload_type: message
            .payload
            .get("type")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        reply_text: reply.as_ref().map(|reply| reply.text.clone()),
        reply_correlation_id: reply
            .as_ref()
            .and_then(|reply| reply.correlation_id.clone()),
        reply_string_payload: match &message.payload {
            Value::String(text) => Some(text.clone()),
            _ => None,
        },
        created_at: message.created_at,
    }
}
