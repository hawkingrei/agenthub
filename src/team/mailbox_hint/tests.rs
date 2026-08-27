use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

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
use super::{
    TeamMailboxRuntimeDeliveryWorker, TeamMailboxRuntimeDeliveryWorkerSettings,
    build_actor_mailbox_immediate_hint_prompt,
};
use crate::team::mailbox_hint::prompts::build_actor_mailbox_unread_summary_prompt;
use crate::team::{TeamManager, mailbox_runtime_delivery_id};

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
    runtimes: HashMap<String, RunningActorRuntime>,
    sent: Mutex<Vec<(String, String)>>,
    failed: Mutex<HashSet<String>>,
    blocked: HashSet<String>,
}

impl RecordingNudger {
    fn failing(targets: impl IntoIterator<Item = &'static str>) -> Self {
        Self {
            runtimes: HashMap::from([
                (
                    "idle-worker".to_string(),
                    RunningActorRuntime {
                        session_id: "session-idle".to_string(),
                        current_run_id: Some("run-1".to_string()),
                    },
                ),
                (
                    "busy-worker".to_string(),
                    RunningActorRuntime {
                        session_id: "session-busy".to_string(),
                        current_run_id: Some("run-1".to_string()),
                    },
                ),
            ]),
            sent: Mutex::default(),
            failed: Mutex::new(targets.into_iter().map(str::to_string).collect()),
            blocked: HashSet::new(),
        }
    }

    fn blocking(target: &'static str) -> Self {
        let mut nudger = Self::failing([]);
        nudger.blocked.insert(target.to_string());
        nudger
    }
}

#[async_trait]
impl TeamMailboxHintAgentNudger for RecordingNudger {
    async fn running_actor_runtime(&self, actor_id: &str) -> Option<RunningActorRuntime> {
        self.runtimes.get(actor_id).cloned()
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
        delivery_id: &str,
        _prompt: &str,
    ) -> anyhow::Result<()> {
        if self.blocked.contains(actor_id) {
            std::future::pending::<()>().await;
        }
        if self.failed.lock().await.contains(actor_id) {
            anyhow::bail!("busy");
        }
        self.sent
            .lock()
            .await
            .push((actor_id.to_string(), delivery_id.to_string()));
        Ok(())
    }
}

async fn setup_runtime_delivery_manager() -> Arc<TeamManager> {
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect runtime delivery test database");
    for statement in [
        r#"
        CREATE TABLE team_definitions (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            spec_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )
        "#,
        r#"
        CREATE TABLE team_runs (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            context_id TEXT NOT NULL,
            status TEXT NOT NULL,
            input_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id)
        )
        "#,
        r#"
        CREATE TABLE team_actor_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            from_actor_id TEXT NOT NULL,
            to_actor_id TEXT NOT NULL,
            channel TEXT NOT NULL,
            transport TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        )
        "#,
        r#"
        CREATE TABLE team_runtime_delivery_receipts (
            delivery_id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            message_id INTEGER NOT NULL,
            actor_id TEXT NOT NULL,
            prompt TEXT NOT NULL,
            state TEXT NOT NULL CHECK (state IN ('pending', 'in_flight', 'delivered')),
            attempt INTEGER NOT NULL DEFAULT 0 CHECK (attempt >= 0),
            next_retry_at INTEGER,
            lease_expires_at INTEGER,
            last_error TEXT,
            session_id TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            delivered_at INTEGER,
            UNIQUE(run_id, message_id, actor_id),
            FOREIGN KEY(run_id) REFERENCES team_runs(id),
            FOREIGN KEY(message_id) REFERENCES team_actor_messages(id) ON DELETE CASCADE
        )
        "#,
    ] {
        sqlx::query(statement)
            .execute(&db)
            .await
            .expect("create runtime delivery test schema");
    }
    sqlx::query(
        r#"
        INSERT INTO team_definitions (id, name, spec_json, created_at, updated_at)
        VALUES ('team-1', 'Runtime Delivery Team', '{}', 100, 100)
        "#,
    )
    .execute(&db)
    .await
    .expect("insert runtime delivery team");
    sqlx::query(
        r#"
        INSERT INTO team_runs (id, team_id, context_id, status, input_json, created_at)
        VALUES ('run-1', 'team-1', 'ctx-1', 'running', '{}', 100)
        "#,
    )
    .execute(&db)
    .await
    .expect("insert runtime delivery run");
    sqlx::query(
        r#"
        INSERT INTO team_actor_messages (
            id, run_id, from_actor_id, to_actor_id, channel, transport, payload_json, status,
            created_at
        )
        VALUES (11, 'run-1', 'planner', 'idle-worker', 'default', 'local', '{}', 'pending', 100)
        "#,
    )
    .execute(&db)
    .await
    .expect("insert runtime delivery mailbox message");
    Arc::new(TeamManager::new(db))
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
async fn runtime_delivery_receipts_retry_with_stable_ids_after_restart() {
    let teams = setup_runtime_delivery_manager().await;
    let nudger = Arc::new(RecordingNudger::failing(["busy-worker"]));
    let worker = TeamMailboxRuntimeDeliveryWorker::with_agent_nudger(teams.clone(), nudger.clone());
    let plan = ActorMailboxImmediateHintPlan {
        target_actor_ids: vec!["idle-worker".to_string(), "busy-worker".to_string()],
        reason: ActorMailboxImmediateHintReason::DirectAgentMessage,
    };

    let delivery = worker
        .enqueue_and_dispatch("run-1", 11, &plan)
        .await
        .expect("enqueue runtime deliveries");

    assert_eq!(delivery.sent_actor_ids, vec!["idle-worker".to_string()]);
    assert_eq!(delivery.failed_actor_ids, vec!["busy-worker".to_string()]);
    let idle_delivery_id = mailbox_runtime_delivery_id("run-1", 11, "idle-worker");
    let busy_delivery_id = mailbox_runtime_delivery_id("run-1", 11, "busy-worker");
    assert_eq!(
        *nudger.sent.lock().await,
        vec![("idle-worker".to_string(), idle_delivery_id.clone())]
    );
    let delivered = teams
        .mailbox_runtime_delivery_for_test(&idle_delivery_id)
        .await
        .expect("read delivered receipt");
    assert_eq!(delivered.state, "delivered");
    assert_eq!(delivered.session_id.as_deref(), Some("session-idle"));
    let pending = teams
        .mailbox_runtime_delivery_for_test(&busy_delivery_id)
        .await
        .expect("read retry receipt");
    assert_eq!(pending.state, "pending");
    assert_eq!(pending.attempt, 1);
    assert!(pending.next_retry_at.is_some());

    nudger.failed.lock().await.remove("busy-worker");
    let restarted =
        TeamMailboxRuntimeDeliveryWorker::with_agent_nudger(teams.clone(), nudger.clone());
    let retried = restarted
        .dispatch_once_at(
            TeamMailboxRuntimeDeliveryWorkerSettings {
                poll_interval: Duration::from_millis(10),
                batch_size: 10,
                lease_seconds: 30,
            },
            pending.next_retry_at.expect("retry deadline"),
        )
        .await
        .expect("dispatch recovered receipt");
    assert_eq!(retried.delivery_ids, vec![busy_delivery_id.clone()]);
    assert_eq!(retried.sent_actor_ids, vec!["busy-worker".to_string()]);
    let recovered = teams
        .mailbox_runtime_delivery_for_test(&busy_delivery_id)
        .await
        .expect("read recovered receipt");
    assert_eq!(recovered.state, "delivered");
    assert_eq!(recovered.attempt, 2);
    assert_eq!(recovered.session_id.as_deref(), Some("session-busy"));
    assert!(
        !teams
            .acknowledge_mailbox_runtime_delivery(
                &busy_delivery_id,
                recovered.attempt,
                "different-session",
                recovered.delivered_at.expect("delivered timestamp") + 1,
            )
            .await
            .expect("idempotent runtime acknowledgement")
    );
}

