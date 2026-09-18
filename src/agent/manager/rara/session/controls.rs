use chrono::Utc;

use super::*;

impl RaraHandle {
    /// Capture the currently owned turn once; a late request must not stop its successor.
    pub(crate) async fn stop_turn(&self, interrupt: bool) -> anyhow::Result<()> {
        let _gate = self.input_gate.lock().await;
        self.await_admitted_events().await?;
        let turn_id = match &self.state.read().await.phase {
            SessionPhase::Running { turn_id }
            | SessionPhase::Cancelling { turn_id }
            | SessionPhase::AwaitingInput { turn_id } => turn_id.clone(),
            _ => anyhow::bail!("direct runtime has no active or waiting turn"),
        };
        let request = if interrupt {
            ControlRequest::Interrupt {
                turn_id: turn_id.clone(),
            }
        } else {
            ControlRequest::Cancel {
                turn_id: turn_id.clone(),
            }
        };
        let result = receipts::control(
            &self.tasks,
            &self.client,
            &self.store,
            Some(self.stream.native_session_id()),
            request,
        )
        .await?;
        self.record_ack_cursor(&result);
        anyhow::ensure!(
            matches!(result, RuntimeRequestAck::Accepted { .. }),
            "direct runtime rejected turn cancellation"
        );
        // The event consumer will also retire the callback on discard. Doing it here
        // closes the ACK/event window without expiring a newer turn's interaction.
        let mut permission = self.permission.lock().await;
        if permission
            .as_ref()
            .is_some_and(|permission| permission.turn_id() == turn_id)
        {
            let active = permission.take().expect("matching permission");
            active.cancel(&self.permissions).await?;
        }
        drop(permission);
        let mut state = self.state.write().await;
        if matches!(&state.phase, SessionPhase::Running { turn_id: active } if active == &turn_id) {
            state.phase = SessionPhase::Cancelling { turn_id };
        }
        drop(state);
        self.emit_history(serde_json::json!({"type":"runtime_control", "action":if interrupt {"interrupt"} else {"cancel"},
            "status":"accepted", "updated_at":Utc::now().timestamp()})).await?;
        Ok(())
    }
}
