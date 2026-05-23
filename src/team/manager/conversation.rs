use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{Error as SqlxError, QueryBuilder, Sqlite};

use super::codec::{parse_team_conversation_message_row, parse_team_conversation_row};
use super::{
    MESSAGE_ARCHIVE_APPEND_TIMEOUT, SQLITE_CONSTRAINT_UNIQUE_CODE, TeamManager,
    team_conversation_message_archive_document,
};
use crate::team::{
    TeamConversationMessageRecord, TeamConversationRecord, TeamConversationStreamEvent,
    TeamTaskNoteCreateInput,
};
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

async fn fetch_task_conversation_message_by_idempotency(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    conversation_id: &str,
    from_actor_id: &str,
    idempotency_key: &str,
) -> Result<TeamConversationMessageRecord, sqlx::Error> {
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

fn ensure_task_conversation_message_idempotency_compatible(
    task_id: &str,
    from_actor_id: &str,
    to_actor_id: Option<&str>,
    route: &str,
    payload: &Value,
    existing: &TeamConversationMessageRecord,
) -> Result<(), super::TaskConversationMessageStoreError> {
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
        return Err(super::TaskConversationMessageStoreError::IdempotencyConflict);
    }
    Ok(())
}

fn is_task_conversation_message_idempotency_unique_violation(err: &SqlxError) -> bool {
    match err {
        SqlxError::Database(db_err) => {
            db_err.code().as_deref() == Some(SQLITE_CONSTRAINT_UNIQUE_CODE)
                && db_err
                    .message()
                    .contains(TASK_CONVERSATION_MESSAGE_IDEMPOTENCY_UNIQUE_COLUMNS)
        }
        _ => false,
    }
}

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
            .bind(input.from_actor_id)
            .bind(to_actor_id.as_deref())
            .bind(input.route)
            .bind(&correlation_id)
            .bind(group_id.as_deref())
            .bind(&payload_json)
            .bind(idempotency_key)
            .bind(now)
            .execute(&mut **tx)
            .await
            {
                Ok(result) => (
                    TeamConversationMessageRecord {
                        message_id: result.last_insert_rowid(),
                        conversation_id: conversation.id.clone(),
                        task_id: task_id.to_string(),
                        group_id: group_id.clone(),
                        from_actor_id: input.from_actor_id.to_string(),
                        to_actor_id: to_actor_id.clone(),
                        route: input.route.to_string(),
                        payload: redacted_payload.clone(),
                        created_at: now,
                    },
                    true,
                ),
                Err(err) if is_task_conversation_message_idempotency_unique_violation(&err) => {
                    let existing = fetch_task_conversation_message_by_idempotency(
                        tx,
                        &conversation.id,
                        input.from_actor_id,
                        idempotency_key,
                    )
                    .await?;
                    ensure_task_conversation_message_idempotency_compatible(
                        task_id,
                        input.from_actor_id,
                        to_actor_id.as_deref(),
                        input.route,
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
            .bind(input.from_actor_id)
            .bind(to_actor_id.as_deref())
            .bind(input.route)
            .bind(&correlation_id)
            .bind(group_id.as_deref())
            .bind(&payload_json)
            .bind(now)
            .execute(&mut **tx)
            .await?;
            (
                TeamConversationMessageRecord {
                    message_id: result.last_insert_rowid(),
                    conversation_id: conversation.id.clone(),
                    task_id: task_id.to_string(),
                    group_id,
                    from_actor_id: input.from_actor_id.to_string(),
                    to_actor_id,
                    route: input.route.to_string(),
                    payload: redacted_payload,
                    created_at: now,
                },
                true,
            )
        };

        Ok((conversation, message, created))
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

        let (message, created) = if let Some(idempotency_key) = idempotency_key.as_deref() {
            let mut tx = self.db.begin().await?;
            let outcome = match sqlx::query(
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
            .bind(now)
            .execute(&mut *tx)
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
                        created_at: now,
                    },
                    true,
                ),
                Err(err) if is_task_conversation_message_idempotency_unique_violation(&err) => {
                    let existing = fetch_task_conversation_message_by_idempotency(
                        &mut tx,
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
            };
            tx.commit().await?;
            outcome
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
            .execute(&self.db)
            .await?;
            (
                TeamConversationMessageRecord {
                    message_id: result.last_insert_rowid(),
                    conversation_id: conversation.id.clone(),
                    task_id: task_id.to_string(),
                    group_id: group_id.clone(),
                    from_actor_id: from_actor_id.to_string(),
                    to_actor_id: to_actor_id.clone(),
                    route: route.to_string(),
                    payload: redacted_payload.clone(),
                    created_at: now,
                },
                true,
            )
        };

        if created {
            self.spawn_archive_task_conversation_message(&conversation, &message);
            self.emit_conversation_event(TeamConversationStreamEvent {
                team_id: conversation.team_id.clone(),
                task_id: task_id.to_string(),
                conversation_id: conversation.id.clone(),
                message_id: Some(message.message_id),
                source: "conversation_message".to_string(),
            });
        }

        Ok((message, created))
    }

    pub(super) fn spawn_archive_task_conversation_message(
        &self,
        conversation: &TeamConversationRecord,
        message: &TeamConversationMessageRecord,
    ) {
        let Some(archive) = self.message_archive.as_ref() else {
            return;
        };
        let archive = archive.clone();
        let document = team_conversation_message_archive_document(conversation, message);
        let conversation_id = message.conversation_id.clone();
        let message_id = message.message_id;

        tokio::spawn(async move {
            match tokio::time::timeout(
                MESSAGE_ARCHIVE_APPEND_TIMEOUT,
                archive.append_documents(&[document]),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::warn!(
                        error = ?error,
                        conversation_id = %conversation_id,
                        message_id,
                        "failed to dual-write team conversation message to archive"
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        conversation_id = %conversation_id,
                        message_id,
                        timeout_ms = MESSAGE_ARCHIVE_APPEND_TIMEOUT.as_millis(),
                        "timed out dual-writing team conversation message to archive"
                    );
                }
            }
        });
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
