use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use agenthub_agent_domain::{
    loop_runtime::{
        LoopAdmission, LoopCleanupDisposition, LoopLimits, LoopPolicyState, LoopReservation,
        LoopSessionPolicy, LoopSourceReferences, LoopTriggerInput, LoopTriggerKind,
    },
    mcp_operations::{McpOperationRecord, McpOperationStatus},
};
use agenthub_db::{
    DaemonGeneration,
    loop_runtime::{LoopPolicyUpdate, LoopStore},
};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use serde_json::json;
use sqlx::SqlitePool;
use tokio::sync::Notify;
use uuid::Uuid;

use super::*;

mod batch;
mod continuation;
mod recovery;
mod task;
use crate::{
    http::{HttpContext, McpHttpTransport},
    policy::{McpBinding, McpCallContext, McpPolicyError, McpToolCatalog, TrustedReplayPolicy},
    protocol::ProtocolVersion,
};

struct Fixture {
    directory: PathBuf,
    pool: SqlitePool,
    daemon: DaemonGeneration,
    loops: LoopStore,
    journal: McpOperationStore,
}

impl Fixture {
    async fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("agenthub-mcp-call-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let pool = agenthub_db::init_db_at_path(&directory.join("control.sqlite"))
            .await
            .unwrap();
        sqlx::query("INSERT INTO agents(id, name, workdir, command, args, worktree_mode, status, created_at, updated_at) VALUES ('worker', 'Worker', '/tmp', 'fixture', '[]', 'use_existing', 'created', 1, 1)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO team_definitions(id, name, spec_json, created_at, updated_at) VALUES ('team', 'Team', ?, 1, 1)")
            .bind(json!({"members":[{"member_id":"worker"}]}).to_string()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO team_runs(id, team_id, context_id, status, input_json, created_at) VALUES ('mailbox', 'team', 'loop', 'submitted', '{}', 1)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO loop_mailbox_partitions(run_id, team_id, created_at) VALUES ('mailbox', 'team', 1)")
            .execute(&pool).await.unwrap();
        let daemon = agenthub_db::claim_daemon_generation(&pool, "main", "daemon", 1, now())
            .await
            .unwrap();
        let loops = LoopStore::new(pool.clone());
        loops
            .configure(
                LoopPolicyUpdate {
                    actor_id: "worker",
                    team_id: "team",
                    expected_revision: 0,
                    state: LoopPolicyState::Enabled,
                    session_policy: LoopSessionPolicy::Fresh,
                    limits: &LoopLimits::default(),
                },
                now(),
            )
            .await
            .unwrap();
        let journal = McpOperationStore::new(pool.clone(), daemon.clone());
        Self {
            directory,
            pool,
            daemon,
            loops,
            journal,
        }
    }

    async fn running(&self) -> LoopReservation {
        let receipt = self
            .loops
            .accept_trigger(
                &LoopTriggerInput {
                    actor_id: "worker".into(),
                    team_id: "team".into(),
                    kind: LoopTriggerKind::Operator,
                    source_key: Uuid::new_v4().to_string(),
                    due_at: None,
                    references: LoopSourceReferences::default(),
                },
                now(),
            )
            .await
            .unwrap();
        let LoopAdmission::Admitted(reservation) = self
            .loops
            .admit("team", &receipt.activation_id, "executor-owner", now())
            .await
            .unwrap()
        else {
            panic!("activation not admitted")
        };
        self.loops
            .bind_mailbox(&reservation, "mailbox", now())
            .await
            .unwrap();
        let session = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES (?, 'worker', 'running', ?)")
            .bind(&session).bind(now()).execute(&self.pool).await.unwrap();
        let reservation = self
            .loops
            .bind_session(&reservation, &session, now())
            .await
            .unwrap();
        self.loops.mark_running(&reservation, now()).await.unwrap();
        reservation
    }

    async fn stop(&self, executor: &LoopReservation) {
        self.loops
            .cancel("team", executor.activation_id.as_deref().unwrap(), now())
            .await
            .unwrap();
        self.loops
            .cleanup_verified(executor, LoopCleanupDisposition::Exited, now())
            .await
            .unwrap();
        sqlx::query("UPDATE agent_sessions SET status = 'completed' WHERE id = ?")
            .bind(&executor.session_id)
            .execute(&self.pool)
            .await
            .unwrap();
    }

    async fn operations(&self) -> Vec<McpOperationRecord> {
        let ids: Vec<String> =
            sqlx::query_scalar("SELECT id FROM mcp_operations ORDER BY created_at, id")
                .fetch_all(&self.pool)
                .await
                .unwrap();
        let mut records = Vec::new();
        for id in ids {
            records.push(
                self.journal
                    .operation("team", "worker", &id)
                    .await
                    .unwrap()
                    .unwrap(),
            );
        }
        records
    }

    async fn close(self) {
        self.pool.close().await;
        std::fs::remove_dir_all(self.directory).unwrap();
    }
}

struct Upstream {
    endpoint: String,
    state: Arc<UpstreamState>,
    task: tokio::task::JoinHandle<()>,
}

struct UpstreamState {
    pool: SqlitePool,
    requests: Mutex<Vec<Value>>,
    response: Mutex<Value>,
    mode: AtomicUsize,
    received: Notify,
    release: Notify,
    resumptions: Mutex<BTreeMap<String, Value>>,
    resumed: Mutex<Vec<String>>,
}

impl Upstream {
    async fn new(pool: SqlitePool) -> Self {
        let state = Arc::new(UpstreamState {
            pool,
            requests: Mutex::new(Vec::new()),
            response: Mutex::new(
                json!({"content":[{"type":"text","text":"private-result"}],"extension":{"kept":true}}),
            ),
            mode: AtomicUsize::new(0),
            received: Notify::new(),
            release: Notify::new(),
            resumptions: Mutex::new(BTreeMap::new()),
            resumed: Mutex::new(Vec::new()),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!(
            "http://{}/private-mcp?credential=endpoint-secret",
            listener.local_addr().unwrap()
        );
        let router = Router::new()
            .route("/private-mcp", post(upstream).get(recovery::resume))
            .with_state(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            endpoint,
            state,
            task,
        }
    }

    fn binding(&self, replay: TrustedReplayPolicy) -> McpBinding {
        let headers = HeaderMap::from_iter([(
            "authorization".parse().unwrap(),
            "Bearer upstream-secret".parse().unwrap(),
        )]);
        McpBinding::new(
            "fixture".into(),
            &json!({"service":"canonical-service","namespace":"space-a"}),
            &json!({"revision":1}),
            McpHttpTransport::new(&self.endpoint, headers, Duration::from_secs(2)).unwrap(),
            BTreeMap::from([("write".into(), replay)]),
        )
        .unwrap()
    }

    fn count(&self) -> usize {
        self.state.requests.lock().unwrap().len()
    }
}

impl Drop for Upstream {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn upstream(
    State(state): State<Arc<UpstreamState>>,
    headers: HeaderMap,
    Json(message): Json<Value>,
) -> Response {
    assert_eq!(headers["authorization"], "Bearer upstream-secret");
    let sent: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_attempts WHERE status = 'sent'")
            .fetch_one(&state.pool)
            .await
            .unwrap();
    if message["method"]
        .as_str()
        .is_some_and(|method| method.starts_with("tasks/"))
    {
        if message["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"] == "2026-07-28" {
            assert_eq!(
                headers
                    .get("mcp-name")
                    .and_then(|value| value.to_str().ok()),
                message["params"]["taskId"].as_str()
            );
        }
        let lookups: i64 = sqlx::query_scalar(if message["method"] == "tasks/cancel" {
            "SELECT COUNT(*) FROM mcp_operation_task_cancellations WHERE completed_at IS NULL"
        } else if message["method"] == "tasks/update" {
            "SELECT COUNT(*) FROM mcp_operation_task_updates WHERE completed_at IS NULL"
        } else {
            "SELECT COUNT(*) FROM mcp_operation_task_lookups WHERE completed_at IS NULL"
        })
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert!(lookups > 0, "task lookup must commit before HTTP");
        assert_eq!(sent, 0, "polling cannot start another tool send");
    } else {
        assert!(
            sent > 0,
            "upstream observed bytes before durable send permit"
        );
    }
    state.requests.lock().unwrap().push(message.clone());
    state.received.notify_one();
    let mode = state.mode.load(Ordering::SeqCst);
    if let Some(members) = message.as_array() {
        let tools = members
            .iter()
            .filter(|member| member["method"] == "tools/call")
            .count();
        assert_eq!(
            sent, tools as i64,
            "every batch write must be durably sent before HTTP"
        );
        for member in members
            .iter()
            .filter(|member| member["method"] == "tools/call")
        {
            assert_eq!(member["params"]["arguments"]["space_id"], "space-a");
        }
        let mut responses: Vec<_> = members.iter().filter(|member| member.get("id").is_some())
            .map(|member| json!({"jsonrpc":"2.0","id":member["id"],"result":{"content":[],"body":member.pointer("/params/arguments/body")}})).collect();
        if mode == 9 {
            let first = responses.remove(0);
            state
                .resumptions
                .lock()
                .unwrap()
                .insert("private-batch-cursor".into(), Value::Array(responses));
            return recovery::stream(format!(
                "id: private-batch-cursor\nretry: 10\ndata: {first}\n\n"
            ));
        }
        if mode == 8 {
            return (
                [("content-type", "text/event-stream")],
                format!("data: {}\n\n", responses[0]),
            )
                .into_response();
        }
        responses.reverse();
        if mode == 13 {
            responses.push(
                json!({"jsonrpc":"2.0","method":"notifications/tasks/status",
                "params":{"taskId":"unaccepted-task","status":"completed"}}),
            );
            return (
                [("content-type", "text/event-stream")],
                format!("data: {}\n\n", Value::Array(responses)),
            )
                .into_response();
        }
        return Json(Value::Array(responses)).into_response();
    }
    if mode == 1 {
        // The server consumed the entire request, then returned an incomplete body.
        return axum::http::Response::builder()
            .header("content-type", "application/json")
            .body(axum::body::Body::from("{\"jsonrpc\":\"2.0\",\"result\":"))
            .unwrap();
    }
    if mode == 2 {
        state.release.notified().await;
    }
    let result = state.response.lock().unwrap().clone();
    let response = if mode == 5 {
        json!({"jsonrpc":"2.0","error":result})
    } else if mode == 3 {
        json!({"jsonrpc":"2.0","id":message["id"],"error":result})
    } else {
        json!({"jsonrpc":"2.0","id":message["id"],"result":result})
    };
    if mode == 12 {
        use futures::StreamExt;
        let notice = json!({"jsonrpc":"2.0","method":"notifications/tasks/status","params":{
            "taskId":"private-task-id","status":"completed","ttl":null,
            "createdAt":"2026-09-16T00:00:00Z","lastUpdatedAt":"2026-09-16T00:00:00Z"}});
        let callback =
            json!({"jsonrpc":"2.0","id":"roots-before-task-receipt","method":"roots/list"});
        let initial = futures::stream::once(async move {
            Ok::<_, std::io::Error>(format!("data: {notice}\n\ndata: {callback}\n\n"))
        });
        let receipt = futures::stream::once(async move {
            state.release.notified().await;
            Ok::<_, std::io::Error>(format!("data: {response}\n\n"))
        });
        return axum::http::Response::builder()
            .header("content-type", "text/event-stream")
            .body(axum::body::Body::from_stream(initial.chain(receipt)))
            .unwrap();
    }
    if mode == 15 {
        let notice = json!({"jsonrpc":"2.0","method":"notifications/tasks/status","params":{
            "taskId":"foreign-task","status":"completed","ttl":null,
            "createdAt":"2026-09-16T00:00:00Z","lastUpdatedAt":"2026-09-16T00:00:00Z"}});
        return (
            [("content-type", "text/event-stream")],
            format!("data: {notice}\n\ndata: {response}\n\n"),
        )
            .into_response();
    }
    if matches!(mode, 9..=11) {
        let cursor = format!("private-stream-{}", message["id"]);
        state
            .resumptions
            .lock()
            .unwrap()
            .insert(cursor.clone(), response);
        let extra = if mode == 11 { "id:\ndata:\n\n" } else { "" };
        let retry = if mode == 10 { 10_000 } else { 10 };
        return recovery::stream(format!("id: {cursor}\nretry: {retry}\ndata:\n\n{extra}"));
    }
    if mode == 6 {
        // Valid short numeric spellings can exceed the output limit after JSON serialization.
        let values = "1e6,".repeat(crate::MAX_MESSAGE_BYTES / 10);
        let event = format!(
            r#"{{"jsonrpc":"2.0","method":"notifications/progress","params":{{"progressToken":"token","progress":1,"extra":[{values}1e6]}}}}"#
        );
        assert!(event.len() < crate::MAX_MESSAGE_BYTES);
        return (
            [("content-type", "text/event-stream")],
            format!("data: {event}\n\ndata: {response}\n\n"),
        )
            .into_response();
    }
    if mode == 2 || mode == 4 {
        let event = json!({"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":"token","progress":1}});
        return (
            [("content-type", "text/event-stream")],
            format!("data: {event}\n\ndata: {response}\n\n"),
        )
            .into_response();
    }
    (
        if mode == 3 || mode == 5 {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::OK
        },
        Json(response),
    )
        .into_response()
}

fn catalog() -> McpToolCatalog {
    McpToolCatalog::from_tools(&json!([{
        "name":"write", "description":"Discovered at runtime", "annotations":{"idempotentHint":true},
        "inputSchema":{"type":"object","properties":{
            "space_id":{"type":"string"}, "request_id":{"type":"string"}, "body":{"type":"string"}
        }}, "custom":{"unchanged":true}
    }]), ProtocolVersion::November2025).unwrap()
}

fn message(id: i64) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"write","arguments":{"body":"private-arguments","request_id":"caller-stable-id"}}})
}

