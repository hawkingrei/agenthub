//! A daemon-owned MCP session. RPC adapters provide current execution authority and task ownership.

use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use agenthub_agent_domain::loop_runtime::LoopReservation;
use serde_json::{Value, json};
use tokio::sync::{Mutex, OwnedMutexGuard, OwnedSemaphorePermit, Semaphore, mpsc};

use crate::{
    MAX_MESSAGE_BYTES, McpTransportError,
    http::{HttpContext, PreparedHttpRequest},
    journal::JournaledMcpClient,
    policy::{McpBinding, McpCallContext, McpPolicyError, McpToolCatalog, PreparedToolCall},
    protocol::{MessageKind, message_kind},
    session::McpProtocolSession,
};

type BindArguments = dyn Fn(&str, &Value, Value) -> Result<Value, McpPolicyError> + Send + Sync;

/// Built from trusted integration configuration, never from provider-supplied endpoint data.
pub struct McpProxyBinding {
    policy: McpBinding,
    bind_arguments: Arc<BindArguments>,
    revoked: AtomicBool,
}

impl McpProxyBinding {
    pub fn server_id(&self) -> &str {
        self.policy.server_id()
    }

    pub fn new(policy: McpBinding, bind_arguments: Arc<BindArguments>) -> Self {
        Self {
            policy,
            bind_arguments,
            revoked: AtomicBool::new(false),
        }
    }

    /// In-flight operations retain their factual outcome; future requests lose authority.
    pub fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
    }
}

/// The RPC adapter serializes only message_json and finished, never session/binding material.
pub struct McpProxyFrame {
    pub message_json: String,
    pub finished: bool,
}

pub struct McpProxySession {
    id: String,
    binding: Arc<McpProxyBinding>,
    protocol: Mutex<McpProtocolSession>,
    lifecycle_gate: Arc<Mutex<()>>,
    upstream_context: Mutex<Option<HttpContext>>,
    discovery: Mutex<Discovery>,
    request_ids: Mutex<HashSet<String>>,
    callbacks: Mutex<HashSet<String>>,
    tool_slots: Arc<Semaphore>,
    control_slots: Arc<Semaphore>,
    closed: AtomicBool,
}

#[derive(Default)]
struct Discovery {
    generation: u64,
    tools: Vec<Value>,
    next_cursor: Option<String>,
    catalog: Option<McpToolCatalog>,
}

pub struct PreparedProxyExchange {
    session: Arc<McpProxySession>,
    message: Value,
    context: HttpContext,
    kind: Exchange,
    _slot: OwnedSemaphorePermit,
    _lifecycle: Option<OwnedMutexGuard<()>>,
}

enum Exchange {
    Tool(Box<PreparedToolCall>),
    Control(PreparedHttpRequest),
    Discovery {
        request: PreparedHttpRequest,
        generation: u64,
        cursor: Option<String>,
    },
}

impl McpProxySession {
    pub fn new(id: String, binding: Arc<McpProxyBinding>) -> Arc<Self> {
        Arc::new(Self {
            id,
            binding,
            protocol: Mutex::new(McpProtocolSession::default()),
            lifecycle_gate: Arc::new(Mutex::new(())),
            upstream_context: Mutex::new(None),
            discovery: Mutex::new(Discovery::default()),
            request_ids: Mutex::new(HashSet::new()),
            callbacks: Mutex::new(HashSet::new()),
            tool_slots: Arc::new(Semaphore::new(4)),
            control_slots: Arc::new(Semaphore::new(8)),
            closed: AtomicBool::new(false),
        })
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }

    pub fn is_active(&self) -> bool {
        !self.closed.load(Ordering::Acquire) && !self.binding.revoked.load(Ordering::Acquire)
    }

