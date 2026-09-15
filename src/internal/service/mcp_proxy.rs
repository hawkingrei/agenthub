use agenthub_agent_domain::loop_runtime::LoopReservation;
use agenthub_db::loop_runtime::LoopStore;
use std::pin::Pin;
use tokio::sync::mpsc;

use super::loop_activation::ExecutionAdmission;
use super::*;
use crate::internal::proto::agenthub::internal::v1::{
    CloseMcpProxyRequest, CloseMcpProxyResponse, ExchangeMcpProxyRequest, McpProxyFrame,
    OpenMcpProxyRequest, OpenMcpProxyResponse,
};

pub(super) type McpResponseStream =
    Pin<Box<dyn futures::Stream<Item = Result<McpProxyFrame, Status>> + Send>>;

impl TeamInternalControlService {
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
        let message = agenthub_mcp::protocol::parse_message(payload.message_json.as_bytes())
            .map_err(|_| Status::invalid_argument("invalid or oversized MCP message"))?;
        let (executor, guard) = self
            .mcp_executor(&metadata, message_admission(&message))
            .await?;
        let hub = self.deps.agents.mcp_proxy()?;
        let session = hub.session(&executor, &payload.session_id).await?;
        let (output, receiver) = mpsc::channel(8);
        match session.prepare(&executor, message.clone()).await {
            Ok(prepared) => {
                let journal = hub.journal.clone();
                if self
                    .deps
                    .agents
                    .daemon_tasks()
                    .spawn_runtime_task("mcp-exchange", async move {
                        let _guard = guard;
                        prepared.run(journal, output).await;
                        Ok(())
                    })
                    .is_err()
                {
                    session.close();
                    return Err(Status::unavailable("MCP proxy is shutting down"));
                }
            }
            Err(error) => {
                if agenthub_mcp::protocol::message_kind(&message).ok()
                    != Some(agenthub_mcp::protocol::MessageKind::Request)
                {
                    // Notifications and callback responses have no response of their own. End
                    // the invalid exchange without fabricating an unsolicited JSON-RPC error.
                    session.close();
                    return Err(Status::failed_precondition("MCP message admission failed"));
                }
                let response = serde_json::json!({"jsonrpc":"2.0","id":message.get("id"),"error":{"code":-32000,"message":error.to_string()}});
                let _ = output.try_send(agenthub_mcp::bridge::McpProxyFrame {
                    message_json: response.to_string(),
                    finished: true,
                });
            }
        }
        let stream = futures::stream::unfold(receiver, |mut receiver| async move {
            receiver.recv().await.map(|frame| {
                (
                    Ok(McpProxyFrame {
                        message_json: frame.message_json,
                        finished: frame.finished,
                    }),
                    receiver,
                )
            })
        });
        Ok(Response::new(Box::pin(stream)))
    }
}

fn message_admission(message: &serde_json::Value) -> ExecutionAdmission {
    use agenthub_mcp::protocol::{MessageKind, message_kind};

    let method = message["method"].as_str().unwrap_or("");
    let bootstrap = match message_kind(message) {
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
        _ => false,
    };
    if bootstrap {
        ExecutionAdmission::Bootstrap
    } else {
        ExecutionAdmission::Running
    }
}
