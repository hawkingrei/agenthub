use serde_json::Value;
use sqlx::sqlite::SqliteRow;
use sqlx::{Executor, Row, Sqlite};

use super::mailbox::{normalize_optional_sqlite_string, parse_optional_sqlite_json_value};
use crate::team::{TeamActorMessageStatus, TeamActorMessageTransport};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ActorMessageHandlingDisposition, ActorMessageKind,
    parse_actor_message_handling_disposition, parse_actor_message_kind,
};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ReplyObligationMessageSnapshot {
    pub(super) message_id: i64,
    pub(super) from_actor_id: String,
    pub(super) to_actor_id: String,
    pub(super) to_peer_id: String,
    pub(super) transport: TeamActorMessageTransport,
    pub(super) status: TeamActorMessageStatus,
    pub(super) handling_disposition: ActorMessageHandlingDisposition,
    pub(super) message_kind: ActorMessageKind,
    pub(super) source_surface: Option<String>,
    pub(super) reply_target: Option<Value>,
    pub(super) conversation_id: Option<String>,
    pub(super) thread_root_message_id: Option<i64>,
    pub(super) requires_user_visible_reply: bool,
    pub(super) mailbox_resolution_kind: Option<String>,
    pub(super) reply_payload_type: Option<String>,
    pub(super) reply_text: Option<String>,
    pub(super) reply_correlation_id: Option<String>,
    pub(super) reply_string_payload: Option<String>,
    pub(super) created_at: i64,
}

fn parse_reply_obligation_message_snapshot_row(
    row: &SqliteRow,
) -> Result<ReplyObligationMessageSnapshot, sqlx::Error> {
    let transport_raw: String = row.get("transport");
    let status_raw: String = row.get("status");
    let handling_disposition_raw: String = row
        .try_get("handling_disposition")
        .unwrap_or_else(|_| "untriaged".to_string());
    let message_kind_raw: String = row.try_get("message_kind").unwrap_or_default();
    let requires_user_visible_reply = row
        .try_get::<Option<i64>, _>("requires_user_visible_reply")
        .ok()
        .flatten()
        .unwrap_or_default()
        != 0;
    Ok(ReplyObligationMessageSnapshot {
        message_id: row.get("id"),
        from_actor_id: row.get("from_actor_id"),
        to_actor_id: row.get("to_actor_id"),
        to_peer_id: row
            .try_get("to_peer_id")
            .unwrap_or_else(|_| ACTOR_MAIN_PEER_ID.to_string()),
        transport: super::codec_status::team_actor_message_transport_from_str(&transport_raw),
        status: super::codec_status::team_actor_message_status_from_str(&status_raw),
        handling_disposition: parse_actor_message_handling_disposition(&handling_disposition_raw),
        message_kind: parse_actor_message_kind(&message_kind_raw),
        source_surface: normalize_optional_sqlite_string(row.try_get("source_surface")?),
        reply_target: parse_optional_sqlite_json_value(row.try_get("reply_target_json")?)?,
        conversation_id: normalize_optional_sqlite_string(row.try_get("conversation_id")?),
        thread_root_message_id: row.try_get("thread_root_message_id").ok().flatten(),
        requires_user_visible_reply,
        mailbox_resolution_kind: normalize_optional_sqlite_string(
            row.try_get("mailbox_resolution_kind")?,
        ),
        reply_payload_type: normalize_optional_sqlite_string(row.try_get("reply_payload_type")?),
        reply_text: normalize_optional_sqlite_string(row.try_get("reply_text")?),
        reply_correlation_id: normalize_optional_sqlite_string(
            row.try_get("reply_correlation_id")?,
        ),
        reply_string_payload: normalize_optional_sqlite_string(
            row.try_get("reply_string_payload")?,
        ),
        created_at: row.get("created_at"),
    })
}

pub(super) async fn load_reply_obligation_message_snapshots_on_executor<'e, E>(
    executor: E,
    run_id: &str,
) -> Result<Vec<ReplyObligationMessageSnapshot>, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows = sqlx::query(
        r#"
        SELECT
            id,
            from_actor_id,
            to_actor_id,
            to_peer_id,
            transport,
            status,
            message_kind,
            handling_disposition,
            trim(COALESCE(json_extract(payload_json, '$.source_surface'), '')) AS source_surface,
            json_extract(payload_json, '$.reply_target') AS reply_target_json,
            trim(COALESCE(
                json_extract(payload_json, '$.task_conversation_id'),
                json_extract(payload_json, '$.channel_conversation_id'),
                json_extract(payload_json, '$.conversation_id'),
                ''
            )) AS conversation_id,
            CAST(json_extract(payload_json, '$.thread_root_message_id') AS INTEGER) AS thread_root_message_id,
            COALESCE(CAST(json_extract(payload_json, '$.requires_user_visible_reply') AS INTEGER), 0) AS requires_user_visible_reply,
            trim(COALESCE(json_extract(payload_json, '$.mailbox_resolution.kind'), '')) AS mailbox_resolution_kind,
            trim(COALESCE(json_extract(payload_json, '$.type'), '')) AS reply_payload_type,
            trim(COALESCE(json_extract(payload_json, '$.text'), '')) AS reply_text,
            trim(COALESCE(json_extract(payload_json, '$.correlation_id'), '')) AS reply_correlation_id,
            CASE
                WHEN json_type(payload_json) = 'text' THEN trim(COALESCE(json_extract(payload_json, '$'), ''))
                ELSE NULL
            END AS reply_string_payload,
            created_at
        FROM team_actor_messages
        WHERE run_id = ?1
          AND (
              COALESCE(CAST(json_extract(payload_json, '$.requires_user_visible_reply') AS INTEGER), 0) != 0
              OR message_kind = 'human_request'
              OR (
                  transport = 'local'
                  AND to_peer_id = 'main'
                  AND (
                      to_actor_id = 'user'
                      OR to_actor_id = 'human'
                      OR to_actor_id LIKE 'user:%'
                      OR to_actor_id LIKE 'human:%'
                  )
              )
          )
        ORDER BY id ASC
        "#,
    )
    .bind(run_id)
    .fetch_all(executor)
    .await?;
    let mut snapshots = Vec::with_capacity(rows.len());
    for row in rows {
        snapshots.push(parse_reply_obligation_message_snapshot_row(&row)?);
    }
    Ok(snapshots)
}