    /// Preparation is serialized through lifecycle delivery. Callback responses bypass that gate
    /// so an upstream initialize request can ask the client for roots/sampling without deadlocking.
    pub async fn prepare(
        self: &Arc<Self>,
        executor: &LoopReservation,
        message: Value,
    ) -> Result<PreparedProxyExchange, McpPolicyError> {
        if !self.is_active() {
            return Err(McpPolicyError::Scope);
        }
        let message_kind = message_kind(&message)?;
        if message_kind == MessageKind::Batch {
            return Err(McpPolicyError::Call);
        }
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let is_response = message_kind == MessageKind::Response;
        let is_tool = method == "tools/call";
        let slot = if is_tool {
            &self.tool_slots
        } else {
            &self.control_slots
        }
        .clone()
        .try_acquire_owned()
        .map_err(|_| McpPolicyError::Call)?;
        let lifecycle = if is_response {
            None
        } else {
            Some(self.lifecycle_gate.clone().lock_owned().await)
        };
        if !self.is_active() {
            return Err(McpPolicyError::Scope);
        }
        if !is_response
            && !matches!(
                method.as_str(),
                "initialize"
                    | "server/discover"
                    | "ping"
                    | "tools/list"
                    | "tools/call"
                    | "notifications/initialized"
                    | "notifications/cancelled"
                    | "notifications/progress"
                    | "notifications/roots/list_changed"
                    | "resources/list"
                    | "resources/templates/list"
                    | "resources/read"
                    | "resources/subscribe"
                    | "resources/unsubscribe"
                    | "prompts/list"
                    | "prompts/get"
                    | "completion/complete"
                    | "logging/setLevel"
            )
        {
            return Err(McpPolicyError::Call);
        }
        if message_kind == MessageKind::Request {
            let mut ids = self.request_ids.lock().await;
            if ids.len() >= 4096 || !ids.insert(message["id"].to_string()) {
                return Err(McpPolicyError::Call);
            }
        }
        let mut context = self.protocol.lock().await.begin(&message)?;
        if is_response {
            let key = message["id"].to_string();
            if !self.callbacks.lock().await.remove(&key) {
                return Err(McpPolicyError::Call);
            }
            if let Some(current) = self.upstream_context.lock().await.as_ref() {
                context = current.clone();
            }
        }
        let kind = if is_tool {
            let discovery = self.discovery.lock().await;
            let catalog = discovery
                .catalog
                .as_ref()
                .ok_or(McpPolicyError::ToolNotAvailable)?;
            let name = message["params"]["name"]
                .as_str()
                .ok_or(McpPolicyError::Call)?;
            let call = self.binding.policy.prepare_call(
                catalog,
                &McpCallContext {
                    executor,
                    proxy_session_id: &self.id,
                    http: &context,
                },
                message.clone(),
                |schema, arguments| (self.binding.bind_arguments)(name, schema, arguments),
            )?;
            Exchange::Tool(Box::new(call))
        } else {
            let request = self
                .binding
                .policy
                .transport
                .prepare_post(&context, &message, None)?;
            if method == "tools/list" {
                let cursor = message
                    .pointer("/params/cursor")
                    .map(|value| {
                        value
                            .as_str()
                            .map(str::to_owned)
                            .ok_or(McpPolicyError::Call)
                    })
                    .transpose()?;
                let mut discovery = self.discovery.lock().await;
                if cursor.is_none() {
                    discovery.generation = discovery
                        .generation
                        .checked_add(1)
                        .ok_or(McpPolicyError::Catalog)?;
                } else if discovery.next_cursor != cursor {
                    return Err(McpPolicyError::Catalog);
                }
                Exchange::Discovery {
                    request,
                    generation: discovery.generation,
                    cursor,
                }
            } else {
                Exchange::Control(request)
            }
        };
        let lifecycle = if matches!(method.as_str(), "initialize" | "notifications/initialized") {
            lifecycle
        } else {
            None
        };
        Ok(PreparedProxyExchange {
            session: self.clone(),
            message,
            context,
            kind,
            _slot: slot,
            _lifecycle: lifecycle,
        })
    }

    async fn observe(&self, message: &Value) -> Result<(), McpTransportError> {
        if message_kind(message)? == MessageKind::Request {
            let mut callbacks = self.callbacks.lock().await;
            if callbacks.len() >= 64 || !callbacks.insert(message["id"].to_string()) {
                return Err(McpTransportError::InvalidResponse);
            }
        }
        if message["method"] == "notifications/tools/list_changed" {
            let mut discovery = self.discovery.lock().await;
            discovery.generation = discovery.generation.saturating_add(1);
            discovery.catalog = None;
            discovery.next_cursor = None;
        }
        Ok(())
    }

    async fn apply_discovery(
        &self,
        response: &mut Value,
        context: &HttpContext,
        generation: u64,
        cursor: &Option<String>,
    ) -> Result<(), McpTransportError> {
        if response.get("error").is_some() {
            return Ok(());
        }
        let tools = response
            .pointer("/result/tools")
            .ok_or(McpTransportError::InvalidResponse)?;
        let page = McpToolCatalog::from_tools(tools, context.version)
            .map_err(|_| McpTransportError::InvalidResponse)?;
        let advertised = page.advertised_tools();
        response["result"]["tools"] = advertised.clone();
        let mut discovery = self.discovery.lock().await;
        if generation != discovery.generation
            || (cursor.is_some() && cursor != &discovery.next_cursor)
        {
            // Preserve this request's response, but never let an older refresh or page replace
            // the catalog that now authorizes calls.
            return Ok(());
        }
        let mut all_tools = if cursor.is_some() {
            discovery.tools.clone()
        } else {
            Vec::new()
        };
        all_tools.extend(advertised.as_array().unwrap().iter().cloned());
        let catalog = McpToolCatalog::from_tools(&Value::Array(all_tools.clone()), context.version)
            .map_err(|_| McpTransportError::InvalidResponse)?;
        let next_cursor = response
            .pointer("/result/nextCursor")
            .map(|cursor| {
                cursor
                    .as_str()
                    .filter(|cursor| cursor.len() <= 4096)
                    .map(str::to_owned)
                    .ok_or(McpTransportError::InvalidResponse)
            })
            .transpose()?;
        if next_cursor.is_some() && &next_cursor == cursor {
            return Err(McpTransportError::InvalidResponse);
        }
        discovery.tools = all_tools;
        discovery.catalog = Some(catalog);
        discovery.next_cursor = next_cursor;
        Ok(())
    }
}

