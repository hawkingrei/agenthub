use std::sync::{
    Mutex as StdMutex,
    atomic::{AtomicUsize, Ordering},
};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use futures::StreamExt;
use tokio::sync::Notify;

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Failure {
    Rejected,
    Anonymous,
    Malformed,
    Disconnected,
    Initialized,
    InitializedBatch,
}

struct Upstream {
    failure: Failure,
    initializes: AtomicUsize,
    finish: Notify,
    fresh_replied: Notify,
    callback_received: Notify,
    callback_release: Notify,
    hold_callback: AtomicBool,
    fail_delete: AtomicBool,
    reject_initialized: AtomicBool,
    deletes: StdMutex<Vec<String>>,
}

fn rejection(id: Option<Value>) -> Value {
    let mut error = json!({"jsonrpc":"2.0","error":{"code":-32001,"message":"Handshake rejected","data":{"retry":true}}});
    if let Some(id) = id {
        error["id"] = id;
    }
    error
}

async fn post_message(
    State(upstream): State<Arc<Upstream>>,
    headers: HeaderMap,
    Json(message): Json<Value>,
) -> Response {
    assert_eq!(headers["authorization"], "Bearer fixture-secret");
    let members = message
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(std::slice::from_ref(&message));
    let first = &members[0];
    if first.get("method").is_none() {
        assert_eq!(first["id"], "roots-stale");
        assert_eq!(first["result"]["roots"], json!([]));
        if headers["mcp-session-id"] == "old-session" {
            upstream.callback_received.notify_one();
            if upstream.hold_callback.load(Ordering::Acquire) {
                upstream.callback_release.notified().await;
            }
        } else {
            assert_eq!(headers["mcp-session-id"], "new-session");
            upstream.fresh_replied.notify_one();
        }
        return StatusCode::ACCEPTED.into_response();
    }
    if first["method"] == "notifications/initialized" {
        if headers["mcp-session-id"] == "old-session"
            && upstream.reject_initialized.load(Ordering::Acquire)
        {
            return (StatusCode::BAD_REQUEST, Json(rejection(None))).into_response();
        }
        return StatusCode::ACCEPTED.into_response();
    }
    assert_eq!(first["method"], "initialize");
    assert!(headers.get("mcp-session-id").is_none());
    let fresh = upstream.initializes.fetch_add(1, Ordering::AcqRel) != 0;
    if fresh {
        assert_eq!(*upstream.deletes.lock().unwrap(), ["old-session"]);
    }
    if !fresh && upstream.failure == Failure::Anonymous {
        return (
            StatusCode::BAD_REQUEST,
            [("mcp-session-id", "old-session")],
            Json(rejection(None)),
        )
            .into_response();
    }
    let callback = json!({"jsonrpc":"2.0","id":"roots-stale","method":"roots/list"});
    let first_frame =
        futures::stream::once(
            async move { Ok::<_, std::io::Error>(format!("data: {callback}\n\n")) },
        );
    let second_frame = futures::stream::once(async move {
        if fresh {
            upstream.fresh_replied.notified().await;
        } else {
            upstream.finish.notified().await;
        }
        if !fresh && upstream.failure == Failure::Disconnected {
            return Err(std::io::Error::other("fixture disconnect"));
        }
        let response = if !fresh && upstream.failure == Failure::Rejected {
            rejection(Some(message["id"].clone()))
        } else if !fresh && upstream.failure == Failure::Malformed {
            json!({"jsonrpc":"2.0","id":message["id"],"result":{"protocolVersion":"2025-03-26","capabilities":{}}})
        } else {
            json!({"jsonrpc":"2.0","id":message["id"],"result":{"protocolVersion":"2025-03-26","capabilities":{},"serverInfo":{"name":"fixture","version":"1"}}})
        };
        Ok(format!("data: {response}\n\n"))
    });
    axum::http::Response::builder()
        .header("content-type", "text/event-stream")
        .header(
            "mcp-session-id",
            if fresh { "new-session" } else { "old-session" },
        )
        .body(axum::body::Body::from_stream(
            first_frame.chain(second_frame),
        ))
        .unwrap()
}

async fn delete_session(State(upstream): State<Arc<Upstream>>, headers: HeaderMap) -> StatusCode {
    assert_eq!(headers["authorization"], "Bearer fixture-secret");
    upstream
        .deletes
        .lock()
        .unwrap()
        .push(headers["mcp-session-id"].to_str().unwrap().to_owned());
    if upstream.fail_delete.load(Ordering::Acquire) {
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::NO_CONTENT
    }
}

