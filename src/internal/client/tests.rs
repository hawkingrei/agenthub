use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use crate::agent::{AgentConfig, AgentNodeConfig, WorktreeMode};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, ActorInboxRequest, ActorMailboxService,
    ActorMessageStatus, ActorMessageTransport, ActorSendRequest, ActorServiceErrorCode,
};
use serde_json::{Value, json};
use tonic::transport::{Certificate, Identity, ServerTlsConfig};
use uuid::Uuid;

use super::mailbox::{map_grpc_status, parse_message, parse_status, parse_transport};
use super::{
    InternalCreateTeamTaskRequest, InternalGrpcMailboxClient, InternalGrpcMailboxClientConfig,
    InternalTeamTaskPatch, normalize_existing_path, parse_output_stream, tls_path_if_exists,
};
use crate::api::team_tests::build_test_state;
use crate::internal::auth::{InternalAction, InternalAuthz, InternalAuthzConfig, InternalRole};
use crate::internal::p2p::NodeTransportMetadata;
use crate::internal::proto::agenthub::internal::v1::ActorMessage as GrpcActorMessage;
use crate::internal::proto::agenthub::internal::v1::team_internal_control_server::TeamInternalControlServer;
use crate::internal::service::TeamInternalControlService;
use crate::internal::tls::{
    InternalGrpcSecurityMode, ensure_tls_material, install_rustls_crypto_provider,
};
use crate::team::{
    SendActorMessageInput, TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX, TeamActorMessageTransport,
    TeamTaskDetailRecord, TeamTaskListQuery,
};

const TEST_INTERNAL_SHARED_SECRET: &str = "agenthub-internal-client-test-secret";

fn build_authz() -> InternalAuthz {
    InternalAuthz::new(InternalAuthzConfig {
        shared_secret: TEST_INTERNAL_SHARED_SECRET.to_string(),
        expected_issuer: Some("agenthub".to_string()),
        expected_audience: Some("agenthub-internal".to_string()),
    })
}

fn issue_token(authz: &InternalAuthz, run_id: Option<&str>, permissions: Vec<String>) -> String {
    let (token, _expires_at) = authz
        .issue_access_token(InternalRole::Coordinator, None, run_id, permissions, 600)
        .expect("issue internal token");
    token
}

fn issue_mailbox_token(authz: &InternalAuthz, run_id: &str) -> String {
    issue_token(
        authz,
        Some(run_id),
        vec![
            InternalAction::MessageSend.as_str().to_string(),
            InternalAction::InboxList.as_str().to_string(),
            InternalAction::MessageAck.as_str().to_string(),
        ],
    )
}

fn issue_agent_manage_token(authz: &InternalAuthz) -> String {
    issue_token(
        authz,
        None,
        vec![InternalAction::AgentManage.as_str().to_string()],
    )
}

fn grpc_message(
    route_json: &str,
    payload_json: &str,
    transport: &str,
    status: &str,
    from_peer_id: &str,
    to_peer_id: &str,
) -> GrpcActorMessage {
    GrpcActorMessage {
        message_id: 41,
        run_id: "run-1".to_string(),
        from_actor_id: "planner".to_string(),
        to_actor_id: "reviewer".to_string(),
        channel: "coordination".to_string(),
        transport: transport.to_string(),
        route_json: route_json.to_string(),
        payload_json: payload_json.to_string(),
        status: status.to_string(),
        created_at: 111,
        delivered_at: 222,
        idempotency_key: "idem-1".to_string(),
        from_peer_id: from_peer_id.to_string(),
        to_peer_id: to_peer_id.to_string(),
    }
}

fn test_cert_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "agenthub-internal-client-{}-{}",
        name,
        Uuid::new_v4()
    ))
}

#[test]
fn helper_parsers_cover_known_and_default_values() {
    assert_eq!(parse_transport("remote"), ActorMessageTransport::Remote);
    assert_eq!(parse_transport("other"), ActorMessageTransport::Local);
    assert_eq!(
        parse_output_stream("stderr"),
        crate::agent::OutputStream::Stderr
    );
    assert_eq!(
        parse_output_stream("system"),
        crate::agent::OutputStream::System
    );
    assert_eq!(parse_output_stream("acp"), crate::agent::OutputStream::Acp);
    assert_eq!(
        parse_output_stream("other"),
        crate::agent::OutputStream::Stdout
    );
    assert_eq!(parse_status("delivered"), ActorMessageStatus::Delivered);
    assert_eq!(parse_status("dead_letter"), ActorMessageStatus::DeadLetter);
    assert_eq!(parse_status("other"), ActorMessageStatus::Pending);
}

#[test]
fn parse_message_defaults_blank_peer_ids_to_main() {
    let message = parse_message(grpc_message(
        r#"{"kind":"grpc"}"#,
        r#"{"type":"chat_message","text":"hello"}"#,
        "remote",
        "delivered",
        "",
        "",
    ))
    .expect("parse grpc message");
    assert_eq!(message.from_peer_id, ACTOR_MAIN_PEER_ID);
    assert_eq!(message.to_peer_id, ACTOR_MAIN_PEER_ID);
    assert_eq!(message.transport, ActorMessageTransport::Remote);
    assert_eq!(message.status, ActorMessageStatus::Delivered);
    assert_eq!(message.route, Some(json!({"kind":"grpc"})));
    assert_eq!(message.payload["text"], "hello");
}

#[test]
fn parse_message_rejects_invalid_route_or_payload_json() {
    let route_err = parse_message(grpc_message(
        "{",
        r#"{"type":"chat_message"}"#,
        "local",
        "pending",
        "main",
        "main",
    ))
    .expect_err("invalid route json should fail");
    assert!(route_err.message.contains("decode route_json"));

    let payload_err = parse_message(grpc_message("", "{", "local", "pending", "main", "main"))
        .expect_err("invalid payload json should fail");
    assert!(payload_err.message.contains("decode payload_json"));
}

