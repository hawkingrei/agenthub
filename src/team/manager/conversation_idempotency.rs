use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{Error as SqlxError, Sqlite};

use super::TaskConversationMessageStoreError;
use super::codec_rows::parse_team_conversation_message_row;
use crate::team::TeamConversationMessageRecord;
use agenthub_team_actor::canonical_json;

const TASK_CONVERSATION_MESSAGE_IDEMPOTENCY_UNIQUE_COLUMNS: &str = "team_conversation_messages.conversation_id, team_conversation_messages.from_actor_id, team_conversation_messages.idempotency_key";

pub(crate) fn task_conversation_payload_correlation_id(payload: &Value) -> String {
    payload
        .get("correlation_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn push_fingerprint_component(hasher: &mut Sha256, value: &str) {
    hasher.update(value.as_bytes());
    hasher.update([0_u8]);
}

fn task_conversation_message_fingerprint(
    task_id: &str,
    from_actor_id: &str,
    to_actor_id: Option<&str>,
    route: &str,
    payload: &Value,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update("task-conversation-message-fingerprint:v1");
    push_fingerprint_component(&mut hasher, task_id);
    push_fingerprint_component(&mut hasher, from_actor_id);
    push_fingerprint_component(&mut hasher, to_actor_id.unwrap_or(""));
    push_fingerprint_component(&mut hasher, route);
    push_fingerprint_component(&mut hasher, canonical_json(payload).as_str());
    super::hex_encode(&hasher.finalize())
}

/// Returns the matching message and whether its body was moved into the body store (so the caller can
/// rehydrate before comparing payloads).
pub(super) async fn fetch_task_conversation_message_by_idempotency(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    conversation_id: &str,
    from_actor_id: &str,
    idempotency_key: &str,
) -> Result<(TeamConversationMessageRecord, bool), sqlx::Error> {
    let row = sqlx::query(
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
        WHERE conversation_id = ?1
          AND from_actor_id = ?2
          AND idempotency_key = ?3
        LIMIT 1
        "#,
    )
    .bind(conversation_id)
    .bind(from_actor_id)
    .bind(idempotency_key)
    .fetch_one(&mut **tx)
    .await?;
    parse_team_conversation_message_row(&row).map_err(|err| sqlx::Error::Protocol(err.to_string()))
}

pub(super) fn ensure_task_conversation_message_idempotency_compatible(
    task_id: &str,
    from_actor_id: &str,
    to_actor_id: Option<&str>,
    route: &str,
    payload: &Value,
    existing: &TeamConversationMessageRecord,
) -> Result<(), TaskConversationMessageStoreError> {
    let incoming_fp =
        task_conversation_message_fingerprint(task_id, from_actor_id, to_actor_id, route, payload);
    let existing_fp = task_conversation_message_fingerprint(
        &existing.task_id,
        &existing.from_actor_id,
        existing.to_actor_id.as_deref(),
        &existing.route,
        &existing.payload,
    );
    if incoming_fp != existing_fp {
        return Err(TaskConversationMessageStoreError::IdempotencyConflict);
    }
    Ok(())
}

pub(super) fn is_task_conversation_message_idempotency_unique_violation(err: &SqlxError) -> bool {
    match err {
        SqlxError::Database(db_err) => {
            db_err.code().as_deref() == Some(super::SQLITE_CONSTRAINT_UNIQUE_CODE)
                && db_err
                    .message()
                    .contains(TASK_CONVERSATION_MESSAGE_IDEMPOTENCY_UNIQUE_COLUMNS)
        }
        _ => false,
    }
}
