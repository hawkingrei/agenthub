use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const DEFAULT_CLUSTER_ID: &str = "agenthub-cluster";
const DIRECT_NODE_SCOPE: &str = "node:p2p";

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

pub(crate) trait CredentialProvider {
    fn issue_node_access_token(
        &self,
        request: NodeCredentialRequest,
    ) -> anyhow::Result<IssuedNodeAccessToken>;
}

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
