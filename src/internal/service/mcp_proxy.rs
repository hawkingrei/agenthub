use agenthub_agent_domain::loop_runtime::LoopReservation;
use agenthub_db::loop_runtime::LoopStore;
use std::pin::Pin;
use tokio::sync::mpsc;

use super::loop_activation::ExecutionAdmission;
use super::*;
use crate::internal::proto::agenthub::internal::v1::{
    CloseMcpProxyRequest, CloseMcpProxyResponse, ExchangeMcpProxyRequest, ListenMcpProxyRequest,
    McpProxyFrame, OpenMcpProxyRequest, OpenMcpProxyResponse,
};

pub(super) type McpResponseStream =
    Pin<Box<dyn futures::Stream<Item = Result<McpProxyFrame, Status>> + Send>>;

impl TeamInternalControlService {
    pub(super) async fn listen_mcp_proxy_request(
        &self,
        request: Request<ListenMcpProxyRequest>,
    ) -> Result<Response<McpResponseStream>, Status> {
        let (executor, guard) = self
            .mcp_executor(request.metadata(), ExecutionAdmission::Bootstrap)
            .await?;
        let hub = self.deps.agents.mcp_proxy()?;
        let session = hub
            .session(&executor, &request.into_inner().session_id)
            .await?;
        let prepared = session
            .prepare_listener()
            .await
            .map_err(|_| Status::failed_precondition("MCP listener admission failed"))?;
        let (output, receiver) = mpsc::channel(8);
        let agents = self.deps.agents.clone();
        let store = LoopStore::new(self.deps.db.clone());
        let cleanup = session.clone();
        self.deps
            .agents
            .daemon_tasks()
            .spawn_runtime_task("mcp-listener", async move {
                prepared
                    .run(output, || {
                        validate_stream(
                            agents.clone(),
                            store.clone(),
                            executor.clone(),
                            ExecutionAdmission::Bootstrap,
                        )
                    })
                    .await;
                if !cleanup.is_active() {
                    let _ = cleanup.shutdown().await;
                }
                Ok(())
            })
            .map_err(|_| Status::unavailable("MCP proxy is shutting down"))?;
        drop(guard);
        Ok(Response::new(response_stream(receiver, session)))
    }

    async fn mcp_executor(
        &self,
        metadata: &MetadataMap,
        admission: ExecutionAdmission,
    ) -> Result<(LoopReservation, tokio::sync::OwnedRwLockReadGuard<()>), Status> {
        let (principal, guard) = self
            .authenticate_execution_admission(metadata, admission)
            .await?;
        self.authz
            .ensure_permission(&principal, InternalAction::McpProxy)?;
        let execution = principal.loop_execution.as_ref().ok_or_else(|| {
            Status::permission_denied("MCP requires signed activation credentials")
        })?;
        let reservation = LoopStore::new(self.deps.db.clone())
            .executor_reservation(
                principal
                    .actor_id
                    .as_deref()
                    .ok_or_else(|| Status::permission_denied("MCP actor is required"))?,
                principal
                    .run_id
                    .as_deref()
                    .ok_or_else(|| Status::permission_denied("MCP mailbox is required"))?,
                &execution.activation_id,
                execution.generation,
            )
            .await
            .map_err(|_| Status::permission_denied("MCP execution scope is no longer active"))?;
        Ok((
            reservation,
            guard.ok_or_else(|| Status::permission_denied("MCP execution guard is required"))?,
        ))
    }

    pub(super) async fn open_mcp_proxy_request(
        &self,
        request: Request<OpenMcpProxyRequest>,
    ) -> Result<Response<OpenMcpProxyResponse>, Status> {
        let (executor, _guard) = self
            .mcp_executor(request.metadata(), ExecutionAdmission::Bootstrap)
            .await?;
        let session_id = self
            .deps
            .agents
            .mcp_proxy()?
            .open(&executor, &request.into_inner().server_id)
            .await?;
        Ok(Response::new(OpenMcpProxyResponse { session_id }))
    }

