use agenthub_rara::{RecoveryResolution, RecoveryTarget};
use tokio::sync::oneshot;

use super::*;

impl RaraHandle {
    /// Own reconciliation through caller disconnects; applying it never starts a turn.
    pub(crate) async fn reconcile_recovery(
        &self,
        target: RecoveryTarget,
        note: String,
    ) -> anyhow::Result<()> {
        target.validate()?;
        let resolution = RecoveryResolution {
            recovery_id: target.recovery_id.clone(),
            note,
        };
        resolution.validate()?;
        let runtime = self.clone();
        let (reply, response) = oneshot::channel();
        self.tasks.spawn_runtime_task(
            format!("direct-reconcile:{}", Uuid::now_v7()),
            async move {
                let result = runtime.apply_reconciliation(target, resolution).await;
                let _ = reply.send(result);
                Ok(())
            },
        )?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("native reconciliation owner stopped"))?
    }

    async fn apply_reconciliation(
        &self,
        target: RecoveryTarget,
        resolution: RecoveryResolution,
    ) -> anyhow::Result<()> {
        let _operation = self.authorize_loop_input().await?;
        anyhow::ensure!(
            self.state.read().await.entry_ready,
            "native recovery entry is not ready"
        );
        let _gate = self.input_gate.lock().await;
        self.await_admitted_events().await?;
        self.verify_loop_input().await?;
        anyhow::ensure!(
            target.runtime_id == self.store.runtime_id()
                && target.session_id == self.stream.native_session_id(),
            "native recovery owner changed"
        );
        self.client
            .handshake()
            .require_methods(&["session.resolve_recovery"])?;
        {
            let state = self.state.read().await;
            anyhow::ensure!(
                state.entry_ready && state.pending.is_none(),
                "native recovery entry is not ready"
            );
            let recovery = state
                .recovery
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("native recovery state is unavailable"))?;
            if matches!(state.phase, SessionPhase::Idle)
                && recovery.blocked.is_none()
                && recovery.last_resolution.as_ref() == Some(&resolution)
            {
                return Ok(());
            }
            anyhow::ensure!(
                matches!(&state.phase, SessionPhase::RecoveryRequired { recovery_id } if recovery_id == &target.recovery_id)
                    && recovery
                        .blocked
                        .as_ref()
                        .is_some_and(|block| block.recovery_id == target.recovery_id),
                "native recovery token changed"
            );
        }
        let result = async {
            let before = *self.progress.borrow();
            let ack = receipts::control(
                &self.tasks,
                &self.client,
                &self.store,
                Some(self.stream.native_session_id()),
                ControlRequest::ResolveRecovery(resolution.clone()),
            )
            .await?;
            let RuntimeRequestAck::Accepted {
                last_sequence: Some(cursor),
                turn_id: None,
                ..
            } = ack
            else {
                anyhow::bail!("native recovery resolution was not acknowledged");
            };
            anyhow::ensure!(
                cursor > before,
                "native recovery resolution prefix is stale"
            );
            self.record_ack_cursor(&ack);
            self.await_admitted_events().await?;
            let state = self.state.read().await;
            anyhow::ensure!(
                state.recovery_sequence > before
                    && state.recovery_sequence <= cursor
                    && matches!(state.phase, SessionPhase::Idle)
                    && state.pending.is_none()
                    && state
                        .recovery
                        .as_ref()
                        .is_some_and(|recovery| recovery.blocked.is_none()
                            && recovery.last_resolution.as_ref() == Some(&resolution)),
                "native recovery resolution has no matching committed state"
            );
            Ok(())
        }
        .await;
        if result.is_err() {
            self.client.abort();
        }
        result
    }
}
