use std::fs::{self, File};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Once;
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow};
use chrono::Utc;
use jwt_simple::algorithms::MACLike;
use jwt_simple::prelude::{Claims, Duration as JwtDuration, HS256Key};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tonic::Request;
use tonic::metadata::MetadataValue;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};
use uuid::Uuid;

mod internal_proto {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/internal/proto/agenthub.internal.v1.rs"
    ));
}

use internal_proto::team_internal_control_client::TeamInternalControlClient;
use internal_proto::{AckActorMessageRequest, ListActorInboxRequest};

const TEST_INTERNAL_ISSUER: &str = "agenthub";
const TEST_INTERNAL_AUDIENCE: &str = "agenthub-internal";
const TEST_INTERNAL_ROLE: &str = "leader";
const TEST_LOG_ROOT: &str = "target/p2p-pipeline-logs";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InternalAccessClaims {
    role: String,
    cluster_id: Option<String>,
    source_node_id: Option<String>,
    actor_id: Option<String>,
    run_id: Option<String>,
    permissions: Vec<String>,
    scope: Vec<String>,
    issuer: Option<String>,
    audience: Option<String>,
    kid: Option<String>,
}

#[derive(Debug)]
struct SpawnedNode {
    name: String,
    home_dir: PathBuf,
    web_addr: SocketAddr,
    grpc_addr: SocketAddr,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    child: Option<Child>,
}

struct RemoteMessageSeed<'a> {
    run_id: &'a str,
    from_actor_id: &'a str,
    from_peer_id: &'a str,
    to_actor_id: &'a str,
    to_peer_id: &'a str,
    payload: Value,
    route_json: &'a Value,
    idempotency_key: &'a str,
    created_at: i64,
}

impl SpawnedNode {
    fn db_path(&self) -> PathBuf {
        self.home_dir.join(".agenthub/agenthub.db")
    }

    fn base_url(&self) -> String {
        format!("http://{}", self.web_addr)
    }
}

impl Drop for SpawnedNode {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn blackbox_distributed_p2p_pipeline_relays_and_acks_over_real_nodes() -> anyhow::Result<()> {
    install_rustls_crypto_provider();
    let test_id = format!("distributed-p2p-{}", Uuid::new_v4());
    let log_root = PathBuf::from(TEST_LOG_ROOT).join(&test_id);
    fs::create_dir_all(&log_root).context("create p2p log root")?;

    let shared_secret = format!("agenthub-p2p-secret-{test_id}");
    let cluster_root = log_root.join("cluster");
    let shared_cert_dir = cluster_root.join("shared-internal-grpc");
    let node_a_home = cluster_root.join("node-a-home");
    let node_b_home = cluster_root.join("node-b-home");
    fs::create_dir_all(&shared_cert_dir).context("create shared cert dir")?;
    fs::create_dir_all(node_a_home.join(".agenthub")).context("create node-a home")?;
    fs::create_dir_all(node_b_home.join(".agenthub")).context("create node-b home")?;

    let node_a = spawn_node(
        "node-a",
        &node_a_home,
        &shared_cert_dir,
        &shared_secret,
        &log_root,
    )
    .context("spawn node-a")?;
    wait_for_health(&node_a).await?;
    let node_b = spawn_node(
        "node-b",
        &node_b_home,
        &shared_cert_dir,
        &shared_secret,
        &log_root,
    )
    .context("spawn node-b")?;
    wait_for_health(&node_b).await?;

