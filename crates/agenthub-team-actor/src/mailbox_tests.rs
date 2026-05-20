use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};
use thiserror::Error;
use tokio::sync::Mutex;

use super::*;
use crate::{ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, infer_actor_identity_kind};

#[derive(Debug, Error)]
#[error("{0}")]
struct TestStoreError(String);

#[derive(Debug, Clone)]
struct StoredMessage {
    record: ActorMessageRecord,
    relay_attempt: i64,
    relay_next_retry_at: Option<i64>,
    relay_last_error: Option<String>,
    dead_letter_at: Option<i64>,
}

#[derive(Debug, Clone, Default)]
struct EventRecord {
    run_id: String,
    event_type: String,
}

#[derive(Debug, Default)]
struct StoreState {
    next_message_id: i64,
    messages: Vec<StoredMessage>,
    events: Vec<EventRecord>,
    idempotency_index: HashMap<(String, String, String, String), i64>,
}

#[derive(Debug, Clone)]
struct MessageSnapshot {
    record: ActorMessageRecord,
    relay_attempt: i64,
    relay_next_retry_at: Option<i64>,
    relay_last_error: Option<String>,
    dead_letter_at: Option<i64>,
}

#[derive(Clone, Default)]
struct TestStore {
    state: Arc<Mutex<StoreState>>,
}

impl TestStore {
    async fn event_types(&self, run_id: &str) -> Vec<String> {
        let state = self.state.lock().await;
        state
            .events
            .iter()
            .filter(|event| event.run_id == run_id)
            .map(|event| event.event_type.clone())
            .collect()
    }

    async fn event_count(&self, run_id: &str, event_type: &str) -> usize {
        let state = self.state.lock().await;
        state
            .events
            .iter()
            .filter(|event| event.run_id == run_id && event.event_type == event_type)
            .count()
    }

    async fn snapshot(&self, message_id: i64) -> MessageSnapshot {
        let state = self.state.lock().await;
        let found = state
            .messages
            .iter()
            .find(|message| message.record.message_id == message_id)
            .cloned()
            .expect("message not found");
        MessageSnapshot {
            record: found.record,
            relay_attempt: found.relay_attempt,
            relay_next_retry_at: found.relay_next_retry_at,
            relay_last_error: found.relay_last_error,
            dead_letter_at: found.dead_letter_at,
        }
    }
}

#[async_trait]
impl ActorMailboxStore for TestStore {
    type Error = TestStoreError;

    async fn create_pending_message(
        &self,
        cmd: &SendActorMessageCommand,
    ) -> Result<CreatePendingMessageResult, Self::Error> {
        let mut state = self.state.lock().await;
        if let Some(idempotency_key) = cmd.idempotency_key.as_deref() {
            let dedupe_key = (
                cmd.run_id.clone(),
                cmd.from_actor_id.clone(),
                cmd.from_peer_id.clone(),
                idempotency_key.to_string(),
            );
            if let Some(existing_id) = state.idempotency_index.get(&dedupe_key).copied() {
                let existing = state
                    .messages
                    .iter()
                    .find(|entry| entry.record.message_id == existing_id)
                    .ok_or_else(|| {
                        TestStoreError("idempotency index points to missing message".to_string())
                    })?;
                return Ok(CreatePendingMessageResult {
                    message: existing.record.clone(),
                    created: false,
                });
            }
        }

        state.next_message_id += 1;
        let message = ActorMessageRecord {
            message_id: state.next_message_id,
            run_id: cmd.run_id.clone(),
            from_actor_id: cmd.from_actor_id.clone(),
            from_peer_id: cmd.from_peer_id.clone(),
            from_actor_kind: infer_actor_identity_kind(cmd.from_actor_id.as_str()),
            to_actor_id: cmd.to_actor_id.clone(),
            to_peer_id: cmd.to_peer_id.clone(),
            to_actor_kind: infer_actor_identity_kind(cmd.to_actor_id.as_str()),
            channel: cmd.channel.clone(),
            transport: cmd.transport.clone(),
            route: cmd.route.clone(),
            payload: cmd.payload.clone(),
            message_kind: cmd.message_kind.clone(),
            status: ActorMessageStatus::Pending,
            handling_disposition: super::ActorMessageHandlingDisposition::Untriaged,
            handled_by_actor_id: None,
            thread_topic_key: None,
            thread_claim_status: None,
            thread_owner_actor_id: None,
            thread_lease_expires_at: None,
            linked_task_id: None,
            linked_task_relation: None,
            handled_at: None,
            created_at: cmd.created_at,
            delivered_at: None,
        };
        if let Some(idempotency_key) = cmd.idempotency_key.as_deref() {
            state.idempotency_index.insert(
                (
                    cmd.run_id.clone(),
                    cmd.from_actor_id.clone(),
                    cmd.from_peer_id.clone(),
                    idempotency_key.to_string(),
                ),
                message.message_id,
            );
        }
        state.messages.push(StoredMessage {
            record: message.clone(),
            relay_attempt: 0,
            relay_next_retry_at: None,
            relay_last_error: None,
            dead_letter_at: None,
        });
        Ok(CreatePendingMessageResult {
            message,
            created: true,
        })
    }

