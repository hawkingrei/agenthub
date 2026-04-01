use std::path::Path;

use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ActorAckRequest, ActorAckResponse, ActorInboxRequest, ActorInboxResponse,
    ActorMailboxService, ActorMessageRecord, ActorMessageStatus, ActorMessageTransport,
    ActorSendRequest, ActorSendResponse, ActorServiceError, ActorServiceErrorCode,
};
use async_trait::async_trait;
use tonic::Code;

use super::super::p2p::P2PTransport;
use super::super::proto::agenthub::internal::v1::{
    AckActorMessageRequest as GrpcAckActorMessageRequest, ActorMessage as GrpcActorMessage,
    ListActorInboxRequest as GrpcListActorInboxRequest,
    SendActorMessageRequest as GrpcSendActorMessageRequest,
};
use super::{
    InternalGrpcMailboxClient, format_internal_grpc_status_message, timeout_internal_grpc_call,
};

pub(super) fn parse_transport(raw: &str) -> ActorMessageTransport {
    match raw.trim() {
        "remote" => ActorMessageTransport::Remote,
        _ => ActorMessageTransport::Local,
    }
}

pub(super) fn parse_status(raw: &str) -> ActorMessageStatus {
    match raw.trim() {
        "delivered" => ActorMessageStatus::Delivered,
        "dead_letter" => ActorMessageStatus::DeadLetter,
        _ => ActorMessageStatus::Pending,
    }
}

pub(super) fn parse_message(
    message: GrpcActorMessage,
) -> Result<ActorMessageRecord, ActorServiceError> {
    let from_actor_kind = agenthub_team_actor::infer_actor_identity_kind(&message.from_actor_id);
    let to_actor_kind = agenthub_team_actor::infer_actor_identity_kind(&message.to_actor_id);
    let route = if message.route_json.trim().is_empty() {
        None
    } else {
        Some(serde_json::from_str(&message.route_json).map_err(|err| {
            ActorServiceError::new(
                ActorServiceErrorCode::Internal,
                format!("decode route_json: {}", err),
            )
        })?)
    };
    let payload = serde_json::from_str(&message.payload_json).map_err(|err| {
        ActorServiceError::new(
            ActorServiceErrorCode::Internal,
            format!("decode payload_json: {}", err),
        )
    })?;
    Ok(ActorMessageRecord {
        message_id: message.message_id,
        run_id: message.run_id,
        from_actor_id: message.from_actor_id,
        from_peer_id: if message.from_peer_id.trim().is_empty() {
            ACTOR_MAIN_PEER_ID.to_string()
        } else {
            message.from_peer_id
        },
        from_actor_kind,
        to_actor_id: message.to_actor_id,
        to_peer_id: if message.to_peer_id.trim().is_empty() {
            ACTOR_MAIN_PEER_ID.to_string()
        } else {
            message.to_peer_id
        },
        to_actor_kind,
        channel: message.channel,
        transport: parse_transport(&message.transport),
        route,
        payload,
        status: parse_status(&message.status),
        created_at: message.created_at,
        delivered_at: (message.delivered_at > 0).then_some(message.delivered_at),
    })
}

pub(super) fn map_grpc_status(status: tonic::Status) -> ActorServiceError {
    let code = match status.code() {
        Code::InvalidArgument => ActorServiceErrorCode::BadRequest,
        Code::Unauthenticated => ActorServiceErrorCode::Unauthorized,
        Code::PermissionDenied => ActorServiceErrorCode::Forbidden,
        Code::NotFound => ActorServiceErrorCode::NotFound,
        Code::AlreadyExists | Code::Aborted => ActorServiceErrorCode::Conflict,
        Code::FailedPrecondition => ActorServiceErrorCode::Gone,
        Code::ResourceExhausted => ActorServiceErrorCode::TooManyRequests,
        Code::Unavailable | Code::DeadlineExceeded => ActorServiceErrorCode::TooManyRequests,
        _ => ActorServiceErrorCode::Internal,
    };
    ActorServiceError::new(code, format_internal_grpc_status_message(&status))
}

