//! Actual HTTP sends through durable operation permits. The caller must keep run owned by the
//! daemon task group, independently of the provider RPC and its event receiver.

#[cfg(test)]
mod tests;

mod batch;
pub use batch::McpBatchResult;

use std::time::{SystemTime, UNIX_EPOCH};

use agenthub_agent_domain::mcp_operations::{
    McpAmbiguityReason, McpCompletion, McpDeferralKind, classify_response_error,
};
use agenthub_db::mcp_operations::{McpJournalError, McpOperationStore};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::mpsc;

use crate::{
    McpTransportError,
    budget::{Budgeted, ByteBudget, json_bytes},
    digest::digest,
    http::HttpEvent,
    policy::PreparedToolCall,
};

/// Fixed error categories are safe across the provider boundary; database error sources are not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum McpCallError {
    #[error("MCP call identity conflicts with a recorded operation")]
    IdentityConflict,
    #[error("MCP call is already in flight")]
    InFlight,
    #[error("MCP call already has a recorded result; it was not sent again")]
    AlreadyCompleted,
    #[error("MCP call may have taken effect; replay is not authorized")]
    UnsafeReplay,
    #[error("MCP call requires a linked continuation or task result")]
    ContinuationRequired,
    #[error("MCP send authority is no longer current")]
    Authority,
    #[error("MCP journal is unavailable")]
    Journal,
    #[error(transparent)]
    Transport(#[from] McpTransportError),
}

/// Raw result is transient and intentionally has no Debug/Serialize implementation.
pub struct McpCallResult {
    pub operation_id: String,
    pub attempt_number: u32,
    pub completion: McpCompletion,
    pub response: Value,
    /// A lost callback/notification requires closing the provider stream. It never cancels a
    /// send or suppresses persistence of a factual result received later by the daemon.
    pub event_delivery_lost: bool,
    pub http_status: u16,
}

#[derive(Clone)]
pub struct JournaledMcpClient {
    journal: McpOperationStore,
    delivery: ByteBudget,
}

impl JournaledMcpClient {
    pub fn new(journal: McpOperationStore, delivery: ByteBudget) -> Self {
        Self { journal, delivery }
    }

    pub async fn run(
        &self,
        call: PreparedToolCall,
        events: mpsc::Sender<Budgeted<HttpEvent>>,
    ) -> Result<McpCallResult, McpCallError> {
        let permit = if let Some(input) = &call.continuation {
            self.journal
                .begin_continuation(&call.executor, &call.intent, input, now())
                .await
                .map_err(journal_error)?
        } else {
            let operation = self
                .journal
                .prepare(&call.executor, &call.intent, now())
                .await
                .map_err(journal_error)?;
            self.journal
                .begin_send(
                    &call.executor,
                    &operation.id,
                    operation.attempt_count,
                    now(),
                )
                .await
                .map_err(journal_error)?
        };
        let mut delivery_lost = false;
        let observed = async {
            let mut exchange = call.transport.send_resumable(call.request).await?;
            while let Some(event) = exchange.next_event().await? {
                let unidentified_http_error = exchange.status_code() >= 400;
                if let Some(message) = event.message.as_ref()
                    && let Some(response) =
                        matching_response(message, &call.response_id, unidentified_http_error)?
                {
                    let completion = classify_completion(response)?;
                    return Ok((message.clone(), completion, exchange.status_code()));
                }
                // Queue pressure loses delivery only. The reserved HTTP workspace keeps draining
                // until a factual result can be committed, even after the caller has gone away.
                if !self.deliver(event, &events) {
                    delivery_lost = true;
                }
            }
            Err(McpTransportError::Disconnected)
        }
        .await;
        match observed {
            Ok((response, completion, http_status)) => {
                // Do not expose a terminal result until its receipt is durable. Losing the
                // provider or replacing executor authority cannot discard an observed result.
                self.journal
                    .complete(&permit, &completion, now())
                    .await
                    .map_err(journal_error)?;
                Ok(McpCallResult {
                    operation_id: permit.operation_id().to_owned(),
                    attempt_number: permit.attempt_number(),
                    completion,
                    response,
                    event_delivery_lost: delivery_lost,
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
                    .complete(&permit, &McpCompletion::OutcomeUnknown { reason }, now())
                    .await
                    .map_err(journal_error)?;
                Err(error.into())
            }
        }
    }

    fn deliver(&self, event: HttpEvent, events: &mpsc::Sender<Budgeted<HttpEvent>>) -> bool {
        // Resumption owns cursor/retry-only frames. Dropping one from the provider queue is
        // not lost MCP delivery and must not close an otherwise healthy provider session.
        if event.message.is_none() {
            return true;
        }
        let bytes = event
            .message
            .as_ref()
            .map(json_bytes)
            .transpose()
            .map(|bytes| bytes.unwrap_or(0) + event.cursor.as_ref().map_or(0, String::len) + 16);
        bytes
            .and_then(|bytes| self.delivery.retain(event, bytes))
            .is_ok_and(|event| events.try_send(event).is_ok())
    }
}

fn matching_response<'a>(
    message: &'a Value,
    id: &Value,
    unidentified_http_error: bool,
) -> Result<Option<&'a Value>, McpTransportError> {
    if let Some(batch) = message.as_array() {
        let mut found = None;
        for message in batch {
            if let Some(response) = matching_response(message, id, unidentified_http_error)? {
                if found.is_some() {
                    return Err(McpTransportError::InvalidResponse);
                }
                found = Some(response);
            }
        }
        Ok(found)
    } else {
        // An HTTP error may precede JSON-RPC ID parsing. This exchange contains exactly one
        // outgoing call, so retain its unidentified error as a failure receipt without rewriting it.
        let matches_id = message.get("id") == Some(id)
            || (unidentified_http_error
                && message.get("id").is_none_or(Value::is_null)
                && message.get("error").is_some());
        Ok((matches_id && message.get("method").is_none()).then_some(message))
    }
}

