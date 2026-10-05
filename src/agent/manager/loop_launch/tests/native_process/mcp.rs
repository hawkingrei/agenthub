use agenthub_agent_domain::app_tools::{AppConnection, AppManifest, AppReplayPolicy, AppTool};
use agenthub_db::app_registry::{AppBindingUpdate, AppGrantUpdate, AppRegistry, RegisterApp};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use std::sync::Mutex as StdMutex;

use super::*;

struct Upstream {
    uncertain: bool,
    revoked: std::sync::atomic::AtomicBool,
    calls: StdMutex<Vec<Value>>,
    requests: StdMutex<Vec<Value>>,
    owner: StdMutex<Option<(sqlx::SqlitePool, String, String)>>,
    finish_command: StdMutex<String>,
}

/// Exercise the real native process, two real proxy shims and the durable operation store.
#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY built from PINNED_UPSTREAM_REVISION"]
async fn controlled_proxy_sources_enforce_scope_revocation_and_uncertainty() {
    for uncertain in ["false", "true"] {
        super::super::mcp::run_configured_child_with_env(
            "agent::manager::loop_launch::tests::native_process::mcp::controlled_proxy_child",
            &[
                ("TEST_NATIVE_APP_TOKEN", "native-app-secret"),
                ("TEST_NATIVE_UNCERTAIN", uncertain),
            ],
        )
        .await;
    }
}

