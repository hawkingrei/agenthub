use std::collections::HashSet;

use super::*;
use crate::protocol::{correlation_id, message_kind};

/// A single owned HTTP batch and the tool intents derived from its exact bound members.
pub struct PreparedBatchCall {
    pub(crate) transport: McpHttpTransport,
    pub(crate) request: PreparedHttpRequest,
    pub(crate) executor: LoopReservation,
    pub(crate) tools: Vec<(Value, McpOperationIntent)>,
    pub(crate) expected: HashSet<[u8; 32]>,
}

impl McpBinding {
    /// The session controller owns method/callback/lifecycle admission for non-tool members.
    /// Tools use precisely the same scope and replay policy as an individual call.
    pub fn prepare_batch(
        &self,
        catalog: Option<&McpToolCatalog>,
        context: &McpCallContext<'_>,
        mut message: Value,
        mut bind_arguments: impl FnMut(&str, &Value, Value) -> Result<Value, McpPolicyError>,
    ) -> Result<PreparedBatchCall, McpPolicyError> {
        if context.http.version != ProtocolVersion::March2025
            || validate_versioned_message(&message, context.http.version)? != MessageKind::Batch
        {
            return Err(McpPolicyError::Call);
        }
        crate::budget::json_bytes(&message)?;
        let mut tools = Vec::new();
        let mut expected = HashSet::new();
        for member in message.as_array_mut().unwrap() {
            if message_kind(member)? == MessageKind::Request
                && !expected.insert(correlation_id(&member["id"]))
            {
                return Err(McpPolicyError::Call);
            }
            if member["method"] == "tools/call" {
                let name = member["params"]["name"]
                    .as_str()
                    .ok_or(McpPolicyError::Call)?
                    .to_owned();
                let call = self.prepare_call(
                    catalog.ok_or(McpPolicyError::ToolNotAvailable)?,
                    context,
                    member.clone(),
                    |schema, arguments| bind_arguments(&name, schema, arguments),
                )?;
                *member = call.request.message()?;
                tools.push((call.response_id, call.intent));
            }
        }
        let request = self.transport.prepare_post(context.http, &message, None)?;
        Ok(PreparedBatchCall {
            transport: self.transport.clone(),
            request,
            executor: context.executor.clone(),
            tools,
            expected,
        })
    }
}
