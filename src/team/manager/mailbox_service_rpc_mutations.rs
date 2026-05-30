use agenthub_team_actor::{
    ActorAckRequest, ActorAckResponse, ActorServiceError, ActorServiceErrorCode,
    ActorTaskLinkRequest, ActorTaskLinkResponse, ActorTriageRequest, ActorTriageResponse,
};

use super::mailbox::{map_actor_service_error, required_trimmed_field};
use super::mailbox_service::TeamActorMailboxService;

impl TeamActorMailboxService {
    pub(super) async fn actor_ack_impl(
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

    pub(super) async fn actor_triage_impl(
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
            .triage_actor_message(
                run_id,
                actor_id,
                request.message_id,
                request.disposition,
                request.reason,
            )
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

    pub(super) async fn actor_task_link_impl(
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
