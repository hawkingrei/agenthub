use agenthub_agent_domain::loop_runtime::LoopOutcome;
use agenthub_db::loop_runtime::LoopStore;

use super::*;

impl TeamInternalControlService {
    pub(super) async fn complete_control_request<T: Send + 'static>(
        &self,
        metadata: &MetadataMap,
        operation: impl std::future::Future<Output = Result<Response<T>, Status>> + Send + 'static,
    ) -> Result<Response<T>, Status> {
        let executor = self
            .authz
            .authenticate(metadata)
            .ok()
            .filter(|principal| principal.loop_execution.is_some());
        let Some(executor) = executor else {
            return operation.await;
        };
        let (sender, receiver) = tokio::sync::oneshot::channel();
        // SQLite may finish an enqueued write after its caller disconnects. Keep the entire
        // admitted request and its operation guard owned by the daemon until it settles.
        self.deps
            .agents
            .daemon_tasks()
            .spawn_runtime_task(
                format!(
                    "loop-control:{}",
                    executor.actor_id.as_deref().unwrap_or("unknown")
                ),
                async move {
                    let _ = sender.send(operation.await);
                    Ok(())
                },
            )
            .map_err(|_| Status::unavailable("loop control is shutting down"))?;
        receiver
            .await
            .map_err(|_| Status::unavailable("loop control request did not settle"))?
    }

    pub(super) async fn load_team_context(
        &self,
        principal: &super::super::auth::InternalPrincipal,
        team_id: Option<&str>,
        run_id: Option<&str>,
        actor_id: &str,
    ) -> Result<crate::team::TeamContextRecord, Status> {
        if principal.loop_execution.is_none() {
            return load_team_context_for_actor(
                &self.deps.agents,
                &self.deps.teams,
                team_id,
                run_id,
                actor_id,
            )
            .await;
        }
        self.authz
            .ensure_worker_actor(principal, actor_id, "actor_id")?;
        if let Some(run) = run_id {
            self.authz.ensure_run_scope(principal, run)?;
        }
        // The signed mailbox is canonical. Reading recovery state must not poll/clean a process
        // while this request holds its operation guard or infer authority from historical runs.
        let run_id = principal
            .run_id
            .as_deref()
            .ok_or_else(|| Status::permission_denied("executor mailbox is required"))?;
        let resolved_team = self
            .deps
            .teams
            .resolve_team_scope(team_id, Some(run_id))
            .await
            .map_err(map_team_context_error)?;
        ensure_team_member_access(&self.deps.teams, &resolved_team, actor_id).await?;
        self.deps
            .teams
            .describe_team_context(Some(&resolved_team), Some(run_id))
            .await
            .map_err(map_team_context_error)
    }

    pub(super) async fn authenticate_execution(
        &self,
        metadata: &MetadataMap,
        allow_finish_replay: bool,
    ) -> Result<
        (
            super::super::auth::InternalPrincipal,
            Option<tokio::sync::OwnedRwLockReadGuard<()>>,
        ),
        Status,
    > {
        let principal = self.authz.authenticate(metadata)?;
        let Some(executor) = principal.loop_execution.as_ref() else {
            return Ok((principal, None));
        };
        let actor = principal
            .actor_id
            .as_deref()
            .ok_or_else(|| Status::permission_denied("executor actor is required"))?;
        let run = principal
            .run_id
            .as_deref()
            .ok_or_else(|| Status::permission_denied("executor mailbox is required"))?;
        let guard = self
            .deps
            .agents
            .loop_operation_gate(actor)
            .await
            .read_owned()
            .await;
        if !allow_finish_replay {
            let store = LoopStore::new(self.deps.db.clone());
            let reservation = store
                .executor_reservation(actor, run, &executor.activation_id, executor.generation)
                .await
                .map_err(|_| {
                    Status::permission_denied("executor scope does not match the activation")
                })?;
            if reservation.owner_id != self.deps.agents.loop_owner_id() {
                return Err(Status::permission_denied(
                    "executor belongs to an earlier daemon",
                ));
            }
            store
                .verify_executor_live(&reservation, chrono::Utc::now().timestamp())
                .await
                .map_err(|_| {
                    Status::permission_denied("executor generation is no longer active")
                })?;
        }
        Ok((principal, Some(guard)))
    }

    pub(super) async fn finish_loop_activation_request(
        &self,
        request: Request<FinishLoopActivationRequest>,
    ) -> Result<Response<FinishLoopActivationResponse>, Status> {
        let (principal, _execution_guard) = self
            .authenticate_execution(request.metadata(), true)
            .await?;
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
        if reservation.owner_id != self.deps.agents.loop_owner_id() {
            let recorded = store
                .activation(&reservation.team_id, &executor.activation_id)
                .await
                .map_err(|_| Status::unavailable("could not inspect finish receipt"))?
                .is_some_and(|activation| activation.outcome.is_some());
            if !recorded {
                return Err(Status::permission_denied(
                    "executor belongs to an earlier daemon",
                ));
            }
        }
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