    let test_result: anyhow::Result<()> = async {
        let node_a_db = open_sqlite(&node_a.db_path()).await?;
        let node_b_db = open_sqlite(&node_b.db_path()).await?;

        let team_id = format!("team-{}", Uuid::new_v4());
        let team_name = format!("distributed-p2p-team-{}", Uuid::new_v4());
        let run_id = format!("run-{}", Uuid::new_v4());

        seed_team_run(&node_a_db, &team_id, &team_name, &run_id).await?;
        seed_team_run(&node_b_db, &team_id, &team_name, &run_id).await?;
        insert_agent_node(&node_a_db, "node-b", node_b.grpc_addr).await?;
        insert_agent_node(&node_b_db, "node-a", node_a.grpc_addr).await?;

        let mailbox_token = issue_mailbox_token(&shared_secret, &run_id)?;
        let route_to_a = grpc_route_json(node_a.grpc_addr, &mailbox_token, "node-b", "node-a");
        let route_to_b = grpc_route_json(node_b.grpc_addr, &mailbox_token, "node-a", "node-b");
        let now = Utc::now().timestamp();

        insert_remote_message(
            &node_a_db,
            RemoteMessageSeed {
                run_id: &run_id,
                from_actor_id: "planner-a",
                from_peer_id: "main",
                to_actor_id: "reviewer-b",
                to_peer_id: "node",
                payload: json!({
                    "type": "chat_message",
                    "text": "node-a-1",
                    "sequence": 1,
                    "correlation_id": "corr-a-1"
                }),
                route_json: &route_to_b,
                idempotency_key: "p2p-a-1",
                created_at: now,
            },
        )
        .await?;
        insert_remote_message(
            &node_a_db,
            RemoteMessageSeed {
                run_id: &run_id,
                from_actor_id: "planner-a",
                from_peer_id: "main",
                to_actor_id: "reviewer-b",
                to_peer_id: "node",
                payload: json!({
                    "type": "chat_message",
                    "text": "node-a-2",
                    "sequence": 2,
                    "correlation_id": "corr-a-2"
                }),
                route_json: &route_to_b,
                idempotency_key: "p2p-a-2",
                created_at: now + 1,
            },
        )
        .await?;
        insert_remote_message(
            &node_a_db,
            RemoteMessageSeed {
                run_id: &run_id,
                from_actor_id: "planner-a",
                from_peer_id: "main",
                to_actor_id: "reviewer-b",
                to_peer_id: "node",
                payload: json!({
                    "type": "chat_message",
                    "text": "node-a-3",
                    "sequence": 3,
                    "correlation_id": "corr-a-3"
                }),
                route_json: &route_to_b,
                idempotency_key: "p2p-a-3",
                created_at: now + 2,
            },
        )
        .await?;
        insert_remote_message(
            &node_b_db,
            RemoteMessageSeed {
                run_id: &run_id,
                from_actor_id: "reviewer-b",
                from_peer_id: "main",
                to_actor_id: "planner-a",
                to_peer_id: "node",
                payload: json!({
                    "type": "chat_message",
                    "text": "node-b-1",
                    "sequence": 1,
                    "correlation_id": "corr-b-1"
                }),
                route_json: &route_to_a,
                idempotency_key: "p2p-b-1",
                created_at: now + 3,
            },
        )
        .await?;
        insert_remote_message(
            &node_b_db,
            RemoteMessageSeed {
                run_id: &run_id,
                from_actor_id: "reviewer-b",
                from_peer_id: "main",
                to_actor_id: "planner-a",
                to_peer_id: "node",
                payload: json!({
                    "type": "chat_message",
                    "text": "node-b-2",
                    "sequence": 2,
                    "correlation_id": "corr-b-2"
                }),
                route_json: &route_to_a,
                idempotency_key: "p2p-b-2",
                created_at: now + 4,
            },
        )
        .await?;

        wait_for_remote_delivery_count(&node_a_db, &run_id, 3, "node-a source remote delivery")
            .await?;
        wait_for_remote_delivery_count(&node_b_db, &run_id, 2, "node-b source remote delivery")
            .await?;

        let node_b_inbox = wait_for_inbox_texts(
            node_b.grpc_addr,
            &shared_cert_dir,
            &mailbox_token,
            &run_id,
            "reviewer-b",
            3,
            "node-b inbox",
        )
        .await?;
        let node_a_inbox = wait_for_inbox_texts(
            node_a.grpc_addr,
            &shared_cert_dir,
            &mailbox_token,
            &run_id,
            "planner-a",
            2,
            "node-a inbox",
        )
        .await?;

        assert_eq!(
            payload_texts(&node_b_inbox)?,
            vec!["node-a-1", "node-a-2", "node-a-3"]
        );
        assert_eq!(payload_sequences(&node_b_inbox)?, vec![1, 2, 3]);
        assert!(
            node_b_inbox
                .iter()
                .all(|message| message.from_peer_id == "node-a" && message.to_peer_id == "main")
        );
        assert_eq!(payload_texts(&node_a_inbox)?, vec!["node-b-1", "node-b-2"]);
        assert_eq!(payload_sequences(&node_a_inbox)?, vec![1, 2]);
        assert!(
            node_a_inbox
                .iter()
                .all(|message| message.from_peer_id == "node-b" && message.to_peer_id == "main")
        );

        ack_messages(
            node_b.grpc_addr,
            &shared_cert_dir,
            &mailbox_token,
            &run_id,
            "reviewer-b",
            &node_b_inbox,
        )
        .await?;
        ack_messages(
            node_a.grpc_addr,
            &shared_cert_dir,
            &mailbox_token,
            &run_id,
            "planner-a",
            &node_a_inbox,
        )
        .await?;

        wait_for_local_delivery_count(&node_b_db, &run_id, "reviewer-b", 3, "node-b local ack")
            .await?;
        wait_for_local_delivery_count(&node_a_db, &run_id, "planner-a", 2, "node-a local ack")
            .await?;

        assert_eq!(
            count_null_local_routes(&node_b_db, &run_id, "reviewer-b").await?,
            3
        );
        assert_eq!(
            count_null_local_routes(&node_a_db, &run_id, "planner-a").await?,
            2
        );
        assert_eq!(count_dead_letters(&node_a_db, &run_id).await?, 0);
        assert_eq!(count_dead_letters(&node_b_db, &run_id).await?, 0);

        Ok(())
    }
    .await;