#[test]
fn map_grpc_status_maps_common_codes() {
    let invalid = map_grpc_status(tonic::Status::invalid_argument("bad input"));
    assert_eq!(
        invalid.code,
        agenthub_team_actor::ActorServiceErrorCode::BadRequest
    );

    let denied = map_grpc_status(tonic::Status::permission_denied("denied"));
    assert_eq!(
        denied.code,
        agenthub_team_actor::ActorServiceErrorCode::Forbidden
    );

    let failed = map_grpc_status(tonic::Status::failed_precondition("gone"));
    assert_eq!(
        failed.code,
        agenthub_team_actor::ActorServiceErrorCode::Gone
    );

    let deadline = map_grpc_status(tonic::Status::deadline_exceeded("request stalled"));
    assert_eq!(
        deadline.code,
        agenthub_team_actor::ActorServiceErrorCode::TooManyRequests
    );
    assert!(deadline.message.contains("timed out"));
    assert!(deadline.message.contains("request stalled"));

    let duplicated = map_grpc_status(tonic::Status::deadline_exceeded(
        "internal gRPC request timed out after 15s",
    ));
    assert_eq!(
        duplicated.message,
        "internal gRPC request timed out after 15s"
    );
}

#[tokio::test]
async fn connect_times_out_when_tls_peer_accepts_but_never_completes_handshake() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stalled peer listener");
    let addr = listener.local_addr().expect("listener addr");
    let server = tokio::spawn(async move {
        let (_socket, _) = listener.accept().await.expect("accept stalled client");
        tokio::time::sleep(Duration::from_secs(30)).await;
    });

    let start = tokio::time::Instant::now();
    let err = match InternalGrpcMailboxClient::connect(InternalGrpcMailboxClientConfig {
        target: format!("https://{addr}"),
        access_token: "test-token".to_string(),
        ca_cert_path: None,
        tls_server_name: Some("localhost".to_string()),
        client_cert_path: None,
        client_key_path: None,
    })
    .await
    {
        Ok(_) => panic!("stalled handshake should time out"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("internal gRPC connect timed out"));
    assert!(start.elapsed() < Duration::from_secs(10));

    server.abort();
}

#[test]
fn path_helpers_validate_existing_and_missing_paths() {
    assert_eq!(
        normalize_existing_path(Some("   "), "ca_cert").expect("empty path is ignored"),
        None
    );

    let existing = test_cert_dir("path-helper");
    std::fs::create_dir_all(&existing).expect("create helper dir");
    let existing_file = existing.join("ca-cert.pem");
    std::fs::write(&existing_file, b"pem").expect("write cert");

    assert_eq!(
        normalize_existing_path(existing_file.to_str(), "ca_cert").expect("existing path"),
        Some(existing_file.to_string_lossy().to_string())
    );
    assert_eq!(
        tls_path_if_exists(&existing_file),
        Some(existing_file.to_string_lossy().to_string())
    );

    let missing = existing.join("missing.pem");
    assert!(tls_path_if_exists(&missing).is_none());
    let err =
        normalize_existing_path(missing.to_str(), "ca_cert").expect_err("missing path should fail");
    assert!(err.to_string().contains("ca_cert does not exist"));
}

struct StartedInternalGrpcServer {
    addr: SocketAddr,
    handle: tokio::task::JoinHandle<()>,
}

async fn spawn_mtls_internal_grpc_server(
    state: crate::state::AppState,
    authz: InternalAuthz,
    cert_dir: PathBuf,
) -> StartedInternalGrpcServer {
    let server = TeamInternalControlServer::new(TeamInternalControlService::new(
        crate::internal::team_internal_control_deps(&state),
        authz,
        InternalGrpcSecurityMode::Mtls,
        cert_dir.clone(),
        "bootstrap-token".to_string(),
    ));
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    drop(listener);

    let server_cert_pem =
        std::fs::read(cert_dir.join("server-cert.pem")).expect("read server cert pem");
    let server_key_pem =
        std::fs::read(cert_dir.join("server-key.pem")).expect("read server key pem");
    let ca_cert_pem = std::fs::read(cert_dir.join("ca-cert.pem")).expect("read ca cert pem");
    let handle = tokio::spawn(async move {
        let tls = ServerTlsConfig::new()
            .identity(Identity::from_pem(server_cert_pem, server_key_pem))
            .client_ca_root(Certificate::from_pem(ca_cert_pem))
            .client_auth_optional(true);
        tonic::transport::Server::builder()
            .tls_config(tls)
            .expect("tls config")
            .add_service(server)
            .serve(addr)
            .await
            .expect("serve internal grpc");
    });

    tokio::time::sleep(Duration::from_millis(150)).await;
    StartedInternalGrpcServer { addr, handle }
}

fn mtls_client_config(
    addr: SocketAddr,
    access_token: String,
    cert_dir: &Path,
) -> InternalGrpcMailboxClientConfig {
    InternalGrpcMailboxClientConfig {
        target: format!("https://{}", addr),
        access_token,
        ca_cert_path: Some(cert_dir.join("ca-cert.pem").to_string_lossy().to_string()),
        tls_server_name: Some("localhost".to_string()),
        client_cert_path: Some(
            cert_dir
                .join("client-cert.pem")
                .to_string_lossy()
                .to_string(),
        ),
        client_key_path: Some(
            cert_dir
                .join("client-key.pem")
                .to_string_lossy()
                .to_string(),
        ),
    }
}

fn grpc_relay_route(
    addr: SocketAddr,
    access_token: &str,
    source_node_id: &str,
    target_node_id: &str,
) -> Value {
    let mut route = serde_json::Map::new();
    route.insert("kind".to_string(), json!("grpc"));
    route.insert(
        "grpc_target".to_string(),
        json!(format!("https://{}", addr)),
    );
    route.insert("access_token".to_string(), json!(access_token));
    route.insert("tls_server_name".to_string(), json!("localhost"));
    NodeTransportMetadata {
        cluster_id: "agenthub".to_string(),
        source_node_id: source_node_id.to_string(),
        target_node_id: target_node_id.to_string(),
        broadcast_id: None,
        correlation_id: None,
        idempotency_key: None,
        scope: vec!["node:p2p".to_string()],
        audience: vec!["agenthub-internal".to_string()],
        issued_at: chrono::Utc::now().timestamp(),
        expires_at: chrono::Utc::now().timestamp() + 600,
        kid: "shared-hs256-test".to_string(),
        payload_digest: None,
    }
    .apply_to_route(&mut route);
    Value::Object(route)
}

