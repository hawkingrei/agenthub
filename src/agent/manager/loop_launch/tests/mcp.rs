use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use serde_json::{Value, json};
use std::sync::Mutex;

use super::*;

struct Upstream {
    db: Mutex<Option<sqlx::SqlitePool>>,
    messages: Mutex<Vec<Value>>,
}

async fn upstream(
    State(state): State<Arc<Upstream>>,
    headers: HeaderMap,
    Json(message): Json<Value>,
) -> Response {
    assert_eq!(headers["authorization"], "Bearer configured-secret");
    assert_eq!(headers["x-nmem-tool-set"], "external-agent");
    state.messages.lock().unwrap().push(message.clone());
    let result = match message["method"].as_str().unwrap() {
        "initialize" => {
            json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"fixture","version":"1"}})
        }
        "notifications/initialized" => return StatusCode::ACCEPTED.into_response(),
        "tools/list" => {
            json!({"tools":[{"name":"fixture_write","description":"Fixture write","inputSchema":{"type":"object","properties":{"body":{"type":"string"},"space_id":{"type":"string"}}},"extension":{"preserved":true}}]})
        }
        "tools/call" => {
            assert_eq!(
                message["params"]["arguments"],
                json!({"body":"private-business-body","space_id":"space-a"})
            );
            let db = state.db.lock().unwrap().clone().unwrap();
            let sent: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM mcp_operation_attempts WHERE status = 'sent'",
            )
            .fetch_one(&db)
            .await
            .unwrap();
            assert_eq!(sent, 1);
            json!({"content":[],"structuredContent":{"written":true}})
        }
        _ => panic!("unexpected upstream method"),
    };
    Json(json!({"jsonrpc":"2.0","id":message["id"],"result":result})).into_response()
}

#[tokio::test]
async fn configured_mcp_launch_isolates_inherited_secrets() {
    let mut child = tokio::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "agent::manager::loop_launch::tests::mcp::configured_mcp_child",
            "--ignored",
            "--nocapture",
        ])
        .env("TEST_MEM_UPSTREAM_KEY", "configured-secret")
        .env("TEST_OTHER_MEM_KEY", "other-profile-secret")
        .env("NMEM_API_KEY", "ambient-secret")
        .env("NMEM_API_URL", "https://private-upstream.example/mcp")
        .env("NOWLEDGE_MEM_HEADERS", "private-ambient-headers")
        .env("MCP_HTTP_HEADERS", "private-mcp-headers")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    use tokio::io::AsyncReadExt;
    let reader = tokio::spawn(async move {
        let mut out = Vec::new();
        let mut err = Vec::new();
        tokio::try_join!(stdout.read_to_end(&mut out), stderr.read_to_end(&mut err)).unwrap();
        (out, err)
    });
    let status = tokio::time::timeout(Duration::from_secs(45), child.wait())
        .await
        .unwrap()
        .unwrap();
    let (out, err) = reader.await.unwrap();
    assert!(
        status.success(),
        "child stdout: {}\nchild stderr: {}",
        String::from_utf8_lossy(&out),
        String::from_utf8_lossy(&err)
    );
}

#[tokio::test]
#[ignore = "Executed by the parent with an isolated inherited environment"]
async fn configured_mcp_child() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!(
        "http://{}/private-endpoint/mcp",
        listener.local_addr().unwrap()
    );
    let state = Arc::new(Upstream {
        db: Mutex::new(None),
        messages: Mutex::new(Vec::new()),
    });
    let router = axum::Router::new()
        .route("/private-endpoint/mcp", post(upstream))
        .with_state(state.clone());
    let http = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let fixture = Fixture::new_with_mem("mcp", Some(&endpoint)).await;
    *state.db.lock().unwrap() = Some(fixture.state.db.clone());
    let activation = fixture.execute("configured-mcp").await;
    assert_eq!(activation.state, LoopActivationState::Finished);
    let log = std::fs::read_to_string(fixture.directory.join("requests.jsonl")).unwrap();
    let events: Vec<Value> = log
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(events.iter().any(|event| event["mcp_bootstrap"] == true));
    assert!(events.iter().any(|event| event["mcp_write"] == true));
    for private in [
        &endpoint,
        "configured-secret",
        "other-profile-secret",
        "ambient-secret",
        "private-business-body",
        "private-mcp-headers",
        "private-ambient-headers",
    ] {
        assert!(!log.contains(private));
    }
    let succeeded: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mcp_operation_attempts WHERE status = 'succeeded'",
    )
    .fetch_one(&fixture.state.db)
    .await
    .unwrap();
    assert_eq!(succeeded, 1);
    assert_eq!(
        state
            .messages
            .lock()
            .unwrap()
            .iter()
            .filter(|message| message["method"] == "tools/call")
            .count(),
        1
    );
    fixture.close().await;
    http.abort();
}
