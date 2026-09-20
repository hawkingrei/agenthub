use super::*;

#[tokio::test]
async fn registered_app_proxy_pins_versions_filters_scopes_and_validates_native_calls() {
    let fixture = AppFixture::new().await;
    let session = fixture.open().await;
    let discovery = fixture
        .call(&session, "discovery", "server/discover", json!({}))
        .await;
    for name in ["resources", "prompts", "logging"] {
        assert!(discovery["result"]["capabilities"].get(name).is_none());
    }
    let tools = fixture
        .call(&session, "tools", "tools/list", json!({}))
        .await;
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 1);
    assert_eq!(tools["result"]["tools"][0]["name"], "write");
    let before = fixture.upstream.calls.lock().unwrap().len();
    for (id, method, params) in [
        (
            "scope",
            "tools/call",
            json!({"name":"read","arguments":{"body":"foreign"}}),
        ),
        (
            "undeclared",
            "tools/call",
            json!({"name":"undeclared","arguments":{}}),
        ),
        (
            "invalid",
            "tools/call",
            json!({"name":"write","arguments":{"body":17}}),
        ),
        ("resource", "resources/list", json!({})),
    ] {
        assert!(
            fixture
                .call(&session, id, method, params)
                .await
                .get("error")
                .is_some()
        );
    }
    assert_eq!(fixture.upstream.calls.lock().unwrap().len(), before);
    let result = fixture
        .call(
            &session,
            "write",
            "tools/call",
            json!({"name":"write","arguments":{"body":"claimed-actor"}}),
        )
        .await;
    assert_eq!(result["result"]["structuredContent"]["ok"], true);
    assert_eq!(result["result"]["native_extension"], 7);
    let headers = fixture
        .upstream
        .headers
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(headers["mcp-param-x-agenthub-actor-id"], "claimed-actor");
    assert_eq!(headers["x-agenthub-actor-id"], "reviewer");
    assert_eq!(
        headers["x-agenthub-activation-id"],
        fixture.h.reservation.activation_id.as_deref().unwrap()
    );
    assert_eq!(
        fixture.upstream.calls.lock().unwrap().last().unwrap()["params"]["arguments"],
        json!({"body":"claimed-actor"})
    );
    let now = chrono::Utc::now().timestamp();
    let mut changed = manifest();
    changed.tools[0].input_schema["properties"]["body"]["minLength"] = json!(5);
    fixture
        .registry
        .publish_version(&fixture.app.id, "app-owner", 1, &changed, now)
        .await
        .unwrap();
    fixture
        .registry
        .bind_member(
            AppBindingUpdate {
                app_id: &fixture.app.id,
                team_id: &fixture.h.run.team_id,
                actor_id: &fixture.h.reservation.actor_id,
                version: 2,
                expected_revision: 1,
                scopes: &["write".into()].into(),
            },
            now,
        )
        .await
        .unwrap();
    assert!(
        fixture
            .call(
                &session,
                "retained",
                "tools/call",
                json!({"name":"write","arguments":{"body":"old"}})
            )
            .await
            .get("result")
            .is_some()
    );
    assert!(
        crate::mcp_proxy::apps::validate_configuration(
            &fixture.registry,
            &fixture.h.run.team_id,
            &fixture.h.reservation.actor_id,
            fixture.h.reservation.activation_id.as_deref(),
            |_| Some("key".into())
        )
        .await
        .unwrap()
    );
    let history = LoopStore::new(fixture.h.state.db.clone())
        .activation_tool_history(
            &fixture.h.run.team_id,
            &fixture.h.reservation.actor_id,
            fixture.h.reservation.activation_id.as_deref().unwrap(),
            None,
            100,
        )
        .await
        .unwrap()
        .unwrap();
    let tools: Vec<_> = history
        .tools
        .iter()
        .filter(|tool| {
            tool.surface == agenthub_agent_domain::loop_runtime::LoopToolSurface::McpTool
        })
        .collect();
    assert_eq!(tools.len(), 2);
    assert!(tools.iter().all(|tool| {
        tool.app
            .as_ref()
            .is_some_and(|app| app.app_id == fixture.app.id && app.version == 1)
    }));
    assert!(
        history
            .tools
            .iter()
            .filter(|tool| tool.surface
                != agenthub_agent_domain::loop_runtime::LoopToolSurface::McpTool)
            .all(|tool| tool.app.is_none())
    );
    assert!(
        crate::mcp_proxy::apps::validate_configuration(
            &fixture.registry,
            &fixture.h.run.team_id,
            &fixture.h.reservation.actor_id,
            None,
            |_| None
        )
        .await
        .is_err()
    );
    fixture.close().await;
}

#[tokio::test]
async fn registered_app_invalid_output_is_unknown_and_discovery_drift_closes_the_session() {
    let fixture = AppFixture::new().await;
    let session = fixture.open().await;
    fixture
        .call(&session, "tools", "tools/list", json!({}))
        .await;
    fixture
        .upstream
        .invalid_result
        .store(true, Ordering::Release);
    for id in ["invalid-result", "retry"] {
        assert!(
            fixture
                .call(
                    &session,
                    id,
                    "tools/call",
                    json!({"name":"write","arguments":{"body":"effect"}})
                )
                .await
                .get("error")
                .is_some()
        );
    }
    assert_eq!(
        fixture
            .upstream
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call["method"] == "tools/call")
            .count(),
        1
    );
    let status: String = sqlx::query_scalar("SELECT status FROM mcp_operation_attempts")
        .fetch_one(&fixture.h.state.db)
        .await
        .unwrap();
    assert_eq!(status, "outcome_unknown");
    fixture.upstream.schema_drift.store(true, Ordering::Release);
    let second = fixture.open().await;
    assert!(
        fixture
            .call(&second, "drift", "tools/list", json!({}))
            .await
            .get("error")
            .is_some()
    );
    assert!(
        !fixture
            .h
            .state
            .agents
            .mcp_proxy()
            .unwrap()
            .session(&fixture.h.reservation, &second)
            .await
            .unwrap()
            .is_active()
    );
    fixture.close().await;
}

#[tokio::test]
async fn registered_app_session_capacity_is_bounded_and_cleanup_releases_slots() {
    let fixture = AppFixture::new().await;
    let mut sessions = Vec::new();
    for _ in 0..32 {
        sessions.push(fixture.open().await);
    }
    assert_eq!(
        fixture
            .h
            .service
            .open_mcp_proxy(authenticated_request(
                OpenMcpProxyRequest {
                    server_id: fixture.server_id.clone()
                },
                &fixture.token
            ))
            .await
            .unwrap_err()
            .code(),
        Code::ResourceExhausted
    );
    let mut foreign = fixture.h.reservation.clone();
    foreign.generation += 1;
    let hub = fixture.h.state.agents.mcp_proxy().unwrap();
    assert!(hub.session(&foreign, &sessions[0]).await.is_err());
    hub.close(&fixture.h.reservation, &sessions[0])
        .await
        .unwrap();
    fixture.open().await;
    fixture.close().await;
}
