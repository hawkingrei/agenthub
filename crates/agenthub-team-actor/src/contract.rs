use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::message::{ActorMessageRecord, ActorMessageStatus, ActorMessageTransport};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorSendRequest {
    pub run_id: String,
    pub from_actor_id: String,
    pub from_peer_id: Option<String>,
    pub to_actor_id: Option<String>,
    pub channel_id: Option<String>,
    pub to_peer_id: Option<String>,
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
    pub message: ActorMessageRecord,
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

pub async fn actor_inbox_with_auto_ack<S: ActorMailboxService>(
    service: &S,
    request: ActorInboxRequest,
) -> Result<ActorInboxResponse, ActorServiceError> {
    let run_id = request.run_id.clone();
    let response = service.actor_inbox(request).await?;
    let mut messages = Vec::with_capacity(response.messages.len());
    for message in response.messages {
        if message.status != ActorMessageStatus::Pending {
            messages.push(message);
            continue;
        }
        let acked = service
            .actor_ack(ActorAckRequest {
                run_id: run_id.clone(),
                actor_id: message.to_actor_id.clone(),
                message_id: message.message_id,
                ack_token: None,
                result: None,
            })
            .await;
        match acked {
            Ok(acked) => messages.push(acked.message),
            Err(err) if err.code == ActorServiceErrorCode::NotFound => messages.push(message),
            Err(err) => return Err(err),
        }
    }
    Ok(ActorInboxResponse {
        messages,
        next_cursor: response.next_cursor,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use super::{
        ActorAckRequest, ActorAckResponse, ActorInboxRequest, ActorInboxResponse,
        ActorMailboxService, ActorMessageStatus, ActorSendRequest, ActorSendResponse,
        ActorServiceError, ActorServiceErrorCode, actor_inbox_with_auto_ack,
    };
    use crate::message::{
        ACTOR_MAIN_PEER_ID, ActorIdentityKind, ActorMessageRecord, ActorMessageTransport,
    };

    #[derive(Clone)]
    struct MockMailboxService {
        inbox: Vec<ActorMessageRecord>,
        acked_ids: Arc<Mutex<Vec<i64>>>,
        ack_error: Option<ActorServiceErrorCode>,
    }

    #[async_trait]
    impl ActorMailboxService for MockMailboxService {
        async fn actor_send(
            &self,
            _request: ActorSendRequest,
        ) -> Result<ActorSendResponse, ActorServiceError> {
            unreachable!("send is not used in this test")
        }

        async fn actor_inbox(
            &self,
            _request: ActorInboxRequest,
        ) -> Result<ActorInboxResponse, ActorServiceError> {
            Ok(ActorInboxResponse {
                messages: self.inbox.clone(),
                next_cursor: self.inbox.last().map(|item| item.message_id),
            })
        }

        async fn actor_ack(
            &self,
            request: ActorAckRequest,
        ) -> Result<ActorAckResponse, ActorServiceError> {
            self.acked_ids
                .lock()
                .expect("acquire acked_ids mutex")
                .push(request.message_id);
            if let Some(code) = self.ack_error {
                return Err(ActorServiceError::new(code, "ack failed"));
            }
            let message = self
                .inbox
                .iter()
                .find(|item| item.message_id == request.message_id)
                .expect("find acked message")
                .clone();
            Ok(ActorAckResponse {
                message_id: message.message_id,
                state: ActorMessageStatus::Delivered,
                acked_at: 100,
                message: ActorMessageRecord {
                    status: ActorMessageStatus::Delivered,
                    delivered_at: Some(100),
                    ..message
                },
            })
        }
    }

    fn mock_message(message_id: i64, status: ActorMessageStatus) -> ActorMessageRecord {
        ActorMessageRecord {
            message_id,
            run_id: "run-1".to_string(),
            from_actor_id: "planner".to_string(),
            from_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            from_actor_kind: ActorIdentityKind::Agent,
            to_actor_id: "reviewer".to_string(),
            to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            to_actor_kind: ActorIdentityKind::Agent,
            channel: "default".to_string(),
            transport: ActorMessageTransport::Local,
            route: None,
            payload: serde_json::json!({"type":"chat_message","text":"hello"}),
            status,
            created_at: 1,
            delivered_at: None,
        }
    }

    #[tokio::test]
    async fn actor_inbox_with_auto_ack_marks_pending_as_delivered() {
        let service = MockMailboxService {
            inbox: vec![
                mock_message(1, ActorMessageStatus::Pending),
                mock_message(2, ActorMessageStatus::Delivered),
            ],
            acked_ids: Arc::new(Mutex::new(Vec::new())),
            ack_error: None,
        };
        let response = actor_inbox_with_auto_ack(
            &service,
            ActorInboxRequest {
                run_id: "run-1".to_string(),
                actor_id: "reviewer".to_string(),
                cursor: None,
                limit: Some(20),
                states: None,
            },
        )
        .await
        .expect("auto ack inbox");
        assert_eq!(response.messages.len(), 2);
        assert_eq!(response.messages[0].status, ActorMessageStatus::Delivered);
        assert_eq!(response.messages[1].status, ActorMessageStatus::Delivered);
        assert_eq!(
            *service
                .acked_ids
                .lock()
                .expect("acquire acked_ids for assertion"),
            vec![1]
        );
    }

    #[tokio::test]
    async fn actor_inbox_with_auto_ack_keeps_pending_on_not_found() {
        let service = MockMailboxService {
            inbox: vec![mock_message(11, ActorMessageStatus::Pending)],
            acked_ids: Arc::new(Mutex::new(Vec::new())),
            ack_error: Some(ActorServiceErrorCode::NotFound),
        };
        let response = actor_inbox_with_auto_ack(
            &service,
            ActorInboxRequest {
                run_id: "run-1".to_string(),
                actor_id: "reviewer".to_string(),
                cursor: None,
                limit: Some(20),
                states: None,
            },
        )
        .await
        .expect("auto ack inbox");
        assert_eq!(response.messages.len(), 1);
        assert_eq!(response.messages[0].status, ActorMessageStatus::Pending);
    }
}