async fn seed_team_run(
    state: &crate::state::AppState,
    team_id: &str,
    team_name: &str,
    run_id: &str,
) {
    seed_team_run_with_spec(
        state,
        team_id,
        team_name,
        run_id,
        &json!({
            "entrypoint":"planner",
            "members":[
                {"member_id":"planner"},
                {"member_id":"reviewer"}
            ]
        }),
    )
    .await;
}

async fn seed_team_run_with_spec(
    state: &crate::state::AppState,
    team_id: &str,
    team_name: &str,
    run_id: &str,
    spec: &Value,
) {
    let now = chrono::Utc::now().timestamp();
    sqlx::query(
        r#"
            INSERT INTO team_definitions (
                id,
                name,
                description,
                spec_json,
                owner_user_id,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6)
            "#,
    )
    .bind(team_id)
    .bind(team_name)
    .bind("grpc relay pipeline test team")
    .bind(spec.to_string())
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert team definition");

    sqlx::query(
        r#"
            INSERT INTO team_runs (
                id,
                team_id,
                context_id,
                status,
                input_json,
                created_at
            )
            VALUES (?1, ?2, ?3, 'working', ?4, ?5)
            "#,
    )
    .bind(run_id)
    .bind(team_id)
    .bind(format!("ctx-{run_id}"))
    .bind(json!({"prompt":"validate grpc relay pipeline"}).to_string())
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert team run");
}

async fn seed_safe_path(state: &crate::state::AppState, path: &std::path::Path) {
    sqlx::query(
        r#"
            INSERT OR IGNORE INTO safe_paths (path, created_at)
            VALUES (?1, ?2)
            "#,
    )
    .bind(path.to_string_lossy().to_string())
    .bind(chrono::Utc::now().timestamp())
    .execute(&state.db)
    .await
    .expect("insert safe path");
}

async fn configure_remote_grpc_relay(
    state: &crate::state::AppState,
    cert_dir: &Path,
    node_id: &str,
    addr: SocketAddr,
) {
    sqlx::query(
        r#"
            CREATE TABLE IF NOT EXISTS agent_nodes (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                grpc_target TEXT NOT NULL,
                tls_server_name TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
    )
    .execute(&state.db)
    .await
    .expect("create agent_nodes table");
    state
        .teams
        .configure_internal_grpc_relay(cert_dir, InternalGrpcSecurityMode::Mtls);
    state
        .agents
        .create_agent_node(AgentNodeConfig {
            id: node_id.to_string(),
            name: format!("Node {node_id}"),
            grpc_target: format!("https://{}", addr),
            tls_server_name: Some("localhost".to_string()),
            default_worktree_root: None,
            group_id: None,
        })
        .await
        .expect("create agent node");
}

#[tokio::test]
async fn remote_actor_grpc_pipeline_delivers_and_acks_over_tls() {
    install_rustls_crypto_provider();
    let source_state = build_test_state().await;
    let remote_state = build_test_state().await;
    let team_id = format!("team-{}", Uuid::new_v4());
    let team_name = format!("grpc-relay-team-{}", Uuid::new_v4());
    let run_id = format!("run-{}", Uuid::new_v4());
    seed_team_run(&source_state, &team_id, &team_name, &run_id).await;
    seed_team_run(&remote_state, &team_id, &team_name, &run_id).await;

    let cert_dir = test_cert_dir("relay-pipeline");
    ensure_tls_material(&cert_dir, InternalGrpcSecurityMode::Mtls)
        .expect("generate tls material")
        .expect("tls material");
    let authz = build_authz();
    let access_token = issue_mailbox_token(&authz, &run_id);

    let server =
        spawn_mtls_internal_grpc_server(remote_state.clone(), authz.clone(), cert_dir.clone())
            .await;
    configure_remote_grpc_relay(&source_state, &cert_dir, "node-remote", server.addr).await;

    let route = grpc_relay_route(server.addr, &access_token, "node-source", "node-remote");

    let sent = source_state
        .teams
        .send_actor_message(SendActorMessageInput {
            run_id: &run_id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_NODE_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Remote,
            route: Some(route),
            payload: json!({"type":"chat_message","text":"review this patch"}),
            idempotency_key: Some("grpc-relay-pipeline"),
        })
        .await
        .expect("send remote actor message");
    assert_eq!(sent.status, crate::team::TeamActorMessageStatus::Pending);

    let relay_result = source_state
        .teams
        .relay_remote_messages_once(100, 3, 30)
        .await
        .expect("relay remote messages");
    assert_eq!(relay_result.scanned, 1);
    assert_eq!(relay_result.delivered, 1);
    assert_eq!(relay_result.retried, 0);
    assert_eq!(relay_result.dead_lettered, 0);

    let client = InternalGrpcMailboxClient::connect(InternalGrpcMailboxClientConfig {
        ..mtls_client_config(server.addr, issue_mailbox_token(&authz, &run_id), &cert_dir)
    })
    .await
    .expect("connect grpc mailbox client");

    let inbox = client
        .actor_inbox(ActorInboxRequest {
            run_id: run_id.clone(),
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: None,
        })
        .await
        .expect("list remote inbox");
    assert_eq!(inbox.messages.len(), 1);
    let pending = &inbox.messages[0];
    assert_eq!(pending.from_actor_id, "planner");
    assert_eq!(pending.to_actor_id, "reviewer");
    assert_eq!(pending.channel, "coordination");
    assert_eq!(pending.transport, ActorMessageTransport::Local);
    assert_eq!(pending.payload["text"], "review this patch");
    assert_eq!(pending.status, ActorMessageStatus::Pending);
    assert_eq!(pending.from_peer_id, "node-source");
    assert_eq!(pending.to_peer_id, "main");

    let ack = client
        .actor_ack(agenthub_team_actor::ActorAckRequest {
            run_id: run_id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: pending.message_id,
            ack_token: None,
            result: None,
        })
        .await
        .expect("ack remote inbox message");
    assert_eq!(ack.state, ActorMessageStatus::Delivered);
    assert!(ack.status_changed);
    assert!(ack.acked_at >= ack.message.created_at);

    let delivered_inbox = client
        .actor_inbox(ActorInboxRequest {
            run_id,
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![ActorMessageStatus::Delivered]),
        })
        .await
        .expect("list delivered remote inbox");
    assert_eq!(delivered_inbox.messages.len(), 1);
    assert_eq!(
        delivered_inbox.messages[0].status,
        ActorMessageStatus::Delivered
    );

    server.handle.abort();
}

