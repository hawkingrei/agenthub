#![cfg_attr(not(test), allow(dead_code))]

use agenthub_message_store::{
    AuthorityIndexProjection, AuthorityMessageId, DeliveryMessageId, IndexReadRepairScheduler,
    IndexRepairReport, MessageIndexStore, MessageKind, MessageRef, keys,
    mark_index_repaired_through, repair_index_from_authority,
};
use serde_json::Value;
use sqlx::Row;

use super::archive_documents::MessageArchiveScopeFallback;
use super::archive_migration_scope::{
    AgentEventScopeCache, TaskConversationCache, agent_event_archive_scope_for_row,
};
use super::archive_payloads::message_archive_payload_string;
use super::{TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND, TeamManager};
use crate::agent::event_message_codec::decode_message_from_storage;

const TEAM_CONVERSATION_MESSAGE_SOURCE: &str = "team_conversation_messages";
const TEAM_ACTOR_MESSAGE_SOURCE: &str = "team_actor_messages";
const TEAM_RUN_EVENT_SOURCE: &str = "team_run_events";
const AGENT_EVENT_SOURCE: &str = "agent_events";
const PER_AGENT_EVENT_SOURCE: &str = "per_agent_agent_events";
const MAIN_AGENT_EVENT_INDEX_STREAM: &str = "agent_events:main";

/// Parses a stored JSON column during index repair, warning (with enough context to locate the row)
/// and indexing as an empty payload instead of aborting the whole repair pass on one corrupt row.
fn parse_projection_json(source: &str, message_id: i64, field: &str, raw: &str) -> Value {
    serde_json::from_str::<Value>(raw).unwrap_or_else(|err| {
        tracing::warn!(
            source,
            message_id,
            field,
            "message index repair: failed to parse {} as JSON, indexing as empty payload: {}",
            field,
            err
        );
        Value::Null
    })
}

