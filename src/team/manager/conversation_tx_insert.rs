use chrono::Utc;
use sqlx::Sqlite;

use super::TeamManager;
use super::codec_rows::parse_team_conversation_row;
use super::conversation_idempotency::task_conversation_payload_correlation_id;
use super::conversation_insert_common::{
    TaskConversationInsertInput, insert_task_conversation_message_with_tx,
};
use crate::team::{TeamConversationMessageRecord, TeamConversationRecord, TeamTaskNoteCreateInput};

impl TeamManager {
    pub(super) async fn insert_task_conversation_message_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        task_id: &str,
        input: &TeamTaskNoteCreateInput<'_>,
    ) -> anyhow::Result<(TeamConversationRecord, TeamConversationMessageRecord, bool)> {
        let now = Utc::now().timestamp();
        let conversation_row = sqlx::query(
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
        .fetch_one(&mut **tx)
        .await?;
        let conversation = parse_team_conversation_row(&conversation_row)?;
        let redacted_payload = super::redact_sensitive_json(&input.payload);
        let payload_json = redacted_payload.to_string();
        let correlation_id = task_conversation_payload_correlation_id(&redacted_payload);
        let group_id = sqlx::query_scalar::<_, Option<String>>(
            "SELECT group_id FROM team_tasks WHERE id = ?1",
        )
        .bind(task_id)
        .fetch_optional(&mut **tx)
        .await?
        .flatten();
        let to_actor_id = input
            .to_actor_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let idempotency_key =
            super::normalize_optional_idempotency_key_input(input.idempotency_key);
        let (message, created) = insert_task_conversation_message_with_tx(
            tx,
            &conversation,
            TaskConversationInsertInput {
                task_id,
                from_actor_id: input.from_actor_id,
                to_actor_id,
                route: input.route,
                group_id,
                redacted_payload,
                payload_json,
                correlation_id,
                idempotency_key,
                created_at: now,
            },
        )
        .await?;

        Ok((conversation, message, created))
    }
}
