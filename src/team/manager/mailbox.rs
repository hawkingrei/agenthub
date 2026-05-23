use agenthub_team_actor::ActorMessageKind;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

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
pub(super) use super::mailbox_store::{
    SqlActorMailboxStore, SqlActorMailboxStoreError, enrich_actor_messages,
};
pub(super) use super::mailbox_threads::apply_thread_claim_transition;
use super::{TeamManager, TeamReplyObligationSummary};
use crate::team::TeamActorMessageTransport;
use tokio::sync::Semaphore;

pub(super) const TEAM_SPECIAL_USER_ACTOR_ALIAS: &str = "user";
pub(super) const TEAM_SPECIAL_USER_ACTOR_PREFIX: &str = "user:";
pub(super) const MAILBOX_RESOLUTION_ESCALATED: &str = "escalated";
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
pub(super) struct ResolvedMailboxRecipientDelivery {
    pub(super) actor_id: String,
    pub(super) to_peer_id: String,
    pub(super) transport: TeamActorMessageTransport,
    pub(super) route: Option<Value>,
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
            super::mailbox_reply_obligations::load_reply_obligation_message_snapshots_on_executor(
                &self.db, run_id,
            )
            .await?;
        Ok(
            super::mailbox_reply_obligations::summarize_open_reply_obligations_from_snapshots(
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

    pub fn spawn_remote_relay_worker(self: Arc<Self>, settings: TeamRemoteRelayWorkerSettings) {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(
                settings.poll_interval_secs.max(1) as u64,
            ));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                match self
                    .relay_remote_messages_once(
                        settings.batch_limit,
                        settings.max_attempts,
                        settings.retry_delay_secs,
                    )
                    .await
                {
                    Ok(summary) => {
                        if summary.scanned > 0 {
                            tracing::debug!(
                                scanned = summary.scanned,
                                delivered = summary.delivered,
                                retried = summary.retried,
                                dead_lettered = summary.dead_lettered,
                                "team relay worker tick"
                            );
                        }
                    }
                    Err(err) => {
                        tracing::warn!("team relay worker tick failed: {}", err);
                    }
                }
            }
        });
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

pub(super) fn normalize_optional_sqlite_string(raw: Option<String>) -> Option<String> {
    raw.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "null")
}

