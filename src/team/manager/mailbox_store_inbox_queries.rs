use agenthub_team_actor::ListActorInboxQuery;
use sqlx::sqlite::SqliteRow;
use sqlx::{Executor, Row, Sqlite};

use super::codec_rows::parse_team_actor_message_row;
use crate::team::TeamActorMessageRecord;

pub(super) async fn count_pending_inbox_on_executor<'e, E>(
    executor: E,
    run_id: &str,
    actor_id: &str,
    peer_id: &str,
) -> Result<i64, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_actor_messages
        WHERE run_id = ?1
          AND to_actor_id = ?2
          AND status = 'pending'
          AND handling_disposition = 'untriaged'
          AND to_peer_id = ?3
        "#,
    )
    .bind(run_id)
    .bind(actor_id)
    .bind(peer_id)
    .fetch_one(executor)
    .await
}

pub(super) async fn list_inbox_rows_on_executor<'e, E>(
    executor: E,
    query: &ListActorInboxQuery,
) -> Result<Vec<SqliteRow>, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    if query.include_delivered {
        if let Some(after_id) = query.after_id {
            sqlx::query(
                r#"
                SELECT
                    id,
                    run_id,
                    from_actor_id,
                    from_peer_id,
                    to_actor_id,
                    to_peer_id,
                    channel,
                    transport,
                    route_json,
                    payload_json,
                    idempotency_key,
                    message_kind,
                    handling_disposition,
                    handled_by_actor_id,
                    handled_at,
                    status,
                    created_at,
                    delivered_at
                FROM team_actor_messages
                WHERE run_id = ?1 AND to_actor_id = ?2 AND to_peer_id = ?3 AND id > ?4
                ORDER BY id ASC
                LIMIT ?5
                "#,
            )
            .bind(&query.run_id)
            .bind(&query.actor_id)
            .bind(&query.peer_id)
            .bind(after_id)
            .bind(query.limit)
            .fetch_all(executor)
            .await
        } else {
            sqlx::query(
                r#"
                SELECT
                    id,
                    run_id,
                    from_actor_id,
                    from_peer_id,
                    to_actor_id,
                    to_peer_id,
                    channel,
                    transport,
                    route_json,
                    payload_json,
                    idempotency_key,
                    message_kind,
                    handling_disposition,
                    handled_by_actor_id,
                    handled_at,
                    status,
                    created_at,
                    delivered_at
                FROM team_actor_messages
                WHERE run_id = ?1 AND to_actor_id = ?2 AND to_peer_id = ?3
                ORDER BY id ASC
                LIMIT ?4
                "#,
            )
            .bind(&query.run_id)
            .bind(&query.actor_id)
            .bind(&query.peer_id)
            .bind(query.limit)
            .fetch_all(executor)
            .await
        }
    } else if let Some(after_id) = query.after_id {
        sqlx::query(
            r#"
            SELECT
                id,
                run_id,
                from_actor_id,
                from_peer_id,
                to_actor_id,
                to_peer_id,
                channel,
                transport,
                route_json,
                payload_json,
                idempotency_key,
                message_kind,
                handling_disposition,
                handled_by_actor_id,
                handled_at,
                status,
                created_at,
                delivered_at
            FROM team_actor_messages
            WHERE run_id = ?1
              AND to_actor_id = ?2
              AND to_peer_id = ?3
              AND status = 'pending'
              AND handling_disposition = 'untriaged'
              AND id > ?4
            ORDER BY id ASC
            LIMIT ?5
            "#,
        )
        .bind(&query.run_id)
        .bind(&query.actor_id)
        .bind(&query.peer_id)
        .bind(after_id)
        .bind(query.limit)
        .fetch_all(executor)
        .await
    } else {
        sqlx::query(
            r#"
            SELECT
                id,
                run_id,
                from_actor_id,
                from_peer_id,
                to_actor_id,
                to_peer_id,
                channel,
                transport,
                route_json,
                payload_json,
                idempotency_key,
                message_kind,
                handling_disposition,
                handled_by_actor_id,
                handled_at,
                status,
                created_at,
                delivered_at
            FROM team_actor_messages
            WHERE run_id = ?1
              AND to_actor_id = ?2
              AND to_peer_id = ?3
              AND status = 'pending'
              AND handling_disposition = 'untriaged'
            ORDER BY id ASC
            LIMIT ?4
            "#,
        )
        .bind(&query.run_id)
        .bind(&query.actor_id)
        .bind(&query.peer_id)
        .bind(query.limit)
        .fetch_all(executor)
        .await
    }
}

pub(super) fn parse_inbox_rows(
    rows: Vec<SqliteRow>,
) -> Result<Vec<TeamActorMessageRecord>, sqlx::Error> {
    let mut messages = Vec::with_capacity(rows.len());
    for row in rows {
        messages.push(
            parse_team_actor_message_row(&row)
                .map_err(|err| sqlx::Error::Protocol(err.to_string()))?,
        );
    }
    Ok(messages)
}

pub(super) fn inbox_rows_include_pending(rows: &[SqliteRow]) -> bool {
    rows.iter().any(|row| {
        let status = row.try_get::<String, _>("status").ok();
        let disposition = row
            .try_get::<String, _>("handling_disposition")
            .unwrap_or_else(|_| "untriaged".to_string());
        status.as_deref() == Some("pending") && disposition == "untriaged"
    })
}
