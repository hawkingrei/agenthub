use std::sync::Arc;

use agenthub_team_actor::{
    ActorAckRequest, ActorInboxRequest, ActorInboxResponse, ActorMailboxService,
    ActorMessageStatus, ActorServiceError, ActorServiceErrorCode,
};

use crate::actor_runtime_env::connect_runtime_internal_mailbox_service;
use crate::internal::auth::InternalAction;
use crate::internal::client::InternalGrpcMailboxClient;

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
    let mut messages = Vec::with_capacity(response.messages.len());
    let mut acked_pending = 0_i64;

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
            Ok(acked) => {
                acked_pending += 1;
                messages.push(acked.message);
            }
            Err(err) if err.code == ActorServiceErrorCode::NotFound => messages.push(message),
            Err(err) => return Err(err),
        }
    }

    Ok(ActorInboxResponse {
        messages,
        next_cursor: response.next_cursor,
        pending_count: pending_count.saturating_sub(acked_pending),
    })
}

pub(super) fn map_actor_service_error(operation: &str, err: ActorServiceError) -> anyhow::Error {
    anyhow::anyhow!("{operation} failed ({:?}): {}", err.code, err.message)
}