    if test_result.is_err() {
        eprintln!(
            "p2p pipeline logs: node-a stdout={} stderr={}, node-b stdout={} stderr={}",
            node_a.stdout_path.display(),
            node_a.stderr_path.display(),
            node_b.stdout_path.display(),
            node_b.stderr_path.display()
        );
    }

    test_result
}

fn spawn_node(
    name: &str,
    home_dir: &Path,
    shared_cert_dir: &Path,
    shared_secret: &str,
    log_root: &Path,
) -> anyhow::Result<SpawnedNode> {
    let web_addr = reserve_addr()?;
    let grpc_addr = reserve_addr()?;
    let config_dir = home_dir.join(".agenthub");
    let worktree_root = home_dir.join("worktrees");
    let push_keys_path = home_dir.join(".agenthub/push-vapid.json");
    fs::create_dir_all(&config_dir).with_context(|| format!("create {name} config dir"))?;
    fs::create_dir_all(&worktree_root).with_context(|| format!("create {name} worktree root"))?;

    let config = format!(
        "[server]\nlisten = \"{web_addr}\"\n\n\
         [web]\n\
         rp_id = \"localhost\"\n\
         rp_origin = \"http://localhost:{web_port}\"\n\
         rp_name = \"AgentHub {name}\"\n\n\
         [push]\n\
         subject = \"mailto:test@example.com\"\n\
         keys_path = \"{push_keys_path}\"\n\n\
         [worktree]\n\
         default_root = \"{worktree_root}\"\n\n\
         [internal_grpc]\n\
         enabled = true\n\
         listen = \"{grpc_addr}\"\n\n\
         [internal_grpc.security]\n\
         mode = \"mtls\"\n\
         cert_dir = \"{shared_cert_dir}\"\n\n\
         [internal_grpc.auth]\n\
         shared_secret = \"{shared_secret}\"\n\
         issuer = \"{TEST_INTERNAL_ISSUER}\"\n\
         audience = \"{TEST_INTERNAL_AUDIENCE}\"\n\n\
         [internal_grpc.bootstrap]\n\
         token = \"bootstrap-token\"\n",
        push_keys_path = push_keys_path.display(),
        worktree_root = worktree_root.display(),
        shared_cert_dir = shared_cert_dir.display(),
        web_port = web_addr.port(),
    );
    fs::write(config_dir.join("config.toml"), config)
        .with_context(|| format!("write {name} config.toml"))?;

    let stdout_path = log_root.join(format!("{name}.stdout.log"));
    let stderr_path = log_root.join(format!("{name}.stderr.log"));
    let stdout = File::create(&stdout_path).with_context(|| format!("create {name} stdout log"))?;
    let stderr = File::create(&stderr_path).with_context(|| format!("create {name} stderr log"))?;
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let binary_path = resolve_agenthub_binary_path();

    let child = Command::new(&binary_path)
        .current_dir(&repo_root)
        .env("HOME", home_dir)
        .env("RUST_LOG", "info")
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .with_context(|| format!("spawn {name} agenthub binary at {}", binary_path.display()))?;

    Ok(SpawnedNode {
        name: name.to_string(),
        home_dir: home_dir.to_path_buf(),
        web_addr,
        grpc_addr,
        stdout_path,
        stderr_path,
        child: Some(child),
    })
}

fn resolve_agenthub_binary_path() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_agenthub") {
        return PathBuf::from(path);
    }

    let current = std::env::current_exe().expect("resolve current test executable path");
    current
        .parent()
        .and_then(|parent| parent.parent())
        .map(|dir| dir.join(format!("agenthub{}", std::env::consts::EXE_SUFFIX)))
        .expect("resolve target dir for agenthub binary")
}

