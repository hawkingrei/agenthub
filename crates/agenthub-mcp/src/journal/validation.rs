use agenthub_agent_domain::mcp_operations::{
    McpTaskLookupInput, McpTaskLookupMethod, McpTaskVersion,
};

use super::*;

/// Validates a completed native tool result before accepting its durable outcome. Errors must be
/// fixed categories; implementations must not log or return payload contents.
pub type ToolResultValidator = dyn Fn(&str, &Value) -> Result<(), McpTransportError> + Send + Sync;

#[derive(Clone, Copy)]
pub(super) enum ToolResultLocation {
    Rpc,
    CompletedTask,
}

impl ToolResultLocation {
    pub(super) fn lookup(input: &McpTaskLookupInput) -> Option<Self> {
        match (input.receipt.version, input.method) {
            (_, McpTaskLookupMethod::Result) => Some(Self::Rpc),
            (McpTaskVersion::July2026, McpTaskLookupMethod::Get) => Some(Self::CompletedTask),
            _ => None,
        }
    }
}

impl JournaledMcpClient {
    pub(crate) fn validating(
        mut self,
        validator: Option<std::sync::Arc<ToolResultValidator>>,
    ) -> Self {
        self.result_validator = validator;
        self
    }

    pub(super) fn validate_result(
        &self,
        tool_name: &str,
        response: &Value,
        location: ToolResultLocation,
    ) -> Result<(), McpTransportError> {
        if response.get("error").is_some() {
            return Ok(());
        }
        let result = match location {
            ToolResultLocation::Rpc => &response["result"],
            ToolResultLocation::CompletedTask => {
                let task = &response["result"];
                if task["status"] != "completed" {
                    return Ok(());
                }
                &task["result"]
            }
        };
        self.validate_native_result(tool_name, result)
    }

    pub(super) fn validate_native_result(
        &self,
        tool_name: &str,
        result: &Value,
    ) -> Result<(), McpTransportError> {
        let Some(validate) = &self.result_validator else {
            return Ok(());
        };
        // A task handle or an input-required receipt is not the tool's eventual output.
        if result["resultType"] == "input_required"
            || result["resultType"] == "task"
            || result.get("task").is_some()
        {
            return Ok(());
        }
        validate(tool_name, result)
    }
}
