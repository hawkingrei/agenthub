use agenthub_agent_domain::mcp_operations::{
    McpCompletion, McpDigest, McpFailureKind, McpTaskLookupInput, McpTaskLookupMethod,
    McpTaskReceipt, McpTaskVersion,
};
use serde_json::{Map, Value, json};

use crate::{
    McpTransportError, digest::digest, http::HttpContext, policy::McpPolicyError,
    protocol::ProtocolVersion,
};

pub(crate) struct TaskContext {
    pub version: McpTaskVersion,
    pub session_digest: Option<McpDigest>,
}

impl TaskContext {
    pub fn new(http: &HttpContext) -> Result<Self, McpTransportError> {
        let version = match http.version {
            ProtocolVersion::November2025 => McpTaskVersion::November2025,
            ProtocolVersion::July2026 => McpTaskVersion::July2026,
            _ => return Err(McpTransportError::UnsupportedVersion),
        };
        Ok(Self {
            version,
            session_digest: http
                .session_id
                .as_ref()
                .map(|session| digest("mcp-task-session-v1", &json!(session.as_bytes())))
                .transpose()?,
        })
    }

    pub fn for_tool(
        http: &HttpContext,
        params: &Map<String, Value>,
        declaration: &Value,
    ) -> Result<Option<Self>, McpPolicyError> {
        if http.version == ProtocolVersion::July2026 {
            if params.contains_key("task") {
                return Err(McpPolicyError::Call);
            }
            return modern_capability(params)
                .then(|| Self::new(http).map_err(Into::into))
                .transpose();
        }
        let support = declaration
            .pointer("/execution/taskSupport")
            .and_then(Value::as_str);
        if params.contains_key("task") {
            if http.version != ProtocolVersion::November2025
                || !matches!(support, Some("optional" | "required"))
                || !params["task"].is_object()
            {
                return Err(McpPolicyError::Call);
            }
            Ok(Some(Self::new(http)?))
        } else if support == Some("required") && http.version == ProtocolVersion::November2025 {
            Err(McpPolicyError::Call)
        } else {
            Ok(None)
        }
    }

    pub fn receipt(&self, response: &Value) -> Result<McpTaskReceipt, McpTransportError> {
        let result = &response["result"];
        let task = match self.version {
            McpTaskVersion::July2026 if result["resultType"] == "task" => result,
            McpTaskVersion::November2025 => &result["task"],
            _ => return Err(McpTransportError::InvalidResponse),
        };
        validate_task(task, self.version)?;
        Ok(McpTaskReceipt {
            task_digest: task_digest(&task["taskId"])?,
            version: self.version,
            session_digest: self.session_digest.clone(),
        })
    }
}

pub(crate) fn modern_capability(params: &Map<String, Value>) -> bool {
    params
        .get("_meta")
        .and_then(|value| value.get("io.modelcontextprotocol/clientCapabilities"))
        .and_then(|value| value.get("extensions"))
        .and_then(|value| value.get("io.modelcontextprotocol/tasks"))
        .is_some_and(Value::is_object)
}

pub(crate) fn task_digest(value: &Value) -> Result<McpDigest, McpTransportError> {
    if value
        .as_str()
        .is_none_or(|id| id.is_empty() || id.len() > 4096)
    {
        return Err(McpTransportError::InvalidMessage);
    }
    digest("mcp-task-handle-v1", value)
}

fn validate_task(task: &Value, version: McpTaskVersion) -> Result<(), McpTransportError> {
    task_digest(&task["taskId"])?;
    let (ttl, poll) = match version {
        McpTaskVersion::July2026 => ("ttlMs", "pollIntervalMs"),
        McpTaskVersion::November2025 => ("ttl", "pollInterval"),
    };
    if !matches!(
        task["status"].as_str(),
        Some("working" | "input_required" | "completed" | "failed" | "cancelled")
    ) || ["createdAt", "lastUpdatedAt"].iter().any(|key| {
        task[*key]
            .as_str()
            .is_none_or(|value| value.is_empty() || value.len() > 128)
    }) || task
        .get(ttl)
        .is_none_or(|value| !value.is_null() && value.as_i64().is_none_or(|value| value < 0))
        || task
            .get(poll)
            .is_some_and(|value| value.as_i64().is_none_or(|value| value < 0))
    {
        return Err(McpTransportError::InvalidResponse);
    }
    Ok(())
}

/// A query RPC error does not prove a tool failure. Only task-specific terminal facts settle it.
pub(crate) fn lookup_outcome(
    input: &McpTaskLookupInput,
    response: &Value,
) -> Result<Option<McpCompletion>, McpTransportError> {
    if response.get("error").is_some() {
        return Ok(None);
    }
    let result = response
        .get("result")
        .filter(|value| value.is_object())
        .ok_or(McpTransportError::InvalidResponse)?;
    if input.method == McpTaskLookupMethod::Result {
        if let Some(id) = result
            .get("_meta")
            .and_then(|meta| meta.get("io.modelcontextprotocol/related-task"))
            .and_then(|task| task.get("taskId"))
            && task_digest(id)? != input.receipt.task_digest
        {
            return Err(McpTransportError::InvalidResponse);
        }
        let completion = crate::journal::classify_completion(response, None)?;
        return Ok(matches!(
            completion,
            McpCompletion::Succeeded { .. } | McpCompletion::Failed { .. }
        )
        .then_some(completion));
    }
    validate_task(result, input.receipt.version)?;
    if task_digest(&result["taskId"])? != input.receipt.task_digest
        || input.receipt.version == McpTaskVersion::July2026 && result["resultType"] != "complete"
    {
        return Err(McpTransportError::InvalidResponse);
    }
    match result["status"].as_str().unwrap() {
        "completed" if input.receipt.version == McpTaskVersion::July2026 => {
            let completion = crate::journal::classify_completion(
                &json!({"jsonrpc":"2.0","result":result["result"]}),
                None,
            )?;
            if !matches!(
                completion,
                McpCompletion::Succeeded { .. } | McpCompletion::Failed { .. }
            ) {
                return Err(McpTransportError::InvalidResponse);
            }
            Ok(Some(completion))
        }
        "failed" => {
            if input.receipt.version == McpTaskVersion::July2026
                && (result["error"]["code"].as_i64().is_none()
                    || result["error"]["message"].as_str().is_none())
            {
                return Err(McpTransportError::InvalidResponse);
            }
            Ok(Some(McpCompletion::Failed {
                reason: McpFailureKind::JsonRpc,
                response_digest: digest("mcp-task-failure-v1", result)?,
            }))
        }
        "cancelled" => Ok(Some(McpCompletion::Failed {
            reason: McpFailureKind::TaskCancelled,
            response_digest: digest("mcp-task-cancelled-v1", result)?,
        })),
        _ => Ok(None),
    }
}