fn prepare(binding: &McpBinding, executor: &LoopReservation, id: i64) -> PreparedToolCall {
    binding
        .prepare_call(
            &catalog(),
            &McpCallContext {
                executor,
                proxy_session_id: "proxy-session",
                http: &HttpContext {
                    version: ProtocolVersion::November2025,
                    session_id: None,
                },
            },
            message(id),
            |_, mut arguments| {
                arguments["space_id"] = "space-a".into();
                Ok(arguments)
            },
        )
        .unwrap()
}

async fn run(fixture: &Fixture, call: PreparedToolCall) -> Result<McpCallResult, McpCallError> {
    let (events, _receiver) = mpsc::channel(8);
    JournaledMcpClient::new(
        fixture.journal.clone(),
        crate::budget::ByteBudget::new(8 * crate::MAX_MESSAGE_BYTES),
    )
    .run(call, events)
    .await
}

#[tokio::test]
async fn wire_and_journal_share_bound_arguments_and_success_is_durable_before_return() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    let result = run(&fixture, prepare(&binding, &executor, 1))
        .await
        .unwrap();
    assert_eq!(
        result.response["result"],
        *upstream.state.response.lock().unwrap()
    );
    let record = fixture
        .journal
        .operation("team", "worker", &result.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, McpOperationStatus::Succeeded);
    let arguments = upstream.state.requests.lock().unwrap()[0]["params"]["arguments"].clone();
    assert_eq!(arguments["space_id"], "space-a");
    assert_eq!(
        record.intent.arguments_digest,
        digest("mcp-tool-arguments-v1", &arguments).unwrap()
    );
    let events = fixture
        .journal
        .events(
            "team",
            "worker",
            executor.activation_id.as_deref().unwrap(),
            0,
            100,
        )
        .await
        .unwrap();
    assert_eq!(
        events.iter().map(|event| event.status).collect::<Vec<_>>(),
        [
            McpOperationStatus::Prepared,
            McpOperationStatus::Sent,
            McpOperationStatus::Succeeded
        ]
    );
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT intent_json || COALESCE(completion_json, '') FROM mcp_operations",
    )
    .fetch_all(&fixture.pool)
    .await
    .unwrap();
    for secret in [
        "upstream-secret",
        "endpoint-secret",
        "private-arguments",
        "private-result",
        "caller-stable-id",
        "space-a",
    ] {
        assert!(!rows.join("").contains(secret));
    }
    assert_eq!(
        run(&fixture, prepare(&binding, &executor, 1)).await.err(),
        Some(McpCallError::AlreadyCompleted)
    );
    assert_eq!(upstream.count(), 1);
    drop(upstream);
    fixture.close().await;
}

