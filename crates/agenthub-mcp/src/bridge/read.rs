//! Session-local MRTR receipts for reads. Tool effects use the durable operation journal.

use std::{collections::HashMap, sync::Mutex};

use agenthub_agent_domain::mcp_operations::{McpDigest, McpInputReceipt};

use super::*;
use crate::{continuation, digest::digest, protocol::ProtocolVersion};

const MAX_READS: usize = 64;
const MAX_ROUNDS: u8 = 10;
// At most 64 input ID digests plus intent, state and correlation digests per receipt.
const RECEIPT_BYTES: usize = 8 * 1024;

#[derive(Default)]
pub(super) struct ReadContinuations(Mutex<HashMap<[u8; 32], Entry>>);

struct Entry {
    intent: McpDigest,
    receipt: Option<McpInputReceipt>,
    rounds: u8,
    reserved: bool,
    _bytes: ByteLease,
}

pub(super) struct ReadRound {
    store: Arc<ReadContinuations>,
    key: [u8; 32],
    continuing: bool,
    sent: bool,
    finished: bool,
}

impl ReadContinuations {
    pub(super) fn prepare(
        self: &Arc<Self>,
        message: &Value,
        budget: &ByteBudget,
    ) -> Result<ReadRound, McpPolicyError> {
        if message_kind(message)? != MessageKind::Request {
            return Err(McpPolicyError::Call);
        }
        let params = message["params"].as_object().ok_or(McpPolicyError::Call)?;
        let continuing =
            params.contains_key("requestState") || params.contains_key("inputResponses");
        let mut semantic = params.clone();
        semantic.remove("requestState");
        semantic.remove("inputResponses");
        if let Some(meta) = semantic.get_mut("_meta").and_then(Value::as_object_mut) {
            meta.remove("progressToken");
            if meta.is_empty() {
                semantic.remove("_meta");
            }
        }
        let intent = digest("mcp-read-intent-v1", &json!([message["method"], semantic]))?;
        let mut entries = self.0.lock().unwrap();
        let key = if continuing {
            let input = continuation::input(params, &message["id"], intent.clone())?;
            // Include reserved receipts when checking ambiguity. A concurrent reservation must
            // not make another identical receipt appear uniquely attributable to this request.
            let mut matches = entries.iter_mut().filter(|(_, entry)| {
                entry.intent == intent
                    && entry.receipt.as_ref().is_some_and(|receipt| {
                        receipt.state_digest == input.state_digest
                            && (receipt.state_digest.is_some()
                                || receipt.input_ids.is_empty() && input.input_ids.is_empty()
                                || input
                                    .input_ids
                                    .iter()
                                    .any(|id| receipt.input_ids.contains(id)))
                    })
            });
            let (key, entry) = matches.next().ok_or(McpPolicyError::Continuation)?;
            if matches.next().is_some() || entry.reserved || entry.rounds >= MAX_ROUNDS {
                return Err(McpPolicyError::Continuation);
            }
            entry.reserved = true;
            *key
        } else {
            let key = correlation_id(&message["id"]);
            if entries.len() >= MAX_READS || entries.contains_key(&key) {
                return Err(McpTransportError::Capacity.into());
            }
            entries.insert(
                key,
                Entry {
                    intent,
                    receipt: None,
                    rounds: 0,
                    reserved: true,
                    _bytes: budget.acquire(RECEIPT_BYTES)?,
                },
            );
            key
        };
        Ok(ReadRound {
            store: self.clone(),
            key,
            continuing,
            sent: false,
            finished: false,
        })
    }
}

impl ReadRound {
    pub(super) fn begin(&mut self) {
        let mut entries = self.store.0.lock().unwrap();
        let entry = entries
            .get_mut(&self.key)
            .expect("read reservation retained");
        entry.rounds += 1;
        self.sent = true;
    }

    pub(super) fn finish(&mut self, response: &Value) -> Result<(), McpTransportError> {
        let receipt = (response["result"]["resultType"] == "input_required")
            .then(|| continuation::receipt(response))
            .transpose()?;
        let mut entries = self.store.0.lock().unwrap();
        if let Some(receipt) = receipt {
            let entry = entries
                .get_mut(&self.key)
                .expect("read reservation retained");
            entry.receipt = Some(receipt);
            entry.reserved = false;
        } else {
            entries.remove(&self.key);
        }
        self.finished = true;
        Ok(())
    }
}

impl Drop for ReadRound {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let mut entries = self.store.0.lock().unwrap();
        if self.sent || !self.continuing {
            // A failed send never silently restores a consumed receipt. A caller may start a
            // fresh read, but cannot retry this continuation automatically after lost delivery.
            entries.remove(&self.key);
        } else if let Some(entry) = entries.get_mut(&self.key) {
            entry.reserved = false;
        }
    }
}

pub(super) fn validate_request(
    message: &Value,
    version: ProtocolVersion,
) -> Result<(), McpPolicyError> {
    if message_kind(message)? != MessageKind::Request {
        return Ok(());
    }
    let params = &message["params"];
    let state = params.get("requestState").is_some();
    let inputs = params.get("inputResponses").is_some();
    if state || inputs {
        let allowed = matches!(
            message["method"].as_str(),
            Some("tools/call" | "prompts/get" | "resources/read")
        ) || message["method"] == "tasks/update" && !state;
        if version != ProtocolVersion::July2026 || !allowed {
            return Err(McpPolicyError::Continuation);
        }
    }
    Ok(())
}

pub(super) fn validate_control_response(
    request: &Value,
    response: &Value,
    version: ProtocolVersion,
) -> Result<(), McpTransportError> {
    let result = &response["result"];
    // The pinned task extension supports tools/call only. Controls cannot create task handles.
    if result["resultType"] == "task" || result.get("task").is_some_and(Value::is_object) {
        return Err(McpTransportError::InvalidResponse);
    }
    if result["resultType"] != "input_required" {
        return Ok(());
    }
    if version != ProtocolVersion::July2026
        || !matches!(
            request["method"].as_str(),
            Some("prompts/get" | "resources/read")
        )
    {
        return Err(McpTransportError::InvalidResponse);
    }
    continuation::receipt(response)?;
    if let Some(inputs) = result["inputRequests"].as_object() {
        let capabilities =
            &request["params"]["_meta"]["io.modelcontextprotocol/clientCapabilities"];
        for input in inputs.values() {
            let family = match input["method"].as_str() {
                Some("roots/list") => "roots",
                Some("sampling/createMessage") => "sampling",
                Some("elicitation/create") => "elicitation",
                _ => return Err(McpTransportError::InvalidResponse),
            };
            if !capabilities[family].is_object() {
                return Err(McpTransportError::InvalidResponse);
            }
        }
    }
    Ok(())
}
