use std::collections::HashMap;

use super::*;
use crate::{budget::Budgeted, http::HttpEvent, policy::PreparedBatchCall};

type Discoveries = HashMap<[u8; 32], (u64, Option<String>)>;
type Methods = HashMap<[u8; 32], String>;

pub(super) struct PreparedProxyBatch {
    call: PreparedBatchCall,
    discoveries: Discoveries,
    methods: Methods,
    initialized: bool,
}

impl McpProxySession {
    pub(super) async fn prepare_batch(
        self: &Arc<Self>,
        executor: &LoopReservation,
        message: Value,
    ) -> Result<PreparedProxyExchange, McpPolicyError> {
        let exchange = self.exchanges.clone().read_owned().await;
        let members = message.as_array().ok_or(McpPolicyError::Call)?;
        for member in members {
            self.binding.access.authorize_request(member)?;
        }
        let responses = members
            .iter()
            .filter(|member| message_kind(member).ok() == Some(MessageKind::Response))
            .count();
        let callbacks_only = responses == members.len();
        if responses != 0 && !callbacks_only {
            return Err(McpPolicyError::Call);
        }
        if !callbacks_only
            && members.iter().any(|member| {
                let method = member["method"].as_str().unwrap_or("");
                !supported_method(method) || method.starts_with("tasks/")
            })
        {
            return Err(McpPolicyError::Call);
        }
        let tools = members
            .iter()
            .filter(|member| member["method"] == "tools/call")
            .count();
        let mut slots = Vec::new();
        for (semaphore, count) in [
            (&self.tool_slots, tools),
            (
                if callbacks_only {
                    &self.callback_slots
                } else {
                    &self.control_slots
                },
                members.len() - tools,
            ),
        ] {
            if count != 0 {
                slots.push(
                    semaphore
                        .clone()
                        .try_acquire_many_owned(count as u32)
                        .map_err(|_| McpPolicyError::Call)?,
                );
            }
        }
        let workspace = self.budget.workspace(callbacks_only)?;
        let lifecycle = if callbacks_only {
            None
        } else {
            Some(self.lifecycle_gate.clone().lock_owned().await)
        };
        if !self.is_active() {
            return Err(McpPolicyError::Scope);
        }
        // Commit lifecycle changes only after every member has passed local admission.
        let mut protocol = self.protocol.lock().await;
        let initialized = protocol.awaiting_initialized()
            && members
                .iter()
                .any(|member| member["method"] == "notifications/initialized");
        let mut candidate = protocol.clone();
        let mut context = candidate.begin(&message)?;
        if callbacks_only && let Some(current) = self.upstream_context.lock().await.as_ref() {
            context = current.clone();
        }
        let mut callbacks = self.callbacks.lock().await;
        let callback_keys: HashSet<_> = if callbacks_only {
            members
                .iter()
                .map(|member| correlation_id(&member["id"]))
                .collect()
        } else {
            HashSet::new()
        };
        if callbacks_only
            && (callback_keys.len() != members.len()
                || callback_keys.iter().any(|key| !callbacks.contains(key)))
        {
            return Err(McpPolicyError::Call);
        }
        let requests: Vec<_> = members
            .iter()
            .filter(|member| message_kind(member).ok() == Some(MessageKind::Request))
            .map(|member| correlation_id(&member["id"]))
            .collect();
        let methods = members
            .iter()
            .filter(|member| message_kind(member).ok() == Some(MessageKind::Request))
            .map(|member| {
                (
                    correlation_id(&member["id"]),
                    member["method"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        let mut request_ids = self.request_ids.lock().await;
        if request_ids.len() + requests.len() > 4096
            || requests.iter().collect::<HashSet<_>>().len() != requests.len()
            || requests.iter().any(|key| request_ids.contains(key))
        {
            return Err(McpPolicyError::Call);
        }
        let mut discovery = self.discovery.lock().await;
        // A tools/list member cannot authorize another member before its response arrives.
        let call = self.binding.policy.prepare_batch(
            discovery.catalog.as_ref(),
            &McpCallContext {
                executor,
                proxy_session_id: &self.id,
                http: &context,
            },
            message.clone(),
            |name, schema, args| (self.binding.bind_arguments)(name, schema, args),
        )?;
        let mut discoveries = HashMap::new();
        let mut generation = discovery.generation;
        for member in members
            .iter()
            .filter(|member| member["method"] == "tools/list")
        {
            let cursor = member
                .pointer("/params/cursor")
                .map(|cursor| {
                    cursor
                        .as_str()
                        .map(str::to_owned)
                        .ok_or(McpPolicyError::Call)
                })
                .transpose()?;
            let page_generation = if cursor.is_none() {
                generation = generation.checked_add(1).ok_or(McpPolicyError::Catalog)?;
                generation
            } else {
                if cursor != discovery.next_cursor {
                    return Err(McpPolicyError::Catalog);
                }
                // An already-known page belongs to the prior catalog, even if a refresh is
                // submitted in the same batch. Arrival order must not mix those generations.
                discovery.generation
            };
            discoveries.insert(correlation_id(&member["id"]), (page_generation, cursor));
        }
        discovery.generation = generation;
        request_ids.extend(requests);
        callbacks.retain(|key| !callback_keys.contains(key));
        *protocol = candidate;
        drop((protocol, callbacks, request_ids, discovery));
        Ok(PreparedProxyExchange {
            session: self.clone(),
            message,
            context,
            kind: Exchange::Batch(Box::new(PreparedProxyBatch {
                call,
                discoveries,
                methods,
                initialized,
            })),
            handshake: initialized,
            _slots: slots,
            _lifecycle: if initialized { lifecycle } else { None },
            _workspace: workspace,
            _exchange: exchange,
        })
    }
}

impl PreparedProxyBatch {
    pub(super) async fn run(
        self,
        session: Arc<McpProxySession>,
        journal: JournaledMcpClient,
        sink: &mut Sink,
        context: &HttpContext,
    ) -> Result<Option<Value>, String> {
        let Self {
            call,
            discoveries,
            methods,
            initialized,
        } = self;
        let (events, mut receiver) = mpsc::channel(8);
        let operation = journal.run_batch(call, events);
        tokio::pin!(operation);
        let mut events_open = true;
        let result = loop {
            tokio::select! {
                biased;
                result = &mut operation => break result,
                event = receiver.recv(), if events_open => {
                    if let Some(event) = event {
                        forward(event, &session, (&discoveries, &methods), context, initialized, sink).await;
                    } else { events_open = false; }
                }
            }
        };
        // A disconnected batch may already have durable partial results queued for delivery.
        // Drain those facts on both success and failure before closing the exchange.
        while let Ok(event) = receiver.try_recv() {
            forward(
                event,
                &session,
                (&discoveries, &methods),
                context,
                initialized,
                sink,
            )
            .await;
        }
        match result {
            Ok(result) => {
                if context.session_id.is_some() && result.http_status == 404 {
                    sink.fail();
                }
                if initialized && result.http_status >= 400 {
                    session.protocol.lock().await.initialization_failed();
                }
                if result.event_delivery_lost {
                    sink.fail();
                }
                Ok(None)
            }
            Err(error) => {
                if initialized {
                    session.protocol.lock().await.initialization_failed();
                }
                Err(error.to_string())
            }
        }
    }
}

async fn forward(
    event: Budgeted<HttpEvent>,
    session: &McpProxySession,
    projection: (&Discoveries, &Methods),
    context: &HttpContext,
    initialized: bool,
    sink: &mut Sink,
) {
    let (discoveries, methods) = projection;
    let (event, bytes) = event.into_parts();
    let Some(mut message) = event.message else {
        return;
    };
    let members = match &mut message {
        Value::Array(members) => members.as_mut_slice(),
        member => std::slice::from_mut(member),
    };
    for member in members {
        if message_kind(member).ok() != Some(MessageKind::Response) {
            continue;
        }
        if let Some(method) = methods.get(&correlation_id(&member["id"]))
            && session
                .binding
                .access
                .project_response(method, member)
                .is_err()
        {
            sink.fail();
        }
        if initialized
            && member.get("id").is_none_or(Value::is_null)
            && member.get("error").is_some()
        {
            session.protocol.lock().await.initialization_failed();
        }
        if let Some((generation, cursor)) = discoveries.get(&correlation_id(&member["id"]))
            && session
                .apply_discovery(member, context, *generation, cursor)
                .await
                .is_err()
        {
            sink.fail();
        }
    }
    if session.observe(&message).await.is_err() {
        sink.fail();
    }
    sink.emit(Some(message), false, Some(bytes));
}
