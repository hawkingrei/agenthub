//! A daemon-owned MCP session. RPC adapters provide current execution authority and task ownership.

mod batch;
mod lifecycle;
mod listen;
mod read;
mod subscription;
pub use listen::PreparedProxyListener;
pub use subscription::PreparedProxySubscription;

use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use agenthub_agent_domain::loop_runtime::LoopReservation;
use serde_json::{Value, json};
use tokio::sync::{
    Mutex, OwnedMutexGuard, OwnedRwLockReadGuard, OwnedSemaphorePermit, RwLock, Semaphore, mpsc,
    watch,
};

use crate::{
    MAX_MESSAGE_BYTES, McpTransportError,
    access::McpAccessPolicy,
    budget::{ByteBudget, ByteLease, McpProxyBudget, json_bytes},
    http::{HttpContext, PreparedHttpRequest},
    journal::JournaledMcpClient,
    policy::{
        McpBinding, McpCallContext, McpPolicyError, McpToolCatalog, PreparedTaskCancellation,
        PreparedTaskLookup, PreparedTaskUpdate, PreparedToolCall,
    },
    protocol::{MessageKind, correlation_id, message_kind},
    session::McpProtocolSession,
};

const CALLBACK_SLOTS: u32 = 8;

type BindArguments = dyn Fn(&str, &Value, Value) -> Result<Value, McpPolicyError> + Send + Sync;

/// Built from trusted integration configuration, never from provider-supplied endpoint data.
pub struct McpProxyBinding {
    policy: McpBinding,
    access: McpAccessPolicy,
    bind_arguments: Arc<BindArguments>,
    revoked: AtomicBool,
}

impl McpProxyBinding {
    pub fn server_id(&self) -> &str {
        self.policy.server_id()
    }

