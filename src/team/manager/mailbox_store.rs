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

use crate::team::TeamActorMessageRecord;

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
        self.create_pending_message_impl(cmd).await
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
        self.ack_message_impl(cmd).await
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
