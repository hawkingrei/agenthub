use super::*;
use axum::{Json, Router, extract::State, response::Response, routing::post};
use std::sync::atomic::AtomicUsize;

type Accepted = (Value, mpsc::UnboundedSender<String>);

struct Fixture {
    session: Arc<McpProxySession>,
    journal: JournaledMcpClient,
    accepted: mpsc::UnboundedReceiver<Accepted>,
    server: tokio::task::JoinHandle<()>,
}

async fn subscribe(
    State(accepted): State<mpsc::UnboundedSender<Accepted>>,
    headers: HeaderMap,
    Json(message): Json<Value>,
) -> Response {
    assert_eq!(headers["mcp-method"], "subscriptions/listen");
    assert_eq!(headers["mcp-protocol-version"], "2026-07-28");
    assert!(headers.get("mcp-session-id").is_none());
    let (feed, receiver) = mpsc::unbounded_channel();
    accepted.send((message, feed)).unwrap();
    let stream = futures::stream::unfold(receiver, |mut receiver| async {
        receiver
            .recv()
            .await
            .map(|data| (Ok::<_, std::io::Error>(data), receiver))
    });
    axum::http::Response::builder()
        .header("content-type", "text/event-stream")
        .body(axum::body::Body::from_stream(stream))
        .unwrap()
}

