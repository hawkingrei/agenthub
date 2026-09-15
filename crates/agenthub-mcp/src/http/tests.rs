use std::sync::{Arc, Mutex};

use axum::{
    Router, body::Body, extract::Request as ServerRequest, response::Response as ServerResponse,
    routing::any,
};
use serde_json::json;

use super::*;

struct Captured {
    method: Method,
    headers: HeaderMap,
    body: Vec<u8>,
}

struct Fixture {
    endpoint: String,
    received: Arc<Mutex<Vec<Captured>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Fixture {
    async fn new(responses: Vec<ServerResponse>) -> Self {
        let received = Arc::new(Mutex::new(Vec::new()));
        let captured = received.clone();
        let responses = Arc::new(Mutex::new(VecDeque::from(responses)));
        let router = Router::new().fallback(any(move |request: ServerRequest| {
            let captured = captured.clone();
            let responses = responses.clone();
            async move {
                let (parts, body) = request.into_parts();
                let body = axum::body::to_bytes(body, MAX_MESSAGE_BYTES).await.unwrap();
                captured.lock().unwrap().push(Captured {
                    method: parts.method,
                    headers: parts.headers,
                    body: body.to_vec(),
                });
                responses.lock().unwrap().pop_front().unwrap_or_else(|| {
                    ServerResponse::builder()
                        .status(500)
                        .body(Body::empty())
                        .unwrap()
                })
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!(
            "http://{}/private-upstream-path",
            listener.local_addr().unwrap()
        );
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            endpoint,
            received,
            task,
        }
    }

    fn transport(&self) -> McpHttpTransport {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer upstream-private-token"),
        );
        McpHttpTransport::new(&self.endpoint, headers, Duration::from_secs(3)).unwrap()
    }
}

fn response(status: u16, content_type: &str, value: Value) -> ServerResponse {
    ServerResponse::builder()
        .status(status)
        .header("content-type", content_type)
        .body(Body::from(value.to_string()))
        .unwrap()
}

fn legacy() -> HttpContext {
    HttpContext {
        version: ProtocolVersion::November2025,
        session_id: None,
    }
}

fn request(id: i64, method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0", "id": id, "method": method, "params": params})
}

fn modern_call() -> Value {
    request(
        7,
        "tools/call",
        json!({"name":"namespace/tool:version", "arguments":{}, "_meta":{
            "io.modelcontextprotocol/protocolVersion":"2026-07-28",
            "io.modelcontextprotocol/clientInfo":{"name":"fixture","version":"1"},
            "io.modelcontextprotocol/clientCapabilities":{}
        }}),
    )
}

#[tokio::test]
async fn construction_does_no_io_and_json_results_and_credentials_are_preserved() {
    let result = json!({"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"memory_add","inputSchema":{"type":"object","additionalProperties":false},"vendorExtension":{"nested":[1,true,null]}}],"nextCursor":"page-2"}});
    let fixture = Fixture::new(vec![response(
        200,
        "application/json; charset=utf-8",
        result.clone(),
    )])
    .await;
    let transport = fixture.transport();
    let message = request(1, "tools/list", json!({"cursor":"opaque-cursor"}));
    let prepared = transport.prepare_post(&legacy(), &message, None).unwrap();
    assert!(fixture.received.lock().unwrap().is_empty());
    let mut exchange = transport.send(prepared).await.unwrap();
    assert_eq!(
        exchange.next_event().await.unwrap().unwrap().message,
        Some(result)
    );
    assert!(exchange.next_event().await.unwrap().is_none());
    let received = fixture.received.lock().unwrap();
    assert_eq!(received.len(), 1);
    assert_eq!(
        received[0].headers["authorization"],
        "Bearer upstream-private-token"
    );
    assert_eq!(
        received[0].headers["accept"],
        "application/json, text/event-stream"
    );
    assert_eq!(received[0].headers["mcp-protocol-version"], "2025-11-25");
    assert_eq!(
        serde_json::from_slice::<Value>(&received[0].body).unwrap(),
        message
    );
}

#[tokio::test]
async fn legacy_session_and_resumption_use_get_without_resending_a_tool_call() {
    let initialize = ServerResponse::builder().header("content-type", "application/json").header("mcp-session-id", "private-session")
        .body(Body::from(json!({"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}}}}).to_string())).unwrap();
    let fixture = Fixture::new(vec![
        initialize,
        ServerResponse::builder()
            .status(202)
            .body(Body::empty())
            .unwrap(),
        ServerResponse::builder()
            .header("content-type", "text/event-stream")
            .body(Body::from(
                "id: next\ndata: {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"content\":[]}}\n\n",
            ))
            .unwrap(),
        ServerResponse::builder()
            .status(204)
            .body(Body::empty())
            .unwrap(),
    ])
    .await;
    let transport = fixture.transport();
    let mut context = legacy();
    let mut exchange = transport
        .send(
            transport
                .prepare_post(
                    &context,
                    &request(1, "initialize", json!({"protocolVersion":"2025-11-25"})),
                    None,
                )
                .unwrap(),
        )
        .await
        .unwrap();
    context.session_id = exchange.session_id();
    assert!(context.session_id.is_some());
    exchange.next_event().await.unwrap().unwrap();
    let initialized = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
    let mut ack = transport
        .send(
            transport
                .prepare_post(&context, &initialized, None)
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(ack.next_event().await.unwrap().is_none());
    let mut resumed = transport
        .send(
            transport
                .prepare_listen(&context, Some("last-seen"))
                .unwrap(),
        )
        .await
        .unwrap();
    let event = resumed.next_event().await.unwrap().unwrap();
    assert_eq!(event.cursor.as_deref(), Some("next"));
    assert_eq!(event.message.unwrap()["id"], 2);
    let mut closed = transport
        .send(transport.prepare_close(&context).unwrap())
        .await
        .unwrap();
    assert!(closed.next_event().await.unwrap().is_none());
    let received = fixture.received.lock().unwrap();
    assert_eq!(
        received
            .iter()
            .map(|value| value.method.as_str())
            .collect::<Vec<_>>(),
        ["POST", "POST", "GET", "DELETE"]
    );
    assert_eq!(received[2].headers["last-event-id"], "last-seen");
    assert_eq!(received[2].headers["mcp-session-id"], "private-session");
}

#[tokio::test]
async fn sse_preserves_callbacks_notifications_control_fields_and_final_results() {
    let body = "\u{feff}: heartbeat\r\nid: resume-first\r\nretry: 25\r\ndata:\r\n\r\nevent: message\r\ndata: {\"jsonrpc\":\"2.0\",\"id\":\"callback\",\"method\":\"roots/list\"}\r\n\r\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\r\ndata: \"params\":{\"progress\":1,\"progressToken\":\"p\"}}\r\n\r\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"isError\":false,\"content\":[]}}\r\n\r\n";
    let chunks = body
        .as_bytes()
        .chunks(1)
        .map(|chunk| Ok::<_, std::io::Error>(axum::body::Bytes::copy_from_slice(chunk)))
        .collect::<Vec<_>>();
    let fixture = Fixture::new(vec![
        ServerResponse::builder()
            .header("content-type", "text/event-stream")
            .body(Body::from_stream(futures::stream::iter(chunks)))
            .unwrap(),
    ])
    .await;
    let transport = fixture.transport();
    let mut exchange = transport
        .send(
            transport
                .prepare_post(
                    &legacy(),
                    &request(1, "tools/call", json!({"name":"memory_add","arguments":{}})),
                    None,
                )
                .unwrap(),
        )
        .await
        .unwrap();
    let priming = exchange.next_event().await.unwrap().unwrap();
    assert!(priming.message.is_none());
    assert_eq!(priming.cursor.as_deref(), Some("resume-first"));
    assert_eq!(priming.retry, Some(Duration::from_millis(25)));
    assert_eq!(
        exchange
            .next_event()
            .await
            .unwrap()
            .unwrap()
            .message
            .unwrap()["method"],
        "roots/list"
    );
    assert_eq!(
        exchange
            .next_event()
            .await
            .unwrap()
            .unwrap()
            .message
            .unwrap()["params"]["progress"],
        1
    );
    assert_eq!(
        exchange
            .next_event()
            .await
            .unwrap()
            .unwrap()
            .message
            .unwrap()["result"]["isError"],
        false
    );
    assert!(exchange.next_event().await.unwrap().is_none());
}

#[tokio::test]
async fn all_upstream_error_envelopes_survive_without_transport_rewrites() {
    let cases = [
        (
            400,
            json!({"jsonrpc":"2.0","id":1,"error":{"code":-32022,"message":"Unsupported protocol version","data":{"supported":["2025-11-25"]}}}),
        ),
        (
            200,
            json!({"jsonrpc":"2.0","id":1,"result":{"isError":true,"content":[{"type":"text","text":"rejected"}]}}),
        ),
        (
            200,
            json!({"jsonrpc":"2.0","id":1,"result":{"error":{"kind":"upstream-denial"}}}),
        ),
    ];
    for (status, expected) in cases {
        let fixture =
            Fixture::new(vec![response(status, "application/json", expected.clone())]).await;
        let transport = fixture.transport();
        let mut exchange = transport
            .send(
                transport
                    .prepare_post(
                        &legacy(),
                        &request(1, "tools/call", json!({"name":"write"})),
                        None,
                    )
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(exchange.status_code(), status);
        assert_eq!(
            exchange.next_event().await.unwrap().unwrap().message,
            Some(expected)
        );
    }
}

#[tokio::test]
async fn modern_metadata_and_nested_parameter_headers_match_unchanged_arguments() {
    let fixture = Fixture::new(vec![response(
        200,
        "application/json",
        json!({"jsonrpc":"2.0","id":7,"result":{"resultType":"complete","content":[]}}),
    )])
    .await;
    let transport = fixture.transport();
    let context = HttpContext {
        version: ProtocolVersion::July2026,
        session_id: None,
    };
    let plan = ToolHeaderPlan::from_schema(&json!({"type":"object","properties":{
        "nested":{"type":"object","properties":{"region":{"type":"string","x-mcp-header":"Region"}}},
        "count":{"type":"integer","x-mcp-header":"Count"},
        "flag":{"type":"boolean","x-mcp-header":"Flag"},
        "absent":{"type":"string","x-mcp-header":"Absent"},
        "nullable":{"type":"string","x-mcp-header":"Nullable"}
    }})).unwrap();
    let mut message = modern_call();
    message["params"]["arguments"] =
        json!({"nested":{"region":" padded\nregion "},"count":42,"flag":false,"nullable":null});
    let mut exchange = transport
        .send(
            transport
                .prepare_post(&context, &message, Some(&plan))
                .unwrap(),
        )
        .await
        .unwrap();
    exchange.next_event().await.unwrap();
    let received = fixture.received.lock().unwrap();
    let headers = &received[0].headers;
    assert_eq!(headers["mcp-method"], "tools/call");
    assert_eq!(headers["mcp-name"], "namespace/tool:version");
    assert_eq!(
        headers["mcp-param-region"],
        "=?base64?IHBhZGRlZApyZWdpb24g?="
    );
    assert_eq!(headers["mcp-param-count"], "42");
    assert_eq!(headers["mcp-param-flag"], "false");
    assert!(!headers.contains_key("mcp-param-absent"));
    assert!(!headers.contains_key("mcp-param-nullable"));
    assert_eq!(
        serde_json::from_slice::<Value>(&received[0].body).unwrap(),
        message
    );
    assert!(transport.prepare_listen(&context, None).is_err());
}

#[tokio::test]
async fn redirects_are_never_followed_and_errors_do_not_expose_transport_secrets() {
    let target = Fixture::new(vec![]).await;
    let source = Fixture::new(vec![
        ServerResponse::builder()
            .status(307)
            .header("location", &target.endpoint)
            .body(Body::empty())
            .unwrap(),
    ])
    .await;
    let transport = source.transport();
    let error = transport
        .send(
            transport
                .prepare_post(
                    &legacy(),
                    &request(1, "tools/call", json!({"name":"write"})),
                    None,
                )
                .unwrap(),
        )
        .await
        .err()
        .unwrap();
    assert_eq!(error, McpTransportError::HttpStatus(307));
    assert_eq!(source.received.lock().unwrap().len(), 1);
    assert!(target.received.lock().unwrap().is_empty());
    let rendered = format!("{error:?}: {error}");
    for secret in [
        &source.endpoint,
        "private-upstream-path",
        "upstream-private-token",
    ] {
        assert!(!rendered.contains(secret));
    }
}

#[tokio::test]
async fn accepted_write_with_truncated_response_is_not_retried() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let transport = McpHttpTransport::new(
        &format!("http://{}/secret-endpoint", listener.local_addr().unwrap()),
        HeaderMap::new(),
        Duration::from_secs(2),
    )
    .unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        let mut chunk = [0; 1024];
        loop {
            let count = socket.read(&mut chunk).await.unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&chunk[..count]);
            if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]);
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|length| length.trim().parse().unwrap())
                    })
                    .unwrap();
                if bytes.len() >= end + 4 + length {
                    break;
                }
            }
        }
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 1000\r\n\r\n{\"jsonrpc\":\"2.0\"").await.unwrap();
        socket.shutdown().await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(150), listener.accept())
                .await
                .is_err()
        );
    });
    let prepared = transport
        .prepare_post(
            &legacy(),
            &request(
                1,
                "tools/call",
                json!({"name":"write","arguments":{"body":"private-memory"}}),
            ),
            None,
        )
        .unwrap();
    let mut exchange = transport.send(prepared).await.unwrap();
    let error = exchange.next_event().await.err().unwrap();
    assert_eq!(error, McpTransportError::Disconnected);
    assert!(!format!("{error:?}").contains("private-memory"));
    task.await.unwrap();
}

