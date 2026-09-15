use std::{collections::VecDeque, sync::Mutex as StdMutex};

use axum::{Json, Router, extract::State, response::IntoResponse, routing::post};

use super::*;

#[derive(Default)]
struct Upstream {
    requests: StdMutex<Vec<Value>>,
    replies: StdMutex<VecDeque<Value>>,
}

struct Fixture {
    session: Arc<McpProxySession>,
    upstream: Arc<Upstream>,
    server: tokio::task::JoinHandle<()>,
}

impl Fixture {
    async fn new() -> Self {
        Self::with_budget(Arc::new(McpProxyBudget::default())).await
    }

    async fn with_budget(budget: Arc<McpProxyBudget>) -> Self {
        let upstream = Arc::new(Upstream::default());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/mcp", listener.local_addr().unwrap());
        let app = Router::new()
            .route(
                "/mcp",
                post(
                    |State(state): State<Arc<Upstream>>, Json(request): Json<Value>| async move {
                        let mut reply = state
                            .replies
                            .lock()
                            .unwrap()
                            .pop_front()
                            .expect("expected HTTP request");
                        reply["jsonrpc"] = json!("2.0");
                        reply["id"] = request["id"].clone();
                        state.requests.lock().unwrap().push(request);
                        if reply["disconnect"] == true {
                            let stream = futures::stream::once(async {
                                Err::<Vec<u8>, _>(std::io::Error::other("fixture lost response"))
                            });
                            return (
                                [("content-type", "application/json")],
                                axum::body::Body::from_stream(stream),
                            )
                                .into_response();
                        }
                        Json(reply).into_response()
                    },
                ),
            )
            .with_state(upstream.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let policy = McpBinding::new(
            "fixture".into(),
            &json!({"space":"read"}),
            &json!({"revision":1}),
            McpHttpTransport::new(&endpoint, HeaderMap::new(), Duration::from_secs(2)).unwrap(),
            BTreeMap::new(),
        )
        .unwrap();
        let session = McpProxySession::new(
            "read-session".into(),
            Arc::new(McpProxyBinding::new(
                policy,
                McpAccessPolicy::unrestricted(),
                Arc::new(|_, _, args| Ok(args)),
            )),
            budget,
        );
        Self {
            session,
            upstream,
            server,
        }
    }

    async fn run(&self, message: Value, reply: Value) -> Value {
        let prepared = self.session.prepare(&executor(), message).await.unwrap();
        self.upstream.replies.lock().unwrap().push_back(reply);
        let journal = JournaledMcpClient::new(
            agenthub_db::mcp_operations::McpOperationStore::new(
                // Read continuations cannot depend on or fabricate a tool operation row.
                sqlx::SqlitePool::connect_lazy("sqlite::memory:").unwrap(),
                agenthub_db::DaemonGeneration {
                    node_id: "main".into(),
                    generation: 1,
                    owner_id: "daemon".into(),
                    owner_pid: 1,
                    claimed_at: 1,
                },
            ),
            self.session.budget.delivery.clone(),
        );
        let (sender, mut receiver) = mpsc::channel(8);
        tokio::time::timeout(Duration::from_secs(3), prepared.run(journal, sender))
            .await
            .unwrap();
        let frame = receiver.recv().await.unwrap();
        assert!(frame.finished);
        serde_json::from_str(&frame.message_json).unwrap()
    }

    async fn rejects(&self, message: Value) {
        let sends = self.upstream.requests.lock().unwrap().len();
        assert!(matches!(
            self.session.prepare(&executor(), message).await,
            Err(McpPolicyError::Continuation)
        ));
        assert_eq!(self.upstream.requests.lock().unwrap().len(), sends);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

fn request(id: i64, method: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":{
        "name":"brief","uri":"mem://allowed","arguments":{"topic":"original"},"vendor":{"preserved":true},
        "_meta":{"progressToken":id,"io.modelcontextprotocol/protocolVersion":"2026-07-28",
            "io.modelcontextprotocol/clientInfo":{"name":"fixture","version":"1"},
            "io.modelcontextprotocol/clientCapabilities":{"roots":{}}}
    }})
}

