#[tokio::test]
async fn loop_configuration_keeps_a_leader_with_zero_workers_offline() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;
    let Json(team) = create_team(State(state.clone()), headers, Json(CreateTeamRequest {
        name: "offline-loop".into(), description: None,
        spec: json!({"execution_mode":"loop", "entrypoint":"planner", "members":[{"member_id":"planner", "role":"coordinator"}]}),
    })).await.unwrap();
    assert_eq!(team.spec["entrypoint"], "planner");
    assert!(team.spec.get("steps").is_none());
    assert!(team.spec["members"][0].get("prompt").is_none());
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_sessions")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(sessions, 0);
    let policy = agenthub_db::loop_runtime::LoopStore::new(state.db.clone())
        .policy(&team.id, "planner")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        policy.state,
        agenthub_agent_domain::loop_runtime::LoopPolicyState::Disabled
    );
    assert!(policy.mailbox_run_id.is_none());
}

#[tokio::test]
async fn loop_configuration_adds_members_without_eager_start_and_guards_authority_changes() {
    use agenthub_agent_domain::loop_runtime::{LoopLimits, LoopPolicyState, LoopSessionPolicy};
    use agenthub_db::loop_runtime::{LoopPolicyUpdate, LoopStore};
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;
    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "offline-loop-members".into(),
            description: None,
            spec: json!({"execution_mode":"loop", "members":[]}),
        }),
    )
    .await
    .unwrap();
    let Json(team) = update_team_spec(State(state.clone()), headers.clone(), Path(team.id), Json(UpdateTeamSpecRequest {
        expected_updated_at: team.updated_at,
        spec: json!({"execution_mode":"loop", "entrypoint":"planner", "members":[{"member_id":"planner", "role":"coordinator"},{"member_id":"reviewer", "role":"worker"}]}),
    })).await.unwrap();
    let store = LoopStore::new(state.db.clone());
    store
        .configure(
            LoopPolicyUpdate {
                actor_id: "reviewer",
                team_id: &team.id,
                expected_revision: 1,
                state: LoopPolicyState::Enabled,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits::default(),
            },
            100,
        )
        .await
        .unwrap();
    let mut changed = team.spec.clone();
    changed["members"][1]["runtime"] = json!({"workdir":"/new-scope"});
    let error = update_team_spec(
        State(state.clone()),
        headers,
        Path(team.id.clone()),
        Json(UpdateTeamSpecRequest {
            expected_updated_at: team.updated_at,
            spec: changed,
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(error.into_response().status(), StatusCode::CONFLICT);
    assert_eq!(
        state.teams.get_team(&team.id).await.unwrap().spec,
        team.spec
    );
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_sessions")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(sessions, 0);
}

#[tokio::test]
async fn loop_configuration_card_copy_uses_a_new_identity_without_runtime_history() {
    use agenthub_agent_domain::loop_runtime::{
        LoopLimits, LoopPolicyState, LoopSessionPolicy, LoopSourceReferences, LoopTriggerInput,
        LoopTriggerKind,
    };
    use agenthub_db::loop_runtime::{LoopPolicyUpdate, LoopStore};
    let state = build_test_state().await;
    let token = create_auth_token(&state).await;
    let headers = build_json_request(Method::GET, "/", Some(&token), None)
        .headers()
        .clone();
    let Json(source) = create_team(State(state.clone()), headers.clone(), Json(CreateTeamRequest {
        name:"copy-source".into(), description:None,
        spec:json!({"execution_mode":"loop","entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
    })).await.unwrap();
    let store = LoopStore::new(state.db.clone());
    store
        .configure(
            LoopPolicyUpdate {
                actor_id: "planner",
                team_id: &source.id,
                expected_revision: 1,
                state: LoopPolicyState::Suspended,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits::default(),
            },
            100,
        )
        .await
        .unwrap();
    let activation = store
        .accept_trigger(
            &LoopTriggerInput {
                actor_id: "planner".into(),
                team_id: source.id.clone(),
                kind: LoopTriggerKind::Operator,
                source_key: "source-only".into(),
                due_at: None,
                references: LoopSourceReferences::default(),
            },
            101,
        )
        .await
        .unwrap();
    let Json(target) = create_team(
        State(state.clone()),
        headers,
        Json(CreateTeamRequest {
            name: "copy-target".into(),
            description: None,
            spec: json!({"execution_mode":"loop","members":[]}),
        }),
    )
    .await
    .unwrap();
    let response = super::router(state.clone()).oneshot(build_json_request(Method::POST,
        &format!("/{}/members/adopt", target.id), Some(&token), Some(json!({
            "source_agent_id":"planner","name":"copied-leader","expected_updated_at":target.updated_at,
            "spec":{"execution_mode":"loop","entrypoint":"__agenthub_adopted_member__","members":[{"member_id":"__agenthub_adopted_member__","role":"coordinator"}]}
        })))).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let copied = decode_json_body(response).await;
    let actor = copied["agent"]["id"].as_str().unwrap();
    assert_ne!(actor, "planner");
    assert_eq!(copied["team"]["spec"]["members"][0]["member_id"], actor);
    let policy = store.policy(&target.id, actor).await.unwrap().unwrap();
    assert_eq!(policy.state, LoopPolicyState::Disabled);
    assert_eq!(policy.generation, 0);
    assert!(policy.mailbox_run_id.is_none());
    let history: i64 = sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM loop_activations WHERE actor_id = ?1) + (SELECT COUNT(*) FROM agent_sessions WHERE agent_id = ?1) + (SELECT COUNT(*) FROM loop_execution_reservations WHERE actor_id = ?1)")
        .bind(actor).fetch_one(&state.db).await.unwrap();
    assert_eq!(history, 0);
    assert!(
        store
            .activation(&source.id, &activation.activation_id)
            .await
            .unwrap()
            .is_some()
    );
    let response = crate::api::agents::router(state.clone())
        .oneshot(build_json_request(
            Method::GET,
            &format!("/{actor}/.well-known/agent-card"),
            Some(&token),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let card = decode_json_body(response).await;
    assert_eq!(card["loop_execution"]["state"], "disabled");
    assert_eq!(card["skills"], json!([]));
    assert!(
        !card["capability_tags"]
            .as_array()
            .unwrap()
            .contains(&json!("team_step_execution_v1"))
    );
    assert!(card.get("access_token").is_none());
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn loop_configuration_preflight_gates_enable_but_keeps_suspension_available() {
    use std::os::unix::fs::PermissionsExt;
    let state = build_test_state().await;
    let token = create_auth_token(&state).await;
    let headers = build_json_request(Method::GET, "/", Some(&token), None)
        .headers()
        .clone();
    let Json(team) = create_team(State(state.clone()), headers, Json(CreateTeamRequest {
        name:"preflight".into(), description:None,
        spec:json!({"execution_mode":"loop","entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
    })).await.unwrap();
    let app = super::router(state.clone());
    let path = format!("/{}/members/planner/loop", team.id);
    let limits = agenthub_agent_domain::loop_runtime::LoopLimits::default();
    let configure = |state: &str, revision: i64| {
        build_json_request(
            Method::PUT,
            &path,
            Some(&token),
            Some(json!({
                "state":state,"expected_revision":revision,"session_policy":"fresh","limits":limits,
            })),
        )
    };
    let response = app.clone().oneshot(configure("enabled", 1)).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let response = app
        .clone()
        .oneshot(configure("suspended", 1))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = decode_json_body(response).await;
    assert_eq!(body["policy"]["state"], "suspended");
    assert_eq!(body["preflight"]["ready"], false);
    let directory = std::env::temp_dir().join(format!("agenthub-preflight-{}", Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let program = directory.join("claude-agent-acp");
    std::fs::write(&program, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    sqlx::query("UPDATE agents SET command = ?, workdir = ?, worktree_mode = 'use_existing' WHERE id = 'planner'")
        .bind(program.to_string_lossy().as_ref()).bind(directory.to_string_lossy().as_ref()).execute(&state.db).await.unwrap();
    state
        .agents
        .publish_loop_control_endpoint(crate::agent::LoopControlEndpoint {
            target: "http://127.0.0.1:1".into(),
            ca_cert_path: None,
            authz: crate::internal::auth::InternalAuthz::new(
                crate::internal::auth::InternalAuthzConfig {
                    shared_secret: "preflight-fixture".into(),
                    expected_issuer: None,
                    expected_audience: None,
                },
            ),
        })
        .await;
    for (state_name, revision) in [("enabled", 2), ("suspended", 3), ("enabled", 4)] {
        let response = app
            .clone()
            .oneshot(configure(state_name, revision))
            .await
            .unwrap();
        let status = response.status();
        let body = decode_json_body(response).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["policy"]["state"], state_name);
    }
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_sessions")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(
        sessions, 0,
        "policy changes do not manufacture a work trigger"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

async fn create_loop_configuration_fixture(state: &AppState) -> crate::team::TeamDefinitionRecord {
    state.teams.create_team(TeamDefinitionConfig {
        name: format!("loop-configuration-{}", Uuid::new_v4()), description: None,
        spec: json!({"execution_mode":"loop", "entrypoint":"planner", "members":[{"member_id":"planner", "role":"coordinator"}]}),
    }).await.unwrap()
}

#[tokio::test]
async fn loop_configuration_rejects_stale_same_second_updates() {
    let state = build_test_state().await;
    let team = create_loop_configuration_fixture(&state).await;
    let mut changed = team.spec.clone();
    changed["label"] = json!("first");
    let updated = state
        .teams
        .update_team_spec_if_unchanged(&team.id, team.updated_at, changed.clone())
        .await
        .unwrap()
        .unwrap();
    assert!(updated.updated_at > team.updated_at);
    changed["label"] = json!("stale");
    assert!(
        state
            .teams
            .update_team_spec_if_unchanged(&team.id, team.updated_at, changed)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        state.teams.get_team(&team.id).await.unwrap().spec["label"],
        "first"
    );
}

#[tokio::test]
async fn loop_configuration_member_removal_races_trigger_without_losing_accepted_work() {
    use agenthub_agent_domain::loop_runtime::{
        LoopLimits, LoopPolicyState, LoopSessionPolicy, LoopSourceReferences, LoopTriggerInput,
        LoopTriggerKind,
    };
    use agenthub_db::loop_runtime::{LoopPolicyUpdate, LoopStore};
    let state = build_test_state().await;
    let team = create_loop_configuration_fixture(&state).await;
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
    let trigger = LoopTriggerInput {
        actor_id: "planner".into(),
        team_id: team.id.clone(),
        kind: LoopTriggerKind::Operator,
        source_key: "concurrent-removal".into(),
        due_at: None,
        references: LoopSourceReferences::default(),
    };
    let (accepted, removed) = tokio::join!(
        store.accept_trigger(&trigger, 101),
        state.teams.update_team_spec_if_unchanged(
            &team.id,
            team.updated_at,
            json!({"execution_mode":"loop", "members":[]})
        )
    );
    match accepted {
        Ok(receipt) => {
            assert!(removed.is_err());
            assert!(
                store
                    .activation(&team.id, &receipt.activation_id)
                    .await
                    .unwrap()
                    .is_some()
            );
            assert_eq!(
                state.teams.get_team(&team.id).await.unwrap().spec,
                team.spec
            );
            store
                .cancel(&team.id, &receipt.activation_id, 102)
                .await
                .unwrap();
            let error = state.agents.delete_agent("planner").await.unwrap_err();
            assert!(error.to_string().contains("activation history"));
            assert!(state.agents.get_agent("planner").await.is_ok());
            let error = state
                .teams
                .delete_team(
                    &team.id,
                    &std::collections::HashSet::from(["planner".into()]),
                )
                .await
                .unwrap_err();
            assert!(error.to_string().contains("activation history"));
        }
        Err(_) => {
            assert!(removed.unwrap().is_some());
            assert!(store.policy(&team.id, "planner").await.unwrap().is_none());
        }
    }
}

#[tokio::test]
async fn loop_configuration_requires_task_and_permission_reconciliation_before_workspace_change() {
    let state = build_test_state().await;
    let team = create_loop_configuration_fixture(&state).await;
    sqlx::query("INSERT INTO team_tasks(id, team_id, title, status, created_by_actor_id, assigned_member_id, context_json, created_at, updated_at) VALUES ('configuration-task', ?, 'work', 'open', 'user', 'planner', '{}', 1, 1)")
        .bind(&team.id).execute(&state.db).await.unwrap();
    let update = || {
        state.agents.update_team_member_runtime_config(
            "planner",
            "/new-workspace",
            WorktreeMode::UseExisting,
            None,
            None,
        )
    };
    assert!(
        update()
            .await
            .unwrap_err()
            .to_string()
            .contains("canonical work")
    );
    sqlx::query("UPDATE team_tasks SET status = 'completed' WHERE id = 'configuration-task'")
        .execute(&state.db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at, ended_at) VALUES ('configuration-session', 'planner', 'exited', 1, 2)").execute(&state.db).await.unwrap();
    sqlx::query("INSERT INTO acp_permission_requests(id, agent_id, session_id, options_json, status, created_at) VALUES ('configuration-permission', 'planner', 'configuration-session', '[]', 'pending', 1)").execute(&state.db).await.unwrap();
    assert!(
        update()
            .await
            .unwrap_err()
            .to_string()
            .contains("permission")
    );
    sqlx::query("UPDATE acp_permission_requests SET status = 'expired' WHERE id = 'configuration-permission'").execute(&state.db).await.unwrap();
    update().await.unwrap();
    assert_eq!(
        state.agents.get_agent("planner").await.unwrap().workdir,
        "/new-workspace"
    );
}

#[tokio::test]
async fn loop_configuration_preflight_reports_required_capabilities_without_secrets() {
    use agenthub_agent_domain::loop_runtime::LoopSessionPolicy;
    let state = build_test_state().await;
    let mut team = create_loop_configuration_fixture(&state).await;
    team.spec["required_capabilities"] = json!(["nowledge_mem", "unsupported_write_tool"]);
    sqlx::query("UPDATE agents SET command = '/missing/provider', workdir = '/missing/workspace', args = '[\"secret-fixture\"]' WHERE id = 'planner'")
        .execute(&state.db).await.unwrap();
    let preflight = state
        .agents
        .loop_preflight(&team.id, &team.spec, "planner", LoopSessionPolicy::Resume)
        .await
        .unwrap();
    assert!(!preflight.ready);
    for reason in [
        "provider_binary_unavailable",
        "workspace_unavailable",
        "mem_binding_unavailable",
        "mem_proxy_unavailable",
        "required_capability_unavailable",
    ] {
        assert!(
            preflight.blockers.contains(&reason),
            "{reason}: {:?}",
            preflight.blockers
        );
    }
    assert!(
        !serde_json::to_string(&preflight)
            .unwrap()
            .contains("secret-fixture")
    );
    assert_eq!(
        preflight.warnings,
        ["resume_capability_is_negotiated_before_entry"]
    );
}

#[tokio::test]
async fn loop_configuration_routes_require_authority_and_current_membership() {
    let state = build_test_state().await;
    let token = create_auth_token(&state).await;
    let viewer = create_auth_token_with_role(&state, UserRole::Viewer).await;
    let team = create_loop_configuration_fixture(&state).await;
    let app = super::router(state.clone());
    let path = format!("/{}/members/planner/loop", team.id);
    assert_eq!(
        app.clone()
            .oneshot(build_json_request(Method::GET, &path, None, None))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let response = app.clone().oneshot(build_json_request(Method::PUT, &path, Some(&viewer), Some(json!({
        "state":"suspended", "expected_revision":1, "session_policy":"fresh", "limits":agenthub_agent_domain::loop_runtime::LoopLimits::default(),
    })))).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .oneshot(build_json_request(
            Method::GET,
            &format!("/{}/members/missing/loop", team.id),
            Some(&token),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn loop_configuration_retained_history_blocks_deletion_and_identity_reuse() {
    use agenthub_agent_domain::loop_runtime::{
        LoopSourceReferences, LoopTriggerInput, LoopTriggerKind,
    };
    use agenthub_db::loop_runtime::LoopStore;
    let state = build_test_state().await;
    let team = create_loop_configuration_fixture(&state).await;
    sqlx::query("UPDATE loop_policies SET state = 'suspended' WHERE actor_id = 'planner'")
        .execute(&state.db)
        .await
        .unwrap();
    let store = LoopStore::new(state.db.clone());
    let receipt = store
        .accept_trigger(
            &LoopTriggerInput {
                actor_id: "planner".into(),
                team_id: team.id.clone(),
                kind: LoopTriggerKind::Operator,
                source_key: "retained-history".into(),
                due_at: None,
                references: LoopSourceReferences::default(),
            },
            100,
        )
        .await
        .unwrap();
    store
        .cancel(&team.id, &receipt.activation_id, 101)
        .await
        .unwrap();
    assert!(
        state
            .agents
            .delete_agent("planner")
            .await
            .unwrap_err()
            .to_string()
            .contains("activation history")
    );
    assert!(
        state
            .teams
            .delete_team(
                &team.id,
                &std::collections::HashSet::from(["planner".into()])
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("activation history")
    );
    state
        .teams
        .update_team_spec_if_unchanged(
            &team.id,
            team.updated_at,
            json!({"execution_mode":"loop", "members":[]}),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(store.policy(&team.id, "planner").await.unwrap().is_some());
    let error = state
        .teams
        .create_team(TeamDefinitionConfig {
            name: "identity-reuse".into(),
            description: None,
            spec: team.spec,
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("retains this identity"));
    assert!(
        store
            .activation(&team.id, &receipt.activation_id)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn loop_configuration_acknowledged_mail_still_requires_a_visible_reply() {
    let state = build_test_state().await;
    let team = create_loop_configuration_fixture(&state).await;
    let run = state
        .teams
        .ensure_loop_mailbox_partition(&team.id)
        .await
        .unwrap();
    let incoming = state.teams.send_actor_message(crate::team::SendActorMessageInput {
        run_id:&run.id, from_actor_id:"user:alice", from_peer_id:agenthub_team_actor::ACTOR_MAIN_PEER_ID,
        to_actor_id:"planner", to_peer_id:agenthub_team_actor::ACTOR_MAIN_PEER_ID,
        channel:"coordination", transport:TeamActorMessageTransport::Local, route:None,
        payload:json!({"type":"chat_message", "text":"Need an update", "requires_user_visible_reply":true}),
        idempotency_key:None, message_kind:None,
    }).await.unwrap();
    sqlx::query("UPDATE team_actor_messages SET status = 'delivered' WHERE id = ?")
        .bind(incoming.message_id)
        .execute(&state.db)
        .await
        .unwrap();
    let update = || {
        state.agents.update_team_member_runtime_config(
            "planner",
            "/new-workspace",
            WorktreeMode::UseExisting,
            None,
            None,
        )
    };
    assert!(
        update()
            .await
            .unwrap_err()
            .to_string()
            .contains("reply obligations")
    );
    state
        .teams
        .send_actor_message(crate::team::SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: agenthub_team_actor::ACTOR_MAIN_PEER_ID,
            to_actor_id: "user:alice",
            to_peer_id: agenthub_team_actor::ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"type":"chat_message", "text":"The update is available"}),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .unwrap();
    update().await.unwrap();
}

#[tokio::test]
async fn loop_configuration_active_snapshot_rejects_acp_mutation_and_uncertain_owner() {
    use agenthub_agent_domain::loop_runtime::{
        LoopAdmission, LoopSessionPolicy, LoopSourceReferences, LoopTriggerInput, LoopTriggerKind,
    };
    use agenthub_db::loop_runtime::LoopStore;
    let state = build_test_state().await;
    let team = create_loop_configuration_fixture(&state).await;
    sqlx::query("UPDATE loop_policies SET state = 'enabled' WHERE actor_id = 'planner'")
        .execute(&state.db)
        .await
        .unwrap();
    let store = LoopStore::new(state.db.clone());
    let receipt = store
        .accept_trigger(
            &LoopTriggerInput {
                actor_id: "planner".into(),
                team_id: team.id.clone(),
                kind: LoopTriggerKind::Operator,
                source_key: "active-config".into(),
                due_at: None,
                references: LoopSourceReferences::default(),
            },
            100,
        )
        .await
        .unwrap();
    assert!(matches!(
        store
            .admit(&team.id, &receipt.activation_id, "old-daemon", 100)
            .await
            .unwrap(),
        LoopAdmission::Admitted(_)
    ));
    for result in [
        state.agents.set_acp_mode("planner", "edit").await,
        state.agents.set_acp_model("planner", "model").await,
        state
            .agents
            .set_acp_config("planner", "thinking", "high")
            .await,
    ] {
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("activation configuration is immutable")
        );
    }
    let preflight = state
        .agents
        .loop_preflight(&team.id, &team.spec, "planner", LoopSessionPolicy::Fresh)
        .await
        .unwrap();
    assert!(preflight.blockers.contains(&"unfenced_executor_retained"));
}
