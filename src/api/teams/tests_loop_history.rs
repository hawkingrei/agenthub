async fn history_api_fixture(state: &AppState) -> (String, String, Vec<String>) {
    use agenthub_agent_domain::loop_runtime::{
        LoopLimits, LoopPolicyState, LoopSessionPolicy, LoopSourceReferences, LoopTriggerInput,
        LoopTriggerKind,
    };
    use agenthub_db::loop_runtime::{LoopPolicyUpdate, LoopStore};
    let owner = create_auth_token(state).await;
    let Json(team) = create_team(State(state.clone()), auth_headers_for_token(&owner), Json(CreateTeamRequest {
        name: format!("history-{}", Uuid::new_v4()), description: None,
        spec: json!({"execution_mode":"loop", "entrypoint":"planner", "members":[{"member_id":"planner", "role":"coordinator"}]}),
    })).await.unwrap();
    let store = LoopStore::new(state.db.clone());
    store
        .configure(
            LoopPolicyUpdate {
                actor_id: "planner",
                team_id: &team.id,
                expected_revision: 1,
                state: LoopPolicyState::Suspended,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits::default(),
            },
            100,
        )
        .await
        .unwrap();
    let mut activations = Vec::new();
    for (key, due) in [
        ("private-alpha", None),
        ("private-beta", None),
        ("private-gamma", Some(200)),
    ] {
        let receipt = store
            .accept_trigger(
                &LoopTriggerInput {
                    actor_id: "planner".into(),
                    team_id: team.id.clone(),
                    kind: LoopTriggerKind::Operator,
                    source_key: key.into(),
                    due_at: due,
                    references: LoopSourceReferences::default(),
                },
                100,
            )
            .await
            .unwrap();
        if !activations.contains(&receipt.activation_id) {
            activations.push(receipt.activation_id);
        }
    }
    for (name, status, completed, duration) in [
        ("list_tasks", "succeeded", Some(101_i64), Some(3_i64)),
        ("read_context", "started", None, None),
    ] {
        sqlx::query("INSERT INTO loop_tool_observations(activation_id, generation, surface, tool_name, status, started_at, completed_at, duration_ms) VALUES (?, 1, 'control_rpc', ?, ?, 100, ?, ?)")
            .bind(&activations[0]).bind(name).bind(status).bind(completed).bind(duration)
            .execute(&state.db).await.unwrap();
    }
    (team.id, owner, activations)
}

#[tokio::test]
async fn loop_history_tracing_correlates_lifecycle_without_private_inputs() {
    use agenthub_agent_domain::loop_runtime::{
        LoopAdmission, LoopCleanupDisposition, LoopLimits, LoopOutcome, LoopOutcomeKind,
        LoopPolicyState, LoopSessionPolicy, LoopSourceReferences, LoopTriggerInput,
        LoopTriggerKind,
    };
    use agenthub_db::loop_runtime::{LoopPolicyUpdate, LoopStore};
    use tracing::instrument::WithSubscriber;
    use tracing_subscriber::prelude::*;

    #[derive(Default)]
    struct Fields(serde_json::Map<String, Value>);

    impl tracing::field::Visit for Fields {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            self.0.insert(field.name().into(), json!(format!("{value:?}")));
        }
        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            self.0.insert(field.name().into(), json!(value));
        }
        fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
            self.0.insert(field.name().into(), json!(value));
        }
        fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
            self.0.insert(field.name().into(), json!(value));
        }
    }

    #[derive(Clone, Default)]
    struct Capture(std::sync::Arc<std::sync::Mutex<std::collections::BTreeMap<u64, Fields>>>);

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Capture {
        fn on_new_span(
            &self,
            attributes: &tracing::span::Attributes<'_>,
            id: &tracing::span::Id,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            if !attributes.metadata().name().starts_with("loop.") {
                return;
            }
            let mut fields = Fields::default();
            attributes.record(&mut fields);
            fields.0.insert("name".into(), json!(attributes.metadata().name()));
            self.0.lock().unwrap().insert(id.into_u64(), fields);
        }

        fn on_record(
            &self,
            id: &tracing::span::Id,
            values: &tracing::span::Record<'_>,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            if let Some(fields) = self.0.lock().unwrap().get_mut(&id.into_u64()) {
                values.record(fields);
            }
        }
    }

    let state = build_test_state().await;
    let (team, _, activations) = history_api_fixture(&state).await;
    // Observe span creation and field updates directly. Formatter-generated events and
    // background SQLite span references have independent timing and are not this contract.
    let capture = Capture::default();
    let subscriber = tracing_subscriber::registry().with(capture.clone());
    async {
        let store = LoopStore::new(state.db.clone());
        let receipt = store.accept_trigger(&LoopTriggerInput {
            actor_id: "planner".into(), team_id: team.clone(), kind: LoopTriggerKind::Operator,
            source_key: "private-alpha".into(), due_at: None, references: LoopSourceReferences::default(),
        }, 100).await.unwrap();
        assert!(receipt.duplicate);
        assert_eq!(receipt.activation_id, activations[0]);
        store.configure(LoopPolicyUpdate {
            actor_id: "planner", team_id: &team, expected_revision: 2,
            state: LoopPolicyState::Enabled, session_policy: LoopSessionPolicy::Fresh,
            limits: &LoopLimits::default(),
        }, 101).await.unwrap();
        let LoopAdmission::Admitted(reservation) = store.admit(&team, &activations[0], "private-owner", 102).await.unwrap() else {
            panic!("expected admission");
        };
        sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES ('trace-session', 'planner', 'running', 102)")
            .execute(&state.db).await.unwrap();
        let reservation = store.bind_session(&reservation, "trace-session", 102).await.unwrap();
        store.mark_running(&reservation, 103).await.unwrap();
        store.finish(&reservation, &LoopOutcome {
            kind: LoopOutcomeKind::NoActionableWork, wait_reason: None, task_note_id: None, continuation: None,
        }, 104).await.unwrap();
        store.cleanup_verified(&reservation, LoopCleanupDisposition::Exited, 105).await.unwrap();
    }.with_subscriber(subscriber).await;
    let records = capture.0.lock().unwrap().values()
        .map(|fields| Value::Object(fields.0.clone()))
        .collect::<Vec<_>>();
    let output = serde_json::to_string(&records).unwrap();
    for private in [
        "private-alpha",
        "private-owner",
        "source_key",
        "input_json",
        "owner_id",
    ] {
        assert!(!output.contains(private), "{private}");
    }
    for name in [
        "loop.trigger_intake",
        "loop.admission",
        "loop.bind_session",
        "loop.running",
        "loop.finish",
        "loop.cleanup",
    ] {
        let span = records
            .iter()
            .find(|record| record["name"] == name)
            .unwrap_or_else(|| panic!("missing {name}: {output}"));
        assert_eq!(span["activation_id"], activations[0], "{name}");
        assert_eq!(span["actor_id"], "planner", "{name}");
        assert_eq!(span["team_id"], team, "{name}");
        if name != "loop.trigger_intake" {
            assert_eq!(span["generation"], 1, "{name}");
        }
    }
}

