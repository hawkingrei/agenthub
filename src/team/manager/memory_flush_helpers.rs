use serde_json::Value;
use sqlx::Row;

use super::context_artifacts::ContextArtifactPointer;
use super::{
    MEMORY_FLUSH_MAX_EVENTS_DEFAULT, MEMORY_FLUSH_MAX_EVENTS_MAX, MEMORY_FLUSH_MAX_EXCERPT_CHARS,
    MEMORY_FLUSH_MAX_SUMMARY_CHARS, TeamManager, TeamMemoryFlushRequest, TeamMemoryFlushResult,
    TeamRunEventRecord,
};
use crate::agent::event_message_codec::decode_message_from_storage;
use agenthub_db::AgentEventDbRouter;
use agenthub_text::truncate_chars;

#[derive(Debug, Clone)]
pub(super) struct NormalizedMemoryFlushRequest {
    pub(super) member_id: String,
    pub(super) session_id: Option<String>,
    pub(super) trigger: String,
    pub(super) max_events: i64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct MemoryFlushFinalizeContext<'a> {
    pub(super) run_id: &'a str,
    pub(super) team_id: &'a str,
    pub(super) member_id: &'a str,
    pub(super) session_id: Option<&'a str>,
    pub(super) trigger: &'a str,
    pub(super) now: i64,
}

#[derive(Debug, Clone)]
pub(super) struct MemoryFlushEventRow {
    pub(super) id: i64,
    pub(super) stream: String,
    pub(super) message: Vec<u8>,
}

pub(super) fn normalize_memory_flush_request(
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

pub(super) fn safe_i64_len(len: usize) -> i64 {
    i64::try_from(len).unwrap_or(i64::MAX)
}

pub(super) fn build_context_artifact_pointer_payload(pointer: &ContextArtifactPointer) -> Value {
    serde_json::json!({
        "kind": pointer.artifact_kind.as_str(),
        "path": pointer.relative_path.as_str(),
        "size_bytes": pointer.artifact_size_bytes,
        "checksum": pointer.content_checksum.as_str(),
    })
}

pub(super) async fn load_memory_flush_team_id_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
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

pub(super) async fn load_memory_flush_checkpoint_event_id_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
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

pub(super) async fn load_memory_flush_event_rows(
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

pub(super) async fn upsert_memory_flush_checkpoint_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
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

pub(super) async fn finalize_memory_flush_failed_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
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

pub(super) async fn finalize_memory_flush_noop_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
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

pub(super) async fn resolve_memory_flush_session_id_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
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

pub(super) fn build_memory_flush_observation(row: &MemoryFlushEventRow) -> Value {
    let event_id = row.id;
    let stream = row.stream.as_str();
    let message = decode_message_from_storage(row.message.as_slice());
    if let Ok(message_json) = serde_json::from_str::<Value>(&message) {
        let redacted = super::redact_sensitive_json(&message_json);
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

pub(super) fn build_memory_flush_summary(observations: &[Value]) -> String {
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
