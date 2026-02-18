use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::message::{ActorMessageRecord, ActorMessageStatus, ActorMessageTransport};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorSendRequest {
    pub run_id: String,
    pub from_actor_id: String,
    pub to_actor_id: String,
    pub channel: Option<String>,
    pub transport: Option<ActorMessageTransport>,
    pub route: Option<Value>,
    pub payload: Value,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorSendResponse {
    pub message_id: i64,
    pub state: ActorMessageStatus,
    pub deduped: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorInboxRequest {
    pub run_id: String,
    pub actor_id: String,
    pub cursor: Option<i64>,
    pub limit: Option<i64>,
    pub states: Option<Vec<ActorMessageStatus>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorInboxResponse {
    pub messages: Vec<ActorMessageRecord>,
    pub next_cursor: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorAckRequest {
    pub run_id: String,
    pub actor_id: String,
    pub message_id: i64,
    pub ack_token: Option<String>,
    pub result: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorAckResponse {
    pub message_id: i64,
    pub state: ActorMessageStatus,
    pub acked_at: i64,
    pub message: ActorMessageRecord,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorServiceErrorCode {
    BadRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Gone,
    UnprocessableEntity,
    TooManyRequests,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActorServiceError {
    pub code: ActorServiceErrorCode,
    pub message: String,
}

impl ActorServiceError {
    pub fn new(code: ActorServiceErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[async_trait]
pub trait ActorMailboxService: Send + Sync {
    async fn actor_send(
        &self,
        request: ActorSendRequest,
    ) -> Result<ActorSendResponse, ActorServiceError>;

    async fn actor_inbox(
        &self,
        request: ActorInboxRequest,
    ) -> Result<ActorInboxResponse, ActorServiceError>;

    async fn actor_ack(
        &self,
        request: ActorAckRequest,
    ) -> Result<ActorAckResponse, ActorServiceError>;
}
