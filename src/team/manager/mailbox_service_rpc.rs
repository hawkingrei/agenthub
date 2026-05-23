use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ActorAckRequest, ActorAckResponse, ActorInboxRequest, ActorInboxResponse,
    ActorMailboxService, ActorSendRequest, ActorSendResponse, ActorServiceError,
    ActorServiceErrorCode, ActorTaskLinkRequest, ActorTaskLinkResponse, ActorTriageRequest,
    ActorTriageResponse, ListActorInboxQuery,
};
use async_trait::async_trait;

use super::mailbox::{
    SendActorMessageInput, SqlActorMailboxStore, map_actor_service_error, optional_trimmed,
    required_trimmed_field,
};
use super::mailbox_service::TeamActorMailboxService;
use crate::team::{TeamActorMessageStatus, TeamActorMessageTransport};

#[async_trait]
impl ActorMailboxService for TeamActorMailboxService {
    async fn actor_send(
        &self,
        request: ActorSendRequest,
    ) -> Result<ActorSendResponse, ActorServiceError> {
        let run_id = required_trimmed_field(&request.run_id, "run_id")?;
        let from_actor_id = required_trimmed_field(&request.from_actor_id, "from_actor_id")?;
        let from_peer_id =
            optional_trimmed(request.from_peer_id.as_deref()).unwrap_or(ACTOR_MAIN_PEER_ID);
        let to_actor_id = optional_trimmed(request.to_actor_id.as_deref());
        let channel_id = optional_trimmed(request.channel_id.as_deref());
        let (to_actor_id, channel_id) = match (to_actor_id, channel_id) {
            (Some(to_actor_id), None) => (Some(to_actor_id), None),
            (None, Some(channel_id)) => (None, Some(channel_id)),
            (Some(_), Some(_)) => {
                return Err(ActorServiceError::new(
                    ActorServiceErrorCode::BadRequest,
                    "to_actor_id and channel_id cannot be used together",
                ));
            }
            (None, None) => {
                return Err(ActorServiceError::new(
                    ActorServiceErrorCode::BadRequest,
                    "to_actor_id or channel_id is required",
                ));
            }
        };
        let to_peer_id =
            optional_trimmed(request.to_peer_id.as_deref()).unwrap_or(ACTOR_MAIN_PEER_ID);
        let channel = request
            .channel
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("default");
        let transport = request
            .transport
            .unwrap_or(TeamActorMessageTransport::Local);
        let idempotency_key = optional_trimmed(request.idempotency_key.as_deref());

        if request.route.is_some() && channel_id.is_some() {
            return Err(ActorServiceError::new(
                ActorServiceErrorCode::BadRequest,
                "channel mailbox target does not support route",
            ));
        }
        if let Some(channel_id) = channel_id {
            return self
                .send_channel_message(
                    run_id,
                    from_actor_id,
                    from_peer_id,
                    channel_id,
                    channel,
                    request.message_kind,
                    request.payload,
                    idempotency_key,
                )
                .await
                .map_err(map_actor_service_error);
        }
        let to_actor_id = to_actor_id.expect("validated actor target");
        match transport {
            TeamActorMessageTransport::Local => {
                self.validate_direct_send_target(run_id, to_actor_id)
                    .await?;
            }
            TeamActorMessageTransport::Remote => {
                Self::validate_direct_remote_route(request.route.as_ref())?;
                if to_peer_id == ACTOR_MAIN_PEER_ID {
                    return Err(ActorServiceError::new(
                        ActorServiceErrorCode::BadRequest,
                        "to_peer_id must not be 'main' for remote transport",
                    ));
                }
            }
        }

        let (message, created) = self
            .manager
            .send_actor_message_with_created_kind(
                SendActorMessageInput {
                    run_id,
                    from_actor_id,
                    from_peer_id,
                    to_actor_id,
                    to_peer_id,
                    channel,
                    transport,
                    route: request.route,
                    payload: request.payload,
                    message_kind: None,
                    idempotency_key,
                },
                request.message_kind,
            )
            .await
            .map_err(map_actor_service_error)?;
        let message_id = message.message_id;
        let state = message.status.clone();
        let created_at = message.created_at;

        Ok(ActorSendResponse {
            message_id,
            state,
            deduped: !created,
            created_at,
            message,
        })
    }

