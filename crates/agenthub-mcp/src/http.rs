//! Streamable HTTP without automatic retries, redirects, or provider-supplied credentials.

mod headers;
mod recovery;

pub use recovery::ResumableExchange;

#[cfg(test)]
mod tests;

use std::{collections::VecDeque, time::Duration};

use reqwest::{
    Client, Method, Request, Response, Url,
    header::{HeaderMap, HeaderValue},
};
use serde_json::Value;

use crate::{
    MAX_MESSAGE_BYTES, McpTransportError, network_error,
    protocol::{
        MessageKind, ProtocolVersion, message_kind, parse_message, validate_versioned_message,
    },
    sse::{SseDecoder, SseFrame},
};

pub use headers::ToolHeaderPlan;

/// Upstream session material stays in the daemon. Do not add Debug or Serialize.
#[derive(Clone)]
pub struct HttpSessionId(HeaderValue);

impl HttpSessionId {
    pub(crate) fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

#[derive(Clone)]
pub struct HttpContext {
    pub version: ProtocolVersion,
    pub session_id: Option<HttpSessionId>,
}

/// Constructed only from operator-resolved endpoint and secret references.
#[derive(Clone)]
pub struct McpHttpTransport {
    client: Client,
    endpoint: Url,
    headers: HeaderMap,
    timeout: Duration,
}

/// Construction performs no I/O; consume this once after committing the journal's sent boundary.
pub struct PreparedHttpRequest {
    request: Request,
    kind: ExchangeKind,
    version: ProtocolVersion,
}

impl PreparedHttpRequest {
    /// Batch composition reads the already bound wire body, never reconstructing caller intent
    /// from an operation digest. This request has not performed any I/O.
    pub(crate) fn message(&self) -> Result<Value, McpTransportError> {
        parse_message(
            self.request
                .body()
                .and_then(reqwest::Body::as_bytes)
                .ok_or(McpTransportError::InvalidMessage)?,
        )
    }
}

#[derive(Clone, Copy)]
enum ExchangeKind {
    Request,
    Acknowledgment,
    Listen,
    Subscription,
    Close,
}

pub struct HttpExchange {
    response: Response,
    session_id: Option<HttpSessionId>,
    mode: BodyMode,
    decoder: SseDecoder,
    frames: VecDeque<SseFrame>,
    version: ProtocolVersion,
}

enum BodyMode {
    Json,
    Events,
    Empty,
    Finished,
}

/// Control fields belong to upstream stream recovery, not the provider's JSONL output.
pub struct HttpEvent {
    pub message: Option<Value>,
    pub cursor: Option<String>,
    pub retry: Option<Duration>,
}

impl McpHttpTransport {
    pub fn new(
        endpoint: &str,
        mut headers: HeaderMap,
        timeout: Duration,
    ) -> Result<Self, McpTransportError> {
        let endpoint = Url::parse(endpoint).map_err(|_| McpTransportError::Configuration)?;
        if !matches!(endpoint.scheme(), "http" | "https")
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.fragment().is_some()
            || timeout.is_zero()
            || timeout > Duration::from_secs(3600)
        {
            return Err(McpTransportError::Configuration);
        }
        for (name, value) in headers.iter_mut() {
            if matches!(
                name.as_str(),
                "host"
                    | "content-length"
                    | "transfer-encoding"
                    | "connection"
                    | "accept"
                    | "content-type"
                    | "mcp-protocol-version"
                    | "mcp-session-id"
                    | "last-event-id"
                    | "mcp-method"
                    | "mcp-name"
                    | "proxy-authorization"
            ) || name.as_str().starts_with("mcp-param-")
            {
                return Err(McpTransportError::Configuration);
            }
            value.set_sensitive(true);
        }
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .referer(false)
            .build()
            .map_err(|_| McpTransportError::Configuration)?;
        Ok(Self {
            client,
            endpoint,
            headers,
            timeout,
        })
    }

