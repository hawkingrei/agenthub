use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use agenthub_mcp::{
    bridge::McpProxyBinding,
    http::McpHttpTransport,
    policy::{McpBinding, McpPolicyError, TrustedReplayPolicy},
    stdio::{read_message, write_message},
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use futures::StreamExt;
use tokio::{
    io::{AsyncReadExt, BufReader},
    sync::Notify,
};

use super::*;
use crate::internal::proto::agenthub::internal::v1::{
    OpenMcpProxyRequest, team_internal_control_server::TeamInternalControlServer,
};
use crate::loop_credentials::{
    LOOP_CREDENTIAL_FILE_ENV, LoopCredentialEnvelope, LoopCredentialFile,
};

mod batch;
mod bootstrap;
mod budget;
mod continuation;
mod discovery;
mod listener;
mod subscription;
mod task;

struct Upstream {
    db: sqlx::SqlitePool,
    callback: Notify,
    initialized: AtomicBool,
    calls: Mutex<Vec<Value>>,
    hold_writes: AtomicBool,
    write_received: Notify,
    write_release: Notify,
    listen_enabled: AtomicBool,
    resume_writes: AtomicBool,
    listened: Notify,
    listener_replied: Notify,
    gets: Mutex<Vec<Option<String>>>,
    deleted: AtomicBool,
    expire_stream: AtomicBool,
    mrtr: AtomicBool,
    mrtr_drop_response: AtomicBool,
    tasks: AtomicBool,
    task_inputs_answered: AtomicBool,
}

async fn handler(
    State(upstream): State<Arc<Upstream>>,
    headers: HeaderMap,
    Json(message): Json<Value>,
) -> Response {
    assert_eq!(headers["authorization"], "Bearer upstream-secret");
    upstream.calls.lock().unwrap().push(message.clone());
    if message.is_array() {
        return batch::handle(upstream, headers, message).await;
    }
    if message.get("method").is_none() {
        assert_eq!(headers["mcp-session-id"], "private-upstream-session");
        assert!(message["id"] == "roots-1" || message["id"] == "roots-listener");
        assert_eq!(message["result"]["roots"], json!([]));
        if message["id"] == "roots-listener" {
            upstream.listener_replied.notify_one();
        } else {
            upstream.callback.notify_one();
        }
        return StatusCode::ACCEPTED.into_response();
    }
    match message["method"].as_str().unwrap() {
        "tasks/get" | "tasks/cancel" | "tasks/update" => task::respond(&upstream, &message).await,
        "subscriptions/listen" => task::subscribe(upstream, message).await,
        "server/discover" => {
            assert_eq!(headers["mcp-protocol-version"], "2026-07-28");
            assert_eq!(headers["mcp-method"], "server/discover");
            assert!(headers.get("mcp-session-id").is_none());
            if message.pointer("/params/_meta/io.modelcontextprotocol~1clientInfo/name")
                == Some(&json!("legacy-probe"))
            {
                return (StatusCode::NOT_FOUND, Json(json!({"jsonrpc":"2.0","id":message["id"],
                    "error":{"code":-32601,"message":"Method not found","data":{"fallback":"initialize"}}}))).into_response();
            }
            Json(json!({"jsonrpc":"2.0","id":message["id"],"result":discovery::result()}))
                .into_response()
        }
        "initialize" => {
            let mut callback = json!({"jsonrpc":"2.0","id":"roots-1","method":"roots/list"});
            if message["params"]["protocolVersion"] == "2025-03-26" {
                callback = json!([callback]);
            }
            let response = json!({"jsonrpc":"2.0","id":message["id"],"result":{
                "protocolVersion":message["params"]["protocolVersion"],"capabilities":{"tools":{"listChanged":true}},
                "serverInfo":{"name":"upstream","version":"1"},"extension":{"preserved":true}
            }});
            let first = futures::stream::once(async move {
                Ok::<_, std::io::Error>(format!("data: {callback}\n\n"))
            });
            let second = futures::stream::once(async move {
                upstream.callback.notified().await;
                Ok::<_, std::io::Error>(format!("data: {response}\n\n"))
            });
            axum::http::Response::builder()
                .header("content-type", "text/event-stream")
                .header("mcp-session-id", "private-upstream-session")
                .body(axum::body::Body::from_stream(first.chain(second)))
                .unwrap()
        }
        "notifications/initialized" => {
            tokio::time::sleep(Duration::from_millis(20)).await;
            upstream.initialized.store(true, Ordering::Release);
            StatusCode::ACCEPTED.into_response()
        }
        "tools/list" => {
            assert!(
                headers
                    .get("mcp-protocol-version")
                    .is_some_and(|version| version == "2026-07-28")
                    || upstream.initialized.load(Ordering::Acquire),
                "tool discovery overtook initialized delivery"
            );
            let mut result = if message.pointer("/params/cursor").is_some() {
                assert_eq!(message["params"]["cursor"], "page-2");
                json!({"tools":[{"name":"read","inputSchema":{"type":"object","properties":{}},"extension":"second-page"}]})
            } else {
                json!({"tools":[{"name":"write","inputSchema":{"type":"object","properties":{"body":{"type":"string"},"space_id":{"type":"string"}}},"extension":"first-page"}],"nextCursor":"page-2"})
            };
            if upstream.mrtr.load(Ordering::Acquire) {
                result["tools"][0]["inputSchema"]["properties"]["request_id"] =
                    json!({"type":"string"});
            }
            let response = json!({"jsonrpc":"2.0","id":message["id"],"result":result});
            if message["id"] == "json-batched-list" {
                return Json(json!([response])).into_response();
            }
            if message["id"] == "batched-list" {
                let batch =
                    json!([response, {"jsonrpc":"2.0","id":"roots-1","method":"roots/list"}]);
                return (
                    [("content-type", "text/event-stream")],
                    format!("data: {batch}\n\n"),
                )
                    .into_response();
            }
            Json(response).into_response()
        }
        "tools/call" => {
            assert_eq!(message["params"]["arguments"]["space_id"], "space-a");
            let sent: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM mcp_operation_attempts WHERE status = 'sent'",
            )
            .fetch_one(&upstream.db)
            .await
            .unwrap();
            assert_eq!(sent, 1);
            if upstream.tasks.load(Ordering::Acquire) {
                return task::respond(&upstream, &message).await;
            }
            if upstream.mrtr.load(Ordering::Acquire) {
                return continuation::respond(&upstream, &headers, &message).await;
            }
            upstream.write_received.notify_one();
            if upstream.resume_writes.load(Ordering::Acquire) {
                return (
                    [("content-type", "text/event-stream")],
                    "id: private-write-cursor\nretry: 10\ndata:\n\n",
                )
                    .into_response();
            }
            if upstream.hold_writes.load(Ordering::Acquire) {
                upstream.write_release.notified().await;
            }
            let progress = json!({"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":"progress-1","progress":1}});
            let response = json!({"jsonrpc":"2.0","id":message["id"],"result":{"content":[{"type":"text","text":"private-tool-result"}],"extension":{"preserved":true}}});
            (
                [("content-type", "text/event-stream")],
                format!("data: {progress}\n\ndata: {response}\n\n"),
            )
                .into_response()
        }
        _ => panic!("unexpected fake upstream method"),
    }
}