fn classify_message_kind(payload: &Value) -> MessageKind {
    match payload
        .get("kind")
        .or_else(|| payload.get("type"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "tool_call" | "tool-call" | "tool" => MessageKind::ToolCall,
        "event" => MessageKind::Event,
        "thought" | "reasoning" => MessageKind::Thought,
        "system" | "system_notice" | "system-notice" => MessageKind::System,
        _ => MessageKind::Text,
    }
}

fn conversation_authority_id(message_id: i64) -> AuthorityMessageId {
    AuthorityMessageId::new(format!("tcm:{message_id}"))
}

fn conversation_delivery_id(conversation_id: &str, message_id: i64) -> DeliveryMessageId {
    DeliveryMessageId::new(format!(
        "team_conversation_message:{conversation_id}:{message_id}"
    ))
}

fn conversation_archive_document_id(conversation_id: &str, message_id: i64) -> String {
    format!("team_conversation_message:{conversation_id}:{message_id}")
}

fn actor_authority_id(message_id: i64) -> AuthorityMessageId {
    AuthorityMessageId::new(format!("tam:{message_id}"))
}

fn actor_delivery_id(run_id: &str, message_id: i64) -> DeliveryMessageId {
    DeliveryMessageId::new(format!("team_actor_message:{run_id}:{message_id}"))
}

fn actor_archive_document_id(run_id: &str, message_id: i64) -> String {
    format!("team_actor_message:{run_id}:{message_id}")
}

fn actor_inbox_id(peer_id: &str, actor_id: &str) -> String {
    format!("{peer_id}:{actor_id}")
}

fn actor_message_kind(message_kind: &str) -> MessageKind {
    match message_kind.trim() {
        "trigger_event" | "task_signal" => MessageKind::Event,
        "thread_reply" | "human_request" | "coordination_request" => MessageKind::Text,
        "system_notice" => MessageKind::System,
        _ => MessageKind::Text,
    }
}

fn run_event_authority_id(run_id: &str, event_id: i64) -> AuthorityMessageId {
    AuthorityMessageId::new(format!("tre:{run_id}:{event_id}"))
}

fn run_event_delivery_id(run_id: &str, event_id: i64) -> DeliveryMessageId {
    DeliveryMessageId::new(format!("team_run_event:{run_id}:{event_id}"))
}

fn run_event_archive_document_id(run_id: &str, event_id: i64) -> String {
    format!("team_run_event:{run_id}:{event_id}")
}

fn run_event_kind(event_type: &str, payload: &Value) -> MessageKind {
    let normalized = event_type.trim().to_ascii_lowercase();
    if normalized.contains("thought") || normalized.contains("reasoning") {
        return MessageKind::Thought;
    }
    if normalized.contains("tool") {
        return MessageKind::ToolCall;
    }
    match payload
        .get("type")
        .or_else(|| payload.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "thought" | "reasoning" => MessageKind::Thought,
        "tool_call" | "tool-call" | "tool" => MessageKind::ToolCall,
        "system" | "system_notice" | "system-notice" => MessageKind::System,
        _ => MessageKind::Event,
    }
}

fn agent_event_authority_id(agent_id: &str, session_id: &str, event_id: i64) -> AuthorityMessageId {
    AuthorityMessageId::new(format!("ae:{agent_id}:{session_id}:{event_id}"))
}

fn agent_event_delivery_id(agent_id: &str, session_id: &str, event_id: i64) -> DeliveryMessageId {
    DeliveryMessageId::new(format!("agent_event:{agent_id}:{session_id}:{event_id}"))
}

fn agent_event_archive_document_id(agent_id: &str, session_id: &str, event_id: i64) -> String {
    format!("agent_event:{agent_id}:{session_id}:{event_id}")
}

fn per_agent_event_index_stream(agent_id: &str) -> String {
    format!("agent_events:agent:{agent_id}")
}

fn agent_event_kind(stream: &str, parsed_message: Option<&Value>) -> MessageKind {
    let normalized_stream = stream.trim().to_ascii_lowercase();
    if normalized_stream == "stderr" || normalized_stream == "system" {
        return MessageKind::System;
    }
    if let Some(payload) = parsed_message {
        return classify_message_kind(payload);
    }
    MessageKind::Text
}

impl TeamManager {
    /// Rebuild queued SQLite-derived index streams after guarded reads fall back to authority.
    ///
    /// The scheduler is intentionally drained before rebuilding. A failed request is put back so
    /// newer guarded reads can raise its target high-water while the worker is retrying later.
    pub(crate) async fn drain_message_index_read_repairs(
        &self,
        index: &dyn MessageIndexStore,
        scheduler: &dyn IndexReadRepairScheduler,
        batch_limit: i64,
    ) -> anyhow::Result<usize> {
        let requests = scheduler.take_pending_repairs()?;
        let mut repaired_streams = 0;

        for request in requests {
            let repair_result = match request.stream_id.as_str() {
                TEAM_CONVERSATION_MESSAGE_SOURCE => {
                    self.repair_team_conversation_message_index(
                        index,
                        batch_limit,
                        request.authority_max as i64,
                    )
                    .await
                }
                TEAM_ACTOR_MESSAGE_SOURCE => {
                    self.repair_team_actor_message_index(
                        index,
                        batch_limit,
                        request.authority_max as i64,
                    )
                    .await
                }
                TEAM_RUN_EVENT_SOURCE => {
                    self.repair_team_run_event_index(
                        index,
                        batch_limit,
                        request.authority_max as i64,
                    )
                    .await
                }
                MAIN_AGENT_EVENT_INDEX_STREAM => {
                    self.repair_main_agent_event_index(
                        index,
                        batch_limit,
                        request.authority_max as i64,
                    )
                    .await
                }
                stream_id => match stream_id.strip_prefix("agent_events:agent:") {
                    Some(agent_id) if !agent_id.is_empty() => {
                        self.repair_per_agent_event_index(
                            index,
                            agent_id,
                            batch_limit,
                            request.authority_max as i64,
                        )
                        .await
                    }
                    _ => Err(anyhow::anyhow!(
                        "unsupported message index repair stream `{stream_id}`"
                    )),
                },
            };

            match repair_result {
                Ok(_) => repaired_streams += 1,
                Err(error) => {
                    tracing::warn!(
                        ?error,
                        stream_id = %request.stream_id,
                        authority_max = request.authority_max,
                        "message index read repair failed; scheduling retry"
                    );
                    if let Err(schedule_error) = scheduler.schedule_read_repair(request) {
                        tracing::warn!(
                            ?schedule_error,
                            "failed to reschedule message index read repair"
                        );
                    }
                }
            }
        }

        Ok(repaired_streams)
    }

    /// Rebuild the body-free delivery index for `team_conversation_messages` from SQLite authority.
    ///
    /// This is a projection repair primitive only. Ordered reads remain guarded and fall back to
    /// SQLite whenever the index is missing, lagging, or incomplete.
    pub(crate) async fn repair_team_conversation_message_index(
        &self,
        index: &dyn MessageIndexStore,
        batch_limit: i64,
        max_message_id: i64,
    ) -> anyhow::Result<IndexRepairReport> {
        let mut last_id = 0_i64;
        let mut report = IndexRepairReport::default();
        let batch_limit = batch_limit.max(1);

        loop {
            let rows = sqlx::query(
                r#"
                SELECT
                    m.id,
                    m.conversation_id,
                    m.group_id,
                    m.from_actor_id,
                    m.to_actor_id,
                    m.route,
                    m.correlation_id,
                    m.payload_json,
                    m.created_at,
                    c.team_id
                FROM team_conversation_messages AS m
                INNER JOIN team_conversations AS c ON c.id = m.conversation_id
                WHERE m.id > ?1 AND m.id <= ?2
                ORDER BY m.id ASC
                LIMIT ?3
                "#,
            )
            .bind(last_id)
            .bind(max_message_id)
            .bind(batch_limit)
            .fetch_all(&self.db)
            .await?;

            if rows.is_empty() {
                break;
            }

            let mut projections = Vec::with_capacity(rows.len() * 2);
            for row in rows {
                let message_id: i64 = row.get("id");
                last_id = message_id;

                let conversation_id: String = row.get("conversation_id");
                let group_id: Option<String> = row.try_get("group_id").ok().flatten();
                let team_id: String = row.get("team_id");
                let payload_json: String = row.get("payload_json");
                let payload = parse_projection_json(
                    TEAM_CONVERSATION_MESSAGE_SOURCE,
                    message_id,
                    "payload_json",
                    &payload_json,
                );
                let correlation_id: String = row.try_get("correlation_id").unwrap_or_default();
                let normalized_correlation_id =
                    (!correlation_id.trim().is_empty()).then_some(correlation_id);
                let index_group_id = group_id.clone().unwrap_or_else(|| team_id.clone());
                let message_ref = MessageRef {
                    message_id: conversation_delivery_id(&conversation_id, message_id),
                    authority_message_id: conversation_authority_id(message_id),
                    archive_document_id: Some(conversation_archive_document_id(
                        &conversation_id,
                        message_id,
                    )),
                    created_at: row.get("created_at"),
                    source_kind: TEAM_CONVERSATION_MESSAGE_SOURCE.to_string(),
                    message_kind: classify_message_kind(&payload),
                    correlation_id: normalized_correlation_id,
                    group_id: Some(index_group_id.clone()),
                    run_id: None,
                    conversation_id: Some(conversation_id.clone()),
                    agent_id: row.try_get("to_actor_id").ok().flatten(),
                };

                projections.push(AuthorityIndexProjection {
                    key: keys::channel_key(&index_group_id, &conversation_id, message_id as u64),
                    message_ref: message_ref.clone(),
                });
                projections.push(AuthorityIndexProjection {
                    key: keys::message_id_key(message_ref.message_id.as_str()),
                    message_ref,
                });
            }

            let batch_report = repair_index_from_authority(index, projections)?;
            report.repaired_refs += batch_report.repaired_refs;
        }

        mark_index_repaired_through(
            index,
            TEAM_CONVERSATION_MESSAGE_SOURCE,
            max_message_id.max(0) as u64,
        )?;
        Ok(report)
    }

    /// Rebuild delivery refs for `team_actor_messages` from SQLite authority.
    ///
    /// This writes the run, recipient-agent, inbox, and id lookup projections. It does not read or
    /// validate `cf_body`; body-loss detection belongs to the later integrity-check pass.
    pub(crate) async fn repair_team_actor_message_index(
        &self,
        index: &dyn MessageIndexStore,
        batch_limit: i64,
        max_message_id: i64,
    ) -> anyhow::Result<IndexRepairReport> {
        let mut last_id = 0_i64;
        let mut report = IndexRepairReport::default();
        let batch_limit = batch_limit.max(1);

        loop {
            let rows = sqlx::query(
                r#"
                SELECT
                    m.id,
                    m.run_id,
                    m.to_actor_id,
                    m.to_peer_id,
                    m.payload_json,
                    m.message_kind,
                    m.group_id,
                    m.created_at,
                    r.team_id
                FROM team_actor_messages AS m
                INNER JOIN team_runs AS r ON r.id = m.run_id
                WHERE m.id > ?1 AND m.id <= ?2
                ORDER BY m.id ASC
                LIMIT ?3
                "#,
            )
            .bind(last_id)
            .bind(max_message_id)
            .bind(batch_limit)
            .fetch_all(&self.db)
            .await?;

            if rows.is_empty() {
                break;
            }

            let mut projections = Vec::with_capacity(rows.len() * 4);
            for row in rows {
                let message_id: i64 = row.get("id");
                last_id = message_id;

                let run_id: String = row.get("run_id");
                let to_actor_id: String = row.get("to_actor_id");
                let to_peer_id: String = row.get("to_peer_id");
                let payload_json: String = row.get("payload_json");
                let payload = parse_projection_json(
                    TEAM_ACTOR_MESSAGE_SOURCE,
                    message_id,
                    "payload_json",
                    &payload_json,
                );
                let group_id: Option<String> = row.try_get("group_id").ok().flatten();
                let team_id: String = row.get("team_id");
                let index_group_id = group_id.clone().unwrap_or(team_id);
                let raw_message_kind: String = row.get("message_kind");
                let message_ref = MessageRef {
                    message_id: actor_delivery_id(&run_id, message_id),
                    authority_message_id: actor_authority_id(message_id),
                    archive_document_id: Some(actor_archive_document_id(&run_id, message_id)),
                    created_at: row.get("created_at"),
                    source_kind: TEAM_ACTOR_MESSAGE_SOURCE.to_string(),
                    message_kind: actor_message_kind(&raw_message_kind),
                    correlation_id: payload
                        .get("correlation_id")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    group_id: Some(index_group_id),
                    run_id: Some(run_id.clone()),
                    conversation_id: payload
                        .get("conversation_id")
                        .or_else(|| payload.get("task_conversation_id"))
                        .or_else(|| payload.get("channel_conversation_id"))
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    agent_id: Some(to_actor_id.clone()),
                };

                projections.push(AuthorityIndexProjection {
                    key: keys::run_key(&run_id, message_id as u64),
                    message_ref: message_ref.clone(),
                });
                projections.push(AuthorityIndexProjection {
                    key: keys::agent_key(&to_actor_id, message_id as u64),
                    message_ref: message_ref.clone(),
                });
                projections.push(AuthorityIndexProjection {
                    key: keys::inbox_key(
                        &actor_inbox_id(&to_peer_id, &to_actor_id),
                        message_id as u64,
                    ),
                    message_ref: message_ref.clone(),
                });
                projections.push(AuthorityIndexProjection {
                    key: keys::message_id_key(message_ref.message_id.as_str()),
                    message_ref,
                });
            }

            let batch_report = repair_index_from_authority(index, projections)?;
            report.repaired_refs += batch_report.repaired_refs;
        }

        mark_index_repaired_through(
            index,
            TEAM_ACTOR_MESSAGE_SOURCE,
            max_message_id.max(0) as u64,
        )?;
        Ok(report)
    }

    /// Rebuild delivery refs for `team_run_events` from SQLite authority.
    ///
    /// Run events project to the run timeline and id lookup keyspace. Shared-thread mailbox bootstrap
    /// runs are excluded to match archive migration scope and avoid duplicating internal mailbox
    /// transport rows in the ordinary run projection.
    pub(crate) async fn repair_team_run_event_index(
        &self,
        index: &dyn MessageIndexStore,
        batch_limit: i64,
        max_event_id: i64,
    ) -> anyhow::Result<IndexRepairReport> {
        let mut last_id = 0_i64;
        let mut report = IndexRepairReport::default();
        let batch_limit = batch_limit.max(1);

        loop {
            let rows = sqlx::query(
                r#"
                SELECT
                    e.id,
                    e.run_id,
                    e.event_type,
                    e.ts,
                    e.payload_json,
                    r.team_id,
                    r.group_id,
                    r.input_json
                FROM team_run_events AS e
                INNER JOIN team_runs AS r ON r.id = e.run_id
                WHERE e.id > ?1
                  AND e.id <= ?2
                  AND trim(COALESCE(json_extract(r.input_json, '$.bootstrap_kind'), '')) != ?3
                ORDER BY e.id ASC
                LIMIT ?4
                "#,
            )
            .bind(last_id)
            .bind(max_event_id)
            .bind(TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND)
            .bind(batch_limit)
            .fetch_all(&self.db)
            .await?;

            if rows.is_empty() {
                break;
            }

            let mut projections = Vec::with_capacity(rows.len() * 2);
            for row in rows {
                let event_id: i64 = row.get("id");
                last_id = event_id;

                let run_id: String = row.get("run_id");
                let event_type: String = row.get("event_type");
                let payload_json: String = row.get("payload_json");
                let payload = parse_projection_json(
                    TEAM_RUN_EVENT_SOURCE,
                    event_id,
                    "payload_json",
                    &payload_json,
                );
                let input_json: String = row.get("input_json");
                let run_input = parse_projection_json(
                    TEAM_RUN_EVENT_SOURCE,
                    event_id,
                    "input_json",
                    &input_json,
                );
                let base_scope = MessageArchiveScopeFallback::from_run_input(&run_input);
                let group_id: Option<String> = row.try_get("group_id").ok().flatten();
                let team_id: String = row.get("team_id");
                let index_group_id = group_id.clone().unwrap_or(team_id);
                let conversation_id = message_archive_payload_string(&payload, "conversation_id")
                    .or_else(|| base_scope.conversation_id.clone());
                let message_ref = MessageRef {
                    message_id: run_event_delivery_id(&run_id, event_id),
                    authority_message_id: run_event_authority_id(&run_id, event_id),
                    archive_document_id: Some(run_event_archive_document_id(&run_id, event_id)),
                    created_at: row.get("ts"),
                    source_kind: TEAM_RUN_EVENT_SOURCE.to_string(),
                    message_kind: run_event_kind(&event_type, &payload),
                    correlation_id: message_archive_payload_string(&payload, "correlation_id"),
                    group_id: Some(index_group_id),
                    run_id: Some(run_id.clone()),
                    conversation_id,
                    agent_id: message_archive_payload_string(&payload, "agent_id"),
                };

                projections.push(AuthorityIndexProjection {
                    key: keys::run_key(&run_id, event_id as u64),
                    message_ref: message_ref.clone(),
                });
                projections.push(AuthorityIndexProjection {
                    key: keys::message_id_key(message_ref.message_id.as_str()),
                    message_ref,
                });
            }

            let batch_report = repair_index_from_authority(index, projections)?;
            report.repaired_refs += batch_report.repaired_refs;
        }

        mark_index_repaired_through(index, TEAM_RUN_EVENT_SOURCE, max_event_id.max(0) as u64)?;
        Ok(report)
    }

    /// Rebuild delivery refs for the main `agent_events` authority table.
    pub(crate) async fn repair_main_agent_event_index(
        &self,
        index: &dyn MessageIndexStore,
        batch_limit: i64,
        max_event_id: i64,
    ) -> anyhow::Result<IndexRepairReport> {
        repair_agent_event_index(
            &self.db,
            &self.db,
            index,
            batch_limit,
            max_event_id,
            AgentEventProjectionSource::Main,
        )
        .await
    }

    /// Rebuild delivery refs for one per-agent event database.
    pub(crate) async fn repair_per_agent_event_index(
        &self,
        index: &dyn MessageIndexStore,
        agent_id: &str,
        batch_limit: i64,
        max_event_id: i64,
    ) -> anyhow::Result<IndexRepairReport> {
        let event_db = self.event_dbs.pool_for_agent(agent_id).await?;
        repair_agent_event_index(
            &self.db,
            &event_db,
            index,
            batch_limit,
            max_event_id,
            AgentEventProjectionSource::PerAgent { agent_id },
        )
        .await
    }
}

#[derive(Clone, Copy)]
enum AgentEventProjectionSource<'a> {
    Main,
    PerAgent { agent_id: &'a str },
}

async fn repair_agent_event_index(
    main_db: &sqlx::SqlitePool,
    event_db: &sqlx::SqlitePool,
    index: &dyn MessageIndexStore,
    batch_limit: i64,
    max_event_id: i64,
    source: AgentEventProjectionSource<'_>,
) -> anyhow::Result<IndexRepairReport> {
    let mut last_id = 0_i64;
    let mut report = IndexRepairReport::default();
    let batch_limit = batch_limit.max(1);
    let mut scope_cache = AgentEventScopeCache::new();
    let mut task_conversation_cache = TaskConversationCache::new();

    loop {
        let rows =
            fetch_agent_event_projection_rows(event_db, source, last_id, max_event_id, batch_limit)
                .await?;
        if rows.is_empty() {
            break;
        }

        let mut projections = Vec::with_capacity(rows.len() * 3);
        for row in rows {
            last_id = row.event_id;
            let scope = agent_event_archive_scope_for_row(
                main_db,
                &row,
                &mut scope_cache,
                &mut task_conversation_cache,
            )
            .await?;
            let parsed_message = serde_json::from_str::<Value>(row.message.as_str()).ok();
            let message_ref = MessageRef {
                message_id: agent_event_delivery_id(&row.agent_id, &row.session_id, row.event_id),
                authority_message_id: agent_event_authority_id(
                    &row.agent_id,
                    &row.session_id,
                    row.event_id,
                ),
                archive_document_id: Some(agent_event_archive_document_id(
                    &row.agent_id,
                    &row.session_id,
                    row.event_id,
                )),
                created_at: row.ts,
                source_kind: agent_event_source_kind(source).to_string(),
                message_kind: agent_event_kind(&row.stream, parsed_message.as_ref()),
                correlation_id: parsed_message
                    .as_ref()
                    .and_then(|payload| message_archive_payload_string(payload, "correlation_id")),
                group_id: None,
                run_id: scope.as_ref().and_then(|scope| scope.run_id.clone()),
                conversation_id: scope
                    .as_ref()
                    .and_then(|scope| scope.conversation_id.clone()),
                agent_id: Some(row.agent_id.clone()),
            };

            projections.push(AuthorityIndexProjection {
                key: keys::agent_key(&row.agent_id, row.event_id as u64),
                message_ref: message_ref.clone(),
            });
            if let Some(run_id) = message_ref.run_id.as_deref() {
                projections.push(AuthorityIndexProjection {
                    key: keys::run_key(run_id, row.event_id as u64),
                    message_ref: message_ref.clone(),
                });
            }
            projections.push(AuthorityIndexProjection {
                key: keys::message_id_key(message_ref.message_id.as_str()),
                message_ref,
            });
        }

        let batch_report = repair_index_from_authority(index, projections)?;
        report.repaired_refs += batch_report.repaired_refs;
    }

    mark_index_repaired_through(
        index,
        &agent_event_index_stream(source),
        max_event_id.max(0) as u64,
    )?;
    Ok(report)
}

fn agent_event_index_stream(source: AgentEventProjectionSource<'_>) -> String {
    match source {
        AgentEventProjectionSource::Main => MAIN_AGENT_EVENT_INDEX_STREAM.to_string(),
        AgentEventProjectionSource::PerAgent { agent_id } => per_agent_event_index_stream(agent_id),
    }
}

fn agent_event_source_kind(source: AgentEventProjectionSource<'_>) -> &'static str {
    match source {
        AgentEventProjectionSource::Main => AGENT_EVENT_SOURCE,
        AgentEventProjectionSource::PerAgent { .. } => PER_AGENT_EVENT_SOURCE,
    }
}

async fn fetch_agent_event_projection_rows(
    db: &sqlx::SqlitePool,
    source: AgentEventProjectionSource<'_>,
    last_id: i64,
    max_event_id: i64,
    batch_limit: i64,
) -> anyhow::Result<Vec<super::archive_documents::AgentEventArchiveRow>> {
    let rows = match source {
        AgentEventProjectionSource::Main => sqlx::query(
            r#"
            SELECT id, agent_id, session_id, ts, stream, message
            FROM agent_events
            WHERE id > ?1 AND id <= ?2
            ORDER BY id ASC
            LIMIT ?3
            "#,
        )
        .bind(last_id)
        .bind(max_event_id)
        .bind(batch_limit)
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|row| super::archive_documents::AgentEventArchiveRow {
            event_id: row.get("id"),
            agent_id: row.get("agent_id"),
            session_id: row.get("session_id"),
            ts: row.get("ts"),
            stream: row.get("stream"),
            message: decode_message_from_storage(row.get::<Vec<u8>, _>("message").as_slice()),
        })
        .collect(),
        AgentEventProjectionSource::PerAgent { agent_id } => sqlx::query(
            r#"
            SELECT id, session_id, ts, stream, message
            FROM agent_events
            WHERE id > ?1 AND id <= ?2
            ORDER BY id ASC
            LIMIT ?3
            "#,
        )
        .bind(last_id)
        .bind(max_event_id)
        .bind(batch_limit)
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|row| super::archive_documents::AgentEventArchiveRow {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_projection_json_returns_parsed_value_for_valid_json() {
        let parsed = parse_projection_json(
            TEAM_ACTOR_MESSAGE_SOURCE,
            42,
            "payload_json",
            r#"{"text":"hello"}"#,
        );
        assert_eq!(parsed, serde_json::json!({"text": "hello"}));
    }

    #[test]
    fn parse_projection_json_indexes_as_null_instead_of_panicking_on_corrupt_json() {
        let parsed = parse_projection_json(
            TEAM_ACTOR_MESSAGE_SOURCE,
            42,
            "payload_json",
            "{not valid json",
        );
        assert_eq!(parsed, Value::Null);
    }

    #[test]
    fn classify_message_kind_uses_payload_hint() {
        assert_eq!(
            classify_message_kind(&serde_json::json!({"type": "tool_call"})),
            MessageKind::ToolCall
        );
        assert_eq!(
            classify_message_kind(&serde_json::json!({"kind": "system_notice"})),
            MessageKind::System
        );
        assert_eq!(
            classify_message_kind(&serde_json::json!({})),
            MessageKind::Text
        );
    }
}