#[tokio::test]
async fn lost_write_result_blocks_new_rpc_id_and_fresh_activation_after_reopen() {
    let mut fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    upstream.state.mode.store(1, Ordering::SeqCst);
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    assert!(matches!(
        run(&fixture, prepare(&binding, &executor, 1)).await,
        Err(McpCallError::Transport(_))
    ));
    assert_eq!(
        run(&fixture, prepare(&binding, &executor, 2)).await.err(),
        Some(McpCallError::UnsafeReplay)
    );
    assert_eq!(upstream.count(), 1);
    fixture.stop(&executor).await;
    drop(upstream);
    fixture.pool.close().await;
    fixture.pool = agenthub_db::init_db_at_path(&fixture.directory.join("control.sqlite"))
        .await
        .unwrap();
    fixture.journal = McpOperationStore::new(fixture.pool.clone(), fixture.daemon.clone());
    fixture.loops = LoopStore::new(fixture.pool.clone());
    let next = fixture.running().await;
    assert_eq!(
        run(&fixture, prepare(&binding, &next, 3)).await.err(),
        Some(McpCallError::UnsafeReplay)
    );
    let operations = fixture.operations().await;
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].status, McpOperationStatus::OutcomeUnknown);
    fixture.close().await;
}

#[tokio::test]
async fn declared_identity_retries_the_original_operation_without_changing_wire_identity() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    upstream.state.mode.store(1, Ordering::SeqCst);
    let binding = upstream.binding(TrustedReplayPolicy::StableIdentity {
        property_path: vec!["request_id".into()],
    });
    assert!(
        run(&fixture, prepare(&binding, &executor, 1))
            .await
            .is_err()
    );
    fixture.stop(&executor).await;
    let next = fixture.running().await;
    upstream.state.mode.store(0, Ordering::SeqCst);
    let result = run(&fixture, prepare(&binding, &next, 20)).await.unwrap();
    assert_eq!(result.attempt_number, 2);
    let requests = upstream.state.requests.lock().unwrap().clone();
    assert_eq!(requests[0]["params"], requests[1]["params"]);
    assert_ne!(requests[0]["id"], requests[1]["id"]);
    let operations = fixture.operations().await;
    assert_eq!(operations.len(), 1);
    let attempts = fixture
        .journal
        .attempts("team", "worker", &result.operation_id, 0, 100)
        .await
        .unwrap();
    assert_ne!(attempts[0].activation_id, attempts[1].activation_id);
    assert_eq!(attempts[0].status, McpOperationStatus::OutcomeUnknown);
    assert_eq!(attempts[1].status, McpOperationStatus::Succeeded);
    drop(upstream);
    fixture.close().await;
}

