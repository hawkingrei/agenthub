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
    let bytes = message_bytes(message)?;
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

/// A detached stdio thread can exit with the process without keeping Tokio's blocking pool alive
/// while the provider leaves stdin open after an RPC failure.
pub fn read_message_blocking(
    reader: &mut impl std::io::BufRead,
) -> Result<Option<Value>, McpTransportError> {
    let mut line = Vec::new();
    let mut limited = std::io::Read::take(reader, (MAX_MESSAGE_BYTES + 2) as u64);
    let count = std::io::BufRead::read_until(&mut limited, b'\n', &mut line)
        .map_err(|_| McpTransportError::Stdio)?;
    if count == 0 {
        return Ok(None);
    }
    if line.pop() != Some(b'\n') {
        return Err(if count >= MAX_MESSAGE_BYTES + 2 {
            McpTransportError::MessageTooLarge
        } else {
            McpTransportError::InvalidMessage
        });
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    parse_message(&line).map(Some)
}

pub fn write_message_blocking(
    writer: &mut impl std::io::Write,
    message: &Value,
) -> Result<(), McpTransportError> {
    let bytes = message_bytes(message)?;
    writer
        .write_all(&bytes)
        .and_then(|()| writer.write_all(b"\n"))
        .and_then(|()| writer.flush())
        .map_err(|_| McpTransportError::Stdio)
}

fn message_bytes(message: &Value) -> Result<Vec<u8>, McpTransportError> {
    message_kind(message)?;
    let bytes = serde_json::to_vec(message).map_err(|_| McpTransportError::InvalidMessage)?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(McpTransportError::MessageTooLarge);
    }
    Ok(bytes)
}
