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
