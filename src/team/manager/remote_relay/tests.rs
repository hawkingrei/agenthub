use super::TeamRemoteRelayAdapter;
use crate::internal::client::InternalGrpcPeerClientConfig;
use crate::internal::p2p::NodeTransportMetadata;
use crate::internal::tls::InternalGrpcSecurityMode;
use crate::team::manager::remote_relay_route::{
    apply_route_signing, parse_remote_route, parse_route_header_name, parse_route_header_value,
    relay_timeout_ms,
};
use crate::team::manager::remote_relay_types::{
    GrpcRelayTlsDefaults, GrpcRemoteRelayRouteValue, ParsedRemoteRelayRoute,
    RELAY_DEFAULT_TIMEOUT_MS, RELAY_TIMEOUT_MAX_MS, RELAY_TIMEOUT_MIN_MS, RemoteRelaySigningValue,
};
use crate::team::{TeamActorMessageRecord, TeamActorMessageStatus, TeamActorMessageTransport};
use agenthub_team_actor::{ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, ActorIdentityKind};
use chrono::Utc;
use serde_json::json;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

async fn test_db() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect sqlite")
}

async fn create_agent_nodes_table(db: &SqlitePool) {
    sqlx::query(
        r#"
        CREATE TABLE agent_nodes (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            grpc_target TEXT NOT NULL,
            tls_server_name TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(db)
    .await
    .expect("create agent_nodes table");
}

async fn insert_agent_node(
    db: &SqlitePool,
    node_id: &str,
    grpc_target: &str,
    tls_server_name: Option<&str>,
) {
    sqlx::query(
        r#"
        INSERT INTO agent_nodes (id, name, grpc_target, tls_server_name, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, 1, 1)
        "#,
    )
    .bind(node_id)
    .bind(format!("Node {node_id}"))
    .bind(grpc_target)
    .bind(tls_server_name)
    .execute(db)
    .await
    .expect("insert agent node");
}

fn grpc_route_value(
    grpc_target: &str,
    access_token: &str,
    tls_server_name: &str,
) -> GrpcRemoteRelayRouteValue {
    GrpcRemoteRelayRouteValue {
        kind: Some("grpc".to_string()),
        grpc_target: grpc_target.to_string(),
        access_token: Some(access_token.to_string()),
        tls_server_name: Some(tls_server_name.to_string()),
    }
}

fn test_remote_message(
    target_node_id: &str,
    grpc_target: &str,
    tls_server_name: &str,
) -> TeamActorMessageRecord {
    let mut route = serde_json::Map::new();
    route.insert("kind".to_string(), json!("grpc"));
    route.insert("grpc_target".to_string(), json!(grpc_target));
    route.insert("access_token".to_string(), json!("secret-token"));
    route.insert("tls_server_name".to_string(), json!(tls_server_name));
    NodeTransportMetadata {
        cluster_id: "agenthub".to_string(),
        source_node_id: "node-a".to_string(),
        target_node_id: target_node_id.to_string(),
        broadcast_id: None,
        correlation_id: None,
        idempotency_key: Some("relay-test".to_string()),
        scope: vec!["node:p2p".to_string()],
        audience: vec!["agenthub-internal".to_string()],
        issued_at: Utc::now().timestamp(),
        expires_at: Utc::now().timestamp() + 600,
        kid: "shared-hs256-test".to_string(),
        payload_digest: None,
    }
    .apply_to_route(&mut route);

    TeamActorMessageRecord {
        message_id: 1,
        run_id: "run-1".to_string(),
        from_actor_id: "planner".to_string(),
        from_peer_id: "node-a".to_string(),
        from_actor_kind: ActorIdentityKind::Agent,
        to_actor_id: "reviewer".to_string(),
        to_peer_id: ACTOR_NODE_PEER_ID.to_string(),
        to_actor_kind: ActorIdentityKind::Agent,
        channel: "coordination".to_string(),
        transport: TeamActorMessageTransport::Remote,
        route: Some(json!(route)),
        payload: json!({"type":"chat_message","text":"hello"}),
        idempotency_key: Some("relay-test".to_string()),
        message_kind: agenthub_team_actor::ActorMessageKind::CoordinationRequest,
        status: TeamActorMessageStatus::Pending,
        handling_disposition: agenthub_team_actor::ActorMessageHandlingDisposition::Untriaged,
        handled_by_actor_id: None,
        thread_topic_key: None,
        thread_claim_status: None,
        thread_owner_actor_id: None,
        thread_lease_expires_at: None,
        linked_task_id: None,
        linked_task_relation: None,
        handled_at: None,
        created_at: Utc::now().timestamp(),
        delivered_at: None,
    }
}

#[test]
fn relay_timeout_ms_defaults_and_clamps() {
    assert_eq!(relay_timeout_ms(None), RELAY_DEFAULT_TIMEOUT_MS);
    assert_eq!(relay_timeout_ms(Some(1)), RELAY_TIMEOUT_MIN_MS);
    assert_eq!(
        relay_timeout_ms(Some(RELAY_TIMEOUT_MAX_MS + 1000)),
        RELAY_TIMEOUT_MAX_MS
    );
}

#[test]
fn parse_route_header_name_rejects_empty_and_invalid() {
    assert!(parse_route_header_name(" ").is_err());
    assert!(parse_route_header_name("bad space").is_err());
    assert!(parse_route_header_name("x-agenthub-header").is_ok());
}

#[test]
fn parse_route_header_value_rejects_control_chars() {
    assert!(parse_route_header_value("ok-value").is_ok());
    assert!(parse_route_header_value("bad\nvalue").is_err());
    assert!(parse_route_header_value("bad\rvalue").is_err());
}

#[test]
fn parse_remote_route_accepts_grpc_variant() {
    let parsed = parse_remote_route(&json!({
        "kind": "grpc",
        "grpc_target": "https://node.example.internal:50051",
        "access_token": "secret-token",
        "tls_server_name": "node.example.internal",
    }))
    .expect("parse grpc route");
    match parsed {
        ParsedRemoteRelayRoute::Grpc(route) => {
            assert_eq!(route.grpc_target, "https://node.example.internal:50051");
            assert_eq!(route.access_token.as_deref(), Some("secret-token"));
            assert_eq!(
                route.tls_server_name.as_deref(),
                Some("node.example.internal")
            );
        }
        ParsedRemoteRelayRoute::Http(_) => panic!("expected grpc route"),
    }
}

#[test]
fn parse_remote_route_accepts_grpc_variant_without_access_token() {
    let parsed = parse_remote_route(&json!({
        "kind": "grpc",
        "grpc_target": "https://node.example.internal:50051",
        "tls_server_name": "node.example.internal",
        "target_node_id": "node-b",
    }))
    .expect("parse grpc route without access token");
    match parsed {
        ParsedRemoteRelayRoute::Grpc(route) => {
            assert_eq!(route.grpc_target, "https://node.example.internal:50051");
            assert!(route.access_token.is_none());
            assert_eq!(
                route.tls_server_name.as_deref(),
                Some("node.example.internal")
            );
        }
        ParsedRemoteRelayRoute::Http(_) => panic!("expected grpc route"),
    }
}

#[test]
fn parse_remote_route_rejects_path_based_tls_fields_for_grpc() {
    let err = parse_remote_route(&json!({
        "kind": "grpc",
        "grpc_target": "https://node.example.internal:50051",
        "access_token": "secret-token",
        "tls_server_name": "node.example.internal",
        "ca_cert_path": "/tmp/ca.pem",
    }))
    .expect_err("gRPC relay route should reject path-based TLS overrides");
    assert!(
        err.to_string()
            .contains("route.ca_cert_path is not allowed"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn grpc_relay_requires_registered_target_node_match() {
    let db = test_db().await;
    create_agent_nodes_table(&db).await;
    insert_agent_node(
        &db,
        "node-b",
        "https://node-b.internal:50051",
        Some("node-b.internal"),
    )
    .await;

    let adapter = TeamRemoteRelayAdapter::new(db);
    adapter.configure_grpc_tls_defaults(Some(GrpcRelayTlsDefaults {
        ca_cert_path: Some("/tmp/ca.pem".to_string()),
        client_cert_path: None,
        client_key_path: None,
    }));

    let route = super::GrpcRemoteRelayRouteValue {
        kind: Some("grpc".to_string()),
        grpc_target: "https://node-b.internal:50051".to_string(),
        access_token: Some("secret-token".to_string()),
        tls_server_name: Some("node-b.internal".to_string()),
    };

    adapter
        .resolve_registered_grpc_route("node-b", &route)
        .await
        .expect("registered node should resolve");

    let mismatch = super::GrpcRemoteRelayRouteValue {
        grpc_target: "https://evil.example:50051".to_string(),
        ..route
    };
    let err = adapter
        .resolve_registered_grpc_route("node-b", &mismatch)
        .await
        .expect_err("mismatched grpc target should fail");
    assert!(
        err.to_string()
            .contains("route.grpc_target must match registered agent node"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn build_registered_grpc_route_for_target_node_omits_transient_credentials() {
    let db = test_db().await;
    create_agent_nodes_table(&db).await;
    insert_agent_node(
        &db,
        "node-b",
        "https://node-b.internal:50051",
        Some("node-b.internal"),
    )
    .await;

    let adapter = TeamRemoteRelayAdapter::new(db);
    adapter.configure_grpc_peer_client(Some(InternalGrpcPeerClientConfig {
        shared_secret: "relay-secret".to_string(),
        expected_issuer: Some("agenthub".to_string()),
        expected_audience: Some("agenthub-internal".to_string()),
        source_node_id: "main".to_string(),
        cert_dir: std::env::temp_dir().to_string_lossy().to_string(),
        security_mode: InternalGrpcSecurityMode::Disabled,
    }));

    let route = adapter
        .build_registered_grpc_route_for_target_node("node-b")
        .await
        .expect("build registered route");
    assert_eq!(route["kind"], json!("grpc"));
    assert_eq!(route["grpc_target"], json!("https://node-b.internal:50051"));
    assert_eq!(route["tls_server_name"], json!("node-b.internal"));
    assert_eq!(route["target_node_id"], json!("node-b"));
    assert!(
        route.get("access_token").is_none(),
        "route should stay stable: {route}"
    );
    assert!(
        route.get("issued_at").is_none(),
        "route should omit transient metadata: {route}"
    );
    assert!(
        route.get("expires_at").is_none(),
        "route should omit transient metadata: {route}"
    );
}

#[tokio::test]
async fn grpc_relay_rejects_main_target_node() {
    let db = test_db().await;
    create_agent_nodes_table(&db).await;
    let adapter = TeamRemoteRelayAdapter::new(db);
    let err = adapter
        .resolve_registered_grpc_route(
            ACTOR_MAIN_PEER_ID,
            &grpc_route_value(
                "https://node-b.internal:50051",
                "secret-token",
                "node-b.internal",
            ),
        )
        .await
        .expect_err("main node target should be rejected");
    assert!(
        err.to_string()
            .contains("must reference a registered remote agent node"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn grpc_relay_requires_registered_tls_server_name_match() {
    let db = test_db().await;
    create_agent_nodes_table(&db).await;
    insert_agent_node(
        &db,
        "node-b",
        "https://node-b.internal:50051",
        Some("node-b.internal"),
    )
    .await;
    let adapter = TeamRemoteRelayAdapter::new(db);
    let err = adapter
        .resolve_registered_grpc_route(
            "node-b",
            &grpc_route_value(
                "https://node-b.internal:50051",
                "secret-token",
                "wrong.internal",
            ),
        )
        .await
        .expect_err("tls server name mismatch should fail");
    assert!(
        err.to_string()
            .contains("route.tls_server_name must match registered agent node"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn grpc_relay_rejects_unregistered_target_node() {
    let db = test_db().await;
    create_agent_nodes_table(&db).await;
    let adapter = TeamRemoteRelayAdapter::new(db);
    let err = adapter
        .resolve_registered_grpc_route(
            "node-missing",
            &grpc_route_value(
                "https://node-b.internal:50051",
                "secret-token",
                "node-b.internal",
            ),
        )
        .await
        .expect_err("missing registry entry should fail");
    assert!(
        err.to_string().contains("is not registered for gRPC relay"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn deliver_grpc_requires_cluster_tls_defaults() {
    let db = test_db().await;
    create_agent_nodes_table(&db).await;
    insert_agent_node(
        &db,
        "node-b",
        "https://node-b.internal:50051",
        Some("node-b.internal"),
    )
    .await;
    let adapter = TeamRemoteRelayAdapter::new(db);
    let message = test_remote_message("node-b", "https://node-b.internal:50051", "node-b.internal");
    let err = adapter
        .deliver_grpc(
            &message,
            grpc_route_value(
                "https://node-b.internal:50051",
                "secret-token",
                "node-b.internal",
            ),
        )
        .await
        .expect_err("missing tls defaults should fail");
    assert!(
        err.to_string().contains("TLS defaults are unavailable"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn deliver_grpc_requires_peer_client_when_route_omits_access_token() {
    let db = test_db().await;
    create_agent_nodes_table(&db).await;
    insert_agent_node(
        &db,
        "node-b",
        "https://node-b.internal:50051",
        Some("node-b.internal"),
    )
    .await;
    let adapter = TeamRemoteRelayAdapter::new(db);
    adapter.configure_grpc_tls_defaults(Some(GrpcRelayTlsDefaults {
        ca_cert_path: Some("/tmp/ca.pem".to_string()),
        client_cert_path: None,
        client_key_path: None,
    }));
    let message = test_remote_message("node-b", "https://node-b.internal:50051", "node-b.internal");
    let err = adapter
        .deliver_grpc(
            &message,
            super::GrpcRemoteRelayRouteValue {
                kind: Some("grpc".to_string()),
                grpc_target: "https://node-b.internal:50051".to_string(),
                access_token: None,
                tls_server_name: Some("node-b.internal".to_string()),
            },
        )
        .await
        .expect_err("missing peer config should fail");
    assert!(
        err.to_string()
            .contains("internal gRPC peer client config is unavailable"),
        "unexpected error: {err}"
    );
}

#[test]
fn grpc_relay_tls_defaults_follow_security_mode() {
    let dir = std::env::temp_dir().join(format!(
        "agenthub-remote-relay-tls-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(dir.join("ca-cert.pem"), "ca").expect("write ca");
    std::fs::write(dir.join("client-cert.pem"), "cert").expect("write client cert");
    std::fs::write(dir.join("client-key.pem"), "key").expect("write client key");

    let tls_only = GrpcRelayTlsDefaults::from_cert_dir(
        &dir,
        crate::internal::tls::InternalGrpcSecurityMode::Tls,
    );
    assert!(tls_only.ca_cert_path.is_some());
    assert!(tls_only.client_cert_path.is_none());
    assert!(tls_only.client_key_path.is_none());

    let mtls = GrpcRelayTlsDefaults::from_cert_dir(
        &dir,
        crate::internal::tls::InternalGrpcSecurityMode::Mtls,
    );
    assert!(mtls.ca_cert_path.is_some());
    assert!(mtls.client_cert_path.is_some());
    assert!(mtls.client_key_path.is_some());

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn apply_route_signing_sets_signature_timestamp_and_message_id_headers() {
    let client = reqwest::Client::new();
    let request = client.post("https://example.com/relay");
    let signed = apply_route_signing(
        request,
        Some(&RemoteRelaySigningValue::HmacSha256 {
            secret: "test-secret".to_string(),
            header: None,
            timestamp_header: None,
        }),
        br#"{"payload":"x"}"#,
        42,
    )
    .expect("sign request");
    let built = signed.build().expect("build signed request");
    let headers = built.headers();

    let signature = headers
        .get("X-AgentHub-Signature")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(signature.starts_with("hmac-sha256="));
    assert!(headers.contains_key("X-AgentHub-Timestamp"));
    assert_eq!(
        headers
            .get("X-AgentHub-Message-Id")
            .and_then(|value| value.to_str().ok()),
        Some("42")
    );
}
