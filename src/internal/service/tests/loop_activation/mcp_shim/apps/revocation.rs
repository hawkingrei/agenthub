use super::*;

#[tokio::test]
async fn registered_app_revocation_stops_open_sessions_and_idle_subscriptions() {
    for revoke in ["binding", "team", "app"] {
        let fixture = AppFixture::new().await;
        let session = fixture.open().await;
        fixture
            .call(&session, "tools", "tools/list", json!({}))
            .await;
        let mut stream = access::exchange(
            &fixture.h,
            &fixture.token,
            &session,
            request(
                "subscription",
                "subscriptions/listen",
                json!({"notifications":{"toolsListChanged":true}}),
            ),
        )
        .await;
        let frame = tokio::time::timeout(Duration::from_secs(3), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&frame.message_json).unwrap()["method"],
            "notifications/subscriptions/acknowledged"
        );
        let now = chrono::Utc::now().timestamp();
        match revoke {
            "binding" => {
                fixture
                    .registry
                    .revoke_member_binding(
                        &fixture.app.id,
                        &fixture.h.run.team_id,
                        &fixture.h.reservation.actor_id,
                        1,
                        now,
                    )
                    .await
                    .unwrap();
            }
            "team" => {
                fixture
                    .registry
                    .revoke_team_grant(&fixture.app.id, &fixture.h.run.team_id, 1, now)
                    .await
                    .unwrap();
            }
            "app" => {
                fixture
                    .registry
                    .revoke_app(&fixture.app.id, "app-owner", 1, now)
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        }
        // No new RPC or upstream event is needed to discover a committed revocation.
        assert!(
            tokio::time::timeout(Duration::from_secs(3), stream.next())
                .await
                .unwrap()
                .is_none()
        );
        let denied = fixture
            .h
            .service
            .exchange_mcp_proxy(authenticated_request(
                ExchangeMcpProxyRequest {
                    session_id: session.clone(),
                    message_json: request(
                        "denied",
                        "tools/call",
                        json!({"name":"write","arguments":{"body":"denied"}}),
                    )
                    .to_string(),
                },
                &fixture.token,
            ))
            .await
            .err()
            .unwrap();
        assert_eq!(denied.code(), Code::PermissionDenied);
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
            Code::PermissionDenied
        );
        fixture
            .h
            .service
            .close_mcp_proxy(authenticated_request(
                CloseMcpProxyRequest {
                    session_id: session,
                },
                &fixture.token,
            ))
            .await
            .unwrap();
        assert!(
            fixture
                .upstream
                .calls
                .lock()
                .unwrap()
                .iter()
                .all(|call| call["method"] != "tools/call")
        );
        assert!(
            fixture
                .registry
                .active_member_bindings(&fixture.h.run.team_id, &fixture.h.reservation.actor_id)
                .await
                .unwrap()
                .is_empty()
        );
        fixture.close().await;
    }
}

#[tokio::test]
async fn registered_app_revoke_retains_an_admitted_writes_factual_completion() {
    let fixture = AppFixture::new().await;
    let session = fixture.open().await;
    fixture
        .call(&session, "tools", "tools/list", json!({}))
        .await;
    fixture.upstream.hold_write.store(true, Ordering::Release);
    let mut stream = access::exchange(
        &fixture.h,
        &fixture.token,
        &session,
        request(
            "write",
            "tools/call",
            json!({"name":"write","arguments":{"body":"admitted"}}),
        ),
    )
    .await;
    tokio::time::timeout(Duration::from_secs(3), fixture.upstream.received.notified())
        .await
        .unwrap();
    fixture
        .registry
        .revoke_app(
            &fixture.app.id,
            "app-owner",
            1,
            chrono::Utc::now().timestamp(),
        )
        .await
        .unwrap();
    fixture.upstream.release.notify_one();
    let frame = tokio::time::timeout(Duration::from_secs(3), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&frame.message_json).unwrap()["result"]["structuredContent"]
            ["ok"],
        true
    );
    let status: String = sqlx::query_scalar("SELECT status FROM mcp_operation_attempts")
        .fetch_one(&fixture.h.state.db)
        .await
        .unwrap();
    assert_eq!(status, "succeeded");
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
    let tool = history
        .tools
        .iter()
        .find(|tool| tool.surface == agenthub_agent_domain::loop_runtime::LoopToolSurface::McpTool)
        .unwrap();
    assert_eq!(tool.app.as_ref().unwrap().app_id, fixture.app.id);
    assert_eq!(tool.app.as_ref().unwrap().version, 1);
    #[cfg(debug_assertions)]
    {
        let report = agenthub_diagnostics::agent_trace::collect_from_pool(
            &fixture.h.state.db,
            std::env::temp_dir(),
            agenthub_diagnostics::agent_trace::AgentTraceRequest {
                activation_id: fixture.h.reservation.activation_id.clone(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let human = agenthub_diagnostics::agent_trace::render_human(&report);
        assert!(human.contains(&format!("app_id={} version=1", fixture.app.id)));
        for private in [
            "app-private-key",
            "APP_FIXTURE_TOKEN",
            "fixture-workspace",
            "fixture-service",
        ] {
            assert!(!human.contains(private));
        }
    }
    fixture.close().await;
}
