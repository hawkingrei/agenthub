use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::credentials::BroadcastIntent;
use super::metadata::NodeTransportMetadata;

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
