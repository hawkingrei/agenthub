use std::{future::Future, time::Duration};

use super::*;
use crate::journal::{TaskEventDisposition, TaskEventDrain};

pub struct PreparedProxyListener {
    session: Arc<McpProxySession>,
    request: PreparedHttpRequest,
    drain: TaskEventDrain,
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
            drain: TaskEventDrain::new(
                self.bound_task_observer(&context)?,
                self.binding.policy.transport.timeout(),
            ),
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
        let mut drain = self.drain;
        let mut changes = drain.changes();
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
                        _ = changes.changed(), if drain.has_pending() => {
                            flush_task_events(&session, &mut drain, &mut sink, &validate).await?;
                        }
                        _ = tick.tick() => {
                            flush_task_events(&session, &mut drain, &mut sink, &validate).await?;
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
                match drain.accept(&message).await {
                    TaskEventDisposition::Forward => {}
                    TaskEventDisposition::Held => continue,
                    TaskEventDisposition::Rejected => {
                        return Err(McpTransportError::InvalidResponse);
                    }
                }
                let _guard = validate().await?;
                if !session.is_active() {
                    return Ok(());
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

async fn flush_task_events<F, Fut, Guard>(
    session: &McpProxySession,
    drain: &mut TaskEventDrain,
    sink: &mut Sink,
    validate: &F,
) -> Result<(), McpTransportError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<Guard, McpTransportError>>,
{
    let ready = drain.flush().await;
    let _guard = validate().await?;
    if !session.is_active() {
        return Err(McpTransportError::Disconnected);
    }
    for message in ready {
        let (message, _) = message.into_parts();
        session.observe(&message).await?;
        sink.emit(Some(message), false, None);
    }
    if drain.lost || sink.lost {
        return Err(McpTransportError::InvalidResponse);
    }
    Ok(())
}
