use std::collections::HashMap;

use serde_json::Value;
use sqlx::{Row, SqlitePool};

use super::TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND;
use super::archive_documents::{
    AgentEventArchiveRow, AgentEventArchiveScope, MessageArchiveScopeFallback,
};
use super::archive_scope::message_archive_scope_for_payload_db;

pub(super) type AgentEventScopeCache =
    HashMap<(String, String, i64), Option<AgentEventArchiveScope>>;
pub(super) type TaskConversationCache = HashMap<(String, String), Option<String>>;

pub(super) async fn agent_event_archive_scope_for_row(
    db: &SqlitePool,
    row: &AgentEventArchiveRow,
    scope_cache: &mut AgentEventScopeCache,
    task_conversation_cache: &mut TaskConversationCache,
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
    task_conversation_cache: &mut TaskConversationCache,
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
