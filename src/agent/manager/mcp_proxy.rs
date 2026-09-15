use agenthub_agent_domain::loop_runtime::LoopReservation;
use agenthub_mcp::bridge::McpProxyBinding;
use std::sync::Arc;

use super::AgentManager;

impl AgentManager {
    pub(crate) fn initialize_mcp_proxy(
        &self,
        journal: agenthub_db::mcp_operations::McpOperationStore,
        mounts: Vec<(LoopReservation, Arc<McpProxyBinding>)>,
    ) -> anyhow::Result<()> {
        let hub = crate::mcp_proxy::McpProxyHub::new(journal, mounts)?;
        self.mcp_proxy
            .set(Arc::new(hub))
            .map_err(|_| anyhow::anyhow!("MCP proxy was already initialized"))
    }

    pub(crate) fn mcp_proxy(&self) -> Result<Arc<crate::mcp_proxy::McpProxyHub>, tonic::Status> {
        self.mcp_proxy
            .get()
            .cloned()
            .ok_or_else(|| tonic::Status::unavailable("MCP proxy is not initialized"))
    }

    pub(super) async fn release_mcp_activation(&self, executor: &LoopReservation) {
        if let Some(hub) = self.mcp_proxy.get() {
            hub.release_activation(executor).await;
        }
    }
}
