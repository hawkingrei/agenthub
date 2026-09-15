use agenthub_agent_domain::mcp_operations::{McpContinuationInput, McpDigest, McpInputReceipt};
use serde_json::{Map, Value, json};

use crate::{McpTransportError, digest::digest, policy::McpPolicyError};

fn state_digest(value: Option<&Value>) -> Result<Option<McpDigest>, McpTransportError> {
    value
        .map(|value| {
            if !value.is_string() {
                return Err(McpTransportError::InvalidMessage);
            }
            digest("mcp-request-state-v1", value)
        })
        .transpose()
}

fn input_ids(value: Option<&Value>) -> Result<Vec<McpDigest>, McpTransportError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let object = value.as_object().ok_or(McpTransportError::InvalidMessage)?;
    if object.len() > 64 {
        return Err(McpTransportError::Capacity);
    }
    object
        .keys()
        .map(|key| digest("mcp-input-id-v1", &json!(key)))
        .collect()
}

pub(crate) fn input(
    params: &Map<String, Value>,
    id: &Value,
    request_digest: McpDigest,
) -> Result<McpContinuationInput, McpPolicyError> {
    let state_digest = state_digest(params.get("requestState"))?;
    let input_ids = input_ids(params.get("inputResponses"))?;
    if let Some(responses) = params.get("inputResponses").and_then(Value::as_object)
        && responses.values().any(|response| !response.is_object())
    {
        return Err(McpPolicyError::Continuation);
    }
    Ok(McpContinuationInput {
        state_digest,
        input_ids,
        request_id_digest: digest("mcp-json-rpc-id-v1", id)?,
        request_digest,
    })
}

pub(crate) fn receipt(response: &Value) -> Result<McpInputReceipt, McpTransportError> {
    let result = response["result"]
        .as_object()
        .ok_or(McpTransportError::InvalidResponse)?;
    if !result.contains_key("requestState") && !result.contains_key("inputRequests") {
        return Err(McpTransportError::InvalidResponse);
    }
    let state_digest = state_digest(result.get("requestState"))?;
    let input_ids = input_ids(result.get("inputRequests"))?;
    if let Some(requests) = result.get("inputRequests").and_then(Value::as_object)
        && requests
            .values()
            .any(|request| !valid_input_request(request))
    {
        return Err(McpTransportError::InvalidResponse);
    }
    Ok(McpInputReceipt {
        state_digest,
        input_ids,
        request_id_digest: digest("mcp-json-rpc-id-v1", &response["id"])?,
    })
}

pub(crate) fn valid_input_request(request: &Value) -> bool {
    matches!(
        request["method"].as_str(),
        Some("elicitation/create" | "sampling/createMessage" | "roots/list")
    ) && request.get("params").is_none_or(Value::is_object)
}