#[tokio::test]
async fn grpc_actor_send_returns_server_message_for_channel_targets() {
    install_rustls_crypto_provider();
    let state = build_test_state().await;
    let team_id = format!("team-{}", Uuid::new_v4());
    let team_name = format!("grpc-channel-team-{}", Uuid::new_v4());
    let run_id = format!("run-{}", Uuid::new_v4());
    seed_team_run(&state, &team_id, &team_name, &run_id).await;

    let cert_dir = test_cert_dir("grpc-channel-send");
    ensure_tls_material(&cert_dir, InternalGrpcSecurityMode::Mtls)
        .expect("generate tls material")
        .expect("tls material");
    let authz = build_authz();
    let server =
        spawn_mtls_internal_grpc_server(state.clone(), authz.clone(), cert_dir.clone()).await;
    let client = InternalGrpcMailboxClient::connect(mtls_client_config(
        server.addr,
        issue_mailbox_token(&authz, &run_id),
        &cert_dir,
    ))
    .await
    .expect("connect grpc mailbox client");

    let sent = client
        .actor_send(ActorSendRequest {
            run_id: run_id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            to_actor_id: None,
            channel_id: Some("all".to_string()),
            to_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            channel: Some("coordination".to_string()),
            transport: Some(ActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"@reviewer please inspect"
            }),
            idempotency_key: Some("grpc-channel-send-1".to_string()),
        })
        .await
        .expect("send channel message over grpc");
    assert_eq!(sent.state, ActorMessageStatus::Pending);
    assert_eq!(sent.message.to_actor_id, "reviewer");
    assert_eq!(sent.message.channel, "coordination");
    assert_eq!(sent.message.transport, ActorMessageTransport::Local);
    assert_eq!(
        sent.message.payload["delivery_scope"],
        json!("channel_broadcast")
    );
    assert_eq!(
        sent.message.payload["mention_actor_ids"],
        json!(["reviewer"])
    );

    server.handle.abort();
}

#[tokio::test]
async fn grpc_actor_send_rejects_role_alias_target_on_server() {
    install_rustls_crypto_provider();
    let state = build_test_state().await;
    let team_id = format!("team-{}", Uuid::new_v4());
    let team_name = format!("grpc-direct-send-team-{}", Uuid::new_v4());
    let run_id = format!("run-{}", Uuid::new_v4());
    let coordinator_member_id = "595d1ae8-fcbd-4111-b5c7-d446a12c044b";
    let worker_member_id = "c319f933-1358-4418-a111-872304052422";
    seed_team_run_with_spec(
        &state,
        &team_id,
        &team_name,
        &run_id,
        &json!({
            "entrypoint": coordinator_member_id,
            "members": [
                {"member_id": coordinator_member_id, "role": "coordinator"},
                {"member_id": worker_member_id, "role": "worker"}
            ]
        }),
    )
    .await;

    let cert_dir = test_cert_dir("grpc-direct-send-validation");
    ensure_tls_material(&cert_dir, InternalGrpcSecurityMode::Mtls)
        .expect("generate tls material")
        .expect("tls material");
    let authz = build_authz();
    let server =
        spawn_mtls_internal_grpc_server(state.clone(), authz.clone(), cert_dir.clone()).await;
    let client = InternalGrpcMailboxClient::connect(mtls_client_config(
        server.addr,
        issue_mailbox_token(&authz, &run_id),
        &cert_dir,
    ))
    .await
    .expect("connect grpc mailbox client");

    let err = client
        .actor_send(ActorSendRequest {
            run_id: run_id.clone(),
            from_actor_id: worker_member_id.to_string(),
            from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            to_actor_id: Some("coordinator".to_string()),
            channel_id: None,
            to_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            channel: Some("coordination".to_string()),
            transport: Some(ActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"please review"
            }),
            idempotency_key: Some("grpc-direct-send-role-alias".to_string()),
        })
        .await
        .expect_err("role alias target should be rejected by server");

    assert_eq!(err.code, ActorServiceErrorCode::BadRequest);
    assert!(err.message.contains("not a canonical team member_id"));
    assert!(err.message.contains(coordinator_member_id));

    server.handle.abort();
}

