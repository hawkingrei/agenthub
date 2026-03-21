use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use agenthub_team_actor::{
    ActorAckRequest, ActorAckResponse, ActorInboxRequest, ActorInboxResponse, ActorSendRequest,
    ActorSendResponse, ActorServiceError,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::agent::{AGENT_NODE_MAIN_ID, AgentNodeRecord};
use crate::team::TeamActorMessageRecord;

pub(crate) const DEFAULT_CLUSTER_ID: &str = "agenthub-cluster";
const DIRECT_NODE_SCOPE: &str = "node:p2p";
const UNKNOWN_SHARED_KEY_ID: &str = "phase1-shared-key";

pub(crate) fn derive_cluster_id(expected_issuer: Option<&str>) -> String {
    expected_issuer
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_CLUSTER_ID)
        .to_string()
}

pub(crate) fn shared_key_kid(shared_secret: &str) -> String {
    let digest = Sha256::digest(shared_secret.as_bytes());
    let mut suffix = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        let _ = write!(&mut suffix, "{byte:02x}");
    }
    format!("shared-hs256-{suffix}")
}

pub(crate) fn normalize_scope(scope: Vec<String>, permissions: &[String]) -> Vec<String> {
    let mut values = scope
        .into_iter()
        .chain(permissions.iter().cloned())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    values.insert(DIRECT_NODE_SCOPE.to_string());
    values.into_iter().collect()
}

pub(crate) fn normalize_audience(audience: Vec<String>, fallback: Option<&str>) -> Vec<String> {
    let mut values = audience
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    if let Some(fallback) = fallback.map(str::trim).filter(|value| !value.is_empty()) {
        values.insert(fallback.to_string());
    }
    values.into_iter().collect()
}