    async fn list_inbox(
        &self,
        query: &ListActorInboxQuery,
    ) -> Result<Vec<ActorMessageRecord>, Self::Error> {
        let state = self.state.lock().await;
        let mut records: Vec<ActorMessageRecord> = state
            .messages
            .iter()
            .filter(|entry| entry.record.run_id == query.run_id)
            .filter(|entry| entry.record.to_actor_id == query.actor_id)
            .filter(|entry| entry.record.to_peer_id == query.peer_id)
            .filter(|entry| {
                query.include_delivered || entry.record.status == ActorMessageStatus::Pending
            })
            .filter(|entry| {
                query
                    .after_id
                    .is_none_or(|after| entry.record.message_id > after)
            })
            .map(|entry| entry.record.clone())
            .collect();
        records.sort_by_key(|entry| entry.message_id);
        let limit = usize::try_from(query.limit.max(1)).unwrap_or(usize::MAX);
        records.truncate(limit);
        Ok(records)
    }

    async fn ack_message(
        &self,
        cmd: &AckActorMessageCommand,
    ) -> Result<AckActorMessageResult, Self::Error> {
        let mut state = self.state.lock().await;
        let stored = state
            .messages
            .iter_mut()
            .find(|entry| {
                entry.record.run_id == cmd.run_id
                    && entry.record.message_id == cmd.message_id
                    && entry.record.to_actor_id == cmd.actor_id
                    && entry.record.to_peer_id == cmd.peer_id
            })
            .ok_or_else(|| TestStoreError("message not found".to_string()))?;

        let mut status_changed = false;
        if stored.record.status == ActorMessageStatus::Pending {
            stored.record.status = ActorMessageStatus::Delivered;
            stored.record.delivered_at =
                Some(stored.record.delivered_at.unwrap_or(cmd.delivered_at));
            status_changed = true;
        }

        Ok(AckActorMessageResult {
            message: stored.record.clone(),
            status_changed,
        })
    }

    async fn triage_message(
        &self,
        cmd: &TriageActorMessageCommand,
    ) -> Result<TriageActorMessageResult, Self::Error> {
        let mut state = self.state.lock().await;
        let stored = state
            .messages
            .iter_mut()
            .find(|entry| {
                entry.record.run_id == cmd.run_id
                    && entry.record.message_id == cmd.message_id
                    && entry.record.to_actor_id == cmd.actor_id
                    && entry.record.to_peer_id == cmd.peer_id
            })
            .ok_or_else(|| TestStoreError("message not found".to_string()))?;
        let handling_changed = stored.record.handling_disposition != cmd.disposition;
        stored.record.handling_disposition = cmd.disposition.clone();
        stored.record.handled_by_actor_id = Some(cmd.actor_id.clone());
        stored.record.handled_at = Some(cmd.handled_at);
        Ok(TriageActorMessageResult {
            message: stored.record.clone(),
            handling_changed,
        })
    }