#[tokio::test]
async fn grpc_actor_send_allows_remote_target_outside_team_spec() {
    install_rustls_crypto_provider();
    let state = build_test_state().await;
    let team_id = format!("team-{}", Uuid::new_v4());
    let team_name = format!("grpc-remote-send-team-{}", Uuid::new_v4());
    let run_id = format!("run-{}", Uuid::new_v4());
    seed_team_run(&state, &team_id, &team_name, &run_id).await;

    let cert_dir = test_cert_dir("grpc-remote-send-external-target");
    ensure_tls_material(&cert_dir, InternalGrpcSecurityMode::Mtls)
        .expect("generate tls material")
        .expect("tls material");
    let authz = build_authz();
    let server =
        spawn_mtls_internal_grpc_server(state.clone(), authz.clone(), cert_dir.clone()).await;
    let client = InternalGrpcMailboxClient::connect(mtls_client_config(
        server.addr,
        issue_mailbox_token(&authz, &run_id),
        &cert_dir,
    ))
    .await
    .expect("connect grpc mailbox client");

    let sent = client
        .actor_send(ActorSendRequest {
            run_id: run_id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            to_actor_id: Some("remote-reviewer".to_string()),
            channel_id: None,
            to_peer_id: Some(ACTOR_NODE_PEER_ID.to_string()),
            channel: Some("federation".to_string()),
            transport: Some(ActorMessageTransport::Remote),
            route: Some(json!({"endpoint":"https://remote.example/a2a"})),
            payload: json!({
                "type":"chat_message",
                "text":"federated request"
            }),
            idempotency_key: Some("grpc-remote-send-external-target".to_string()),
        })
        .await
        .expect("remote transport should allow external actor target");

    assert_eq!(sent.state, ActorMessageStatus::Pending);
    assert_eq!(sent.message.to_actor_id, "remote-reviewer");
    assert_eq!(sent.message.transport, ActorMessageTransport::Remote);
    assert_eq!(
        sent.message.route,
        Some(json!({"endpoint":"https://remote.example/a2a"}))
    );

    server.handle.abort();
}

#[tokio::test]
async fn grpc_team_task_client_handles_orphan_lists_and_detail_limit() {
    install_rustls_crypto_provider();
    let state = build_test_state().await;
    let team_id = format!("team-{}", Uuid::new_v4());
    let team_name = format!("grpc-team-task-team-{}", Uuid::new_v4());
    let run_id = format!("run-{}", Uuid::new_v4());
    seed_team_run(&state, &team_id, &team_name, &run_id).await;

    let cert_dir = test_cert_dir("grpc-team-task-control");
    ensure_tls_material(&cert_dir, InternalGrpcSecurityMode::Mtls)
        .expect("generate tls material")
        .expect("tls material");
    let authz = build_authz();
    let token = issue_token(
        &authz,
        Some(&run_id),
        vec![
            InternalAction::TeamRead.as_str().to_string(),
            InternalAction::TeamTaskWrite.as_str().to_string(),
        ],
    );
    let server =
        spawn_mtls_internal_grpc_server(state.clone(), authz.clone(), cert_dir.clone()).await;
    let client =
        InternalGrpcMailboxClient::connect(mtls_client_config(server.addr, token, &cert_dir))
            .await
            .expect("connect grpc mailbox client");

    let created = client
        .create_team_task(InternalCreateTeamTaskRequest {
            team_id: &team_id,
            actor_id: "planner",
            title: "Investigate kanban actor cli",
            status: "open",
            priority: "high",
            assigned_member_id: "planner",
            topic: Some("kanban"),
            context: &json!({"source":"grpc-client"}),
        })
        .await
        .expect("create grpc team task");
    let task_id = created["task"]["id"]
        .as_str()
        .expect("created task id")
        .to_string();

    let orphan = client
        .create_team_task(InternalCreateTeamTaskRequest {
            team_id: &team_id,
            actor_id: "planner",
            title: "Legacy orphan task",
            status: "open",
            priority: "high",
            assigned_member_id: "planner",
            topic: Some("legacy"),
            context: &json!({"source":"legacy"}),
        })
        .await
        .expect("create orphan candidate task");
    let orphan_task_id = orphan["task"]["id"]
        .as_str()
        .expect("orphan task id")
        .to_string();
    sqlx::query("DELETE FROM team_conversations WHERE task_id = ?1")
        .bind(&orphan_task_id)
        .execute(&state.db)
        .await
        .expect("delete orphan task conversation");

    let listed = client
        .list_team_tasks(
            "planner",
            &TeamTaskListQuery {
                team_id: None,
                run_id: Some(run_id.clone()),
                limit: 20,
                status: None,
                priority: None,
                task_id: None,
                assigned_member_id: None,
                topic: None,
                include_shared_thread: false,
            },
        )
        .await
        .expect("list grpc team tasks");
    assert!(listed.iter().any(|task| task.id == task_id));
    assert!(listed.iter().any(|task| task.id == orphan_task_id));

    let merge_context = json!({"issue":235});
    let updated = client
        .update_team_task(
            &team_id,
            "planner",
            &task_id,
            InternalTeamTaskPatch {
                status: Some("in_progress"),
                priority: Some("critical"),
                assigned_member_id: Some("reviewer"),
                clear_assigned_member_id: false,
                context_json: None,
                context_merge_json: Some(&merge_context),
                note_kind: Some("decision"),
                note_text: Some("handoff to reviewer for active execution"),
            },
        )
        .await
        .expect("update grpc team task");
    assert_eq!(updated.status, crate::team::TeamTaskStatus::InProgress);
    assert_eq!(updated.priority, crate::team::TeamTaskPriority::Critical);
    assert_eq!(updated.assigned_member_id.as_deref(), Some("reviewer"));
    assert_eq!(updated.context["issue"], json!(235));

    for idx in 0..(TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX + 25) {
        state
            .teams
            .append_task_conversation_message(
                &task_id,
                "planner",
                None,
                "group_chat",
                json!({
                    "kind":"comment",
                    "text": format!("note-{idx}")
                }),
            )
            .await
            .expect("append task conversation message");
    }

    let detail = client
        .get_team_task(
            "planner",
            None,
            Some(&run_id),
            &task_id,
            TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX + 100,
        )
        .await
        .expect("get grpc team task detail");
    assert_eq!(
        detail.recent_messages.len() as i64,
        TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX
    );
    assert_eq!(
        detail
            .recent_messages
            .last()
            .map(|message| &message.payload["text"]),
        Some(&json!(format!(
            "note-{}",
            TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX + 24
        )))
    );

    let note = client
        .append_team_task_note(
            "planner",
            None,
            Some(&run_id),
            &task_id,
            "result",
            "implemented",
        )
        .await
        .expect("append grpc team task note");
    assert_eq!(note.payload["kind"], json!("result"));
    assert_eq!(note.payload["text"], json!("implemented"));

    let detail_after_note: TeamTaskDetailRecord = client
        .get_team_task(
            "planner",
            Some(&team_id),
            None,
            &task_id,
            TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX + 100,
        )
        .await
        .expect("get grpc team task detail after note");
    assert_eq!(
        detail_after_note.recent_messages.len() as i64,
        TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX
    );
    assert_eq!(
        detail_after_note
            .recent_messages
            .last()
            .map(|message| &message.payload["text"]),
        Some(&json!("implemented"))
    );

    server.handle.abort();
}

