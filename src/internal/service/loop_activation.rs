use agenthub_agent_domain::loop_runtime::{LoopOutcome, LoopToolStatus, LoopToolSurface};
use agenthub_db::loop_runtime::{LoopStore, LoopToolObservation};
use tracing::Instrument;

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ExecutionAdmission {
    Running,
    Bootstrap,
    FinishReplay,
}

impl TeamInternalControlService {
    pub(super) async fn complete_control_request<T: Send + 'static>(
        &self,
        metadata: &MetadataMap,
        operation_name: &'static str,
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
        let span = tracing::info_span!(
            "loop.control_rpc",
            activation_id = executor
                .loop_execution
                .as_ref()
                .map(|value| value.activation_id.as_str()),
            generation = executor
                .loop_execution
                .as_ref()
                .map(|value| value.generation),
            actor_id = executor.actor_id.as_deref(),
            mailbox_run_id = executor.run_id.as_deref(),
            rpc = operation_name,
        );
        let service = self.clone();
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
                    let observation = service
                        .start_control_observation(&executor, operation_name)
                        .await;
                    let context = crate::team::loop_context::LoopSchedulingContext {
                        actor_id: executor.actor_id,
                        activation_id: executor
                            .loop_execution
                            .map(|execution| execution.activation_id),
                        user_id: None,
                    };
                    let result =
                        crate::team::loop_context::with_scheduling_context(context, operation)
                            .await;
                    let status = match &result {
                        Ok(_) => LoopToolStatus::Succeeded,
                        Err(error)
                            if matches!(
                                error.code(),
                                tonic::Code::Cancelled
                                    | tonic::Code::Unknown
                                    | tonic::Code::DeadlineExceeded
                                    | tonic::Code::Unavailable
                                    | tonic::Code::Internal
                                    | tonic::Code::DataLoss
                            ) =>
                        {
                            LoopToolStatus::OutcomeUnknown
                        }
                        Err(_) => LoopToolStatus::Failed,
                    };
                    if let Some(observation) = observation
                        && LoopStore::new(service.deps.db.clone())
                            .complete_tool_observation(
                                observation,
                                status,
                                chrono::Utc::now().timestamp(),
                            )
                            .await
                            .is_err()
                    {
                        tracing::warn!(
                            "loop control completion observation could not be persisted"
                        );
                    }
                    tracing::debug!(status = status.as_str(), "loop control boundary returned");
                    let _ = sender.send(result);
                    Ok(())
                }
                .instrument(span),
            )
            .map_err(|_| Status::unavailable("loop control is shutting down"))?;
        receiver
            .await
            .map_err(|_| Status::unavailable("loop control request did not settle"))?
    }

    async fn start_control_observation(
        &self,
        principal: &super::super::auth::InternalPrincipal,
        operation_name: &'static str,
    ) -> Option<LoopToolObservation> {
        let execution = principal.loop_execution.as_ref()?;
        let store = LoopStore::new(self.deps.db.clone());
        let reservation = store
            .executor_reservation(
                principal.actor_id.as_deref()?,
                principal.run_id.as_deref()?,
                &execution.activation_id,
                execution.generation,
            )
            .await
            .ok()?;
        if reservation.owner_id != self.deps.agents.loop_owner_id() {
            return None;
        }
        // Observation is separate from handler authorization. In particular, a historical
        // finish replay remains valid even when no live reservation can issue an observation.
        store
            .begin_tool_observation(
                &reservation,
                LoopToolSurface::ControlRpc,
                operation_name,
                None,
                chrono::Utc::now().timestamp(),
            )
            .await
            .ok()
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
        let admission = if allow_finish_replay {
            ExecutionAdmission::FinishReplay
        } else {
            ExecutionAdmission::Running
        };
        self.authenticate_execution_admission(metadata, admission)
            .await
    }

    pub(super) async fn authenticate_execution_admission(
        &self,
        metadata: &MetadataMap,
        admission: ExecutionAdmission,
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
        if admission != ExecutionAdmission::FinishReplay {
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
            let now = chrono::Utc::now().timestamp();
            let validation = if admission == ExecutionAdmission::Bootstrap {
                store
                    .verify_executor_bootstrap_live(&reservation, now)
                    .await
            } else {
                store.verify_executor_live(&reservation, now).await
            };
            validation.map_err(|_| {
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
