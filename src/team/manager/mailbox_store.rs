use agenthub_message_archive::MessageArchiveStoreRef;
use agenthub_team_actor::{
    AckActorMessageCommand, AckActorMessageResult, ActorMailboxStore,
    ActorMessageHandlingDisposition, ActorMessageTaskRelation, CreatePendingMessageResult,
    LinkActorMessageTaskCommand, LinkActorMessageTaskResult, ListActorInboxQuery,
    PendingRemoteRelayRecord, SendActorMessageCommand, TriageActorMessageCommand,
};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::{Row, Sqlite, SqlitePool};
use thiserror::Error;

use super::codec::{
    parse_team_actor_message_row, team_actor_message_status_to_str,
    team_actor_message_transport_to_str,
};
use super::mailbox::{
    fetch_enriched_message_by_id, fetch_message_by_idempotency, fetch_message_for_actor,
    mailbox_run_event_archive_semaphore, maybe_persist_human_visible_chat_reply,
    resolve_team_id_for_run,
};
pub(super) use super::mailbox_store_inbox::enrich_actor_messages;
use super::mailbox_threads::{apply_thread_claim_transition, ensure_idempotency_compatible};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActorThreadClaimRecord {
    pub(super) topic_key: String,
    pub(super) task_id: Option<String>,
    pub(super) root_message_id: Option<i64>,
    pub(super) owner_actor_id: String,
    pub(super) claim_status: agenthub_team_actor::ActorThreadClaimStatus,
    pub(super) claimed_at: i64,
    pub(super) lease_expires_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActorMessageTaskLinkRecord {
    pub(super) task_id: String,
    pub(super) relation: ActorMessageTaskRelation,
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
            ensure_idempotency_compatible(cmd, &message)?;
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
        self.list_inbox_messages(query).await
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