#[tokio::test]
async fn deadline_is_bounded_without_exposing_the_endpoint() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let transport = McpHttpTransport::new(
        &format!(
            "http://{}/credential-secret",
            listener.local_addr().unwrap()
        ),
        HeaderMap::new(),
        Duration::from_millis(30),
    )
    .unwrap();
    let task = tokio::spawn(async move {
        let _socket = listener.accept().await.unwrap();
        std::future::pending::<()>().await;
    });
    let error = transport
        .send(
            transport
                .prepare_post(&legacy(), &request(1, "tools/list", json!({})), None)
                .unwrap(),
        )
        .await
        .err()
        .unwrap();
    assert_eq!(error, McpTransportError::Deadline);
    assert!(!error.to_string().contains("credential-secret"));
    task.abort();
}

#[test]
fn header_annotations_reject_unreachable_conflicting_or_unsafe_schemas() {
    for schema in [
        json!(true),
        json!({"type":"string","x-mcp-header":"Root"}),
        json!({"properties":{"x":{"type":"number","x-mcp-header":"X"}}}),
        json!({"properties":{"x":{"type":"string","x-mcp-header":""}}}),
        json!({"properties":{"x":{"type":"string","x-mcp-header":"X\r\nAuthorization"}}}),
        json!({"properties":{"x":{"type":"string","x-mcp-header":"X"},"y":{"type":"string","x-mcp-header":"x"}}}),
        json!({"properties":{"x":{"type":"array","items":{"type":"string","x-mcp-header":"X"}}}}),
        json!({"allOf":[{"properties":{"x":{"type":"string","x-mcp-header":"X"}}}]}),
        json!({"$defs":{"data":{"properties":{"x":{"type":"string","x-mcp-header":"X"}}}}}),
        json!({"properties":{"x":{"items":[{"type":"string","x-mcp-header":"X"}]}}}),
    ] {
        assert!(ToolHeaderPlan::from_schema(&schema).is_err());
    }
    assert!(
        ToolHeaderPlan::from_schema(
            &json!({"type":"object","examples":[{"x-mcp-header":"instance-data"}]})
        )
        .is_ok()
    );
}

