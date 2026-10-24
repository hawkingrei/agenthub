use super::*;

fn advertise_sources(fixture: &Fixture) {
    let config_path = fixture.directory.join("native-fixture.json");
    let mut config: Value = serde_json::from_slice(&std::fs::read(&config_path).unwrap()).unwrap();
    let hello = &mut config["handshake"];
    hello["request_families"]
        .as_array_mut()
        .unwrap()
        .push(json!("mcp_source"));
    hello["request_methods"].as_array_mut().unwrap().extend([
        json!("mcp_source.register"),
        json!("mcp_source.unregister"),
        json!("mcp_source.query"),
    ]);
    hello["event_families"]
        .as_array_mut()
        .unwrap()
        .push(json!("mcp"));
    std::fs::write(&config_path, serde_json::to_vec(&config).unwrap()).unwrap();
}

#[tokio::test]
async fn native_loop_enables_source_launch_only_after_an_authorized_member_binding() {
    use agenthub_agent_domain::app_tools::AppConnection;
    use agenthub_db::app_registry::{AppBindingUpdate, AppGrantUpdate, AppRegistry, RegisterApp};

    let fixture = with_native_provider(Fixture::new("apps").await, "finish").await;
    advertise_sources(&fixture);
    sqlx::query("INSERT INTO users(id, username, display_name, role, created_at) VALUES ('app-owner', 'app-owner', 'App owner', 'admin', 1)")
        .execute(&fixture.state.db).await.unwrap();
    let registry = AppRegistry::new(fixture.state.db.clone());
    let now = Utc::now().timestamp();
    let app = registry
        .register(
            RegisterApp {
                owner_user_id: "app-owner",
                name: "Launch policy fixture",
                connection: &AppConnection {
                    endpoint: "http://127.0.0.1:9/mcp".into(),
                    credential_env: None,
                    authority: "fixture".into(),
                    namespace: "launch-policy".into(),
                },
                manifest: &super::super::apps::manifest(),
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
    let unbound = fixture.execute("source-unbound").await;
    assert_eq!(unbound.state, LoopActivationState::Finished);
    let arguments: Vec<String> =
        serde_json::from_slice(&std::fs::read(fixture.directory.join("native-argv.json")).unwrap())
            .unwrap();
    assert!(
        !arguments
            .iter()
            .any(|arg| arg == "--allow-controlled-mcp-sources")
    );
    let unbound_digest: String = sqlx::query_scalar(
        "SELECT configuration_digest FROM loop_native_sessions WHERE actor_id = 'worker' AND state = 'bound'",
    ).fetch_one(&fixture.state.db).await.unwrap();

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
    let bound = fixture.execute("source-bound").await;
    assert_eq!(bound.state, LoopActivationState::Finished);
    let arguments: Vec<String> =
        serde_json::from_slice(&std::fs::read(fixture.directory.join("native-argv.json")).unwrap())
            .unwrap();
    assert_eq!(
        arguments
            .iter()
            .filter(|arg| *arg == "--allow-controlled-mcp-sources")
            .count(),
        1
    );
    let bound_digest: String = sqlx::query_scalar(
        "SELECT configuration_digest FROM loop_native_sessions WHERE actor_id = 'worker' AND state = 'bound'",
    ).fetch_one(&fixture.state.db).await.unwrap();
    assert_ne!(unbound_digest, bound_digest);
    let pins = registry
        .activation_pins(&fixture.team_id, &bound.id)
        .await
        .unwrap();
    assert_eq!(pins.len(), 1);
    assert_eq!(pins[0].app_id, app.id);
    let requests =
        std::fs::read_to_string(fixture.directory.join("native-requests.jsonl")).unwrap();
    let registrations: Vec<Value> = requests
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|request| request["payload"]["envelope"]["request"]["type"] == "mcp_source")
        .collect();
    assert_eq!(registrations.len(), 1);
    assert_eq!(
        registrations[0]["payload"]["envelope"]["request"]["payload"]["payload"]["source_id"],
        format!("app-{}", app.id)
    );
    fixture.close().await;
}

#[tokio::test]
async fn native_loop_source_registration_preserves_authorized_proxy_descriptors() {
    let fixture = fixture("no-outcome").await;
    advertise_sources(&fixture);

    let manager = &fixture.state.agents;
    let store = LoopStore::new(fixture.state.db.clone());
    let reservation = fixture.admit("proxy-descriptors").await;
    let mailbox = fixture
        .state
        .teams
        .ensure_loop_mailbox_partition(&fixture.team_id)
        .await
        .unwrap();
    store
        .bind_mailbox(&reservation, &mailbox.id, Utc::now().timestamp())
        .await
        .unwrap();
    let mut context = crate::team::build_team_member_actor_context_for_role(
        &fixture.team_id,
        Some(&mailbox.id),
        "worker",
        "worker",
    );
    context.contract_version = Some(LOOP_ACTIVATION_CONTRACT_VERSION.into());
    manager
        .start_loop_agent(&reservation, context.clone())
        .await
        .unwrap();
    let executable = fixture.directory.join("local-proxy");
    let credential_file = fixture.directory.join("activation-credential.json");
    {
        // Pin synthetic authorized descriptors; this fixture never opens a proxy connection.
        let mut credentials = manager.loop_credentials.lock().await;
        let launch = &mut credentials
            .get_mut("worker")
            .unwrap()
            .native_sources
            .as_mut()
            .unwrap()
            .launch;
        for source in ["app-fixture", "nowledge-mem"] {
            launch
                .add_mcp_proxy(&executable, &credential_file, source, &"a".repeat(64))
                .unwrap();
        }
    }
    let reservation = store
        .reservation(&fixture.team_id, "worker")
        .await
        .unwrap()
        .unwrap();
    store
        .mark_running(&reservation, Utc::now().timestamp())
        .await
        .unwrap();
    let entry = manager
        .prepare_loop_entry(
            &fixture.state.teams,
            &reservation,
            &context,
            "Execute assigned work".into(),
        )
        .await
        .unwrap()
        .expect("fresh activation entry prompt");
    assert!(entry.contains("registered loop activation"));
    let requests: Vec<Value> =
        std::fs::read_to_string(fixture.directory.join("native-requests.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
    let registrations: Vec<_> = requests
        .iter()
        .filter_map(|request| {
            let envelope = &request["payload"]["envelope"];
            (envelope["request"]["type"] == "mcp_source")
                .then_some(&envelope["request"]["payload"]["payload"])
        })
        .collect();
    assert_eq!(registrations.len(), 2);
    for (registration, source) in registrations.iter().zip(["app-fixture", "nowledge-mem"]) {
        assert_eq!(registration["source_id"], source);
        assert_eq!(registration["command"], executable.to_str().unwrap());
        assert_eq!(
            registration["args"],
            json!(["mcp-proxy", "--server-id", source])
        );
        assert_eq!(
            registration["env"],
            json!({"AGENTHUB_LOOP_CREDENTIAL_FILE":credential_file})
        );
    }
    let input = manager.inner.read().await["worker"].input.clone();
    let AgentInput::Rara(runtime) = input else {
        panic!("native runtime")
    };
    assert!(
        runtime
            .send_input(&entry, Some("unrelated-input"), None, None)
            .await
            .is_err()
    );
    let submission = format!(
        "loop-entry:{}:{}",
        reservation.activation_id.as_deref().unwrap(),
        reservation.generation
    );
    runtime
        .send_input(&entry, Some(&submission), None, None)
        .await
        .unwrap();
    let log = std::fs::read_to_string(fixture.directory.join("native-requests.jsonl")).unwrap();
    assert!(!log.contains("unrelated-input"));
    assert_eq!(log.matches("submit_guarded_prompt").count(), 1);
    manager.stop_agent("worker").await.unwrap();
    fixture.close().await;
}