    async fn link_message_task(
        &self,
        cmd: &LinkActorMessageTaskCommand,
    ) -> Result<LinkActorMessageTaskResult, Self::Error> {
        let mut state = self.state.lock().await;
        let stored = state
            .messages
            .iter_mut()
            .find(|entry| {
                entry.record.run_id == cmd.run_id
                    && entry.record.message_id == cmd.message_id
                    && entry.record.to_actor_id == cmd.actor_id
                    && entry.record.to_peer_id == cmd.peer_id
            })
            .ok_or_else(|| TestStoreError("message not found".to_string()))?;
        let created = stored.record.linked_task_id.as_deref() != Some(cmd.task_id.as_str())
            || stored.record.linked_task_relation.as_ref() != Some(&cmd.relation);
        stored.record.linked_task_id = Some(cmd.task_id.clone());
        stored.record.linked_task_relation = Some(cmd.relation.clone());
        Ok(LinkActorMessageTaskResult {
            message: stored.record.clone(),
            task_id: cmd.task_id.clone(),
            relation: cmd.relation.clone(),
            created,
        })
    }

    async fn list_remote_pending_messages(
        &self,
        limit: i64,
        now: i64,
    ) -> Result<Vec<PendingRemoteRelayRecord>, Self::Error> {
        let state = self.state.lock().await;
        let mut records: Vec<PendingRemoteRelayRecord> = state
            .messages
            .iter()
            .filter(|entry| entry.record.transport == ActorMessageTransport::Remote)
            .filter(|entry| entry.record.status == ActorMessageStatus::Pending)
            .filter(|entry| {
                entry
                    .relay_next_retry_at
                    .is_none_or(|retry_at| retry_at <= now)
            })
            .map(|entry| PendingRemoteRelayRecord {
                message: entry.record.clone(),
                attempt: entry.relay_attempt,
            })
            .collect();
        records.sort_by_key(|entry| entry.message.message_id);
        let limit = usize::try_from(limit.max(1)).unwrap_or(usize::MAX);
        records.truncate(limit);
        Ok(records)
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
        let mut state = self.state.lock().await;
        if let Some(stored) = state.messages.iter_mut().find(|entry| {
            entry.record.run_id == run_id
                && entry.record.message_id == message_id
                && entry.record.transport == ActorMessageTransport::Remote
                && entry.record.status == ActorMessageStatus::Pending
        }) {
            stored.relay_attempt = attempt;
            stored.relay_next_retry_at = Some(next_retry_at.max(ts));
            stored.relay_last_error = Some(error.to_string());
        }
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
        let mut state = self.state.lock().await;
        if let Some(stored) = state.messages.iter_mut().find(|entry| {
            entry.record.run_id == run_id
                && entry.record.message_id == message_id
                && entry.record.transport == ActorMessageTransport::Remote
                && entry.record.status == ActorMessageStatus::Pending
        }) {
            stored.record.status = ActorMessageStatus::DeadLetter;
            stored.relay_attempt = attempt;
            stored.dead_letter_at = Some(ts);
            stored.relay_last_error = Some(error.to_string());
        }
        Ok(())
    }

    async fn append_run_event(
        &self,
        run_id: &str,
        event_type: &str,
        _ts: i64,
        _payload: Value,
    ) -> Result<(), Self::Error> {
        let mut state = self.state.lock().await;
        state.events.push(EventRecord {
            run_id: run_id.to_string(),
            event_type: event_type.to_string(),
        });
        Ok(())
    }
}

#[derive(Debug, Error)]
enum TestRelayError {
    #[error("retryable relay failure")]
    Retryable,
    #[error("permanent relay failure")]
    Permanent,
}

struct TestRelay;

#[async_trait]
impl ActorMessageRelay for TestRelay {
    type Error = TestRelayError;

