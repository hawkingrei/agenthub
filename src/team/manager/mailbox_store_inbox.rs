use std::collections::HashMap;

use agenthub_team_actor::{
    ActorMessageTaskRelation, ListActorInboxQuery, derive_actor_message_topic_metadata,
    parse_actor_message_task_relation, parse_actor_thread_claim_status,
};
use chrono::Utc;
use sqlx::sqlite::SqliteRow;
use sqlx::{Executor, QueryBuilder, Row, Sqlite, SqlitePool};

use super::codec::parse_team_actor_message_row;
use super::mailbox_store::{
    ActorMessageTaskLinkRecord, ActorThreadClaimRecord, SqlActorMailboxStore,
    SqlActorMailboxStoreError,
};
use crate::team::TeamActorMessageRecord;

#[derive(Debug)]
pub(super) struct ActorInboxSnapshot {
    pub(super) messages: Vec<TeamActorMessageRecord>,
    pub(super) pending_count: i64,
}

async fn count_pending_inbox_on_executor<'e, E>(
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

async fn list_inbox_rows_on_executor<'e, E>(
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

fn parse_inbox_rows(rows: Vec<SqliteRow>) -> Result<Vec<TeamActorMessageRecord>, sqlx::Error> {
    let mut messages = Vec::with_capacity(rows.len());
    for row in rows {
        messages.push(
            parse_team_actor_message_row(&row)
                .map_err(|err| sqlx::Error::Protocol(err.to_string()))?,
        );
    }
    Ok(messages)
}

fn apply_payload_task_fallback(message: &mut TeamActorMessageRecord) {
    if message.linked_task_id.is_some() {
        return;
    }
    let Some(metadata) =
        derive_actor_message_topic_metadata(message.message_id, &message.payload, None)
    else {
        return;
    };
    if let Some(task_id) = metadata.task_id {
        message.linked_task_id = Some(task_id);
        if message.linked_task_relation.is_none() {
            message.linked_task_relation = Some(ActorMessageTaskRelation::RelatedTask);
        }
    }
}

async fn load_latest_message_task_links(
    pool: &SqlitePool,
    run_id: &str,
    message_ids: &[i64],
) -> Result<HashMap<i64, ActorMessageTaskLinkRecord>, sqlx::Error> {
    if message_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut builder = QueryBuilder::<Sqlite>::new(
        r#"
        SELECT links.message_id, links.task_id, links.relation
        FROM team_actor_message_links AS links
        JOIN (
            SELECT message_id, MAX(id) AS max_id
            FROM team_actor_message_links
            WHERE run_id = "#,
    );
    builder.push_bind(run_id);
    builder.push(" AND message_id IN (");
    {
        let mut separated = builder.separated(", ");
        for message_id in message_ids {
            separated.push_bind(message_id);
        }
    }
    builder.push(
        r#")
            GROUP BY message_id
        ) AS latest
          ON latest.max_id = links.id
        "#,
    );
    let rows = builder.build().fetch_all(pool).await?;
    let mut out = HashMap::with_capacity(rows.len());
    for row in rows {
        let relation_raw: String = row.get("relation");
        let Some(relation) = parse_actor_message_task_relation(&relation_raw) else {
            continue;
        };
        out.insert(
            row.get("message_id"),
            ActorMessageTaskLinkRecord {
                task_id: row.get("task_id"),
                relation,
            },
        );
    }
    Ok(out)
}

async fn load_active_thread_claims(
    pool: &SqlitePool,
    run_id: &str,
    topic_keys: &[String],
) -> Result<HashMap<String, ActorThreadClaimRecord>, sqlx::Error> {
    if topic_keys.is_empty() {
        return Ok(HashMap::new());
    }
    let now = Utc::now().timestamp();
    let mut builder = QueryBuilder::<Sqlite>::new(
        r#"
        SELECT
            topic_key,
            task_id,
            root_message_id,
            owner_actor_id,
            claim_status,
            claimed_at,
            lease_expires_at
        FROM team_actor_thread_claims
        WHERE run_id = "#,
    );
    builder.push_bind(run_id);
    builder.push(" AND topic_key IN (");
    {
        let mut separated = builder.separated(", ");
        for topic_key in topic_keys {
            separated.push_bind(topic_key);
        }
    }
    builder.push(
        ") AND ((claim_status = 'claimed' AND (lease_expires_at IS NULL OR lease_expires_at > ",
    );
    builder.push_bind(now);
    builder.push(")) OR claim_status = 'completed')");
    let rows = builder.build().fetch_all(pool).await?;
    let mut out = HashMap::with_capacity(rows.len());
    for row in rows {
        let claim_status_raw: String = row.get("claim_status");
        let Some(claim_status) = parse_actor_thread_claim_status(&claim_status_raw) else {
            continue;
        };
        let topic_key: String = row.get("topic_key");
        out.insert(
            topic_key.clone(),
            ActorThreadClaimRecord {
                topic_key,
                task_id: row.try_get("task_id").ok(),
                root_message_id: row.try_get("root_message_id").ok(),
                owner_actor_id: row.get("owner_actor_id"),
                claim_status,
                claimed_at: row.get("claimed_at"),
                lease_expires_at: row.try_get("lease_expires_at").ok(),
            },
        );
    }
    Ok(out)
}