    async fn actor_inbox(
        &self,
        request: ActorInboxRequest,
    ) -> Result<ActorInboxResponse, ActorServiceError> {
        let run_id = required_trimmed_field(&request.run_id, "run_id")?;
        let actor_id = required_trimmed_field(&request.actor_id, "actor_id")?;
        let limit = request.limit.unwrap_or(50).clamp(1, 1000);
        let include_delivered = request
            .states
            .as_ref()
            .is_some_and(|states| states.contains(&TeamActorMessageStatus::Delivered));
        let states = request
            .states
            .unwrap_or_else(|| vec![TeamActorMessageStatus::Pending]);
        let snapshot = SqlActorMailboxStore {
            db: self.manager.db.clone(),
            message_archive: None,
        }
        .read_inbox_snapshot(&ListActorInboxQuery {
            run_id: run_id.to_string(),
            actor_id: actor_id.to_string(),
            peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            limit,
            after_id: request.cursor,
            include_delivered,
        })
        .await
        .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?;
        let messages = snapshot
            .messages
            .into_iter()
            .filter(|message| states.contains(&message.status))
            .collect::<Vec<_>>();
        let next_cursor = messages.last().map(|message| message.message_id);

        Ok(ActorInboxResponse {
            messages,
            next_cursor,
            pending_count: snapshot.pending_count,
        })
    }

    async fn actor_ack(
        &self,
        request: ActorAckRequest,
    ) -> Result<ActorAckResponse, ActorServiceError> {
        let run_id = required_trimmed_field(&request.run_id, "run_id")?;
        let actor_id = required_trimmed_field(&request.actor_id, "actor_id")?;
        if request.message_id <= 0 {
            return Err(ActorServiceError::new(
                ActorServiceErrorCode::BadRequest,
                "message_id must be positive",
            ));
        }

        let result = self
            .manager
            .ack_actor_message(run_id, actor_id, request.message_id)
            .await
            .map_err(map_actor_service_error)?;
        let message = result.message;
        let state = message.status.clone();
        let acked_at = message.delivered_at.unwrap_or(message.created_at);

        Ok(ActorAckResponse {
            message_id: message.message_id,
            state,
            acked_at,
            status_changed: result.status_changed,
            message,
        })
    }

    async fn actor_triage(
        &self,
        request: ActorTriageRequest,
    ) -> Result<ActorTriageResponse, ActorServiceError> {
        let run_id = required_trimmed_field(&request.run_id, "run_id")?;
        let actor_id = required_trimmed_field(&request.actor_id, "actor_id")?;
        if request.message_id <= 0 {
            return Err(ActorServiceError::new(
                ActorServiceErrorCode::BadRequest,
                "message_id must be positive",
            ));
        }

        let result = self
            .manager
            .triage_actor_message(run_id, actor_id, request.message_id, request.disposition)
            .await
            .map_err(map_actor_service_error)?;
        let triaged_at = result.message.handled_at.unwrap_or(
            result
                .message
                .delivered_at
                .unwrap_or(result.message.created_at),
        );

        Ok(ActorTriageResponse {
            message_id: result.message.message_id,
            disposition: result.message.handling_disposition.clone(),
            triaged_at,
            handling_changed: result.handling_changed,
            message: result.message,
        })
    }

    async fn actor_task_link(
        &self,
        request: ActorTaskLinkRequest,
    ) -> Result<ActorTaskLinkResponse, ActorServiceError> {
        let run_id = required_trimmed_field(&request.run_id, "run_id")?;
        let actor_id = required_trimmed_field(&request.actor_id, "actor_id")?;
        let task_id = required_trimmed_field(&request.task_id, "task_id")?;
        if request.message_id <= 0 {
            return Err(ActorServiceError::new(
                ActorServiceErrorCode::BadRequest,
                "message_id must be positive",
            ));
        }

        let result = self
            .manager
            .link_actor_message_task(
                run_id,
                actor_id,
                request.message_id,
                task_id,
                request.relation,
            )
            .await
            .map_err(map_actor_service_error)?;
        let linked_at = result
            .message
            .handled_at
            .unwrap_or(result.message.created_at);

        Ok(ActorTaskLinkResponse {
            message_id: result.message.message_id,
            task_id: result.task_id,
            relation: result.relation,
            linked_at,
            created: result.created,
            message: result.message,
        })
    }
}