fn signed_token(
    authz: &InternalAuthz,
    reservation: &LoopReservation,
    run_id: &str,
    permissions: Vec<String>,
) -> String {
    authz
        .issue_loop_access_token(
            NodeCredentialRequest {
                source_node_id: "main".into(),
                role: "worker".into(),
                actor_id: Some(reservation.actor_id.clone()),
                run_id: Some(run_id.into()),
                permissions,
                scope: Vec::new(),
                audience: Vec::new(),
                ttl_seconds: 600,
            },
            LoopExecutionClaims {
                activation_id: reservation.activation_id.clone().unwrap(),
                generation: reservation.generation,
            },
        )
        .unwrap()
        .access_token
}

struct Harness {
    state: crate::state::AppState,
    service: TeamInternalControlService,
    authz: InternalAuthz,
    run: crate::team::TeamRunRecord,
    reservation: LoopReservation,
    journal: agenthub_db::mcp_operations::McpOperationStore,
    upstream: Arc<Upstream>,
    binding: Arc<McpProxyBinding>,
    http: tokio::task::JoinHandle<()>,
}

async fn setup() -> Harness {
    setup_with_running(true).await
}

async fn setup_with_running(mark_running: bool) -> Harness {
    setup_with_replay(mark_running, TrustedReplayPolicy::NonIdempotent).await
}

