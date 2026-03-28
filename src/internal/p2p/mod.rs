#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub(crate) use agenthub_internal_p2p::{
    BroadcastAudienceMember, BroadcastIntent, BroadcastPlanner, CredentialProvider,
    DEFAULT_CLUSTER_ID, IssuedNodeAccessToken, MembershipView, NodeBroadcastAck,
    NodeBroadcastEnvelope, NodeCredentialRequest, NodeScopedBroadcastPlanner,
    NodeTransportMetadata, P2PTransport, ResolvedNodeEndpoint, TransportMetadataInput,
    build_transport_metadata, derive_cluster_id, normalize_audience, normalize_scope,
    payload_digest, shared_key_kid,
};

use crate::agent::{AGENT_NODE_MAIN_ID, AgentNodeRecord};
use crate::team::TeamActorMessageRecord;

pub(crate) fn build_message_metadata(message: &TeamActorMessageRecord) -> NodeTransportMetadata {
    let route_metadata = message
        .route
        .as_ref()
        .and_then(NodeTransportMetadata::from_route_value);
    let payload_correlation_id = message
        .payload
        .get("correlation_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let payload_broadcast_id = message
        .payload
        .get("broadcast_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let fallback_target = if message.to_peer_id.trim().is_empty() {
        AGENT_NODE_MAIN_ID
    } else {
        message.to_peer_id.trim()
    };
    let fallback_source = if message.from_peer_id.trim().is_empty() {
        AGENT_NODE_MAIN_ID
    } else {
        message.from_peer_id.trim()
    };
    build_transport_metadata(TransportMetadataInput {
        route_metadata,
        payload_correlation_id,
        payload_broadcast_id,
        payload: &message.payload,
        created_at: message.created_at,
        fallback_source_node_id: fallback_source,
        fallback_target_node_id: fallback_target,
        run_id: &message.run_id,
        message_id: message.message_id,
    })
}

pub(crate) fn resolved_node_endpoint_from_agent_node_record(
    cluster_id: &str,
    node: AgentNodeRecord,
) -> ResolvedNodeEndpoint {
    ResolvedNodeEndpoint {
        cluster_id: cluster_id.to_string(),
        node_id: node.id,
        grpc_target: node.grpc_target,
        tls_server_name: node.tls_server_name,
        is_main: node.is_main,
    }
}
