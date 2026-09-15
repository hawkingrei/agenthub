//! Trusted binding and discovery policy. None of these capabilities deserialize from provider RPCs.

use std::collections::BTreeMap;

use agenthub_agent_domain::{
    loop_runtime::LoopReservation,
    mcp_operations::{McpDigest, McpOperationIntent, McpReplaySafety},
};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{
    McpTransportError,
    digest::digest,
    http::{HttpContext, McpHttpTransport, PreparedHttpRequest, ToolHeaderPlan},
    protocol::{MessageKind, ProtocolVersion, validate_versioned_message},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum McpPolicyError {
    #[error("invalid MCP discovery catalog")]
    Catalog,
    #[error("MCP tool is not in the approved discovery catalog")]
    ToolNotAvailable,
    #[error("invalid MCP binding policy")]
    Binding,
    #[error("invalid MCP tool call")]
    Call,
    #[error("MCP call scope does not match its binding")]
    Scope,
    #[error("MCP call requires its declared stable identity")]
    StableIdentity,
    #[error("MCP continuation requires a linked upstream receipt")]
    Continuation,
    #[error(transparent)]
    Transport(#[from] McpTransportError),
}

/// Granted by the configured integration, never inferred from an untrusted idempotentHint.
#[derive(Clone)]
pub enum TrustedReplayPolicy {
    ReadOnly,
    NonIdempotent,
    StableIdentity { property_path: Vec<String> },
}

struct DiscoveredTool {
    declaration: Value,
    schema_digest: McpDigest,
    headers: ToolHeaderPlan,
}

/// A bounded complete discovery snapshot. The controller owns pagination and refresh sequencing.
pub struct McpToolCatalog {
    version: ProtocolVersion,
    tools: BTreeMap<String, DiscoveredTool>,
    order: Vec<String>,
}

impl McpToolCatalog {
    pub fn from_tools(tools: &Value, version: ProtocolVersion) -> Result<Self, McpPolicyError> {
        let declarations = tools.as_array().ok_or(McpPolicyError::Catalog)?;
        if declarations.len() > 1024 {
            return Err(McpPolicyError::Catalog);
        }
        // Also bound the aggregate catalog, not just each individual schema.
        digest("mcp-catalog-v1", tools)?;
        let mut catalog = Self {
            version,
            tools: BTreeMap::new(),
            order: Vec::new(),
        };
        let mut names = std::collections::HashSet::new();
        for declaration in declarations {
            let name = declaration
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| {
                    !name.is_empty() && name.len() <= 4096 && !name.chars().any(char::is_control)
                })
                .ok_or(McpPolicyError::Catalog)?;
            if !names.insert(name) {
                return Err(McpPolicyError::Catalog);
            }
            let schema = declaration
                .get("inputSchema")
                .filter(|schema| schema.is_object())
                .ok_or(McpPolicyError::Catalog)?;
            if schema.get("type").and_then(Value::as_str) != Some("object") {
                return Err(McpPolicyError::Catalog);
            }
            // Header annotations affect HTTP admission in July 2026. Older versions preserve
            // unknown annotations as data and do not acquire newer requirements retroactively.
            let headers = if version.uses_initialization() {
                ToolHeaderPlan::from_schema(&json!({}))?
            } else {
                let Ok(headers) = ToolHeaderPlan::from_schema(schema) else {
                    continue;
                };
                headers
            };
            catalog.order.push(name.to_owned());
            catalog.tools.insert(
                name.to_owned(),
                DiscoveredTool {
                    declaration: declaration.clone(),
                    schema_digest: digest("mcp-tool-schema-v1", schema)?,
                    headers,
                },
            );
        }
        Ok(catalog)
    }

    pub fn advertised_tools(&self) -> Value {
        Value::Array(
            self.order
                .iter()
                .map(|name| self.tools[name].declaration.clone())
                .collect(),
        )
    }
}

/// The scope identity must name the effective upstream authority and namespace independently of
/// profile aliases, binding revisions and rotating secrets. Resolve it in the integration adapter.
pub struct McpBinding {
    server_id: String,
    scope_digest: McpDigest,
    binding_digest: McpDigest,
    transport: McpHttpTransport,
    replay: BTreeMap<String, TrustedReplayPolicy>,
}

pub struct McpCallContext<'a> {
    pub executor: &'a LoopReservation,
    pub proxy_session_id: &'a str,
    pub http: &'a HttpContext,
}

/// The wire bytes and their journal intent are inseparable and non-cloneable after preparation.
/// Payloads, credentials, and opaque upstream state must not be exposed through Debug/Serialize.
pub struct PreparedToolCall {
    pub(crate) transport: McpHttpTransport,
    pub(crate) request: PreparedHttpRequest,
    pub(crate) intent: McpOperationIntent,
    pub(crate) executor: LoopReservation,
    pub(crate) response_id: Value,
}

impl McpBinding {
    pub fn new(
        server_id: String,
        effective_scope: &Value,
        revision: &Value,
        transport: McpHttpTransport,
        replay: BTreeMap<String, TrustedReplayPolicy>,
    ) -> Result<Self, McpPolicyError> {
        if server_id.is_empty()
            || server_id.len() > 128
            || !server_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
            || !effective_scope.is_object()
            || effective_scope
                .as_object()
                .is_none_or(|scope| scope.is_empty())
            || revision.is_null()
        {
            return Err(McpPolicyError::Binding);
        }
        let policy_fingerprint: BTreeMap<_, _> = replay
            .iter()
            .map(|(tool, policy)| {
                let policy = match policy {
                    TrustedReplayPolicy::ReadOnly => json!({"kind":"read_only"}),
                    TrustedReplayPolicy::NonIdempotent => json!({"kind":"non_idempotent"}),
                    TrustedReplayPolicy::StableIdentity { property_path } => {
                        json!({"kind":"stable_identity","property_path":property_path})
                    }
                };
                (tool, policy)
            })
            .collect();
        Ok(Self {
            server_id,
            scope_digest: digest("mcp-effective-scope-v1", effective_scope)?,
            binding_digest: digest(
                "mcp-binding-revision-v1",
                &json!([revision, policy_fingerprint]),
            )?,
            transport,
            replay,
        })
    }

