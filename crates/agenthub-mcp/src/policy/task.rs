use agenthub_agent_domain::mcp_operations::{
    McpTaskAuthority, McpTaskCancellationInput, McpTaskLookupInput, McpTaskLookupMethod,
    McpTaskReceipt, McpTaskUpdateInput, McpTaskVersion,
};

use super::*;
use crate::task::{TaskContext, modern_capability, task_digest};

pub type PreparedTaskLookup = PreparedTaskRequest<McpTaskLookupInput>;
pub type PreparedTaskCancellation = PreparedTaskRequest<McpTaskCancellationInput>;
pub type PreparedTaskUpdate = PreparedTaskRequest<McpTaskUpdateInput>;

pub struct PreparedTaskRequest<I> {
    pub(crate) transport: McpHttpTransport,
    pub(crate) request: PreparedHttpRequest,
    pub(crate) executor: LoopReservation,
    pub(crate) authority: McpTaskAuthority,
    pub(crate) input: I,
    pub(crate) response_id: Value,
}

impl McpBinding {
    pub fn prepare_task_lookup(
        &self,
        catalog: &McpToolCatalog,
        context: &McpCallContext<'_>,
        message: Value,
    ) -> Result<PreparedTaskLookup, McpPolicyError> {
        let method = match message["method"].as_str() {
            Some("tasks/get") => McpTaskLookupMethod::Get,
            Some("tasks/result") if context.http.version == ProtocolVersion::November2025 => {
                McpTaskLookupMethod::Result
            }
            _ => return Err(McpPolicyError::Call),
        };
        self.prepare_task_request(
            catalog,
            context,
            message,
            "mcp-task-query-v1",
            |receipt, request_key, request_digest| McpTaskLookupInput {
                receipt,
                method,
                request_key,
                request_digest,
            },
        )
    }

    pub fn prepare_task_cancellation(
        &self,
        catalog: &McpToolCatalog,
        context: &McpCallContext<'_>,
        message: Value,
    ) -> Result<PreparedTaskCancellation, McpPolicyError> {
        if message["method"] != "tasks/cancel" {
            return Err(McpPolicyError::Call);
        }
        self.prepare_task_request(
            catalog,
            context,
            message,
            "mcp-task-cancel-v1",
            |receipt, request_key, request_digest| McpTaskCancellationInput {
                receipt,
                request_key,
                request_digest,
            },
        )
    }

    pub fn prepare_task_update(
        &self,
        catalog: &McpToolCatalog,
        context: &McpCallContext<'_>,
        message: Value,
    ) -> Result<PreparedTaskUpdate, McpPolicyError> {
        if message["method"] != "tasks/update" || context.http.version != ProtocolVersion::July2026
        {
            return Err(McpPolicyError::Call);
        }
        let inputs = crate::task::input_responses(&message["params"]["inputResponses"])?;
        self.prepare_task_request(
            catalog,
            context,
            message,
            "mcp-task-update-v1",
            |receipt, request_key, request_digest| McpTaskUpdateInput {
                receipt,
                request_key,
                request_digest,
                inputs,
            },
        )
    }

    fn prepare_task_request<I>(
        &self,
        catalog: &McpToolCatalog,
        context: &McpCallContext<'_>,
        message: Value,
        digest_domain: &str,
        input: impl FnOnce(McpTaskReceipt, McpDigest, McpDigest) -> I,
    ) -> Result<PreparedTaskRequest<I>, McpPolicyError> {
        if catalog.version != context.http.version
            || validate_versioned_message(&message, context.http.version)? != MessageKind::Request
        {
            return Err(McpPolicyError::Call);
        }
        let task = TaskContext::new(context.http)?;
        let params = message["params"].as_object().ok_or(McpPolicyError::Call)?;
        if task.version == McpTaskVersion::July2026 && !modern_capability(params) {
            return Err(McpPolicyError::Call);
        }
        if params.contains_key("inputResponses") && message["method"] != "tasks/update"
            || params.contains_key("requestState")
        {
            return Err(McpPolicyError::Call);
        }
        let activation = context
            .executor
            .activation_id
            .as_deref()
            .ok_or(McpPolicyError::Scope)?;
        let receipt = McpTaskReceipt {
            task_digest: task_digest(params.get("taskId").ok_or(McpPolicyError::Call)?)?,
            version: task.version,
            session_digest: task.session_digest,
        };
        let request_key = digest(
            "mcp-rpc-request-v1",
            &json!([activation, context.proxy_session_id, message["id"]]),
        )?;
        let request_digest = digest(digest_domain, &json!([message["method"], params]))?;
        Ok(PreparedTaskRequest {
            request: self.transport.prepare_post(context.http, &message, None)?,
            transport: self.transport.clone(),
            executor: context.executor.clone(),
            input: input(receipt, request_key, request_digest),
            response_id: message["id"].clone(),
            authority: self.task_authority(catalog),
        })
    }
    pub(crate) fn task_authority(&self, catalog: &McpToolCatalog) -> McpTaskAuthority {
        McpTaskAuthority {
            server_id: self.server_id.clone(),
            scope_digest: self.scope_digest.clone(),
            binding_digest: self.binding_digest.clone(),
            tools: catalog
                .tools
                .iter()
                .map(|(name, tool)| (name.clone(), tool.schema_digest.clone()))
                .collect(),
        }
    }
}