#[tokio::test]
async fn lost_event_receiver_and_cancelled_executor_do_not_discard_late_factual_result() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    upstream.state.mode.store(2, Ordering::SeqCst);
    let client = JournaledMcpClient::new(
        fixture.journal.clone(),
        crate::budget::ByteBudget::new(8 * crate::MAX_MESSAGE_BYTES),
    );
    let call = prepare(
        &upstream.binding(TrustedReplayPolicy::NonIdempotent),
        &executor,
        1,
    );
    let (events, receiver) = mpsc::channel(1);
    let operation = tokio::spawn(async move { client.run(call, events).await });
    upstream.state.received.notified().await;
    drop(receiver);
    fixture
        .loops
        .cancel("team", executor.activation_id.as_deref().unwrap(), now())
        .await
        .unwrap();
    upstream.state.release.notify_one();
    let result = operation.await.unwrap().unwrap();
    assert!(result.event_delivery_lost);
    assert_eq!(
        fixture.operations().await[0].status,
        McpOperationStatus::Succeeded
    );
    drop(upstream);
    fixture.close().await;
}

#[tokio::test]
async fn oversized_serialized_progress_does_not_discard_the_terminal_write_result() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    upstream.state.mode.store(6, Ordering::SeqCst);
    let result = run(
        &fixture,
        prepare(
            &upstream.binding(TrustedReplayPolicy::NonIdempotent),
            &executor,
            1,
        ),
    )
    .await
    .unwrap();
    assert!(result.event_delivery_lost);
    assert_eq!(
        result.response["result"]["content"][0]["text"],
        "private-result"
    );
    assert_eq!(
        fixture.operations().await[0].status,
        McpOperationStatus::Succeeded
    );
    assert_eq!(upstream.count(), 1);
    drop(upstream);
    fixture.close().await;
}