#[tokio::test]
async fn grpc_team_channel_client_controls_are_wire_compatible() {
    install_rustls_crypto_provider();
    let state = build_test_state().await;
    let team_id = format!("team-{}", Uuid::new_v4());
    let team_name = format!("grpc-team-channel-team-{}", Uuid::new_v4());
    let run_id = format!("run-{}", Uuid::new_v4());
    seed_team_run(&state, &team_id, &team_name, &run_id).await;

    let cert_dir = test_cert_dir("grpc-team-channel-control");
    ensure_tls_material(&cert_dir, InternalGrpcSecurityMode::Mtls)
        .expect("generate tls material")
        .expect("tls material");
    let authz = build_authz();
    let token = issue_token(
        &authz,
        Some(&run_id),
        vec![
            InternalAction::TeamRead.as_str().to_string(),
            InternalAction::TeamTaskWrite.as_str().to_string(),
        ],
    );
    let server =
        spawn_mtls_internal_grpc_server(state.clone(), authz.clone(), cert_dir.clone()).await;
    let client =
        InternalGrpcMailboxClient::connect(mtls_client_config(server.addr, token, &cert_dir))
            .await
            .expect("connect grpc mailbox client");

    let channel = client
        .create_team_channel(&team_id, "planner", " Review ", Some("Review lane"))
        .await
        .expect("create grpc team channel");
    assert_eq!(channel.team_id, team_id);
    assert_eq!(channel.channel_id, "review");
    assert_eq!(channel.description.as_deref(), Some("Review lane"));

    let listed = client
        .list_team_tasks(
            "planner",
            &TeamTaskListQuery {
                team_id: Some(team_id.clone()),
                run_id: None,
                limit: 20,
                status: None,
                priority: None,
                task_id: None,
                assigned_member_id: None,
                topic: None,
                include_shared_thread: false,
            },
        )
        .await
        .expect("list tasks after channel create");
    assert!(
        listed.iter().all(|task| task.id != channel.task_id),
        "channel bootstrap task should stay hidden from grpc task listing"
    );

    let root_message = state
        .teams
        .append_task_conversation_message(
            &channel.task_id,
            "planner",
            None,
            "group_chat",
            json!({
                "type":"chat_message",
                "text":"please review this patch"
            }),
        )
        .await
        .expect("append grpc channel root message");

    let thread = client
        .open_team_thread(
            "planner",
            None,
            Some(&run_id),
            "REVIEW",
            root_message.message_id,
        )
        .await
        .expect("open grpc team thread");
    assert_eq!(thread.team_id, team_id);
    assert_eq!(thread.channel_id, "review");
    assert_eq!(thread.task_id, channel.task_id);
    assert_eq!(thread.conversation_id, channel.conversation_id);
    assert_eq!(thread.root_message_id, root_message.message_id);

    let reply = client
        .reply_team_thread(
            "reviewer",
            None,
            Some(&run_id),
            " review ",
            root_message.message_id,
            "Threaded review note",
        )
        .await
        .expect("reply grpc team thread");
    assert_eq!(reply.thread.thread_id, root_message.message_id.to_string());
    assert_eq!(reply.thread.channel_id, "review");
    assert_eq!(reply.message.route, "team_thread_reply");
    assert_eq!(reply.message.from_actor_id, "reviewer");
    assert_eq!(
        reply.message.payload["thread_root_message_id"],
        json!(root_message.message_id)
    );
    assert_eq!(reply.message.payload["text"], json!("Threaded review note"));

    let deleted = client
        .delete_team_channel(&team_id, "planner", " Review ")
        .await
        .expect("delete grpc team channel");
    assert_eq!(deleted.channel_id, "review");
    assert_eq!(deleted.task_id, channel.task_id);
    assert_eq!(deleted.conversation_id, channel.conversation_id);

    server.handle.abort();
}

#[tokio::test]
async fn grpc_client_resolves_unique_actor_run_scope_from_team_context() {
    install_rustls_crypto_provider();
    let state = build_test_state().await;
    let team_id = format!("team-{}", Uuid::new_v4());
    let team_name = format!("grpc-run-scope-team-{}", Uuid::new_v4());
    let run_id = format!("run-{}", Uuid::new_v4());
    seed_team_run(&state, &team_id, &team_name, &run_id).await;

    let cert_dir = test_cert_dir("grpc-run-scope");
    ensure_tls_material(&cert_dir, InternalGrpcSecurityMode::Mtls)
        .expect("generate tls material")
        .expect("tls material");
    let authz = build_authz();
    let token = issue_token(
        &authz,
        None,
        vec![InternalAction::TeamRead.as_str().to_string()],
    );
    let server =
        spawn_mtls_internal_grpc_server(state.clone(), authz.clone(), cert_dir.clone()).await;
    let client =
        InternalGrpcMailboxClient::connect(mtls_client_config(server.addr, token, &cert_dir))
            .await
            .expect("connect grpc mailbox client");

    let resolved = client
        .resolve_actor_run_scope("planner", Some(&team_id))
        .await
        .expect("resolve actor run scope");
    assert_eq!(resolved.run_id, run_id);
    assert_eq!(resolved.team_id.as_deref(), Some(team_id.as_str()));
    assert_eq!(resolved.source, "team_active_run");

    server.handle.abort();
}