fn classify_completion(response: &Value) -> Result<McpCompletion, McpTransportError> {
    // IDs change on a legitimate retry, so receipt equivalence covers the actual result/error.
    let mut outcome = response.clone();
    outcome
        .as_object_mut()
        .ok_or(McpTransportError::InvalidResponse)?
        .remove("id");
    let response_digest = digest("mcp-response-v1", &outcome)?;
    if let Some(reason) = classify_response_error(response) {
        return Ok(McpCompletion::Failed {
            reason,
            response_digest,
        });
    }
    let result = response
        .get("result")
        .filter(|result| result.is_object())
        .ok_or(McpTransportError::InvalidResponse)?;
    let deferred = if result["resultType"] == "input_required" {
        Some(McpDeferralKind::InputRequired)
    } else if result["resultType"] == "task" || result.get("task").is_some() {
        Some(McpDeferralKind::TaskAccepted)
    } else {
        None
    };
    Ok(if let Some(reason) = deferred {
        McpCompletion::Deferred {
            reason,
            response_digest,
            input_receipt: (reason == McpDeferralKind::InputRequired)
                .then(|| crate::continuation::receipt(response).ok())
                .flatten(),
        }
    } else {
        McpCompletion::Succeeded { response_digest }
    })
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}

fn journal_error(error: anyhow::Error) -> McpCallError {
    if error
        .downcast_ref::<agenthub_db::loop_runtime::LoopStoreError>()
        .is_some()
    {
        return McpCallError::Authority;
    }
    match error.downcast_ref::<McpJournalError>() {
        Some(McpJournalError::IdentityConflict) => McpCallError::IdentityConflict,
        Some(McpJournalError::InFlight) => McpCallError::InFlight,
        Some(McpJournalError::AlreadyCompleted) => McpCallError::AlreadyCompleted,
        Some(McpJournalError::UnsafeReplay) => McpCallError::UnsafeReplay,
        Some(McpJournalError::ContinuationRequired) => McpCallError::ContinuationRequired,
        Some(
            McpJournalError::StaleAttempt
            | McpJournalError::StaleDaemon
            | McpJournalError::ScopeMismatch,
        ) => McpCallError::Authority,
        None => McpCallError::Journal,
    }
}
