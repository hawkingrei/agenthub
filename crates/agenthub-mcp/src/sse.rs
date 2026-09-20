use crate::{MAX_MESSAGE_BYTES, McpTransportError};

#[derive(Default)]
pub(crate) struct SseDecoder {
    line: Vec<u8>,
    data: Vec<u8>,
    cursor: Option<String>,
    retry_ms: Option<u64>,
    skip_lf: bool,
    first_line: bool,
}

pub(crate) struct SseFrame {
    pub data: Vec<u8>,
    pub cursor: Option<String>,
    pub retry_ms: Option<u64>,
}

impl SseDecoder {
    pub fn new() -> Self {
        Self {
            first_line: true,
            ..Self::default()
        }
    }

    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<SseFrame>, McpTransportError> {
        let mut frames = Vec::new();
        for &byte in bytes {
            if self.skip_lf && byte == b'\n' {
                self.skip_lf = false;
                continue;
            }
            self.skip_lf = false;
            if byte == b'\n' || byte == b'\r' {
                self.skip_lf = byte == b'\r';
                if let Some(frame) = self.finish_line()? {
                    frames.push(frame);
                }
            } else {
                if self.line.len().saturating_add(self.data.len()) >= MAX_MESSAGE_BYTES {
                    return Err(McpTransportError::MessageTooLarge);
                }
                self.line.push(byte);
            }
        }
        Ok(frames)
    }

    fn finish_line(&mut self) -> Result<Option<SseFrame>, McpTransportError> {
        let line = std::mem::take(&mut self.line);
        let line = if self.first_line {
            line.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&line)
        } else {
            &line
        };
        self.first_line = false;
        if line.is_empty() {
            if self.data.is_empty() && self.cursor.is_none() && self.retry_ms.is_none() {
                return Ok(None);
            }
            if self.data.last() == Some(&b'\n') {
                self.data.pop();
            }
            return Ok(Some(SseFrame {
                data: std::mem::take(&mut self.data),
                cursor: self.cursor.take(),
                retry_ms: self.retry_ms.take(),
            }));
        }
        let (field, value) = line
            .iter()
            .position(|byte| *byte == b':')
            .map_or((line, &b""[..]), |index| {
                (&line[..index], &line[index + 1..])
            });
        let value = value.strip_prefix(b" ").unwrap_or(value);
        match field {
            b"data" => {
                self.data.extend_from_slice(value);
                self.data.push(b'\n');
            }
            b"id" if !value.contains(&0) => {
                if value.len() > 4096 {
                    return Err(McpTransportError::MessageTooLarge);
                }
                self.cursor = Some(
                    std::str::from_utf8(value)
                        .map_err(|_| McpTransportError::InvalidResponse)?
                        .to_owned(),
                );
            }
            b"retry" if !value.is_empty() && value.iter().all(u8::is_ascii_digit) => {
                self.retry_ms = std::str::from_utf8(value)
                    .ok()
                    .and_then(|value| value.parse().ok());
            }
            _ => {}
        }
        if self.data.len() > MAX_MESSAGE_BYTES {
            return Err(McpTransportError::MessageTooLarge);
        }
        Ok(None)
    }
}
