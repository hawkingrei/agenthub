use agenthub_agent_domain::mcp_operations::McpFailureKind;

use super::*;
use crate::policy::{PreparedTaskCancellation, PreparedTaskLookup, PreparedTaskRequest};
use agenthub_db::mcp_operations::{McpTaskCancellationPermit, McpTaskLookupPermit};

enum TaskPermit {
    Lookup(McpTaskLookupPermit),
    Cancellation(McpTaskCancellationPermit),
}

impl TaskPermit {
    fn operation_id(&self) -> &str {
        match self {
            Self::Lookup(p) => p.operation_id(),
            Self::Cancellation(p) => p.operation_id(),
        }
    }
    fn attempt_number(&self) -> u32 {
        match self {
            Self::Lookup(p) => p.attempt_number(),
            Self::Cancellation(p) => p.attempt_number(),
        }
    }
}

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
        self.run_task_request(
            call,
            TaskPermit::Lookup(permit),
            events,
            crate::task::lookup_outcome,
        )
        .await
    }

    pub async fn run_task_cancellation(
        &self,
        call: PreparedTaskCancellation,
        events: mpsc::Sender<Budgeted<HttpEvent>>,
    ) -> Result<McpCallResult, McpCallError> {
        let permit = self
            .journal
            .begin_task_cancellation(&call.executor, &call.authority, &call.input, now())
            .await
            .map_err(journal_error)?;
        self.run_task_request(
            call,
            TaskPermit::Cancellation(permit),
            events,
            crate::task::cancellation_outcome,
        )
        .await
    }

    async fn complete_task_request(
        &self,
        permit: &TaskPermit,
        completion: &McpCompletion,
        outcome: Option<&McpCompletion>,
    ) -> Result<(), McpCallError> {
        match permit {
            TaskPermit::Lookup(permit) => {
                self.journal
                    .complete_task_lookup(permit, completion, outcome, now())
                    .await
            }
            TaskPermit::Cancellation(permit) => {
                self.journal
                    .complete_task_cancellation(permit, completion, outcome, now())
                    .await
            }
        }
        .map_err(journal_error)
    }

    async fn run_task_request<I>(
        &self,
        call: PreparedTaskRequest<I>,
        permit: TaskPermit,
        events: mpsc::Sender<Budgeted<HttpEvent>>,
        outcome: fn(&I, &Value) -> Result<Option<McpCompletion>, McpTransportError>,
    ) -> Result<McpCallResult, McpCallError> {
        let observed = async {
            let (response, http_status, delivery_lost) = self
                .receive(call.transport, call.request, &call.response_id, &events)
                .await?;
            let member = matching_response(&response, &call.response_id, http_status >= 400)?
                .ok_or(McpTransportError::InvalidResponse)?;
            let outcome = outcome(&call.input, member)?;
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
                self.complete_task_request(&permit, &completion, outcome.as_ref())
                    .await?;
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
                self.complete_task_request(
                    &permit,
                    &McpCompletion::OutcomeUnknown { reason },
                    None,
                )
                .await?;
                Err(error.into())
            }
        }
    }
}