#[test]
fn metadata_rejects_version_mismatch_and_unsafe_integer_values_before_send() {
    let transport = McpHttpTransport::new(
        "https://example.invalid/mcp",
        HeaderMap::new(),
        Duration::from_secs(1),
    )
    .unwrap();
    let context = HttpContext {
        version: ProtocolVersion::July2026,
        session_id: None,
    };
    let plan = ToolHeaderPlan::from_schema(
        &json!({"properties":{"number":{"type":"integer","x-mcp-header":"Number"}}}),
    )
    .unwrap();
    let mut message = modern_call();
    for value in [
        json!(9_007_199_254_740_992u64),
        json!(-9_007_199_254_740_992i64),
        json!(1.5),
        json!("1"),
    ] {
        message["params"]["arguments"] = json!({"number":value});
        assert!(
            transport
                .prepare_post(&context, &message, Some(&plan))
                .is_err()
        );
    }
    message["params"]["arguments"] = json!({"number":1});
    message["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"] = "2025-11-25".into();
    assert!(
        transport
            .prepare_post(&context, &message, Some(&plan))
            .is_err()
    );
}

#[tokio::test]
async fn modern_mrtr_state_and_input_responses_are_forwarded_without_automatic_followup() {
    let pending = json!({"jsonrpc":"2.0","id":7,"result":{"resultType":"input_required",
        "requestState":"opaque-state-never-decoded",
        "inputRequests":{"confirm":{"method":"elicitation/create","params":{"mode":"form","message":"Confirm","requestedSchema":{"type":"object"}}}}
    }});
    let fixture = Fixture::new(vec![
        response(200, "application/json", pending.clone()),
        response(
            200,
            "application/json",
            json!({"jsonrpc":"2.0","id":8,"result":{"resultType":"complete","content":[]}}),
        ),
    ])
    .await;
    let transport = fixture.transport();
    let context = HttpContext {
        version: ProtocolVersion::July2026,
        session_id: None,
    };
    let plan = ToolHeaderPlan::from_schema(&json!({"type":"object"})).unwrap();
    let first = modern_call();
    let mut exchange = transport
        .send(
            transport
                .prepare_post(&context, &first, Some(&plan))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        exchange.next_event().await.unwrap().unwrap().message,
        Some(pending.clone())
    );
    assert!(exchange.next_event().await.unwrap().is_none());
    assert_eq!(fixture.received.lock().unwrap().len(), 1);
    let mut followup = first;
    followup["id"] = 8.into();
    followup["params"]["requestState"] = pending["result"]["requestState"].clone();
    followup["params"]["inputResponses"] = json!({"confirm":{"action":"accept","content":{}}});
    let mut exchange = transport
        .send(
            transport
                .prepare_post(&context, &followup, Some(&plan))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        exchange
            .next_event()
            .await
            .unwrap()
            .unwrap()
            .message
            .unwrap()["id"],
        8
    );
    let received = fixture.received.lock().unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&received[1].body).unwrap(),
        followup
    );
}

