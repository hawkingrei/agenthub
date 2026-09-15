use agenthub_agent_domain::loop_runtime::LoopOutcome;
use agenthub_db::loop_runtime::LoopStore;

use super::*;

impl TeamInternalControlService {
    pub(super) async fn finish_loop_activation_request(
        &self,
        request: Request<FinishLoopActivationRequest>,
    ) -> Result<Response<FinishLoopActivationResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::LoopFinish)?;
        let executor = principal
            .loop_execution
            .as_ref()
            .ok_or_else(|| Status::permission_denied("activation-scoped credentials required"))?;
        let actor_id = principal
            .actor_id
            .as_deref()
            .ok_or_else(|| Status::permission_denied("executor actor is required"))?;
        let run_id = principal
            .run_id
            .as_deref()
            .ok_or_else(|| Status::permission_denied("executor mailbox scope is required"))?;
        if executor.generation <= 0 {
            return Err(Status::permission_denied("invalid executor generation"));
        }
        let payload = request.into_inner();
        if payload.outcome_json.len() > 16_384 {
            return Err(Status::invalid_argument(
                "outcome exceeds the bounded finish request",
            ));
        }
        let outcome: LoopOutcome = serde_json::from_str(&payload.outcome_json)
            .map_err(|_| Status::invalid_argument("invalid structured loop outcome"))?;
        outcome
            .validate()
            .map_err(|_| Status::invalid_argument("invalid structured loop outcome"))?;
        let store = LoopStore::new(self.deps.db.clone());
        let reservation = store
            .executor_reservation(
                actor_id,
                run_id,
                &executor.activation_id,
                executor.generation,
            )
            .await
            .map_err(|_| {
                Status::permission_denied("executor scope does not match the activation")
            })?;
        let receipt = store
            .finish(&reservation, &outcome, chrono::Utc::now().timestamp())
            .await
            .map_err(|_| {
                Status::failed_precondition("activation cannot accept this finish request")
            })?;
        // Recording is independent of cleanup success. The receipt remains durable and a cleanup
        // failure keeps the writer fenced; an authenticated retry returns the same receipt.
        self.deps
            .agents
            .schedule_loop_finalization(reservation)
            .map_err(|_| {
                Status::unavailable("outcome recorded; cleanup scheduling requires retry")
            })?;
        Ok(Response::new(FinishLoopActivationResponse {
            receipt_json: serde_json::to_string(&receipt)
                .map_err(|_| Status::internal("could not encode finish receipt"))?,
        }))
    }
}