    /// bind_arguments is trusted integration policy (Mem scope injection, or manifest validation),
    /// not a provider hook. It runs against the exact discovered schema before hashing or sending.
    pub fn prepare_call(
        &self,
        catalog: &McpToolCatalog,
        context: &McpCallContext<'_>,
        mut message: Value,
        bind_arguments: impl FnOnce(&Value, Value) -> Result<Value, McpPolicyError>,
    ) -> Result<PreparedToolCall, McpPolicyError> {
        if catalog.version != context.http.version
            || validate_versioned_message(&message, context.http.version)? != MessageKind::Request
            || message["method"] != "tools/call"
            || context.proxy_session_id.is_empty()
            || context.proxy_session_id.len() > 128
        {
            return Err(McpPolicyError::Call);
        }
        let activation = context
            .executor
            .activation_id
            .as_deref()
            .ok_or(McpPolicyError::Scope)?;
        let response_id = message["id"].clone();
        let params = message
            .get_mut("params")
            .and_then(Value::as_object_mut)
            .ok_or(McpPolicyError::Call)?;
        // The continuation controller must bind these to a recorded upstream receipt. They cannot
        // enter the ordinary-call path and disguise a repeated write as a new JSON-RPC request.
        if params.contains_key("requestState") || params.contains_key("inputResponses") {
            return Err(McpPolicyError::Continuation);
        }
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or(McpPolicyError::Call)?
            .to_owned();
        let tool = catalog
            .tools
            .get(&name)
            .ok_or(McpPolicyError::ToolNotAvailable)?;
        let schema = &tool.declaration["inputSchema"];
        let original_arguments = params.get("arguments").cloned();
        let arguments = original_arguments.clone().unwrap_or_else(|| json!({}));
        if !arguments.is_object() {
            return Err(McpPolicyError::Call);
        }
        let arguments = bind_arguments(schema, arguments)?;
        if !arguments.is_object() {
            return Err(McpPolicyError::Call);
        }
        let replay_safety = match self
            .replay
            .get(&name)
            .unwrap_or(&TrustedReplayPolicy::NonIdempotent)
        {
            TrustedReplayPolicy::ReadOnly => McpReplaySafety::ReadOnly,
            TrustedReplayPolicy::NonIdempotent => McpReplaySafety::NonIdempotent,
            TrustedReplayPolicy::StableIdentity { property_path } => {
                McpReplaySafety::StableIdentity {
                    identity_digest: stable_identity(schema, &arguments, property_path)?,
                }
            }
        };
        if original_arguments.is_some() || arguments != json!({}) {
            params.insert("arguments".into(), arguments.clone());
        }
        // Progress tokens are delivery correlation only. All other parameters remain fixed on a
        // retry, including protocol metadata and extensions that may affect upstream behavior.
        let mut semantic_request = Value::Object(params.clone());
        if let Some(meta) = semantic_request
            .get_mut("_meta")
            .and_then(Value::as_object_mut)
        {
            meta.remove("progressToken");
            if meta.is_empty() {
                semantic_request.as_object_mut().unwrap().remove("_meta");
            }
        }
        let request_key = if let Some(identity) = replay_safety.identity_digest() {
            digest(
                "mcp-stable-request-v1",
                &json!([self.scope_digest, name, identity]),
            )?
        } else {
            digest(
                "mcp-rpc-request-v1",
                &json!([activation, context.proxy_session_id, response_id]),
            )?
        };
        let intent = McpOperationIntent {
            request_key,
            server_id: self.server_id.clone(),
            scope_digest: self.scope_digest.clone(),
            binding_digest: self.binding_digest.clone(),
            tool_name: name,
            schema_digest: tool.schema_digest.clone(),
            arguments_digest: digest("mcp-tool-arguments-v1", &arguments)?,
            request_digest: Some(digest("mcp-call-parameters-v1", &semantic_request)?),
            replay_safety,
        };
        let request = self
            .transport
            .prepare_post(context.http, &message, Some(&tool.headers))?;
        Ok(PreparedToolCall {
            transport: self.transport.clone(),
            request,
            intent,
            executor: context.executor.clone(),
            response_id,
        })
    }
}

fn stable_identity(
    schema: &Value,
    arguments: &Value,
    path: &[String],
) -> Result<McpDigest, McpPolicyError> {
    if path.is_empty() || path.len() > 64 {
        return Err(McpPolicyError::Binding);
    }
    let mut declaration = schema;
    let mut value = arguments;
    for property in path {
        declaration = declaration
            .get("properties")
            .and_then(|properties| properties.get(property))
            .ok_or(McpPolicyError::StableIdentity)?;
        value = value.get(property).ok_or(McpPolicyError::StableIdentity)?;
    }
    if declaration.get("type").and_then(Value::as_str) != Some("string")
        || value
            .as_str()
            .is_none_or(|identity| identity.is_empty() || identity.len() > 4096)
    {
        return Err(McpPolicyError::StableIdentity);
    }
    Ok(digest("mcp-caller-identity-v1", value)?)
}
