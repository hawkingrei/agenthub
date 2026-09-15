use super::*;
use tokio::time::{Instant, sleep, timeout, timeout_at};

/// One originating stream. Cursor/session metadata never crosses into another exchange or stdout.
pub struct ResumableExchange {
    transport: McpHttpTransport,
    context: HttpContext,
    exchange: HttpExchange,
    deadline: Option<Instant>,
    cursor: Option<String>,
    retry: Duration,
    reconnects: u8,
    resumable: bool,
    listening: bool,
}

impl McpHttpTransport {
    /// The original request is consumed exactly once. Recovery can only create GET requests.
    pub async fn send_resumable(
        &self,
        mut request: PreparedHttpRequest,
    ) -> Result<ResumableExchange, McpTransportError> {
        let listening = matches!(
            request.kind,
            ExchangeKind::Listen | ExchangeKind::Subscription
        );
        let deadline = (!listening).then(|| Instant::now() + self.timeout);
        let eligible = matches!(request.kind, ExchangeKind::Request | ExchangeKind::Listen)
            && request.version.uses_initialization();
        let mut context = HttpContext {
            version: request.version,
            session_id: request
                .request
                .headers()
                .get("mcp-session-id")
                .cloned()
                .map(HttpSessionId),
        };
        if listening {
            // Idle server notification streams are cancellable by their owner, not bounded by
            // the lifetime of an ordinary tool request. Connection establishment stays bounded.
            *request.request.timeout_mut() = None;
        }
        let exchange = timeout(self.timeout, self.send(request))
            .await
            .map_err(|_| McpTransportError::Deadline)??;
        if let Some(session) = exchange.session_id() {
            context.session_id = Some(session);
        }
        let resumable = eligible && matches!(exchange.mode, BodyMode::Events);
        Ok(ResumableExchange {
            transport: self.clone(),
            context,
            exchange,
            deadline,
            cursor: None,
            retry: Duration::from_millis(100),
            reconnects: 0,
            resumable,
            listening,
        })
    }
}

impl ResumableExchange {
    pub fn status_code(&self) -> u16 {
        self.exchange.status_code()
    }

    pub fn session_id(&self) -> Option<HttpSessionId> {
        self.context.session_id.clone()
    }

    pub async fn next_event(&mut self) -> Result<Option<HttpEvent>, McpTransportError> {
        loop {
            let next = if let Some(deadline) = self.deadline {
                timeout_at(deadline, self.exchange.next_event())
                    .await
                    .map_err(|_| McpTransportError::Deadline)?
            } else {
                self.exchange.next_event().await
            };
            let failure = match next {
                Ok(Some(event)) => {
                    if let Some(cursor) = &event.cursor {
                        if self.listening && self.cursor.as_ref() != Some(cursor) {
                            self.reconnects = 0;
                        }
                        self.cursor = (!cursor.is_empty()).then(|| cursor.clone());
                    }
                    if let Some(retry) = event.retry {
                        self.retry = retry;
                    }
                    return Ok(Some(event));
                }
                Ok(None) if !self.resumable || self.cursor.is_none() => return Ok(None),
                Ok(None) => McpTransportError::Disconnected,
                Err(error @ (McpTransportError::Disconnected | McpTransportError::Deadline)) => {
                    error
                }
                Err(error) => return Err(error),
            };
            if !self.resumable || self.cursor.is_none() || self.reconnects >= 3 {
                return Err(failure);
            }
            if self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                return Err(McpTransportError::Deadline);
            }
            self.reconnects += 1;
            let mut request = self
                .transport
                .prepare_listen(&self.context, self.cursor.as_deref())?;
            if self.listening {
                *request.request.timeout_mut() = None;
            }
            let reconnect = async {
                // A server's retry hint is a minimum delay, never clamped down to fit our budget.
                sleep(self.retry).await;
                timeout(self.transport.timeout, self.transport.send(request))
                    .await
                    .map_err(|_| McpTransportError::Deadline)?
            };
            let result = if let Some(deadline) = self.deadline {
                timeout_at(deadline, reconnect)
                    .await
                    .map_err(|_| McpTransportError::Deadline)?
            } else {
                reconnect.await
            };
            match result {
                Ok(exchange) => {
                    if !(matches!(exchange.mode, BodyMode::Events)
                        || matches!(exchange.mode, BodyMode::Json) && exchange.status_code() >= 400)
                    {
                        // GET resumption cannot silently become an unrelated JSON response or
                        // a fresh unsupported subscription. Keep the original write uncertain.
                        return Err(McpTransportError::HttpStatus(exchange.status_code()));
                    }
                    if let Some(session) = exchange.session_id()
                        && self
                            .context
                            .session_id
                            .as_ref()
                            .is_none_or(|current| current.0 != session.0)
                    {
                        return Err(McpTransportError::InvalidResponse);
                    }
                    self.resumable = matches!(exchange.mode, BodyMode::Events);
                    self.exchange = exchange;
                }
                Err(McpTransportError::Disconnected) => continue,
                Err(error) => return Err(error),
            }
        }
    }
}
