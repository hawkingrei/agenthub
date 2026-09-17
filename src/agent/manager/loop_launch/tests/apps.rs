use agenthub_agent_domain::app_tools::{AppConnection, AppManifest, AppReplayPolicy, AppTool};
use agenthub_db::app_registry::{AppBindingUpdate, AppGrantUpdate, AppRegistry, RegisterApp};
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

mod provider;
pub(super) const PROVIDER: &str = provider::SCRIPT;

fn manifest() -> AppManifest {
    AppManifest {
        schema_version: 1,
        scopes: ["write".into()].into(),
        tools: vec![AppTool {
            name: "write".into(),
            input_schema: json!({"type":"object","properties":{"body":{"type":"string"}},"required":["body"],"additionalProperties":false}),
            output_schema: Some(
                json!({"type":"object","properties":{"written":{"type":"boolean"}},"required":["written"]}),
            ),
            required_scopes: ["write".into()].into(),
            replay: AppReplayPolicy::NonIdempotent,
        }],
    }
}

struct Upstream {
    calls: Mutex<Vec<Value>>,
    headers: Mutex<Vec<HeaderMap>>,
}

async fn app_http(
    State(state): State<Arc<Upstream>>,
    headers: HeaderMap,
    Json(message): Json<Value>,
) -> Response {
    assert_eq!(headers["authorization"], "Bearer app-private-key");
    assert_eq!(headers["x-agenthub-actor-id"], "worker");
    assert_eq!(headers["x-agenthub-app-version"], "1");
    state.calls.lock().unwrap().push(message.clone());
    state.headers.lock().unwrap().push(headers);
    let result = match message["method"].as_str().unwrap() {
        "initialize" => {
            json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"App fixture","version":"1"}})
        }
        "notifications/initialized" => return StatusCode::ACCEPTED.into_response(),
        "tools/list" => {
            let tool = manifest().tools.remove(0);
            json!({"tools":[{"name":tool.name,"inputSchema":tool.input_schema,"outputSchema":tool.output_schema}]})
        }
        "tools/call" => {
            assert_eq!(
                message["params"]["arguments"],
                json!({"body":"native-app-input"})
            );
            json!({"content":[],"structuredContent":{"written":true}})
        }
        _ => panic!("unexpected App method"),
    };
    Json(json!({"jsonrpc":"2.0","id":message["id"],"result":result})).into_response()
}

#[tokio::test]
async fn registered_app_launch_uses_real_shim_and_isolates_all_registered_credentials() {
    mcp::run_configured_child_with_env(
        "agent::manager::loop_launch::tests::apps::registered_app_child",
        &[
            ("TEST_APP_TOKEN", "app-private-key"),
            ("TEST_UNUSED_APP_TOKEN", "unused-private-key"),
            ("TEST_REVOKED_APP_TOKEN", "revoked-private-key"),
        ],
    )
    .await;
}

#[tokio::test]
#[ignore = "Executed by the parent with an isolated inherited environment"]
async fn registered_app_child() {
    for available in [true, false] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/mcp", listener.local_addr().unwrap());
        let state = Arc::new(Upstream {
            calls: Mutex::new(Vec::new()),
            headers: Mutex::new(Vec::new()),
        });
        let http = if available {
            let router = axum::Router::new()
                .route("/mcp", post(app_http))
                .with_state(state.clone());
            Some(tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            }))
        } else {
            drop(listener);
            None
        };
        let fixture = Fixture::new("apps").await;
        if !available {
            std::fs::write(fixture.directory.join("app-unavailable"), "").unwrap();
        }
        sqlx::query("INSERT INTO users(id, username, display_name, role, created_at) VALUES ('app-owner', 'app-owner', 'App owner', 'admin', 1)")
            .execute(&fixture.state.db).await.unwrap();
        let registry = AppRegistry::new(fixture.state.db.clone());
        let now = Utc::now().timestamp();
        let mut selected = None;
        for (index, key) in [
            "TEST_APP_TOKEN",
            "TEST_UNUSED_APP_TOKEN",
            "TEST_REVOKED_APP_TOKEN",
        ]
        .into_iter()
        .enumerate()
        {
            let app = registry
                .register(
                    RegisterApp {
                        owner_user_id: "app-owner",
                        name: "Fixture App",
                        connection: &AppConnection {
                            endpoint: endpoint.clone(),
                            credential_env: Some(key.into()),
                            authority: "fixture".into(),
                            namespace: format!("namespace-{index}"),
                        },
                        manifest: &manifest(),
                    },
                    now,
                )
                .await
                .unwrap();
            if index == 0 {
                selected = Some(app);
            } else if index == 2 {
                registry
                    .revoke_app(&app.id, "app-owner", 1, now)
                    .await
                    .unwrap();
            }
        }
        let app = selected.unwrap();
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
        let activation = fixture.execute("registered-app").await;
        assert_eq!(activation.state, LoopActivationState::Finished);
        let pins = registry
            .activation_pins(&fixture.team_id, &activation.id)
            .await
            .unwrap();
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].app_id, app.id);
        assert_eq!(pins[0].version, 1);
        let log = std::fs::read_to_string(fixture.directory.join("requests.jsonl")).unwrap();
        let events: Vec<Value> = log
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(
            events
                .iter()
                .any(|event| event["app_available"] == available)
        );
        for private in [
            &endpoint,
            "app-private-key",
            "unused-private-key",
            "revoked-private-key",
            "native-app-input",
        ] {
            assert!(!log.contains(private));
        }
        if available {
            assert!(state.headers.lock().unwrap().iter().all(
                |headers| headers["x-agenthub-activation-id"] == activation.id
                    && headers["x-agenthub-team-id"] == fixture.team_id
            ));
            let status: String = sqlx::query_scalar("SELECT status FROM mcp_operation_attempts")
                .fetch_one(&fixture.state.db)
                .await
                .unwrap();
            assert_eq!(status, "succeeded");
        }
        fixture.close().await;
        if let Some(http) = http {
            http.abort();
        }
    }
}
