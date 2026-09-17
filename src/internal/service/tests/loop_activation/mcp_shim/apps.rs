use crate::internal::proto::agenthub::internal::v1::{
    CloseMcpProxyRequest, ExchangeMcpProxyRequest,
};
use agenthub_agent_domain::app_tools::{AppConnection, AppManifest, AppReplayPolicy, AppTool};
use agenthub_db::app_registry::{
    AppBindingUpdate, AppGrantUpdate, AppRegistry, RegisterApp, RegisteredApp,
};

use super::*;

fn manifest() -> AppManifest {
    AppManifest {
        schema_version: 1, scopes: ["read".into(), "write".into()].into(),
        tools: ["write", "read"].into_iter().map(|name| AppTool {
            name: name.into(),
            input_schema: json!({"type":"object","properties":{"body":{"type":"string","x-mcp-header":"x-agenthub-actor-id"}},"required":["body"],"additionalProperties":false}),
            output_schema: Some(json!({"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"]})),
            required_scopes: [name.into()].into(), replay: AppReplayPolicy::NonIdempotent,
        }).collect(),
    }
}

struct AppUpstream {
    calls: Mutex<Vec<Value>>,
    headers: Mutex<Vec<HeaderMap>>,
    schema_drift: AtomicBool,
    invalid_result: AtomicBool,
    hold_write: AtomicBool,
    received: Notify,
    release: Notify,
}

async fn app_http(
    State(state): State<Arc<AppUpstream>>,
    headers: HeaderMap,
    Json(message): Json<Value>,
) -> Response {
    assert_eq!(headers["authorization"], "Bearer app-private-key");
    assert_eq!(headers["x-agenthub-actor-id"], "reviewer");
    assert_eq!(headers["x-agenthub-app-version"], "1");
    assert_eq!(headers["x-agenthub-workspace"].as_bytes().len(), 64);
    state.headers.lock().unwrap().push(headers);
    state.calls.lock().unwrap().push(message.clone());
    let result = match message["method"].as_str().unwrap() {
        "server/discover" => {
            let mut result = discovery::result();
            result["capabilities"]["tools"] = json!({"listChanged":true});
            result["capabilities"]["resources"] = json!({});
            result["capabilities"]["prompts"] = json!({});
            result["capabilities"]["logging"] = json!({});
            result
        }
        "tools/list" => {
            let mut tools: Vec<Value> = manifest().tools.into_iter().map(|tool| json!({
                "name":tool.name, "inputSchema":tool.input_schema, "outputSchema":tool.output_schema,
                "annotations":{"readOnlyHint":true,"idempotentHint":true},
            })).collect();
            tools.push(json!({"name":"undeclared","inputSchema":{"type":"object"}}));
            if state.schema_drift.load(Ordering::Acquire) {
                tools[0]["inputSchema"]["properties"]["body"]["type"] = json!("integer");
            }
            json!({"resultType":"complete","tools":tools})
        }
        "subscriptions/listen" => {
            let ack = json!({"jsonrpc":"2.0","method":"notifications/subscriptions/acknowledged", "params":{
                "_meta":{"io.modelcontextprotocol/subscriptionId":message["id"]}, "notifications":message["params"]["notifications"]}});
            let first = futures::stream::once(async move {
                Ok::<_, std::io::Error>(format!("data: {ack}\n\n"))
            });
            return axum::http::Response::builder()
                .header("content-type", "text/event-stream")
                .body(axum::body::Body::from_stream(
                    first.chain(futures::stream::pending()),
                ))
                .unwrap();
        }
        "tools/call" => {
            state.received.notify_one();
            if state.hold_write.load(Ordering::Acquire) {
                state.release.notified().await;
            }
            json!({"resultType":"complete", "content":[], "structuredContent":{"ok":if state.invalid_result.load(Ordering::Acquire) { json!("invalid") } else { json!(true) }},"native_extension":7})
        }
        _ => panic!("unexpected App method"),
    };
    Json(json!({"jsonrpc":"2.0","id":message["id"],"result":result})).into_response()
}

struct AppFixture {
    h: Harness,
    registry: AppRegistry,
    app: RegisteredApp,
    upstream: Arc<AppUpstream>,
    http: tokio::task::JoinHandle<()>,
    token: String,
    server_id: String,
}