#[tokio::test]
async fn modern_http_rejects_legacy_server_requests_on_sse() {
    let fixture = Fixture::new(vec![
        ServerResponse::builder()
            .header("content-type", "text/event-stream")
            .body(Body::from(
                "data: {\"jsonrpc\":\"2.0\",\"id\":\"server\",\"method\":\"roots/list\"}\n\n",
            ))
            .unwrap(),
    ])
    .await;
    let transport = fixture.transport();
    let context = HttpContext {
        version: ProtocolVersion::July2026,
        session_id: None,
    };
    let plan = ToolHeaderPlan::from_schema(&json!({"type":"object"})).unwrap();
    let mut exchange = transport
        .send(
            transport
                .prepare_post(&context, &modern_call(), Some(&plan))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        exchange.next_event().await.err(),
        Some(McpTransportError::InvalidResponse)
    );
}

#[tokio::test]
async fn march_batches_are_preserved_and_rejected_by_later_protocol_versions() {
    let batch = json!([
        request(1, "ping", json!({})),
        request(2, "tools/list", json!({}))
    ]);
    let replies = json!([{"jsonrpc":"2.0","id":2,"result":{"tools":[]}},{"jsonrpc":"2.0","id":1,"result":{}}]);
    let fixture = Fixture::new(vec![response(200, "application/json", replies.clone())]).await;
    let transport = fixture.transport();
    let context = HttpContext {
        version: ProtocolVersion::March2025,
        session_id: None,
    };
    assert!(transport.prepare_post(&legacy(), &batch, None).is_err());
    let mut exchange = transport
        .send(transport.prepare_post(&context, &batch, None).unwrap())
        .await
        .unwrap();
    assert_eq!(
        exchange.next_event().await.unwrap().unwrap().message,
        Some(replies)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&fixture.received.lock().unwrap()[0].body).unwrap(),
        batch
    );
    let mixed = json!([request(1,"ping",json!({})),{"jsonrpc":"2.0","id":2,"result":{}}]);
    assert!(transport.prepare_post(&context, &mixed, None).is_err());
}

