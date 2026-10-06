use agenthub_rara::RecoveryStatus;
use tokio::sync::oneshot;

use super::super::NativeRecoveryView;
use super::*;

impl RaraHandle {
    pub(crate) async fn query_recovery(&self) -> anyhow::Result<NativeRecoveryView> {
        let runtime = self.clone();
        let (reply, response) = oneshot::channel();
        self.tasks.spawn_runtime_task(
            format!("direct-recovery-query:{}", Uuid::now_v7()),
            async move {
                let result = runtime.read_recovery().await;
                let _ = reply.send(result);
                Ok(())
            },
        )?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("native recovery query owner stopped"))?
    }

    async fn read_recovery(&self) -> anyhow::Result<NativeRecoveryView> {
        // Observation needs current process ownership, but grants no execution lease.
        let _operation = match &self.loop_owner {
            Some(owner) => Some(owner.operations.clone().read_owned().await),
            None => None,
        };
        anyhow::ensure!(
            self.state.read().await.entry_ready,
            "native recovery entry is not ready"
        );
        let _gate = self.input_gate.lock().await;
        self.await_admitted_events().await?;
        let recovery = self.query_recovery_state().await?;
        Ok(NativeRecoveryView {
            local_session_id: self.store.local_session_id().into(),
            runtime_id: self.store.runtime_id().into(),
            session_id: self.stream.native_session_id().into(),
            recovery,
        })
    }

    /// The caller holds input_gate and has consumed the preceding admitted prefix.
    pub(super) async fn query_recovery_state(&self) -> anyhow::Result<RecoveryStatus> {
        self.client
            .handshake()
            .require_methods(&["session.query_recovery"])?;
        anyhow::ensure!(
            !matches!(
                self.state.read().await.phase,
                SessionPhase::Running { .. } | SessionPhase::Cancelling { .. }
            ),
            "native recovery query requires an idle or blocked session"
        );
        let before = *self.progress.borrow();
        let ack = receipts::control(
            &self.tasks,
            &self.client,
            &self.store,
            Some(self.stream.native_session_id()),
            ControlRequest::QueryRecovery,
        )
        .await?;
        let result = async {
            let RuntimeRequestAck::Accepted {
                last_sequence: Some(cursor),
                turn_id: None,
                ..
            } = ack
            else {
                anyhow::bail!("native recovery query was not acknowledged");
            };
            anyhow::ensure!(cursor > before, "native recovery query prefix is stale");
            self.record_ack_cursor(&ack);
            self.await_admitted_events().await?;
            let state = self.state.read().await;
            anyhow::ensure!(
                state.recovery_sequence > before && state.recovery_sequence <= cursor,
                "native recovery query has no matching committed state"
            );
            state
                .recovery
                .clone()
                .ok_or_else(|| anyhow::anyhow!("native recovery state is unavailable"))
        }
        .await;
        if result.is_err() && matches!(ack, RuntimeRequestAck::Accepted { .. }) {
            self.client.abort();
        }
        result
    }
}
