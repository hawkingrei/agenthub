use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::{Handshake, ProtocolError, protocol::validate_id};

/// An explicitly authorized local source. Launch data must not enter diagnostics.
#[derive(Clone, Serialize)]
pub struct McpSource {
    pub source_id: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests;

impl McpSource {
    pub fn require_capability(handshake: &Handshake) -> Result<(), ProtocolError> {
        handshake.require_methods(&[
            "mcp_source.register",
            "mcp_source.unregister",
            "mcp_source.query",
        ])?;
        if !handshake
            .event_families
            .iter()
            .any(|family| family == "mcp")
        {
            return Err(ProtocolError::UnsupportedHandshake);
        }
        Ok(())
    }

    pub(super) fn operation(&self) -> Result<Value, ProtocolError> {
        validate_id(&self.source_id)?;
        if !Path::new(&self.command).is_absolute()
            || self.command.contains('\0')
            || self.args.len() > 128
            || self.args.iter().any(|arg| arg.contains('\0'))
            || self.env.len() > 64
            || self.env.iter().any(|(key, value)| {
                key.is_empty()
                    || key.len() > 256
                    || !key
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                    || value.contains('\0')
            })
        {
            return Err(ProtocolError::MalformedFrame);
        }
        let value = serde_json::to_value(self).map_err(|_| ProtocolError::Serialization)?;
        if serde_json::to_vec(&value)
            .map_err(|_| ProtocolError::Serialization)?
            .len()
            > 64 * 1024
        {
            return Err(ProtocolError::FrameTooLarge);
        }
        Ok(value)
    }
}