#[tokio::test]
async fn malformed_ack_oversized_body_and_non_rpc_errors_fail_without_echoing_bodies() {
    let fixture = Fixture::new(vec![
        ServerResponse::builder()
            .status(202)
            .body(Body::from("private-body"))
            .unwrap(),
        ServerResponse::builder()
            .header("content-type", "application/json")
            .body(Body::from(vec![b'x'; MAX_MESSAGE_BYTES + 1]))
            .unwrap(),
        ServerResponse::builder()
            .status(500)
            .header("content-type", "text/plain")
            .body(Body::from("private-body"))
            .unwrap(),
    ])
    .await;
    let transport = fixture.transport();
    let notify = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
    let mut ack = transport
        .send(transport.prepare_post(&legacy(), &notify, None).unwrap())
        .await
        .unwrap();
    assert_eq!(
        ack.next_event().await.err(),
        Some(McpTransportError::InvalidResponse)
    );
    let message = request(1, "tools/list", json!({}));
    assert_eq!(
        transport
            .send(transport.prepare_post(&legacy(), &message, None).unwrap())
            .await
            .err(),
        Some(McpTransportError::MessageTooLarge)
    );
    let error = transport
        .send(transport.prepare_post(&legacy(), &message, None).unwrap())
        .await
        .err()
        .unwrap();
    assert_eq!(error, McpTransportError::HttpStatus(500));
    assert!(!format!("{error:?}").contains("private-body"));
}