fn reserve_addr() -> anyhow::Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0").context("bind localhost:0")?;
    let addr = listener.local_addr().context("read local addr")?;
    drop(listener);
    Ok(addr)
}

async fn wait_for_health(node: &SpawnedNode) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .context("build health client")?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let health_url = format!("{}/health", node.base_url());
    let mut last_error = String::new();
    while Instant::now() < deadline {
        match client.get(&health_url).send().await {
            Ok(response) if response.status() == StatusCode::OK => return Ok(()),
            Ok(response) => {
                last_error = format!("unexpected status {}", response.status());
            }
            Err(err) => {
                last_error = err.to_string();
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(anyhow!(
        "{} did not become healthy via {}: {} (stdout: {}, stderr: {})",
        node.name,
        health_url,
        last_error,
        node.stdout_path.display(),
        node.stderr_path.display()
    ))
}

async fn open_sqlite(path: &Path) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await
        .with_context(|| format!("connect sqlite at {}", path.display()))
}

async fn seed_team_run(
    db: &SqlitePool,
    team_id: &str,
    team_name: &str,
    run_id: &str,
) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
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
    .bind("distributed p2p pipeline test team")
    .bind(
        json!({
            "entrypoint": "planner-a",
            "members": [
                {"member_id": "planner-a"},
                {"member_id": "reviewer-b"}
            ]
        })
        .to_string(),
    )
    .bind(now)
    .bind(now)
    .execute(db)
    .await
    .context("insert team definition")?;

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
    .bind(json!({"prompt": "validate distributed p2p pipeline"}).to_string())
    .bind(now)
    .execute(db)
    .await
    .context("insert team run")?;

    Ok(())
}

async fn insert_remote_message(db: &SqlitePool, seed: RemoteMessageSeed<'_>) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO team_actor_messages (
            run_id,
            from_actor_id,
            from_peer_id,
            to_actor_id,
            to_peer_id,
            channel,
            transport,
            route_json,
            payload_json,
            idempotency_key,
            status,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, 'coordination', 'remote', ?6, ?7, ?8, 'pending', ?9)
        "#,
    )
    .bind(seed.run_id)
    .bind(seed.from_actor_id)
    .bind(seed.from_peer_id)
    .bind(seed.to_actor_id)
    .bind(seed.to_peer_id)
    .bind(seed.route_json.to_string())
    .bind(seed.payload.to_string())
    .bind(seed.idempotency_key)
    .bind(seed.created_at)
    .execute(db)
    .await
    .with_context(|| format!("insert remote message {}", seed.idempotency_key))?;
    Ok(())
}

fn issue_mailbox_token(shared_secret: &str, run_id: &str) -> anyhow::Result<String> {
    let key = HS256Key::from_bytes(shared_secret.as_bytes());
    let claims = Claims::with_custom_claims(
        InternalAccessClaims {
            role: TEST_INTERNAL_ROLE.to_string(),
            cluster_id: Some(TEST_INTERNAL_ISSUER.to_string()),
            source_node_id: Some("main".to_string()),
            actor_id: None,
            run_id: Some(run_id.to_string()),
            permissions: vec![
                "team:message:send".to_string(),
                "team:inbox:list".to_string(),
                "team:message:ack".to_string(),
            ],
            scope: vec!["node:p2p".to_string()],
            issuer: Some(TEST_INTERNAL_ISSUER.to_string()),
            audience: Some(TEST_INTERNAL_AUDIENCE.to_string()),
            kid: Some("shared-hs256-blackbox".to_string()),
        },
        JwtDuration::from_secs(600),
    );
    key.authenticate(claims)
        .map_err(|err| anyhow!("issue mailbox token: {err}"))
}

