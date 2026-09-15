use agenthub_agent_domain::mcp_operations::McpFailureKind;

use super::*;
use crate::policy::PreparedTaskLookup;

impl JournaledMcpClient {
    pub async fn run_task_lookup(
        &self,
        call: PreparedTaskLookup,
        events: mpsc::Sender<Budgeted<HttpEvent>>,
    ) -> Result<McpCallResult, McpCallError> {
        let permit = self
            .journal
            .begin_task_lookup(&call.executor, &call.authority, &call.input, now())
            .await
            .map_err(journal_error)?;
        let observed = async {
            let (response, http_status, delivery_lost) = self
                .receive(call.transport, call.request, &call.response_id, &events)
                .await?;
            let member = matching_response(&response, &call.response_id, http_status >= 400)?
                .ok_or(McpTransportError::InvalidResponse)?;
            let outcome = crate::task::lookup_outcome(&call.input, member)?;
            let mut receipt = member.clone();
            receipt.as_object_mut().unwrap().remove("id");
            let response_digest = digest("mcp-task-query-response-v1", &receipt)?;
            let completion = if member.get("error").is_some() {
                McpCompletion::Failed {
                    reason: McpFailureKind::JsonRpc,
                    response_digest,
                }
            } else {
                McpCompletion::Succeeded { response_digest }
            };
            Ok::<_, McpTransportError>((response, completion, outcome, http_status, delivery_lost))
        }
        .await;
        match observed {
            Ok((response, completion, outcome, http_status, event_delivery_lost)) => {
                self.journal
                    .complete_task_lookup(&permit, &completion, outcome.as_ref(), now())
                    .await
                    .map_err(journal_error)?;
                Ok(McpCallResult {
                    operation_id: permit.operation_id().to_owned(),
                    attempt_number: permit.attempt_number(),
                    completion,
                    response,
                    event_delivery_lost,
                    http_status,
                })
            }
            Err(error) => {
                let reason = match error {
                    McpTransportError::Deadline => McpAmbiguityReason::Deadline,
                    McpTransportError::Disconnected => McpAmbiguityReason::TransportLost,
                    _ => McpAmbiguityReason::InvalidResponse,
                };
                self.journal
                    .complete_task_lookup(
                        &permit,
                        &McpCompletion::OutcomeUnknown { reason },
                        None,
                        now(),
                    )
                    .await
                    .map_err(journal_error)?;
                Err(error.into())
            }
        }
    }
}
