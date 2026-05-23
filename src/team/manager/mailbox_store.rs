use std::collections::HashMap;

use agenthub_message_archive::MessageArchiveStoreRef;
use agenthub_team_actor::{
    AckActorMessageCommand, AckActorMessageResult, ActorMailboxStore,
    ActorMessageHandlingDisposition, ActorMessageTaskRelation, CreatePendingMessageResult,
    LinkActorMessageTaskCommand, LinkActorMessageTaskResult, ListActorInboxQuery,
    PendingRemoteRelayRecord, SendActorMessageCommand, TriageActorMessageCommand,
    derive_actor_message_topic_metadata, parse_actor_message_task_relation,
    parse_actor_thread_claim_status,
};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use sqlx::sqlite::SqliteRow;
use sqlx::{Executor, QueryBuilder, Row, Sqlite, SqlitePool};
use thiserror::Error;

use super::codec::{
    parse_team_actor_message_row, team_actor_message_status_to_str,
    team_actor_message_transport_to_str,
};
use super::mailbox::{
    apply_thread_claim_transition, fetch_enriched_message_by_id, fetch_message_by_idempotency,
    fetch_message_for_actor, mailbox_run_event_archive_semaphore,
    maybe_persist_human_visible_chat_reply, resolve_team_id_for_run,
};
use super::team_run_event_archive_document_for_db;
use crate::team::{TeamActorMessageRecord, TeamActorMessageStatus, TeamRunEventRecord};

#[derive(Clone)]
pub(super) struct SqlActorMailboxStore {
    pub(super) db: SqlitePool,
    pub(super) message_archive: Option<MessageArchiveStoreRef>,
}

#[derive(Debug, Error)]
pub(super) enum SqlActorMailboxStoreError {
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error("actor message idempotency conflict")]
    IdempotencyConflict,
    #[error(
        "reply-required mailbox work cannot be completed before a visible reply is emitted or the item is explicitly escalated/transferred"
    )]
    ReplyRequiredVisibleOutcomeMissing,
    #[error("only human-originated reply-required mailbox work can be escalated")]
    ReplyRequiredEscalationUnsupported,
    #[error("reply-required mailbox work is already assigned to coordinator")]
    ReplyRequiredEscalationAlreadyAtCoordinator,
    #[error("reply-required mailbox escalation target is unavailable")]
    ReplyRequiredEscalationTargetUnavailable,
    #[error("thread topic is already claimed by actor `{owner_actor_id}`")]
    ThreadClaimConflict { owner_actor_id: String },
    #[error("thread topic must be claimed by the acting actor")]
    ThreadClaimOwnershipRequired,
}

