use std::sync::Arc;

use agenthub_team_actor::{
    ActorAckRequest, ActorInboxRequest, ActorInboxResponse, ActorMailboxService,
    ActorMessageHandlingDisposition, ActorMessageStatus, ActorServiceError, ActorServiceErrorCode,
    ActorTriageRequest,
};
use futures::{StreamExt, TryStreamExt, stream};

use crate::actor_runtime_env::connect_runtime_internal_mailbox_service;
use crate::internal::auth::InternalAction;
use crate::internal::client::InternalGrpcMailboxClient;

const RECEIVE_ACK_CONCURRENCY: usize = 8;

pub(super) async fn init_actor_control_client(
    actor_id: &str,
    run_id: Option<&str>,
    permissions: &[InternalAction],
    operation: &str,
) -> anyhow::Result<InternalGrpcMailboxClient> {
    connect_runtime_internal_mailbox_service(actor_id, run_id, permissions)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{operation} is unavailable because internal gRPC control is not configured"
            )
        })
}

pub(super) async fn init_actor_mailbox_service(
    actor_id: &str,
    run_id: &str,
) -> anyhow::Result<Arc<dyn ActorMailboxService>> {
    let client = init_actor_control_client(
        actor_id,
        Some(run_id),
        &[
            InternalAction::MessageSend,
            InternalAction::InboxList,
            InternalAction::MessageAck,
        ],
        "actor mailbox control",
    )
    .await?;
    Ok(Arc::new(client))
}

pub(super) async fn init_actor_task_link_service(
    actor_id: &str,
    run_id: &str,
) -> anyhow::Result<Arc<dyn ActorMailboxService>> {
    let client = init_actor_control_client(
        actor_id,
        Some(run_id),
        &[
            InternalAction::InboxList,
            InternalAction::MessageAck,
            InternalAction::TeamTaskWrite,
        ],
        "actor mailbox task-link control",
    )
    .await?;
    Ok(Arc::new(client))
}

pub(super) async fn init_actor_permission_review_client(
    actor_id: &str,
) -> anyhow::Result<InternalGrpcMailboxClient> {
    init_actor_control_client(
        actor_id,
        None,
        &[InternalAction::PermissionReview],
        "actor permission review control",
    )
    .await
}

pub(super) async fn load_actor_inbox<S: ActorMailboxService + ?Sized>(
    service: &S,
    request: ActorInboxRequest,
) -> Result<ActorInboxResponse, ActorServiceError> {
    service.actor_inbox(request).await
}

pub(super) async fn receive_actor_inbox<S: ActorMailboxService + ?Sized>(
    service: &S,
    request: ActorInboxRequest,
) -> Result<ActorInboxResponse, ActorServiceError> {
    let run_id = request.run_id.clone();
    let response = service.actor_inbox(request).await?;
    let pending_count = response.pending_count;
    let next_cursor = response.next_cursor;
    // Ack pending messages with bounded concurrency but preserve inbox output ordering.
    let mut indexed_messages = stream::iter(response.messages.into_iter().enumerate())
        .map(|(idx, message)| {
            let run_id = run_id.clone();
            async move {
                if message.status != ActorMessageStatus::Pending {
                    return Ok((idx, message, false));
                }
                let acked = service
                    .actor_ack(ActorAckRequest {
                        run_id,
                        actor_id: message.to_actor_id.clone(),
                        message_id: message.message_id,
                        ack_token: None,
                        result: None,
                    })
                    .await;
                match acked {
                    Ok(acked) => {
                        let triaged = service
                            .actor_triage(ActorTriageRequest {
                                run_id: acked.message.run_id.clone(),
                                actor_id: acked.message.to_actor_id.clone(),
                                message_id: acked.message.message_id,
                                disposition: ActorMessageHandlingDisposition::Claimed,
                            })
                            .await;
                        match triaged {
                            Ok(triaged) => Ok((idx, triaged.message, true)),
                            Err(err) if err.code == ActorServiceErrorCode::NotFound => {
                                Ok((idx, acked.message, true))
                            }
                            Err(err) => Err(err),
                        }
                    }
                    Err(err) if err.code == ActorServiceErrorCode::NotFound => {
                        Ok((idx, message, false))
                    }
                    Err(err) => Err(err),
                }
            }
        })
        .buffer_unordered(RECEIVE_ACK_CONCURRENCY)
        .try_collect::<Vec<_>>()
        .await?;
    indexed_messages.sort_by_key(|(idx, _, _)| *idx);
    let acked_pending = indexed_messages
        .iter()
        .filter(|(_, _, acked)| *acked)
        .count() as i64;
    let messages = indexed_messages
        .into_iter()
        .map(|(_, message, _)| message)
        .collect();

    Ok(ActorInboxResponse {
        messages,
        next_cursor,
        pending_count: pending_count.saturating_sub(acked_pending),
    })
}

pub(super) fn map_actor_service_error(operation: &str, err: ActorServiceError) -> anyhow::Error {
    anyhow::anyhow!("{operation} failed ({:?}): {}", err.code, err.message)
}
