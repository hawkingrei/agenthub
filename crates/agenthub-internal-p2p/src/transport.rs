use agenthub_team_actor::{
    ActorAckRequest, ActorAckResponse, ActorInboxRequest, ActorInboxResponse, ActorSendRequest,
    ActorSendResponse, ActorServiceError,
};
use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedNodeEndpoint {
    pub cluster_id: String,
    pub node_id: String,
    pub grpc_target: Option<String>,
    pub tls_server_name: Option<String>,
    pub is_main: bool,
}

#[async_trait]
pub trait MembershipView {
    async fn resolve_node(&self, node_id: &str) -> anyhow::Result<ResolvedNodeEndpoint>;
}

#[async_trait]
pub trait P2PTransport {
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
