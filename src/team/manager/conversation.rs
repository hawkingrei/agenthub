use std::collections::HashMap;

use agenthub_message_store::keys;
use sqlx::{QueryBuilder, Sqlite};

use super::TEAM_CHANNEL_BOOTSTRAP_KIND;
use super::TeamManager;
use super::codec_rows::{parse_team_conversation_message_row, parse_team_conversation_row};
use crate::team::{TeamConversationMessageRecord, TeamConversationRecord};

impl TeamManager {
    pub async fn get_task_conversation(
        &self,
        task_id: &str,
    ) -> anyhow::Result<TeamConversationRecord> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                team_id,
                task_id,
                mode,
                topic,
                created_at,
                updated_at
            FROM team_conversations
            WHERE task_id = ?1
            "#,
        )
        .bind(task_id)
        .fetch_one(&self.db)
        .await?;
        parse_team_conversation_row(&row)
    }

    pub async fn list_task_conversation_messages(
        &self,
        task_id: &str,
        limit: i64,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<TeamConversationMessageRecord>> {
        let conversation = self.get_task_conversation(task_id).await?;
        match self
            .try_list_task_conversation_messages_from_index(&conversation, limit, before_id)
            .await
        {
            Ok(Some(messages)) => return Ok(messages),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    ?error,
                    task_id,
                    conversation_id = %conversation.id,
                    "falling back to SQLite after message index read failed"
                );
            }
        }
        self.list_task_conversation_messages_from_sqlite(&conversation.id, limit, before_id)
            .await
    }

    async fn list_task_conversation_messages_from_sqlite(
        &self,
        conversation_id: &str,
        limit: i64,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<TeamConversationMessageRecord>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                id,
                conversation_id,
                task_id,
                group_id,
                from_actor_id,
                to_actor_id,
                route,
                payload_json,
                created_at
            FROM team_conversation_messages
            WHERE conversation_id = "#,
        );
        builder.push_bind(conversation_id);
        if let Some(before_id) = before_id {
            builder.push(" AND id < ");
            builder.push_bind(before_id);
        }
        builder.push(" ORDER BY id DESC LIMIT ");
        builder.push_bind(limit.max(1));

        let rows = builder.build().fetch_all(&self.db).await?;
        let mut messages = Vec::with_capacity(rows.len());
        for row in rows {
            let (mut message, body_moved) = parse_team_conversation_message_row(&row)?;
            if body_moved {
                self.rehydrate_moved_conversation_payload(&mut message)
                    .await?;
            }
            messages.push(message);
        }
        messages.reverse();
        Ok(messages)
    }

    async fn try_list_task_conversation_messages_from_index(
        &self,
        conversation: &TeamConversationRecord,
        limit: i64,
        before_id: Option<i64>,
    ) -> anyhow::Result<Option<Vec<TeamConversationMessageRecord>>> {
        let Some(index) = self.message_index.as_deref() else {
            return Ok(None);
        };

        let authority_max = self
            .max_conversation_message_id_for_page(&conversation.id, before_id)
            .await?;
        if authority_max == 0 {
            return Ok(Some(Vec::new()));
        }
        let freshness = agenthub_message_store::check_index_freshness(
            index,
            "team_conversation_messages",
            authority_max as u64,
        )?;
        if !freshness.is_fresh() {
            self.schedule_index_read_repair(
                "team_conversation_messages",
                authority_max as u64,
                freshness,
            );
            return Ok(None);
        }

        let group_id = self
            .task_index_group_id(&conversation.task_id, &conversation.team_id)
            .await?;
        let prefix = keys::channel_prefix(&group_id, &conversation.id);
        let refs = index.scan_prefix(&prefix)?;
        let mut ids = Vec::new();
        for message_ref in refs {
            let Some(message_id) = conversation_message_id_from_delivery_id(
                &conversation.id,
                message_ref.message_id.as_str(),
            ) else {
                self.schedule_incomplete_index_repair(
                    "team_conversation_messages",
                    authority_max as u64,
                    authority_max as u64,
                );
                return Ok(None);
            };
            if before_id.is_some_and(|before_id| message_id >= before_id) {
                continue;
            }
            ids.push(message_id);
        }
        let limit = limit.max(1) as usize;
        if ids.len() > limit {
            ids = ids.split_off(ids.len() - limit);
        }
        if ids
            != self
                .expected_conversation_message_ids(&conversation.id, limit, before_id)
                .await?
        {
            self.schedule_incomplete_index_repair(
                "team_conversation_messages",
                authority_max as u64,
                authority_max as u64,
            );
            return Ok(None);
        }
        self.load_conversation_messages_by_ids(&conversation.id, &ids)
            .await
            .map(Some)
    }

    async fn expected_conversation_message_ids(
        &self,
        conversation_id: &str,
        limit: usize,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<i64>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT id FROM team_conversation_messages WHERE conversation_id = ",
        );
        builder.push_bind(conversation_id);
        if let Some(before_id) = before_id {
            builder.push(" AND id < ");
            builder.push_bind(before_id);
        }
        builder.push(" ORDER BY id DESC LIMIT ");
        builder.push_bind(limit as i64);
        let mut ids = builder.build_query_scalar().fetch_all(&self.db).await?;
        ids.reverse();
        Ok(ids)
    }

    async fn max_conversation_message_id_for_page(
        &self,
        conversation_id: &str,
        before_id: Option<i64>,
    ) -> anyhow::Result<i64> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT COALESCE(MAX(id), 0) FROM team_conversation_messages WHERE conversation_id = ",
        );
        builder.push_bind(conversation_id);
        if let Some(before_id) = before_id {
            builder.push(" AND id < ");
            builder.push_bind(before_id);
        }
        Ok(builder.build_query_scalar().fetch_one(&self.db).await?)
    }

    async fn task_index_group_id(&self, task_id: &str, team_id: &str) -> anyhow::Result<String> {
        let group_id = sqlx::query_scalar::<_, Option<String>>(
            "SELECT group_id FROM team_tasks WHERE id = ?1",
        )
        .bind(task_id)
        .fetch_optional(&self.db)
        .await?
        .flatten()
        .unwrap_or_else(|| team_id.to_string());
        Ok(group_id)
    }

    async fn load_conversation_messages_by_ids(
        &self,
        conversation_id: &str,
        ids: &[i64],
    ) -> anyhow::Result<Vec<TeamConversationMessageRecord>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                id,
                conversation_id,
                task_id,
                group_id,
                from_actor_id,
                to_actor_id,
                route,
                payload_json,
                created_at
            FROM team_conversation_messages
            WHERE conversation_id = "#,
        );
        builder.push_bind(conversation_id);
        builder.push(" AND id IN (");
        let mut separated = builder.separated(", ");
        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");

        let rows = builder.build().fetch_all(&self.db).await?;
        if rows.len() != ids.len() {
            anyhow::bail!(
                "message index referenced {} rows for conversation {}, but SQLite hydrated {}",
                ids.len(),
                conversation_id,
                rows.len()
            );
        }

        let mut by_id = HashMap::with_capacity(rows.len());
        for row in rows {
            let (mut message, body_moved) = parse_team_conversation_message_row(&row)?;
            if body_moved {
                self.rehydrate_moved_conversation_payload(&mut message)
                    .await?;
            }
            by_id.insert(message.message_id, message);
        }

        let mut messages = Vec::with_capacity(ids.len());
        for id in ids {
            let Some(message) = by_id.remove(id) else {
                anyhow::bail!("message index referenced row {id} that SQLite did not return");
            };
            messages.push(message);
        }
        Ok(messages)
    }

    pub async fn get_channel_conversation_message(
        &self,
        team_id: &str,
        channel_id: &str,
        message_id: i64,
    ) -> anyhow::Result<TeamConversationMessageRecord> {
        let row = sqlx::query(
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
                m.created_at
            FROM team_conversation_messages AS m
            INNER JOIN team_tasks AS t ON t.id = m.task_id
            WHERE m.id = ?1
              AND t.team_id = ?2
              AND lower(trim(COALESCE(json_extract(t.context_json, '$.bootstrap_kind'), ''))) = ?3
              AND lower(trim(COALESCE(json_extract(t.context_json, '$.channel_id'), ''))) = ?4
            "#,
        )
        .bind(message_id)
        .bind(team_id)
        .bind(TEAM_CHANNEL_BOOTSTRAP_KIND)
        .bind(channel_id.trim().to_lowercase())
        .fetch_one(&self.db)
        .await?;
        let (mut message, body_moved) = parse_team_conversation_message_row(&row)?;
        if body_moved {
            self.rehydrate_moved_conversation_payload(&mut message)
                .await?;
        }
        Ok(message)
    }
}

fn conversation_message_id_from_delivery_id(
    conversation_id: &str,
    delivery_id: &str,
) -> Option<i64> {
    delivery_id
        .strip_prefix(&format!("team_conversation_message:{conversation_id}:"))?
        .parse()
        .ok()
}
