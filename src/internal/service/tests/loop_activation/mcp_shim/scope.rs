use super::*;
use agenthub_config::{
    AppConfig, NowledgeMemConfig, NowledgeMemProfileConfig, NowledgeMemTeamBindingConfig,
};
use axum::{extract::Path, routing::get};
use std::collections::HashMap;

struct ScopeUpstream {
    db: sqlx::SqlitePool,
    writes: Mutex<Vec<String>>,
}

async fn membership(Path(namespace): Path<String>, headers: HeaderMap) -> Json<Value> {
    assert!(
        headers["authorization"]
            .to_str()
            .unwrap()
            .starts_with("Bearer scope-key-")
    );
    let workspace = if namespace == "foreign" {
        "9405e041-1948-46d4-bc91-404e64ab6006"
    } else {
        "cd270331-80bc-4f90-8cc0-3fefbc7f74ab"
    };
    Json(json!({"workspace_id":workspace,
        "key_scope":{"scope_mode":"narrowed","grants":["space-a"],"write_space":"space-a"},
        "key_write_target":{"write_space":"space-a","write_space_live":true}}))
}

async fn scoped_mcp(
    State(upstream): State<Arc<ScopeUpstream>>,
    Path(namespace): Path<String>,
    headers: HeaderMap,
    Json(message): Json<Value>,
) -> Response {
    assert!(
        headers["authorization"]
            .to_str()
            .unwrap()
            .starts_with("Bearer scope-key-")
    );
    let result = match message["method"].as_str().unwrap() {
        "server/discover" => discovery::result(),
        "tools/list" => json!({"resultType":"complete","tools":[{"name":"write",
            "inputSchema":{"type":"object","properties":{"body":{"type":"string"},"space_id":{"type":"string"}}}}]}),
        "tools/call" => {
            assert_eq!(
                message["params"]["arguments"],
                json!({"body":"private-effect","space_id":"space-a"})
            );
            let sent: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM mcp_operation_attempts WHERE status = 'sent'",
            )
            .fetch_one(&upstream.db)
            .await
            .unwrap();
            assert_eq!(sent, 1);
            upstream.writes.lock().unwrap().push(namespace.clone());
            if namespace == "original" {
                // The effect happened, but EOF supplies no response or evidence that it failed.
                return ([("content-type", "text/event-stream")], "").into_response();
            }
            json!({"resultType":"complete","content":[{"type":"text","text":"done"}]})
        }
        _ => panic!("unexpected scoped MCP request"),
    };
    Json(json!({"jsonrpc":"2.0","id":message["id"],"result":result})).into_response()
}

fn message(id: &str, method: &str) -> Value {
    let mut request = discovery::request(id, "scope-fixture");
    request["method"] = method.into();
    if method == "tools/call" {
        request["params"]["name"] = "write".into();
        request["params"]["arguments"] = json!({"body":"private-effect"});
    }
    request
}

