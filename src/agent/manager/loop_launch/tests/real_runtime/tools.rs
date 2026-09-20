use super::*;
use agenthub_agent_domain::app_tools::{AppConnection, AppManifest};
use agenthub_db::app_registry::{AppBindingUpdate, AppGrantUpdate, AppRegistry, RegisterApp};
use axum::{
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::get,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Default)]
pub(super) struct Upstream {
    pub outage: AtomicBool,
    pub contexts: AtomicUsize,
    pub searches: AtomicUsize,
    pub writes: AtomicUsize,
}

async fn mem(
    State(state): State<Arc<Upstream>>,
    headers: HeaderMap,
    Json(message): Json<Value>,
) -> Response {
    assert_eq!(headers["authorization"], "Bearer acceptance-mem-key");
    let result = match message["method"].as_str().unwrap() {
        "initialize" => {
            json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"Acceptance Mem","version":"1"}})
        }
        "notifications/initialized" => return StatusCode::ACCEPTED.into_response(),
        "tools/list" => json!({"tools":[
            {"name":"read_context_bundle","inputSchema":{"type":"object","properties":{"space_id":{"type":"string"}},"additionalProperties":false}},
            {"name":"memory_search","inputSchema":{"type":"object","properties":{"space_id":{"type":"string"},"query":{"type":"string"}},"required":["query"],"additionalProperties":false}}
        ]}),
        "tools/call" => {
            assert_eq!(message["params"]["arguments"]["space_id"], "space-a");
            if state.outage.load(Ordering::SeqCst) {
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
            if message["params"]["name"] == "read_context_bundle" {
                let epoch = state.contexts.fetch_add(1, Ordering::SeqCst) + 1;
                let bundle = json!({"format":"markdown","space_id":"space-a","content":format!("Attributed DATA: acceptance scope space-a, epoch {epoch}." )});
                json!({"content":[{"type":"text","text":bundle.to_string()}]})
            } else {
                assert_eq!(message["params"]["name"], "memory_search");
                state.searches.fetch_add(1, Ordering::SeqCst);
                json!({"content":[{"type":"text","text":"Scoped acceptance knowledge"}]})
            }
        }
        _ => panic!("unexpected Mem method"),
    };
    Json(json!({"jsonrpc":"2.0","id":message["id"],"result":result})).into_response()
}

fn manifest() -> AppManifest {
    serde_json::from_value(json!({"schema_version":1,"scopes":["write"],
        "tools":[{"name":"write","input_schema":{"type":"object","properties":{"body":{"type":"string"}},"required":["body"],"additionalProperties":false},"output_schema":{"type":"object","properties":{"written":{"type":"boolean"}},"required":["written"]},"required_scopes":["write"],"replay":{"kind":"non_idempotent"}}],
        "events":[{"name":"changed","required_scopes":["write"]}]})).unwrap()
}

async fn app(
    State(state): State<Arc<Upstream>>,
    headers: HeaderMap,
    Json(message): Json<Value>,
) -> Response {
    assert_eq!(headers["authorization"], "Bearer acceptance-app-key");
    assert_eq!(headers["x-agenthub-app-version"], "1");
    let result = match message["method"].as_str().unwrap() {
        "initialize" => {
            json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"Acceptance App","version":"1"}})
        }
        "notifications/initialized" => return StatusCode::ACCEPTED.into_response(),
        "tools/list" => {
            let tool = manifest().tools.remove(0);
            json!({"tools":[{"name":tool.name,"inputSchema":tool.input_schema,"outputSchema":tool.output_schema}]})
        }
        "tools/call" => {
            assert_eq!(headers["x-agenthub-actor-id"], "worker");
            assert_eq!(
                message["params"]["arguments"],
                json!({"body":"accepted write"})
            );
            state.writes.fetch_add(1, Ordering::SeqCst);
            json!({"content":[],"structuredContent":{"written":true}})
        }
        _ => panic!("unexpected App method"),
    };
    Json(json!({"jsonrpc":"2.0","id":message["id"],"result":result})).into_response()
}