#[tokio::test]
async fn runtime_delivery_receipts_fence_stale_attempts_after_lease_expiry() {
    let teams = setup_runtime_delivery_manager().await;
    let actor_ids = vec!["idle-worker".to_string()];
    let receipts = teams
        .ensure_mailbox_runtime_deliveries("run-1", 11, &actor_ids, "prompt", 100)
        .await
        .expect("persist runtime delivery");
    let delivery_id = receipts[0].delivery_id.clone();
    let first = teams
        .claim_mailbox_runtime_delivery(&delivery_id, 100, 30)
        .await
        .expect("claim first attempt")
        .expect("first attempt is due");
    assert!(
        teams
            .claim_mailbox_runtime_delivery(&delivery_id, 129, 30)
            .await
            .expect("check active lease")
            .is_none()
    );
    let second = teams
        .claim_mailbox_runtime_delivery(&delivery_id, 130, 30)
        .await
        .expect("reclaim expired delivery")
        .expect("expired lease is due");
    assert_eq!(second.attempt, first.attempt + 1);
    assert!(
        !teams
            .retry_mailbox_runtime_delivery(&delivery_id, first.attempt, 131, 132, "stale failure",)
            .await
            .expect("fence stale retry")
    );
    assert!(
        !teams
            .acknowledge_mailbox_runtime_delivery(
                &delivery_id,
                first.attempt,
                "stale-session",
                131,
            )
            .await
            .expect("fence stale acknowledgement")
    );
    assert!(
        teams
            .acknowledge_mailbox_runtime_delivery(
                &delivery_id,
                second.attempt,
                "current-session",
                132,
            )
            .await
            .expect("acknowledge current attempt")
    );
    let delivered = teams
        .mailbox_runtime_delivery_for_test(&delivery_id)
        .await
        .expect("read fenced receipt");
    assert_eq!(delivered.state, "delivered");
    assert_eq!(delivered.session_id.as_deref(), Some("current-session"));
}

#[tokio::test]
async fn runtime_delivery_receipts_timeout_blocked_input_and_retry() {
    let teams = setup_runtime_delivery_manager().await;
    let actor_ids = vec!["idle-worker".to_string()];
    let receipts = teams
        .ensure_mailbox_runtime_deliveries("run-1", 11, &actor_ids, "prompt", 100)
        .await
        .expect("persist runtime delivery");
    let delivery_id = receipts[0].delivery_id.clone();
    let nudger = Arc::new(RecordingNudger::blocking("idle-worker"));
    let worker = TeamMailboxRuntimeDeliveryWorker::with_agent_nudger(teams.clone(), nudger);

    let delivery = worker
        .dispatch_once_at(
            TeamMailboxRuntimeDeliveryWorkerSettings {
                poll_interval: Duration::from_millis(10),
                batch_size: 10,
                lease_seconds: 1,
            },
            100,
        )
        .await
        .expect("bound blocked runtime input");

    assert_eq!(delivery.failed_actor_ids, vec!["idle-worker".to_string()]);
    let pending = teams
        .mailbox_runtime_delivery_for_test(&delivery_id)
        .await
        .expect("read timed out receipt");
    assert_eq!(pending.state, "pending");
    assert_eq!(pending.attempt, 1);
    assert_eq!(
        pending.last_error.as_deref(),
        Some("runtime input delivery timed out")
    );
}
