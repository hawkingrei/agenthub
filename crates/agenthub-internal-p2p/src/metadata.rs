use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::credentials::DEFAULT_CLUSTER_ID;

const DIRECT_NODE_SCOPE: &str = "node:p2p";
const UNKNOWN_SHARED_KEY_ID: &str = "phase1-shared-key";

pub fn payload_digest(payload: &Value) -> anyhow::Result<String> {
    let encoded = serde_json::to_vec(payload)?;
    let digest = Sha256::digest(&encoded);
    let mut out = String::with_capacity(64);
    for byte in digest {
        let _ = write!(&mut out, "{byte:02x}");
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeTransportMetadata {
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
    pub fn from_route_value(route: &Value) -> Option<Self> {
        serde_json::from_value(route.clone()).ok()
    }

    pub fn apply_to_route(&self, route: &mut Map<String, Value>) {
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

pub struct TransportMetadataInput<'a> {
    pub route_metadata: Option<NodeTransportMetadata>,
    pub payload_correlation_id: Option<String>,
    pub payload_broadcast_id: Option<String>,
    pub payload: &'a Value,
    pub created_at: i64,
    pub fallback_source_node_id: &'a str,
    pub fallback_target_node_id: &'a str,
    pub run_id: &'a str,
    pub message_id: i64,
}

pub fn build_transport_metadata(input: TransportMetadataInput<'_>) -> NodeTransportMetadata {
    let route_metadata = input.route_metadata;
    let payload_correlation_id = input.payload_correlation_id;
    let payload_broadcast_id = input.payload_broadcast_id;
    let payload_digest = route_metadata
        .as_ref()
        .and_then(|value| value.payload_digest.clone())
        .or_else(|| payload_digest(input.payload).ok());
    let created_at = if input.created_at > 0 {
        input.created_at
    } else {
        chrono::Utc::now().timestamp()
    };
    let fallback_target = input.fallback_target_node_id.trim();
    let fallback_source = input.fallback_source_node_id.trim();
    let metadata = route_metadata.unwrap_or_else(|| NodeTransportMetadata {
        cluster_id: DEFAULT_CLUSTER_ID.to_string(),
        source_node_id: fallback_source.to_string(),
        target_node_id: fallback_target.to_string(),
        broadcast_id: payload_broadcast_id.clone(),
        correlation_id: payload_correlation_id.clone(),
        idempotency_key: Some(format!(
            "remote-relay:{}:{}",
            input.run_id, input.message_id
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
                input.run_id, input.message_id
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
