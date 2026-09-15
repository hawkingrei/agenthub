//! Raw MCP transport and policy boundaries shared by local integrations.

pub mod access;
pub mod bridge;
pub mod budget;
mod capabilities;
mod continuation;
mod digest;
pub mod http;
pub mod journal;
pub mod policy;
pub mod protocol;
pub mod session;
mod sse;
pub mod stdio;
mod task;

#[cfg(test)]
mod tests;

use thiserror::Error;

pub const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;

/// No variant retains reqwest/JSON errors, request bodies, headers, or upstream URLs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum McpTransportError {
    #[error("invalid MCP upstream configuration")]
    Configuration,
    #[error("unsupported MCP protocol version")]
    UnsupportedVersion,
    #[error("invalid MCP protocol message")]
    InvalidMessage,
    #[error("invalid MCP request metadata")]
    InvalidMetadata,
    #[error("MCP message exceeds the transport limit")]
    MessageTooLarge,
    #[error("MCP proxy payload capacity is exhausted")]
    Capacity,
    #[error("MCP transport disconnected")]
    Disconnected,
    #[error("MCP request deadline expired")]
    Deadline,
    #[error("MCP upstream returned an invalid response")]
    InvalidResponse,
    #[error("MCP upstream returned HTTP status {0}")]
    HttpStatus(u16),
    #[error("MCP stdio transport failed")]
    Stdio,
}

fn network_error(error: reqwest::Error) -> McpTransportError {
    if error.is_timeout() {
        McpTransportError::Deadline
    } else {
        McpTransportError::Disconnected
    }
}