impl Fixture {
    async fn new() -> Self {
        let (accepted, receiver) = mpsc::unbounded_channel();
        let router = Router::new()
            .route("/mcp", post(subscribe))
            .with_state(accepted);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/mcp", listener.local_addr().unwrap());
        let server = tokio::spawn(async {
            axum::serve(listener, router).await.unwrap();
        });
        let transport =
            McpHttpTransport::new(&endpoint, HeaderMap::new(), Duration::from_millis(150)).unwrap();
        let policy = McpBinding::new(
            "fixture".into(),
            &json!({"scope":1}),
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
        Self {
            session,
            journal,
            accepted: receiver,
            server,
        }
    }

    async fn start(
        &mut self,
        request: Value,
    ) -> (
        mpsc::Receiver<McpProxyFrame>,
        mpsc::UnboundedSender<String>,
        tokio::task::JoinHandle<()>,
    ) {
        let prepared = self
            .session
            .prepare_subscription(&executor(), self.journal.clone(), request.clone())
            .await
            .unwrap();
        let (output, receiver) = mpsc::channel(8);
        let task = tokio::spawn(prepared.run(output, || async { Ok(()) }));
        let (actual, feed) = tokio::time::timeout(Duration::from_secs(2), self.accepted.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(actual, request);
        (receiver, feed, task)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

fn request(id: Value, filter: Value) -> Value {
    // ClientInfo is optional; capabilities remain mandatory on each request.
    json!({"jsonrpc":"2.0","id":id,"method":"subscriptions/listen","params":{"notifications":filter,
        "_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}})
}

fn notification(id: Value, method: &str, params: Value) -> Value {
    let mut params = params;
    params["_meta"] = json!({"io.modelcontextprotocol/subscriptionId":id});
    json!({"jsonrpc":"2.0","method":method,"params":params})
}

fn ack(id: Value, filter: Value) -> Value {
    notification(
        id,
        "notifications/subscriptions/acknowledged",
        json!({"notifications":filter}),
    )
}

fn send(feed: &mpsc::UnboundedSender<String>, message: &Value) {
    feed.send(format!("data: {message}\n\n")).unwrap();
}

async fn next(receiver: &mut mpsc::Receiver<McpProxyFrame>) -> Value {
    let frame = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(!frame.finished);
    serde_json::from_str(&frame.message_json).unwrap()
}

#[tokio::test]
async fn subscription_streams_preserve_filters_and_idle_until_explicit_cancellation() {
    let mut f = Fixture::new().await;
    let filter =
        json!({"toolsListChanged":true,"resourceSubscriptions":["file:///private-resource"]});
    let (mut first, feed, task) = f.start(request(json!(1), filter.clone())).await;
    let accepted = ack(json!(1), filter);
    send(&feed, &accepted);
    assert_eq!(next(&mut first).await, accepted);
    // Cross both the ordinary HTTP timeout and the live-authority tick with one pinned read.
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let changed = notification(
        json!(1),
        "notifications/resources/updated",
        json!({"uri":"file:///private-resource","extension":{"preserved":true}}),
    );
    send(&feed, &changed);
    assert_eq!(next(&mut first).await, changed);
    let (mut second, second_feed, second_task) = f
        .start(request(json!("1"), json!({"toolsListChanged":true})))
        .await;
    send(
        &second_feed,
        &ack(json!("1"), json!({"toolsListChanged":true})),
    );
    next(&mut second).await;
    let cancel = json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1,
        "_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}});
    let prepared = f.session.prepare(&executor(), cancel).await.unwrap();
    let (output, mut cancelled) = mpsc::channel(8);
    prepared.run(f.journal.clone(), output).await;
    assert!(cancelled.recv().await.unwrap().finished);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    assert!(first.recv().await.unwrap().finished);
    assert!(f.session.is_active());
    assert!(
        f.accepted.try_recv().is_err(),
        "stdio cancellation must close its HTTP stream without a POST"
    );
    assert_eq!(f.session.subscriptions.lock().unwrap().len(), 1);
    let close = json!({"jsonrpc":"2.0","id":"1","result":{"resultType":"complete","_meta":{"io.modelcontextprotocol/subscriptionId":"1"}}});
    send(&second_feed, &close);
    assert_eq!(next(&mut second).await, close);
    second_task.await.unwrap();
    assert!(second.recv().await.unwrap().finished);
    assert!(f.session.subscriptions.lock().unwrap().is_empty());
}

#[tokio::test]
async fn subscription_rejects_pre_ack_foreign_and_unacknowledged_notifications() {
    for scenario in [
        "before_ack",
        "foreign_id",
        "ack_expansion",
        "duplicate_ack",
        "foreign_resource",
        "foreign_task",
        "wrong_type",
        "wrong_close",
    ] {
        let mut f = Fixture::new().await;
        let filter = json!({"toolsListChanged":true,"resourceSubscriptions":["file:///allowed"]});
        let (mut output, feed, task) = f.start(request(json!(1), filter.clone())).await;
        if !matches!(scenario, "before_ack" | "ack_expansion" | "foreign_id") {
            send(&feed, &ack(json!(1), filter));
            next(&mut output).await;
        }
        let invalid = match scenario {
            "before_ack" => notification(json!(1), "notifications/tools/list_changed", json!({})),
            "foreign_id" => ack(json!("1"), json!({})),
            "ack_expansion" => ack(json!(1), json!({"taskIds":["foreign-task"]})),
            "duplicate_ack" => ack(json!(1), json!({})),
            "foreign_resource" => notification(
                json!(1),
                "notifications/resources/updated",
                json!({"uri":"file:///private"}),
            ),
            "foreign_task" => {
                notification(json!(1), "notifications/tasks", json!({"taskId":"private"}))
            }
            "wrong_type" => notification(json!(1), "notifications/prompts/list_changed", json!({})),
            "wrong_close" => json!({"jsonrpc":"2.0","id":1,"result":{"resultType":"complete"}}),
            _ => unreachable!(),
        };
        send(&feed, &invalid);
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
        assert!(
            output.recv().await.is_none(),
            "{scenario} cannot reach the provider"
        );
        assert!(!f.session.is_active());
    }
}

#[tokio::test]
async fn idle_subscription_releases_execution_guards_and_closes_on_revocation() {
    let mut f = Fixture::new().await;
    let prepared = f
        .session
        .prepare_subscription(&executor(), f.journal.clone(), request(json!(1), json!({})))
        .await
        .unwrap();
    let gate = Arc::new(RwLock::new(()));
    let live = Arc::new(AtomicBool::new(true));
    let checks = Arc::new(AtomicUsize::new(0));
    let (output, mut receiver) = mpsc::channel(8);
    let task = tokio::spawn({
        let gate = gate.clone();
        let live = live.clone();
        let checks = checks.clone();
        async move {
            prepared
                .run(output, || {
                    let gate = gate.clone();
                    let live = live.clone();
                    let checks = checks.clone();
                    async move {
                        let guard = gate.read_owned().await;
                        checks.fetch_add(1, Ordering::AcqRel);
                        if !live.load(Ordering::Acquire) {
                            return Err(McpTransportError::Disconnected);
                        }
                        Ok(guard)
                    }
                })
                .await;
        }
    });
    let (_, feed) = f.accepted.recv().await.unwrap();
    send(&feed, &ack(json!(1), json!({})));
    next(&mut receiver).await;
    let writer = tokio::time::timeout(Duration::from_millis(500), gate.write())
        .await
        .expect("idle stream retained execution guard");
    live.store(false, Ordering::Release);
    drop(writer);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    assert!(checks.load(Ordering::Acquire) >= 3);
    assert!(receiver.recv().await.is_none());
    assert!(!f.session.is_active());
}

#[tokio::test]
async fn subscription_admission_and_abandoned_preparations_release_bounded_capacity() {
    let f = Fixture::new().await;
    for filter in [
        json!({"taskIds":["a","a"]}),
        json!({"resourceSubscriptions":[1]}),
        json!({"toolsListChanged":"yes"}),
        json!({"unknown":true}),
    ] {
        assert!(
            f.session
                .prepare_subscription(&executor(), f.journal.clone(), request(json!(1), filter))
                .await
                .is_err()
        );
    }
    for index in 0..16 {
        let prepared = f
            .session
            .prepare_subscription(
                &executor(),
                f.journal.clone(),
                request(json!(index), json!({})),
            )
            .await
            .unwrap();
        assert_eq!(f.session.subscriptions.lock().unwrap().len(), 1);
        drop(prepared);
        assert!(f.session.subscriptions.lock().unwrap().is_empty());
    }
    let mut held = Vec::new();
    for id in 100..108 {
        held.push(
            f.session
                .prepare_subscription(
                    &executor(),
                    f.journal.clone(),
                    request(json!(id), json!({})),
                )
                .await
                .unwrap(),
        );
    }
    assert!(
        f.session
            .prepare_subscription(
                &executor(),
                f.journal.clone(),
                request(json!(108), json!({}))
            )
            .await
            .is_err()
    );
    drop(held);
    let mut missing = request(json!(108), json!({}));
    missing["params"]["_meta"]
        .as_object_mut()
        .unwrap()
        .remove("io.modelcontextprotocol/clientCapabilities");
    assert!(
        f.session
            .prepare_subscription(&executor(), f.journal.clone(), missing)
            .await
            .is_err()
    );
}
