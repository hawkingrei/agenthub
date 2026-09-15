use agenthub_agent_domain::mcp_operations::{
    McpTaskAuthority, McpTaskLookupInput, McpTaskLookupMethod, McpTaskReceipt, McpTaskVersion,
};

use super::*;
use crate::task::{TaskContext, modern_capability, task_digest};

pub struct PreparedTaskLookup {
    pub(crate) transport: McpHttpTransport,
    pub(crate) request: PreparedHttpRequest,
    pub(crate) executor: LoopReservation,
    pub(crate) authority: McpTaskAuthority,
    pub(crate) input: McpTaskLookupInput,
    pub(crate) response_id: Value,
}

impl McpBinding {
    pub fn prepare_task_lookup(
        &self,
        catalog: &McpToolCatalog,
        context: &McpCallContext<'_>,
        message: Value,
    ) -> Result<PreparedTaskLookup, McpPolicyError> {
        if catalog.version != context.http.version
            || validate_versioned_message(&message, context.http.version)? != MessageKind::Request
        {
            return Err(McpPolicyError::Call);
        }
        let task = TaskContext::new(context.http)?;
        let method = match message["method"].as_str() {
            Some("tasks/get") => McpTaskLookupMethod::Get,
            Some("tasks/result") if task.version == McpTaskVersion::November2025 => {
                McpTaskLookupMethod::Result
            }
            _ => return Err(McpPolicyError::Call),
        };
        let params = message["params"].as_object().ok_or(McpPolicyError::Call)?;
        if task.version == McpTaskVersion::July2026 && !modern_capability(params) {
            return Err(McpPolicyError::Call);
        }
        if params.contains_key("inputResponses") || params.contains_key("requestState") {
            return Err(McpPolicyError::Call);
        }
        let activation = context
            .executor
            .activation_id
            .as_deref()
            .ok_or(McpPolicyError::Scope)?;
        let input = McpTaskLookupInput {
            receipt: McpTaskReceipt {
                task_digest: task_digest(params.get("taskId").ok_or(McpPolicyError::Call)?)?,
                version: task.version,
                session_digest: task.session_digest,
            },
            method,
            request_key: digest(
                "mcp-rpc-request-v1",
                &json!([activation, context.proxy_session_id, message["id"]]),
            )?,
            request_digest: digest("mcp-task-query-v1", &json!([message["method"], params]))?,
        };
        Ok(PreparedTaskLookup {
            request: self.transport.prepare_post(context.http, &message, None)?,
            transport: self.transport.clone(),
            executor: context.executor.clone(),
            input,
            response_id: message["id"].clone(),
            authority: McpTaskAuthority {
                server_id: self.server_id.clone(),
                scope_digest: self.scope_digest.clone(),
                binding_digest: self.binding_digest.clone(),
                tools: catalog
                    .tools
                    .iter()
                    .map(|(name, tool)| (name.clone(), tool.schema_digest.clone()))
                    .collect(),
            },
        })
    }
}