// This is an in-process transport regression test. The blackbox multi-process
// p2p pipeline lives in `tests/distributed_p2p_pipeline.rs`.
#[tokio::test]
async fn bidirectional_actor_grpc_pipeline_relays_seeded_messages_between_in_process_states() {
    install_rustls_crypto_provider();
    let node_a_state = build_test_state().await;
    let node_b_state = build_test_state().await;
    let team_id = format!("team-{}", Uuid::new_v4());
    let team_name = format!("grpc-p2p-team-{}", Uuid::new_v4());
    let run_id = format!("run-{}", Uuid::new_v4());
    let planner_member_id = "planner-a";
    let reviewer_member_id = "reviewer-b";
    let spec = json!({
        "entrypoint": planner_member_id,
        "members": [
            {"member_id": planner_member_id},
            {"member_id": reviewer_member_id}
        ]
    });
    seed_team_run_with_spec(&node_a_state, &team_id, &team_name, &run_id, &spec).await;
    seed_team_run_with_spec(&node_b_state, &team_id, &team_name, &run_id, &spec).await;

    let cert_dir = test_cert_dir("bidirectional-relay-pipeline");
    ensure_tls_material(&cert_dir, InternalGrpcSecurityMode::Mtls)
        .expect("generate tls material")
        .expect("tls material");
    let authz = build_authz();
    let access_token = issue_mailbox_token(&authz, &run_id);

    let node_a_server =
        spawn_mtls_internal_grpc_server(node_a_state.clone(), authz.clone(), cert_dir.clone())
            .await;
    let node_b_server =
        spawn_mtls_internal_grpc_server(node_b_state.clone(), authz.clone(), cert_dir.clone())
            .await;
    configure_remote_grpc_relay(&node_a_state, &cert_dir, "node-b", node_b_server.addr).await;
    configure_remote_grpc_relay(&node_b_state, &cert_dir, "node-a", node_a_server.addr).await;

    let route_to_a = grpc_relay_route(node_a_server.addr, &access_token, "node-b", "node-a");
    let route_to_b = grpc_relay_route(node_b_server.addr, &access_token, "node-a", "node-b");

    node_a_state
        .teams
        .send_actor_message(SendActorMessageInput {
            run_id: &run_id,
            from_actor_id: planner_member_id,
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: reviewer_member_id,
            to_peer_id: ACTOR_NODE_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Remote,
            route: Some(route_to_b.clone()),
            payload: json!({
                "type":"chat_message",
                "text":"node-a-1",
                "sequence":1,
                "correlation_id":"corr-a-1"
            }),
            idempotency_key: Some("p2p-a-1"),
        })
        .await
        .expect("send first seeded node-a message");
    node_a_state
        .teams
        .send_actor_message(SendActorMessageInput {
            run_id: &run_id,
            from_actor_id: planner_member_id,
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: reviewer_member_id,
            to_peer_id: ACTOR_NODE_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Remote,
            route: Some(route_to_b),
            payload: json!({
                "type":"chat_message",
                "text":"node-a-2",
                "sequence":2,
                "correlation_id":"corr-a-2"
            }),
            idempotency_key: Some("p2p-a-2"),
        })
        .await
        .expect("send second seeded node-a message");
    node_b_state
        .teams
        .send_actor_message(SendActorMessageInput {
            run_id: &run_id,
            from_actor_id: reviewer_member_id,
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: planner_member_id,
            to_peer_id: ACTOR_NODE_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Remote,
            route: Some(route_to_a),
            payload: json!({
                "type":"chat_message",
                "text":"node-b-1",
                "sequence":1,
                "correlation_id":"corr-b-1"
            }),
            idempotency_key: Some("p2p-b-1"),
        })
        .await
        .expect("send seeded node-b reply");

    let relay_from_a = node_a_state
        .teams
        .relay_remote_messages_once(100, 3, 30)
        .await
        .expect("relay seeded node-a messages");
    assert_eq!(relay_from_a.scanned, 2);
    assert_eq!(relay_from_a.delivered, 2);
    assert_eq!(relay_from_a.retried, 0);
    assert_eq!(relay_from_a.dead_lettered, 0);

    let relay_from_b = node_b_state
        .teams
        .relay_remote_messages_once(100, 3, 30)
        .await
        .expect("relay seeded node-b reply");
    assert_eq!(relay_from_b.scanned, 1);
    assert_eq!(relay_from_b.delivered, 1);
    assert_eq!(relay_from_b.retried, 0);
    assert_eq!(relay_from_b.dead_lettered, 0);

    let node_a_client = InternalGrpcMailboxClient::connect(mtls_client_config(
        node_a_server.addr,
        issue_mailbox_token(&authz, &run_id),
        &cert_dir,
    ))
    .await
    .expect("connect node-a grpc mailbox client");
    let node_b_client = InternalGrpcMailboxClient::connect(mtls_client_config(
        node_b_server.addr,
        issue_mailbox_token(&authz, &run_id),
        &cert_dir,
    ))
    .await
    .expect("connect node-b grpc mailbox client");

    let node_b_inbox = node_b_client
        .actor_inbox(ActorInboxRequest {
            run_id: run_id.clone(),
            actor_id: reviewer_member_id.to_string(),
            cursor: None,
            limit: Some(20),
            states: None,
        })
        .await
        .expect("list node-b seeded inbox");
    assert_eq!(node_b_inbox.messages.len(), 2);
    assert_eq!(node_b_inbox.messages[0].payload["text"], "node-a-1");
    assert_eq!(node_b_inbox.messages[1].payload["text"], "node-a-2");
    assert_eq!(
        node_b_inbox
            .messages
            .iter()
            .map(|message| message.payload["sequence"].as_i64().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert!(node_b_inbox.messages.iter().all(|message| {
        message.transport == ActorMessageTransport::Local
            && message.route.is_none()
            && message.status == ActorMessageStatus::Pending
    }));
    assert!(
        node_b_inbox
            .messages
            .iter()
            .all(|message| message.from_peer_id == "node-a" && message.to_peer_id == "main")
    );

    let node_a_inbox = node_a_client
        .actor_inbox(ActorInboxRequest {
            run_id: run_id.clone(),
            actor_id: planner_member_id.to_string(),
            cursor: None,
            limit: Some(20),
            states: None,
        })
        .await
        .expect("list node-a seeded inbox");
    assert_eq!(node_a_inbox.messages.len(), 1);
    assert_eq!(node_a_inbox.messages[0].payload["text"], "node-b-1");
    assert_eq!(
        node_a_inbox.messages[0].transport,
        ActorMessageTransport::Local
    );
    assert_eq!(node_a_inbox.messages[0].from_peer_id, "node-b");
    assert_eq!(node_a_inbox.messages[0].to_peer_id, "main");
    assert!(node_a_inbox.messages[0].route.is_none());
    assert_eq!(node_a_inbox.messages[0].status, ActorMessageStatus::Pending);

    for message in &node_b_inbox.messages {
        let ack = node_b_client
            .actor_ack(agenthub_team_actor::ActorAckRequest {
                run_id: run_id.clone(),
                actor_id: reviewer_member_id.to_string(),
                message_id: message.message_id,
                ack_token: None,
                result: None,
            })
            .await
            .expect("ack node-b seeded inbox message");
        assert_eq!(ack.state, ActorMessageStatus::Delivered);
    }

    let node_a_ack = node_a_client
        .actor_ack(agenthub_team_actor::ActorAckRequest {
            run_id: run_id.clone(),
            actor_id: planner_member_id.to_string(),
            message_id: node_a_inbox.messages[0].message_id,
            ack_token: None,
            result: None,
        })
        .await
        .expect("ack node-a seeded inbox message");
    assert_eq!(node_a_ack.state, ActorMessageStatus::Delivered);
    assert!(node_a_ack.status_changed);

    let node_b_delivered = node_b_client
        .actor_inbox(ActorInboxRequest {
            run_id: run_id.clone(),
            actor_id: reviewer_member_id.to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![ActorMessageStatus::Delivered]),
        })
        .await
        .expect("list delivered node-b inbox");
    assert_eq!(node_b_delivered.messages.len(), 2);
    assert!(
        node_b_delivered
            .messages
            .iter()
            .all(|message| message.status == ActorMessageStatus::Delivered)
    );

    let node_a_delivered = node_a_client
        .actor_inbox(ActorInboxRequest {
            run_id,
            actor_id: planner_member_id.to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![ActorMessageStatus::Delivered]),
        })
        .await
        .expect("list delivered node-a inbox");
    assert_eq!(node_a_delivered.messages.len(), 1);
    assert_eq!(
        node_a_delivered.messages[0].status,
        ActorMessageStatus::Delivered
    );

    node_a_server.handle.abort();
    node_b_server.handle.abort();
}