fn required(state: Option<&str>, ids: &[&str]) -> Value {
    let inputs: serde_json::Map<_, _> = ids
        .iter()
        .map(|id| ((*id).into(), json!({"method":"roots/list"})))
        .collect();
    let mut result = json!({"resultType":"input_required","inputRequests":inputs,"vendor":{"opaque":"preserved"}});
    if let Some(state) = state {
        result["requestState"] = json!(state);
    }
    json!({"result":result})
}

fn retry(original: &Value, id: i64, state: Option<&str>, ids: &[&str]) -> Value {
    let mut request = original.clone();
    request["id"] = json!(id);
    request["params"]["_meta"]["progressToken"] = json!(id);
    let inputs: serde_json::Map<_, _> = ids
        .iter()
        .map(|id| ((*id).into(), json!({"roots":[],"vendor":7})))
        .collect();
    request["params"]["inputResponses"] = json!(inputs);
    if let Some(state) = state {
        request["params"]["requestState"] = json!(state);
    }
    request
}

fn complete() -> Value {
    json!({"result":{"resultType":"complete","contents":[{"uri":"mem://allowed","text":"native result","vendor":true}],"messages":[],"vendor":{"preserved":true}}})
}

#[tokio::test]
async fn read_mrtr_preserves_payload_and_binds_each_round_to_original_parameters() {
    for method in ["resources/read", "prompts/get"] {
        let fixture = Fixture::new().await;
        let original = request(1, method);
        let state = "not-json:\0/opaque-unicode-\u{03bb}";
        let first = required(Some(state), &["a", "b"]);
        assert_eq!(
            fixture.run(original.clone(), first.clone()).await["result"],
            first["result"]
        );
        for (id, (pointer, value)) in (2..).zip([
            (
                "/method",
                json!(if method == "resources/read" {
                    "prompts/get"
                } else {
                    "resources/read"
                }),
            ),
            ("/params/name", json!("other")),
            ("/params/uri", json!("mem://other")),
            ("/params/arguments/topic", json!("changed")),
            ("/params/vendor", json!({"preserved":false})),
            (
                "/params/_meta/io.modelcontextprotocol~1clientInfo/name",
                json!("another-client"),
            ),
            (
                "/params/_meta/io.modelcontextprotocol~1clientCapabilities",
                json!({"roots":{},"sampling":{}}),
            ),
            ("/params/requestState", json!("different-state")),
        ]) {
            let mut changed = retry(&original, id, Some(state), &["a"]);
            *changed.pointer_mut(pointer).unwrap() = value;
            fixture.rejects(changed).await;
        }
        let next = retry(&original, 20, Some(state), &["a", "extra"]);
        let second = required(Some("next-state"), &["b"]);
        assert_eq!(
            fixture.run(next.clone(), second.clone()).await["result"],
            second["result"]
        );
        fixture
            .rejects(retry(&original, 21, Some(state), &["a"]))
            .await;
        let last = retry(&original, 22, Some("next-state"), &["b"]);
        let response = fixture.run(last.clone(), complete()).await;
        assert_eq!(response["result"], complete()["result"]);
        fixture
            .rejects(retry(&original, 23, Some("next-state"), &["b"]))
            .await;
        assert_eq!(
            *fixture.upstream.requests.lock().unwrap(),
            [original, next, last]
        );
        assert_eq!(fixture.session.budget.retained.used(), 0);
    }
}