    pub fn prepare_post(
        &self,
        context: &HttpContext,
        message: &Value,
        tool_headers: Option<&ToolHeaderPlan>,
    ) -> Result<PreparedHttpRequest, McpTransportError> {
        let kind = validate_versioned_message(message, context.version)?;
        let contains_request = if let Some(messages) = message.as_array() {
            let has_response = messages
                .iter()
                .any(|message| message_kind(message).ok() == Some(MessageKind::Response));
            if has_response
                && messages
                    .iter()
                    .any(|message| message_kind(message).ok() != Some(MessageKind::Response))
            {
                return Err(McpTransportError::InvalidMessage);
            }
            messages
                .iter()
                .any(|message| message_kind(message).ok() == Some(MessageKind::Request))
        } else {
            kind == MessageKind::Request
        };
        if !context.version.uses_initialization() && kind == MessageKind::Response {
            return Err(McpTransportError::InvalidMessage);
        }
        let mut headers = self.request_headers(context)?;
        headers.insert(
            "accept",
            HeaderValue::from_static("application/json, text/event-stream"),
        );
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        if !context.version.uses_initialization() && kind == MessageKind::Request {
            headers::add_request_metadata(&mut headers, context.version, message, tool_headers)?;
        }
        let body = serde_json::to_vec(message).map_err(|_| McpTransportError::InvalidMessage)?;
        if body.len() > MAX_MESSAGE_BYTES {
            return Err(McpTransportError::MessageTooLarge);
        }
        let request = self
            .client
            .post(self.endpoint.clone())
            .headers(headers)
            .timeout(self.timeout)
            .body(body)
            .build()
            .map_err(|_| McpTransportError::Configuration)?;
        Ok(PreparedHttpRequest {
            request,
            version: context.version,
            kind: if contains_request {
                ExchangeKind::Request
            } else {
                ExchangeKind::Acknowledgment
            },
        })
    }

    pub(crate) fn prepare_subscription(
        &self,
        context: &HttpContext,
        message: &Value,
    ) -> Result<PreparedHttpRequest, McpTransportError> {
        if context.version != ProtocolVersion::July2026
            || message_kind(message)? != MessageKind::Request
            || message["method"] != "subscriptions/listen"
        {
            return Err(McpTransportError::InvalidMessage);
        }
        let mut request = self.prepare_post(context, message, None)?;
        request.kind = ExchangeKind::Subscription;
        Ok(request)
    }

    /// Legacy SSE recovery uses GET, never a repeated POST of the original tool call.
    pub fn prepare_listen(
        &self,
        context: &HttpContext,
        cursor: Option<&str>,
    ) -> Result<PreparedHttpRequest, McpTransportError> {
        if !context.version.uses_initialization() {
            return Err(McpTransportError::UnsupportedVersion);
        }
        let mut headers = self.request_headers(context)?;
        headers.insert("accept", HeaderValue::from_static("text/event-stream"));
        if let Some(cursor) = cursor {
            if cursor.len() > 4096 {
                return Err(McpTransportError::InvalidMetadata);
            }
            let mut value =
                HeaderValue::from_str(cursor).map_err(|_| McpTransportError::InvalidMetadata)?;
            value.set_sensitive(true);
            headers.insert("last-event-id", value);
        }
        let request = self
            .client
            .get(self.endpoint.clone())
            .headers(headers)
            .timeout(self.timeout)
            .build()
            .map_err(|_| McpTransportError::Configuration)?;
        Ok(PreparedHttpRequest {
            request,
            kind: ExchangeKind::Listen,
            version: context.version,
        })
    }

    pub fn prepare_close(
        &self,
        context: &HttpContext,
    ) -> Result<PreparedHttpRequest, McpTransportError> {
        if !context.version.uses_initialization() || context.session_id.is_none() {
            return Err(McpTransportError::InvalidMetadata);
        }
        let request = self
            .client
            .request(Method::DELETE, self.endpoint.clone())
            .headers(self.request_headers(context)?)
            .timeout(self.timeout)
            .build()
            .map_err(|_| McpTransportError::Configuration)?;
        Ok(PreparedHttpRequest {
            request,
            kind: ExchangeKind::Close,
            version: context.version,
        })
    }

    fn request_headers(&self, context: &HttpContext) -> Result<HeaderMap, McpTransportError> {
        let mut headers = self.headers.clone();
        headers.insert(
            "mcp-protocol-version",
            HeaderValue::from_static(context.version.as_str()),
        );
        if let Some(session_id) = &context.session_id {
            if !context.version.uses_initialization() {
                return Err(McpTransportError::InvalidMetadata);
            }
            headers.insert("mcp-session-id", session_id.0.clone());
        }
        Ok(headers)
    }