async fn fixture(
    failure: Failure,
) -> (
    Arc<McpProxySession>,
    Arc<Upstream>,
    tokio::task::JoinHandle<()>,
) {
    let upstream = Arc::new(Upstream {
        failure,
        initializes: AtomicUsize::new(0),
        finish: Notify::new(),
        fresh_replied: Notify::new(),
        callback_received: Notify::new(),
        callback_release: Notify::new(),
        hold_callback: AtomicBool::new(false),
        fail_delete: AtomicBool::new(false),
        reject_initialized: AtomicBool::new(true),
        deletes: StdMutex::new(Vec::new()),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let transport = McpHttpTransport::new(
        &format!("http://{}/mcp", listener.local_addr().unwrap()),
        HeaderMap::from_iter([(
            "authorization".parse().unwrap(),
            "Bearer fixture-secret".parse().unwrap(),
        )]),
        Duration::from_secs(2),
    )
    .unwrap();
    let router = Router::new()
        .route("/mcp", post(post_message).delete(delete_session))
        .with_state(upstream.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let policy = McpBinding::new(
        "fixture".into(),
        &json!({"space":"fixture"}),
        &json!({"revision":1}),
        transport,
        BTreeMap::new(),
    )
    .unwrap();
    let session = McpProxySession::new(
        "proxy".into(),
        Arc::new(McpProxyBinding::new(
            policy,
            Arc::new(|_, _, args| Ok(args)),
        )),
        Arc::new(McpProxyBudget::default()),
    );
    (session, upstream, server)
}

fn initialize(id: i64) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{"roots":{}},"clientInfo":{"name":"fixture","version":"1"}}})
}

fn callback() -> Value {
    json!({"jsonrpc":"2.0","id":"roots-stale","result":{"roots":[]}})
}

async fn run(session: &Arc<McpProxySession>, message: Value) -> mpsc::Receiver<McpProxyFrame> {
    let prepared = session.prepare(&executor(), message).await.unwrap();
    // Control-only exchanges must not touch the journal. This pool intentionally has no schema.
    let store = agenthub_db::mcp_operations::McpOperationStore::new(
        sqlx::SqlitePool::connect_lazy("sqlite::memory:").unwrap(),
        agenthub_db::DaemonGeneration {
            node_id: "main".into(),
            generation: 1,
            owner_id: "daemon".into(),
            owner_pid: 1,
            claimed_at: 1,
        },
    );
    let journal = JournaledMcpClient::new(store, session.budget.delivery.clone());
    let (output, receiver) = mpsc::channel(8);
    tokio::spawn(prepared.run(journal, output));
    receiver
}