async fn setup_with_replay(mark_running: bool, replay: TrustedReplayPolicy) -> Harness {
    let (state, service, authz, run, reservation) = super::fixture_with_running(mark_running).await;
    agenthub_db::mcp_operations::migrate_mcp_operations(&state.db)
        .await
        .unwrap();
    let daemon = agenthub_db::claim_daemon_generation(
        &state.db,
        "main",
        "mcp-daemon",
        1,
        chrono::Utc::now().timestamp(),
    )
    .await
    .unwrap();
    let journal = agenthub_db::mcp_operations::McpOperationStore::new(state.db.clone(), daemon);
    let upstream = Arc::new(Upstream {
        db: state.db.clone(),
        callback: Notify::new(),
        initialized: AtomicBool::new(false),
        calls: Mutex::new(Vec::new()),
        hold_writes: AtomicBool::new(false),
        write_received: Notify::new(),
        write_release: Notify::new(),
        listen_enabled: AtomicBool::new(false),
        resume_writes: AtomicBool::new(false),
        listened: Notify::new(),
        listener_replied: Notify::new(),
        gets: Mutex::new(Vec::new()),
        deleted: AtomicBool::new(false),
        expire_stream: AtomicBool::new(false),
        mrtr: AtomicBool::new(false),
        mrtr_drop_response: AtomicBool::new(false),
        tasks: AtomicBool::new(false),
        task_inputs_answered: AtomicBool::new(false),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!(
        "http://{}/mcp?private=endpoint-secret",
        listener.local_addr().unwrap()
    );
    let router = axum::Router::new()
        .route(
            "/mcp",
            post(handler).get(listener::get).delete(listener::delete),
        )
        .with_state(upstream.clone());
    let http = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let transport = McpHttpTransport::new(
        &endpoint,
        HeaderMap::from_iter([(
            "authorization".parse().unwrap(),
            "Bearer upstream-secret".parse().unwrap(),
        )]),
        Duration::from_secs(5),
    )
    .unwrap();
    let policy = McpBinding::new(
        "fixture".into(),
        &json!({"service":"fixture","space":"space-a"}),
        &json!({"revision":1}),
        transport,
        BTreeMap::from([
            ("write".into(), replay),
            ("read".into(), TrustedReplayPolicy::ReadOnly),
        ]),
    )
    .unwrap();
    let binding = Arc::new(McpProxyBinding::new(
        policy,
        Arc::new(|_, _, mut arguments| {
            if arguments
                .get("space_id")
                .is_some_and(|value| value != "space-a")
            {
                return Err(McpPolicyError::Scope);
            }
            arguments["space_id"] = "space-a".into();
            Ok(arguments)
        }),
    ));
    state
        .agents
        .initialize_mcp_proxy(
            journal.clone(),
            vec![(reservation.clone(), binding.clone())],
        )
        .unwrap();
    Harness {
        state,
        service,
        authz,
        run,
        reservation,
        journal,
        upstream,
        binding,
        http,
    }
}

#[tokio::test]
async fn real_mcp_shim_preserves_callbacks_pages_calls_and_reads_rotated_credentials() {
    let Harness {
        state,
        service,
        authz,
        run,
        reservation,
        journal,
        upstream,
        binding,
        http,
    } = setup().await;
    let valid = signed_token(
        &authz,
        &reservation,
        &run.id,
        vec![InternalAction::McpProxy.as_str().into()],
    );
    let legacy = issue_token(
        &authz,
        InternalRole::Worker,
        Some("reviewer"),
        Some(&run.id),
    );
    let denied = service
        .open_mcp_proxy(authenticated_request(
            OpenMcpProxyRequest {
                server_id: "fixture".into(),
            },
            &legacy,
        ))
        .await;
    assert_eq!(denied.unwrap_err().code(), Code::PermissionDenied);
    let wrong_server = service
        .open_mcp_proxy(authenticated_request(
            OpenMcpProxyRequest {
                server_id: "another-binding".into(),
            },
            &valid,
        ))
        .await;
    assert_eq!(wrong_server.unwrap_err().code(), Code::PermissionDenied);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = format!("http://{}", listener.local_addr().unwrap());
    let incoming = futures::stream::unfold(listener, |listener| async move {
        Some((listener.accept().await.map(|(stream, _)| stream), listener))
    });
    let grpc = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(
                TeamInternalControlServer::new(service)
                    .max_decoding_message_size(crate::mcp_proxy::MCP_RPC_MESSAGE_LIMIT)
                    .max_encoding_message_size(crate::mcp_proxy::MCP_RPC_MESSAGE_LIMIT),
            )
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });
    let file = LoopCredentialFile::create().unwrap();
    let mut envelope = LoopCredentialEnvelope {
        actor_id: reservation.actor_id.clone(),
        run_id: run.id.clone(),
        activation_id: reservation.activation_id.clone().unwrap(),
        generation: reservation.generation,
        target,
        access_token: valid,
        expires_at: chrono::Utc::now().timestamp() + 600,
        ca_cert_path: None,
    };
    file.replace(&envelope).unwrap();
    let binary = crate::agenthub_binary::resolve_agenthub_binary_path()
        .expect("build the real agenthub binary before this test");
    let mut child = tokio::process::Command::new(binary)
        .args(["mcp-proxy", "--server-id", "fixture"])
        .env(LOOP_CREDENTIAL_FILE_ENV, &file.path)
        .env_remove("AGENTHUB_INTERNAL_GRPC_TOKEN")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut errors = child.stderr.take().unwrap();
    let probe = discovery::request("probe", "legacy-probe");
    write_message(&mut input, &probe).await.unwrap();
    let rejected_probe = next(&mut output).await;
    assert_eq!(
        rejected_probe,
        json!({"jsonrpc":"2.0","id":"probe",
        "error":{"code":-32601,"message":"Method not found","data":{"fallback":"initialize"}}})
    );
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{"roots":{}},"clientInfo":{"name":"fake-provider","version":"1"}}})).await.unwrap();
    let callback = next(&mut output).await;
    assert_eq!(callback["method"], "roots/list");
    write_message(
        &mut input,
        &json!({"jsonrpc":"2.0","id":callback["id"],"result":{"roots":[]}}),
    )
    .await
    .unwrap();
    let initialized = next(&mut output).await;
    assert_eq!(initialized["id"], 1);
    assert_eq!(initialized["result"]["extension"]["preserved"], true);
    write_message(
        &mut input,
        &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    )
    .await
    .unwrap();
    write_message(
        &mut input,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    )
    .await
    .unwrap();
    let first_page = next(&mut output).await;
    assert_eq!(first_page["result"]["tools"][0]["extension"], "first-page");
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":3,"method":"tools/list","params":{"cursor":first_page["result"]["nextCursor"]}})).await.unwrap();
    let second_page = next(&mut output).await;
    assert_eq!(second_page["result"]["tools"][0]["name"], "read");
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"write","arguments":{"body":"private-argument"},"_meta":{"progressToken":"progress-1"}}})).await.unwrap();
    let progress = next(&mut output).await;
    assert_eq!(progress["method"], "notifications/progress");
    let result = next(&mut output).await;
    assert_eq!(result["id"], 4);
    assert_eq!(
        result["result"]["content"][0]["text"],
        "private-tool-result"
    );
    let events = journal
        .events(
            &run.team_id,
            "reviewer",
            reservation.activation_id.as_deref().unwrap(),
            0,
            100,
        )
        .await
        .unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(
        events[2].status,
        agenthub_agent_domain::mcp_operations::McpOperationStatus::Succeeded
    );
    let visible = json!([
        callback,
        initialized,
        first_page,
        second_page,
        progress,
        result
    ])
    .to_string();
    for secret in [
        "upstream-secret",
        "endpoint-secret",
        "private-upstream-session",
        &envelope.access_token,
    ] {
        assert!(!visible.contains(secret));
    }
    let calls_before = upstream.calls.lock().unwrap().len();

    // The original token remains valid. A cached-token shim would incorrectly send this call.
    envelope.access_token = signed_token(
        &authz,
        &reservation,
        &run.id,
        vec![InternalAction::TeamRead.as_str().into()],
    );
    file.replace(&envelope).unwrap();
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"read","arguments":{}}})).await.unwrap();
    let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("shim must exit even with stdin still open")
        .unwrap();
    assert!(!status.success());
    let mut stderr = String::new();
    errors.read_to_string(&mut stderr).await.unwrap();
    assert!(stderr.contains("PermissionDenied"), "{stderr}");
    assert!(!stderr.contains("upstream-secret"));
    assert_eq!(upstream.calls.lock().unwrap().len(), calls_before);
    drop(input);
    state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(5))
        .await
        .unwrap();
    binding.revoke();
    assert_eq!(
        state
            .agents
            .mcp_proxy()
            .unwrap()
            .open(&reservation, "fixture")
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    grpc.abort();
    http.abort();
}

