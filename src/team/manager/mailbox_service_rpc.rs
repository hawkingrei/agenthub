use agenthub_team_actor::{
    ActorAckRequest, ActorAckResponse, ActorInboxRequest, ActorInboxResponse, ActorMailboxService,
    ActorSendRequest, ActorSendResponse, ActorServiceError, ActorTaskLinkRequest,
    ActorTaskLinkResponse, ActorTriageRequest, ActorTriageResponse,
};
use async_trait::async_trait;

use super::mailbox_service::TeamActorMailboxService;

#[async_trait]
impl ActorMailboxService for TeamActorMailboxService {
    async fn actor_send(
        &self,
        request: ActorSendRequest,
    ) -> Result<ActorSendResponse, ActorServiceError> {
        self.actor_send_impl(request).await
    }

    async fn actor_inbox(
        &self,
        request: ActorInboxRequest,
    ) -> Result<ActorInboxResponse, ActorServiceError> {
        self.actor_inbox_impl(request).await
    }

    async fn actor_ack(
        &self,
        request: ActorAckRequest,
    ) -> Result<ActorAckResponse, ActorServiceError> {
        self.actor_ack_impl(request).await
    }

    async fn actor_triage(
        &self,
        request: ActorTriageRequest,
    ) -> Result<ActorTriageResponse, ActorServiceError> {
        self.actor_triage_impl(request).await
    }

    async fn actor_task_link(
        &self,
        request: ActorTaskLinkRequest,
    ) -> Result<ActorTaskLinkResponse, ActorServiceError> {
        self.actor_task_link_impl(request).await
    }
}