#[tokio::test]
async fn loop_history_api_requires_capability_and_team_access_on_every_surface() {
    let state = build_test_state().await;
    let (team, owner, activations) = history_api_fixture(&state).await;
    let foreign = create_auth_token(&state).await;
    let device = create_auth_token_with_role(&state, UserRole::Device).await;
    let app = super::router(state.clone());
    let base = format!("/{team}/members/planner/loop/activations");
    let mut paths: Vec<String> = [
        String::new(),
        format!("/{}", activations[0]),
        format!("/{}/sources", activations[0]),
        format!("/{}/events", activations[0]),
        format!("/{}/tools", activations[0]),
    ]
    .into_iter()
    .map(|suffix| format!("{base}{suffix}"))
    .collect();
    paths.push(format!("/{team}/members/planner/loop/metrics"));
    for uri in paths {
        for (token, expected) in [
            (None, StatusCode::UNAUTHORIZED),
            (Some(device.as_str()), StatusCode::UNAUTHORIZED),
            (Some(foreign.as_str()), StatusCode::NOT_FOUND),
            (Some(owner.as_str()), StatusCode::OK),
        ] {
            let response = app
                .clone()
                .oneshot(build_json_request(Method::GET, &uri, token, None))
                .await
                .unwrap();
            assert_eq!(response.status(), expected, "{uri}");
        }
    }
    let (viewer_id, viewer) =
        create_auth_token_with_role_and_user_id(&state, UserRole::Viewer).await;
    sqlx::query("INSERT INTO team_members(team_id,user_id,role,created_at,updated_at) VALUES (?,?,'observer',100,100)")
        .bind(&team).bind(&viewer_id).execute(&state.db).await.unwrap();
    assert_eq!(
        app.clone()
            .oneshot(build_json_request(Method::GET, &base, Some(&viewer), None))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    sqlx::query("UPDATE team_members SET revoked_at = 101 WHERE team_id = ? AND user_id = ?")
        .bind(&team)
        .bind(&viewer_id)
        .execute(&state.db)
        .await
        .unwrap();
    assert_eq!(
        app.oneshot(build_json_request(Method::GET, &base, Some(&viewer), None))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn loop_history_api_pages_redacted_records_and_keeps_history_after_membership_changes() {
    let state = build_test_state().await;
    let (team, owner, activations) = history_api_fixture(&state).await;
    let store = agenthub_db::loop_runtime::LoopStore::new(state.db.clone());
    for id in &activations {
        store.cancel(&team, id, 101).await.unwrap();
    }
    sqlx::query("UPDATE team_definitions SET spec_json = ? WHERE id = ?")
        .bind(json!({"execution_mode":"loop","members":[]}).to_string())
        .bind(&team)
        .execute(&state.db)
        .await
        .unwrap();
    let app = super::router(state.clone());
    let base = format!("/{team}/members/planner/loop/activations");
    let mut all = Vec::new();
    let mut uri = format!("{base}?limit=1");
    loop {
        let response = app
            .clone()
            .oneshot(build_json_request(Method::GET, &uri, Some(&owner), None))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let value: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap())
                .unwrap();
        all.extend(
            value["activations"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["id"].as_str().unwrap().to_owned()),
        );
        let Some(cursor) = value["next_cursor"].as_str() else {
            break;
        };
        uri = format!("{base}?limit=1&before_activation_id={cursor}");
    }
    assert_eq!(all.len(), 2);
    assert!(activations.iter().all(|id| all.contains(id)));
    for (suffix, field, cursor_name, expected) in [
        ("sources", "sources", "after_source_id", 2),
        ("events", "events", "after_event_id", 3),
        ("tools", "tools", "after_tool_id", 2),
    ] {
        let endpoint = format!("{base}/{}/{suffix}", activations[0]);
        let mut uri = format!("{endpoint}?limit=1");
        let mut count = 0;
        loop {
            let response = app
                .clone()
                .oneshot(build_json_request(Method::GET, &uri, Some(&owner), None))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
            let text = std::str::from_utf8(&body).unwrap();
            assert!(
                !text.contains("private-")
                    && !text.contains("source_key")
                    && !text.contains("input_json")
            );
            let value: Value = serde_json::from_slice(&body).unwrap();
            count += value[field].as_array().unwrap().len();
            if value["next_cursor"].is_null() {
                break;
            }
            let cursor = value["next_cursor"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value["next_cursor"].to_string());
            uri = format!("{endpoint}?limit=1&{cursor_name}={cursor}");
        }
        assert_eq!(count, expected);
    }
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_sessions")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(sessions, 0);
}

#[tokio::test]
async fn loop_history_api_rejects_foreign_actors_and_invalid_query_cursors() {
    let state = build_test_state().await;
    let (team, owner, activations) = history_api_fixture(&state).await;
    let app = super::router(state);
    let base = format!("/{team}/members/planner/loop/activations");
    for suffix in [
        "?limit=0",
        "?limit=101",
        "?before_activation_id=unknown",
        "?unsupported=1",
    ] {
        let response = app
            .clone()
            .oneshot(build_json_request(
                Method::GET,
                &format!("{base}{suffix}"),
                Some(&owner),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    for suffix in [
        "/sources?after_source_id=unknown",
        "/events?after_event_id=-1",
        "/events?after_event_id=9999",
        "/tools?after_tool_id=-1",
        "/tools?after_tool_id=9999",
        "/tools?limit=0",
        "/tools?limit=101",
    ] {
        let response = app
            .clone()
            .oneshot(build_json_request(
                Method::GET,
                &format!("{base}/{}{suffix}", activations[0]),
                Some(&owner),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    for suffix in ["", "/sources", "/events", "/tools"] {
        let uri = format!(
            "/{team}/members/foreign/loop/activations/{}{suffix}",
            activations[0]
        );
        assert_eq!(
            app.clone()
                .oneshot(build_json_request(Method::GET, &uri, Some(&owner), None))
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
}

#[tokio::test]
async fn loop_history_api_metrics_bound_windows_and_preserve_historical_scope() {
    let state = build_test_state().await;
    let (team, owner, _) = history_api_fixture(&state).await;
    sqlx::query("UPDATE team_definitions SET spec_json = ? WHERE id = ?")
        .bind(json!({"execution_mode":"loop","members":[]}).to_string())
        .bind(&team)
        .execute(&state.db)
        .await
        .unwrap();
    let app = super::router(state);
    let endpoint = format!("/{team}/members/planner/loop/metrics");
    for suffix in [
        "?window_seconds=0",
        "?window_seconds=604801",
        "?window_seconds=-1",
        "?activation_id=private",
    ] {
        assert_eq!(
            app.clone()
                .oneshot(build_json_request(
                    Method::GET,
                    &format!("{endpoint}{suffix}"),
                    Some(&owner),
                    None
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
    let response = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &endpoint,
            Some(&owner),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let text = std::str::from_utf8(&body).unwrap();
    for private in ["private-", "source_key", "input_json", "activation_id"] {
        assert!(!text.contains(private));
    }
    let data: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(data["pending"]["count"], 2);
    assert_eq!(data["duplicates"]["suppressed_total"], 0);
    assert_eq!(data["duplicates"]["sources_with_unknown_baseline"], 0);
    assert!(data["mem"]["latest"].is_null());
    assert_eq!(
        data["observed_at"].as_i64().unwrap() - data["window_start"].as_i64().unwrap(),
        86400
    );
    let foreign = format!("/{team}/members/foreign/loop/metrics");
    let response = app
        .oneshot(build_json_request(
            Method::GET,
            &foreign,
            Some(&owner),
            None,
        ))
        .await
        .unwrap();
    let data: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap()).unwrap();
    assert_eq!(data["pending"]["count"], 0);
}
