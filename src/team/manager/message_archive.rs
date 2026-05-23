use std::collections::HashMap;

use anyhow::Context;
use serde_json::Value;
use sqlx::Row;

use super::archive_documents::AgentEventArchiveMigrationCounts;
use super::archive_migration::{
    migrate_main_agent_events_to_archive, migrate_per_agent_events_to_archive,
};
use super::{
    AgentEventArchiveSnapshot, MessageArchiveScopeFallback,
    TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND, TeamManager, TeamMessageArchiveMigrationReport,
    message_archive_scope_for_payload_db, parse_run_event_row, parse_team_actor_message_row,
    parse_team_conversation_message_row, team_actor_message_archive_document,
    team_conversation_message_archive_document, team_run_event_archive_document,
};
use crate::team::TeamConversationRecord;
use agenthub_message_archive::{MessageArchiveStoreRef, MessageSearchHit, MessageSearchQuery};

impl TeamManager {
    pub async fn search_message_archive(
        &self,
        query: &MessageSearchQuery,
    ) -> anyhow::Result<Vec<MessageSearchHit>> {
        let Some(archive) = self.message_archive.as_ref() else {
            return Ok(Vec::new());
        };
        archive.search(query).await
    }

    async fn message_archive_table_max_id(&self, table_name: &str) -> anyhow::Result<i64> {
        let sql = match table_name {
            "team_conversation_messages" => {
                "SELECT COALESCE(MAX(id), 0) FROM team_conversation_messages"
            }
            "team_run_events" => "SELECT COALESCE(MAX(id), 0) FROM team_run_events",
            "team_actor_messages" => "SELECT COALESCE(MAX(id), 0) FROM team_actor_messages",
            _ => anyhow::bail!("unsupported archive migration table: {table_name}"),
        };
        Ok(sqlx::query_scalar::<_, i64>(sql)
            .fetch_one(&self.db)
            .await?)
    }

    async fn migrate_agent_events_to_archive(
        &self,
        archive: &MessageArchiveStoreRef,
        batch_size: usize,
        snapshot: &AgentEventArchiveSnapshot,
    ) -> anyhow::Result<AgentEventArchiveMigrationCounts> {
        let mut counts = AgentEventArchiveMigrationCounts::default();
        counts.add(
            migrate_main_agent_events_to_archive(
                &self.db,
                archive,
                batch_size,
                snapshot.main_max_id,
            )
            .await
            .context("migrate main agent_events to message archive")?,
        );

        for (agent_id, max_id) in &snapshot.per_agent_max_ids {
            let pool = self.event_dbs.pool_for_agent(agent_id).await?;
            counts.add(
                migrate_per_agent_events_to_archive(
                    &self.db, &pool, archive, agent_id, batch_size, *max_id,
                )
                .await
                .with_context(|| {
                    format!("migrate per-agent agent_events for agent_id={agent_id}")
                })?,
            );
        }
        Ok(counts)
    }

    async fn message_archive_agent_event_snapshot(
        &self,
    ) -> anyhow::Result<AgentEventArchiveSnapshot> {
        let main_max_id =
            sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(id), 0) FROM agent_events")
                .fetch_one(&self.db)
                .await?;
        let agent_ids = sqlx::query_scalar::<_, String>("SELECT id FROM agents ORDER BY id ASC")
            .fetch_all(&self.db)
            .await?;
        let mut per_agent_max_ids = Vec::new();
        for agent_id in agent_ids {
            let db_path = self.event_dbs.db_path_for_agent(&agent_id);
            if !tokio::fs::try_exists(&db_path).await? {
                continue;
            }
            let pool = self.event_dbs.pool_for_agent(&agent_id).await?;
            let max_id =
                sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(id), 0) FROM agent_events")
                    .fetch_one(&pool)
                    .await?;
            per_agent_max_ids.push((agent_id, max_id));
        }
        Ok(AgentEventArchiveSnapshot {
            main_max_id,
            per_agent_max_ids,
        })
    }

    pub async fn migrate_team_messages_to_archive(
        &self,
        batch_size: usize,
    ) -> anyhow::Result<TeamMessageArchiveMigrationReport> {
        let Some(archive) = self.message_archive.as_ref() else {
            anyhow::bail!("message archive is not configured");
        };
        let mut report = TeamMessageArchiveMigrationReport::default();
        let batch_size = batch_size.clamp(1, 1000);
        let batch_limit = i64::try_from(batch_size)?;
        let max_conversation_message_id = self
            .message_archive_table_max_id("team_conversation_messages")
            .await?;
        let max_run_event_id = self.message_archive_table_max_id("team_run_events").await?;
        let max_actor_message_id = self
            .message_archive_table_max_id("team_actor_messages")
            .await?;
        let agent_event_snapshot = self.message_archive_agent_event_snapshot().await?;
        let mut run_scope_cache: HashMap<String, MessageArchiveScopeFallback> = HashMap::new();
        let mut task_conversation_cache: HashMap<(String, String), Option<String>> = HashMap::new();

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
                let message = parse_team_conversation_message_row(&row)?;
                documents.push(team_conversation_message_archive_document(
                    &conversation,
                    &message,
                ));
                report.team_conversation_messages += 1;
            }
            archive.append_documents(&documents).await?;
        }

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
                    &mut task_conversation_cache,
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
                    &mut task_conversation_cache,
                )
                .await?;
                documents.push(team_actor_message_archive_document(
                    &team_id, &message, &scope, group_id,
                ));
                report.team_actor_messages += 1;
            }
            archive.append_documents(&documents).await?;
        }

        let agent_event_counts = self
            .migrate_agent_events_to_archive(archive, batch_size, &agent_event_snapshot)
            .await?;
        report.agent_events = agent_event_counts.agent_events;
        report.aggregated_acp_messages = agent_event_counts.aggregated_acp_messages;
        Ok(report)
    }
}
