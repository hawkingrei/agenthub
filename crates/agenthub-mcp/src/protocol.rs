use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::McpTransportError;

/// Keep replay/callback correlation bounded even when a valid string ID fills a whole frame.
pub(crate) fn correlation_id(value: &Value) -> [u8; 32] {
    Sha256::digest(value.to_string().as_bytes()).into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolVersion {
    March2025,
    June2025,
    November2025,
    July2026,
}

impl ProtocolVersion {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::March2025 => "2025-03-26",
            Self::June2025 => "2025-06-18",
            Self::November2025 => "2025-11-25",
            Self::July2026 => "2026-07-28",
        }
    }

    pub const fn uses_initialization(self) -> bool {
        !matches!(self, Self::July2026)
    }
}

impl std::str::FromStr for ProtocolVersion {
    type Err = McpTransportError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "2025-03-26" => Ok(Self::March2025),
            "2025-06-18" => Ok(Self::June2025),
            "2025-11-25" => Ok(Self::November2025),
            "2026-07-28" => Ok(Self::July2026),
            _ => Err(McpTransportError::UnsupportedVersion),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Request,
    Notification,
    Response,
    Batch,
}

/// Inspect only the JSON-RPC envelope. Preserve extensions and result payloads unchanged.
pub fn message_kind(message: &Value) -> Result<MessageKind, McpTransportError> {
    if let Some(messages) = message.as_array() {
        if messages.is_empty() {
            return Err(McpTransportError::InvalidMessage);
        }
        if messages.len() > 256 {
            return Err(McpTransportError::MessageTooLarge);
        }
        for message in messages {
            if message.is_array() {
                return Err(McpTransportError::InvalidMessage);
            }
            message_kind(message)?;
        }
        return Ok(MessageKind::Batch);
    }
    let object = message
        .as_object()
        .ok_or(McpTransportError::InvalidMessage)?;
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(McpTransportError::InvalidMessage);
    }
    let id = object.get("id");
    if id.is_some_and(|id| !id.is_string() && !id.is_i64() && !id.is_u64() && !id.is_null()) {
        return Err(McpTransportError::InvalidMessage);
    }
    if let Some(method) = object.get("method") {
        if method.as_str().is_none_or(str::is_empty)
            || object.contains_key("result")
            || object.contains_key("error")
        {
            return Err(McpTransportError::InvalidMessage);
        }
        if id.is_some_and(Value::is_null) {
            return Err(McpTransportError::InvalidMessage);
        }
        return Ok(if id.is_some() {
            MessageKind::Request
        } else {
            MessageKind::Notification
        });
    }
    let result = object.contains_key("result");
    let error = object.get("error");
    if result == error.is_some() || (result && id.is_none_or(Value::is_null)) {
        return Err(McpTransportError::InvalidMessage);
    }
    if let Some(error) = error
        && (error.get("code").is_none_or(|value| !value.is_i64())
            || error.get("message").and_then(Value::as_str).is_none())
    {
        return Err(McpTransportError::InvalidMessage);
    }
    // HTTP-level MCP errors may omit the correlation ID, unlike successful RPC responses.
    Ok(MessageKind::Response)
}

pub fn validate_versioned_message(
    message: &Value,
    version: ProtocolVersion,
) -> Result<MessageKind, McpTransportError> {
    let kind = message_kind(message)?;
    if kind == MessageKind::Batch {
        if version != ProtocolVersion::March2025 {
            return Err(McpTransportError::InvalidMessage);
        }
        if message
            .as_array()
            .unwrap()
            .iter()
            .any(|message| message.get("method").and_then(Value::as_str) == Some("initialize"))
        {
            return Err(McpTransportError::InvalidMessage);
        }
    }
    Ok(kind)
}

pub fn parse_message(bytes: &[u8]) -> Result<Value, McpTransportError> {
    if bytes.len() > crate::MAX_MESSAGE_BYTES {
        return Err(McpTransportError::MessageTooLarge);
    }
    let value = serde_json::from_slice(bytes).map_err(|_| McpTransportError::InvalidMessage)?;
    message_kind(&value)?;
    Ok(value)
}