pub(super) async fn serve() -> (String, Arc<Upstream>, tokio::task::JoinHandle<()>) {
    assert_eq!(
        std::env::var("TEST_MEM_UPSTREAM_KEY").unwrap(),
        "acceptance-mem-key"
    );
    assert_eq!(
        std::env::var("TEST_APP_TOKEN").unwrap(),
        "acceptance-app-key"
    );
    let state = Arc::new(Upstream::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let router = axum::Router::new()
        .route("/mem/mcp", post(mem))
        .route("/app/mcp", post(app))
        .route("/mem/members/me", get(|| async {
            Json(json!({"workspace_id":"cd270331-80bc-4f90-8cc0-3fefbc7f74ab","key_scope":{"scope_mode":"narrowed","grants":["space-a"],"write_space":"space-a"},"key_write_target":{"write_space":"space-a","write_space_live":true}}))
        }))
        .with_state(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (endpoint, state, task)
}

pub(super) async fn register(fixture: &Fixture, endpoint: &str) -> String {
    let registry = AppRegistry::new(fixture.state.db.clone());
    let now = Utc::now().timestamp();
    sqlx::query("INSERT INTO users(id, username, display_name, role, created_at) VALUES ('app-owner', 'app-owner', 'App owner', 'admin', 1)").execute(&fixture.state.db).await.unwrap();
    let app = registry
        .register(
            RegisterApp {
                owner_user_id: "app-owner",
                name: "Acceptance App",
                connection: &AppConnection {
                    endpoint: format!("{endpoint}/app/mcp"),
                    credential_env: Some("TEST_APP_TOKEN".into()),
                    authority: "acceptance".into(),
                    namespace: "acceptance".into(),
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
                team_id: &fixture.team_id,
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
                team_id: &fixture.team_id,
                actor_id: "worker",
                version: 1,
                expected_revision: 0,
                scopes: &scopes,
            },
            now,
        )
        .await
        .unwrap();
    app.id
}

pub(super) async fn revoke(db: &sqlx::SqlitePool, team: &str, app: &str) {
    AppRegistry::new(db.clone())
        .revoke_member_binding(app, team, "worker", 1, Utc::now().timestamp())
        .await
        .unwrap();
}

pub(super) async fn signed_event(fixture: &Fixture, app: &str) {
    use agenthub_db::app_registry::AppEventRouteUpdate;
    use base64::{Engine, engine::general_purpose::STANDARD};
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::Sha256;
    use tower::ServiceExt;
    let now = Utc::now().timestamp();
    let registry = AppRegistry::new(fixture.state.db.clone());
    let scopes = ["write".into()].into();
    registry
        .bind_member(
            AppBindingUpdate {
                app_id: app,
                team_id: &fixture.team_id,
                actor_id: "planner",
                version: 1,
                expected_revision: 0,
                scopes: &scopes,
            },
            now,
        )
        .await
        .unwrap();
    assert_eq!(
        std::env::var("TEST_EVENT_KEY").unwrap(),
        STANDARD.encode([19; 32])
    );
    registry
        .configure_event_key(app, 0, Some("TEST_EVENT_KEY"), now)
        .await
        .unwrap();
    registry
        .configure_event_route(
            AppEventRouteUpdate {
                app_id: app,
                team_id: &fixture.team_id,
                actor_id: "planner",
                expected_revision: 0,
                classes: &["changed".into()].into(),
            },
            now,
        )
        .await
        .unwrap();
    let body = json!({"schema_version":1,"event_id":"real-runtime-dispatch","cursor":1,"team_id":fixture.team_id,"actor_id":"planner","event_class":"changed"}).to_string();
    let mut mac = Hmac::<Sha256>::new_from_slice(&[19; 32]).unwrap();
    mac.update(format!("agenthub.app-event.v1\n{app}\n1\n{now}\n").as_bytes());
    mac.update(body.as_bytes());
    let signature = STANDARD.encode(mac.finalize().into_bytes());
    let router = crate::api::router(fixture.state.clone());
    let mut activation = None;
    for duplicate in [false, true] {
        let request = axum::http::Request::builder()
            .method("POST")
            .uri(format!("/apps/{app}/events"))
            .header("content-type", "application/json")
            .header("x-agenthub-app-key-version", "1")
            .header("x-agenthub-app-timestamp", now.to_string())
            .header("x-agenthub-app-signature", &signature)
            .body(axum::body::Body::from(body.clone()))
            .unwrap();
        let response = router.clone().oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            if duplicate {
                StatusCode::OK
            } else {
                StatusCode::ACCEPTED
            }
        );
        let bytes = axum::body::to_bytes(response.into_body(), 16384)
            .await
            .unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["duplicate"], duplicate);
        if let Some(first) = &activation {
            assert_eq!(first, &value["activation_id"]);
        }
        activation = Some(value["activation_id"].clone());
    }
}