pub(super) async fn enrich_actor_messages(
    pool: &SqlitePool,
    messages: &mut [TeamActorMessageRecord],
) -> Result<(), sqlx::Error> {
    if messages.is_empty() {
        return Ok(());
    }
    for message in messages.iter_mut() {
        apply_payload_task_fallback(message);
        message.thread_topic_key = derive_actor_message_topic_metadata(
            message.message_id,
            &message.payload,
            message.linked_task_id.as_deref(),
        )
        .map(|metadata| metadata.topic_key);
        message.thread_claim_status = None;
        message.thread_owner_actor_id = None;
        message.thread_lease_expires_at = None;
    }

    let mut run_groups = HashMap::<String, Vec<usize>>::new();
    for (idx, message) in messages.iter().enumerate() {
        run_groups
            .entry(message.run_id.clone())
            .or_default()
            .push(idx);
    }
    for (run_id, indexes) in run_groups {
        let message_ids = indexes
            .iter()
            .map(|index| messages[*index].message_id)
            .collect::<Vec<_>>();
        let latest_links = load_latest_message_task_links(pool, &run_id, &message_ids).await?;
        for index in &indexes {
            if let Some(link) = latest_links.get(&messages[*index].message_id) {
                messages[*index].linked_task_id = Some(link.task_id.clone());
                messages[*index].linked_task_relation = Some(link.relation.clone());
            }
            messages[*index].thread_topic_key = derive_actor_message_topic_metadata(
                messages[*index].message_id,
                &messages[*index].payload,
                messages[*index].linked_task_id.as_deref(),
            )
            .map(|metadata| metadata.topic_key);
        }
        let topic_keys = indexes
            .iter()
            .filter_map(|index| messages[*index].thread_topic_key.clone())
            .collect::<Vec<_>>();
        let claims = load_active_thread_claims(pool, &run_id, &topic_keys).await?;
        for index in indexes {
            let Some(topic_key) = messages[index].thread_topic_key.clone() else {
                continue;
            };
            let Some(claim) = claims.get(&topic_key) else {
                continue;
            };
            messages[index].thread_claim_status = Some(claim.claim_status.clone());
            messages[index].thread_owner_actor_id = Some(claim.owner_actor_id.clone());
            messages[index].thread_lease_expires_at = claim.lease_expires_at;
        }
    }
    Ok(())
}

fn inbox_rows_include_pending(rows: &[SqliteRow]) -> bool {
    rows.iter().any(|row| {
        let status = row.try_get::<String, _>("status").ok();
        let disposition = row
            .try_get::<String, _>("handling_disposition")
            .unwrap_or_else(|_| "untriaged".to_string());
        status.as_deref() == Some("pending") && disposition == "untriaged"
    })
}

impl SqlActorMailboxStore {
    pub(super) async fn read_inbox_snapshot(
        &self,
        query: &ListActorInboxQuery,
    ) -> Result<ActorInboxSnapshot, SqlActorMailboxStoreError> {
        let mut tx = self.db.begin().await?;
        let pending_count = count_pending_inbox_on_executor(
            &mut *tx,
            &query.run_id,
            &query.actor_id,
            &query.peer_id,
        )
        .await?;
        let rows = if query.include_delivered && query.after_id.is_none() {
            let pending_only_query = ListActorInboxQuery {
                include_delivered: false,
                ..query.clone()
            };
            let pending_rows = list_inbox_rows_on_executor(&mut *tx, &pending_only_query).await?;
            if pending_rows.is_empty() {
                list_inbox_rows_on_executor(&mut *tx, query).await?
            } else {
                let requested_rows = list_inbox_rows_on_executor(&mut *tx, query).await?;
                if inbox_rows_include_pending(&requested_rows) {
                    requested_rows
                } else {
                    pending_rows
                }
            }
        } else {
            list_inbox_rows_on_executor(&mut *tx, query).await?
        };
        let mut messages = parse_inbox_rows(rows)?;
        tx.commit().await?;
        enrich_actor_messages(&self.db, &mut messages).await?;
        Ok(ActorInboxSnapshot {
            messages,
            pending_count,
        })
    }

    pub(super) async fn list_inbox_messages(
        &self,
        query: &ListActorInboxQuery,
    ) -> Result<Vec<TeamActorMessageRecord>, SqlActorMailboxStoreError> {
        let rows = if query.include_delivered && query.after_id.is_none() {
            let pending_only_query = ListActorInboxQuery {
                include_delivered: false,
                ..query.clone()
            };
            let pending_rows = list_inbox_rows_on_executor(&self.db, &pending_only_query).await?;
            if pending_rows.is_empty() {
                list_inbox_rows_on_executor(&self.db, query).await?
            } else {
                let requested_rows = list_inbox_rows_on_executor(&self.db, query).await?;
                if inbox_rows_include_pending(&requested_rows) {
                    requested_rows
                } else {
                    pending_rows
                }
            }
        } else {
            list_inbox_rows_on_executor(&self.db, query).await?
        };
        let mut messages = parse_inbox_rows(rows)?;
        enrich_actor_messages(&self.db, &mut messages).await?;
        Ok(messages)
    }
}