#[async_trait]
impl ActorMailboxService for InternalGrpcMailboxClient {
    async fn actor_send(
        &self,
        request: ActorSendRequest,
    ) -> Result<ActorSendResponse, ActorServiceError> {
        let from_actor_kind =
            agenthub_team_actor::infer_actor_identity_kind(&request.from_actor_id);
        let to_actor_id = request.to_actor_id.clone().unwrap_or_default();
        let to_actor_kind = agenthub_team_actor::infer_actor_identity_kind(&to_actor_id);
        let request_channel = request.channel.clone();
        let request_transport = request.transport.clone();
        let request_channel_id = request.channel_id.clone();
        let grpc_channel = request_channel
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let grpc_transport = request_transport
            .clone()
            .unwrap_or(ActorMessageTransport::Local)
            .as_str()
            .to_string();
        let payload_json = serde_json::to_string(&request.payload).map_err(|err| {
            ActorServiceError::new(
                ActorServiceErrorCode::BadRequest,
                format!("serialize payload: {}", err),
            )
        })?;
        let route_json = request
            .route
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|err| {
                ActorServiceError::new(
                    ActorServiceErrorCode::BadRequest,
                    format!("serialize route: {}", err),
                )
            })?
            .unwrap_or_default();
        let mut client = self.client();
        let response = timeout_internal_grpc_call(client.send_actor_message(self.request(
            GrpcSendActorMessageRequest {
                run_id: request.run_id.clone(),
                from_actor_id: request.from_actor_id.clone(),
                to_actor_id: to_actor_id.clone(),
                channel: grpc_channel,
                transport: grpc_transport,
                route_json,
                payload_json,
                idempotency_key: request.idempotency_key.unwrap_or_default(),
                from_peer_id: request.from_peer_id.clone().unwrap_or_default(),
                to_peer_id: request.to_peer_id.clone().unwrap_or_default(),
                channel_id: request_channel_id.unwrap_or_default(),
            },
        )?))
        .await
        .map_err(map_grpc_status)?
        .into_inner();
        let message = if response.message_json.trim().is_empty() {
            ActorMessageRecord {
                message_id: response.message_id,
                run_id: request.run_id,
                from_actor_id: request.from_actor_id,
                from_peer_id: request
                    .from_peer_id
                    .unwrap_or_else(|| ACTOR_MAIN_PEER_ID.to_string()),
                from_actor_kind,
                to_actor_id,
                to_peer_id: request
                    .to_peer_id
                    .unwrap_or_else(|| ACTOR_MAIN_PEER_ID.to_string()),
                to_actor_kind,
                channel: request_channel.unwrap_or_else(|| "default".to_string()),
                transport: request_transport.unwrap_or(ActorMessageTransport::Local),
                route: request.route,
                payload: request.payload,
                status: parse_status(&response.status),
                created_at: 0,
                delivered_at: None,
            }
        } else {
            serde_json::from_str(&response.message_json).map_err(|err| {
                ActorServiceError::new(
                    ActorServiceErrorCode::Internal,
                    format!("decode send_actor_message response: {}", err),
                )
            })?
        };
        let state = message.status.clone();
        Ok(ActorSendResponse {
            message_id: response.message_id,
            state,
            deduped: false,
            created_at: message.created_at,
            message,
        })
    }

    async fn actor_inbox(
        &self,
        request: ActorInboxRequest,
    ) -> Result<ActorInboxResponse, ActorServiceError> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(
            client.list_actor_inbox(
                self.request(GrpcListActorInboxRequest {
                    run_id: request.run_id,
                    actor_id: request.actor_id,
                    limit: request.limit.unwrap_or(20),
                    after_message_id: request.cursor.unwrap_or_default(),
                    include_delivered: request
                        .states
                        .as_ref()
                        .is_some_and(|states| states.contains(&ActorMessageStatus::Delivered)),
                })?,
            ),
        )
        .await
        .map_err(map_grpc_status)?
        .into_inner();
        let mut messages = Vec::with_capacity(response.messages.len());
        for message in response.messages {
            messages.push(parse_message(message)?);
        }
        let next_cursor = messages.last().map(|message| message.message_id);
        Ok(ActorInboxResponse {
            messages,
            next_cursor,
            pending_count: response.pending_count,
        })
    }

    async fn actor_ack(
        &self,
        request: ActorAckRequest,
    ) -> Result<ActorAckResponse, ActorServiceError> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(client.ack_actor_message(self.request(
            GrpcAckActorMessageRequest {
                run_id: request.run_id,
                actor_id: request.actor_id,
                message_id: request.message_id,
            },
        )?))
        .await
        .map_err(map_grpc_status)?
        .into_inner();
        let message = response.message.ok_or_else(|| {
            ActorServiceError::new(ActorServiceErrorCode::Internal, "missing ack message")
        })?;
        let message = parse_message(message)?;
        Ok(ActorAckResponse {
            message_id: message.message_id,
            state: message.status.clone(),
            acked_at: message.delivered_at.unwrap_or(message.created_at),
            status_changed: response.status_changed,
            message,
        })
    }
}

#[async_trait]
impl P2PTransport for InternalGrpcMailboxClient {
    async fn send_p2p_message(
        &self,
        request: ActorSendRequest,
    ) -> Result<ActorSendResponse, ActorServiceError> {
        self.actor_send(request).await
    }

    async fn list_p2p_inbox(
        &self,
        request: ActorInboxRequest,
    ) -> Result<ActorInboxResponse, ActorServiceError> {
        self.actor_inbox(request).await
    }

    async fn ack_p2p_message(
        &self,
        request: ActorAckRequest,
    ) -> Result<ActorAckResponse, ActorServiceError> {
        self.actor_ack(request).await
    }
}

pub fn normalize_existing_path(
    raw: Option<&str>,
    field_name: &str,
) -> anyhow::Result<Option<String>> {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let path = Path::new(value);
    if !path.exists() {
        anyhow::bail!("{} does not exist: {}", field_name, value);
    }
    Ok(Some(value.to_string()))
}
