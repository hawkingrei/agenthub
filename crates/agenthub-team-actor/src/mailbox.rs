use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

use crate::message::{ActorMessageRecord, ActorMessageStatus, ActorMessageTransport};
use crate::relay::{ActorMessageRelay, ActorRelayError};

#[derive(Debug, Clone)]
pub struct SendActorMessageCommand {
    pub run_id: String,
    pub from_actor_id: String,
    pub from_peer_id: String,
    pub to_actor_id: String,
    pub to_peer_id: String,
    pub channel: String,
    pub transport: ActorMessageTransport,
    pub route: Option<Value>,
    pub payload: Value,
    pub idempotency_key: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct CreatePendingMessageResult {
    pub message: ActorMessageRecord,
    pub created: bool,
}

#[derive(Debug, Clone)]
pub struct ListActorInboxQuery {
    pub run_id: String,
    pub actor_id: String,
    pub peer_id: String,
    pub limit: i64,
    pub after_id: Option<i64>,
    pub include_delivered: bool,
}

#[derive(Debug, Clone)]
pub struct AckActorMessageCommand {
    pub run_id: String,
    pub actor_id: String,
    pub peer_id: String,
    pub message_id: i64,
    pub delivered_at: i64,
}

#[derive(Debug, Clone)]
pub struct AckActorMessageResult {
    pub message: ActorMessageRecord,
    pub status_changed: bool,
}

#[derive(Debug, Clone)]
pub struct PendingRemoteRelayRecord {
    pub message: ActorMessageRecord,
    pub attempt: i64,
}

#[derive(Debug, Clone)]
pub struct RelayRemotePendingCommand {
    pub limit: i64,
    pub now: i64,
    pub max_attempts: i64,
    pub retry_delay_secs: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RelayRemotePendingResult {
    pub scanned: usize,
    pub delivered: usize,
    pub retried: usize,
    pub dead_lettered: usize,
}

#[derive(Debug, Error)]
pub enum ActorMailboxError<E: std::error::Error + Send + Sync + 'static> {
    #[error(transparent)]
    Store(#[from] E),
}

#[async_trait]
pub trait ActorMailboxStore {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn create_pending_message(
        &self,
        cmd: &SendActorMessageCommand,
    ) -> Result<CreatePendingMessageResult, Self::Error>;

    async fn list_inbox(
        &self,
        query: &ListActorInboxQuery,
    ) -> Result<Vec<ActorMessageRecord>, Self::Error>;

    async fn ack_message(
        &self,
        cmd: &AckActorMessageCommand,
    ) -> Result<AckActorMessageResult, Self::Error>;

    async fn list_remote_pending_messages(
        &self,
        limit: i64,
        now: i64,
    ) -> Result<Vec<PendingRemoteRelayRecord>, Self::Error>;

    async fn mark_remote_retry(
        &self,
        run_id: &str,
        message_id: i64,
        ts: i64,
        attempt: i64,
        next_retry_at: i64,
        error: &str,
    ) -> Result<(), Self::Error>;

    async fn mark_remote_dead_letter(
        &self,
        run_id: &str,
        message_id: i64,
        ts: i64,
        attempt: i64,
        error: &str,
    ) -> Result<(), Self::Error>;

