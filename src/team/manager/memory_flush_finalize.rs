use serde_json::Value;

use super::context_artifact_persistence::ContextArtifactPointer;
use super::{TeamManager, TeamMemoryFlushResult, TeamRunEventRecord};

#[derive(Debug, Clone, Copy)]
pub(super) struct MemoryFlushFinalizeContext<'a> {
    pub(super) run_id: &'a str,
    pub(super) team_id: &'a str,
    pub(super) member_id: &'a str,
    pub(super) session_id: Option<&'a str>,
    pub(super) trigger: &'a str,
    pub(super) now: i64,
}

pub(super) fn build_context_artifact_pointer_payload(pointer: &ContextArtifactPointer) -> Value {
    serde_json::json!({
        "kind": pointer.artifact_kind.as_str(),
        "path": pointer.relative_path.as_str(),
        "size_bytes": pointer.artifact_size_bytes,
        "checksum": pointer.content_checksum.as_str(),
    })
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
