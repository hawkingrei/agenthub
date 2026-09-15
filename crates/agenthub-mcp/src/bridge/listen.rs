use std::{future::Future, time::Duration};

use super::*;

pub struct PreparedProxyListener {
    session: Arc<McpProxySession>,
    request: PreparedHttpRequest,
    _slot: OwnedSemaphorePermit,
    _workspace: ByteLease,
}

impl McpProxySession {
    pub async fn prepare_listener(
        self: &Arc<Self>,
    ) -> Result<PreparedProxyListener, McpPolicyError> {
        let _lifecycle = self.lifecycle_gate.lock().await;
        if !self.is_active() {
            return Err(McpPolicyError::Scope);
        }
        if !self.listen_ready.load(Ordering::Acquire) {
            return Err(McpPolicyError::Call);
        }
        let context = self
            .protocol
            .lock()
            .await
            .ready_context()
            .ok_or(McpPolicyError::Call)?;
        let slot = self
            .listener_slot
            .clone()
            .try_acquire_owned()
            .map_err(|_| McpPolicyError::Call)?;
        let workspace = self.budget.listener()?;
        let request = self
            .binding
            .policy
            .transport
            .prepare_listen(&context, None)?;
        Ok(PreparedProxyListener {
            session: self.clone(),
            request,
            _slot: slot,
            _workspace: workspace,
        })
    }
}

impl PreparedProxyListener {
    /// Validation returns a short-lived executor guard. Never retain it over idle SSE reads;
    /// cleanup must be able to close this listener while the upstream keeps its socket open.
    pub async fn run<F, Fut, Guard>(self, output: mpsc::Sender<McpProxyFrame>, validate: F)
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<Guard, McpTransportError>>,
    {
        let session = self.session;
        let mut sink = Sink {
            output,
            session: session.clone(),
            lost: false,
        };
        let mut closed = session.closed_signal.subscribe();
        let operation = async {
            let guard = validate().await?;
            if !session.is_active() {
                return Err(McpTransportError::Disconnected);
            }
            let mut exchange = session
                .binding
                .policy
                .transport
                .send_resumable(self.request)
                .await?;
            drop(guard);
            if exchange.status_code() == 405 {
                return Ok(());
            }
            let mut tick = tokio::time::interval(Duration::from_secs(1));
            loop {
                if !session.is_active() {
                    return Ok(());
                }
                let next = exchange.next_event();
                tokio::pin!(next);
                // Keep the read future pinned across authority ticks, preserving parser and
                // reconnect progress even when a read or server retry delay exceeds one second.
                let event = loop {
                    tokio::select! {
                        event = &mut next => break event?,
                        _ = closed.changed() => return Ok(()),
                        _ = sink.output.closed() => return Err(McpTransportError::Disconnected),
                        _ = tick.tick() => {
                            drop(validate().await?);
                            if !session.is_active() { return Ok(()); }
                        }
                    }
                };
                let Some(event) = event else {
                    return Err(McpTransportError::Disconnected);
                };
                let Some(message) = event.message else {
                    continue;
                };
                let _guard = validate().await?;
                if !session.is_active() {
                    return Ok(());
                }
                let members = message
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or(std::slice::from_ref(&message));
                if members
                    .iter()
                    .any(|member| message_kind(member).ok() == Some(MessageKind::Response))
                {
                    return Err(McpTransportError::InvalidResponse);
                }
                session.observe(&message).await?;
                sink.emit(Some(message), false, None);
                if sink.lost {
                    return Err(McpTransportError::Disconnected);
                }
            }
        };
        if operation.await.is_ok() {
            sink.emit(None, true, None);
        } else {
            sink.fail();
        }
    }
}