#[tokio::test]
async fn stale_executor_and_changed_retry_parameters_never_reach_upstream() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    let binding = upstream.binding(TrustedReplayPolicy::StableIdentity {
        property_path: vec!["request_id".into()],
    });
    upstream.state.mode.store(1, Ordering::SeqCst);
    assert!(
        run(&fixture, prepare(&binding, &executor, 1))
            .await
            .is_err()
    );
    let mut changed = message(2);
    changed["params"]["extension"] = json!({"affectsOperation":true});
    let call = binding
        .prepare_call(
            &catalog(),
            &McpCallContext {
                executor: &executor,
                proxy_session_id: "proxy-session",
                http: &HttpContext {
                    version: ProtocolVersion::November2025,
                    session_id: None,
                },
            },
            changed,
            |_, mut arguments| {
                arguments["space_id"] = "space-a".into();
                Ok(arguments)
            },
        )
        .unwrap();
    assert_eq!(
        run(&fixture, call).await.err(),
        Some(McpCallError::IdentityConflict)
    );
    let prepared_before_cancellation = prepare(&binding, &executor, 3);
    fixture.stop(&executor).await;
    assert_eq!(
        run(&fixture, prepared_before_cancellation).await.err(),
        Some(McpCallError::Authority)
    );
    assert_eq!(upstream.count(), 1);
    drop(upstream);
    fixture.close().await;
}

