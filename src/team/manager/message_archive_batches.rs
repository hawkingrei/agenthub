use std::collections::HashMap;

use serde_json::Value;
use sqlx::Row;

use super::archive_scope::message_archive_scope_for_payload_db;
use super::{
    MessageArchiveScopeFallback, TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND, TeamManager,
    TeamMessageArchiveMigrationReport, parse_run_event_row, parse_team_actor_message_row,
    parse_team_conversation_message_row, team_actor_message_archive_document,
    team_conversation_message_archive_document, team_run_event_archive_document,
};
use crate::team::TeamConversationRecord;
use agenthub_message_archive::MessageArchiveStoreRef;

impl TeamManager {
    pub(super) async fn migrate_team_conversation_messages_to_archive(
        &self,
        archive: &MessageArchiveStoreRef,
        batch_limit: i64,
        max_conversation_message_id: i64,
        report: &mut TeamMessageArchiveMigrationReport,
    ) -> anyhow::Result<()> {
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
                last_id = sqlx::Row::get(&row, "id");
                let conversation = TeamConversationRecord {
                    id: sqlx::Row::get(&row, "conversation_id"),
                    team_id: sqlx::Row::get(&row, "team_id"),
                    task_id: sqlx::Row::get(&row, "task_id"),
                    mode: String::new(),
                    topic: None,
                    created_at: 0,
                    updated_at: 0,
                };
                let (mut message, body_moved) = parse_team_conversation_message_row(&row)?;
                if body_moved {
                    self.rehydrate_moved_conversation_payload(&mut message)
                        .await?;
                }
                documents.push(team_conversation_message_archive_document(
                    &conversation,
                    &message,
                ));
                report.team_conversation_messages += 1;
            }
            archive.append_documents(&documents).await?;
        }
        Ok(())
    }

    pub(super) async fn migrate_team_run_events_to_archive(
        &self,
        archive: &MessageArchiveStoreRef,
        batch_limit: i64,
        max_run_event_id: i64,
        run_scope_cache: &mut HashMap<String, MessageArchiveScopeFallback>,
        task_conversation_cache: &mut HashMap<(String, String), Option<String>>,
        report: &mut TeamMessageArchiveMigrationReport,
    ) -> anyhow::Result<()> {
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
                    r.group_id,
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
                last_id = sqlx::Row::get(&row, "id");
                let run_id: String = sqlx::Row::get(&row, "run_id");
                let team_id: String = sqlx::Row::get(&row, "team_id");
                let group_id: Option<String> = sqlx::Row::get(&row, "group_id");
                if !run_scope_cache.contains_key(&run_id) {
                    let input_json: String = sqlx::Row::get(&row, "input_json");
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
                    task_conversation_cache,
                )
                .await?;
                documents.push(team_run_event_archive_document(
                    &team_id,
                    &event,
                    &scope,
                    group_id.as_deref(),
                ));
                report.team_run_events += 1;
            }
            archive.append_documents(&documents).await?;
        }
        Ok(())
    }

    pub(super) async fn migrate_team_actor_messages_to_archive(
        &self,
        archive: &MessageArchiveStoreRef,
        batch_limit: i64,
        max_actor_message_id: i64,
        run_scope_cache: &mut HashMap<String, MessageArchiveScopeFallback>,
        task_conversation_cache: &mut HashMap<(String, String), Option<String>>,
        report: &mut TeamMessageArchiveMigrationReport,
    ) -> anyhow::Result<()> {
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
                    m.message_kind,
                    m.handling_disposition,
                    m.handled_by_actor_id,
                    m.handled_at,
                    m.group_id,
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
                last_id = sqlx::Row::get(&row, "id");
                let run_id: String = sqlx::Row::get(&row, "run_id");
                let team_id: String = sqlx::Row::get(&row, "team_id");
                if !run_scope_cache.contains_key(&run_id) {
                    let input_json: String = sqlx::Row::get(&row, "input_json");
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
                let group_id: Option<String> = row.try_get("group_id")?;
                let scope = message_archive_scope_for_payload_db(
                    &self.db,
                    &team_id,
                    &message.payload,
                    &base_scope,
                    task_conversation_cache,
                )
                .await?;
                documents.push(team_actor_message_archive_document(
                    &team_id, &message, &scope, group_id,
                ));
                report.team_actor_messages += 1;
            }
            archive.append_documents(&documents).await?;
        }
        Ok(())
    }
}
