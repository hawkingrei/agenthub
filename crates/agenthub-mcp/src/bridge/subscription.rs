use std::{future::Future, time::Duration};

use agenthub_agent_domain::mcp_operations::{McpTaskReceipt, McpTaskVersion};
use agenthub_db::mcp_operations::McpTaskNotificationPermit;

use super::*;
use crate::{
    protocol::ProtocolVersion,
    task::{modern_capability, task_digest},
};

mod filter;
use filter::{Event, Filter, SubscriptionState};

pub struct PreparedProxySubscription {
    session: Arc<McpProxySession>,
    journal: JournaledMcpClient,
    request: PreparedHttpRequest,
    state: SubscriptionState,
    tasks: HashMap<[u8; 32], McpTaskNotificationPermit>,
    cancelled: watch::Receiver<bool>,
    _registration: Registration,
    _slot: OwnedSemaphorePermit,
    _workspace: ByteLease,
}

struct Registration {
    session: Arc<McpProxySession>,
    request_id: [u8; 32],
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.session
            .subscriptions
            .lock()
            .unwrap()
            .remove(&self.request_id);
    }
}

impl McpProxySession {
    pub async fn prepare_subscription(
        self: &Arc<Self>,
        executor: &LoopReservation,
        journal: JournaledMcpClient,
        message: Value,
    ) -> Result<PreparedProxySubscription, McpPolicyError> {
        let _lifecycle = self.lifecycle_gate.lock().await;
        if !self.is_active() {
            return Err(McpPolicyError::Scope);
        }
        json_bytes(&message)?;
        self.binding.access.authorize_request(&message)?;
        if message_kind(&message)? != MessageKind::Request
            || message["method"] != "subscriptions/listen"
        {
            return Err(McpPolicyError::Call);
        }
        let context = self.protocol.lock().await.begin(&message)?;
        if context.version != ProtocolVersion::July2026 {
            return Err(McpPolicyError::Call);
        }
        let slot = self
            .subscription_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| McpPolicyError::Call)?;
        let workspace = self.budget.listener()?;
        let filter = Filter::parse(&message["params"]["notifications"])?;
        let request = self
            .binding
            .policy
            .transport
            .prepare_subscription(&context, &message)?;
        let mut tasks = HashMap::new();
        if !filter.task_ids.is_empty() {
            if !modern_capability(message["params"].as_object().ok_or(McpPolicyError::Call)?) {
                return Err(McpPolicyError::Call);
            }
            let discovery = self.discovery.lock().await;
            let catalog = discovery
                .catalog
                .as_ref()
                .ok_or(McpPolicyError::ToolNotAvailable)?;
            let authority = self.binding.policy.task_authority(catalog);
            for id in &filter.task_ids {
                let value = json!(id);
                let receipt = McpTaskReceipt {
                    task_digest: task_digest(&value)?,
                    version: McpTaskVersion::July2026,
                    session_digest: None,
                };
                let permit = journal
                    .authorize_task_notifications(executor, &authority, &receipt)
                    .await
                    .map_err(|_| McpPolicyError::Scope)?;
                tasks.insert(correlation_id(&value), permit);
            }
        }
        let request_id = correlation_id(&message["id"]);
        let mut ids = self.request_ids.lock().await;
        if ids.len() >= 4096 || !ids.insert(request_id) {
            return Err(McpPolicyError::Call);
        }
        let (cancel, cancelled) = watch::channel(false);
        self.subscriptions
            .lock()
            .unwrap()
            .insert(request_id, cancel);
        Ok(PreparedProxySubscription {
            session: self.clone(),
            journal,
            request,
            tasks,
            cancelled,
            state: SubscriptionState::new(message["id"].clone(), filter),
            _registration: Registration {
                session: self.clone(),
                request_id,
            },
            _slot: slot,
            _workspace: workspace,
        })
    }
}

impl PreparedProxySubscription {
    /// The caller owns this stream in the daemon task group. Idle reads hold no executor or
    /// ordinary-exchange guard; cancellation and cleanup must remain able to close the socket.
    pub async fn run<F, Fut, Guard>(mut self, output: mpsc::Sender<McpProxyFrame>, validate: F)
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<Guard, McpTransportError>>,
    {
        let session = self.session.clone();
        let mut sink = Sink {
            output,
            session: session.clone(),
            lost: false,
        };
        let mut closed = session.closed_signal.subscribe();
        let operation = async {
            let guard = validate().await?;
            if !session.is_active() || *self.cancelled.borrow() {
                return Ok(());
            }
            let mut exchange = session
                .binding
                .policy
                .transport
                .send_resumable(self.request)
                .await?;
            drop(guard);
            let status = exchange.status_code();
            let mut tick = tokio::time::interval(Duration::from_secs(1));
            loop {
                let next = exchange.next_event();
                tokio::pin!(next);
                let event = loop {
                    tokio::select! {
                        event = &mut next => break event?,
                        _ = closed.changed() => return Ok(()),
                        _ = self.cancelled.changed() => return Ok(()),
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
                let event = self.state.observe(&message, status)?;
                if matches!(event, Event::Task) {
                    let permit = self
                        .tasks
                        .get(&correlation_id(&message["params"]["taskId"]))
                        .ok_or(McpTransportError::InvalidResponse)?;
                    // Once received, facts survive executor revocation; delivery still requires
                    // current authority. The immutable permit cannot authorize another send.
                    self.journal
                        .record_task_notification(permit, &message)
                        .await
                        .map_err(|_| McpTransportError::InvalidResponse)?;
                }
                let _guard = validate().await?;
                if !session.is_active() {
                    return Ok(());
                }
                session.observe(&message).await?;
                let complete = matches!(event, Event::Complete);
                sink.emit(Some(message), false, None);
                if sink.lost {
                    return Err(McpTransportError::Disconnected);
                }
                if complete {
                    return Ok(());
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
