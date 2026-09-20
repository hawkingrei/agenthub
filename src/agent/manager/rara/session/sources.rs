use agenthub_rara::{ConnectionStatus, SourceRegistration};

use super::*;

impl RaraHandle {
    pub(crate) async fn register_loop_sources(
        &self,
        sources: Vec<SourceRegistration>,
    ) -> anyhow::Result<()> {
        SourceRegistration::validate_batch(&sources, self.client.handshake())?;
        let _gate = self.input_gate.lock().await;
        self.await_admitted_events().await?;
        {
            let mut state = self.state.write().await;
            anyhow::ensure!(
                !state.sources_registered
                    && !state.input_attempted
                    && !state.terminal_turn
                    && matches!(state.phase, SessionPhase::Idle)
                    && state.pending.is_none(),
                "direct loop sources require an unused native session"
            );
            // A partially acknowledged bootstrap cannot be retried on this session.
            state.sources_registered = true;
        }
        let result = async {
            for source in sources {
                let ack = receipts::control(
                    &self.tasks,
                    &self.client,
                    &self.store,
                    Some(self.stream.native_session_id()),
                    ControlRequest::RegisterSource(source),
                )
                .await?;
                anyhow::ensure!(
                    matches!(ack, RuntimeRequestAck::Accepted { .. }),
                    "direct runtime rejected a required loop source"
                );
                self.record_ack_cursor(&ack);
            }
            self.await_admitted_events().await
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
        state.terminal_turn && state.pending.is_none() && matches!(state.phase, SessionPhase::Idle)
    }
}
