use std::collections::HashMap;

use super::*;
use crate::{
    policy::PreparedBatchCall,
    protocol::{MessageKind, correlation_id, message_kind},
};

pub struct McpBatchResult {
    pub event_delivery_lost: bool,
    pub http_status: u16,
}

impl JournaledMcpClient {
    /// The caller retains the shared workspace and executor guard until every admitted tool has
    /// settled. Responses remain individual journal facts even when carried in one HTTP frame.
    pub async fn run_batch(
        &self,
        batch: PreparedBatchCall,
        events: mpsc::Sender<Budgeted<HttpEvent>>,
    ) -> Result<McpBatchResult, McpCallError> {
        let mut operations = Vec::with_capacity(batch.tools.len());
        for (_, intent) in &batch.tools {
            operations.push(
                self.journal
                    .prepare(&batch.executor, intent, now())
                    .await
                    .map_err(journal_error)?,
            );
        }
        let permits = if operations.is_empty() {
            Vec::new()
        } else {
            let attempts: Vec<_> = operations
                .iter()
                .map(|operation| (operation.id.as_str(), operation.attempt_count))
                .collect();
            self.journal
                .begin_send_batch(&batch.executor, &attempts, now())
                .await
                .map_err(journal_error)?
        };
        let mut pending: HashMap<_, _> = batch
            .tools
            .iter()
            .zip(permits)
            .map(|((id, _), permit)| (correlation_id(id), permit))
            .collect();
        let mut expected = batch.expected;
        let mut delivery_lost = false;
        let mut task_events = TaskEventDrain::new(None, batch.transport.timeout());
        let mut http_status = 200;
        let observed: Result<(), McpCallError> = async {
            let mut exchange = batch.transport.send_resumable(batch.request).await?;
            http_status = exchange.status_code();
            while let Some(event) = exchange.next_event().await? {
                http_status = exchange.status_code();
                if let Some(message) = &event.message {
                    let members = message
                        .as_array()
                        .map(Vec::as_slice)
                        .unwrap_or(std::slice::from_ref(message));
                    for response in members {
                        if message_kind(response)? != MessageKind::Response {
                            continue;
                        }
                        if exchange.status_code() >= 400
                            && response.get("id").is_none_or(Value::is_null)
                            && response.get("error").is_some()
                        {
                            let completion = classify_completion(response, None)?;
                            for permit in pending.values() {
                                self.journal
                                    .complete(permit, &completion, now())
                                    .await
                                    .map_err(journal_error)?;
                            }
                            pending.clear();
                            expected.clear();
                            continue;
                        }
                        let key = correlation_id(&response["id"]);
                        if !expected.contains(&key) {
                            return Err(McpTransportError::InvalidResponse.into());
                        }
                        if let Some(permit) = pending.get(&key) {
                            let completion = classify_completion(response, None)?;
                            self.journal
                                .complete(permit, &completion, now())
                                .await
                                .map_err(journal_error)?;
                            pending.remove(&key);
                        }
                        expected.remove(&key);
                    }
                }
                let forward = if let Some(message) = &event.message {
                    matches!(
                        task_events.accept(message).await,
                        TaskEventDisposition::Forward
                    )
                } else {
                    true
                };
                // March batching cannot carry task notifications. Still commit each actual
                // response above before rejecting delivery of an invalid mixed frame.
                if !forward || !self.deliver(event, &events) {
                    delivery_lost = true;
                }
                if expected.is_empty() {
                    return Ok(());
                }
            }
            if expected.is_empty() {
                Ok(())
            } else {
                Err(McpTransportError::Disconnected.into())
            }
        }
        .await;
        if let Err(mut error) = observed {
            let reason = match error {
                McpCallError::Transport(McpTransportError::Deadline) => {
                    McpAmbiguityReason::Deadline
                }
                McpCallError::Transport(McpTransportError::Disconnected) => {
                    McpAmbiguityReason::TransportLost
                }
                _ => McpAmbiguityReason::InvalidResponse,
            };
            // Already completed members are absent from pending. A truncated response cannot
            // downgrade their facts or authorize replay of the remaining members.
            for permit in pending.values() {
                if self
                    .journal
                    .complete(permit, &McpCompletion::OutcomeUnknown { reason }, now())
                    .await
                    .is_err()
                {
                    error = McpCallError::Journal;
                }
            }
            return Err(error);
        }
        Ok(McpBatchResult {
            event_delivery_lost: delivery_lost,
            http_status,
        })
    }
}