#[derive(Debug)]
pub(super) struct ActorInboxSnapshot {
    pub(super) messages: Vec<TeamActorMessageRecord>,
    pub(super) pending_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActorThreadClaimRecord {
    topic_key: String,
    task_id: Option<String>,
    root_message_id: Option<i64>,
    owner_actor_id: String,
    claim_status: agenthub_team_actor::ActorThreadClaimStatus,
    claimed_at: i64,
    lease_expires_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActorMessageTaskLinkRecord {
    pub(super) task_id: String,
    pub(super) relation: ActorMessageTaskRelation,
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

async fn ensure_reply_required_completion_allowed(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    message: &TeamActorMessageRecord,
) -> Result<(), SqlActorMailboxStoreError> {
    if super::mailbox_reply_obligations::reply_actor_pair_for_inbound_obligation(message).is_none()
    {
        return Ok(());
    }
    let messages =
        super::mailbox_reply_obligations::load_reply_obligation_message_snapshots_on_executor(
            &mut **tx,
            &message.run_id,
        )
        .await?;
    if super::mailbox_reply_obligations::has_visible_reply_credit_for_message(
        &messages,
        message.message_id,
    ) {
        return Ok(());
    }
    Err(SqlActorMailboxStoreError::ReplyRequiredVisibleOutcomeMissing)
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
}

#[async_trait]
impl ActorMailboxStore for SqlActorMailboxStore {
    type Error = SqlActorMailboxStoreError;

    async fn create_pending_message(
        &self,
        cmd: &SendActorMessageCommand,
    ) -> Result<CreatePendingMessageResult, Self::Error> {
        let route_json = cmd
            .route
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|err| sqlx::Error::Protocol(err.to_string()))?;
        let payload_json = serde_json::to_string(&cmd.payload)
            .map_err(|err| sqlx::Error::Protocol(err.to_string()))?;
        let transport_raw = team_actor_message_transport_to_str(&cmd.transport);
        let status_raw = team_actor_message_status_to_str(&TeamActorMessageStatus::Pending);

        let mut tx = self.db.begin().await?;
        let inserted = sqlx::query(
            r#"
            INSERT OR IGNORE INTO team_actor_messages (
                run_id,
                from_actor_id,
                from_peer_id,
                to_actor_id,
                to_peer_id,
                channel,
                transport,
                route_json,
                payload_json,
                message_kind,
                group_id,
                status,
                created_at,
                idempotency_key
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, (SELECT group_id FROM team_runs WHERE id = ?1), ?11, ?12, ?13)
            "#,
        )
        .bind(&cmd.run_id)
        .bind(&cmd.from_actor_id)
        .bind(&cmd.from_peer_id)
        .bind(&cmd.to_actor_id)
        .bind(&cmd.to_peer_id)
        .bind(&cmd.channel)
        .bind(transport_raw)
        .bind(route_json)
        .bind(payload_json)
        .bind(cmd.message_kind.as_str())
        .bind(status_raw)
        .bind(cmd.created_at)
        .bind(&cmd.idempotency_key)
        .execute(&mut *tx)
        .await?;

        let (message_id, created) = if inserted.rows_affected() == 1 {
            let message_id = inserted.last_insert_rowid();
            maybe_persist_human_visible_chat_reply(&mut tx, cmd).await?;
            (message_id, true)
        } else if let Some(idempotency_key) = cmd.idempotency_key.as_deref() {
            let message = fetch_message_by_idempotency(
                &mut tx,
                &cmd.run_id,
                &cmd.from_actor_id,
                &cmd.from_peer_id,
                idempotency_key,
            )
            .await?;
            super::mailbox::ensure_idempotency_compatible(cmd, &message)?;
            (message.message_id, false)
        } else {
            return Err(sqlx::Error::Protocol(
                "insert was ignored without idempotency_key".to_string(),
            )
            .into());
        };
        tx.commit().await?;
        let message = fetch_enriched_message_by_id(&self.db, message_id).await?;
        Ok(CreatePendingMessageResult { message, created })
    }

    async fn list_inbox(
        &self,
        query: &ListActorInboxQuery,
    ) -> Result<Vec<TeamActorMessageRecord>, Self::Error> {
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

    async fn ack_message(
        &self,
        cmd: &AckActorMessageCommand,
    ) -> Result<AckActorMessageResult, Self::Error> {
        let mut tx = self.db.begin().await?;
        let current = fetch_message_for_actor(
            &mut tx,
            &cmd.run_id,
            &cmd.actor_id,
            &cmd.peer_id,
            cmd.message_id,
        )
        .await?;
        let status_changed = if current.status == TeamActorMessageStatus::Pending {
            sqlx::query(
                r#"
                UPDATE team_actor_messages
                SET status = 'delivered', delivered_at = COALESCE(delivered_at, ?1)
                WHERE id = ?2 AND run_id = ?3 AND to_actor_id = ?4 AND to_peer_id = ?5 AND status = 'pending'
                "#,
            )
            .bind(cmd.delivered_at)
            .bind(cmd.message_id)
            .bind(&cmd.run_id)
            .bind(&cmd.actor_id)
            .bind(&cmd.peer_id)
            .execute(&mut *tx)
            .await?
            .rows_affected()
                > 0
        } else {
            false
        };

        tx.commit().await?;
        let message = fetch_enriched_message_by_id(&self.db, cmd.message_id).await?;
        Ok(AckActorMessageResult {
            message,
            status_changed,
        })
    }

    async fn triage_message(
        &self,
        cmd: &TriageActorMessageCommand,
    ) -> Result<agenthub_team_actor::TriageActorMessageResult, Self::Error> {
        let mut tx = self.db.begin().await?;
        let message = fetch_message_for_actor(
            &mut tx,
            &cmd.run_id,
            &cmd.actor_id,
            &cmd.peer_id,
            cmd.message_id,
        )
        .await?;
        if cmd.disposition == ActorMessageHandlingDisposition::Completed {
            ensure_reply_required_completion_allowed(&mut tx, &message).await?;
        }
        apply_thread_claim_transition(&mut tx, &message, cmd).await?;
        let update = sqlx::query(
            r#"
            UPDATE team_actor_messages
            SET
                handling_disposition = ?1,
                handled_by_actor_id = ?2,
                handled_at = ?3
            WHERE id = ?4
              AND run_id = ?5
              AND to_actor_id = ?6
              AND to_peer_id = ?7
              AND handling_disposition <> ?1
            "#,
        )
        .bind(cmd.disposition.as_str())
        .bind(&cmd.actor_id)
        .bind(cmd.handled_at)
        .bind(cmd.message_id)
        .bind(&cmd.run_id)
        .bind(&cmd.actor_id)
        .bind(&cmd.peer_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let message = fetch_enriched_message_by_id(&self.db, cmd.message_id).await?;
        Ok(agenthub_team_actor::TriageActorMessageResult {
            message,
            handling_changed: update.rows_affected() > 0,
        })
    }

    async fn link_message_task(
        &self,
        cmd: &LinkActorMessageTaskCommand,
    ) -> Result<LinkActorMessageTaskResult, Self::Error> {
        let mut tx = self.db.begin().await?;
        fetch_message_for_actor(
            &mut tx,
            &cmd.run_id,
            &cmd.actor_id,
            &cmd.peer_id,
            cmd.message_id,
        )
        .await?;
        let team_id = resolve_team_id_for_run(&mut tx, &cmd.run_id).await?;
        let task_exists = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT 1
            FROM team_tasks
            WHERE id = ?1 AND team_id = ?2
            LIMIT 1
            "#,
        )
        .bind(&cmd.task_id)
        .bind(&team_id)
        .fetch_optional(&mut *tx)
        .await?;
        if task_exists.is_none() {
            return Err(sqlx::Error::RowNotFound.into());
        }
        let inserted = sqlx::query(
            r#"
            INSERT OR IGNORE INTO team_actor_message_links (
                run_id,
                message_id,
                task_id,
                relation,
                created_by_actor_id,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&cmd.run_id)
        .bind(cmd.message_id)
        .bind(&cmd.task_id)
        .bind(cmd.relation.as_str())
        .bind(&cmd.actor_id)
        .bind(cmd.linked_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let message = fetch_enriched_message_by_id(&self.db, cmd.message_id).await?;
        Ok(LinkActorMessageTaskResult {
            message,
            task_id: cmd.task_id.clone(),
            relation: cmd.relation.clone(),
            created: inserted.rows_affected() > 0,
        })
    }

    async fn list_remote_pending_messages(
        &self,
        limit: i64,
        now: i64,
    ) -> Result<Vec<PendingRemoteRelayRecord>, Self::Error> {
        let rows = sqlx::query(
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
                message_kind,
                handling_disposition,
                handled_by_actor_id,
                handled_at,
                status,
                created_at,
                delivered_at,
                relay_attempt
            FROM team_actor_messages
            WHERE transport = 'remote'
                AND status = 'pending'
                AND (
                    relay_next_retry_at IS NULL
                    OR relay_next_retry_at <= ?1
                )
            ORDER BY id ASC
            LIMIT ?2
            "#,
        )
        .bind(now)
        .bind(limit.max(1))
        .fetch_all(&self.db)
        .await?;

        let mut messages = Vec::with_capacity(rows.len());
        for row in rows {
            let message = parse_team_actor_message_row(&row)
                .map_err(|err| sqlx::Error::Protocol(err.to_string()))?;
            let attempt: i64 = row.try_get("relay_attempt").unwrap_or(0);
            messages.push(PendingRemoteRelayRecord { message, attempt });
        }
        Ok(messages)
    }

    async fn mark_remote_retry(
        &self,
        run_id: &str,
        message_id: i64,
        ts: i64,
        attempt: i64,
        next_retry_at: i64,
        error: &str,
    ) -> Result<(), Self::Error> {
        sqlx::query(
            r#"
            UPDATE team_actor_messages
            SET
                relay_attempt = ?1,
                relay_next_retry_at = ?2,
                relay_last_error = ?3
            WHERE id = ?4 AND run_id = ?5 AND transport = 'remote' AND status = 'pending'
            "#,
        )
        .bind(attempt)
        .bind(next_retry_at.max(ts))
        .bind(error)
        .bind(message_id)
        .bind(run_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn mark_remote_dead_letter(
        &self,
        run_id: &str,
        message_id: i64,
        ts: i64,
        attempt: i64,
        error: &str,
    ) -> Result<(), Self::Error> {
        sqlx::query(
            r#"
            UPDATE team_actor_messages
            SET
                status = 'dead_letter',
                relay_attempt = ?1,
                dead_letter_at = ?2,
                relay_last_error = ?3
            WHERE id = ?4 AND run_id = ?5 AND transport = 'remote' AND status = 'pending'
            "#,
        )
        .bind(attempt)
        .bind(ts)
        .bind(error)
        .bind(message_id)
        .bind(run_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn append_run_event(
        &self,
        run_id: &str,
        event_type: &str,
        ts: i64,
        payload: Value,
    ) -> Result<(), Self::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
            VALUES (?1, NULL, ?2, ?3, ?4)
            "#,
        )
        .bind(run_id)
        .bind(event_type)
        .bind(ts)
        .bind(payload.to_string())
        .execute(&self.db)
        .await?;
        let event = TeamRunEventRecord {
            event_id: result.last_insert_rowid(),
            run_id: run_id.to_string(),
            step_id: None,
            event_type: event_type.to_string(),
            ts,
            payload,
        };
        if let Some(archive) = self.message_archive.as_ref().cloned() {
            let db = self.db.clone();
            let permit = mailbox_run_event_archive_semaphore()
                .acquire_owned()
                .await
                .expect("mailbox run event archive semaphore stays open");
            let run_id = event.run_id.clone();
            let event_id = event.event_id;
            tokio::spawn(async move {
                let _permit = permit;
                match team_run_event_archive_document_for_db(&db, &event).await {
                    Ok(Some(document)) => {
                        match tokio::time::timeout(
                            super::MESSAGE_ARCHIVE_APPEND_TIMEOUT,
                            archive.append_documents(&[document]),
                        )
                        .await
                        {
                            Ok(Ok(())) => {}
                            Ok(Err(error)) => {
                                tracing::warn!(
                                    error = ?error,
                                    run_id = %run_id,
                                    event_id,
                                    "failed to dual-write team actor mailbox run event to archive"
                                );
                            }
                            Err(_) => {
                                tracing::warn!(
                                    run_id = %run_id,
                                    event_id,
                                    timeout_ms = super::MESSAGE_ARCHIVE_APPEND_TIMEOUT.as_millis(),
                                    "timed out dual-writing team actor mailbox run event to archive"
                                );
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        tracing::warn!(
                            error = ?error,
                            run_id = %run_id,
                            event_id,
                            "failed to build team actor mailbox run event archive document"
                        );
                    }
                }
            });
        }
        Ok(())
    }
}