    async fn deliver(
        &self,
        message: &ActorMessageRecord,
    ) -> Result<(), ActorRelayError<Self::Error>> {
        if message.to_actor_id.contains("retry") {
            return Err(ActorRelayError::retryable(TestRelayError::Retryable));
        }
        if message.to_actor_id.contains("dead") {
            return Err(ActorRelayError::permanent(TestRelayError::Permanent));
        }
        Ok(())
    }
}

fn remote_message_command(
    run_id: &str,
    to_actor_id: &str,
    created_at: i64,
) -> SendActorMessageCommand {
    SendActorMessageCommand {
        run_id: run_id.to_string(),
        from_actor_id: "planner".to_string(),
        from_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
        to_actor_id: to_actor_id.to_string(),
        to_peer_id: ACTOR_NODE_PEER_ID.to_string(),
        channel: "coordination".to_string(),
        transport: ActorMessageTransport::Remote,
        route: Some(json!({"endpoint":"mock://relay"})),
        payload: json!({"text":"hello"}),
        message_kind: ActorMessageKind::CoordinationRequest,
        idempotency_key: None,
        created_at,
    }
}

#[tokio::test]
async fn send_and_ack_emit_expected_events() {
    let store = TestStore::default();
    let mailbox = ActorMailbox::new(store.clone());

    let message = mailbox
        .send(SendActorMessageCommand {
            run_id: "run-send-ack".to_string(),
            from_actor_id: "planner".to_string(),
            from_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            to_actor_id: "reviewer".to_string(),
            to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            channel: "coordination".to_string(),
            transport: ActorMessageTransport::Local,
            route: None,
            payload: json!({"text":"review this"}),
            message_kind: ActorMessageKind::CoordinationRequest,
            idempotency_key: None,
            created_at: 10,
        })
        .await
        .expect("send message");
    assert_eq!(message.status, ActorMessageStatus::Pending);

    let first_ack = mailbox
        .ack(AckActorMessageCommand {
            run_id: message.run_id.clone(),
            actor_id: message.to_actor_id.clone(),
            peer_id: message.to_peer_id.clone(),
            message_id: message.message_id,
            delivered_at: 20,
        })
        .await
        .expect("first ack");
    assert!(first_ack.status_changed);
    assert_eq!(first_ack.message.status, ActorMessageStatus::Delivered);
    assert_eq!(first_ack.message.delivered_at, Some(20));

    let second_ack = mailbox
        .ack(AckActorMessageCommand {
            run_id: message.run_id.clone(),
            actor_id: message.to_actor_id.clone(),
            peer_id: message.to_peer_id.clone(),
            message_id: message.message_id,
            delivered_at: 30,
        })
        .await
        .expect("second ack should be idempotent");
    assert!(!second_ack.status_changed);
    assert_eq!(second_ack.message.status, ActorMessageStatus::Delivered);
    assert_eq!(second_ack.message.delivered_at, Some(20));

    let pending_inbox = mailbox
        .list_inbox(ListActorInboxQuery {
            run_id: message.run_id.clone(),
            actor_id: message.to_actor_id.clone(),
            peer_id: message.to_peer_id.clone(),
            limit: 100,
            after_id: None,
            include_delivered: false,
        })
        .await
        .expect("list pending inbox");
    assert!(pending_inbox.is_empty());

    let delivered_inbox = mailbox
        .list_inbox(ListActorInboxQuery {
            run_id: message.run_id.clone(),
            actor_id: message.to_actor_id.clone(),
            peer_id: message.to_peer_id.clone(),
            limit: 100,
            after_id: None,
            include_delivered: true,
        })
        .await
        .expect("list inbox with delivered");
    assert_eq!(delivered_inbox.len(), 1);
    assert_eq!(delivered_inbox[0].status, ActorMessageStatus::Delivered);

    let event_types = store.event_types("run-send-ack").await;
    assert_eq!(
        event_types,
        vec![
            "actor_message_sent".to_string(),
            "actor_message_delivered".to_string()
        ]
    );
    assert_eq!(
        store
            .event_count("run-send-ack", "actor_message_delivered")
            .await,
        1
    );
}