#[tokio::test]
#[ignore = "isolated environment child of the native controlled proxy test"]
async fn controlled_proxy_child() {
    let upstream = Arc::new(Upstream {
        uncertain: std::env::var("TEST_NATIVE_UNCERTAIN").unwrap() == "true",
        revoked: std::sync::atomic::AtomicBool::new(false),
        calls: StdMutex::new(Vec::new()),
        requests: StdMutex::new(Vec::new()),
        owner: StdMutex::new(None),
        finish_command: StdMutex::new(String::new()),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new()
        .route("/v1/chat/completions", post(model))
        .route("/mem/mcp", post(mem))
        .route("/mem/members/me", get(membership))
        .route("/app/mcp", post(app))
        .with_state(upstream.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let mut fixture =
        Fixture::new_with_mem("no-outcome", Some(&format!("http://{address}/mem/mcp"))).await;
    let registry = AppRegistry::new(fixture.state.db.clone());
    sqlx::query("INSERT INTO users(id, username, display_name, role, created_at) VALUES ('native-owner', 'native-owner', 'Native owner', 'admin', 1)")
        .execute(&fixture.state.db).await.unwrap();
    let manifest = AppManifest {
        schema_version: 1,
        events: vec![],
        scopes: ["write".into()].into(),
        tools: vec![AppTool {
            name: "write".into(),
            input_schema: json!({"type":"object", "properties":{"body":{"type":"string"}}, "required":["body"], "additionalProperties":false}),
            output_schema: None,
            required_scopes: ["write".into()].into(),
            replay: AppReplayPolicy::NonIdempotent,
        }],
    };
    let now = Utc::now().timestamp();
    let registered = registry
        .register(
            RegisterApp {
                owner_user_id: "native-owner",
                name: "Native fixture",
                connection: &AppConnection {
                    endpoint: format!("http://{address}/app/mcp"),
                    credential_env: Some("TEST_NATIVE_APP_TOKEN".into()),
                    authority: "native-fixture".into(),
                    namespace: "native-fixture".into(),
                },
                manifest: &manifest,
            },
            now,
        )
        .await
        .unwrap();
    registry
        .approve_team(
            "native-owner",
            AppGrantUpdate {
                app_id: &registered.id,
                team_id: &fixture.team_id,
                expected_revision: 0,
                scopes: &manifest.scopes,
            },
            now,
        )
        .await
        .unwrap();
    registry
        .bind_member(
            AppBindingUpdate {
                app_id: &registered.id,
                team_id: &fixture.team_id,
                actor_id: "worker",
                version: 1,
                expected_revision: 0,
                scopes: &manifest.scopes,
            },
            now,
        )
        .await
        .unwrap();
    *upstream.owner.lock().unwrap() = Some((
        fixture.state.db.clone(),
        registered.id,
        fixture.team_id.clone(),
    ));
    let control = crate::agenthub_binary::resolve_agenthub_binary_path().unwrap();
    let finish = fixture.directory.join("native-finish.json");
    std::fs::write(&finish, r#"{"kind":"no_actionable_work"}"#).unwrap();
    let quote =
        |path: &std::path::Path| format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"));
    *upstream.finish_command.lock().unwrap() = format!(
        "{} actor loop-finish --outcome-file {} --json",
        quote(&control),
        quote(&finish)
    );
    let native_state = fixture.directory.join("native-state");
    std::fs::create_dir(&native_state).unwrap();
    std::fs::write(native_state.join("config.json"), json!({"provider":"deepseek", "api_key":"fixture-key", "model":"fixture-model", "base_url":format!("http://{address}/v1")}).to_string()).unwrap();
    std::fs::write(
        fixture.directory.join("native-settings.json"),
        json!({"binary":std::env::var("AGENTHUB_RARA_TEST_BINARY").unwrap()}).to_string(),
    )
    .unwrap();
    let wrapper = fixture.directory.join("native-runtime");
    let script = WRAPPER.replace("os.execv(", "(root / 'native-environment.json').write_text(json.dumps([key for key in os.environ if key in ['TEST_NATIVE_APP_TOKEN', 'TEST_MEM_UPSTREAM_KEY', 'TEST_OTHER_MEM_KEY', 'NMEM_API_KEY', 'NMEM_API_URL', 'NOWLEDGE_MEM_HEADERS', 'MCP_HTTP_HEADERS']]))\nos.execv(");
    std::fs::write(&wrapper, script).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut config = (*fixture.state.agents.loop_app_config).clone();
    config.rara = Some(agenthub_config::RaraConfig {
        binary: Some(wrapper.to_string_lossy().into_owned()),
        ..Default::default()
    });
    fixture.state.agents = Arc::new((*fixture.state.agents).clone().with_loop_app_config(config));
    sqlx::query("UPDATE agents SET command = 'rara', args = '[]', runtime_model = 'fixture-model' WHERE id = 'worker'").execute(&fixture.state.db).await.unwrap();
    let reservation = fixture.admit("native-proxy").await;
    let activation_id = reservation.activation_id.clone().unwrap();
    // Finish the borrowed execution future before retiring the fixture.
    {
        let execution = fixture
            .state
            .agents
            .execute_loop_activation(fixture.state.teams.clone(), reservation.clone());
        tokio::pin!(execution);
        let approvals = async {
            loop {
                tokio::time::sleep(Duration::from_millis(20)).await;
                let id: Option<String> = sqlx::query_scalar("SELECT id FROM acp_permission_requests WHERE agent_id = 'worker' AND status = 'pending'").fetch_optional(&fixture.state.db).await.unwrap();
                if let Some(id) = id {
                    fixture
                        .state
                        .agents
                        .permissions
                        .respond(
                            &id,
                            RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                                "once",
                            )),
                            Some("once".into()),
                            Some("fixture".into()),
                        )
                        .await
                        .unwrap();
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(30), async {
        tokio::select! { result = &mut execution => result.unwrap(), _ = approvals => unreachable!() }
    }).await.unwrap();
    }
    let activation = LoopStore::new(fixture.state.db.clone())
        .activation(&fixture.team_id, &activation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        activation.state,
        LoopActivationState::Finished,
        "{activation:?}"
    );
    let calls = upstream.calls.lock().unwrap().clone();
    assert_eq!(
        calls.len(),
        2,
        "foreign scope and revoked App calls must not reach the upstream"
    );
    assert_eq!(
        calls[0]["params"]["arguments"],
        json!({"body":"native-memory", "space_id":"space-a"})
    );
    assert_eq!(
        calls[1]["params"]["arguments"],
        json!({"body":"native-app"})
    );
    let statuses: Vec<String> =
        sqlx::query_scalar("SELECT status FROM mcp_operation_attempts ORDER BY rowid")
            .fetch_all(&fixture.state.db)
            .await
            .unwrap();
    assert_eq!(
        statuses
            .iter()
            .filter(|status| *status == "succeeded")
            .count(),
        if upstream.uncertain { 1 } else { 2 }
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|status| *status == "outcome_unknown")
            .count(),
        usize::from(upstream.uncertain)
    );
    let history = fixture
        .state
        .agents
        .runtime_history(
            "worker",
            activation.session_id.as_deref().unwrap(),
            100,
            None,
        )
        .await
        .unwrap()
        .unwrap();
    assert!(history.closed);
    assert_eq!(
        history
            .receipts
            .iter()
            .filter(|receipt| receipt.kind == RuntimeRequestKind::McpSource
                && receipt.status == RuntimeRequestStatus::Accepted)
            .count(),
        2
    );
    let inherited: Vec<String> = serde_json::from_slice(
        &std::fs::read(fixture.directory.join("native-environment.json")).unwrap(),
    )
    .unwrap();
    assert!(inherited.is_empty(), "{inherited:?}");
    let requests = upstream.requests.lock().unwrap().clone();
    assert!(
        requests
            .iter()
            .any(|request| completed(request, "revoked-app"))
    );
    let encoded = serde_json::to_string(&requests).unwrap();
    for private in [
        "native-app-secret",
        "configured-secret",
        "other-profile-secret",
        "ambient-secret",
        "/mem/mcp",
        "/app/mcp",
        "AGENTHUB_LOOP_CREDENTIAL_FILE",
    ] {
        assert!(
            !encoded.contains(private),
            "provider request exposed {private}"
        );
    }
    fixture.close().await;
    server.abort();
}

async fn membership(headers: HeaderMap) -> Json<Value> {
    assert_eq!(headers["authorization"], "Bearer configured-secret");
    Json(
        json!({"workspace_id":"cd270331-80bc-4f90-8cc0-3fefbc7f74ab", "key_scope":{"scope_mode":"narrowed", "grants":["space-a"], "write_space":"space-a"}, "key_write_target":{"write_space":"space-a", "write_space_live":true}}),
    )
}

async fn mem(
    State(state): State<Arc<Upstream>>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Response {
    assert_eq!(headers["authorization"], "Bearer configured-secret");
    exchange(state, request, false).await
}

async fn app(
    State(state): State<Arc<Upstream>>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Response {
    assert_eq!(headers["authorization"], "Bearer native-app-secret");
    assert_eq!(headers["x-agenthub-actor-id"], "worker");
    exchange(state, request, true).await
}

async fn exchange(state: Arc<Upstream>, request: Value, app: bool) -> Response {
    let result = match request["method"].as_str().unwrap() {
        "initialize" => {
            json!({"protocolVersion":"2025-11-25", "capabilities":{"tools":{}}, "serverInfo":{"name":"native-fixture", "version":"1"}})
        }
        "notifications/initialized" => return StatusCode::ACCEPTED.into_response(),
        "tools/list" => {
            let schema = if app {
                json!({"type":"object", "properties":{"body":{"type":"string"}}, "required":["body"], "additionalProperties":false})
            } else {
                json!({"type":"object", "properties":{"body":{"type":"string"}, "space_id":{"type":"string"}}})
            };
            json!({"tools":[{"name":if app {"write"} else {"remember"}, "description":if app {"Native App write"} else {"Native Mem write"}, "inputSchema":schema}]})
        }
        "tools/call" => {
            state.calls.lock().unwrap().push(request.clone());
            if app {
                if state.uncertain {
                    // The upstream applied this write but did not supply an MCP result.
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
                json!({"content":[], "structuredContent":{"written":true}})
            } else {
                json!({"content":[], "structuredContent":{"remembered":true}})
            }
        }
        _ => panic!("unexpected MCP method"),
    };
    Json(json!({"jsonrpc":"2.0", "id":request["id"], "result":result})).into_response()
}

fn completed(request: &Value, id: &str) -> bool {
    request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|message| message["role"] == "tool" && message["tool_call_id"] == id)
}

async fn model(
    State(state): State<Arc<Upstream>>,
    Json(request): Json<Value>,
) -> ([(&'static str, &'static str); 1], String) {
    state.requests.lock().unwrap().push(request.clone());
    if let Some(response) = super::semantic_guard::compatible_response(&request) {
        return response;
    }
    if completed(&request, "app")
        && !state
            .revoked
            .swap(true, std::sync::atomic::Ordering::SeqCst)
    {
        let (db, app, team) = state.owner.lock().unwrap().clone().unwrap();
        AppRegistry::new(db)
            .revoke_team_grant(&app, &team, 1, Utc::now().timestamp())
            .await
            .unwrap();
    }
    let controlled = |description: &str| {
        request["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["function"]["description"] == description)
            .unwrap()["function"]["name"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let next = [
        (
            "foreign-memory",
            "Native Mem write",
            json!({"body":"foreign", "space_id":"foreign"}),
        ),
        (
            "memory",
            "Native Mem write",
            json!({"body":"native-memory"}),
        ),
        ("app", "Native App write", json!({"body":"native-app"})),
        (
            "revoked-app",
            "Native App write",
            json!({"body":"after-revocation"}),
        ),
    ]
    .into_iter()
    .find(|(id, _, _)| !completed(&request, id));
    let (message, finish) = if let Some((id, description, args)) = next {
        (
            json!({"role":"assistant", "content":null, "tool_calls":[{"index":0,"id":id,"type":"function","function":{"name":controlled(description),"arguments":args.to_string()}}]}),
            "tool_calls",
        )
    } else if !completed(&request, "finish") {
        let command = state.finish_command.lock().unwrap().clone();
        (
            json!({"role":"assistant", "content":null, "tool_calls":[{"index":0,"id":"finish","type":"function","function":{"name":"bash","arguments":json!({"command":command,"sandbox_permissions":"require_escalated","justification":"Complete the local fixture"}).to_string()}}]}),
            "tool_calls",
        )
    } else {
        (
            json!({"role":"assistant","content":"Proxy fixture complete"}),
            "stop",
        )
    };
    let usage = json!({"prompt_tokens":10,"completion_tokens":10,"total_tokens":20});
    if request["stream"] == true {
        let chunk = json!({"id":"fixture","object":"chat.completion.chunk","model":"fixture-model","choices":[{"index":0,"delta":message,"finish_reason":finish}],"usage":usage});
        (
            [("content-type", "text/event-stream")],
            format!("data: {chunk}\n\ndata: [DONE]\n\n"),
        )
    } else {
        ([("content-type", "application/json")], json!({"id":"fixture","object":"chat.completion","model":"fixture-model","choices":[{"index":0,"message":message,"finish_reason":finish}],"usage":usage}).to_string())
    }
}