fn grpc_route_json(
    target: SocketAddr,
    access_token: &str,
    source_node_id: &str,
    target_node_id: &str,
) -> Value {
    json!({
        "kind": "grpc",
        "grpc_target": format!("https://{}", target),
        "access_token": access_token,
        "tls_server_name": "localhost",
        "cluster_id": TEST_INTERNAL_ISSUER,
        "source_node_id": source_node_id,
        "target_node_id": target_node_id,
        "scope": ["node:p2p"],
        "audience": [TEST_INTERNAL_AUDIENCE],
        "issued_at": Utc::now().timestamp(),
        "expires_at": Utc::now().timestamp() + 600,
        "kid": "shared-hs256-blackbox",
    })
}

async fn insert_agent_node(
    db: &SqlitePool,
    node_id: &str,
    grpc_addr: SocketAddr,
) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    sqlx::query(
        r#"
        INSERT INTO agent_nodes (
            id,
            name,
            grpc_target,
            tls_server_name,
            created_at,
            updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(node_id)
    .bind(format!("Node {node_id}"))
    .bind(format!("https://{}", grpc_addr))
    .bind("localhost")
    .bind(now)
    .bind(now)
    .execute(db)
    .await
    .with_context(|| format!("insert agent node {node_id}"))?;
    Ok(())
}

async fn wait_for_remote_delivery_count(
    db: &SqlitePool,
    run_id: &str,
    expected: i64,
    label: &str,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(25);
    while Instant::now() < deadline {
        let delivered = count_remote_messages(db, run_id, "delivered").await?;
        if delivered == expected {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(anyhow!(
        "{label} did not reach delivered={expected} in time"
    ))
}

async fn wait_for_local_delivery_count(
    db: &SqlitePool,
    run_id: &str,
    actor_id: &str,
    expected: i64,
    label: &str,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let delivered = count_local_messages(db, run_id, actor_id, "delivered").await?;
        if delivered == expected {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(anyhow!(
        "{label} did not reach delivered={expected} for actor {actor_id} in time"
    ))
}

async fn wait_for_inbox_texts(
    grpc_addr: SocketAddr,
    cert_dir: &Path,
    token: &str,
    run_id: &str,
    actor_id: &str,
    expected_len: usize,
    label: &str,
) -> anyhow::Result<Vec<internal_proto::ActorMessage>> {
    let deadline = Instant::now() + Duration::from_secs(25);
    let mut last_count = 0usize;
    while Instant::now() < deadline {
        let mut client = connect_internal_client(grpc_addr, cert_dir).await?;
        let messages = list_actor_inbox(&mut client, token, run_id, actor_id, false).await?;
        last_count = messages.len();
        if messages.len() == expected_len {
            return Ok(messages);
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    Err(anyhow!(
        "{label} did not reach inbox size {expected_len}; last_count={last_count}"
    ))
}

async fn ack_messages(
    grpc_addr: SocketAddr,
    cert_dir: &Path,
    token: &str,
    run_id: &str,
    actor_id: &str,
    messages: &[internal_proto::ActorMessage],
) -> anyhow::Result<()> {
    let mut client = connect_internal_client(grpc_addr, cert_dir).await?;
    for message in messages {
        let request = auth_request(
            AckActorMessageRequest {
                run_id: run_id.to_string(),
                actor_id: actor_id.to_string(),
                message_id: message.message_id,
            },
            token,
        )?;
        let response = client
            .ack_actor_message(request)
            .await
            .with_context(|| format!("ack {} for actor {}", message.message_id, actor_id))?
            .into_inner();
        let delivered = response
            .message
            .as_ref()
            .ok_or_else(|| anyhow!("ack response missing message"))?;
        if delivered.status != "delivered" {
            return Err(anyhow!(
                "expected delivered ack status for message {}, got {}",
                delivered.message_id,
                delivered.status
            ));
        }
    }
    Ok(())
}

async fn connect_internal_client(
    grpc_addr: SocketAddr,
    cert_dir: &Path,
) -> anyhow::Result<TeamInternalControlClient<Channel>> {
    let ca_cert = fs::read(cert_dir.join("ca-cert.pem")).context("read ca cert")?;
    let client_cert = fs::read(cert_dir.join("client-cert.pem")).context("read client cert")?;
    let client_key = fs::read(cert_dir.join("client-key.pem")).context("read client key")?;
    let tls = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(ca_cert))
        .domain_name("localhost")
        .identity(Identity::from_pem(client_cert, client_key));
    let channel = Endpoint::from_shared(format!("https://{}", grpc_addr))
        .context("build grpc endpoint")?
        .tls_config(tls)
        .context("configure grpc tls")?
        .connect()
        .await
        .context("connect grpc client")?;
    Ok(TeamInternalControlClient::new(channel))
}

fn install_rustls_crypto_provider() {
    static INSTALL_RUSTLS_PROVIDER: Once = Once::new();
    INSTALL_RUSTLS_PROVIDER.call_once(|| {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

async fn list_actor_inbox(
    client: &mut TeamInternalControlClient<Channel>,
    token: &str,
    run_id: &str,
    actor_id: &str,
    include_delivered: bool,
) -> anyhow::Result<Vec<internal_proto::ActorMessage>> {
    let request = auth_request(
        ListActorInboxRequest {
            run_id: run_id.to_string(),
            actor_id: actor_id.to_string(),
            limit: 64,
            after_message_id: 0,
            include_delivered,
        },
        token,
    )?;
    let response = client
        .list_actor_inbox(request)
        .await
        .with_context(|| format!("list inbox for actor {actor_id}"))?
        .into_inner();
    Ok(response.messages)
}

fn auth_request<T>(message: T, token: &str) -> anyhow::Result<Request<T>> {
    let mut request = Request::new(message);
    let value = MetadataValue::try_from(format!("Bearer {token}"))
        .map_err(|err| anyhow!("build authorization metadata: {err}"))?;
    request.metadata_mut().insert("authorization", value);
    Ok(request)
}

fn payload_texts(messages: &[internal_proto::ActorMessage]) -> anyhow::Result<Vec<String>> {
    messages
        .iter()
        .map(|message| {
            let payload: Value = serde_json::from_str(&message.payload_json)
                .with_context(|| format!("parse payload for message {}", message.message_id))?;
            payload["text"]
                .as_str()
                .map(|value| value.to_string())
                .ok_or_else(|| {
                    anyhow!(
                        "payload for message {} missing string field 'text'",
                        message.message_id
                    )
                })
        })
        .collect()
}

fn payload_sequences(messages: &[internal_proto::ActorMessage]) -> anyhow::Result<Vec<i64>> {
    messages
        .iter()
        .map(|message| {
            let payload: Value = serde_json::from_str(&message.payload_json)
                .with_context(|| format!("parse payload for message {}", message.message_id))?;
            payload["sequence"].as_i64().ok_or_else(|| {
                anyhow!(
                    "payload for message {} missing integer field 'sequence'",
                    message.message_id
                )
            })
        })
        .collect()
}

async fn count_remote_messages(db: &SqlitePool, run_id: &str, status: &str) -> anyhow::Result<i64> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM team_actor_messages
        WHERE run_id = ?1 AND transport = 'remote' AND status = ?2
        "#,
    )
    .bind(run_id)
    .bind(status)
    .fetch_one(db)
    .await
    .context("count remote messages")?;
    Ok(count)
}

async fn count_local_messages(
    db: &SqlitePool,
    run_id: &str,
    actor_id: &str,
    status: &str,
) -> anyhow::Result<i64> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM team_actor_messages
        WHERE run_id = ?1
          AND transport = 'local'
          AND to_actor_id = ?2
          AND status = ?3
        "#,
    )
    .bind(run_id)
    .bind(actor_id)
    .bind(status)
    .fetch_one(db)
    .await
    .context("count local messages")?;
    Ok(count)
}

async fn count_null_local_routes(
    db: &SqlitePool,
    run_id: &str,
    actor_id: &str,
) -> anyhow::Result<i64> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM team_actor_messages
        WHERE run_id = ?1
          AND transport = 'local'
          AND to_actor_id = ?2
          AND route_json IS NULL
        "#,
    )
    .bind(run_id)
    .bind(actor_id)
    .fetch_one(db)
    .await
    .context("count null local routes")?;
    Ok(count)
}

async fn count_dead_letters(db: &SqlitePool, run_id: &str) -> anyhow::Result<i64> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM team_actor_messages
        WHERE run_id = ?1
          AND (status = 'dead_letter' OR dead_letter_at IS NOT NULL)
        "#,
    )
    .bind(run_id)
    .fetch_one(db)
    .await
    .context("count dead letters")?;
    Ok(count)
}
