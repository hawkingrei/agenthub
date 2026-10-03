use std::io::Write;

use tokio::io::{AsyncBufRead, AsyncBufReadExt};

use crate::{ClientFrame, MAX_FRAME_BYTES, ProtocolError, ServerFrame};

/// Keeps partial bytes across cancellation of `next`, including select-loop wakeups.
pub struct FrameReader<R> {
    input: R,
    pending: Vec<u8>,
}

impl<R: AsyncBufRead + Unpin> FrameReader<R> {
    pub fn new(input: R) -> Self {
        Self {
            input,
            pending: Vec::with_capacity(MAX_FRAME_BYTES + 1),
        }
    }

    pub async fn next(&mut self) -> Result<ServerFrame, ProtocolError> {
        self.read_frame().await?.ok_or(ProtocolError::TransportLost)
    }

    /// Clean EOF is distinct from I/O failure when verifying a semantic shutdown.
    pub(crate) async fn read_frame(&mut self) -> Result<Option<ServerFrame>, ProtocolError> {
        loop {
            let bytes = self
                .input
                .fill_buf()
                .await
                .map_err(|_| ProtocolError::TransportLost)?;
            if bytes.is_empty() {
                return if self.pending.is_empty() {
                    Ok(None)
                } else {
                    Err(ProtocolError::MalformedFrame)
                };
            }
            let delimiter = bytes.iter().position(|byte| *byte == b'\n');
            let count = delimiter.unwrap_or(bytes.len());
            if count > (MAX_FRAME_BYTES + 1).saturating_sub(self.pending.len()) {
                return Err(ProtocolError::FrameTooLarge);
            }
            self.pending.extend_from_slice(&bytes[..count]);
            self.input.consume(count + usize::from(delimiter.is_some()));
            if delimiter.is_some() {
                if self.pending.last() == Some(&b'\r') {
                    self.pending.pop();
                }
                let result = decode(&self.pending);
                self.pending.clear();
                return result.map(Some);
            }
        }
    }
}

fn decode(payload: &[u8]) -> Result<ServerFrame, ProtocolError> {
    if payload.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    if payload.contains(&b'\r') || payload.contains(&b'\n') {
        return Err(ProtocolError::MalformedFrame);
    }
    let frame: ServerFrame =
        serde_json::from_slice(payload).map_err(|_| ProtocolError::MalformedFrame)?;
    frame.validate()?;
    Ok(frame)
}

pub fn encode_request(frame: &ClientFrame) -> Result<Vec<u8>, ProtocolError> {
    frame.validate()?;
    let mut writer = BoundedWriter {
        bytes: Vec::new(),
        exceeded: false,
    };
    if serde_json::to_writer(&mut writer, frame).is_err() {
        return Err(if writer.exceeded {
            ProtocolError::FrameTooLarge
        } else {
            ProtocolError::Serialization
        });
    }
    writer.bytes.reserve_exact(1);
    writer.bytes.push(b'\n');
    Ok(writer.bytes)
}

struct BoundedWriter {
    bytes: Vec<u8>,
    exceeded: bool,
}

impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_FRAME_BYTES.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(std::io::Error::other("frame limit"));
        }
        let needed = self.bytes.len() + bytes.len();
        if needed > self.bytes.capacity() {
            let capacity = needed.next_power_of_two().min(MAX_FRAME_BYTES + 1);
            self.bytes.reserve_exact(capacity - self.bytes.len());
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