async fn next(receiver: &mut mpsc::Receiver<McpProxyFrame>) -> (Option<Value>, bool) {
    let frame = tokio::time::timeout(Duration::from_secs(3), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    let (message, finished, _bytes) = frame.into_parts();
    (
        (!message.is_empty()).then(|| serde_json::from_str(&message).unwrap()),
        finished,
    )
}

async fn finish(receiver: &mut mpsc::Receiver<McpProxyFrame>) -> Vec<Value> {
    let mut messages = Vec::new();
    loop {
        let (message, finished) = next(receiver).await;
        messages.extend(message);
        if finished {
            return messages;
        }
    }
}

async fn retry(session: &Arc<McpProxySession>, upstream: &Upstream) {
    assert!(!session.can_listen().await);
    assert!(session.is_active());
    assert!(session.callbacks.lock().await.is_empty());
    assert!(session.upstream_context.lock().await.is_none());
    assert!(session.discovery.lock().await.catalog.is_none());
    assert_eq!(*upstream.deletes.lock().unwrap(), ["old-session"]);
    assert!(session.prepare(&executor(), callback()).await.is_err());
    let mut retried = run(session, initialize(2)).await;
    assert_eq!(next(&mut retried).await.0.unwrap()["id"], "roots-stale");
    assert!(finish(&mut run(session, callback()).await).await.is_empty());
    assert!(finish(&mut retried).await[0].get("result").is_some());
    finish(
        &mut run(
            session,
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        )
        .await,
    )
    .await;
    assert!(session.can_listen().await);
    session.shutdown().await.unwrap();
    assert_eq!(
        *upstream.deletes.lock().unwrap(),
        ["old-session", "new-session"]
    );
}

#[tokio::test]
async fn failed_handshakes_retire_provisional_sessions_and_allow_fresh_callbacks() {
    for failure in [
        Failure::Rejected,
        Failure::Anonymous,
        Failure::Malformed,
        Failure::Disconnected,
        Failure::Initialized,
        Failure::InitializedBatch,
    ] {
        let (session, upstream, server) = fixture(failure).await;
        let mut exchange = run(&session, initialize(1)).await;
        if failure != Failure::Anonymous {
            assert_eq!(next(&mut exchange).await.0.unwrap()["id"], "roots-stale");
            upstream.finish.notify_one();
        }
        let messages = finish(&mut exchange).await;
        match failure {
            Failure::Rejected => assert_eq!(messages, [rejection(Some(json!(1)))]),
            Failure::Anonymous => assert_eq!(messages, [rejection(None)]),
            Failure::Malformed | Failure::Disconnected => {
                assert_eq!(messages[0]["error"]["code"], -32000)
            }
            Failure::Initialized | Failure::InitializedBatch => {
                assert!(messages[0].get("result").is_some());
                let mut message = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
                if failure == Failure::InitializedBatch {
                    message = json!([message]);
                }
                assert_eq!(
                    finish(&mut run(&session, message).await).await,
                    [rejection(None)]
                );
            }
        }
        retry(&session, &upstream).await;
        server.abort();
    }
}

#[tokio::test]
async fn failed_handshake_waits_for_admitted_callback_before_deleting_session() {
    let (session, upstream, server) = fixture(Failure::Rejected).await;
    upstream.hold_callback.store(true, Ordering::Release);
    let mut exchange = run(&session, initialize(1)).await;
    next(&mut exchange).await;
    let mut reply = run(&session, callback()).await;
    upstream.callback_received.notified().await;
    upstream.finish.notify_one();
    assert!(
        tokio::time::timeout(Duration::from_millis(50), exchange.recv())
            .await
            .is_err()
    );
    assert!(upstream.deletes.lock().unwrap().is_empty());
    upstream.callback_release.notify_one();
    assert!(finish(&mut reply).await.is_empty());
    assert_eq!(finish(&mut exchange).await, [rejection(Some(json!(1)))]);
    retry(&session, &upstream).await;
    server.abort();
}

#[tokio::test]
async fn failed_retirement_preserves_error_and_context_for_final_shutdown() {
    let (session, upstream, server) = fixture(Failure::Rejected).await;
    upstream.fail_delete.store(true, Ordering::Release);
    let mut exchange = run(&session, initialize(1)).await;
    next(&mut exchange).await;
    upstream.finish.notify_one();
    assert_eq!(
        next(&mut exchange).await,
        (Some(rejection(Some(json!(1)))), false)
    );
    assert!(exchange.recv().await.is_none());
    assert!(!session.is_active());
    assert!(session.prepare(&executor(), initialize(2)).await.is_err());
    assert_eq!(upstream.initializes.load(Ordering::Acquire), 1);
    upstream.fail_delete.store(false, Ordering::Release);
    session.shutdown().await.unwrap();
    assert_eq!(
        *upstream.deletes.lock().unwrap(),
        ["old-session", "old-session"]
    );
    server.abort();
}

#[tokio::test]
async fn repeated_initialized_errors_do_not_retire_an_operating_session() {
    let (session, upstream, server) = fixture(Failure::Initialized).await;
    upstream.reject_initialized.store(false, Ordering::Release);
    let mut exchange = run(&session, initialize(1)).await;
    next(&mut exchange).await;
    upstream.finish.notify_one();
    finish(&mut exchange).await;
    let initialized = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
    finish(&mut run(&session, initialized.clone()).await).await;
    assert!(session.can_listen().await);
    upstream.reject_initialized.store(true, Ordering::Release);
    for message in [initialized.clone(), json!([initialized])] {
        assert_eq!(
            finish(&mut run(&session, message).await).await,
            [rejection(None)]
        );
        assert!(session.can_listen().await);
        assert!(upstream.deletes.lock().unwrap().is_empty());
    }
    session.shutdown().await.unwrap();
    assert_eq!(*upstream.deletes.lock().unwrap(), ["old-session"]);
    server.abort();
}
