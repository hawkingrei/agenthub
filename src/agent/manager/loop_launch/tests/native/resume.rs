use super::*;

#[tokio::test]
async fn native_resume_requires_complete_recovery_before_creating_or_binding_a_conversation() {
    for missing in [
        "session.resume",
        "session.query_recovery",
        "session.resolve_recovery",
        "session.evaluate_reentry",
        "approval_persistence",
    ] {
        let fixture = fixture("finish").await;
        let path = fixture.directory.join("native-fixture.json");
        let mut config: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let handshake = &mut config["handshake"];
        handshake["capabilities"]["approval_persistence"] =
            json!(missing != "approval_persistence");
        let methods = handshake["request_methods"].as_array_mut().unwrap();
        methods.extend(
            [
                "session.resume",
                "session.query_recovery",
                "session.resolve_recovery",
                "session.evaluate_reentry",
            ]
            .into_iter()
            .filter(|method| *method != missing)
            .map(|method| json!(method)),
        );
        std::fs::write(path, config.to_string()).unwrap();
        fixture.session_policy(LoopSessionPolicy::Resume).await;
        let owner = fixture.admit("unsupported-resume").await;
        assert!(
            fixture
                .state
                .agents
                .execute_loop_activation(fixture.state.teams.clone(), owner.clone())
                .await
                .is_err(),
            "{missing}"
        );
        fixture
            .state
            .agents
            .fence_loop_reservation(&owner)
            .await
            .unwrap();
        let bindings: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM loop_native_sessions WHERE actor_id = 'worker'",
        )
        .fetch_one(&fixture.state.db)
        .await
        .unwrap();
        assert_eq!(bindings, 0, "incomplete recovery must fail before opening");
        let requests = std::fs::read_to_string(fixture.directory.join("native-requests.jsonl"))
            .unwrap_or_default();
        assert!(!requests.contains("create_session") && !requests.contains("resume_session"));
        fixture.close().await;
    }
}
