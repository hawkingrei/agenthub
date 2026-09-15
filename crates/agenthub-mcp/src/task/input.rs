use agenthub_agent_domain::mcp_operations::{
    McpTaskInputRequest, McpTaskInputResponse, McpTaskUpdateInput,
};

use super::*;

fn input_id(key: &str) -> Result<McpDigest, McpTransportError> {
    if key.len() > 4096 {
        return Err(McpTransportError::Capacity);
    }
    digest("mcp-input-id-v1", &json!(key))
}

pub(crate) fn input_responses(
    value: &Value,
) -> Result<Vec<McpTaskInputResponse>, McpTransportError> {
    let responses = value.as_object().ok_or(McpTransportError::InvalidMessage)?;
    if responses.len() > 64 {
        return Err(McpTransportError::Capacity);
    }
    responses
        .iter()
        .map(|(key, response)| {
            if !response.is_object() {
                return Err(McpTransportError::InvalidMessage);
            }
            Ok(McpTaskInputResponse {
                input_id_digest: input_id(key)?,
                response_digest: digest("mcp-task-input-response-v1", response)?,
            })
        })
        .collect()
}

pub(crate) fn lookup_observation(
    input: &McpTaskLookupInput,
    response: &Value,
) -> Result<TaskObservation, McpTransportError> {
    let outcome = lookup_outcome(input, response)?;
    let inputs = if input.method == McpTaskLookupMethod::Get {
        task_inputs(&input.receipt, &response["result"])?
    } else {
        None
    };
    Ok(TaskObservation { outcome, inputs })
}

pub(crate) fn notification_observation(
    receipt: &McpTaskReceipt,
    message: &Value,
) -> Result<TaskObservation, McpTransportError> {
    let method = match receipt.version {
        McpTaskVersion::July2026 => "notifications/tasks",
        McpTaskVersion::November2025 => "notifications/tasks/status",
    };
    if crate::protocol::message_kind(message)? != crate::protocol::MessageKind::Notification
        || message["method"] != method
    {
        return Err(McpTransportError::InvalidResponse);
    }
    let task = &message["params"];
    Ok(TaskObservation {
        outcome: status_outcome(receipt, task)?,
        inputs: task_inputs(receipt, task)?,
    })
}

fn task_inputs(
    receipt: &McpTaskReceipt,
    task: &Value,
) -> Result<Option<Vec<McpTaskInputRequest>>, McpTransportError> {
    let inputs =
        if receipt.version == McpTaskVersion::July2026 && task["status"] == "input_required" {
            let requests = task["inputRequests"]
                .as_object()
                .ok_or(McpTransportError::InvalidResponse)?;
            if requests.len() > 64 {
                return Err(McpTransportError::Capacity);
            }
            Some(
                requests
                    .iter()
                    .map(|(key, request)| {
                        if !crate::continuation::valid_input_request(request) {
                            return Err(McpTransportError::InvalidResponse);
                        }
                        Ok(McpTaskInputRequest {
                            input_id_digest: input_id(key)?,
                            request_digest: digest("mcp-task-input-request-v1", request)?,
                        })
                    })
                    .collect::<Result<Vec<_>, McpTransportError>>()?,
            )
        } else {
            None
        };
    Ok(inputs)
}

pub(crate) fn update_observation(
    _: &McpTaskUpdateInput,
    response: &Value,
) -> Result<TaskObservation, McpTransportError> {
    if response.get("error").is_none() && response["result"]["resultType"] != "complete" {
        return Err(McpTransportError::InvalidResponse);
    }
    // This acknowledges supplied inputs; the task result still requires a later observation.
    Ok(TaskObservation::default())
}
