use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

use crate::{
    MAX_MESSAGE_BYTES, McpTransportError,
    protocol::{message_kind, parse_message},
};

/// Read JSONL without allowing read_until to allocate an unbounded provider-controlled line.
pub async fn read_message(
    reader: &mut (impl AsyncBufRead + Unpin),
) -> Result<Option<Value>, McpTransportError> {
    let mut line = Vec::new();
    loop {
        let available = reader
            .fill_buf()
            .await
            .map_err(|_| McpTransportError::Stdio)?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(McpTransportError::InvalidMessage)
            };
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let count = newline.map_or(available.len(), |index| index + 1);
        if line.len().saturating_add(count) > MAX_MESSAGE_BYTES + 2 {
            return Err(McpTransportError::MessageTooLarge);
        }
        line.extend_from_slice(&available[..count]);
        reader.consume(count);
        if newline.is_some() {
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            return parse_message(&line).map(Some);
        }
    }
}

pub async fn write_message(
    writer: &mut (impl AsyncWrite + Unpin),
    message: &Value,
) -> Result<(), McpTransportError> {
    message_kind(message)?;
    let bytes = serde_json::to_vec(message).map_err(|_| McpTransportError::InvalidMessage)?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(McpTransportError::MessageTooLarge);
    }
    writer
        .write_all(&bytes)
        .await
        .map_err(|_| McpTransportError::Stdio)?;
    writer
        .write_all(b"\n")
        .await
        .map_err(|_| McpTransportError::Stdio)?;
    writer.flush().await.map_err(|_| McpTransportError::Stdio)
}