#[tokio::test]
async fn send_with_same_idempotency_key_reuses_message_and_event() {
    let store = TestStore::default();
    let mailbox = ActorMailbox::new(store.clone());

    let first = mailbox
        .send(SendActorMessageCommand {
            run_id: "run-send-idempotent".to_string(),
            from_actor_id: "planner".to_string(),
            from_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            to_actor_id: "reviewer".to_string(),
            to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            channel: "coordination".to_string(),
            transport: ActorMessageTransport::Local,
            route: None,
            payload: json!({"text":"review this"}),
            message_kind: ActorMessageKind::CoordinationRequest,
            idempotency_key: Some("msg-1".to_string()),
            created_at: 10,
        })
        .await
        .expect("first send");
    let second = mailbox
        .send(SendActorMessageCommand {
            run_id: "run-send-idempotent".to_string(),
            from_actor_id: "planner".to_string(),
            from_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            to_actor_id: "reviewer".to_string(),
            to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            channel: "coordination".to_string(),
            transport: ActorMessageTransport::Local,
            route: None,
            payload: json!({"text":"review this retry"}),
            message_kind: ActorMessageKind::CoordinationRequest,
            idempotency_key: Some("msg-1".to_string()),
            created_at: 20,
        })
        .await
        .expect("second send");
    assert_eq!(first.message_id, second.message_id);
    assert_eq!(
        store
            .event_count("run-send-idempotent", "actor_message_sent")
            .await,
        1
    );
}

#[tokio::test]
async fn relay_remote_pending_delivers_successfully() {
    let store = TestStore::default();
    let mailbox = ActorMailbox::new(store.clone());
    let sent = mailbox
        .send(remote_message_command("run-relay-ok", "remote-ok", 100))
        .await
        .expect("send remote message");

    let result = mailbox
        .relay_remote_pending(
            &TestRelay,
            RelayRemotePendingCommand {
                limit: 100,
                now: 200,
                max_attempts: 3,
                retry_delay_secs: 30,
            },
        )
        .await
        .expect("relay remote pending");
    assert_eq!(result.scanned, 1);
    assert_eq!(result.delivered, 1);
    assert_eq!(result.retried, 0);
    assert_eq!(result.dead_lettered, 0);

    let snapshot = store.snapshot(sent.message_id).await;
    assert_eq!(snapshot.record.status, ActorMessageStatus::Delivered);
    assert_eq!(snapshot.record.delivered_at, Some(200));
}