async fn next(reader: &mut BufReader<tokio::process::ChildStdout>) -> Value {
    tokio::time::timeout(Duration::from_secs(8), read_message(reader))
        .await
        .expect("MCP output deadline")
        .unwrap()
        .expect("MCP output frame")
}

#[tokio::test]
async fn mcp_rpc_stream_loss_keeps_a_sent_write_owned_and_scope_isolation_intact() {
    use crate::internal::proto::agenthub::internal::v1::ExchangeMcpProxyRequest;

    let Harness {
        state,
        service,
        authz,
        run,
        reservation,
        journal,
        upstream,
        binding: _,
        http,
    } = setup().await;
    let token = signed_token(
        &authz,
        &reservation,
        &run.id,
        vec![InternalAction::McpProxy.as_str().into()],
    );
    let session_id = service
        .open_mcp_proxy(authenticated_request(
            OpenMcpProxyRequest {
                server_id: "fixture".into(),
            },
            &token,
        ))
        .await
        .unwrap()
        .into_inner()
        .session_id;
    let mut other = reservation.clone();
    other.actor_id = "another-actor".into();
    assert!(
        state
            .agents
            .mcp_proxy()
            .unwrap()
            .session(&other, &session_id)
            .await
            .is_err()
    );
    other = reservation.clone();
    other.activation_id = Some("another-activation".into());
    assert!(
        state
            .agents
            .mcp_proxy()
            .unwrap()
            .session(&other, &session_id)
            .await
            .is_err()
    );
    let metadata = json!({
        "io.modelcontextprotocol/protocolVersion":"2026-07-28",
        "io.modelcontextprotocol/clientInfo":{"name":"fixture","version":"1"},
        "io.modelcontextprotocol/clientCapabilities":{}
    });
    let request = json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":metadata}});
    let mut discovery = service
        .exchange_mcp_proxy(authenticated_request(
            ExchangeMcpProxyRequest {
                session_id: session_id.clone(),
                message_json: request.to_string(),
            },
            &token,
        ))
        .await
        .unwrap()
        .into_inner();
    let frame = discovery.next().await.unwrap().unwrap();
    assert!(frame.finished);
    assert_eq!(
        serde_json::from_str::<Value>(&frame.message_json).unwrap()["result"]["tools"][0]["name"],
        "write"
    );
    drop(discovery);
    upstream.hold_writes.store(true, Ordering::Release);
    let request = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"write","arguments":{"body":"private-write"},"_meta":metadata}});
    let stream = service
        .exchange_mcp_proxy(authenticated_request(
            ExchangeMcpProxyRequest {
                session_id: session_id.clone(),
                message_json: request.to_string(),
            },
            &token,
        ))
        .await
        .unwrap()
        .into_inner();
    tokio::time::timeout(Duration::from_secs(5), upstream.write_received.notified())
        .await
        .unwrap();
    drop(stream);
    let gate = state
        .agents
        .loop_operation_gate(&reservation.actor_id)
        .await;
    assert!(gate.try_write().is_err());
    upstream.write_release.notify_one();
    state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(5))
        .await
        .unwrap();
    assert!(gate.try_write().is_ok());
    let events = journal
        .events(
            &run.team_id,
            &reservation.actor_id,
            reservation.activation_id.as_deref().unwrap(),
            0,
            100,
        )
        .await
        .unwrap();
    assert_eq!(
        events.last().unwrap().status,
        agenthub_agent_domain::mcp_operations::McpOperationStatus::Succeeded
    );
    state
        .agents
        .mcp_proxy()
        .unwrap()
        .release_activation(&reservation)
        .await;
    assert!(
        state
            .agents
            .mcp_proxy()
            .unwrap()
            .session(&reservation, &session_id)
            .await
            .is_err()
    );
    assert!(
        state
            .agents
            .mcp_proxy()
            .unwrap()
            .open(&reservation, "fixture")
            .await
            .is_err()
    );
    http.abort();
}

