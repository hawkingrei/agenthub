use agenthub_message_archive::MessageArchiveStoreRef;
use agenthub_team_actor::{
    AckActorMessageCommand, AckActorMessageResult, ActorMailboxStore, CreatePendingMessageResult,
    LinkActorMessageTaskCommand, LinkActorMessageTaskResult, ListActorInboxQuery,
    PendingRemoteRelayRecord, SendActorMessageCommand, TriageActorMessageCommand,
};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::SqlitePool;
use thiserror::Error;

use super::codec::{team_actor_message_status_to_str, team_actor_message_transport_to_str};
use super::mailbox::{
    fetch_enriched_message_by_id, fetch_message_by_idempotency, fetch_message_for_actor,
    maybe_persist_human_visible_chat_reply,
};
pub(super) use super::mailbox_store_inbox::enrich_actor_messages;
use super::mailbox_threads::ensure_idempotency_compatible;
use crate::team::{TeamActorMessageRecord, TeamActorMessageStatus};

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
    pub(super) relation: agenthub_team_actor::ActorMessageTaskRelation,
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
        self.triage_message_impl(cmd).await
    }

    async fn link_message_task(
        &self,
        cmd: &LinkActorMessageTaskCommand,
    ) -> Result<LinkActorMessageTaskResult, Self::Error> {
        self.link_message_task_impl(cmd).await
    }

    async fn list_remote_pending_messages(
        &self,
        limit: i64,
        now: i64,
    ) -> Result<Vec<PendingRemoteRelayRecord>, Self::Error> {
        self.list_remote_pending_messages_impl(limit, now).await
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
        self.mark_remote_retry_impl(run_id, message_id, ts, attempt, next_retry_at, error)
            .await
    }

    async fn mark_remote_dead_letter(
        &self,
        run_id: &str,
        message_id: i64,
        ts: i64,
        attempt: i64,
        error: &str,
    ) -> Result<(), Self::Error> {
        self.mark_remote_dead_letter_impl(run_id, message_id, ts, attempt, error)
            .await
    }

    async fn append_run_event(
        &self,
        run_id: &str,
        event_type: &str,
        ts: i64,
        payload: Value,
    ) -> Result<(), Self::Error> {
        self.append_run_event_impl(run_id, event_type, ts, payload)
            .await
    }
}