pub(super) fn parse_optional_sqlite_json_value(
    raw: Option<String>,
) -> Result<Option<Value>, sqlx::Error> {
    let Some(raw) = normalize_optional_sqlite_string(raw) else {
        return Ok(None);
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(value) => Ok(Some(value)),
        Err(_) => Ok(Some(Value::String(raw))),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn mock_member_specs(member_specs: &[(&str, &str)]) -> Vec<TeamMemberSpecView> {
        member_specs
            .iter()
            .map(|(member_id, role)| TeamMemberSpecView {
                member_id: (*member_id).to_string(),
                role: (*role).to_string(),
                description: None,
            })
            .collect()
    }

    fn build_mailbox_record(
        message_id: i64,
        from_actor_id: &str,
        to_actor_id: &str,
        payload: Value,
    ) -> TeamActorMessageRecord {
        TeamActorMessageRecord {
            message_id,
            run_id: "run-1".to_string(),
            from_actor_id: from_actor_id.to_string(),
            from_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            from_actor_kind: agenthub_team_actor::infer_actor_identity_kind(from_actor_id),
            to_actor_id: to_actor_id.to_string(),
            to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            to_actor_kind: agenthub_team_actor::infer_actor_identity_kind(to_actor_id),
            channel: "default".to_string(),
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: payload.clone(),
            idempotency_key: None,
            message_kind: infer_actor_message_kind(from_actor_id, &payload, None),
            status: TeamActorMessageStatus::Pending,
            handling_disposition: ActorMessageHandlingDisposition::Untriaged,
            handled_by_actor_id: None,
            thread_topic_key: None,
            thread_claim_status: None,
            thread_owner_actor_id: None,
            thread_lease_expires_at: None,
            linked_task_id: None,
            linked_task_relation: None,
            created_at: 1_700_000_000 + message_id,
            delivered_at: None,
            handled_at: None,
        }
    }

    #[test]
    fn normalize_channel_message_payload_wraps_non_object_inputs() {
        let text = normalize_channel_message_payload(Value::String("hello".to_string()));
        let number = normalize_channel_message_payload(json!(42));

        assert_eq!(
            text,
            json!({
                "type": "chat_message",
                "text": "hello"
            })
        );
        assert_eq!(
            number,
            json!({
                "type": "chat_message",
                "text": "42"
            })
        );
    }

    #[test]
    fn ensure_channel_message_correlation_id_preserves_existing_or_uses_fallback() {
        let existing = ensure_channel_message_correlation_id(
            json!({
                "type": "chat_message",
                "text": "hello",
                "correlation_id": "corr-existing"
            }),
            Some("corr-fallback"),
        );
        let fallback = ensure_channel_message_correlation_id(
            json!({
                "type": "chat_message",
                "text": "hello"
            }),
            Some("corr-fallback"),
        );

        assert_eq!(
            channel_payload_correlation_id(&existing),
            Some("corr-existing")
        );
        assert_eq!(
            channel_payload_correlation_id(&fallback),
            Some("corr-fallback")
        );
    }

    #[test]
    fn map_actor_service_error_surfaces_readonly_database_failures() {
        let mapped =
            map_actor_service_error(anyhow::anyhow!("attempt to write a readonly database"));

        assert_eq!(mapped.code, ActorServiceErrorCode::Internal);
        assert_eq!(
            mapped.message,
            "mailbox write failed: attempt to write a readonly database"
        );
    }

    #[test]
    fn human_visible_chat_reply_requires_local_non_human_to_human_chat_message() {
        let payload = json!({
            "type": "chat_message",
            "text": "final answer"
        });

        assert!(should_persist_human_visible_chat_reply_for_payload(
            &agenthub_team_actor::ActorMessageTransport::Local,
            "user",
            ACTOR_MAIN_PEER_ID,
            "worker",
            &payload,
        ));
        assert!(!should_persist_human_visible_chat_reply_for_payload(
            &agenthub_team_actor::ActorMessageTransport::Remote,
            "user",
            ACTOR_MAIN_PEER_ID,
            "worker",
            &payload,
        ));
        assert!(!should_persist_human_visible_chat_reply_for_payload(
            &agenthub_team_actor::ActorMessageTransport::Local,
            "reviewer",
            ACTOR_MAIN_PEER_ID,
            "worker",
            &payload,
        ));
        assert!(!should_persist_human_visible_chat_reply_for_payload(
            &agenthub_team_actor::ActorMessageTransport::Local,
            "user:123",
            ACTOR_MAIN_PEER_ID,
            "user",
            &payload,
        ));
        assert!(!should_persist_human_visible_chat_reply_for_payload(
            &agenthub_team_actor::ActorMessageTransport::Local,
            "user",
            ACTOR_MAIN_PEER_ID,
            "worker",
            &json!({
                "type": "tool_result",
                "text": "not a chat"
            }),
        ));
    }

    #[test]
    fn resolve_canonical_chat_reply_accepts_stringified_json_chat_message() {
        let reply = resolve_canonical_chat_reply(&Value::String(
            r#"{"type":"chat_message","text":"hello","correlation_id":"corr-9"}"#.to_string(),
        ))
        .expect("resolve chat reply");

        assert_eq!(reply.text, "hello");
        assert_eq!(reply.correlation_id.as_deref(), Some("corr-9"));
    }

    #[test]
    fn summarize_open_reply_obligations_counts_unanswered_human_requests() {
        let messages = vec![build_mailbox_record(
            1,
            "user",
            "worker",
            json!({
                "type": "chat_message",
                "text": "Need update",
                "requires_user_visible_reply": true
            }),
        )];

        let summary = summarize_open_reply_obligations_from_messages(messages.as_slice());

        assert_eq!(summary.open_total, 1);
        assert_eq!(summary.open_by_actor.get("worker").copied(), Some(1));
        assert_eq!(summary.open_items.len(), 1);
        let obligation = &summary.open_items[0];
        assert_eq!(obligation.agent_actor_id, "worker");
        assert_eq!(obligation.human_actor_id, "user");
        assert_eq!(obligation.source_surface, "mailbox");
        assert_eq!(obligation.text_excerpt.as_deref(), Some("Need update"));
    }

    #[test]
    fn summarize_open_reply_obligations_consumes_visible_reply_credit() {
        let mut inbound = build_mailbox_record(
            1,
            "user",
            "worker",
            json!({
                "type": "chat_message",
                "text": "Need update",
                "requires_user_visible_reply": true
            }),
        );
        inbound.status = TeamActorMessageStatus::Delivered;
        let outbound = build_mailbox_record(
            2,
            "worker",
            "user",
            json!({
                "type": "chat_message",
                "text": "Here is the update"
            }),
        );

        let summary = summarize_open_reply_obligations_from_messages(&[inbound, outbound]);

        assert_eq!(summary.open_total, 0);
        assert!(summary.open_by_actor.is_empty());
    }

    #[test]
    fn summarize_open_reply_obligations_skips_ignored_and_completed_items() {
        let mut ignored = build_mailbox_record(
            1,
            "user",
            "worker",
            json!({
                "type": "chat_message",
                "text": "Need update",
                "requires_user_visible_reply": true
            }),
        );
        ignored.handling_disposition = ActorMessageHandlingDisposition::Ignored;
        let mut completed = build_mailbox_record(
            2,
            "user",
            "reviewer",
            json!({
                "type": "chat_message",
                "text": "Please review",
                "requires_user_visible_reply": true
            }),
        );
        completed.handling_disposition = ActorMessageHandlingDisposition::Completed;

        let summary = summarize_open_reply_obligations_from_messages(&[ignored, completed]);

        assert_eq!(summary.open_total, 0);
        assert!(summary.open_by_actor.is_empty());
    }

    #[test]
    fn validate_direct_mailbox_target_rejects_role_alias() {
        let member_specs = mock_member_specs(&[
            ("595d1ae8-fcbd-4111-b5c7-d446a12c044b", "coordinator"),
            ("c319f933-1358-4418-a111-872304052422", "worker"),
        ]);
        let err = validate_direct_mailbox_target_for_member_specs(&member_specs, "coordinator")
            .expect_err("role alias should be rejected");
        assert_eq!(err.code, ActorServiceErrorCode::BadRequest);
        assert!(err.message.contains("not a canonical team member_id"));
        assert!(err.message.contains("595d1ae8-fcbd-4111-b5c7-d446a12c044b"));
    }

    #[test]
    fn validate_direct_mailbox_target_allows_member_id_and_human_mailbox() {
        let member_specs = mock_member_specs(&[
            ("595d1ae8-fcbd-4111-b5c7-d446a12c044b", "coordinator"),
            ("c319f933-1358-4418-a111-872304052422", "worker"),
        ]);
        validate_direct_mailbox_target_for_member_specs(
            &member_specs,
            "595d1ae8-fcbd-4111-b5c7-d446a12c044b",
        )
        .expect("member id should be accepted");
        validate_direct_mailbox_target_for_member_specs(&member_specs, "user")
            .expect("human alias should be accepted");
        validate_direct_mailbox_target_for_member_specs(&member_specs, "user:test")
            .expect("human actor target should be accepted");
    }

    #[test]
    fn validate_direct_mailbox_target_rejects_empty_human_mailbox_suffix() {
        let member_specs = mock_member_specs(&[
            ("595d1ae8-fcbd-4111-b5c7-d446a12c044b", "coordinator"),
            ("c319f933-1358-4418-a111-872304052422", "worker"),
        ]);
        let err = validate_direct_mailbox_target_for_member_specs(&member_specs, "user:")
            .expect_err("human mailbox target with empty suffix should be rejected");
        assert_eq!(err.code, ActorServiceErrorCode::BadRequest);
        assert!(
            err.message
                .contains("must reference spec.members[].member_id")
        );
    }

    #[test]
    fn validate_direct_mailbox_target_rejects_ambiguous_role_alias() {
        let member_specs = mock_member_specs(&[
            ("planner", "coordinator"),
            ("worker-a", "worker"),
            ("worker-b", "worker"),
        ]);
        let err = validate_direct_mailbox_target_for_member_specs(&member_specs, "worker")
            .expect_err("ambiguous role alias should be rejected");
        assert_eq!(err.code, ActorServiceErrorCode::BadRequest);
        assert!(err.message.contains("matches multiple team members"));
        assert!(err.message.contains("worker-a"));
        assert!(err.message.contains("worker-b"));
    }
}
