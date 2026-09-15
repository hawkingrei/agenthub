use agenthub_agent_domain::mcp_operations::McpFailureKind;

use super::*;
use crate::policy::{
    PreparedTaskCancellation, PreparedTaskLookup, PreparedTaskRequest, PreparedTaskUpdate,
};
use agenthub_db::mcp_operations::{
    McpTaskCancellationPermit, McpTaskLookupPermit, McpTaskUpdatePermit,
};

use crate::task::TaskObservation;

enum TaskPermit {
    Lookup(McpTaskLookupPermit),
    Cancellation(McpTaskCancellationPermit),
    Update(McpTaskUpdatePermit),
}

impl TaskPermit {
    fn operation_id(&self) -> &str {
        match self {
            Self::Lookup(p) => p.operation_id(),
            Self::Cancellation(p) => p.operation_id(),
            Self::Update(p) => p.operation_id(),
        }
    }
    fn attempt_number(&self) -> u32 {
        match self {
            Self::Lookup(p) => p.attempt_number(),
            Self::Cancellation(p) => p.attempt_number(),
            Self::Update(p) => p.attempt_number(),
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
            crate::task::lookup_observation,
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
            |input, response| {
                crate::task::cancellation_outcome(input, response)
                    .map(TaskObservation::from_outcome)
            },
        )
        .await
    }

    pub async fn run_task_update(
        &self,
        call: PreparedTaskUpdate,
        events: mpsc::Sender<Budgeted<HttpEvent>>,
    ) -> Result<McpCallResult, McpCallError> {
        let permit = self
            .journal
            .begin_task_update(&call.executor, &call.authority, &call.input, now())
            .await
            .map_err(journal_error)?;
        self.run_task_request(
            call,
            TaskPermit::Update(permit),
            events,
            crate::task::update_observation,
        )
        .await
    }

    async fn complete_task_request(
        &self,
        permit: &TaskPermit,
        completion: &McpCompletion,
        observation: &TaskObservation,
    ) -> Result<(), McpCallError> {
        match permit {
            TaskPermit::Lookup(permit) => {
                self.journal
                    .complete_task_lookup_with_inputs(
                        permit,
                        completion,
                        observation.outcome.as_ref(),
                        observation.inputs.as_deref(),
                        now(),
                    )
                    .await
            }
            TaskPermit::Cancellation(permit) => {
                self.journal
                    .complete_task_cancellation(
                        permit,
                        completion,
                        observation.outcome.as_ref(),
                        now(),
                    )
                    .await
            }
            TaskPermit::Update(permit) => {
                self.journal
                    .complete_task_update(permit, completion, now())
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
        observe: fn(&I, &Value) -> Result<TaskObservation, McpTransportError>,
    ) -> Result<McpCallResult, McpCallError> {
        let observed = async {
            let (response, http_status, delivery_lost, drain) = self
                .receive(call.transport, call.request, &call.response_id, &events)
                .await?;
            let member = matching_response(&response, &call.response_id, http_status >= 400)?
                .ok_or(McpTransportError::InvalidResponse)?;
            let observation = observe(&call.input, member)?;
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
            Ok::<_, McpTransportError>((
                response,
                completion,
                observation,
                http_status,
                delivery_lost,
                drain,
            ))
        }
        .await;
        match observed {
            Ok((
                response,
                completion,
                observation,
                http_status,
                event_delivery_lost,
                mut drain,
            )) => {
                self.complete_task_request(&permit, &completion, &observation)
                    .await?;
                drain.finish(self, &events).await;
                Ok(McpCallResult {
                    operation_id: permit.operation_id().to_owned(),
                    attempt_number: permit.attempt_number(),
                    completion,
                    response,
                    event_delivery_lost: event_delivery_lost || drain.lost,
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
                    &TaskObservation::default(),
                )
                .await?;
                Err(error.into())
            }
        }
    }
}
