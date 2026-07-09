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
        builder.push_bind(&conversation.id);
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
