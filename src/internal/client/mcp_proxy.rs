use super::*;
use crate::internal::proto::agenthub::internal::v1::{
    CloseMcpProxyRequest, ExchangeMcpProxyRequest, McpProxyFrame, OpenMcpProxyRequest,
};

impl InternalGrpcMailboxClient {
    pub(crate) fn with_mcp_access_token(&self, access_token: String) -> Self {
        Self {
            channel: self.channel.clone(),
            access_token,
        }
    }

    fn mcp_client(&self) -> TeamInternalControlClient<Channel> {
        self.client()
            .max_encoding_message_size(crate::mcp_proxy::MCP_RPC_MESSAGE_LIMIT)
            .max_decoding_message_size(crate::mcp_proxy::MCP_RPC_MESSAGE_LIMIT)
    }

    pub(crate) async fn open_mcp_proxy(&self, server_id: String) -> anyhow::Result<String> {
        let request = self.control_request(OpenMcpProxyRequest { server_id })?;
        let response = self
            .mcp_client()
            .open_mcp_proxy(request)
            .await
            .map_err(mcp_rpc_error)?;
        Ok(response.into_inner().session_id)
    }

    pub(crate) async fn exchange_mcp_proxy(
        &self,
        session_id: String,
        message_json: String,
    ) -> anyhow::Result<tonic::Streaming<McpProxyFrame>> {
        let request = self.control_request(ExchangeMcpProxyRequest {
            session_id,
            message_json,
        })?;
        Ok(self
            .mcp_client()
            .exchange_mcp_proxy(request)
            .await
            .map_err(mcp_rpc_error)?
            .into_inner())
    }

    pub(crate) async fn close_mcp_proxy(&self, session_id: String) -> anyhow::Result<()> {
        let request = self.control_request(CloseMcpProxyRequest { session_id })?;
        self.mcp_client()
            .close_mcp_proxy(request)
            .await
            .map_err(mcp_rpc_error)?;
        Ok(())
    }
}

fn mcp_rpc_error(error: tonic::Status) -> anyhow::Error {
    anyhow::anyhow!("MCP proxy RPC failed ({:?})", error.code())
}
