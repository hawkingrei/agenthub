use chrono::Utc;
use serde_json::Value;

use super::TeamManager;
use super::conversation_idempotency::task_conversation_payload_correlation_id;
use super::conversation_insert_common::{
    TaskConversationInsertInput, insert_task_conversation_message_with_tx,
};
use crate::team::TeamConversationMessageRecord;

impl TeamManager {
    pub async fn append_task_conversation_message(
        &self,
        task_id: &str,
        from_actor_id: &str,
        to_actor_id: Option<&str>,
        route: &str,
        payload: Value,
    ) -> anyhow::Result<TeamConversationMessageRecord> {
        let (message, _created) = self
            .append_task_conversation_message_with_created(
                task_id,
                from_actor_id,
                to_actor_id,
                route,
                payload,
                None,
            )
            .await?;
        Ok(message)
    }

    pub async fn append_task_conversation_message_with_created(
        &self,
        task_id: &str,
        from_actor_id: &str,
        to_actor_id: Option<&str>,
        route: &str,
        payload: Value,
        idempotency_key: Option<&str>,
    ) -> anyhow::Result<(TeamConversationMessageRecord, bool)> {
        let now = Utc::now().timestamp();
        let conversation = self.get_task_conversation(task_id).await?;
        let redacted_payload = super::redact_sensitive_json(&payload);
        let payload_json = redacted_payload.to_string();
        let correlation_id = task_conversation_payload_correlation_id(&redacted_payload);
        let group_id = sqlx::query_scalar::<_, Option<String>>(
            "SELECT group_id FROM team_tasks WHERE id = ?1",
        )
        .bind(task_id)
        .fetch_optional(&self.db)
        .await?
        .flatten();
        let to_actor_id = to_actor_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let idempotency_key = super::normalize_optional_idempotency_key_input(idempotency_key);
        let mut tx = self.db.begin().await?;
        let outcome = insert_task_conversation_message_with_tx(
            &mut tx,
            &conversation,
            TaskConversationInsertInput {
                task_id,
                from_actor_id,
                to_actor_id,
                route,
                group_id,
                redacted_payload,
                payload_json,
                correlation_id,
                idempotency_key,
                created_at: now,
            },
        )
        .await?;
        tx.commit().await?;
        let (message, created) = outcome;

        if created {
            self.spawn_archive_task_conversation_message(&conversation, &message);
            self.emit_task_conversation_message_created(&conversation, &message);
        }

        Ok((message, created))
    }
}