#[tokio::test]
async fn read_mrtr_without_state_requires_unambiguous_issued_inputs_and_survives_unsent_drop() {
    let fixture = Fixture::new().await;
    let original = request(1, "prompts/get");
    fixture
        .run(original.clone(), required(None, &["a", "b"]))
        .await;
    fixture
        .run(request(2, "prompts/get"), required(None, &["c"]))
        .await;
    fixture
        .rejects(retry(&original, 3, None, &["foreign"]))
        .await;
    fixture
        .rejects(retry(&original, 4, Some("invented"), &["a"]))
        .await;
    fixture
        .rejects(retry(&original, 5, None, &["a", "c"]))
        .await;
    let prepared = fixture
        .session
        .prepare(&executor(), retry(&original, 6, None, &["a"]))
        .await
        .unwrap();
    fixture.rejects(retry(&original, 7, None, &["a"])).await;
    // A reserved receipt still participates in ambiguity checks.
    fixture
        .rejects(retry(&original, 8, None, &["a", "c"]))
        .await;
    drop(prepared);
    fixture
        .run(retry(&original, 9, None, &["a", "ignored"]), complete())
        .await;
    fixture
        .run(retry(&original, 10, None, &["c"]), complete())
        .await;
    assert_eq!(fixture.session.budget.retained.used(), 0);
}

#[tokio::test]
async fn read_mrtr_invalid_responses_do_not_grant_continuation_or_create_tasks() {
    let fixture = Fixture::new().await;
    for (id, method, response) in [
        (1, "resources/list", required(Some("bad"), &["a"])),
        (
            2,
            "prompts/get",
            json!({"result":{"resultType":"input_required","requestState":"bad","inputRequests":{"a":{"method":"sampling/createMessage"}}}}),
        ),
        (
            3,
            "resources/read",
            json!({"result":{"resultType":"task","taskId":"unexpected"}}),
        ),
        (
            4,
            "prompts/get",
            json!({"result":{"resultType":"input_required","requestState":42}}),
        ),
        (
            5,
            "prompts/get",
            json!({"result":{"resultType":"input_required"}}),
        ),
    ] {
        let original = request(id, method);
        assert!(
            fixture
                .run(original.clone(), response)
                .await
                .get("error")
                .is_some()
        );
        fixture
            .rejects(retry(&original, id + 10, Some("bad"), &["a"]))
            .await;
        assert_eq!(fixture.session.budget.retained.used(), 0);
    }
    fixture.run(request(30, "prompts/get"), complete()).await;
}

#[tokio::test]
async fn read_mrtr_is_session_scoped_and_does_not_retry_failed_continuations() {
    let first = Fixture::new().await;
    let second = Fixture::new().await;
    let original = request(1, "resources/read");
    first
        .run(original.clone(), required(Some("state"), &["a"]))
        .await;
    second
        .rejects(retry(&original, 2, Some("state"), &["a"]))
        .await;
    let error =
        json!({"error":{"code":-32001,"message":"upstream rejected","data":{"preserved":true}}});
    assert_eq!(
        first
            .run(retry(&original, 3, Some("state"), &["a"]), error.clone())
            .await["error"],
        error["error"]
    );
    first
        .rejects(retry(&original, 4, Some("state"), &["a"]))
        .await;
    first.run(request(5, "resources/read"), complete()).await;
    assert_eq!(first.upstream.requests.lock().unwrap().len(), 3);
    assert_eq!(first.session.budget.retained.used(), 0);
}

#[tokio::test]
async fn read_mrtr_rounds_are_bounded_without_blocking_independent_reads() {
    let fixture = Fixture::new().await;
    let original = request(1, "resources/read");
    fixture
        .run(original.clone(), required(Some("state"), &[]))
        .await;
    for id in 2..=10 {
        fixture
            .run(
                retry(&original, id, Some("state"), &[]),
                required(Some("state"), &[]),
            )
            .await;
    }
    fixture
        .rejects(retry(&original, 11, Some("state"), &[]))
        .await;
    fixture.run(request(12, "resources/read"), complete()).await;
    assert_eq!(fixture.upstream.requests.lock().unwrap().len(), 11);
}