pub(crate) fn payload_digest(payload: &Value) -> anyhow::Result<String> {
    let encoded = serde_json::to_vec(payload)?;
    let digest = Sha256::digest(&encoded);
    let mut out = String::with_capacity(64);
    for byte in digest {
        let _ = write!(&mut out, "{byte:02x}");
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct NodeTransportMetadata {
    pub cluster_id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broadcast_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub scope: Vec<String>,
    #[serde(default)]
    pub audience: Vec<String>,
    pub issued_at: i64,
    pub expires_at: i64,
    pub kid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_digest: Option<String>,
}

impl NodeTransportMetadata {
    pub(crate) fn from_route_value(route: &Value) -> Option<Self> {
        serde_json::from_value(route.clone()).ok()
    }

    #[allow(dead_code)]
    pub(crate) fn apply_to_route(&self, route: &mut Map<String, Value>) {
        route.insert("cluster_id".to_string(), json!(self.cluster_id));
        route.insert("source_node_id".to_string(), json!(self.source_node_id));
        route.insert("target_node_id".to_string(), json!(self.target_node_id));
        route.insert("scope".to_string(), json!(self.scope));
        route.insert("audience".to_string(), json!(self.audience));
        route.insert("issued_at".to_string(), json!(self.issued_at));
        route.insert("expires_at".to_string(), json!(self.expires_at));
        route.insert("kid".to_string(), json!(self.kid));
        if let Some(value) = self.broadcast_id.as_deref() {
            route.insert("broadcast_id".to_string(), json!(value));
        }
        if let Some(value) = self.correlation_id.as_deref() {
            route.insert("correlation_id".to_string(), json!(value));
        }
        if let Some(value) = self.idempotency_key.as_deref() {
            route.insert("idempotency_key".to_string(), json!(value));
        }
        if let Some(value) = self.payload_digest.as_deref() {
            route.insert("payload_digest".to_string(), json!(value));
        }
    }
}

pub(crate) fn build_message_metadata(message: &TeamActorMessageRecord) -> NodeTransportMetadata {
    let route_metadata = message
        .route
        .as_ref()
        .and_then(NodeTransportMetadata::from_route_value);
    let payload_correlation_id = message
        .payload
        .get("correlation_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let payload_broadcast_id = message
        .payload
        .get("broadcast_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let payload_digest = route_metadata
        .as_ref()
        .and_then(|value| value.payload_digest.clone())
        .or_else(|| payload_digest(&message.payload).ok());
    let created_at = if message.created_at > 0 {
        message.created_at
    } else {
        chrono::Utc::now().timestamp()
    };
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
    let metadata = route_metadata.unwrap_or_else(|| NodeTransportMetadata {
        cluster_id: DEFAULT_CLUSTER_ID.to_string(),
        source_node_id: fallback_source.to_string(),
        target_node_id: fallback_target.to_string(),
        broadcast_id: payload_broadcast_id.clone(),
        correlation_id: payload_correlation_id.clone(),
        idempotency_key: Some(format!(
            "remote-relay:{}:{}",
            message.run_id, message.message_id
        )),
        scope: vec![DIRECT_NODE_SCOPE.to_string()],
        audience: Vec::new(),
        issued_at: created_at,
        expires_at: created_at + 3600,
        kid: UNKNOWN_SHARED_KEY_ID.to_string(),
        payload_digest: payload_digest.clone(),
    });
    NodeTransportMetadata {
        cluster_id: metadata.cluster_id,
        source_node_id: if metadata.source_node_id.trim().is_empty() {
            fallback_source.to_string()
        } else {
            metadata.source_node_id
        },
        target_node_id: if metadata.target_node_id.trim().is_empty() {
            fallback_target.to_string()
        } else {
            metadata.target_node_id
        },
        broadcast_id: metadata.broadcast_id.or(payload_broadcast_id),
        correlation_id: metadata.correlation_id.or(payload_correlation_id),
        idempotency_key: metadata.idempotency_key.or_else(|| {
            Some(format!(
                "remote-relay:{}:{}",
                message.run_id, message.message_id
            ))
        }),
        scope: if metadata.scope.is_empty() {
            vec![DIRECT_NODE_SCOPE.to_string()]
        } else {
            metadata.scope
        },
        audience: metadata.audience,
        issued_at: metadata.issued_at.max(created_at),
        expires_at: metadata.expires_at.max(created_at + 1),
        kid: if metadata.kid.trim().is_empty() {
            UNKNOWN_SHARED_KEY_ID.to_string()
        } else {
            metadata.kid
        },
        payload_digest: metadata.payload_digest.or(payload_digest),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NodeCredentialRequest {
    pub source_node_id: String,
    pub role: String,
    pub actor_id: Option<String>,
    pub run_id: Option<String>,
    pub permissions: Vec<String>,
    pub scope: Vec<String>,
    pub audience: Vec<String>,
    pub ttl_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IssuedNodeAccessToken {
    pub source_node_id: String,
    pub role: String,
    pub access_token: String,
    pub cluster_id: String,
    pub scope: Vec<String>,
    pub audience: Vec<String>,
    pub kid: String,
    pub issued_at: i64,
    pub expires_at: i64,
}

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

pub(crate) trait CredentialProvider {
    fn issue_node_access_token(
        &self,
        request: NodeCredentialRequest,
    ) -> anyhow::Result<IssuedNodeAccessToken>;
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

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BroadcastIntent {
    pub cluster_id: String,
    pub source_node_id: String,
    pub broadcast_id: String,
    pub correlation_id: Option<String>,
    pub scope: Vec<String>,
    pub audience: Vec<String>,
    pub issued_at: i64,
    pub expires_at: i64,
    pub kid: String,
    pub payload_digest: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BroadcastAudienceMember {
    pub member_id: String,
    pub target_node_id: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct NodeBroadcastEnvelope {
    pub metadata: NodeTransportMetadata,
    pub member_ids: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct NodeBroadcastAck {
    pub broadcast_id: String,
    pub target_node_id: String,
    pub delivered_member_ids: Vec<String>,
    pub failed_member_ids: Vec<String>,
    pub acked_at: i64,
}

#[allow(dead_code)]
pub(crate) trait BroadcastPlanner {
    fn plan_node_broadcast(
        &self,
        intent: &BroadcastIntent,
        members: &[BroadcastAudienceMember],
    ) -> anyhow::Result<Vec<NodeBroadcastEnvelope>>;
}

#[allow(dead_code)]
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct NodeScopedBroadcastPlanner;

impl BroadcastPlanner for NodeScopedBroadcastPlanner {
    fn plan_node_broadcast(
        &self,
        intent: &BroadcastIntent,
        members: &[BroadcastAudienceMember],
    ) -> anyhow::Result<Vec<NodeBroadcastEnvelope>> {
        let mut grouped = BTreeMap::<String, Vec<String>>::new();
        for member in members {
            let target_node_id = member.target_node_id.trim();
            if target_node_id.is_empty() {
                anyhow::bail!("broadcast audience member is missing target_node_id");
            }
            let member_id = member.member_id.trim();
            if member_id.is_empty() {
                anyhow::bail!("broadcast audience member is missing member_id");
            }
            grouped
                .entry(target_node_id.to_string())
                .or_default()
                .push(member_id.to_string());
        }
        let envelopes = grouped
            .into_iter()
            .map(|(target_node_id, member_ids)| NodeBroadcastEnvelope {
                metadata: NodeTransportMetadata {
                    cluster_id: intent.cluster_id.clone(),
                    source_node_id: intent.source_node_id.clone(),
                    target_node_id,
                    broadcast_id: Some(intent.broadcast_id.clone()),
                    correlation_id: intent.correlation_id.clone(),
                    idempotency_key: Some(intent.broadcast_id.clone()),
                    scope: intent.scope.clone(),
                    audience: intent.audience.clone(),
                    issued_at: intent.issued_at,
                    expires_at: intent.expires_at,
                    kid: intent.kid.clone(),
                    payload_digest: Some(intent.payload_digest.clone()),
                },
                member_ids,
            })
            .collect::<Vec<_>>();
        Ok(envelopes)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::{
        BroadcastAudienceMember, BroadcastIntent, BroadcastPlanner, NodeScopedBroadcastPlanner,
        NodeTransportMetadata, build_message_metadata, normalize_audience, normalize_scope,
        shared_key_kid,
    };
    use crate::team::{TeamActorMessageRecord, TeamActorMessageStatus, TeamActorMessageTransport};

    #[test]
    fn normalize_scope_keeps_direct_node_scope_and_permissions() {
        let scope = normalize_scope(
            vec!["cluster:broadcast".to_string()],
            &[
                "team:message:send".to_string(),
                "team:message:send".to_string(),
            ],
        );
        assert_eq!(
            scope,
            vec![
                "cluster:broadcast".to_string(),
                "node:p2p".to_string(),
                "team:message:send".to_string()
            ]
        );
    }

    #[test]
    fn normalize_audience_keeps_unique_values() {
        let audience = normalize_audience(
            vec![
                "agenthub-internal".to_string(),
                "agenthub-internal".to_string(),
            ],
            Some("node"),
        );
        assert_eq!(
            audience,
            vec!["agenthub-internal".to_string(), "node".to_string()]
        );
    }

    #[test]
    fn shared_key_kid_is_deterministic() {
        let first = shared_key_kid("cluster-secret");
        let second = shared_key_kid("cluster-secret");
        assert_eq!(first, second);
        assert!(first.starts_with("shared-hs256-"));
    }

    #[test]
    fn build_message_metadata_prefers_route_fields() {
        let route = json!({
            "cluster_id": "cluster-a",
            "source_node_id": "node-a",
            "target_node_id": "node-b",
            "broadcast_id": "broadcast-1",
            "correlation_id": "corr-1",
            "idempotency_key": "idempotent-1",
            "scope": ["node:p2p", "team:message:send"],
            "audience": ["agenthub-internal"],
            "issued_at": 100,
            "expires_at": 200,
            "kid": "shared-hs256-1234",
            "payload_digest": "digest-1"
        });
        let metadata = build_message_metadata(&TeamActorMessageRecord {
            message_id: 1,
            run_id: "run-1".to_string(),
            from_actor_id: "planner".to_string(),
            from_peer_id: "main".to_string(),
            from_actor_kind: agenthub_team_actor::ActorIdentityKind::Agent,
            to_actor_id: "reviewer".to_string(),
            to_peer_id: "node".to_string(),
            to_actor_kind: agenthub_team_actor::ActorIdentityKind::Agent,
            channel: "coordination".to_string(),
            transport: TeamActorMessageTransport::Remote,
            route: Some(route),
            payload: json!({"text":"hello","correlation_id":"ignored-corr"}),
            status: TeamActorMessageStatus::Pending,
            created_at: 50,
            delivered_at: None,
        });
        assert_eq!(metadata.cluster_id, "cluster-a");
        assert_eq!(metadata.source_node_id, "node-a");
        assert_eq!(metadata.target_node_id, "node-b");
        assert_eq!(metadata.broadcast_id.as_deref(), Some("broadcast-1"));
        assert_eq!(metadata.correlation_id.as_deref(), Some("corr-1"));
        assert_eq!(metadata.idempotency_key.as_deref(), Some("idempotent-1"));
        assert_eq!(metadata.scope, vec!["node:p2p", "team:message:send"]);
        assert_eq!(metadata.audience, vec!["agenthub-internal"]);
        assert_eq!(metadata.issued_at, 100);
        assert_eq!(metadata.expires_at, 200);
        assert_eq!(metadata.kid, "shared-hs256-1234");
        assert_eq!(metadata.payload_digest.as_deref(), Some("digest-1"));
    }

    #[test]
    fn node_transport_metadata_round_trips_through_route_json() {
        let metadata = NodeTransportMetadata {
            cluster_id: "cluster-a".to_string(),
            source_node_id: "node-a".to_string(),
            target_node_id: "node-b".to_string(),
            broadcast_id: Some("broadcast-1".to_string()),
            correlation_id: Some("corr-1".to_string()),
            idempotency_key: Some("idem-1".to_string()),
            scope: vec!["node:p2p".to_string()],
            audience: vec!["agenthub-internal".to_string()],
            issued_at: 100,
            expires_at: 200,
            kid: "shared-hs256-1".to_string(),
            payload_digest: Some("digest-1".to_string()),
        };
        let mut route = Map::new();
        route.insert("kind".to_string(), json!("grpc"));
        route.insert(
            "grpc_target".to_string(),
            json!("https://node-b.internal:50051"),
        );
        metadata.apply_to_route(&mut route);
        let parsed =
            NodeTransportMetadata::from_route_value(&Value::Object(route)).expect("parse route");
        assert_eq!(parsed, metadata);
    }

    #[test]
    fn node_scoped_broadcast_planner_groups_members_by_target_node() {
        let planner = NodeScopedBroadcastPlanner;
        let envelopes = planner
            .plan_node_broadcast(
                &BroadcastIntent {
                    cluster_id: "cluster-a".to_string(),
                    source_node_id: "leader-node".to_string(),
                    broadcast_id: "broadcast-1".to_string(),
                    correlation_id: Some("corr-1".to_string()),
                    scope: vec!["node:p2p".to_string()],
                    audience: vec!["agenthub-internal".to_string()],
                    issued_at: 100,
                    expires_at: 200,
                    kid: "shared-hs256-1".to_string(),
                    payload_digest: "digest-1".to_string(),
                },
                &[
                    BroadcastAudienceMember {
                        member_id: "worker-a".to_string(),
                        target_node_id: "node-a".to_string(),
                    },
                    BroadcastAudienceMember {
                        member_id: "worker-b".to_string(),
                        target_node_id: "node-a".to_string(),
                    },
                    BroadcastAudienceMember {
                        member_id: "worker-c".to_string(),
                        target_node_id: "node-b".to_string(),
                    },
                ],
            )
            .expect("plan broadcast");
        assert_eq!(envelopes.len(), 2);
        assert_eq!(envelopes[0].metadata.target_node_id, "node-a");
        assert_eq!(
            envelopes[0].member_ids,
            vec!["worker-a".to_string(), "worker-b".to_string()]
        );
        assert_eq!(envelopes[1].metadata.target_node_id, "node-b");
        assert_eq!(envelopes[1].member_ids, vec!["worker-c".to_string()]);
    }
}