    async fn append_run_event(
        &self,
        run_id: &str,
        event_type: &str,
        ts: i64,
        payload: Value,
    ) -> Result<(), Self::Error>;
}

pub struct ActorMailbox<S> {
    store: S,
}

impl<S> ActorMailbox<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S> ActorMailbox<S>
where
    S: ActorMailboxStore,
{
    pub async fn send_with_result(
        &self,
        cmd: SendActorMessageCommand,
    ) -> Result<CreatePendingMessageResult, ActorMailboxError<S::Error>> {
        let result = self.store.create_pending_message(&cmd).await?;
        if result.created {
            let event_payload = serde_json::json!({
                "message_id": result.message.message_id,
                "from_actor_id": result.message.from_actor_id,
                "from_peer_id": result.message.from_peer_id,
                "to_actor_id": result.message.to_actor_id,
                "to_peer_id": result.message.to_peer_id,
                "channel": result.message.channel,
                "transport": result.message.transport.as_str(),
                "status": result.message.status.as_str(),
            });
            self.store
                .append_run_event(
                    &result.message.run_id,
                    "actor_message_sent",
                    cmd.created_at,
                    event_payload,
                )
                .await?;
        }
        Ok(result)
    }

    pub async fn send(
        &self,
        cmd: SendActorMessageCommand,
    ) -> Result<ActorMessageRecord, ActorMailboxError<S::Error>> {
        let result = self.send_with_result(cmd).await?;
        Ok(result.message)
    }

    pub async fn list_inbox(
        &self,
        query: ListActorInboxQuery,
    ) -> Result<Vec<ActorMessageRecord>, ActorMailboxError<S::Error>> {
        let messages = self.store.list_inbox(&query).await?;
        Ok(messages)
    }

    pub async fn ack(
        &self,
        cmd: AckActorMessageCommand,
    ) -> Result<AckActorMessageResult, ActorMailboxError<S::Error>> {
        let result = self.store.ack_message(&cmd).await?;
        if result.status_changed {
            let event_payload = serde_json::json!({
                "message_id": result.message.message_id,
                "from_actor_id": result.message.from_actor_id,
                "from_peer_id": result.message.from_peer_id,
                "to_actor_id": result.message.to_actor_id,
                "to_peer_id": result.message.to_peer_id,
                "channel": result.message.channel,
                "transport": result.message.transport.as_str(),
                "status": result.message.status.as_str(),
            });
            self.store
                .append_run_event(
                    &result.message.run_id,
                    "actor_message_delivered",
                    cmd.delivered_at,
                    event_payload,
                )
                .await?;
        }
        Ok(result)
    }

    pub async fn relay_remote_pending<R>(
        &self,
        relay: &R,
        cmd: RelayRemotePendingCommand,
    ) -> Result<RelayRemotePendingResult, ActorMailboxError<S::Error>>
    where
        R: ActorMessageRelay,
    {
        let limit = cmd.limit.max(1);
        let max_attempts = cmd.max_attempts.max(1);
        let retry_delay_secs = cmd.retry_delay_secs.max(1);
        let records = self
            .store
            .list_remote_pending_messages(limit, cmd.now)
            .await?;

        let mut result = RelayRemotePendingResult {
            scanned: records.len(),
            ..RelayRemotePendingResult::default()
        };
        for record in records {
            let message = record.message;
            let next_attempt = record.attempt + 1;

            match relay.deliver(&message).await {
                Ok(()) => {
                    let delivered = self
                        .ack(AckActorMessageCommand {
                            run_id: message.run_id.clone(),
                            actor_id: message.to_actor_id.clone(),
                            peer_id: message.to_peer_id.clone(),
                            message_id: message.message_id,
                            delivered_at: cmd.now,
                        })
                        .await?;
                    if delivered.message.status == ActorMessageStatus::Delivered {
                        result.delivered += 1;
                    }
                }
                Err(ActorRelayError::Retryable(err)) => {
                    if next_attempt >= max_attempts {
                        let error_text = err.to_string();
                        self.store
                            .mark_remote_dead_letter(
                                &message.run_id,
                                message.message_id,
                                cmd.now,
                                next_attempt,
                                &error_text,
                            )
                            .await?;
                        let event_payload = serde_json::json!({
                            "message_id": message.message_id,
                            "from_actor_id": message.from_actor_id,
                            "from_peer_id": message.from_peer_id,
                            "to_actor_id": message.to_actor_id,
                            "to_peer_id": message.to_peer_id,
                            "channel": message.channel,
                            "transport": message.transport.as_str(),
                            "status": ActorMessageStatus::DeadLetter.as_str(),
                            "attempt": next_attempt,
                            "error": error_text,
                        });
                        self.store
                            .append_run_event(
                                &message.run_id,
                                "actor_message_dead_letter",
                                cmd.now,
                                event_payload,
                            )
                            .await?;
                        result.dead_lettered += 1;
                    } else {
                        let error_text = err.to_string();
                        let next_retry_at = cmd.now + retry_delay_secs;
                        self.store
                            .mark_remote_retry(
                                &message.run_id,
                                message.message_id,
                                cmd.now,
                                next_attempt,
                                next_retry_at,
                                &error_text,
                            )
                            .await?;
                        let event_payload = serde_json::json!({
                            "message_id": message.message_id,
                            "from_actor_id": message.from_actor_id,
                            "from_peer_id": message.from_peer_id,
                            "to_actor_id": message.to_actor_id,
                            "to_peer_id": message.to_peer_id,
                            "channel": message.channel,
                            "transport": message.transport.as_str(),
                            "status": message.status.as_str(),
                            "attempt": next_attempt,
                            "next_retry_at": next_retry_at,
                            "error": error_text,
                        });
                        self.store
                            .append_run_event(
                                &message.run_id,
                                "actor_message_relay_retry",
                                cmd.now,
                                event_payload,
                            )
                            .await?;
                        result.retried += 1;
                    }
                }
                Err(ActorRelayError::Permanent(err)) => {
                    let error_text = err.to_string();
                    self.store
                        .mark_remote_dead_letter(
                            &message.run_id,
                            message.message_id,
                            cmd.now,
                            next_attempt,
                            &error_text,
                        )
                        .await?;
                    let event_payload = serde_json::json!({
                        "message_id": message.message_id,
                        "from_actor_id": message.from_actor_id,
                        "from_peer_id": message.from_peer_id,
                        "to_actor_id": message.to_actor_id,
                        "to_peer_id": message.to_peer_id,
                        "channel": message.channel,
                        "transport": message.transport.as_str(),
                        "status": ActorMessageStatus::DeadLetter.as_str(),
                        "attempt": next_attempt,
                        "error": error_text,
                    });
                    self.store
                        .append_run_event(
                            &message.run_id,
                            "actor_message_dead_letter",
                            cmd.now,
                            event_payload,
                        )
                        .await?;
                    result.dead_lettered += 1;
                }
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
#[path = "mailbox_tests.rs"]
mod tests;
