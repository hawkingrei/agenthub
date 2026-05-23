use std::collections::HashMap;

use serde_json::Value;
use sqlx::{Row, SqlitePool};

use super::TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND;
use super::archive_documents::{
    AgentEventArchiveMigrationCounts, AgentEventArchiveRow, AgentEventArchiveScope,
    MessageArchiveScopeFallback, agent_event_archive_document,
    aggregated_acp_message_archive_document, message_archive_scope_for_payload_db,
};
use crate::agent::event_message_codec::decode_message_from_storage;
use agenthub_message_archive::{
    AcpChunkAccumulator, AcpEventRow, MessageArchiveStoreRef, MessageDocument,
};

pub(super) async fn migrate_main_agent_events_to_archive(
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

pub(super) async fn migrate_per_agent_events_to_archive(
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