    pub fn new(
        policy: McpBinding,
        access: McpAccessPolicy,
        bind_arguments: Arc<BindArguments>,
    ) -> Self {
        Self {
            policy,
            access,
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
    bytes: ByteLease,
}

impl McpProxyFrame {
    pub fn new(
        message_json: String,
        finished: bool,
        budget: &ByteBudget,
    ) -> Result<Self, McpTransportError> {
        if message_json.len() > MAX_MESSAGE_BYTES {
            return Err(McpTransportError::MessageTooLarge);
        }
        let bytes = budget.acquire(message_json.len())?;
        Ok(Self {
            message_json,
            finished,
            bytes,
        })
    }

    pub fn into_parts(self) -> (String, bool, ByteLease) {
        (self.message_json, self.finished, self.bytes)
    }
}

pub struct McpProxySession {
    id: String,
    binding: Arc<McpProxyBinding>,
    task_observer: Option<crate::journal::JournaledTaskObserver>,
    protocol: Mutex<McpProtocolSession>,
    lifecycle_gate: Arc<Mutex<()>>,
    upstream_context: Mutex<Option<HttpContext>>,
    discovery: Mutex<Discovery>,
    request_ids: Mutex<HashSet<[u8; 32]>>,
    callbacks: Mutex<HashSet<[u8; 32]>>,
    read_continuations: Arc<read::ReadContinuations>,
    budget: Arc<McpProxyBudget>,
    tool_slots: Arc<Semaphore>,
    control_slots: Arc<Semaphore>,
    callback_slots: Arc<Semaphore>,
    listener_slot: Arc<Semaphore>,
    subscription_slots: Arc<Semaphore>,
    subscriptions: std::sync::Mutex<HashMap<[u8; 32], watch::Sender<bool>>>,
    exchanges: Arc<RwLock<()>>,
    close_gate: Mutex<()>,
    close_sent: AtomicBool,
    closed_signal: watch::Sender<bool>,
    listen_ready: AtomicBool,
    closed: AtomicBool,
}

#[derive(Default)]
struct Discovery {
    generation: u64,
    tools: Vec<Value>,
    next_cursor: Option<String>,
    catalog: Option<McpToolCatalog>,
    bytes: Option<ByteLease>,
}

pub struct PreparedProxyExchange {
    session: Arc<McpProxySession>,
    message: Value,
    context: HttpContext,
    kind: Exchange,
    handshake: bool,
    _slots: Vec<OwnedSemaphorePermit>,
    _lifecycle: Option<OwnedMutexGuard<()>>,
    _workspace: ByteLease,
    _exchange: OwnedRwLockReadGuard<()>,
}

enum Exchange {
    Batch(Box<batch::PreparedProxyBatch>),
    Tool(Box<PreparedToolCall>),
    Task(Box<PreparedTaskLookup>),
    TaskCancellation(Box<PreparedTaskCancellation>),
    TaskUpdate(Box<PreparedTaskUpdate>),
    Control(PreparedHttpRequest),
    Read {
        request: PreparedHttpRequest,
        round: read::ReadRound,
    },
    CancelSubscription(Option<watch::Sender<bool>>),
    Discovery {
        request: PreparedHttpRequest,
        generation: u64,
        cursor: Option<String>,
    },
}

impl McpProxySession {
    pub fn new(
        id: String,
        binding: Arc<McpProxyBinding>,
        budget: Arc<McpProxyBudget>,
    ) -> Arc<Self> {
        Self::with_task_observer(id, binding, budget, None)
    }

    pub fn with_task_observer(
        id: String,
        binding: Arc<McpProxyBinding>,
        budget: Arc<McpProxyBudget>,
        task_observer: Option<crate::journal::JournaledTaskObserver>,
    ) -> Arc<Self> {
        Arc::new(Self {
            id,
            binding,
            task_observer,
            protocol: Mutex::new(McpProtocolSession::new(budget.retained.clone())),
            lifecycle_gate: Arc::new(Mutex::new(())),
            upstream_context: Mutex::new(None),
            discovery: Mutex::new(Discovery::default()),
            request_ids: Mutex::new(HashSet::new()),
            callbacks: Mutex::new(HashSet::new()),
            read_continuations: Arc::new(read::ReadContinuations::default()),
            budget,
            tool_slots: Arc::new(Semaphore::new(4)),
            control_slots: Arc::new(Semaphore::new(8)),
            callback_slots: Arc::new(Semaphore::new(CALLBACK_SLOTS as usize)),
            listener_slot: Arc::new(Semaphore::new(1)),
            subscription_slots: Arc::new(Semaphore::new(8)),
            subscriptions: std::sync::Mutex::new(HashMap::new()),
            exchanges: Arc::new(RwLock::new(())),
            close_gate: Mutex::new(()),
            close_sent: AtomicBool::new(false),
            closed_signal: watch::channel(false).0,
            listen_ready: AtomicBool::new(false),
            closed: AtomicBool::new(false),
        })
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.closed_signal.send_replace(true);
    }

    fn bound_task_observer(
        &self,
        context: &HttpContext,
    ) -> Result<Option<crate::journal::BoundTaskObserver>, McpTransportError> {
        self.task_observer
            .as_ref()
            .map(|observer| observer.bind(self.binding.policy.observation_binding(), context))
            .transpose()
            .map(Option::flatten)
    }

    pub fn is_active(&self) -> bool {
        !self.closed.load(Ordering::Acquire) && !self.binding.revoked.load(Ordering::Acquire)
    }

    pub async fn can_listen(&self) -> bool {
        self.is_active() && self.listen_ready.load(Ordering::Acquire)
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
        json_bytes(&message)?;
        let message_kind = message_kind(&message)?;
        if message_kind == MessageKind::Batch {
            return self.prepare_batch(executor, message).await;
        }
        let exchange = self.exchanges.clone().read_owned().await;
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let is_response = message_kind == MessageKind::Response;
        let workspace = self.budget.workspace(is_response)?;
        let is_tool = method == "tools/call";
        let slot = if is_response {
            &self.callback_slots
        } else if is_tool {
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
        if !is_response && !supported_method(&method) {
            return Err(McpPolicyError::Call);
        }
        self.binding.access.authorize_request(&message)?;
        if message_kind == MessageKind::Request {
            let mut ids = self.request_ids.lock().await;
            if ids.len() >= 4096 || !ids.insert(correlation_id(&message["id"])) {
                return Err(McpPolicyError::Call);
            }
        }
        let mut protocol = self.protocol.lock().await;
        let handshake = method == "initialize"
            || method == "notifications/initialized" && protocol.awaiting_initialized();
        let mut candidate = protocol.clone();
        let mut context = candidate.begin(&message)?;
        read::validate_request(&message, context.version)?;
        *protocol = candidate;
        drop(protocol);
        if method == "initialize" {
            self.listen_ready.store(false, Ordering::Release);
        }
        if is_response {
            let key = correlation_id(&message["id"]);
            if !self.callbacks.lock().await.remove(&key) {
                return Err(McpPolicyError::Call);
            }
            if let Some(current) = self.upstream_context.lock().await.as_ref() {
                context = current.clone();
            }
        }
        if (is_tool && message.pointer("/params/task").is_some() || method.starts_with("tasks/"))
            && context.version.uses_initialization()
            && !self
                .protocol
                .lock()
                .await
                .server_capabilities()
                .and_then(|caps| caps.pointer("/tasks/requests/tools/call"))
                .is_some_and(Value::is_object)
        {
            return Err(McpPolicyError::Call);
        }
        if method == "tasks/cancel"
            && context.version.uses_initialization()
            && !self
                .protocol
                .lock()
                .await
                .server_capabilities()
                .and_then(|caps| caps.pointer("/tasks/cancel"))
                .is_some_and(Value::is_object)
        {
            return Err(McpPolicyError::Call);
        }
        let kind = if method == "notifications/cancelled"
            && context.version == crate::protocol::ProtocolVersion::July2026
        {
            let id = message
                .pointer("/params/requestId")
                .filter(|id| id.is_string() || id.is_i64() || id.is_u64())
                .ok_or(McpPolicyError::Call)?;
            Exchange::CancelSubscription(
                self.subscriptions
                    .lock()
                    .unwrap()
                    .get(&correlation_id(id))
                    .cloned(),
            )
        } else if matches!(
            method.as_str(),
            "tasks/get" | "tasks/result" | "tasks/cancel" | "tasks/update"
        ) {
            let discovery = self.discovery.lock().await;
            let catalog = discovery
                .catalog
                .as_ref()
                .ok_or(McpPolicyError::ToolNotAvailable)?;
            let call_context = McpCallContext {
                executor,
                proxy_session_id: &self.id,
                http: &context,
            };
            if method == "tasks/update" {
                Exchange::TaskUpdate(Box::new(self.binding.policy.prepare_task_update(
                    catalog,
                    &call_context,
                    message.clone(),
                )?))
            } else if method == "tasks/cancel" {
                Exchange::TaskCancellation(Box::new(
                    self.binding.policy.prepare_task_cancellation(
                        catalog,
                        &call_context,
                        message.clone(),
                    )?,
                ))
            } else {
                Exchange::Task(Box::new(self.binding.policy.prepare_task_lookup(
                    catalog,
                    &call_context,
                    message.clone(),
                )?))
            }
        } else if is_tool {
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
            } else if context.version == crate::protocol::ProtocolVersion::July2026
                && matches!(method.as_str(), "prompts/get" | "resources/read")
            {
                Exchange::Read {
                    request,
                    round: self
                        .read_continuations
                        .prepare(&message, &self.budget.retained)?,
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
            handshake,
            _slots: vec![slot],
            _lifecycle: lifecycle,
            _workspace: workspace,
            _exchange: exchange,
        })
    }

    async fn observe(&self, message: &Value) -> Result<(), McpTransportError> {
        let members = message
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(std::slice::from_ref(message));
        for member in members {
            self.binding.access.authorize_server_message(member)?;
        }
        let keys: Vec<_> = members
            .iter()
            .filter(|member| message_kind(member).ok() == Some(MessageKind::Request))
            .map(|member| correlation_id(&member["id"]))
            .collect();
        if !keys.is_empty() {
            let mut callbacks = self.callbacks.lock().await;
            if callbacks.len() + keys.len() > 64
                || keys.iter().collect::<HashSet<_>>().len() != keys.len()
                || keys.iter().any(|key| callbacks.contains(key))
            {
                return Err(McpTransportError::InvalidResponse);
            }
            callbacks.extend(keys);
        }
        if members
            .iter()
            .any(|member| member["method"] == "notifications/tools/list_changed")
        {
            let mut discovery = self.discovery.lock().await;
            discovery.generation = discovery.generation.saturating_add(1);
            discovery.catalog = None;
            discovery.next_cursor = None;
            discovery.tools.clear();
            discovery.bytes = None;
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
        let mut advertised = page.advertised_tools();
        self.binding.access.filter_tools(&mut advertised)?;
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
        // Both the page accumulator and the admission catalog own the declarations. Working
        // copies during refresh belong to this exchange's separately reserved workspace.
        let retained_bytes = 2 * json_bytes(&Value::Array(all_tools.clone()))?;
        if let Some(bytes) = &mut discovery.bytes {
            bytes.resize(retained_bytes)?;
        } else {
            discovery.bytes = Some(self.budget.retained.acquire(retained_bytes)?);
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
        if !session.is_active() {
            session.close();
            return;
        }
        let journal = match session.bound_task_observer(&self.context) {
            Ok(observer) => journal.observing(observer),
            Err(_) => {
                session.close();
                return;
            }
        };
        let request_id = (message_kind(&self.message).ok() == Some(MessageKind::Request))
            .then(|| self.message["id"].clone());
        let method = self.message["method"].as_str().unwrap_or("").to_owned();
        let handshake = self.handshake;
        let initialized = handshake && method != "initialize";
        let mut sink = Sink {
            output,
            session: session.clone(),
            lost: false,
        };
        let outcome: Result<Option<Value>, String> = match self.kind {
            Exchange::CancelSubscription(cancel) => {
                if let Some(cancel) = cancel {
                    cancel.send_replace(true);
                }
                Ok(None)
            }
            Exchange::Batch(batch) => {
                batch
                    .run(session.clone(), journal, &mut sink, &self.context)
                    .await
            }
            kind @ (Exchange::Tool(_)
            | Exchange::Task(_)
            | Exchange::TaskCancellation(_)
            | Exchange::TaskUpdate(_)) => {
                let (events, mut receiver) = mpsc::channel(8);
                let operation = async {
                    match kind {
                        Exchange::Tool(call) => journal.run(*call, events).await,
                        Exchange::Task(call) => journal.run_task_lookup(*call, events).await,
                        Exchange::TaskUpdate(call) => journal.run_task_update(*call, events).await,
                        Exchange::TaskCancellation(call) => {
                            journal.run_task_cancellation(*call, events).await
                        }
                        _ => unreachable!(),
                    }
                };
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
                                    let (event, bytes) = event.into_parts();
                                    if let Some(message) = event.message {
                                        if session.observe(&message).await.is_err() { sink.fail(); }
                                        sink.emit(Some(message), false, Some(bytes));
                                    }
                                }
                                if result.event_delivery_lost { sink.fail(); }
                                if session.observe(&result.response).await.is_err() { sink.fail(); }
                                if self.context.session_id.is_some() && result.http_status == 404 {
                                    session.close();
                                    sink.emit(Some(result.response), false, None);
                                    sink.fail();
                                    Ok(None)
                                } else {
                                    Ok(Some(result.response))
                                }
                            }
                            Err(error) => {
                                if self.context.session_id.is_some() && matches!(error, crate::journal::McpCallError::Transport(McpTransportError::HttpStatus(404))) { sink.fail(); }
                                Err(error.to_string())
                            },
                        },
                        event = receiver.recv(), if events_open => {
                            if let Some(event) = event {
                                let (event, bytes) = event.into_parts();
                                if let Some(message) = event.message {
                                    if session.observe(&message).await.is_err() { sink.fail(); }
                                    sink.emit(Some(message), false, Some(bytes));
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
                &self.message,
                handshake,
                ControlResponse::default(),
                &mut sink,
            )
            .await
            .map_err(|error| error.to_string()),
            Exchange::Read { request, round } => run_control(
                &session,
                request,
                &self.context,
                &self.message,
                handshake,
                ControlResponse {
                    read: Some(round),
                    ..ControlResponse::default()
                },
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
                &self.message,
                handshake,
                ControlResponse {
                    discovery: Some((generation, cursor)),
                    ..ControlResponse::default()
                },
                &mut sink,
            )
            .await
            .map_err(|error| error.to_string()),
        };
        let cleanup_failed = if handshake
            && (outcome.is_err() || session.protocol.lock().await.http_context().is_none())
        {
            session.retire_failed_initialization().await.is_err()
        } else {
            false
        };
        if cleanup_failed {
            session.close();
        }
        if initialized {
            session.listen_ready.store(
                session.protocol.lock().await.ready_context().is_some(),
                Ordering::Release,
            );
        }
        match outcome {
            Ok(response) => {
                sink.emit(response, !cleanup_failed, None);
            }
            Err(error) => {
                if request_id.is_some() {
                    sink.emit(Some(json!({"jsonrpc":"2.0","id":request_id,"error":{"code":-32000,"message":error}})), !cleanup_failed, None);
                } else {
                    sink.fail();
                }
            }
        }
        if cleanup_failed {
            sink.fail();
        }
    }
}

fn supported_method(method: &str) -> bool {
    matches!(
        method,
        "initialize"
            | "server/discover"
            | "ping"
            | "tools/list"
            | "tools/call"
            | "tasks/get"
            | "tasks/result"
            | "tasks/cancel"
            | "tasks/update"
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
    fn emit(&mut self, message: Option<Value>, finished: bool, bytes: Option<ByteLease>) {
        if self.lost {
            return;
        }
        let message_json = message
            .map(|message| message.to_string())
            .unwrap_or_default();
        let frame = if let Some(mut bytes) = bytes {
            bytes.resize(message_json.len()).map(|()| McpProxyFrame {
                message_json,
                finished,
                bytes,
            })
        } else {
            McpProxyFrame::new(message_json, finished, &self.session.budget.delivery)
        };
        if !frame.is_ok_and(|frame| self.output.try_send(frame).is_ok()) {
            self.fail();
        }
    }
}

#[derive(Default)]
struct ControlResponse {
    discovery: Option<(u64, Option<String>)>,
    read: Option<read::ReadRound>,
}

async fn run_control(
    session: &McpProxySession,
    request: PreparedHttpRequest,
    context: &HttpContext,
    request_message: &Value,
    handshake: bool,
    mut projection: ControlResponse,
    sink: &mut Sink,
) -> Result<Option<Value>, McpTransportError> {
    let method = request_message["method"].as_str().unwrap_or("");
    let request_id =
        (message_kind(request_message)? == MessageKind::Request).then(|| &request_message["id"]);
    let mut task_events = crate::journal::TaskEventDrain::new(
        session.bound_task_observer(context)?,
        session.binding.policy.transport.timeout(),
    );
    if let Some(round) = projection.read.as_mut() {
        round.begin();
    }
    let mut exchange = match session
        .binding
        .policy
        .transport
        .send_resumable(request)
        .await
    {
        Ok(exchange) => exchange,
        Err(error) => {
            if context.session_id.is_some() && error == McpTransportError::HttpStatus(404) {
                sink.fail();
            }
            return Err(error);
        }
    };
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
        if handshake && method == "notifications/initialized" && exchange.status_code() >= 400 {
            session.protocol.lock().await.initialization_failed();
        }
        let members = match &mut message {
            Value::Array(members) => members.as_mut_slice(),
            member => std::slice::from_mut(member),
        };
        let mut terminal = false;
        let mut terminal_index = 0;
        for (index, member) in members.iter_mut().enumerate() {
            let is_error = exchange.status_code() >= 400
                && member.get("error").is_some()
                && member.get("id").is_none_or(Value::is_null);
            if member.get("method").is_none()
                && (request_id.is_some() && member.get("id") == request_id || is_error)
            {
                if terminal {
                    return Err(McpTransportError::InvalidResponse);
                }
                terminal = true;
                terminal_index = index;
                read::validate_control_response(request_message, member, context.version)?;
                session.binding.access.project_response(method, member)?;
                if method == "initialize" {
                    if is_error {
                        session.protocol.lock().await.initialization_failed();
                    } else {
                        let mut protocol = session.protocol.lock().await;
                        if protocol.accept_initialize_response(member, http_session.clone())? {
                            *session.upstream_context.lock().await = protocol.http_context();
                        }
                    }
                }
                if let Some((generation, cursor)) = projection.discovery.as_ref() {
                    session
                        .apply_discovery(member, context, *generation, cursor)
                        .await?;
                }
            }
        }
        match task_events.accept(&message).await {
            crate::journal::TaskEventDisposition::Forward => {}
            crate::journal::TaskEventDisposition::Held => continue,
            crate::journal::TaskEventDisposition::Rejected => {
                return Err(McpTransportError::InvalidResponse);
            }
        }
        session.observe(&message).await?;
        if context.session_id.is_some() && exchange.status_code() == 404 {
            session.close();
            sink.emit(Some(message), false, None);
            sink.fail();
            return Ok(None);
        }
        if terminal {
            for pending in task_events.settle().await {
                let (pending, _) = pending.into_parts();
                session.observe(&pending).await?;
                sink.emit(Some(pending), false, None);
            }
            if task_events.lost {
                return Err(McpTransportError::InvalidResponse);
            }
            if let Some(round) = projection.read.as_mut() {
                let response = message
                    .as_array()
                    .map(|members| &members[terminal_index])
                    .unwrap_or(&message);
                round.finish(response)?;
            }
            return Ok(Some(message));
        }
        sink.emit(Some(message), false, None);
    }
    if request_id.is_some() {
        Err(McpTransportError::Disconnected)
    } else {
        for pending in task_events.settle().await {
            let (pending, _) = pending.into_parts();
            session.observe(&pending).await?;
            sink.emit(Some(pending), false, None);
        }
        if task_events.lost {
            return Err(McpTransportError::InvalidResponse);
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests;
