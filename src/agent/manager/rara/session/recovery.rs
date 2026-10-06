use agenthub_agent_domain::loop_runtime::LoopReservation;
use agenthub_db::loop_runtime::LoopStore;
use agenthub_rara::{GuardedPrompt, ReentryGuard, ReentryTarget, SemanticGuardEvent};
use chrono::Utc;
use tokio::sync::OwnedRwLockReadGuard;

use super::*;

#[derive(Clone)]
pub(super) struct LoopOwner {
    pub store: LoopStore,
    pub reservation: LoopReservation,
    pub operations: Arc<RwLock<()>>,
}

pub(super) struct ReentryCheck {
    pub target: ReentryTarget,
    pub evaluation: Option<(u64, SemanticGuardEvent)>,
    pub accepted: bool,
}

pub(super) fn matches_target(target: &ReentryTarget, state: &LiveState) -> bool {
    match (target, &state.phase) {
        (ReentryTarget::Waiting { turn_id }, SessionPhase::AwaitingInput { turn_id: current }) => {
            turn_id == current
                && state
                    .pending
                    .as_ref()
                    .is_some_and(|pending| &pending.turn_id == turn_id)
        }
        (
            ReentryTarget::Recovery { recovery_id },
            SessionPhase::RecoveryRequired {
                recovery_id: current,
            },
        ) => recovery_id == current && state.pending.is_none(),
        _ => false,
    }
}

impl RaraHandle {
    pub(super) async fn verify_input_owner(&self) -> anyhow::Result<()> {
        if let Some(owner) = &self.loop_owner {
            owner
                .store
                .verify_executor_live(&owner.reservation, Utc::now().timestamp())
                .await?;
        }
        if let Some(owner) = &self.standalone_owner {
            owner.store.verify_live(&owner.reservation).await?;
        }
        Ok(())
    }

    pub(super) async fn authorize_input_owner(
        &self,
    ) -> anyhow::Result<Option<OwnedRwLockReadGuard<()>>> {
        let operations = if let Some(owner) = &self.loop_owner {
            &owner.operations
        } else if let Some(owner) = &self.standalone_owner {
            &owner.operations
        } else {
            return Ok(None);
        };
        let operation = operations.clone().read_owned().await;
        self.verify_input_owner().await?;
        Ok(Some(operation))
    }

    pub(super) async fn publish_pending_permission(&self) -> anyhow::Result<()> {
        let state = self.state.read().await;
        let pending = state
            .pending
            .clone()
            .zip(state.pending_tool_call.clone())
            .filter(|_| state.entry_ready);
        drop(state);
        if let Some((pending, tool_call_id)) = pending {
            self.request_permission(pending, tool_call_id).await?;
        }
        Ok(())
    }

    /// A restored wait is entered by a nonexecuting control, never an ordinary prompt.
    pub(crate) async fn enter_recovered_loop(&self, prompt: &str) -> anyhow::Result<bool> {
        let _operation = self.authorize_input_owner().await?;
        let _gate = self.input_gate.lock().await;
        self.await_admitted_events().await?;
        self.verify_input_owner().await?;
        let (request, request_id) = {
            let mut state = self.state.write().await;
            let target = match &state.phase {
                SessionPhase::Idle => return Ok(false),
                SessionPhase::AwaitingInput { turn_id } => ReentryTarget::Waiting {
                    turn_id: turn_id.clone(),
                },
                SessionPhase::RecoveryRequired { recovery_id } => ReentryTarget::Recovery {
                    recovery_id: recovery_id.clone(),
                },
                _ => anyhow::bail!("native recovery entry requires a blocked session"),
            };
            self.client
                .handshake()
                .require_methods(&["session.query_recovery", "session.evaluate_reentry"])?;
            anyhow::ensure!(
                self.client.handshake().capabilities.approval_persistence,
                "native durable interaction capability is missing"
            );
            let guard = state
                .guard
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("native reentry context is missing"))?;
            anyhow::ensure!(
                guard.reentry.is_none(),
                "native recovery entry was already attempted"
            );
            let request = ReentryGuard {
                target: target.clone(),
                guard: GuardedPrompt {
                    prompt: prompt.into(),
                    context: guard.context.clone(),
                },
            };
            request.validate()?;
            guard.reentry = Some(ReentryCheck {
                target,
                evaluation: None,
                accepted: false,
            });
            let request_id = guard.request_id.clone();
            state.input_attempted = true;
            (request, request_id)
        };
        let result = async {
            self.query_recovery_state().await?;
            self.verify_input_owner().await?;
            let ack = receipts::control_with_id(
                &self.tasks,
                &self.client,
                &self.store,
                Some(self.stream.native_session_id()),
                ControlRequest::EvaluateReentry(request),
                request_id,
            )
            .await?;
            let RuntimeRequestAck::Accepted {
                last_sequence: Some(cursor),
                turn_id: None,
                ..
            } = ack
            else {
                anyhow::bail!("native reentry evaluation was not acknowledged");
            };
            self.record_ack_cursor(&ack);
            self.await_admitted_events().await?;
            self.verify_input_owner().await?;
            let mut state = self.state.write().await;
            anyhow::ensure!(
                state
                    .guard
                    .as_ref()
                    .and_then(|guard| guard.reentry.as_ref())
                    .is_some_and(|check| matches_target(&check.target, &state)),
                "native reentry state changed before its receipt committed"
            );
            let guard = state
                .guard
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("native entry context disappeared"))?;
            let check = guard
                .reentry
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("native entry target disappeared"))?;
            let (sequence, result) = check
                .evaluation
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("native reentry result is missing"))?;
            anyhow::ensure!(
                *sequence <= cursor,
                "native reentry result is outside its receipt prefix"
            );
            let declined =
                matches!(result, SemanticGuardEvent::Decided { decision } if decision.is_decline());
            check.accepted = true;
            state.entry_ready = !declined;
            drop(state);
            self.publish_pending_permission().await?;
            Ok(true)
        }
        .await;
        if result.is_err() {
            self.client.abort();
        }
        result
    }
}
