use std::collections::HashMap;

use serde_json::Value;
use sqlx::{Row, SqlitePool};

use super::archive_documents::{MessageArchiveScopeFallback, team_run_event_archive_document};
use super::archive_payloads::message_archive_payload_string;
use super::{TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND, TeamRunEventRecord};
use agenthub_message_archive::MessageDocument;

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