    pub(super) async fn close_mcp_proxy_request(
        &self,
        request: Request<CloseMcpProxyRequest>,
    ) -> Result<Response<CloseMcpProxyResponse>, Status> {
        let (executor, _guard) = self
            .mcp_executor(request.metadata(), ExecutionAdmission::Bootstrap)
            .await?;
        self.deps
            .agents
            .mcp_proxy()?
            .close(&executor, &request.into_inner().session_id)
            .await?;
        Ok(Response::new(CloseMcpProxyResponse {}))
    }

    pub(super) async fn exchange_mcp_proxy_request(
        &self,
        request: Request<ExchangeMcpProxyRequest>,
    ) -> Result<Response<McpResponseStream>, Status> {
        let metadata = request.metadata().clone();
        let payload = request.into_inner();
        let hub = self.deps.agents.mcp_proxy()?;
        let _ingress = hub
            .budget
            .ingress
            .acquire(payload.message_json.len())
            .map_err(|_| Status::resource_exhausted("MCP ingress capacity reached"))?;
        let message = agenthub_mcp::protocol::parse_message(payload.message_json.as_bytes())
            .map_err(|_| Status::invalid_argument("invalid or oversized MCP message"))?;
        let (executor, guard) = self
            .mcp_executor(&metadata, message_admission(&message))
            .await?;
        let session = hub.session(&executor, &payload.session_id).await?;
        let (output, receiver) = mpsc::channel(8);
        if message["method"] == "subscriptions/listen" {
            match session
                .prepare_subscription(&executor, hub.journal.clone(), message.clone())
                .await
            {
                Ok(prepared) => {
                    let agents = self.deps.agents.clone();
                    let store = LoopStore::new(self.deps.db.clone());
                    let cleanup = session.clone();
                    let admission = message_admission(&message);
                    self.deps
                        .agents
                        .daemon_tasks()
                        .spawn_runtime_task("mcp-subscription", async move {
                            drop(guard);
                            prepared
                                .run(output, || {
                                    validate_stream(
                                        agents.clone(),
                                        store.clone(),
                                        executor.clone(),
                                        admission,
                                    )
                                })
                                .await;
                            if !cleanup.is_active() {
                                let _ = cleanup.shutdown().await;
                            }
                            Ok(())
                        })
                        .map_err(|_| Status::unavailable("MCP proxy is shutting down"))?;
                }
                Err(error) => {
                    let response =
                        admission_error(&message, &error.to_string()).ok_or_else(|| {
                            Status::failed_precondition("MCP subscription admission failed")
                        })?;
                    let frame = agenthub_mcp::bridge::McpProxyFrame::new(
                        response.to_string(),
                        true,
                        &hub.budget.delivery,
                    )
                    .map_err(|_| Status::resource_exhausted("MCP response capacity reached"))?;
                    let _ = output.try_send(frame);
                }
            }
            return Ok(Response::new(response_stream(receiver, session)));
        }
        match session.prepare(&executor, message.clone()).await {
            Ok(prepared) => {
                let journal = hub.journal.clone();
                let cleanup = session.clone();
                if self
                    .deps
                    .agents
                    .daemon_tasks()
                    .spawn_runtime_task("mcp-exchange", async move {
                        let _guard = guard;
                        prepared.run(journal, output).await;
                        if !cleanup.is_active() {
                            let _ = cleanup.shutdown().await;
                        }
                        Ok(())
                    })
                    .is_err()
                {
                    session.close();
                    return Err(Status::unavailable("MCP proxy is shutting down"));
                }
            }
            Err(error) => {
                let Some(response) = admission_error(&message, &error.to_string()) else {
                    // Notifications and callback responses have no response of their own. End
                    // the invalid exchange without fabricating an unsolicited JSON-RPC error.
                    session.close();
                    return Err(Status::failed_precondition("MCP message admission failed"));
                };
                let frame = agenthub_mcp::bridge::McpProxyFrame::new(
                    response.to_string(),
                    true,
                    &hub.budget.delivery,
                )
                .map_err(|_| Status::resource_exhausted("MCP response capacity reached"))?;
                let _ = output.try_send(frame);
            }
        }
        Ok(Response::new(response_stream(receiver, session)))
    }
}