impl PreparedProxyExchange {
    /// Must run in the daemon's task group, with the live executor operation guard held. Receiver
    /// loss closes the provider session but does not cancel the HTTP send or durable completion.
    pub async fn run(self, journal: JournaledMcpClient, output: mpsc::Sender<McpProxyFrame>) {
        let session = self.session.clone();
        let request_id = (message_kind(&self.message).ok() == Some(MessageKind::Request))
            .then(|| self.message["id"].clone());
        let method = self.message["method"].as_str().unwrap_or("").to_owned();
        let mut sink = Sink {
            output,
            session: session.clone(),
            lost: false,
        };
        let outcome: Result<Option<Value>, String> = match self.kind {
            Exchange::Tool(call) => {
                let (events, mut receiver) = mpsc::channel(8);
                let operation = journal.run(*call, events);
                tokio::pin!(operation);
                let mut events_open = true;
                loop {
                    tokio::select! {
                        biased;
                        result = &mut operation => break match result {
                            Ok(result) => {
                                // The terminal result is already durable. Drain earlier queued
                                // events before forwarding it so the provider observes wire order.
                                while let Ok(event) = receiver.try_recv() {
                                    if let Some(message) = event.message {
                                        if session.observe(&message).await.is_err() { sink.fail(); }
                                        sink.emit(Some(message), false);
                                    }
                                }
                                if result.event_delivery_lost { sink.fail(); }
                                Ok(Some(result.response))
                            }
                            Err(error) => Err(error.to_string()),
                        },
                        event = receiver.recv(), if events_open => {
                            if let Some(event) = event {
                                if let Some(message) = event.message {
                                    if session.observe(&message).await.is_err() { sink.fail(); }
                                    sink.emit(Some(message), false);
                                }
                            } else {events_open = false;}
                        }
                    }
                }
            }
            Exchange::Control(request) => run_control(
                &session,
                request,
                &self.context,
                &method,
                request_id.as_ref(),
                None,
                &mut sink,
            )
            .await
            .map_err(|error| error.to_string()),
            Exchange::Discovery {
                request,
                generation,
                cursor,
            } => run_control(
                &session,
                request,
                &self.context,
                &method,
                request_id.as_ref(),
                Some((generation, cursor)),
                &mut sink,
            )
            .await
            .map_err(|error| error.to_string()),
        };
        match outcome {
            Ok(response) => sink.emit(response, true),
            Err(error) => {
                if matches!(method.as_str(), "initialize" | "notifications/initialized") {
                    session.protocol.lock().await.initialization_failed();
                }
                if request_id.is_some() {
                    sink.emit(Some(json!({"jsonrpc":"2.0","id":request_id,"error":{"code":-32000,"message":error}})), true);
                } else {
                    sink.fail();
                }
            }
        }
    }
}

struct Sink {
    output: mpsc::Sender<McpProxyFrame>,
    session: Arc<McpProxySession>,
    lost: bool,
}

impl Sink {
    fn fail(&mut self) {
        self.lost = true;
        self.session.close();
    }
    fn emit(&mut self, message: Option<Value>, finished: bool) {
        if self.lost {
            return;
        }
        let message_json = message
            .map(|message| message.to_string())
            .unwrap_or_default();
        if message_json.len() > MAX_MESSAGE_BYTES
            || self
                .output
                .try_send(McpProxyFrame {
                    message_json,
                    finished,
                })
                .is_err()
        {
            self.fail();
        }
    }
}

async fn run_control(
    session: &McpProxySession,
    request: PreparedHttpRequest,
    context: &HttpContext,
    method: &str,
    request_id: Option<&Value>,
    discovery: Option<(u64, Option<String>)>,
    sink: &mut Sink,
) -> Result<Option<Value>, McpTransportError> {
    let mut exchange = session.binding.policy.transport.send(request).await?;
    let http_session = exchange.session_id();
    if http_session.is_some() {
        *session.upstream_context.lock().await = Some(HttpContext {
            version: context.version,
            session_id: http_session.clone(),
        });
    }
    while let Some(event) = exchange.next_event().await? {
        let Some(mut message) = event.message else {
            continue;
        };
        let is_error = exchange.status_code() >= 400
            && message.get("error").is_some()
            && message.get("id").is_none_or(Value::is_null);
        if message.get("method").is_none()
            && (request_id.is_some() && message.get("id") == request_id || is_error)
        {
            if method == "initialize" {
                if is_error {
                    session.protocol.lock().await.initialization_failed();
                } else {
                    let mut protocol = session.protocol.lock().await;
                    protocol.accept_initialize_response(&message, http_session)?;
                    *session.upstream_context.lock().await = protocol.http_context();
                }
            }
            if let Some((generation, cursor)) = discovery.as_ref() {
                session
                    .apply_discovery(&mut message, context, *generation, cursor)
                    .await?;
            }
            return Ok(Some(message));
        }
        session.observe(&message).await?;
        sink.emit(Some(message), false);
    }
    if request_id.is_some() {
        Err(McpTransportError::Disconnected)
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests;
