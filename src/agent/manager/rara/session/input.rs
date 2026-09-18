use agenthub_agent_event_codec::{encode_message_for_storage, persist_agent_event};
use agenthub_db::runtime_events::{
    RuntimeEventError, RuntimeHistoryEntry, RuntimeRequestIntent, RuntimeRequestStatus,
};
use agenthub_rara::{InputTarget, PendingInputKind};
use chrono::Utc;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::*;
use crate::agent::{AgentSendInputError, OutputStream};

impl RaraHandle {
    pub(crate) async fn send_input(
        &self,
        text: &str,
        message_id: Option<&str>,
        target: Option<&InputTarget>,
        origin: Option<Value>,
    ) -> anyhow::Result<()> {
        let id = message_id
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::now_v7().to_string());
        let runtime = self.clone();
        let text = text.to_owned();
        let target = target.cloned();
        let (reply, response) = oneshot::channel();
        self.tasks
            .spawn_runtime_task(format!("direct-input:{id}"), async move {
                let result = runtime.submit_input(text, id, target, origin).await;
                let _ = reply.send(result);
                Ok(())
            })?;
        response
            .await
            .map_err(|_| anyhow::anyhow!("direct input owner stopped"))?
    }

    async fn submit_input(
        &self,
        text: String,
        id: String,
        target: Option<InputTarget>,
        origin: Option<Value>,
    ) -> anyhow::Result<()> {
        let _gate = self.input_gate.lock().await;
        self.await_admitted_events().await?;
        let request = {
            let state = self.state.read().await;
            match target {
                Some(ref target) => {
                    let pending = state
                        .pending
                        .as_ref()
                        .ok_or(AgentSendInputError::NativeInputMismatch)?;
                    anyhow::ensure!(
                        target.runtime_id == self.store.runtime_id()
                            && target.session_id == self.stream.native_session_id()
                            && target.turn_id == pending.turn_id
                            && matches!(pending.kind, PendingInputKind::User { .. }),
                        AgentSendInputError::NativeInputMismatch
                    );
                    ControlRequest::UserAnswer {
                        turn_id: pending.turn_id.clone(),
                        answer: text.clone(),
                    }
                }
                None => {
                    anyhow::ensure!(
                        state.pending.is_none(),
                        AgentSendInputError::NativeInputRequired
                    );
                    if matches!(
                        state.phase,
                        SessionPhase::Running { .. } | SessionPhase::Cancelling { .. }
                    ) || origin.is_some()
                    {
                        ControlRequest::FollowUp {
                            prompt: text.clone(),
                        }
                    } else {
                        ControlRequest::Prompt {
                            prompt: text.clone(),
                        }
                    }
                }
            }
        };
        let frame = request.frame(
            self.store.runtime_id(),
            &id,
            Some(self.stream.native_session_id()),
        )?;
        let ts = Utc::now().timestamp();
        let seq = Uuid::now_v7().to_string();
        let mut message = json!({"type":"user_message", "text":text, "chunk":false, "message_id":id,
            "meta":{"delivery":"pending", "provider_runtime":{"provider":"rara","runtime_id":self.store.runtime_id(),
                "native_session_id":self.stream.native_session_id(),"request_id":id}}});
        if let Some(origin) = origin {
            message["origin"] = origin;
        }
        let message = message.to_string();
        let encoded = encode_message_for_storage(&OutputStream::Acp, &message);
        let prepared = self
            .store
            .prepare_input_request(
                RuntimeRequestIntent {
                    request_id: &id,
                    kind: receipts::kind(request.kind()),
                    target_session_id: Some(self.stream.native_session_id()),
                    expected_turn_id: request.expected_turn_id(),
                },
                ts,
                RuntimeHistoryEntry {
                    seq: &seq,
                    ts,
                    stream: OutputStream::Acp,
                    message: &encoded,
                },
            )
            .await;
        let event_id = match prepared {
            Ok(id) => id,
            Err(error)
                if matches!(
                    error.downcast_ref::<RuntimeEventError>(),
                    Some(RuntimeEventError::RequestReused)
                ) =>
            {
                return Err(AgentSendInputError::NativeRequestReused { request_id: id }.into());
            }
            Err(error) => return Err(error),
        };
        let _ = self.output_tx.send(AgentOutput {
            event_id,
            agent_id: self.agent_id.clone(),
            session_id: self.store.local_session_id().into(),
            seq,
            ts,
            stream: OutputStream::Acp,
            message,
        });
        let result = match self
            .store
            .mark_request_sent(&id, Utc::now().timestamp())
            .await
        {
            Ok(permit) => receipts::send_prepared(&self.client, &self.store, frame, permit).await,
            Err(error) => {
                self.client.abort();
                self.store.close(Utc::now().timestamp()).await?;
                Err(error)
            }
        };
        if let Ok(ack) = &result {
            self.record_ack_cursor(ack);
        }
        if let Some(receipt) = self.store.request_receipt(&id).await? {
            self.emit_history(
                json!({"type":"input_receipt","message_id":id,"receipt":receipt,
                "meta":{"provider_runtime":{"provider":"rara","runtime_id":self.store.runtime_id(),
                    "native_session_id":self.stream.native_session_id(),"request_id":id}}}),
            )
            .await?;
            if result.is_ok()
                && matches!(
                    receipt.status,
                    RuntimeRequestStatus::Accepted | RuntimeRequestStatus::Queued
                )
            {
                return Ok(());
            }
        }
        Err(AgentSendInputError::NativeInputNotAccepted { request_id: id }.into())
    }

    pub(super) async fn emit_history(&self, value: Value) -> anyhow::Result<()> {
        let message = value.to_string();
        let seq = Uuid::now_v7().to_string();
        let ts = Utc::now().timestamp();
        let event_id = persist_agent_event(
            &self.event_dbs,
            self.idle_gc.as_ref(),
            &self.agent_id,
            self.store.local_session_id(),
            &seq,
            ts,
            &OutputStream::Acp,
            &message,
        )
        .await?;
        let _ = self.output_tx.send(AgentOutput {
            event_id,
            agent_id: self.agent_id.clone(),
            session_id: self.store.local_session_id().into(),
            seq,
            ts,
            stream: OutputStream::Acp,
            message,
        });
        Ok(())
    }
}