#[tokio::test]
async fn invalid_notifications_and_callback_responses_do_not_fabricate_rpc_replies() {
    use crate::internal::proto::agenthub::internal::v1::ExchangeMcpProxyRequest;

    let Harness {
        state,
        service,
        authz,
        run,
        reservation,
        upstream,
        http,
        ..
    } = setup().await;
    let token = signed_token(
        &authz,
        &reservation,
        &run.id,
        vec![InternalAction::McpProxy.as_str().into()],
    );
    for message in [
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":"unsolicited","result":{}}),
    ] {
        let session_id = service
            .open_mcp_proxy(authenticated_request(
                OpenMcpProxyRequest {
                    server_id: "fixture".into(),
                },
                &token,
            ))
            .await
            .unwrap()
            .into_inner()
            .session_id;
        let session = state
            .agents
            .mcp_proxy()
            .unwrap()
            .session(&reservation, &session_id)
            .await
            .unwrap();
        let result = service
            .exchange_mcp_proxy(authenticated_request(
                ExchangeMcpProxyRequest {
                    session_id,
                    message_json: message.to_string(),
                },
                &token,
            ))
            .await;
        assert_eq!(
            result.err().unwrap().code(),
            tonic::Code::FailedPrecondition
        );
        assert!(!session.is_active());
    }
    assert!(upstream.calls.lock().unwrap().is_empty());
    http.abort();
}
