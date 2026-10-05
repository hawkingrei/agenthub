use agenthub_rara::{ConnectionStatus, GuardedPrompt, SemanticGuardContext, SemanticGuardDecision};

use super::*;

pub(super) struct GuardedActivation {
    pub context: SemanticGuardContext,
    pub request_id: String,
    pub task_id: Option<String>,
    pub accepted_turn: Option<String>,
    pub completed: Option<(String, SemanticGuardDecision)>,
}

impl RaraHandle {
    pub(crate) async fn configure_loop_guard(
        &self,
        context: SemanticGuardContext,
        task_id: Option<String>,
        request_id: String,
    ) -> anyhow::Result<()> {
        GuardedPrompt::require_capability(self.client.handshake())?;
        let _gate = self.input_gate.lock().await;
        let mut state = self.state.write().await;
        anyhow::ensure!(
            state.sources_registered && !state.input_attempted && state.guard.is_none(),
            "native guard requires an unused configured activation"
        );
        state.guard = Some(GuardedActivation {
            context,
            request_id,
            task_id,
            accepted_turn: None,
            completed: None,
        });
        Ok(())
    }

    pub(crate) async fn semantic_loop_outcome(
        &self,
    ) -> Option<(SemanticGuardDecision, Option<String>)> {
        if !matches!(self.client.status(), ConnectionStatus::Running)
            || *self.delivery.borrow() == Some(false)
        {
            return None;
        }
        let state = self.state.read().await;
        if !state.terminal_turn
            || state.pending.is_some()
            || !matches!(state.phase, SessionPhase::Idle)
            || state.sequence < self.ack_cursor.load(Ordering::Acquire)
        {
            return None;
        }
        let guard = state.guard.as_ref()?;
        let (turn, decision) = guard.completed.as_ref()?;
        (guard.accepted_turn.as_ref() == Some(turn))
            .then(|| (decision.clone(), guard.task_id.clone()))
    }
}
