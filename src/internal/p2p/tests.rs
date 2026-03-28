use serde_json::json;

use super::build_message_metadata;
use crate::team::{TeamActorMessageRecord, TeamActorMessageStatus, TeamActorMessageTransport};

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