#[tokio::test]
async fn configured_mcp_endpoint_alias_cannot_replay_unknown_write_in_a_new_activation() {
    let mut h = setup().await;
    let upstream = Arc::new(ScopeUpstream {
        db: h.state.db.clone(),
        writes: Mutex::new(Vec::new()),
    });
    let router = axum::Router::new()
        .route("/{namespace}/members/me", get(membership))
        .route("/{namespace}/mcp", post(scoped_mcp))
        .with_state(upstream.clone());
    let mut endpoints = Vec::new();
    let mut servers = Vec::new();
    for _ in 0..2 {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        endpoints.push(format!("http://{}", listener.local_addr().unwrap()));
        let router = router.clone();
        servers.push(tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap()
        }));
    }
    let hub = h.state.agents.mcp_proxy().unwrap();
    let loops = LoopStore::new(h.state.db.clone());
    let mut fingerprints = Vec::new();
    for (index, namespace) in ["original", "alias", "foreign"].into_iter().enumerate() {
        if index > 0 {
            hub.release_activation(&h.reservation).await;
            let now = chrono::Utc::now().timestamp();
            loops
                .cancel(
                    &h.run.team_id,
                    h.reservation.activation_id.as_deref().unwrap(),
                    now,
                )
                .await
                .unwrap();
            loops
                .cleanup_verified(&h.reservation, LoopCleanupDisposition::Exited, now)
                .await
                .unwrap();
            sqlx::query("UPDATE agent_sessions SET status = 'exited', ended_at = ? WHERE id = ?")
                .bind(now)
                .bind(&h.reservation.session_id)
                .execute(&h.state.db)
                .await
                .unwrap();
            h.reservation = reserve_fixture(&h.state, &h.run, namespace, true).await;
        }
        let profile = format!("profile-{index}");
        let config = AppConfig {
            nowledge_mem: Some(NowledgeMemConfig {
                profiles: Some(HashMap::from([(
                    profile.clone(),
                    NowledgeMemProfileConfig {
                        endpoint: format!("{}/{namespace}/mcp", endpoints[index.min(1)]),
                        credential_env: format!("TEAM_SCOPE_KEY_{index}"),
                        tool_set: None,
                    },
                )])),
                team_bindings: Some(HashMap::from([(
                    h.run.team_id.clone(),
                    NowledgeMemTeamBindingConfig {
                        profile,
                        space_id: "space-a".into(),
                        actor_profiles: None,
                    },
                )])),
            }),
            ..Default::default()
        };
        let resolved = crate::mcp_proxy::configured::resolve_mem(
            &config,
            &h.run.team_id,
            &h.reservation.actor_id,
            |_| Some(format!("scope-key-{index}")),
        )
        .await
        .unwrap();
        fingerprints.push(resolved.fingerprint);
        hub.mount(&h.reservation, resolved.binding).await.unwrap();
        let token = signed_token(
            &h.authz,
            &h.reservation,
            &h.run.id,
            vec![InternalAction::McpProxy.as_str().into()],
        );
        let session = h
            .service
            .open_mcp_proxy(authenticated_request(
                OpenMcpProxyRequest {
                    server_id: "nowledge-mem".into(),
                },
                &token,
            ))
            .await
            .unwrap()
            .into_inner()
            .session_id;
        access::call(&h, &token, &session, message("discover", "server/discover")).await;
        access::call(&h, &token, &session, message("catalog", "tools/list")).await;
        let request = message(&format!("write-{index}"), "tools/call");
        let response = h
            .service
            .exchange_mcp_proxy(authenticated_request(
                crate::internal::proto::agenthub::internal::v1::ExchangeMcpProxyRequest {
                    session_id: session,
                    message_json: request.to_string(),
                },
                &token,
            ))
            .await;
        let frames = response.unwrap().into_inner().collect::<Vec<_>>().await;
        assert_eq!(frames.len(), 1);
        let frame = frames[0].as_ref().unwrap();
        assert!(frame.finished);
        let value: Value = serde_json::from_str(&frame.message_json).unwrap();
        assert_eq!(value["id"], request["id"]);
        match namespace {
            "alias" => assert_eq!(
                value["error"],
                json!({"code":-32000,
                "message":"MCP call may have taken effect; replay is not authorized"})
            ),
            "foreign" => assert_eq!(value["result"]["content"][0]["text"], "done"),
            _ => assert_eq!(value["error"]["code"], -32000),
        }
    }
    assert_ne!(fingerprints[0], fingerprints[1]);
    assert_eq!(*upstream.writes.lock().unwrap(), ["original", "foreign"]);
    let records: Vec<(String, String)> =
        sqlx::query_as("SELECT status, intent_json FROM mcp_operations ORDER BY created_at, rowid")
            .fetch_all(&h.state.db)
            .await
            .unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].0, "outcome_unknown");
    assert_eq!(records[1].0, "succeeded");
    let intents: Vec<Value> = records
        .iter()
        .map(|(_, raw)| serde_json::from_str(raw).unwrap())
        .collect();
    assert_ne!(intents[0]["scope_digest"], intents[1]["scope_digest"]);
    for (_, record) in records {
        assert_eq!(
            serde_json::from_str::<Value>(&record).unwrap()["scope_identity"],
            "verified_authority"
        );
        assert!(
            !record.contains("private-effect")
                && !record.contains("scope-key")
                && !record.contains("http://")
        );
    }
    h.state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(3))
        .await
        .unwrap();
    for server in servers {
        server.abort();
    }
    h.http.abort();
}