#[tokio::test]
async fn errors_remain_raw_and_input_required_or_task_receipts_never_claim_tool_success() {
    for (mode, response, completion) in [
        (
            5,
            json!({"code":-32700,"message":"private-unidentified-error"}),
            "json_rpc",
        ),
        (
            3,
            json!({"code":-32602,"message":"private-error","data":{"kept":1}}),
            "json_rpc",
        ),
        (
            0,
            json!({"isError":true,"content":[{"type":"text","text":"private-error"}]}),
            "mcp_result",
        ),
        (
            0,
            json!({"error":{"message":"private-error"},"content":[]}),
            "success_envelope",
        ),
        (
            0,
            json!({"resultType":"input_required","requestState":"opaque-private-state"}),
            "input_required",
        ),
        (
            0,
            json!({"task":{"taskId":"private-task","status":"working"}}),
            "task_accepted",
        ),
    ] {
        let fixture = Fixture::new().await;
        let executor = fixture.running().await;
        let upstream = Upstream::new(fixture.pool.clone()).await;
        upstream.state.mode.store(mode, Ordering::SeqCst);
        *upstream.state.response.lock().unwrap() = response.clone();
        let binding = upstream.binding(TrustedReplayPolicy::StableIdentity {
            property_path: vec!["request_id".into()],
        });
        let result = run(&fixture, prepare(&binding, &executor, 1))
            .await
            .unwrap();
        assert_eq!(
            result.response[if mode == 3 || mode == 5 {
                "error"
            } else {
                "result"
            }],
            response
        );
        let record = &fixture.operations().await[0];
        let serialized = serde_json::to_value(record.completion.as_ref().unwrap()).unwrap();
        assert_eq!(serialized["reason"], completion);
        if matches!(result.completion, McpCompletion::Deferred { .. }) {
            assert_eq!(record.status, McpOperationStatus::OutcomeUnknown);
            assert_eq!(
                run(&fixture, prepare(&binding, &executor, 2)).await.err(),
                Some(McpCallError::ContinuationRequired)
            );
        } else {
            assert_eq!(record.status, McpOperationStatus::Failed);
        }
        assert_eq!(upstream.count(), 1);
        assert!(!serde_json::to_string(record).unwrap().contains("private"));
        drop(upstream);
        fixture.close().await;
    }
}

