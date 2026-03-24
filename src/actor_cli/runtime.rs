use std::sync::Arc;

use agenthub_team_actor::{
    ActorInboxRequest, ActorInboxResponse, ActorMailboxService, ActorServiceError,
    actor_inbox_with_auto_ack,
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
    auto_ack: bool,
) -> Result<ActorInboxResponse, ActorServiceError> {
    if auto_ack {
        actor_inbox_with_auto_ack(service, request).await
    } else {
        service.actor_inbox(request).await
    }
}

pub(super) fn map_actor_service_error(operation: &str, err: ActorServiceError) -> anyhow::Error {
    anyhow::anyhow!("{operation} failed ({:?}): {}", err.code, err.message)
}
