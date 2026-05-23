use serde_json::Value;
use sqlx::Sqlite;

use super::conversation_idempotency::{
    ensure_task_conversation_message_idempotency_compatible,
    fetch_task_conversation_message_by_idempotency,
    is_task_conversation_message_idempotency_unique_violation,
};
use crate::team::{TeamConversationMessageRecord, TeamConversationRecord};

pub(super) struct TaskConversationInsertInput<'a> {
    pub(super) task_id: &'a str,
    pub(super) from_actor_id: &'a str,
    pub(super) to_actor_id: Option<String>,
    pub(super) route: &'a str,
    pub(super) group_id: Option<String>,
    pub(super) redacted_payload: Value,
    pub(super) payload_json: String,
    pub(super) correlation_id: String,
    pub(super) idempotency_key: Option<String>,
    pub(super) created_at: i64,
}

pub(super) async fn insert_task_conversation_message_with_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    conversation: &TeamConversationRecord,
    input: TaskConversationInsertInput<'_>,
) -> anyhow::Result<(TeamConversationMessageRecord, bool)> {
    let TaskConversationInsertInput {
        task_id,
        from_actor_id,
        to_actor_id,
        route,
        group_id,
        redacted_payload,
        payload_json,
        correlation_id,
        idempotency_key,
        created_at,
    } = input;

    let (message, created) = if let Some(idempotency_key) = idempotency_key.as_deref() {
        match sqlx::query(
            r#"
            INSERT INTO team_conversation_messages (
                conversation_id,
                task_id,
                from_actor_id,
                to_actor_id,
                route,
                correlation_id,
                group_id,
                payload_json,
                idempotency_key,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(&conversation.id)
        .bind(task_id)
        .bind(from_actor_id)
        .bind(to_actor_id.as_deref())
        .bind(route)
        .bind(&correlation_id)
        .bind(group_id.as_deref())
        .bind(&payload_json)
        .bind(idempotency_key)
        .bind(created_at)
        .execute(&mut **tx)
        .await
        {
            Ok(result) => (
                TeamConversationMessageRecord {
                    message_id: result.last_insert_rowid(),
                    conversation_id: conversation.id.clone(),
                    task_id: task_id.to_string(),
                    group_id: group_id.clone(),
                    from_actor_id: from_actor_id.to_string(),
                    to_actor_id: to_actor_id.clone(),
                    route: route.to_string(),
                    payload: redacted_payload.clone(),
                    created_at,
                },
                true,
            ),
            Err(err) if is_task_conversation_message_idempotency_unique_violation(&err) => {
                let existing = fetch_task_conversation_message_by_idempotency(
                    tx,
                    &conversation.id,
                    from_actor_id,
                    idempotency_key,
                )
                .await?;
                ensure_task_conversation_message_idempotency_compatible(
                    task_id,
                    from_actor_id,
                    to_actor_id.as_deref(),
                    route,
                    &redacted_payload,
                    &existing,
                )?;
                (existing, false)
            }
            Err(err) => return Err(err.into()),
        }
    } else {
        let result = sqlx::query(
            r#"
            INSERT INTO team_conversation_messages (
                conversation_id,
                task_id,
                from_actor_id,
                to_actor_id,
                route,
                correlation_id,
                group_id,
                payload_json,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&conversation.id)
        .bind(task_id)
        .bind(from_actor_id)
        .bind(to_actor_id.as_deref())
        .bind(route)
        .bind(&correlation_id)
        .bind(group_id.as_deref())
        .bind(&payload_json)
        .bind(created_at)
        .execute(&mut **tx)
        .await?;
        (
            TeamConversationMessageRecord {
                message_id: result.last_insert_rowid(),
                conversation_id: conversation.id.clone(),
                task_id: task_id.to_string(),
                group_id,
                from_actor_id: from_actor_id.to_string(),
                to_actor_id,
                route: route.to_string(),
                payload: redacted_payload,
                created_at,
            },
            true,
        )
    };

    Ok((message, created))
}
