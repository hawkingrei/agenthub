use agenthub_team_actor::{
    ActorAckRequest, ActorAckResponse, ActorInboxRequest, ActorInboxResponse, ActorSendRequest,
    ActorSendResponse, ActorServiceError,
};
use async_trait::async_trait;

use crate::agent::AgentNodeRecord;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedNodeEndpoint {
    pub cluster_id: String,
    pub node_id: String,
    pub grpc_target: Option<String>,
    pub tls_server_name: Option<String>,
    pub is_main: bool,
}

impl ResolvedNodeEndpoint {
    pub(crate) fn from_agent_node_record(cluster_id: &str, node: AgentNodeRecord) -> Self {
        Self {
            cluster_id: cluster_id.to_string(),
            node_id: node.id,
            grpc_target: node.grpc_target,
            tls_server_name: node.tls_server_name,
            is_main: node.is_main,
        }
    }
}

#[async_trait]
pub(crate) trait MembershipView {
    async fn resolve_node(&self, node_id: &str) -> anyhow::Result<ResolvedNodeEndpoint>;
}

#[allow(dead_code)]
#[async_trait]
pub(crate) trait P2PTransport {
    async fn send_p2p_message(
        &self,
        request: ActorSendRequest,
    ) -> Result<ActorSendResponse, ActorServiceError>;

    async fn list_p2p_inbox(
        &self,
        request: ActorInboxRequest,
    ) -> Result<ActorInboxResponse, ActorServiceError>;

    async fn ack_p2p_message(
        &self,
        request: ActorAckRequest,
    ) -> Result<ActorAckResponse, ActorServiceError>;
}