impl AppFixture {
    async fn new() -> Self {
        let h = setup_with_running(false).await;
        sqlx::query("INSERT INTO users(id, username, display_name, role, created_at) VALUES ('app-owner', 'app-owner', 'App owner', 'admin', 1)")
            .execute(&h.state.db).await.unwrap();
        let upstream = Arc::new(AppUpstream {
            calls: Mutex::new(Vec::new()),
            headers: Mutex::new(Vec::new()),
            schema_drift: AtomicBool::new(false),
            invalid_result: AtomicBool::new(false),
            hold_write: AtomicBool::new(false),
            received: Notify::new(),
            release: Notify::new(),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/mcp", listener.local_addr().unwrap());
        let router = axum::Router::new()
            .route("/mcp", post(app_http))
            .with_state(upstream.clone());
        let http = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let registry = AppRegistry::new(h.state.db.clone());
        let now = chrono::Utc::now().timestamp();
        let app = registry
            .register(
                RegisterApp {
                    owner_user_id: "app-owner",
                    name: "App fixture",
                    connection: &AppConnection {
                        endpoint,
                        credential_env: Some("APP_FIXTURE_TOKEN".into()),
                        authority: "fixture-service".into(),
                        namespace: "fixture-workspace".into(),
                    },
                    manifest: &manifest(),
                },
                now,
            )
            .await
            .unwrap();
        let scopes = ["write".into()].into();
        registry
            .approve_team(
                "app-owner",
                AppGrantUpdate {
                    app_id: &app.id,
                    team_id: &h.run.team_id,
                    expected_revision: 0,
                    scopes: &scopes,
                },
                now,
            )
            .await
            .unwrap();
        registry
            .bind_member(
                AppBindingUpdate {
                    app_id: &app.id,
                    team_id: &h.run.team_id,
                    actor_id: &h.reservation.actor_id,
                    version: 1,
                    expected_revision: 0,
                    scopes: &scopes,
                },
                now,
            )
            .await
            .unwrap();
        let pins = registry.pin_activation(&h.reservation, now).await.unwrap();
        let pinned = crate::mcp_proxy::apps::resolve_pinned(
            &registry,
            pins[0].clone(),
            &std::env::temp_dir(),
            |_| Some("app-private-key".into()),
        )
        .await
        .unwrap();
        // Credential rotation and fresh activation metadata do not change the configuration revision.
        let mut next_pin = pins[0].clone();
        next_pin.activation_id = "next-activation".into();
        next_pin.pinned_generation += 1;
        next_pin.binding_revision += 1;
        let rotated = crate::mcp_proxy::apps::resolve_pinned(
            &registry,
            next_pin,
            &std::env::temp_dir(),
            |_| Some("rotated-key".into()),
        )
        .await
        .unwrap();
        assert_eq!(pinned.fingerprint, rotated.fingerprint);
        let server_id = pinned.binding.server_id().to_owned();
        h.state
            .agents
            .mcp_proxy()
            .unwrap()
            .mount_app(&h.reservation, pinned)
            .await
            .unwrap();
        LoopStore::new(h.state.db.clone())
            .mark_running(&h.reservation, now)
            .await
            .unwrap();
        let token = signed_token(
            &h.authz,
            &h.reservation,
            &h.run.id,
            vec![InternalAction::McpProxy.as_str().into()],
        );
        Self {
            h,
            registry,
            app,
            upstream,
            http,
            token,
            server_id,
        }
    }

    async fn open(&self) -> String {
        self.h
            .service
            .open_mcp_proxy(authenticated_request(
                OpenMcpProxyRequest {
                    server_id: self.server_id.clone(),
                },
                &self.token,
            ))
            .await
            .unwrap()
            .into_inner()
            .session_id
    }

    async fn call(&self, session: &str, id: &str, method: &str, params: Value) -> Value {
        access::call(&self.h, &self.token, session, request(id, method, params)).await
    }

    async fn close(self) {
        self.h
            .state
            .agents
            .mcp_proxy()
            .unwrap()
            .release_activation(&self.h.reservation)
            .await;
        self.h
            .state
            .agents
            .daemon_tasks()
            .shutdown_runtime(Duration::from_secs(5))
            .await
            .unwrap();
        self.http.abort();
        self.h.http.abort();
    }
}

fn request(id: &str, method: &str, params: Value) -> Value {
    let mut message = discovery::request(id, "app-fixture");
    message["method"] = json!(method);
    message["params"]
        .as_object_mut()
        .unwrap()
        .extend(params.as_object().unwrap().clone());
    message
}

mod calls;
mod revocation;
