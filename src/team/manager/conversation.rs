use sqlx::{QueryBuilder, Sqlite};

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
            messages.push(parse_team_conversation_message_row(&row)?);
        }
        messages.reverse();
        Ok(messages)
    }
}