#[tokio::test]
async fn read_mrtr_pending_receipts_have_a_shared_capacity_and_release_with_session() {
    let fixture = Fixture::new().await;
    for id in 1..=64 {
        fixture
            .run(
                request(id, "prompts/get"),
                required(Some(&format!("state-{id}")), &[]),
            )
            .await;
    }
    assert!(matches!(
        fixture
            .session
            .prepare(&executor(), request(65, "prompts/get"))
            .await,
        Err(McpPolicyError::Transport(McpTransportError::Capacity))
    ));
    let budget = fixture.session.budget.retained.clone();
    assert_eq!(budget.used(), 64 * 8 * 1024);
    fixture
        .run(
            retry(&request(1, "prompts/get"), 66, Some("state-1"), &[]),
            complete(),
        )
        .await;
    fixture.run(request(67, "prompts/get"), complete()).await;
    assert_eq!(budget.used(), 63 * 8 * 1024);
    drop(fixture);
    assert_eq!(budget.used(), 0);
}

#[tokio::test]
async fn read_mrtr_lost_http_response_consumes_receipt_without_an_automatic_post() {
    let fixture = Fixture::new().await;
    let original = request(1, "resources/read");
    fixture
        .run(original.clone(), required(Some("state"), &["a"]))
        .await;
    let response = fixture
        .run(
            retry(&original, 2, Some("state"), &["a"]),
            json!({"disconnect":true}),
        )
        .await;
    assert!(response.get("error").is_some());
    fixture
        .rejects(retry(&original, 3, Some("state"), &["a"]))
        .await;
    assert_eq!(fixture.upstream.requests.lock().unwrap().len(), 2);
    fixture.run(request(4, "resources/read"), complete()).await;
    assert_eq!(fixture.session.budget.retained.used(), 0);
}

#[tokio::test]
async fn read_mrtr_receipts_share_the_retained_budget_across_sessions() {
    let budget = Arc::new(McpProxyBudget::new(8, 1, 64 * 1024 * 1024, 8192));
    let first = Fixture::with_budget(budget.clone()).await;
    let second = Fixture::with_budget(budget.clone()).await;
    let original = request(1, "prompts/get");
    first
        .run(original.clone(), required(Some("state"), &[]))
        .await;
    assert!(matches!(
        second
            .session
            .prepare(&executor(), request(2, "prompts/get"))
            .await,
        Err(McpPolicyError::Transport(McpTransportError::Capacity))
    ));
    first
        .run(retry(&original, 3, Some("state"), &[]), complete())
        .await;
    second.run(request(4, "prompts/get"), complete()).await;
    assert_eq!(budget.retained.used(), 0);
}

#[tokio::test]
async fn read_mrtr_cannot_enter_legacy_batches_or_unsupported_methods() {
    let session = session();
    batch::awaiting_initialized(&session).await;
    let initialized = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
    let bad = json!({"jsonrpc":"2.0","id":2,"method":"prompts/get","params":{"name":"brief","inputResponses":{}}});
    assert!(matches!(
        session
            .prepare(&executor(), json!([initialized, bad]))
            .await,
        Err(McpPolicyError::Continuation)
    ));
    assert!(session.protocol.lock().await.awaiting_initialized());
    assert!(session.request_ids.lock().await.is_empty());
    let fixture = Fixture::new().await;
    for (id, method) in [
        "server/discover",
        "resources/list",
        "prompts/list",
        "completion/complete",
        "ping",
    ]
    .iter()
    .enumerate()
    {
        let mut message = request(id as i64, method);
        message["params"]["ref"] = json!({"type":"ref/prompt","name":"brief"});
        fixture
            .rejects(retry(&message, id as i64, Some("state"), &[]))
            .await;
    }
}

#[tokio::test]
async fn read_mrtr_invalid_initialization_does_not_poison_a_later_handshake() {
    let session = session();
    let mut initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","clientInfo":{"name":"fixture","version":"1"},"capabilities":{},"requestState":"invented"}});
    assert!(matches!(
        session.prepare(&executor(), initialize.clone()).await,
        Err(McpPolicyError::Continuation)
    ));
    initialize["params"]
        .as_object_mut()
        .unwrap()
        .remove("requestState");
    initialize["id"] = json!(2);
    assert!(session.prepare(&executor(), initialize).await.is_ok());
}
