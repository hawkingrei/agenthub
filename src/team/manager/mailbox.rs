use agenthub_team_actor::ActorMessageKind;
use std::sync::Arc;
use std::sync::OnceLock;

use serde_json::Value;

pub(super) use super::mailbox_errors::{
    map_actor_mailbox_store_error, map_actor_service_error, optional_trimmed,
    required_trimmed_field, validate_direct_mailbox_target_for_member_specs,
};
pub(super) use super::mailbox_mentions::{
    build_channel_mailbox_forward_payload, find_raw_actor_mention_range,
};
pub(super) use super::mailbox_queries::{
    fetch_enriched_message_by_id, fetch_message_by_idempotency, fetch_message_for_actor,
    resolve_team_id_for_run,
};
use super::mailbox_service::TeamActorMailboxService;
pub(super) use super::mailbox_shared_thread::maybe_persist_human_visible_chat_reply;
pub(super) use super::mailbox_sqlite::{
    normalize_optional_sqlite_string, parse_optional_sqlite_json_value,
};
pub(super) use super::mailbox_store::{SqlActorMailboxStore, SqlActorMailboxStoreError};
pub(super) use super::mailbox_store_inbox_enrichment::enrich_actor_messages;
pub(super) use super::mailbox_threads::{
    apply_thread_claim_takeover, apply_thread_claim_transition,
};
use super::{TeamManager, TeamReplyObligationSummary};
use crate::team::TeamActorMessageTransport;
use tokio::sync::Semaphore;

pub(super) const TEAM_SPECIAL_USER_ACTOR_ALIAS: &str = "user";
pub(super) const TEAM_SPECIAL_USER_ACTOR_PREFIX: &str = "user:";
pub(super) const MAILBOX_RESOLUTION_ESCALATED: &str = "escalated";
pub(super) const MAILBOX_RESOLUTION_TRANSFERRED: &str = "transferred";
pub(super) const MAILBOX_RESOLUTION_TAKEN_OVER: &str = "taken_over";
const MAILBOX_RUN_EVENT_ARCHIVE_MAX_CONCURRENCY: usize = 4;
pub(super) const ACTOR_THREAD_CLAIM_DEFAULT_LEASE_SECS: i64 = 30 * 60;

static MAILBOX_RUN_EVENT_ARCHIVE_SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();

pub(super) fn mailbox_run_event_archive_semaphore() -> Arc<Semaphore> {
    MAILBOX_RUN_EVENT_ARCHIVE_SEMAPHORE
        .get_or_init(|| Arc::new(Semaphore::new(MAILBOX_RUN_EVENT_ARCHIVE_MAX_CONCURRENCY)))
        .clone()
}

#[derive(Debug, Clone)]
pub(super) struct SharedThreadTarget {
    pub(super) task_id: String,
    pub(super) conversation_id: String,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedChannelMailboxTarget {
    pub(super) team_id: String,
    pub(super) task_id: String,
    pub(super) conversation_id: String,
    pub(super) recipient_actor_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedMailboxRecipientDelivery {
    pub(crate) actor_id: String,
    pub(crate) to_peer_id: String,
    pub(crate) transport: TeamActorMessageTransport,
    pub(crate) route: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ReplyActorPairKey {
    pub(super) agent_actor_id: String,
    pub(super) human_actor_id: String,
}

#[derive(Debug, Clone, Copy)]
pub struct TeamRemoteRelayWorkerSettings {
    pub poll_interval_secs: i64,
    pub batch_limit: i64,
    pub max_attempts: i64,
    pub retry_delay_secs: i64,
}

impl Default for TeamRemoteRelayWorkerSettings {
    fn default() -> Self {
        Self {
            poll_interval_secs: 5,
            batch_limit: 128,
            max_attempts: 5,
            retry_delay_secs: 15,
        }
    }
}

impl TeamManager {
    pub fn is_actor_message_idempotency_conflict(err: &anyhow::Error) -> bool {
        err.downcast_ref::<SqlActorMailboxStoreError>()
            .is_some_and(|cause| matches!(cause, SqlActorMailboxStoreError::IdempotencyConflict))
    }

    pub(crate) async fn summarize_open_reply_obligations(
        &self,
        run_id: &str,
    ) -> anyhow::Result<TeamReplyObligationSummary> {
        let messages =
            super::mailbox_reply_obligation_summary::load_reply_obligation_message_snapshots_on_executor(
                &self.db, run_id,
            )
            .await?;
        Ok(
            super::mailbox_reply_obligation_summary::summarize_open_reply_obligations_from_snapshots(
                messages.as_slice(),
            ),
        )
    }

    pub fn actor_thread_claim_conflict_owner(err: &anyhow::Error) -> Option<&str> {
        err.downcast_ref::<SqlActorMailboxStoreError>()
            .and_then(|cause| {
                if let SqlActorMailboxStoreError::ThreadClaimConflict { owner_actor_id } = cause {
                    Some(owner_actor_id.as_str())
                } else {
                    None
                }
            })
    }

    pub fn actor_mailbox_service(&self) -> TeamActorMailboxService {
        TeamActorMailboxService::new(self.clone())
    }
}

pub struct SendActorMessageInput<'a> {
    pub run_id: &'a str,
    pub from_actor_id: &'a str,
    pub from_peer_id: &'a str,
    pub to_actor_id: &'a str,
    pub to_peer_id: &'a str,
    pub channel: &'a str,
    pub transport: TeamActorMessageTransport,
    pub route: Option<Value>,
    pub payload: Value,
    pub message_kind: Option<ActorMessageKind>,
    pub idempotency_key: Option<&'a str>,
}