async fn validate_stream(
    agents: std::sync::Arc<AgentManager>,
    store: LoopStore,
    executor: LoopReservation,
    admission: ExecutionAdmission,
) -> Result<tokio::sync::OwnedRwLockReadGuard<()>, agenthub_mcp::McpTransportError> {
    let guard = agents
        .loop_operation_gate(&executor.actor_id)
        .await
        .read_owned()
        .await;
    if executor.owner_id != agents.loop_owner_id() {
        return Err(agenthub_mcp::McpTransportError::Disconnected);
    }
    let result = match admission {
        ExecutionAdmission::Bootstrap => {
            store
                .verify_executor_bootstrap_live(&executor, chrono::Utc::now().timestamp())
                .await
        }
        _ => {
            store
                .verify_executor_live(&executor, chrono::Utc::now().timestamp())
                .await
        }
    };
    result.map_err(|_| agenthub_mcp::McpTransportError::Disconnected)?;
    Ok(guard)
}

fn response_stream(
    receiver: mpsc::Receiver<agenthub_mcp::bridge::McpProxyFrame>,
    session: std::sync::Arc<agenthub_mcp::bridge::McpProxySession>,
) -> McpResponseStream {
    let stream = futures::stream::unfold(
        (receiver, None, session),
        |(mut receiver, _previous, session)| async move {
            let frame = receiver.recv().await?;
            let can_listen = session.can_listen().await;
            let (message_json, finished, bytes) = frame.into_parts();
            Some((
                Ok(McpProxyFrame {
                    message_json,
                    finished,
                    can_listen,
                }),
                (receiver, Some(bytes), session),
            ))
        },
    );
    Box::pin(stream)
}

fn message_admission(message: &serde_json::Value) -> ExecutionAdmission {
    use agenthub_mcp::protocol::{MessageKind, message_kind};

    let method = message["method"].as_str().unwrap_or("");
    let bootstrap = match message_kind(message) {
        Ok(MessageKind::Request) if method == "subscriptions/listen" => message
            .pointer("/params/notifications")
            .and_then(serde_json::Value::as_object)
            .is_some_and(|filters| {
                filters.keys().all(|key| {
                    matches!(
                        key.as_str(),
                        "toolsListChanged" | "promptsListChanged" | "resourcesListChanged"
                    )
                })
            }),
        Ok(MessageKind::Request) => matches!(
            method,
            "initialize"
                | "ping"
                | "server/discover"
                | "tools/list"
                | "resources/list"
                | "resources/templates/list"
                | "prompts/list"
        ),
        Ok(MessageKind::Notification) => matches!(
            method,
            "notifications/initialized"
                | "notifications/progress"
                | "notifications/cancelled"
                | "notifications/roots/list_changed"
        ),
        // Session preparation independently requires a pending upstream callback ID. This arm
        // cannot manufacture permission to send an unsolicited response or another tool call.
        Ok(MessageKind::Response) => true,
        Ok(MessageKind::Batch) => message
            .as_array()
            .unwrap()
            .iter()
            .all(|member| matches!(message_admission(member), ExecutionAdmission::Bootstrap)),
        _ => false,
    };
    if bootstrap {
        ExecutionAdmission::Bootstrap
    } else {
        ExecutionAdmission::Running
    }
}

fn admission_error(message: &serde_json::Value, error: &str) -> Option<serde_json::Value> {
    use agenthub_mcp::protocol::{MessageKind, message_kind};

    match message_kind(message).ok()? {
        MessageKind::Request => Some(serde_json::json!({"jsonrpc":"2.0","id":message["id"],
            "error":{"code":-32000,"message":error}})),
        MessageKind::Batch => {
            let responses: Vec<_> = message
                .as_array()?
                .iter()
                .filter_map(|member| admission_error(member, error))
                .collect();
            (!responses.is_empty()).then_some(serde_json::Value::Array(responses))
        }
        _ => None,
    }
}