#[tokio::test]
async fn relay_remote_pending_retries_then_dead_letters() {
    let store = TestStore::default();
    let mailbox = ActorMailbox::new(store.clone());
    let sent = mailbox
        .send(remote_message_command(
            "run-relay-retry",
            "remote-retry",
            100,
        ))
        .await
        .expect("send remote retry message");

    let first = mailbox
        .relay_remote_pending(
            &TestRelay,
            RelayRemotePendingCommand {
                limit: 10,
                now: 1000,
                max_attempts: 3,
                retry_delay_secs: 15,
            },
        )
        .await
        .expect("first relay");
    assert_eq!(first.scanned, 1);
    assert_eq!(first.retried, 1);
    assert_eq!(first.dead_lettered, 0);

    let snapshot_after_first = store.snapshot(sent.message_id).await;
    assert_eq!(
        snapshot_after_first.record.status,
        ActorMessageStatus::Pending
    );
    assert_eq!(snapshot_after_first.relay_attempt, 1);
    assert_eq!(snapshot_after_first.relay_next_retry_at, Some(1015));
    assert!(snapshot_after_first.dead_letter_at.is_none());
    assert_eq!(
        snapshot_after_first.relay_last_error.as_deref(),
        Some("retryable relay failure")
    );

    let before_retry_window = mailbox
        .relay_remote_pending(
            &TestRelay,
            RelayRemotePendingCommand {
                limit: 10,
                now: 1010,
                max_attempts: 3,
                retry_delay_secs: 15,
            },
        )
        .await
        .expect("relay before retry window");
    assert_eq!(before_retry_window.scanned, 0);

    let second = mailbox
        .relay_remote_pending(
            &TestRelay,
            RelayRemotePendingCommand {
                limit: 10,
                now: 1020,
                max_attempts: 3,
                retry_delay_secs: 15,
            },
        )
        .await
        .expect("second relay");
    assert_eq!(second.scanned, 1);
    assert_eq!(second.retried, 1);
    assert_eq!(second.dead_lettered, 0);

    let third = mailbox
        .relay_remote_pending(
            &TestRelay,
            RelayRemotePendingCommand {
                limit: 10,
                now: 1040,
                max_attempts: 3,
                retry_delay_secs: 15,
            },
        )
        .await
        .expect("third relay");
    assert_eq!(third.scanned, 1);
    assert_eq!(third.retried, 0);
    assert_eq!(third.dead_lettered, 1);

    let snapshot_after_third = store.snapshot(sent.message_id).await;
    assert_eq!(
        snapshot_after_third.record.status,
        ActorMessageStatus::DeadLetter
    );
    assert_eq!(snapshot_after_third.relay_attempt, 3);
    assert_eq!(snapshot_after_third.dead_letter_at, Some(1040));
    assert_eq!(
        snapshot_after_third.relay_last_error.as_deref(),
        Some("retryable relay failure")
    );
    assert_eq!(
        store
            .event_count("run-relay-retry", "actor_message_relay_retry")
            .await,
        2
    );
    assert_eq!(
        store
            .event_count("run-relay-retry", "actor_message_dead_letter")
            .await,
        1
    );
}

#[tokio::test]
async fn relay_remote_pending_dead_letters_permanent_failures() {
    let store = TestStore::default();
    let mailbox = ActorMailbox::new(store.clone());
    let sent = mailbox
        .send(remote_message_command("run-relay-dead", "remote-dead", 100))
        .await
        .expect("send remote dead message");

    let result = mailbox
        .relay_remote_pending(
            &TestRelay,
            RelayRemotePendingCommand {
                limit: 10,
                now: 200,
                max_attempts: 5,
                retry_delay_secs: 30,
            },
        )
        .await
        .expect("relay permanent failure");
    assert_eq!(result.scanned, 1);
    assert_eq!(result.delivered, 0);
    assert_eq!(result.retried, 0);
    assert_eq!(result.dead_lettered, 1);

    let snapshot = store.snapshot(sent.message_id).await;
    assert_eq!(snapshot.record.status, ActorMessageStatus::DeadLetter);
    assert_eq!(snapshot.relay_attempt, 1);
    assert_eq!(snapshot.dead_letter_at, Some(200));
    assert_eq!(
        snapshot.relay_last_error.as_deref(),
        Some("permanent relay failure")
    );
}

#[tokio::test]
async fn relay_remote_pending_normalizes_zero_limits() {
    let store = TestStore::default();
    let mailbox = ActorMailbox::new(store.clone());
    let sent = mailbox
        .send(remote_message_command(
            "run-relay-normalize",
            "remote-retry",
            100,
        ))
        .await
        .expect("send remote retry message");

    let result = mailbox
        .relay_remote_pending(
            &TestRelay,
            RelayRemotePendingCommand {
                limit: 0,
                now: 200,
                max_attempts: 0,
                retry_delay_secs: 0,
            },
        )
        .await
        .expect("relay with normalized args");
    assert_eq!(result.scanned, 1);
    assert_eq!(result.delivered, 0);
    assert_eq!(result.retried, 0);
    assert_eq!(result.dead_lettered, 1);

    let snapshot = store.snapshot(sent.message_id).await;
    assert_eq!(snapshot.record.status, ActorMessageStatus::DeadLetter);
    assert_eq!(snapshot.relay_attempt, 1);
    assert_eq!(snapshot.dead_letter_at, Some(200));
}