#[test]
fn discovery_preserves_schemas_and_filters_only_invalid_http_header_tools_for_the_new_version() {
    let valid = json!({"name":"good","inputSchema":{"type":"object","properties":{"key":{"type":"string","x-mcp-header":"Key"}}},"extension":[1,2]});
    let invalid = json!({"name":"bad","inputSchema":{"type":"object","x-mcp-header":"Root"}});
    let tools = json!([valid, invalid]);
    assert_eq!(
        McpToolCatalog::from_tools(&tools, ProtocolVersion::November2025)
            .unwrap()
            .advertised_tools(),
        tools
    );
    assert_eq!(
        McpToolCatalog::from_tools(&tools, ProtocolVersion::July2026)
            .unwrap()
            .advertised_tools(),
        json!([valid])
    );
    assert!(McpToolCatalog::from_tools(&json!([valid, valid]), ProtocolVersion::July2026).is_err());
}

#[tokio::test]
async fn identity_and_scope_validation_happen_before_any_journal_entry_or_network_io() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    let binding = upstream.binding(TrustedReplayPolicy::StableIdentity {
        property_path: vec!["request_id".into()],
    });
    let http = HttpContext {
        version: ProtocolVersion::November2025,
        session_id: None,
    };
    let context = McpCallContext {
        executor: &executor,
        proxy_session_id: "session",
        http: &http,
    };
    for invalid in [Value::Null, json!(1), json!("")] {
        let mut request = message(1);
        request["params"]["arguments"]["request_id"] = invalid;
        assert!(matches!(
            binding.prepare_call(&catalog(), &context, request, |_, args| Ok(args)),
            Err(McpPolicyError::StableIdentity)
        ));
    }
    assert!(matches!(
        binding.prepare_call(&catalog(), &context, message(1), |_, _| Err(
            McpPolicyError::Scope
        )),
        Err(McpPolicyError::Scope)
    ));
    let mut continuation = message(1);
    continuation["params"]["requestState"] = "private-state".into();
    assert!(matches!(
        binding.prepare_call(&catalog(), &context, continuation, |_, args| Ok(args)),
        Err(McpPolicyError::Continuation)
    ));
    assert!(fixture.operations().await.is_empty());
    assert_eq!(upstream.count(), 0);
    drop(upstream);
    fixture.close().await;
}

#[tokio::test]
async fn concurrent_sends_cannot_duplicate_a_write_and_read_retry_gets_a_new_attempt() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    upstream.state.mode.store(2, Ordering::SeqCst);
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    let call = prepare(&binding, &executor, 1);
    let client = JournaledMcpClient::new(
        fixture.journal.clone(),
        crate::budget::ByteBudget::new(8 * crate::MAX_MESSAGE_BYTES),
    );
    let (events, _receiver) = mpsc::channel(8);
    let first = tokio::spawn(async move { client.run(call, events).await });
    upstream.state.received.notified().await;
    assert_eq!(
        run(&fixture, prepare(&binding, &executor, 1)).await.err(),
        Some(McpCallError::InFlight)
    );
    assert_eq!(
        run(&fixture, prepare(&binding, &executor, 2)).await.err(),
        Some(McpCallError::InFlight)
    );
    upstream.state.release.notify_one();
    first.await.unwrap().unwrap();
    assert_eq!(upstream.count(), 1);

    let reader = upstream.binding(TrustedReplayPolicy::ReadOnly);
    upstream.state.mode.store(1, Ordering::SeqCst);
    assert!(run(&fixture, prepare(&reader, &executor, 3)).await.is_err());
    upstream.state.mode.store(0, Ordering::SeqCst);
    let read = run(&fixture, prepare(&reader, &executor, 3)).await.unwrap();
    assert_eq!(read.attempt_number, 2);
    assert_eq!(upstream.count(), 3);
    drop(upstream);
    fixture.close().await;
}
