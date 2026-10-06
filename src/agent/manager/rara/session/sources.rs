use agenthub_rara::{ConnectionStatus, SourceRegistration};

use super::*;

impl RaraHandle {
    pub(crate) async fn register_loop_sources(
        &self,
        sources: Vec<SourceRegistration>,
    ) -> anyhow::Result<()> {
        SourceRegistration::validate_batch(&sources, self.client.handshake())?;
        let _operation = self.authorize_loop_input().await?;
        let _gate = self.input_gate.lock().await;
        self.await_admitted_events().await?;
        self.verify_loop_input().await?;
        {
            let mut state = self.state.write().await;
            anyhow::ensure!(
                !state.sources_registered
                    && !state.input_attempted
                    && !state.terminal_turn
                    && matches!(
                        state.phase,
                        SessionPhase::Idle
                            | SessionPhase::AwaitingInput { .. }
                            | SessionPhase::RecoveryRequired { .. }
                    ),
                "direct loop sources require an unused native session"
            );
            // A partially acknowledged bootstrap cannot be retried on this session.
            state.sources_registered = true;
        }
        let result = async {
            for source in sources {
                let kind = match &source {
                    SourceRegistration::Prompt { .. } => "prompt",
                    SourceRegistration::Skill { .. } => "skill",
                    SourceRegistration::Mcp(_) => "MCP",
                };
                let previous_sequence = *self.progress.borrow();
                let ack = receipts::control(
                    &self.tasks,
                    &self.client,
                    &self.store,
                    Some(self.stream.native_session_id()),
                    ControlRequest::RegisterSource(source),
                )
                .await?;
                anyhow::ensure!(
                    matches!(
                        ack,
                        RuntimeRequestAck::Accepted {
                            last_sequence: Some(sequence),
                            ..
                        } if sequence > previous_sequence
                    ),
                    "direct runtime did not acknowledge a required {kind} source event prefix: {ack:?} (previous sequence {previous_sequence})"
                );
                self.record_ack_cursor(&ack);
                self.await_admitted_events().await?;
            }
            Ok(())
        }
        .await;
        if result.is_err() {
            self.client.abort();
        }
        result
    }

    pub(crate) async fn loop_turn_complete(&self) -> bool {
        if !matches!(self.client.status(), ConnectionStatus::Running) {
            return true;
        }
        let state = self.state.read().await;
        if state.guard.as_ref().and_then(|guard| guard.reentry.as_ref()).is_some_and(|check| {
            check.accepted && matches!(&check.evaluation, Some((_, agenthub_rara::SemanticGuardEvent::Decided { decision })) if decision.is_decline())
        }) {
            return true;
        }
        state.terminal_turn && state.pending.is_none() && matches!(state.phase, SessionPhase::Idle)
    }
}