#[tokio::test]
async fn remote_agent_grpc_control_starts_inputs_and_lists_events_over_tls() {
    install_rustls_crypto_provider();
    let remote_state = build_test_state().await;
    let workdir_root =
        std::env::temp_dir().join(format!("agenthub-remote-agent-{}", Uuid::new_v4()));
    let workdir = workdir_root.join("workspace");
    std::fs::create_dir_all(&workdir).expect("create workdir");
    seed_safe_path(&remote_state, &workdir_root).await;

    let cert_dir = test_cert_dir("remote-agent-control");
    ensure_tls_material(&cert_dir, InternalGrpcSecurityMode::Mtls)
        .expect("generate tls material")
        .expect("tls material");
    let authz = build_authz();

    let server =
        spawn_mtls_internal_grpc_server(remote_state.clone(), authz.clone(), cert_dir.clone())
            .await;

    let client = InternalGrpcMailboxClient::connect(mtls_client_config(
        server.addr,
        issue_agent_manage_token(&authz),
        &cert_dir,
    ))
    .await
    .expect("connect grpc control client");

    let agent_id = format!("remote-agent-{}", Uuid::new_v4());
    let agent = client
        .ensure_agent_record(
            &agent_id,
            &AgentConfig {
                name: "Remote Control".to_string(),
                workdir: workdir.to_string_lossy().to_string(),
                command: "/bin/sh".to_string(),
                args: vec![
                    "-lc".to_string(),
                    "printf 'ready\\n'; IFS= read -r line; printf 'echo:%s\\n' \"$line\"; sleep 1"
                        .to_string(),
                ],
                target_node_id: None,
                worktree_mode: WorktreeMode::UseExisting,
                worktree_repo: None,
                worktree_ref: None,
                code_mode: false,
                codex_acp_default_mode: None,
                agent_loop_enabled: false,
                agent_loop_idle_seconds: None,
                agent_loop_prompt: None,
            },
            "manual",
        )
        .await
        .expect("ensure remote agent");
    assert_eq!(agent.id, agent_id);
    assert_eq!(agent.status, crate::agent::AgentStatus::Created);
    assert!(agent.target_node_id.is_none());

    let session_id = client
        .start_managed_agent(&agent_id, None)
        .await
        .expect("start remote agent");
    assert!(!session_id.trim().is_empty());

    let ready_events = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let events = client
                .list_agent_events(&agent_id, 50, Some(&session_id), None)
                .await
                .expect("list ready events");
            if events.iter().any(|event| event.message.contains("ready")) {
                break events;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("ready events timeout");
    assert!(
        ready_events
            .iter()
            .any(|event| event.message.contains("ready"))
    );

    client
        .send_agent_input(&agent_id, "ping", None, Some(&session_id))
        .await
        .expect("send agent input");

    let echoed_events = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let events = client
                .list_agent_events(&agent_id, 100, Some(&session_id), None)
                .await
                .expect("list echoed events");
            if events
                .iter()
                .any(|event| event.message.contains("echo:ping"))
            {
                break events;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("echo events timeout");
    assert!(
        echoed_events
            .iter()
            .any(|event| event.message.contains("echo:ping"))
    );

    client
        .stop_managed_agent(&agent_id)
        .await
        .expect("stop remote agent");
    let stopped_agent = remote_state
        .agents
        .get_agent(&agent_id)
        .await
        .expect("load stopped remote agent");
    assert_eq!(stopped_agent.status, crate::agent::AgentStatus::Stopped);

    server.handle.abort();
}
