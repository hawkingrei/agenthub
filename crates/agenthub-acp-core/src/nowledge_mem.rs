//! Policy primitives for the AgentHub-owned Nowledge Mem MCP proxy.
//!
//! This module deliberately knows nothing about Mem transport credentials or
//! tool names. The proxy receives the upstream schema at runtime and uses these
//! helpers to preserve it while applying an already-bound AgentHub scope.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemScopeBinding {
    pub profile_ref: String,
    pub space_id: String,
}

impl MemScopeBinding {
    pub fn new(
        profile_ref: impl Into<String>,
        space_id: impl Into<String>,
    ) -> anyhow::Result<Self> {
        let profile_ref = profile_ref.into().trim().to_string();
        let space_id = space_id.into().trim().to_string();
        if profile_ref.is_empty() {
            anyhow::bail!("Mem profile reference is required");
        }
        if space_id.is_empty() {
            anyhow::bail!("Mem space_id is required");
        }
        Ok(Self {
            profile_ref,
            space_id,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemJournalStatus {
    Prepared,
    Sent,
    Succeeded,
    Failed,
    OutcomeUnknown,
}

impl MemJournalStatus {
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Prepared, Self::Sent)
                | (Self::Prepared, Self::Failed)
                | (Self::Sent, Self::Succeeded)
                | (Self::Sent, Self::Failed)
                | (Self::Sent, Self::OutcomeUnknown)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemResponseErrorKind {
    McpResult,
    JsonRpc,
    SuccessEnvelope,
}

/// Adds the bound scope only when the upstream JSON schema declares a
/// `space_id` property. A caller-provided conflicting value is rejected rather
/// than silently redirected to another space.
pub fn bind_declared_space_id(
    input_schema: &Value,
    arguments: Value,
    binding: &MemScopeBinding,
) -> anyhow::Result<Value> {
    let declares_space_id = input_schema
        .get("properties")
        .and_then(Value::as_object)
        .is_some_and(|properties| properties.contains_key("space_id"));
    if !declares_space_id {
        return Ok(arguments);
    }

    let mut arguments = arguments.as_object().cloned().ok_or_else(|| {
        anyhow::anyhow!("MCP tool arguments must be an object when schema declares space_id")
    })?;
    if let Some(provided) = arguments.get("space_id").and_then(Value::as_str)
        && provided != binding.space_id
    {
        anyhow::bail!("MCP tool call space_id does not match the bound Mem space");
    }
    arguments.insert(
        "space_id".to_string(),
        Value::String(binding.space_id.clone()),
    );
    Ok(Value::Object(arguments))
}

/// Recognizes the three error shapes observed across the existing Mem MCP
/// surfaces while preserving the upstream value for the caller.
pub fn classify_response_error(response: &Value) -> Option<MemResponseErrorKind> {
    if response.get("error").is_some() {
        return Some(MemResponseErrorKind::JsonRpc);
    }
    if response
        .get("result")
        .and_then(|result| result.get("isError"))
        .and_then(Value::as_bool)
        == Some(true)
    {
        return Some(MemResponseErrorKind::McpResult);
    }
    response
        .get("result")
        .and_then(|result| result.get("error"))
        .is_some()
        .then_some(MemResponseErrorKind::SuccessEnvelope)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        MemJournalStatus, MemResponseErrorKind, MemScopeBinding, bind_declared_space_id,
        classify_response_error,
    };

    fn binding() -> MemScopeBinding {
        MemScopeBinding::new("member-profile", "space-a").expect("valid binding")
    }

    #[test]
    fn declared_space_is_injected() {
        let result = bind_declared_space_id(
            &json!({"properties":{"query":{"type":"string"},"space_id":{"type":"string"}}}),
            json!({"query":"hello"}),
            &binding(),
        )
        .expect("bind scope");
        assert_eq!(result["space_id"], "space-a");
        assert_eq!(result["query"], "hello");
    }

    #[test]
    fn undeclared_space_is_not_added() {
        let arguments = json!({"memory_id":"memory-1"});
        assert_eq!(
            bind_declared_space_id(
                &json!({"properties":{"memory_id":{"type":"string"}}}),
                arguments.clone(),
                &binding(),
            )
            .expect("leave arguments unchanged"),
            arguments
        );
    }

    #[test]
    fn conflicting_space_is_rejected() {
        let err = bind_declared_space_id(
            &json!({"properties":{"space_id":{"type":"string"}}}),
            json!({"space_id":"other-space"}),
            &binding(),
        )
        .expect_err("reject cross-space call");
        assert!(err.to_string().contains("does not match"));
    }

    #[test]
    fn error_shapes_are_classified_without_rewriting() {
        assert_eq!(
            classify_response_error(&json!({"result":{"isError":true}})),
            Some(MemResponseErrorKind::McpResult)
        );
        assert_eq!(
            classify_response_error(&json!({"error":{"code":-32601}})),
            Some(MemResponseErrorKind::JsonRpc)
        );
        assert_eq!(
            classify_response_error(&json!({"result":{"error":"denied"}})),
            Some(MemResponseErrorKind::SuccessEnvelope)
        );
    }

    #[test]
    fn journal_never_replays_an_unknown_outcome() {
        assert!(MemJournalStatus::Prepared.can_transition_to(MemJournalStatus::Sent));
        assert!(MemJournalStatus::Sent.can_transition_to(MemJournalStatus::OutcomeUnknown));
        assert!(!MemJournalStatus::OutcomeUnknown.can_transition_to(MemJournalStatus::Sent));
        assert!(!MemJournalStatus::Succeeded.can_transition_to(MemJournalStatus::Sent));
    }
}
