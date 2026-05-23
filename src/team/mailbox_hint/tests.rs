use std::collections::HashSet;

use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ActorMessageRecord, ActorMessageStatus, ActorMessageTransport,
};
use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::immediate::collect_channel_mention_actor_ids;
use super::types::{
    ActorMailboxImmediateHintPlan, ActorMailboxImmediateHintReason, ActorMailboxPriorityClass,
    RunningActorRuntime, TeamMailboxHintAgentNudger, actor_mailbox_priority_label,
};
use super::worker::{IdleUnreadHintState, actor_mailbox_is_idle, decide_idle_unread_hint_action};
use super::{build_actor_mailbox_immediate_hint_prompt, dispatch_actor_mailbox_immediate_hint};
use crate::team::mailbox_hint::prompts::build_actor_mailbox_unread_summary_prompt;

fn is_user_message_payload(message: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(message)
        .ok()
        .and_then(|payload| {
            payload
                .get("type")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .is_some_and(|value| value == "user_message")
}

#[derive(Default)]
struct RecordingNudger {
    sent: Mutex<Vec<String>>,
    failed: Mutex<HashSet<String>>,
}

impl RecordingNudger {
    fn failing(targets: impl IntoIterator<Item = &'static str>) -> Self {
        Self {
            sent: Mutex::default(),
            failed: Mutex::new(targets.into_iter().map(str::to_string).collect()),
        }
    }
}

#[async_trait]
impl TeamMailboxHintAgentNudger for RecordingNudger {
    async fn running_actor_runtime(&self, _actor_id: &str) -> Option<RunningActorRuntime> {
        None
    }

    async fn mailbox_idle_anchor_ts(
        &self,
        _actor_id: &str,
        _session_id: &str,
    ) -> anyhow::Result<Option<i64>> {
        Ok(None)
    }

    async fn nudge_mailbox_prompt(
        &self,
        actor_id: &str,
        _expected_session_id: Option<&str>,
        _prompt: &str,
    ) -> anyhow::Result<()> {
        if self.failed.lock().await.contains(actor_id) {
            anyhow::bail!("busy");
        }
        self.sent.lock().await.push(actor_id.to_string());
        Ok(())
    }
}

fn actor_send_response(
    from_actor_kind: agenthub_team_actor::ActorIdentityKind,
    to_actor_id: &str,
    payload: Value,
) -> agenthub_team_actor::ActorSendResponse {
    agenthub_team_actor::ActorSendResponse {
        message_id: 11,
        state: ActorMessageStatus::Pending,
        deduped: false,
        created_at: 100,
        message: ActorMessageRecord {
            message_id: 11,
            run_id: "run-1".to_string(),
            from_actor_id: "planner".to_string(),
            from_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            from_actor_kind,
            to_actor_id: to_actor_id.to_string(),
            to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            to_actor_kind: agenthub_team_actor::ActorIdentityKind::Agent,
            channel: "default".to_string(),
            transport: ActorMessageTransport::Local,
            route: None,
            payload,
            idempotency_key: None,
            message_kind: agenthub_team_actor::ActorMessageKind::CoordinationRequest,
            status: ActorMessageStatus::Pending,
            handling_disposition: agenthub_team_actor::ActorMessageHandlingDisposition::Untriaged,
            handled_by_actor_id: None,
            thread_topic_key: None,
            thread_claim_status: None,
            thread_owner_actor_id: None,
            thread_lease_expires_at: None,
            linked_task_id: None,
            linked_task_relation: None,
            handled_at: None,
            created_at: 100,
            delivered_at: None,
        },
    }
}

#[test]
fn collect_channel_mentions_deduplicates_and_skips_sender() {
    let mentions = collect_channel_mention_actor_ids(
        &json!({
            "channel_id": "all",
            "mentioned_actor_ids": ["reviewer", "planner", "reviewer", "worker"]
        }),
        "planner",
    );
    assert_eq!(mentions, vec!["reviewer".to_string(), "worker".to_string()]);
}

#[test]
fn build_immediate_prompt_is_compact() {
    let prompt = build_actor_mailbox_immediate_hint_prompt(
        "run-42",
        ActorMailboxImmediateHintReason::DirectAgentMessage,
    );
    assert!(prompt.contains("Direct mailbox message pending"));
    assert!(prompt.contains("run-42"));
    assert!(prompt.contains("agenthub actor inbox"));
}

#[test]
fn build_unread_summary_prompt_includes_count() {
    let prompt = build_actor_mailbox_unread_summary_prompt("run-7", 3);
    assert!(prompt.contains("3 unread"));
    assert!(prompt.contains("run-7"));
}

#[test]
fn actor_mailbox_priority_classes_are_stable() {
    assert_eq!(
        actor_mailbox_priority_label(ActorMailboxPriorityClass::General),
        "general"
    );
    assert_eq!(
        actor_mailbox_priority_label(ActorMailboxPriorityClass::Urgent),
        "urgent"
    );
    assert_eq!(
        actor_mailbox_priority_label(ActorMailboxPriorityClass::PermissionReview),
        "permission_review"
    );
}

#[test]
fn actor_mailbox_is_idle_respects_threshold() {
    assert!(actor_mailbox_is_idle(400, 180, 200));
    assert!(!actor_mailbox_is_idle(250, 180, 200));
}

#[test]
fn decide_idle_unread_hint_action_requires_threshold() {
    let action = decide_idle_unread_hint_action(400, 180, "session-1", 200, 2, None)
        .expect("idle unread prompt should trigger");
    assert_eq!(action.session_id, "session-1");
    assert_eq!(action.unread_count, 2);
    assert!(
        decide_idle_unread_hint_action(250, 180, "session-1", 200, 2, None).is_none(),
        "threshold should suppress early prompt"
    );
    assert!(
        decide_idle_unread_hint_action(300, 180, "session-1", 200, 0, None).is_none(),
        "zero unread should suppress prompt"
    );
}

#[test]
fn decide_idle_unread_hint_action_dedupes_same_idle_window() {
    let previous = IdleUnreadHintState {
        session_id: "session-1".to_string(),
        idle_anchor_ts: 200,
        unread_count: 2,
    };
    assert!(
        decide_idle_unread_hint_action(400, 180, "session-1", 200, 2, Some(&previous)).is_none()
    );
    assert!(
        decide_idle_unread_hint_action(400, 180, "session-1", 200, 3, Some(&previous)).is_some(),
        "count change should retrigger"
    );
    assert!(
        decide_idle_unread_hint_action(400, 180, "session-2", 200, 2, Some(&previous)).is_some(),
        "session change should retrigger"
    );
    assert!(
        decide_idle_unread_hint_action(400, 180, "session-1", 260, 2, Some(&previous)).is_none(),
        "new idle anchor still needs to satisfy threshold"
    );
    assert!(
        decide_idle_unread_hint_action(500, 180, "session-1", 260, 2, Some(&previous)).is_some(),
        "new idle anchor after threshold should retrigger"
    );
}

#[test]
fn detect_user_message_payload() {
    assert!(is_user_message_payload(
        r#"{"type":"user_message","text":"ping"}"#
    ));
    assert!(!is_user_message_payload(
        r#"{"type":"agent_message","text":"pong"}"#
    ));
    assert!(!is_user_message_payload("not-json"));
}

#[test]
fn direct_agent_message_shape_matches_immediate_contract() {
    let response = actor_send_response(
        agenthub_team_actor::ActorIdentityKind::Agent,
        "reviewer",
        json!({"type":"chat_message","text":"hello"}),
    );
    let plan = ActorMailboxImmediateHintPlan {
        target_actor_ids: vec![response.message.to_actor_id.clone()],
        reason: ActorMailboxImmediateHintReason::DirectAgentMessage,
    };
    assert_eq!(plan.target_actor_ids, vec!["reviewer".to_string()]);
}

#[tokio::test]
async fn immediate_mailbox_hint_dispatch_uses_best_effort_nudger() {
    let nudger = RecordingNudger::failing(["busy-worker"]);
    let plan = ActorMailboxImmediateHintPlan {
        target_actor_ids: vec!["idle-worker".to_string(), "busy-worker".to_string()],
        reason: ActorMailboxImmediateHintReason::DirectAgentMessage,
    };

    let delivery = dispatch_actor_mailbox_immediate_hint(&nudger, "run-1", &plan).await;

    assert_eq!(delivery.sent_actor_ids, vec!["idle-worker".to_string()]);
    assert_eq!(delivery.failed_actor_ids, vec!["busy-worker".to_string()]);
    assert_eq!(*nudger.sent.lock().await, vec!["idle-worker".to_string()]);
}
