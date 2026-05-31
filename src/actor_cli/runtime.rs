use std::sync::Arc;

use agenthub_team_actor::{
    ActorAckRequest, ActorInboxRequest, ActorInboxResponse, ActorMailboxService,
    ActorMessageHandlingDisposition, ActorMessageStatus, ActorServiceError, ActorServiceErrorCode,
    ActorTriageRequest,
};

use crate::actor_runtime_env::connect_runtime_internal_mailbox_service;
use crate::internal::auth::InternalAction;
use crate::internal::client::InternalGrpcMailboxClient;

async fn triage_received_message<S: ActorMailboxService + ?Sized>(
    service: &S,
    acked: agenthub_team_actor::ActorAckResponse,
) -> Result<agenthub_team_actor::ActorMessageRecord, ActorServiceError> {
    let claimed_request = ActorTriageRequest {
        run_id: acked.message.run_id.clone(),
        actor_id: acked.message.to_actor_id.clone(),
        message_id: acked.message.message_id,
        disposition: ActorMessageHandlingDisposition::Claimed,
        reason: None,
    };
    match service.actor_triage(claimed_request.clone()).await {
        Ok(triaged) => Ok(triaged.message),
        Err(err) if err.code == ActorServiceErrorCode::NotFound => Ok(acked.message),
        Err(err) if err.code == ActorServiceErrorCode::Conflict => {
            match service
                .actor_triage(ActorTriageRequest {
                    disposition: ActorMessageHandlingDisposition::Watching,
                    ..claimed_request
                })
                .await
            {
                Ok(triaged) => Ok(triaged.message),
                Err(watch_err) if watch_err.code == ActorServiceErrorCode::NotFound => {
                    Ok(acked.message)
                }
                Err(watch_err) => Err(watch_err),
            }
        }
        Err(err) => Err(err),
    }
}

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
    // Process mailbox acceptance serially so SQLite-backed runtime control does not
    // trip over write-lock contention while ack/triage mutates multiple rows.
    let mut indexed_messages = Vec::new();
    for (idx, message) in response.messages.into_iter().enumerate() {
        if message.status != ActorMessageStatus::Pending {
            indexed_messages.push((idx, message, false));
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
                let message = triage_received_message(service, acked).await?;
                indexed_messages.push((idx, message, true));
            }
            Err(err) if err.code == ActorServiceErrorCode::NotFound => {
                indexed_messages.push((idx, message, false));
            }
            Err(err) => return Err(err),
        }
    }
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