    pub async fn send(
        &self,
        prepared: PreparedHttpRequest,
    ) -> Result<HttpExchange, McpTransportError> {
        let response = self
            .client
            .execute(prepared.request)
            .await
            .map_err(network_error)?;
        let status = response.status();
        if status.is_redirection() {
            return Err(McpTransportError::HttpStatus(status.as_u16()));
        }
        let session_id = response
            .headers()
            .get("mcp-session-id")
            .map(|value| {
                if value.is_empty()
                    || value.as_bytes().len() > 4096
                    || !value
                        .as_bytes()
                        .iter()
                        .all(|byte| (0x21..=0x7e).contains(byte))
                {
                    return Err(McpTransportError::InvalidResponse);
                }
                let mut value = value.clone();
                value.set_sensitive(true);
                Ok(HttpSessionId(value))
            })
            .transpose()?;
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim);
        let mode = if (matches!(prepared.kind, ExchangeKind::Acknowledgment)
            && status.as_u16() == 202)
            || (matches!(prepared.kind, ExchangeKind::Listen) && status.as_u16() == 405)
            || (matches!(prepared.kind, ExchangeKind::Close)
                && (status.is_success() || status.as_u16() == 405 || status.as_u16() == 404))
        {
            BodyMode::Empty
        } else {
            match content_type {
                Some(value) if value.eq_ignore_ascii_case("application/json") => BodyMode::Json,
                Some(value)
                    if value.eq_ignore_ascii_case("text/event-stream") && status.is_success() =>
                {
                    BodyMode::Events
                }
                _ if !status.is_success() => {
                    return Err(McpTransportError::HttpStatus(status.as_u16()));
                }
                _ => return Err(McpTransportError::InvalidResponse),
            }
        };
        if let Some(length) = response.content_length()
            && matches!(mode, BodyMode::Json)
            && length > MAX_MESSAGE_BYTES as u64
        {
            return Err(McpTransportError::MessageTooLarge);
        }
        Ok(HttpExchange {
            response,
            session_id,
            mode,
            decoder: SseDecoder::new(),
            frames: VecDeque::new(),
            version: prepared.version,
        })
    }
}

impl HttpExchange {
    pub fn status_code(&self) -> u16 {
        self.response.status().as_u16()
    }

    pub fn session_id(&self) -> Option<HttpSessionId> {
        self.session_id.clone()
    }

    pub async fn next_event(&mut self) -> Result<Option<HttpEvent>, McpTransportError> {
        match self.mode {
            BodyMode::Finished => Ok(None),
            BodyMode::Empty => {
                self.mode = BodyMode::Finished;
                // Read the acknowledgment so a premature disconnect is not mistaken for success.
                let body = self.read_bounded_body().await?;
                if self.status_code() == 202 && !body.is_empty() {
                    return Err(McpTransportError::InvalidResponse);
                }
                Ok(None)
            }
            BodyMode::Json => {
                self.mode = BodyMode::Finished;
                let message = parse_message(&self.read_bounded_body().await?)?;
                let kind = validate_versioned_message(&message, self.version)?;
                let is_response = kind == MessageKind::Response
                    || (kind == MessageKind::Batch
                        && message.as_array().unwrap().iter().all(|message| {
                            message_kind(message).ok() == Some(MessageKind::Response)
                        }));
                let is_error = message.get("error").is_some()
                    || (kind == MessageKind::Batch
                        && message
                            .as_array()
                            .unwrap()
                            .iter()
                            .all(|message| message.get("error").is_some()));
                if !is_response || (!self.response.status().is_success() && !is_error) {
                    return Err(McpTransportError::InvalidResponse);
                }
                Ok(Some(HttpEvent {
                    message: Some(message),
                    cursor: None,
                    retry: None,
                }))
            }
            BodyMode::Events => loop {
                if let Some(frame) = self.frames.pop_front() {
                    let message = if frame.data.is_empty() {
                        None
                    } else {
                        let message = parse_message(&frame.data)?;
                        validate_versioned_message(&message, self.version)?;
                        if !self.version.uses_initialization()
                            && message_kind(&message)? == MessageKind::Request
                        {
                            return Err(McpTransportError::InvalidResponse);
                        }
                        Some(message)
                    };
                    return Ok(Some(HttpEvent {
                        message,
                        cursor: frame.cursor,
                        retry: frame.retry_ms.map(Duration::from_millis),
                    }));
                }
                let Some(bytes) = self.response.chunk().await.map_err(network_error)? else {
                    self.mode = BodyMode::Finished;
                    return Ok(None);
                };
                if bytes.len() > MAX_MESSAGE_BYTES {
                    return Err(McpTransportError::MessageTooLarge);
                }
                self.frames.extend(self.decoder.push(&bytes)?);
            },
        }
    }

    async fn read_bounded_body(&mut self) -> Result<Vec<u8>, McpTransportError> {
        let mut body = Vec::new();
        while let Some(bytes) = self.response.chunk().await.map_err(network_error)? {
            if body.len().saturating_add(bytes.len()) > MAX_MESSAGE_BYTES {
                return Err(McpTransportError::MessageTooLarge);
            }
            body.extend_from_slice(&bytes);
        }
        Ok(body)
    }
}
