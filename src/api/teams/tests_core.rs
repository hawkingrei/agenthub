async fn wait_for_agent_event_history_cleanup(state: &AppState, agent_id: &str, session_id: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let event_db_path = state.agents.test_event_db_path_for_agent(agent_id);
    let event_db_wal_path = {
        let mut raw = event_db_path.as_os_str().to_os_string();
        raw.push("-wal");
        std::path::PathBuf::from(raw)
    };
    let event_db_shm_path = {
        let mut raw = event_db_path.as_os_str().to_os_string();
        raw.push("-shm");
        std::path::PathBuf::from(raw)
    };
    loop {
        let main_exists = tokio::fs::try_exists(&event_db_path)
            .await
            .expect("check member event db path");
        let wal_exists = tokio::fs::try_exists(&event_db_wal_path)
            .await
            .expect("check member event db wal path");
        let shm_exists = tokio::fs::try_exists(&event_db_shm_path)
            .await
            .expect("check member event db shm path");
        if !main_exists && !wal_exists && !shm_exists {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "member event db and sidecars were not deleted after delete_team: agent_id={agent_id}, session_id={session_id}, path={}, wal_path={}, shm_path={}",
            event_db_path.display(),
            event_db_wal_path.display(),
            event_db_shm_path.display()
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn wait_for_running_agent_session(
    state: &AppState,
    agent_id: &str,
    expected_session_id: &str,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        if state
            .agents
            .running_session_id_for_agent(agent_id)
            .await
            .as_deref()
            == Some(expected_session_id)
        {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "running session did not stabilize before force_new_session: agent_id={agent_id}, expected_session_id={expected_session_id}"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

fn assert_forced_restart_runtime(
    runtime: &crate::team::TeamRuntimeControlRecord,
    team_id: &str,
    member_id: &str,
    original_session_id: &str,
) {
    assert_eq!(runtime.team_id, team_id);
    assert_eq!(runtime.status, crate::team::TeamRuntimeStatus::Running);
    assert_eq!(runtime.members.len(), 1);
    assert_eq!(runtime.members[0].member_id, member_id);
    assert_eq!(runtime.members[0].action, "forced_restart");
    assert_ne!(runtime.members[0].session_id, original_session_id);
}

#[tokio::test]
async fn teams_api_requires_authorization() {
    let state = build_test_state().await;
    let err = list_teams(State(state), HeaderMap::new())
        .await
        .expect_err("should reject without auth");
    assert_eq!(err.into_response().status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn teams_api_create_list_get_and_reject_duplicate_name() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(created) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "review-team".to_string(),
            description: Some("team for review".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");
    assert_eq!(created.spec["spec_version"], Value::from(1));
    assert_eq!(created.spec["coordinator_member_id"], Value::from("planner"));
    assert_eq!(created.spec["entrypoint"], Value::from("coordinator_plan"));
    assert_eq!(
        created.spec["steps"][0]["step_key"],
        Value::from("coordinator_plan")
    );
    assert_eq!(
        created.spec["steps"][0]["member_id"],
        Value::from("planner")
    );
    assert!(
        created.spec["members"][0]["prompt"]
            .as_str()
            .is_some_and(|prompt| !prompt.trim().is_empty())
    );
    assert!(
        created.spec["members"][0].get("skills").is_none(),
        "normalized team spec should not persist member skill config"
    );

    let Json(listed) = list_teams(State(state.clone()), headers.clone())
        .await
        .expect("list teams");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);

    let Json(found) = get_team(
        State(state.clone()),
        headers.clone(),
        Path(created.id.clone()),
    )
    .await
    .expect("get team");
    assert_eq!(found.name, "review-team");

    let err = create_team(
        State(state),
        headers,
        Json(CreateTeamRequest {
            name: "review-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect_err("duplicate team name should fail");
    assert_eq!(err.into_response().status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn teams_api_allows_creating_team_without_members() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(created) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "empty-team".to_string(),
            description: Some("goal only".to_string()),
            spec: json!({
                "spec_version": 1,
                "members": [],
            }),
        }),
    )
    .await
    .expect("create team without members");

    assert_eq!(created.spec["spec_version"], Value::from(1));
    assert_eq!(created.spec["members"], json!([]));
    assert!(created.spec.get("entrypoint").is_none());
    assert!(created.spec.get("coordinator_member_id").is_none());
    assert!(created.spec.get("steps").is_none());

    let Json(runtime) = get_team_runtime(State(state), headers, Path(created.id.clone()))
        .await
        .expect("describe empty team runtime");
    assert_eq!(runtime.team_id, created.id);
    assert_eq!(runtime.status, crate::team::TeamRuntimeStatus::Stopped);
    assert!(runtime.members.is_empty());
}

#[tokio::test]
async fn teams_api_update_team_spec_adds_first_member_and_starts_runtime() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;
    insert_legacy_team_member_agent(&state, "planner").await;

    let Json(created) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "bootstrap-team".to_string(),
            description: Some("team bootstrap".to_string()),
            spec: json!({
                "spec_version": 1,
                "members": [],
            }),
        }),
    )
    .await
    .expect("create empty team");

    let Json(updated) = update_team_spec(
        State(state.clone()),
        headers.clone(),
        Path(created.id.clone()),
        Json(UpdateTeamSpecRequest {
            expected_updated_at: created.updated_at,
            spec: json!({
                "spec_version": 1,
                "entrypoint": "planner",
                "members": [
                    {
                        "member_id": "planner",
                        "role": "coordinator",
                    }
                ],
            }),
        }),
    )
    .await
    .expect("update team spec");

    assert_eq!(updated.spec["coordinator_member_id"], Value::from("planner"));
    assert_eq!(updated.spec["entrypoint"], Value::from("coordinator_plan"));
    assert_eq!(
        updated.spec["steps"][0]["member_id"],
        Value::from("planner")
    );
    let Json(runtime) = get_team_runtime(State(state), headers, Path(updated.id.clone()))
        .await
        .expect("describe updated team runtime");
    assert_eq!(runtime.team_id, updated.id);
    assert_eq!(runtime.members.len(), 1);
    assert_eq!(runtime.members[0].member_id, "planner");
}

#[tokio::test]
async fn teams_api_runtime_reconciles_stale_running_member_sessions() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "stale-runtime-team".to_string(),
            description: Some("stale runtime reconcile coverage".to_string()),
            spec: json!({
                "entrypoint": "planner",
                "members": [
                    {
                        "member_id": "planner",
                        "role": "coordinator",
                    }
                ],
            }),
        }),
    )
    .await
    .expect("create team");

    let _ = stop_team(State(state.clone()), headers.clone(), Path(team.id.clone()))
        .await
        .expect("stop team runtime before seeding stale rows");
    assert!(
        state
            .agents
            .running_session_id_for_agent("planner")
            .await
            .is_none(),
        "planner runtime should be stopped before stale-row reconciliation coverage"
    );

    let now = Utc::now().timestamp();
    sqlx::query("INSERT OR IGNORE INTO safe_paths (path, created_at) VALUES (?1, ?2)")
        .bind("/tmp/team-runtime-stale")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert safe path for stale planner");
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref,
            code_mode, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, 'use_existing', NULL, NULL, 0, 'running', ?6, ?7)
        "#,
    )
    .bind("planner")
    .bind("planner-agent")
    .bind("/tmp/team-runtime-stale")
    .bind("/usr/bin/env")
    .bind("[]")
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert stale planner agent");
    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
        VALUES (?1, ?2, 'running', ?3, NULL)
        "#,
    )
    .bind("stale-team-runtime-session")
    .bind("planner")
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert stale running session");

    let Json(runtime) = get_team_runtime(State(state.clone()), headers, Path(team.id.clone()))
        .await
        .expect("describe reconciled team runtime");

    assert_eq!(runtime.status, crate::team::TeamRuntimeStatus::Stopped);
    assert_eq!(runtime.members.len(), 1);
    assert_eq!(runtime.members[0].member_id, "planner");
    assert_eq!(runtime.members[0].agent_status.as_deref(), Some("exited"));
    assert!(runtime.members[0].session_id.is_none());
    assert!(runtime.members[0].session_status.is_none());

    let agent_status: String = sqlx::query_scalar("SELECT status FROM agents WHERE id = ?1")
        .bind("planner")
        .fetch_one(&state.db)
        .await
        .expect("load reconciled planner status");
    assert_eq!(agent_status, "exited");
    let (session_status, ended_at): (String, Option<i64>) =
        sqlx::query_as("SELECT status, ended_at FROM agent_sessions WHERE id = ?1")
            .bind("stale-team-runtime-session")
            .fetch_one(&state.db)
            .await
            .expect("load reconciled planner session");
    assert_eq!(session_status, "exited");
    assert!(ended_at.is_some());
}

#[tokio::test]
async fn teams_api_rejects_execution_until_team_has_members() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "empty-execution-team".to_string(),
            description: Some("members added later".to_string()),
            spec: json!({
                "spec_version": 1,
                "members": [],
            }),
        }),
    )
    .await
    .expect("create empty execution team");

    let start_err = start_team(State(state.clone()), headers.clone(), Path(team.id.clone()))
        .await
        .expect_err("start should fail without members");
    let start_body = decode_json_body(start_err.into_response()).await;
    assert_eq!(
        start_body["error"],
        Value::from("team has no members configured; add at least one agent first")
    );

    let run_err = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-empty-team".to_string()),
            input: Some(json!({"task": "noop"})),
        }),
    )
    .await
    .expect_err("run creation should fail without members");
    let run_body = decode_json_body(run_err.into_response()).await;
    assert_eq!(
        run_body["error"],
        Value::from("team has no members configured; add at least one agent first")
    );

    // Seed an invalid pre-existing task directly instead of bypassing the
    // canonical creation contract through TeamManager helpers.
    let task_id = Uuid::new_v4().to_string();
    let conversation_id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp_millis();
    sqlx::query(
        r#"
        INSERT INTO team_tasks (
            id,
            team_id,
            title,
            status,
            priority,
            created_by_actor_id,
            assigned_member_id,
            context_json,
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, 'open', 'medium', ?4, NULL, ?5, ?6, ?6)
        "#,
    )
    .bind(&task_id)
    .bind(&team.id)
    .bind("Investigate")
    .bind("user")
    .bind(json!({}).to_string())
    .bind(now)
    .execute(&state.db)
    .await
    .expect("seed task row");
    sqlx::query(
        r#"
        INSERT INTO team_conversations (
            id,
            team_id,
            task_id,
            mode,
            topic,
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, 'group_chat', NULL, ?4, ?4)
        "#,
    )
    .bind(&conversation_id)
    .bind(&team.id)
    .bind(&task_id)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("seed conversation row");
    let detail = state
        .teams
        .get_task_detail(&task_id, 100)
        .await
        .expect("load seeded task detail");
    let task_detail = TeamTaskDetailResponse {
        task: detail.task,
        conversation: detail.conversation,
        latest_run: detail.latest_run,
        notes: detail.notes,
    };

    let compile_err = compile_team_task_run_preview(
        State(state),
        headers,
        Path((team.id.clone(), task_detail.task.id.clone())),
        Json(CompileTeamTaskRunPreviewRequest {
            context_id: Some("ctx-empty-team-compile".to_string()),
        }),
    )
    .await
    .expect_err("compile preview should fail without members");
    let compile_body = decode_json_body(compile_err.into_response()).await;
    assert_eq!(
        compile_body["error"],
        Value::from("team has no members configured; add at least one agent first")
    );
}

#[tokio::test]
async fn teams_api_delete_team_cascades_related_run_data() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;
    let now = Utc::now().timestamp();
    let member_agent_id = "planner";

    sqlx::query(
        r#"
        INSERT OR REPLACE INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref,
            code_mode, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9)
        "#,
    )
    .bind(member_agent_id)
    .bind("planner-agent")
    .bind("/tmp")
    .bind("/usr/bin/env")
    .bind("[]")
    .bind("use_existing")
    .bind("running")
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert member agent");

    let session_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
        VALUES (?1, ?2, 'running', ?3, NULL)
        "#,
    )
    .bind(&session_id)
    .bind(member_agent_id)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert member session");

    let member_event_db = state
        .agents
        .test_event_pool_for_agent(member_agent_id)
        .await
        .expect("open member event db");
    sqlx::query(
        r#"
        INSERT INTO agent_events (session_id, seq, ts, stream, message)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind(&session_id)
    .bind("1")
    .bind(now)
    .bind("stdout")
    .bind("event payload")
    .execute(&member_event_db)
    .await
    .expect("insert member event");

    let permission_request_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO acp_permission_requests (
            id,
            agent_id,
            session_id,
            acp_session_id,
            tool_call_id,
            options_json,
            tool_call_json,
            status,
            selected_option_id,
            created_at,
            responded_at
        )
        VALUES (?1, ?2, ?3, NULL, NULL, ?4, NULL, ?5, NULL, ?6, NULL)
        "#,
    )
    .bind(&permission_request_id)
    .bind(member_agent_id)
    .bind(&session_id)
    .bind("[]")
    .bind("pending")
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert acp permission request");

    let team = state
        .teams
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "delete-team".to_string(),
                description: Some("delete target".to_string()),
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
            },
            None,
        )
        .await
        .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-delete-team".to_string()),
            input: Some(json!({"task":"delete"})),
        }),
    )
    .await
    .expect("create run");

    let step_id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    sqlx::query(
        r#"
        INSERT INTO team_steps (
            id, run_id, step_key, member_id, remote_task_id, status, attempt, depends_on_json,
            input_json, output_json, error_text, started_at, ended_at
        )
        VALUES (?1, ?2, 'worker_step', 'planner', NULL, 'submitted', 0, '[]', NULL, NULL, NULL, NULL, NULL)
        "#,
    )
    .bind(&step_id)
    .bind(&run.id)
    .execute(&state.db)
    .await
    .expect("insert step");

    sqlx::query(
        r#"
        INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
        VALUES (?1, ?2, 'step_submitted', ?3, '{}')
        "#,
    )
    .bind(&run.id)
    .bind(&step_id)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert run event");

    sqlx::query(
        r#"
        INSERT INTO team_actor_messages (
            run_id, from_actor_id, to_actor_id, channel, transport, route_json, payload_json,
            idempotency_key, status, created_at
        )
        VALUES (?1, 'coordinator', 'worker', 'default', 'local', NULL, '{}', NULL, 'pending', ?2)
        "#,
    )
    .bind(&run.id)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert actor message");

    sqlx::query(
        r#"
        INSERT INTO team_context_artifacts (
            team_id, run_id, member_id, session_id, artifact_seq, artifact_kind,
            artifact_path, artifact_size_bytes, content_checksum, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(&team.id)
    .bind(&run.id)
    .bind("planner")
    .bind(&session_id)
    .bind(1_i64)
    .bind("memory_flush")
    .bind("/tmp/memory-flush-artifact.json")
    .bind(128_i64)
    .bind("checksum")
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert context artifact");

    sqlx::query(
        r#"
        INSERT INTO team_context_flush_checkpoint (
            team_id, run_id, member_id, session_id, last_event_id, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&team.id)
    .bind(&run.id)
    .bind("planner")
    .bind(&session_id)
    .bind(9_i64)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert context flush checkpoint");

    drop(member_event_db);

    let Json(deleted) = delete_team(State(state.clone()), headers.clone(), Path(team.id.clone()))
        .await
        .expect("delete team");
    assert_eq!(deleted.id, team.id);

    let get_err = get_team(State(state.clone()), headers.clone(), Path(team.id.clone()))
        .await
        .expect_err("team should be deleted");
    assert_eq!(get_err.into_response().status(), StatusCode::NOT_FOUND);

    let Json(listed) = list_teams(State(state.clone()), headers.clone())
        .await
        .expect("list teams after delete");
    assert!(listed.is_empty());

    let team_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM team_definitions WHERE id = ?1")
        .bind(&team.id)
        .fetch_one(&state.db)
        .await
        .expect("count teams");
    assert_eq!(team_count, 0);

    let run_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM team_runs WHERE id = ?1")
        .bind(&run.id)
        .fetch_one(&state.db)
        .await
        .expect("count runs");
    assert_eq!(run_count, 0);

    let step_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM team_steps WHERE run_id = ?1")
        .bind(&run.id)
        .fetch_one(&state.db)
        .await
        .expect("count steps");
    assert_eq!(step_count, 0);

    let event_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM team_run_events WHERE run_id = ?1")
            .bind(&run.id)
            .fetch_one(&state.db)
            .await
            .expect("count events");
    assert_eq!(event_count, 0);

    let message_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM team_actor_messages WHERE run_id = ?1")
            .bind(&run.id)
            .fetch_one(&state.db)
            .await
            .expect("count messages");
    assert_eq!(message_count, 0);

    let artifact_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM team_context_artifacts WHERE run_id = ?1")
            .bind(&run.id)
            .fetch_one(&state.db)
            .await
            .expect("count context artifacts");
    assert_eq!(artifact_count, 0);

    let checkpoint_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM team_context_flush_checkpoint WHERE run_id = ?1")
            .bind(&run.id)
            .fetch_one(&state.db)
            .await
            .expect("count context flush checkpoint");
    assert_eq!(checkpoint_count, 0);

    let session_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM agent_sessions WHERE agent_id = ?1")
            .bind(member_agent_id)
            .fetch_one(&state.db)
            .await
            .expect("count member sessions");
    assert_eq!(session_count, 0);

    wait_for_agent_event_history_cleanup(&state, member_agent_id, &session_id).await;

    let permission_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM acp_permission_requests WHERE agent_id = ?1")
            .bind(member_agent_id)
            .fetch_one(&state.db)
            .await
            .expect("count permission requests");
    assert_eq!(permission_count, 0);
}

#[tokio::test]
async fn teams_api_create_team_auto_starts_member_runtime() {
    let state = build_test_state().await;
    configure_worker_team_member_agent(&state, "reviewer").await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "auto-start-team".to_string(),
            description: Some("auto start runtime".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"reviewer","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let session_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_sessions WHERE agent_id IN ('planner', 'reviewer')",
    )
    .fetch_one(&state.db)
    .await
    .expect("count auto-started member sessions");
    assert_eq!(session_count, 2);

    let failed_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agents WHERE id IN ('planner', 'reviewer') AND status = 'failed'",
    )
    .fetch_one(&state.db)
    .await
    .expect("count failed member agents");
    assert_eq!(failed_count, 0);

    let Json(deleted) = delete_team(State(state.clone()), headers.clone(), Path(team.id.clone()))
        .await
        .expect("delete team");
    assert_eq!(deleted.id, team.id);
}

#[tokio::test]
async fn team_member_runtime_startup_supports_coordinator_and_worker_roles() {
    let state = build_test_state().await;
    configure_worker_team_member_agent(&state, "reviewer").await;

    let planner_session = state
        .agents
        .start_agent_with_actor_context(
            "planner",
            Some(AcpActorSkillContext {
                team_id: Some("team-runtime-startup".to_string()),
                current_run_id: None,
                actor_id: "planner".to_string(),
                default_channel: "default".to_string(),
                member_role: Some("coordinator".to_string()),
                member_skills: Vec::new(),
                contract_version: None,
                continuity: None,
            }),
        )
        .await
        .expect("start planner runtime");

    let reviewer_session = state
        .agents
        .start_agent_with_actor_context(
            "reviewer",
            Some(AcpActorSkillContext {
                team_id: Some("team-runtime-startup".to_string()),
                current_run_id: None,
                actor_id: "reviewer".to_string(),
                default_channel: "default".to_string(),
                member_role: Some("worker".to_string()),
                member_skills: Vec::new(),
                contract_version: None,
                continuity: None,
            }),
        )
        .await
        .expect("start reviewer runtime");

    assert!(!planner_session.is_empty());
    assert!(!reviewer_session.is_empty());

    let _ = state.agents.stop_agent("planner").await;
    let _ = state.agents.stop_agent("reviewer").await;
}

#[tokio::test]
async fn teams_api_start_and_stop_team_runtime() {
    let state = build_test_state().await;
    configure_worker_team_member_agent(&state, "reviewer").await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "team-runtime-control".to_string(),
            description: Some("start stop runtime".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"reviewer","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(stopped) = stop_team(State(state.clone()), headers.clone(), Path(team.id.clone()))
        .await
        .expect("stop team");
    assert_eq!(stopped.team_id, team.id);
    assert_eq!(stopped.status, crate::team::TeamRuntimeStatus::Stopped);
    assert!(
        state
            .agents
            .running_session_id_for_agent("planner")
            .await
            .is_none()
    );
    assert!(
        state
            .agents
            .running_session_id_for_agent("reviewer")
            .await
            .is_none()
    );

    let Json(started) = start_team(State(state.clone()), headers.clone(), Path(team.id.clone()))
        .await
        .expect("start team");
    assert_eq!(started.team_id, team.id);
    assert_eq!(started.status, crate::team::TeamRuntimeStatus::Running);
    assert_eq!(started.members.len(), 2);
    assert!(
        started
            .members
            .iter()
            .all(|member| matches!(member.action.as_str(), "started" | "reused"))
    );
    let restarted_session_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_sessions WHERE agent_id IN ('planner', 'reviewer')",
    )
    .fetch_one(&state.db)
    .await
    .expect("count restarted member sessions");
    assert!(restarted_session_count >= 4);

    let Json(stopped_again) =
        stop_team(State(state.clone()), headers.clone(), Path(team.id.clone()))
            .await
            .expect("stop team again");
    assert_eq!(
        stopped_again.status,
        crate::team::TeamRuntimeStatus::Stopped
    );
    assert!(stopped_again.members.len() <= 2);

    let Json(deleted) = delete_team(State(state.clone()), headers, Path(team.id.clone()))
        .await
        .expect("delete team");
    assert_eq!(deleted.id, team.id);
}

#[tokio::test]
async fn force_new_session_restarts_member_runtime_with_new_session_id() {
    let state = build_test_state().await;
    configure_long_lived_team_member_agent(&state, "planner").await;
    let headers = auth_headers(&state).await;
    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "team-force-new-session".to_string(),
            description: Some("force one member to use a new session".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner","role":"coordinator"}]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(started) = start_team(State(state.clone()), headers.clone(), Path(team.id.clone()))
        .await
        .expect("start team");
    let original_planner_session = started
        .members
        .iter()
        .find(|member| member.member_id == "planner")
        .expect("planner session")
        .session_id
        .clone();
    wait_for_running_agent_session(&state, "planner", &original_planner_session).await;

    let Json(runtime) = force_new_session_for_team_member(
        State(state.clone()),
        headers,
        Path((team.id.clone(), "planner".to_string())),
    )
    .await
    .expect("force new planner session");
    assert_forced_restart_runtime(&runtime, &team.id, "planner", &original_planner_session);
    state
        .agents
        .stop_agent("planner")
        .await
        .expect("stop planner after force new session test");

    let state = build_test_state().await;
    configure_long_lived_team_member_agent(&state, "reviewer").await;
    let headers = auth_headers(&state).await;
    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "team-force-new-worker-session".to_string(),
            description: Some("force worker member to use a new session".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"reviewer","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(started) = start_team(State(state.clone()), headers, Path(team.id.clone()))
        .await
        .expect("start team");
    let original_reviewer_session = started
        .members
        .iter()
        .find(|member| member.member_id == "reviewer")
        .expect("reviewer session")
        .session_id
        .clone();
    wait_for_running_agent_session(&state, "reviewer", &original_reviewer_session).await;

    let runtime = force_team_member_new_session(&state.agents, &team, "reviewer")
        .await
        .unwrap_or_else(|err| panic!("{err:#}"));
    assert_forced_restart_runtime(&runtime, &team.id, "reviewer", &original_reviewer_session);
    state
        .agents
        .stop_agent("reviewer")
        .await
        .expect("stop reviewer after force new session test");
}

#[tokio::test]
async fn teams_api_start_team_keeps_legacy_worker_use_existing_runtime_when_validation_is_allowed()
{
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;
    let now = Utc::now().timestamp();
    let repo = create_named_worker_test_repo("shiro");
    let worker_id = "shiro-reviewer";
    let legacy_workdir = insert_legacy_team_member_agent(&state, worker_id).await;
    sqlx::query("INSERT OR IGNORE INTO safe_paths (path, created_at) VALUES (?1, ?2)")
        .bind(&repo)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert shiro repo safe path");

    let team = state
        .teams
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "legacy-worker-repair".to_string(),
                description: Some("legacy worker config".to_string()),
                spec: json!({
                    "entrypoint":"planner",
                    "members":[
                        {"member_id":"planner","role":"coordinator"},
                        {
                            "member_id":worker_id,
                            "role":"worker",
                            "prompt":"Investigate issues in github.com/hawkingrei/shiro and report back."
                        }
                    ]
                }),
            },
            None,
        )
        .await
        .expect("insert legacy team");

    let Json(started) = start_team(State(state.clone()), headers, Path(team.id.clone()))
        .await
        .expect("start legacy team");
    assert_eq!(started.status, crate::team::TeamRuntimeStatus::Running);

    let reviewer = state
        .agents
        .get_agent(worker_id)
        .await
        .expect("load repaired reviewer agent");
    assert!(matches!(
        reviewer.worktree_mode,
        crate::agent::WorktreeMode::UseExisting
    ));
    assert_eq!(reviewer.workdir, legacy_workdir);
    assert!(
        reviewer
            .worktree_repo
            .as_deref()
            .map(str::trim)
            .is_none_or(str::is_empty)
    );
    assert!(
        reviewer
            .worktree_ref
            .as_deref()
            .map(str::trim)
            .is_none_or(str::is_empty)
    );
}

#[tokio::test]
async fn teams_api_start_team_returns_bad_request_for_unrecoverable_worker_runtime() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;
    let worker_id = "ghost-reviewer";
    let team = state
        .teams
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "legacy-worker-missing-repo".to_string(),
                description: Some("legacy worker missing repo".to_string()),
                spec: json!({
                    "entrypoint":"planner",
                    "members":[
                        {"member_id":"planner","role":"coordinator"},
                        {
                            "member_id":worker_id,
                            "role":"worker",
                            "prompt":"Investigate the issue and report back."
                        }
                    ]
                }),
            },
            None,
        )
        .await
        .expect("insert legacy team");

    let err = start_team(State(state), headers, Path(team.id))
        .await
        .expect_err("unrecoverable worker runtime should fail");
    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = decode_json_body(response).await;
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|message| message.contains("team member agent")),
        "unexpected error body: {body}",
    );
}

#[test]
fn team_member_actor_context_match_rejects_mismatched_team_runtime() {
    let expected = build_team_member_actor_context(
        "team-runtime-startup",
        &TeamMemberSpec {
            member_id: "planner".to_string(),
            role: "coordinator".to_string(),
            model: None,
            description: None,
            prompt: None,
        },
    )
    .expect("expected team member actor context");

    let mismatched = AcpActorSkillContext {
        team_id: Some("other-team".to_string()),
        current_run_id: None,
        actor_id: "planner".to_string(),
        default_channel: "default".to_string(),
        member_role: Some("coordinator".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    assert!(!team_member_actor_context_matches(
        Some(&mismatched),
        &expected
    ));
    assert!(team_member_actor_context_matches(
        Some(&expected),
        &expected
    ));
    assert!(!team_member_actor_context_matches(None, &expected));
}

#[tokio::test]
async fn teams_api_strips_member_skill_configuration_from_team_spec() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(created) = create_team(
        State(state),
        headers,
        Json(CreateTeamRequest {
            name: "required-role-skills-team".to_string(),
            description: Some("role skill enforcement".to_string()),
            spec: json!({
                "entrypoint":"coordinator-agent",
                "coordinator_member_id":"coordinator-agent",
                "members":[
                    {
                        "member_id":"coordinator-agent",
                        "role":"coordinator",
                        "skills":["planning"]
                    },
                    {
                        "member_id":"worker-agent",
                        "role":"worker",
                        "skills":["coding"]
                    }
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let members = created
        .spec
        .get("members")
        .and_then(Value::as_array)
        .cloned()
        .expect("members array");

    for member in members {
        assert!(
            member.get("skills").is_none(),
            "normalized team spec should not persist member skill config: {member}"
        );
    }
}

#[tokio::test]
async fn teams_api_ignores_legacy_member_skills_payload_shapes() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(created) = create_team(
        State(state),
        headers,
        Json(CreateTeamRequest {
            name: "ignored-legacy-member-skills".to_string(),
            description: Some("legacy skills input should be ignored".to_string()),
            spec: json!({
                "entrypoint":"coordinator-agent",
                "coordinator_member_id":"coordinator-agent",
                "members":[
                    {
                        "member_id":"coordinator-agent",
                        "role":"coordinator",
                        "skills":"planning"
                    },
                    {
                        "member_id":"worker-agent",
                        "role":"worker",
                        "skills":{"custom":"coding"}
                    }
                ]
            }),
        }),
    )
    .await
    .expect("create team with legacy skills payloads");

    let members = created
        .spec
        .get("members")
        .and_then(Value::as_array)
        .cloned()
        .expect("members array");
    for member in members {
        assert!(
            member.get("skills").is_none(),
            "legacy member skill payloads should be dropped from normalized spec: {member}"
        );
    }
}

#[tokio::test]
async fn teams_api_read_paths_strip_legacy_member_skill_configuration() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let team = state
        .teams
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "legacy-skill-read-team".to_string(),
                description: Some("legacy team spec should be sanitized on read".to_string()),
                spec: json!({
                    "entrypoint":"coordinator-agent",
                    "coordinator_member_id":"coordinator-agent",
                    "members":[
                        {
                            "member_id":"coordinator-agent",
                            "role":"coordinator",
                            "skills":["planning","review"]
                        },
                        {
                            "member_id":"worker-agent",
                            "role":"worker",
                            "skills":["coding"]
                        }
                    ]
                }),
            },
            None,
        )
        .await
        .expect("create legacy skill team");

    let Json(listed_teams) = list_teams(State(state.clone()), headers.clone())
        .await
        .expect("list teams");
    let listed = listed_teams
        .into_iter()
        .find(|item| item.id == team.id)
        .expect("listed legacy team");
    for member in listed
        .spec
        .get("members")
        .and_then(Value::as_array)
        .cloned()
        .expect("listed members")
    {
        assert!(
            member.get("skills").is_none(),
            "list teams should redact legacy member skills: {member}"
        );
    }

    let Json(fetched) = get_team(State(state.clone()), headers.clone(), Path(team.id.clone()))
        .await
        .expect("get team");
    for member in fetched
        .spec
        .get("members")
        .and_then(Value::as_array)
        .cloned()
        .expect("fetched members")
    {
        assert!(
            member.get("skills").is_none(),
            "get team should redact legacy member skills: {member}"
        );
    }

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-legacy-skill-read".to_string()),
            input: Some(json!({"goal":"verify read sanitization"})),
        }),
    )
    .await
    .expect("create run for sanitized snapshot");

    let Json(snapshot) = get_team_run_snapshot(
        State(state),
        headers,
        Path(run.id),
        Query(TeamRunSnapshotQuery {
            event_limit: Some(50),
            message_limit: Some(50),
        }),
    )
    .await
    .expect("get team run snapshot");
    for member in snapshot
        .team
        .spec
        .get("members")
        .and_then(Value::as_array)
        .cloned()
        .expect("snapshot members")
    {
        assert!(
            member.get("skills").is_none(),
            "snapshot team payload should redact legacy member skills: {member}"
        );
    }
}

#[tokio::test]
async fn teams_api_injects_role_workflow_prompt_policy_defaults() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(created) = create_team(
        State(state),
        headers,
        Json(CreateTeamRequest {
            name: "role-workflow-prompt-team".to_string(),
            description: Some("role workflow prompt defaults".to_string()),
            spec: json!({
                "entrypoint":"coordinator-agent",
                "coordinator_member_id":"coordinator-agent",
                "members":[
                    {"member_id":"coordinator-agent","role":"coordinator"},
                    {"member_id":"worker-agent","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let members = created
        .spec
        .get("members")
        .and_then(Value::as_array)
        .cloned()
        .expect("members array");
    let coordinator_prompt = members
        .iter()
        .find(|member| member.get("member_id").and_then(Value::as_str) == Some("coordinator-agent"))
        .and_then(|member| member.get("prompt"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(coordinator_prompt.contains("Do not implement feature code directly."));
    assert!(coordinator_prompt.contains("perform targeted technical research"));
    assert!(coordinator_prompt.contains("Start from an empty workspace."));
    assert!(coordinator_prompt.contains("summary entrypoint for one topic"));
    assert!(coordinator_prompt.contains("full-context container for that topic"));
    assert!(coordinator_prompt.contains("agenthub actor team-thread-open"));

    let worker_prompt = members
        .iter()
        .find(|member| member.get("member_id").and_then(Value::as_str) == Some("worker-agent"))
        .and_then(|member| member.get("prompt"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(worker_prompt.contains("Work in your own git worktree only."));
    assert!(worker_prompt.contains("Create a random branch at start"));
    assert!(worker_prompt.contains("open the thread before assuming"));
    assert!(worker_prompt.contains("agenthub actor team-thread-reply"));
}

#[tokio::test]
async fn teams_api_delete_team_returns_not_found_when_missing() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let err = delete_team(State(state), headers, Path("missing-team".to_string()))
        .await
        .expect_err("missing team delete should fail");
    assert_eq!(err.into_response().status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn teams_api_generates_default_steps_for_multi_member_team() {
    let mut spec = json!({
        "entrypoint":"coordinator-agent",
        "members":[
            {"member_id":"coordinator-agent","role":"coordinator"},
            {"member_id":"worker-agent-a","role":"worker"},
            {"member_id":"worker-agent-b","role":"worker"}
        ]
    });
    normalize_team_spec(&mut spec).expect("normalize team spec with generated defaults");
    validate_team_spec(&spec).expect("validate generated default steps");

    assert_eq!(spec["entrypoint"], Value::from("coordinator_plan"));
    let steps = spec["steps"]
        .as_array()
        .expect("generated steps array");
    assert_eq!(steps.len(), 4);
    assert_eq!(steps[0]["step_key"], Value::from("coordinator_plan"));
    assert_eq!(steps[0]["member_id"], Value::from("coordinator-agent"));
    let worker_step_keys = steps
        .iter()
        .filter_map(|step| {
            let member_id = step.get("member_id")?.as_str()?;
            if member_id.starts_with("worker-agent") {
                step.get("step_key")?.as_str().map(str::to_string)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(worker_step_keys.len(), 2);
    let synth_step = steps
        .iter()
        .find(|step| step.get("step_key").and_then(Value::as_str) == Some("coordinator_synthesize"))
        .expect("coordinator_synthesize step");
    let synth_depends = synth_step["depends_on"]
        .as_array()
        .expect("synthesize depends_on");
    assert_eq!(synth_depends.len(), 2);
    for worker_step_key in worker_step_keys {
        assert!(
            synth_depends
                .iter()
                .any(|dep| dep.as_str() == Some(worker_step_key.as_str()))
        );
    }
}

#[tokio::test]
async fn teams_api_rejects_invalid_spec() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;
    let invalid_specs = vec![
        json!("invalid"),
        json!({"entrypoint":"planner"}),
        json!({"entrypoint":"","members":[{"member_id":"planner"}]}),
        json!({"entrypoint":"planner","members":[{"member_id":"planner"},{"member_id":"planner"}]}),
        json!({"entrypoint":"missing","members":[{"member_id":"planner"}]}),
        json!({"entrypoint":"step-a","members":[{"member_id":"planner"}]}),
        json!({"entrypoint":"step-a","members":[{"member_id":"planner"}],"steps":[]}),
        json!({"entrypoint":"step-a","members":[{"member_id":"planner"}],"steps":[{"step_key":"step-a","member_id":"missing"}]}),
        json!({"entrypoint":"step-a","members":[{"member_id":"planner"}],"steps":[{"step_key":"step-a","member_id":"planner","depends_on":["step-b"]}]}),
        json!({"entrypoint":"step-a","members":[{"member_id":"planner"}],"steps":[{"step_key":"step-a","member_id":"planner","depends_on":["step-a"]}]}),
        json!({"entrypoint":"step-a","members":[{"member_id":"planner"}],"steps":[{"step_key":"step-a","member_id":"planner"},{"step_key":"step-b","member_id":"planner","depends_on":["step-a","step-a"]}]}),
        json!({"entrypoint":"step-a","members":[{"member_id":"planner"}],"steps":[{"step_key":"step-a","member_id":"planner","depends_on":["step-b"]},{"step_key":"step-b","member_id":"planner","depends_on":["step-a"]}]}),
        json!({"spec_version":"1","entrypoint":"planner","members":[{"member_id":"planner"}]}),
        json!({"spec_version":2,"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        json!({"entrypoint":"planner","coordinator_member_id":"missing","members":[{"member_id":"planner"}]}),
        json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"captain"}]}),
        json!({"entrypoint":"planner","members":[{"member_id":"planner","skills":["a","a"]}]}),
        json!({"entrypoint":"planner","coordinator_member_id":"coordinator","members":[{"member_id":"planner"},{"member_id":"coordinator"}]}),
    ];
    for (index, spec) in invalid_specs.into_iter().enumerate() {
        let err = create_team(
            State(state.clone()),
            headers.clone(),
            Json(CreateTeamRequest {
                name: format!("invalid-team-{index}"),
                description: None,
                spec,
            }),
        )
        .await
        .expect_err("invalid team spec should fail");
        assert_eq!(err.into_response().status(), StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn teams_api_rejects_run_for_unsupported_stored_spec_version() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let team_id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    sqlx::query(
        r#"
        INSERT INTO team_definitions (id, name, description, spec_json, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&team_id)
    .bind(format!("legacy-team-{}", Uuid::new_v4()))
    .bind("legacy unsupported spec")
    .bind(
        json!({
            "spec_version": 2,
            "entrypoint": "planner",
            "members": [{"member_id":"planner"}]
        })
        .to_string(),
    )
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert legacy team");

    let err = create_team_run(
        State(state),
        headers,
        Path(team_id),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-legacy".to_string()),
            input: Some(json!({"prompt":"run legacy"})),
        }),
    )
    .await
    .expect_err("unsupported stored spec should fail");
    assert_eq!(err.into_response().status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn teams_api_rejects_spec_with_too_many_steps() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;
    let mut steps = Vec::new();
    for index in 0..2049 {
        let depends_on = if index == 0 {
            Vec::new()
        } else {
            vec![format!("step-{}", index - 1)]
        };
        steps.push(json!({
            "step_key": format!("step-{index}"),
            "member_id": "planner",
            "depends_on": depends_on,
        }));
    }

    let err = create_team(
        State(state),
        headers,
        Json(CreateTeamRequest {
            name: "too-many-steps".to_string(),
            description: None,
            spec: json!({
                "entrypoint": "step-0",
                "members": [{"member_id":"planner","role":"coordinator"}],
                "steps": steps,
            }),
        }),
    )
    .await
    .expect_err("spec with too many steps should fail");
    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = decode_json_body(response).await;
    assert_eq!(
        body["error"],
        Value::from("spec.steps must not exceed 2048 entries")
    );
}

#[tokio::test]
async fn teams_api_internal_errors_are_sanitized() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "internal-error-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    sqlx::query("DROP TABLE team_runs")
        .execute(&state.db)
        .await
        .expect("drop team_runs");

    let err = create_team_run(
        State(state),
        headers,
        Path(team.id),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-internal-error".to_string()),
            input: Some(json!({"prompt":"should fail"})),
        }),
    )
    .await
    .expect_err("internal error expected");
    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = decode_json_body(response).await;
    assert_eq!(body["error"], Value::from("internal server error"));
}

#[tokio::test]
async fn team_runs_api_supports_lifecycle_and_event_pagination() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "run-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"executor","members":[{"member_id":"executor","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-a2a".to_string()),
            input: Some(json!({"prompt":"review plan"})),
        }),
    )
    .await
    .expect("create run");
    assert_eq!(run.status, crate::team::TeamRunStatus::Submitted);

    let Json(found_run) = get_team_run(State(state.clone()), headers.clone(), Path(run.id.clone()))
        .await
        .expect("get run");
    assert_eq!(found_run.id, run.id);

    let Json(canceled) =
        cancel_team_run(State(state.clone()), headers.clone(), Path(run.id.clone()))
            .await
            .expect("cancel run");
    assert_eq!(canceled.status, crate::team::TeamRunStatus::Canceled);
    assert!(canceled.ended_at.is_some());

    let Json(events) = list_team_run_events(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunEventsQuery {
            limit: Some(100),
            before_id: None,
        }),
    )
    .await
    .expect("list events");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, "run_submitted");
    assert_eq!(events[1].event_type, "run_canceled");
    assert!(events[0].event_id < events[1].event_id);

    let Json(first_page) = list_team_run_events(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunEventsQuery {
            limit: Some(1),
            before_id: Some(events[1].event_id),
        }),
    )
    .await
    .expect("page events");
    assert_eq!(first_page.len(), 1);
    assert_eq!(first_page[0].event_type, "run_submitted");

    let missing_team_run_err = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path("missing-team".to_string()),
        Json(CreateTeamRunRequest {
            context_id: None,
            input: Some(json!({})),
        }),
    )
    .await
    .expect_err("missing team");
    assert_eq!(
        missing_team_run_err.into_response().status(),
        StatusCode::NOT_FOUND
    );

    let missing_run_err = get_team_run(
        State(state.clone()),
        headers.clone(),
        Path("missing-run".to_string()),
    )
    .await
    .expect_err("missing run");
    assert_eq!(
        missing_run_err.into_response().status(),
        StatusCode::NOT_FOUND
    );

    let missing_events_err = list_team_run_events(
        State(state),
        headers,
        Path("missing-run".to_string()),
        Query(ListTeamRunEventsQuery {
            limit: None,
            before_id: None,
        }),
    )
    .await
    .expect_err("missing run events");
    assert_eq!(
        missing_events_err.into_response().status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn team_runs_api_supports_manual_context_flush() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;
    let now = Utc::now().timestamp();

    sqlx::query(
        r#"
        INSERT OR REPLACE INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref,
            code_mode, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9)
        "#,
    )
    .bind("executor")
    .bind("executor-agent")
    .bind("/tmp")
    .bind("/usr/bin/env")
    .bind("[]")
    .bind("use_existing")
    .bind("running")
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert executor agent");

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "flush-api-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"executor","members":[{"member_id":"executor","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-flush-api".to_string()),
            input: Some(json!({"prompt":"collect"})),
        }),
    )
    .await
    .expect("create run");

    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
        VALUES (?1, ?2, 'running', ?3, NULL)
        "#,
    )
    .bind("session-flush-api")
    .bind("executor")
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert agent session for flush");

    let member_event_db = state
        .agents
        .test_event_pool_for_agent("executor")
        .await
        .expect("open executor event db");
    sqlx::query(
        r#"
        INSERT INTO agent_events (session_id, seq, ts, stream, message)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind("session-flush-api")
    .bind("1")
    .bind(now)
    .bind("acp")
    .bind(r#"{"type":"agent_message","content":"flush me","token":"secret"}"#)
    .execute(&member_event_db)
    .await
    .expect("insert agent event");

    let Json(flush_result) = flush_team_run_context(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(FlushTeamRunContextRequest {
            member_id: "executor".to_string(),
            session_id: Some("session-flush-api".to_string()),
            trigger: Some("manual".to_string()),
            max_events: None,
        }),
    )
    .await
    .expect("flush run context");

    assert_eq!(flush_result.status, "persisted");
    assert_eq!(flush_result.member_id, "executor");
    assert_eq!(
        flush_result.session_id.as_deref(),
        Some("session-flush-api")
    );
    assert!(flush_result.artifact_pointer.is_some());
    assert_eq!(flush_result.flushed_events, 1);

    let Json(events) = list_team_run_events(
        State(state),
        headers,
        Path(run.id),
        Query(ListTeamRunEventsQuery {
            limit: Some(100),
            before_id: None,
        }),
    )
    .await
    .expect("list run events");
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "memory_flush_started")
    );
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "memory_flush_persisted")
    );
}

#[tokio::test]
async fn team_runs_api_rejects_invalid_context_flush_trigger() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "flush-trigger-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"executor","members":[{"member_id":"executor","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-flush-trigger".to_string()),
            input: Some(json!({"prompt":"collect"})),
        }),
    )
    .await
    .expect("create run");

    let err = flush_team_run_context(
        State(state),
        headers,
        Path(run.id),
        Json(FlushTeamRunContextRequest {
            member_id: "executor".to_string(),
            session_id: Some("session-flush-api".to_string()),
            trigger: Some("invalid-trigger".to_string()),
            max_events: None,
        }),
    )
    .await
    .expect_err("invalid trigger should fail");
    assert_eq!(err.into_response().status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn team_runs_api_lists_team_runs_with_status_filter_and_cursor() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "runs-list-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let Json(other_team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "runs-list-other-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create other team");

    let Json(first_run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-runs-list-1".to_string()),
            input: Some(json!({"seq":1})),
        }),
    )
    .await
    .expect("create first run");

    let Json(second_run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-runs-list-2".to_string()),
            input: Some(json!({"seq":2})),
        }),
    )
    .await
    .expect("create second run");

    let Json(other_team_run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(other_team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-runs-list-other".to_string()),
            input: Some(json!({"seq":3})),
        }),
    )
    .await
    .expect("create other team run");

    sqlx::query("UPDATE team_runs SET created_at = ?1 WHERE id = ?2")
        .bind(100_i64)
        .bind(&first_run.id)
        .execute(&state.db)
        .await
        .expect("set first run created_at");
    sqlx::query("UPDATE team_runs SET created_at = ?1 WHERE id = ?2")
        .bind(200_i64)
        .bind(&second_run.id)
        .execute(&state.db)
        .await
        .expect("set second run created_at");
    sqlx::query("UPDATE team_runs SET created_at = ?1 WHERE id = ?2")
        .bind(300_i64)
        .bind(&other_team_run.id)
        .execute(&state.db)
        .await
        .expect("set other team run created_at");

    let Json(runs) = list_team_runs(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Query(ListTeamRunsQuery {
            limit: Some(100),
            status: None,
            before_created_at: None,
        }),
    )
    .await
    .expect("list team runs");
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].id, second_run.id);
    assert_eq!(runs[1].id, first_run.id);

    let _ = cancel_team_run(
        State(state.clone()),
        headers.clone(),
        Path(first_run.id.clone()),
    )
    .await
    .expect("cancel first run");

    let Json(canceled_runs) = list_team_runs(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Query(ListTeamRunsQuery {
            limit: Some(100),
            status: Some("canceled".to_string()),
            before_created_at: None,
        }),
    )
    .await
    .expect("list canceled team runs");
    assert_eq!(canceled_runs.len(), 1);
    assert_eq!(canceled_runs[0].id, first_run.id);

    let Json(cursor_page) = list_team_runs(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Query(ListTeamRunsQuery {
            limit: Some(100),
            status: None,
            before_created_at: Some(200),
        }),
    )
    .await
    .expect("list team runs with cursor");
    assert_eq!(cursor_page.len(), 1);
    assert_eq!(cursor_page[0].id, first_run.id);

    let invalid_status_err = list_team_runs(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Query(ListTeamRunsQuery {
            limit: Some(100),
            status: Some("invalid".to_string()),
            before_created_at: None,
        }),
    )
    .await
    .expect_err("invalid status should fail");
    assert_eq!(
        invalid_status_err.into_response().status(),
        StatusCode::BAD_REQUEST
    );

    let missing_team_err = list_team_runs(
        State(state.clone()),
        headers.clone(),
        Path("missing-team".to_string()),
        Query(ListTeamRunsQuery {
            limit: Some(100),
            status: None,
            before_created_at: None,
        }),
    )
    .await
    .expect_err("missing team should fail");
    assert_eq!(
        missing_team_err.into_response().status(),
        StatusCode::NOT_FOUND
    );

    let Json(deleted_team) =
        delete_team(State(state.clone()), headers.clone(), Path(team.id.clone()))
            .await
            .expect("delete runs-list team");
    assert_eq!(deleted_team.id, team.id);

    let Json(deleted_other_team) =
        delete_team(State(state.clone()), headers, Path(other_team.id.clone()))
            .await
            .expect("delete runs-list other team");
    assert_eq!(deleted_other_team.id, other_team.id);
}

#[tokio::test]
async fn team_runs_api_supports_resume_and_restart_strategy() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "resume-restart-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-resume-restart".to_string()),
            input: Some(json!({"prompt":"recover"})),
        }),
    )
    .await
    .expect("create run");

    let Json(resumed_active) =
        resume_team_run(State(state.clone()), headers.clone(), Path(run.id.clone()))
            .await
            .expect("resume active run");
    assert_eq!(resumed_active.id, run.id);
    assert_eq!(resumed_active.status, crate::team::TeamRunStatus::Submitted);

    let Json(canceled) = cancel_team_run(State(state.clone()), headers.clone(), Path(run.id))
        .await
        .expect("cancel run");
    assert_eq!(canceled.status, crate::team::TeamRunStatus::Canceled);

    let Json(resumed_from_canceled) = resume_team_run(
        State(state.clone()),
        headers.clone(),
        Path(canceled.id.clone()),
    )
    .await
    .expect("resume canceled run");
    assert_ne!(resumed_from_canceled.id, canceled.id);
    assert_eq!(resumed_from_canceled.team_id, canceled.team_id);
    assert_eq!(resumed_from_canceled.context_id, canceled.context_id);
    assert_eq!(resumed_from_canceled.input, canceled.input);
    assert_eq!(
        resumed_from_canceled.status,
        crate::team::TeamRunStatus::Submitted
    );

    let Json(restarted) = restart_team_run(
        State(state.clone()),
        headers.clone(),
        Path(canceled.id.clone()),
    )
    .await
    .expect("restart run");
    assert_ne!(restarted.id, canceled.id);
    assert_eq!(restarted.team_id, canceled.team_id);
    assert_eq!(restarted.context_id, canceled.context_id);
    assert_eq!(restarted.input, canceled.input);
    assert_eq!(restarted.status, crate::team::TeamRunStatus::Submitted);

    let completed_run = state
        .teams
        .create_run(
            &team.id,
            Some("ctx-resume-completed"),
            json!({"prompt":"done"}),
        )
        .await
        .expect("create completed run");
    let completed_step = state
        .teams
        .submit_step(
            &completed_run.id,
            "done",
            "planner",
            Vec::new(),
            Some(json!({"goal":"complete"})),
        )
        .await
        .expect("submit completed step");
    let _ = state
        .teams
        .start_step(&completed_step.id, Some("session-completed"))
        .await
        .expect("start completed step");
    let _ = state
        .teams
        .complete_step(&completed_step.id, Some(json!({"ok":true})))
        .await
        .expect("complete step");

    let completed_resume_err =
        resume_team_run(State(state), headers, Path(completed_run.id.clone()))
            .await
            .expect_err("completed run should reject resume");
    assert_eq!(
        completed_resume_err.into_response().status(),
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn team_runs_api_enforces_team_owner_access() {
    let state = build_test_state().await;
    let owner_headers = auth_headers(&state).await;
    let outsider_headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        owner_headers.clone(),
        Json(CreateTeamRequest {
            name: "run-owner-enforcement-team".to_string(),
            description: Some("owner enforcement for run endpoints".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner","role":"coordinator"}]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        owner_headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-owner-run".to_string()),
            input: Some(json!({"prompt":"owner run"})),
        }),
    )
    .await
    .expect("create run");

    let create_err = create_team_run(
        State(state.clone()),
        outsider_headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-outsider".to_string()),
            input: Some(json!({"prompt":"outsider run"})),
        }),
    )
    .await
    .expect_err("outsider should not create run");
    assert_eq!(create_err.into_response().status(), StatusCode::NOT_FOUND);

    let list_err = list_team_runs(
        State(state.clone()),
        outsider_headers.clone(),
        Path(team.id.clone()),
        Query(ListTeamRunsQuery {
            limit: Some(20),
            status: None,
            before_created_at: None,
        }),
    )
    .await
    .expect_err("outsider should not list team runs");
    assert_eq!(list_err.into_response().status(), StatusCode::NOT_FOUND);

    let get_err = get_team_run(
        State(state.clone()),
        outsider_headers.clone(),
        Path(run.id.clone()),
    )
    .await
    .expect_err("outsider should not read run");
    assert_eq!(get_err.into_response().status(), StatusCode::NOT_FOUND);

    let cancel_err = cancel_team_run(
        State(state.clone()),
        outsider_headers.clone(),
        Path(run.id.clone()),
    )
    .await
    .expect_err("outsider should not cancel run");
    assert_eq!(cancel_err.into_response().status(), StatusCode::NOT_FOUND);

    let events_err = list_team_run_events(
        State(state.clone()),
        outsider_headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunEventsQuery {
            limit: Some(20),
            before_id: None,
        }),
    )
    .await
    .expect_err("outsider should not list run events");
    assert_eq!(events_err.into_response().status(), StatusCode::NOT_FOUND);

    let flush_err = flush_team_run_context(
        State(state.clone()),
        outsider_headers.clone(),
        Path(run.id.clone()),
        Json(FlushTeamRunContextRequest {
            member_id: "planner".to_string(),
            session_id: Some("session-1".to_string()),
            trigger: Some("manual".to_string()),
            max_events: None,
        }),
    )
    .await
    .expect_err("outsider should not flush run context");
    assert_eq!(flush_err.into_response().status(), StatusCode::NOT_FOUND);

    let steps_err = list_team_run_steps(
        State(state.clone()),
        outsider_headers.clone(),
        Path(run.id.clone()),
    )
    .await
    .expect_err("outsider should not list run steps");
    assert_eq!(steps_err.into_response().status(), StatusCode::NOT_FOUND);

    let send_err = send_team_run_message(
        State(state.clone()),
        outsider_headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "planner".to_string(),
            to_peer_id: None,
            channel: None,
            transport: Some("local".to_string()),
            route: None,
            payload: json!({"text":"unauthorized"}),
            idempotency_key: Some("owner-access-denied".to_string()),
        }),
    )
    .await
    .expect_err("outsider should not send run message");
    assert_eq!(send_err.into_response().status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn team_runs_api_paginates_high_volume_without_duplicates_and_honors_status_filter() {
    const TOTAL_RUNS: usize = 120;
    const PAGE_SIZE: i64 = 17;

    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "runs-list-high-volume-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let mut expected_runs = Vec::with_capacity(TOTAL_RUNS);
    let mut canceled_expected = Vec::new();
    for index in 0..TOTAL_RUNS {
        let Json(run) = create_team_run(
            State(state.clone()),
            headers.clone(),
            Path(team.id.clone()),
            Json(CreateTeamRunRequest {
                context_id: Some(format!("ctx-runs-high-volume-{index}")),
                input: Some(json!({"seq":index})),
            }),
        )
        .await
        .expect("create run");

        let created_at = 10_000_i64 + index as i64;
        sqlx::query("UPDATE team_runs SET created_at = ?1 WHERE id = ?2")
            .bind(created_at)
            .bind(&run.id)
            .execute(&state.db)
            .await
            .expect("set run created_at");

        if index % 4 == 0 {
            let _ = cancel_team_run(State(state.clone()), headers.clone(), Path(run.id.clone()))
                .await
                .expect("cancel run");
            canceled_expected.push((created_at, run.id.clone()));
        }

        expected_runs.push((created_at, run.id));
    }

    expected_runs.sort_by(|left, right| right.cmp(left));
    let expected_ids = expected_runs
        .iter()
        .map(|(_, run_id)| run_id.clone())
        .collect::<Vec<_>>();

    let mut collected_ids = Vec::with_capacity(TOTAL_RUNS);
    let mut cursor = None;
    loop {
        let Json(page) = list_team_runs(
            State(state.clone()),
            headers.clone(),
            Path(team.id.clone()),
            Query(ListTeamRunsQuery {
                limit: Some(PAGE_SIZE),
                status: None,
                before_created_at: cursor,
            }),
        )
        .await
        .expect("list high-volume runs");

        if page.is_empty() {
            break;
        }

        assert!((page.len() as i64) <= PAGE_SIZE);
        if let Some(before_created_at) = cursor {
            assert!(
                page.iter().all(|run| run.created_at < before_created_at),
                "cursor filter should only return older runs"
            );
        }

        for [current, next] in page.array_windows::<2>() {
            assert!(
                current.created_at > next.created_at
                    || (current.created_at == next.created_at && current.id > next.id),
                "runs should be sorted by created_at DESC, id DESC"
            );
        }

        cursor = page.last().map(|run| run.created_at);
        collected_ids.extend(page.into_iter().map(|run| run.id));
    }

    assert_eq!(collected_ids.len(), TOTAL_RUNS);
    assert_eq!(
        collected_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        TOTAL_RUNS
    );
    assert_eq!(collected_ids, expected_ids);

    canceled_expected.sort_by(|left, right| right.cmp(left));
    let expected_canceled_ids = canceled_expected
        .iter()
        .map(|(_, run_id)| run_id.clone())
        .collect::<Vec<_>>();

    let mut canceled_ids = Vec::with_capacity(expected_canceled_ids.len());
    let mut canceled_cursor = None;
    loop {
        let Json(page) = list_team_runs(
            State(state.clone()),
            headers.clone(),
            Path(team.id.clone()),
            Query(ListTeamRunsQuery {
                limit: Some(9),
                status: Some("canceled".to_string()),
                before_created_at: canceled_cursor,
            }),
        )
        .await
        .expect("list high-volume canceled runs");

        if page.is_empty() {
            break;
        }
        assert!(
            page.iter()
                .all(|run| run.status == crate::team::TeamRunStatus::Canceled)
        );
        canceled_cursor = page.last().map(|run| run.created_at);
        canceled_ids.extend(page.into_iter().map(|run| run.id));
    }

    assert_eq!(canceled_ids, expected_canceled_ids);
}

#[tokio::test]
async fn team_run_steps_api_supports_scheduler_lifecycle_bridge() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "scheduler-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-scheduler".to_string()),
            input: Some(json!({"prompt":"run scheduler bridge"})),
        }),
    )
    .await
    .expect("create run");

    let Json(initial_steps) =
        list_team_run_steps(State(state.clone()), headers.clone(), Path(run.id.clone()))
            .await
            .expect("list initial steps");
    assert!(initial_steps.is_empty());

    let Json(step) = submit_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SubmitTeamRunStepRequest {
            step_key: "plan-step".to_string(),
            member_id: "planner".to_string(),
            depends_on: Some(vec![]),
            input: Some(json!({"goal":"plan"})),
        }),
    )
    .await
    .expect("submit step");
    assert_eq!(step.status, crate::team::TeamStepStatus::Submitted);
    assert_eq!(step.run_id, run.id);

    let duplicate_submit_err = submit_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SubmitTeamRunStepRequest {
            step_key: "plan-step".to_string(),
            member_id: "planner".to_string(),
            depends_on: None,
            input: None,
        }),
    )
    .await
    .expect_err("duplicate step key should conflict");
    let duplicate_submit_response = duplicate_submit_err.into_response();
    assert_eq!(duplicate_submit_response.status(), StatusCode::CONFLICT);
    let duplicate_submit_body = decode_json_body(duplicate_submit_response).await;
    assert_eq!(
        duplicate_submit_body["error"],
        Value::from("step already exists for run")
    );

    let Json(step_working) = start_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), step.id.clone())),
        Json(StartTeamRunStepRequest {
            runtime_handle_id: Some("remote-task-bridge".to_string()),
        }),
    )
    .await
    .expect("start step");
    assert_eq!(step_working.status, crate::team::TeamStepStatus::Working);
    assert_eq!(
        step_working.runtime_handle_id.as_deref(),
        Some("remote-task-bridge")
    );

    let Json(step_completed) = complete_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), step.id.clone())),
        Json(CompleteTeamRunStepRequest {
            output: Some(json!({"result":"ok"})),
        }),
    )
    .await
    .expect("complete step");
    assert_eq!(
        step_completed.status,
        crate::team::TeamStepStatus::Completed
    );
    assert_eq!(step_completed.output, Some(json!({"result":"ok"})));

    let Json(run_after_complete) =
        get_team_run(State(state.clone()), headers.clone(), Path(run.id.clone()))
            .await
            .expect("get run after complete");
    assert_eq!(
        run_after_complete.status,
        crate::team::TeamRunStatus::Completed
    );

    let Json(steps_after_complete) =
        list_team_run_steps(State(state.clone()), headers.clone(), Path(run.id.clone()))
            .await
            .expect("list steps after complete");
    assert_eq!(steps_after_complete.len(), 1);
    assert_eq!(
        steps_after_complete[0].status,
        crate::team::TeamStepStatus::Completed
    );

    let Json(events) = list_team_run_events(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunEventsQuery {
            limit: Some(100),
            before_id: None,
        }),
    )
    .await
    .expect("list events");
    let event_types: Vec<&str> = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect();
    assert_eq!(
        event_types,
        vec![
            "run_submitted",
            "step_submitted",
            "run_working",
            "step_working",
            "step_completed",
            "continuity_state_updated",
            "run_completed"
        ]
    );

    let missing_step_err = start_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), "missing-step".to_string())),
        Json(StartTeamRunStepRequest {
            runtime_handle_id: None,
        }),
    )
    .await
    .expect_err("missing step should fail");
    assert_eq!(
        missing_step_err.into_response().status(),
        StatusCode::NOT_FOUND
    );

    let Json(run_2) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-scheduler-2".to_string()),
            input: Some(json!({"prompt":"run fail path"})),
        }),
    )
    .await
    .expect("create second run");

    let wrong_run_err = complete_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path((run_2.id.clone(), step.id.clone())),
        Json(CompleteTeamRunStepRequest { output: None }),
    )
    .await
    .expect_err("step should not be visible under another run");
    assert_eq!(
        wrong_run_err.into_response().status(),
        StatusCode::NOT_FOUND
    );

    let Json(step_2) = submit_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path(run_2.id.clone()),
        Json(SubmitTeamRunStepRequest {
            step_key: "fail-step".to_string(),
            member_id: "planner".to_string(),
            depends_on: None,
            input: None,
        }),
    )
    .await
    .expect("submit fail step");
    let _ = start_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path((run_2.id.clone(), step_2.id.clone())),
        Json(StartTeamRunStepRequest {
            runtime_handle_id: Some("remote-task-fail".to_string()),
        }),
    )
    .await
    .expect("start fail step");

    let empty_error_err = fail_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path((run_2.id.clone(), step_2.id.clone())),
        Json(FailTeamRunStepRequest {
            error_text: "   ".to_string(),
        }),
    )
    .await
    .expect_err("empty error text should be rejected");
    assert_eq!(
        empty_error_err.into_response().status(),
        StatusCode::BAD_REQUEST
    );

    let Json(step_failed) = fail_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path((run_2.id.clone(), step_2.id.clone())),
        Json(FailTeamRunStepRequest {
            error_text: "worker failed".to_string(),
        }),
    )
    .await
    .expect("fail step");
    assert_eq!(step_failed.status, crate::team::TeamStepStatus::Failed);

    let Json(run_2_after_fail) = get_team_run(State(state), headers, Path(run_2.id.clone()))
        .await
        .expect("get second run after fail");
    assert_eq!(run_2_after_fail.status, crate::team::TeamRunStatus::Failed);
}

#[tokio::test]
async fn team_run_steps_api_supports_input_required_and_resume() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "input-required-api-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-input-api".to_string()),
            input: Some(json!({"prompt":"need manual input"})),
        }),
    )
    .await
    .expect("create run");

    let Json(step) = submit_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SubmitTeamRunStepRequest {
            step_key: "input-step".to_string(),
            member_id: "planner".to_string(),
            depends_on: None,
            input: Some(json!({"goal":"request approval"})),
        }),
    )
    .await
    .expect("submit step");
    let _ = start_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), step.id.clone())),
        Json(StartTeamRunStepRequest {
            runtime_handle_id: Some("remote-task-input-required".to_string()),
        }),
    )
    .await
    .expect("start step");

    let Json(input_required_step) = set_team_run_step_input_required(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), step.id.clone())),
        Json(SetTeamRunStepInputRequiredRequest {
            reason: Some("approval required".to_string()),
            input: Some(json!({"question":"approve?"})),
        }),
    )
    .await
    .expect("set input required");
    assert_eq!(
        input_required_step.status,
        crate::team::TeamStepStatus::InputRequired
    );
    assert_eq!(
        input_required_step.error_text.as_deref(),
        Some("approval required")
    );
    assert_eq!(
        input_required_step.input,
        Some(json!({
            "goal":"request approval",
            "question":"approve?"
        }))
    );

    let Json(run_after_input_required) =
        get_team_run(State(state.clone()), headers.clone(), Path(run.id.clone()))
            .await
            .expect("get run after input required");
    assert_eq!(
        run_after_input_required.status,
        crate::team::TeamRunStatus::InputRequired
    );

    let invalid_reason_err = set_team_run_step_input_required(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), step.id.clone())),
        Json(SetTeamRunStepInputRequiredRequest {
            reason: Some("   ".to_string()),
            input: None,
        }),
    )
    .await
    .expect_err("blank reason should fail");
    assert_eq!(
        invalid_reason_err.into_response().status(),
        StatusCode::BAD_REQUEST
    );

    let Json(resumed_step) = resume_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), step.id.clone())),
        Json(ResumeTeamRunStepRequest {
            input: Some(json!({"answer":"approved"})),
        }),
    )
    .await
    .expect("resume step");
    assert_eq!(resumed_step.status, crate::team::TeamStepStatus::Working);
    assert!(resumed_step.error_text.is_none());
    assert_eq!(
        resumed_step.input,
        Some(json!({
            "goal":"request approval",
            "question":"approve?",
            "answer":"approved"
        }))
    );

    let Json(run_after_resume) =
        get_team_run(State(state.clone()), headers.clone(), Path(run.id.clone()))
            .await
            .expect("get run after resume");
    assert_eq!(run_after_resume.status, crate::team::TeamRunStatus::Working);

    let _ = complete_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), step.id.clone())),
        Json(CompleteTeamRunStepRequest {
            output: Some(json!({"result":"done"})),
        }),
    )
    .await
    .expect("complete step");
    let Json(run_after_complete) =
        get_team_run(State(state.clone()), headers.clone(), Path(run.id.clone()))
            .await
            .expect("get run after complete");
    assert_eq!(
        run_after_complete.status,
        crate::team::TeamRunStatus::Completed
    );

    let Json(events) = list_team_run_events(
        State(state),
        headers,
        Path(run.id.clone()),
        Query(ListTeamRunEventsQuery {
            limit: Some(100),
            before_id: None,
        }),
    )
    .await
    .expect("list events");
    let event_types: Vec<&str> = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect();
    assert_eq!(
        event_types,
        vec![
            "run_submitted",
            "step_submitted",
            "run_working",
            "step_working",
            "run_input_required",
            "step_input_required",
            "run_working",
            "step_resumed",
            "step_completed",
            "continuity_state_updated",
            "run_completed"
        ]
    );
}

#[tokio::test]
async fn start_team_run_step_requests_reconcile_prompt_for_reconcile_loop_steps() {
    let state = build_test_state_without_seeded_team_member_agents().await;
    let headers = auth_headers(&state).await;
    let now = Utc::now().timestamp();
    let workdir = std::env::temp_dir()
        .join(format!("agenthub-reconcile-prompt-worker-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&workdir).expect("create reconcile prompt worker workdir");
    let workdir = workdir.to_string_lossy().to_string();
    sqlx::query("INSERT OR IGNORE INTO safe_paths (path, created_at) VALUES (?1, ?2)")
        .bind(&workdir)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert reconcile prompt worker safe path");
    for agent_id in ["planner", "worker-1"] {
        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref,
                code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, 'use_existing', NULL, NULL, 0, 'created', ?6, ?7)
            "#,
        )
        .bind(agent_id)
        .bind(format!("{agent_id}-agent"))
        .bind(&workdir)
        .bind("/bin/echo")
        .bind("[]")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert reconcile prompt test agent");
    }

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "step-reconcile-prompt-team".to_string(),
            description: Some("team for reconcile prompt wiring".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Reconcile prompt task".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("planner".to_string()),
            context: Some(json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"worker-1",
                        "goal":"finish the patch",
                        "acceptance":["tests pass"],
                        "execution":{"mode":"reconcile_loop","max_rounds":2}
                    }]
                }
            })),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("reconcile-prompt".to_string()),
        },
    )
    .await
    .expect("create task");

    let run = state
        .teams
        .create_run(
            &team.id,
            Some("ctx-reconcile-prompt"),
            json!({"task_id": created.task.id, "prompt":"start reconcile step"}),
        )
        .await
        .expect("create run");
    let step = state
        .teams
        .list_steps(&run.id)
        .await
        .expect("list steps")
        .into_iter()
        .next()
        .expect("materialized step");

    let Json(started) = start_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), step.id.clone())),
        Json(StartTeamRunStepRequest {
            runtime_handle_id: Some("missing-session".to_string()),
        }),
    )
    .await
    .expect("start step");
    assert_eq!(started.status, crate::team::TeamStepStatus::Working);

    let events = state
        .teams
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "step_reconcile_prompt_requested"
                && event.payload["step_id"] == json!(step.id)),
        "expected reconcile prompt requested event: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "step_reconcile_prompt_failed"
                && event.payload["step_id"] == json!(step.id)),
        "expected reconcile prompt failed event: {events:?}"
    );
}

#[tokio::test]
async fn team_run_messages_api_supports_actor_mailbox_flow() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "actor-mailbox-team".to_string(),
            description: None,
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner","role":"coordinator"},{"member_id":"reviewer","role":"worker"}]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-message-api".to_string()),
            input: Some(json!({"prompt":"mailbox flow"})),
        }),
    )
    .await
    .expect("create run");

    let Json(local_message) = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "reviewer".to_string(),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: json!({"text":"review this"}),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send local message");
    assert_eq!(local_message.channel, "coordination");
    assert_eq!(
        local_message.transport,
        crate::team::TeamActorMessageTransport::Local
    );

    let Json(remote_message) = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "remote-reviewer".to_string(),
            to_peer_id: None,
            channel: Some("federation".to_string()),
            transport: Some("remote".to_string()),
            route: Some(json!({"endpoint":"https://remote.example/a2a"})),
            payload: json!({"text":"federated request"}),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send remote message");
    assert_eq!(
        remote_message.transport,
        crate::team::TeamActorMessageTransport::Remote
    );
    assert_eq!(
        remote_message.route,
        Some(json!({"endpoint":"https://remote.example/a2a"}))
    );

    let missing_route_err = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "remote-reviewer-2".to_string(),
            to_peer_id: None,
            channel: None,
            transport: Some("remote".to_string()),
            route: None,
            payload: json!({"text":"missing route"}),
            idempotency_key: None,
        }),
    )
    .await
    .expect_err("remote message without route should fail");
    assert_eq!(
        missing_route_err.into_response().status(),
        StatusCode::BAD_REQUEST
    );

    let invalid_local_target_err = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "unknown-local".to_string(),
            to_peer_id: None,
            channel: None,
            transport: Some("local".to_string()),
            route: None,
            payload: json!({"text":"invalid local target"}),
            idempotency_key: None,
        }),
    )
    .await
    .expect_err("local target must be a member");
    assert_eq!(
        invalid_local_target_err.into_response().status(),
        StatusCode::BAD_REQUEST
    );

    let Json(inbox) = list_team_run_inbox(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunInboxQuery {
            actor_id: "reviewer".to_string(),
            limit: Some(100),
            after_id: None,
            include_delivered: None,
        }),
    )
    .await
    .expect("list inbox");
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].message_id, local_message.message_id);

    let Json(acked) = ack_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), local_message.message_id)),
        Json(AckTeamRunMessageRequest {
            actor_id: "reviewer".to_string(),
        }),
    )
    .await
    .expect("ack message");
    assert_eq!(acked.status, crate::team::TeamActorMessageStatus::Delivered);

    let Json(pending_after_ack) = list_team_run_inbox(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunInboxQuery {
            actor_id: "reviewer".to_string(),
            limit: Some(100),
            after_id: None,
            include_delivered: Some(false),
        }),
    )
    .await
    .expect("list pending after ack");
    assert!(pending_after_ack.is_empty());

    let Json(inbox_with_delivered) = list_team_run_inbox(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunInboxQuery {
            actor_id: "reviewer".to_string(),
            limit: Some(100),
            after_id: None,
            include_delivered: Some(true),
        }),
    )
    .await
    .expect("list inbox include delivered");
    assert_eq!(inbox_with_delivered.len(), 1);
    assert_eq!(
        inbox_with_delivered[0].status,
        crate::team::TeamActorMessageStatus::Delivered
    );

    let wrong_actor_ack_err = ack_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), remote_message.message_id)),
        Json(AckTeamRunMessageRequest {
            actor_id: "reviewer".to_string(),
        }),
    )
    .await
    .expect_err("ack by non-recipient should fail");
    assert_eq!(
        wrong_actor_ack_err.into_response().status(),
        StatusCode::NOT_FOUND
    );

    let blank_actor_err = ack_team_run_message(
        State(state),
        headers,
        Path((run.id.clone(), remote_message.message_id)),
        Json(AckTeamRunMessageRequest {
            actor_id: "   ".to_string(),
        }),
    )
    .await
    .expect_err("blank actor id should fail");
    assert_eq!(
        blank_actor_err.into_response().status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn team_run_messages_api_triage_resolves_open_reply_obligation() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "actor-mailbox-triage-team".to_string(),
            description: Some("triage reply obligation coverage".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-message-triage".to_string()),
            input: Some(json!({"prompt":"triage mailbox obligation"})),
        }),
    )
    .await
    .expect("create run");

    let now = Utc::now().timestamp();
    let payload_json = json!({
        "type":"chat_message",
        "text":"Please reply with the current status.",
        "source_kind":"human",
        "source_surface":"conversation",
        "conversation_id":"conv-human-1",
        "requires_user_visible_reply": true,
        "reply_target": {
            "surface":"conversation",
            "conversation_id":"conv-human-1",
            "task_message_id": 1
        }
    })
    .to_string();
    let message_id = sqlx::query(
        r#"
        INSERT INTO team_actor_messages (
            run_id,
            from_actor_id,
            to_actor_id,
            channel,
            transport,
            route_json,
            payload_json,
            idempotency_key,
            status,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL, 'pending', ?7)
        "#,
    )
    .bind(&run.id)
    .bind("user")
    .bind("worker-1")
    .bind("default")
    .bind("local")
    .bind(&payload_json)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert human mailbox message")
    .last_insert_rowid();

    let Json(snapshot_before) = get_team_run_snapshot(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(TeamRunSnapshotQuery {
            event_limit: Some(100),
            message_limit: Some(100),
        }),
    )
    .await
    .expect("get snapshot before triage");
    assert_eq!(snapshot_before.mailbox.open_reply_obligation_count, 1);
    assert_eq!(snapshot_before.mailbox.open_reply_obligations.len(), 1);
    assert_eq!(
        snapshot_before.mailbox.open_reply_obligations[0].message_id,
        message_id
    );
    assert_eq!(
        snapshot_before.mailbox.open_reply_obligations[0].agent_actor_id,
        "worker-1"
    );
    assert_eq!(
        snapshot_before.mailbox.open_reply_obligations[0].human_actor_id,
        "user"
    );
    assert_eq!(
        snapshot_before.mailbox.open_reply_obligations[0].source_surface,
        "conversation"
    );
    let worker_before = snapshot_before
        .members
        .iter()
        .find(|member| member.member_id == "worker-1")
        .expect("find worker before triage");
    assert_eq!(worker_before.reply_obligation_count, 1);

    let Json(pending_before) = list_team_run_inbox(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunInboxQuery {
            actor_id: "worker-1".to_string(),
            limit: Some(100),
            after_id: None,
            include_delivered: Some(false),
        }),
    )
    .await
    .expect("list pending before triage");
    assert_eq!(pending_before.len(), 1);
    assert_eq!(pending_before[0].message_id, message_id);

    let visible_reply_payload_json = json!({
        "type":"chat_message",
        "text":"Current status: deploy is in progress."
    })
    .to_string();
    sqlx::query(
        r#"
        INSERT INTO team_actor_messages (
            run_id,
            from_actor_id,
            to_actor_id,
            channel,
            transport,
            route_json,
            payload_json,
            idempotency_key,
            status,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL, 'delivered', ?7)
        "#,
    )
    .bind(&run.id)
    .bind("worker-1")
    .bind("user")
    .bind("default")
    .bind("local")
    .bind(&visible_reply_payload_json)
    .bind(now + 1)
    .execute(&state.db)
    .await
    .expect("insert visible human reply");

    let Json(triaged) = triage_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), message_id)),
        Json(TriageTeamRunMessageRequest {
            actor_id: "worker-1".to_string(),
            disposition: "completed".to_string(),
        }),
    )
    .await
    .expect("triage mailbox message");
    assert_eq!(
        triaged.handling_disposition,
        agenthub_team_actor::ActorMessageHandlingDisposition::Completed
    );

    let Json(pending_after) = list_team_run_inbox(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunInboxQuery {
            actor_id: "worker-1".to_string(),
            limit: Some(100),
            after_id: None,
            include_delivered: Some(false),
        }),
    )
    .await
    .expect("list pending after triage");
    assert!(pending_after.is_empty());

    let Json(snapshot_after) = get_team_run_snapshot(
        State(state),
        headers,
        Path(run.id),
        Query(TeamRunSnapshotQuery {
            event_limit: Some(100),
            message_limit: Some(100),
        }),
    )
    .await
    .expect("get snapshot after triage");
    assert_eq!(snapshot_after.mailbox.open_reply_obligation_count, 0);
    assert!(snapshot_after.mailbox.open_reply_obligations.is_empty());
    let worker_after = snapshot_after
        .members
        .iter()
        .find(|member| member.member_id == "worker-1")
        .expect("find worker after triage");
    assert_eq!(worker_after.reply_obligation_count, 0);
}

#[tokio::test]
async fn team_run_messages_api_triage_rejects_completed_without_visible_reply() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "actor-mailbox-complete-guard-team".to_string(),
            description: Some("completed triage requires visible reply".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-message-complete-guard".to_string()),
            input: Some(json!({"prompt":"completed requires visible reply"})),
        }),
    )
    .await
    .expect("create run");

    let now = Utc::now().timestamp();
    let payload_json = json!({
        "type":"chat_message",
        "text":"Please confirm the rollout result.",
        "source_kind":"human",
        "source_surface":"conversation",
        "conversation_id":"conv-complete-guard-1",
        "requires_user_visible_reply": true,
        "reply_target": {
            "surface":"conversation",
            "conversation_id":"conv-complete-guard-1",
            "task_message_id": 1
        }
    })
    .to_string();
    let message_id = sqlx::query(
        r#"
        INSERT INTO team_actor_messages (
            run_id,
            from_actor_id,
            to_actor_id,
            channel,
            transport,
            route_json,
            payload_json,
            idempotency_key,
            status,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL, 'pending', ?7)
        "#,
    )
    .bind(&run.id)
    .bind("user")
    .bind("worker-1")
    .bind("default")
    .bind("local")
    .bind(&payload_json)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert human mailbox message")
    .last_insert_rowid();

    let completed_err = triage_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), message_id)),
        Json(TriageTeamRunMessageRequest {
            actor_id: "worker-1".to_string(),
            disposition: "completed".to_string(),
        }),
    )
    .await
    .expect_err("completed triage without visible reply should fail");
    assert_eq!(
        completed_err.into_response().status(),
        StatusCode::BAD_REQUEST
    );

    let Json(snapshot_after) = get_team_run_snapshot(
        State(state),
        headers,
        Path(run.id),
        Query(TeamRunSnapshotQuery {
            event_limit: Some(100),
            message_limit: Some(100),
        }),
    )
    .await
    .expect("get snapshot after rejected completion");
    assert_eq!(snapshot_after.mailbox.open_reply_obligation_count, 1);
    let worker_after = snapshot_after
        .members
        .iter()
        .find(|member| member.member_id == "worker-1")
        .expect("find worker after rejected completion");
    assert_eq!(worker_after.reply_obligation_count, 1);
}

#[tokio::test]
async fn team_run_messages_api_triage_ignored_clears_open_reply_obligation_without_visible_reply() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "actor-mailbox-ignore-team".to_string(),
            description: Some("ignored triage clears reply obligation".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-message-ignore-guard".to_string()),
            input: Some(json!({"prompt":"ignored clears reply obligation"})),
        }),
    )
    .await
    .expect("create run");

    let now = Utc::now().timestamp();
    let payload_json = json!({
        "type":"chat_message",
        "text":"Please respond with the rollout owner.",
        "source_kind":"human",
        "source_surface":"conversation",
        "conversation_id":"conv-ignore-guard-1",
        "requires_user_visible_reply": true,
        "reply_target": {
            "surface":"conversation",
            "conversation_id":"conv-ignore-guard-1",
            "task_message_id": 1
        }
    })
    .to_string();
    let message_id = sqlx::query(
        r#"
        INSERT INTO team_actor_messages (
            run_id,
            from_actor_id,
            to_actor_id,
            channel,
            transport,
            route_json,
            payload_json,
            idempotency_key,
            status,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL, 'pending', ?7)
        "#,
    )
    .bind(&run.id)
    .bind("user")
    .bind("worker-1")
    .bind("default")
    .bind("local")
    .bind(&payload_json)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert human mailbox message")
    .last_insert_rowid();

    let Json(ignored) = triage_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), message_id)),
        Json(TriageTeamRunMessageRequest {
            actor_id: "worker-1".to_string(),
            disposition: "ignored".to_string(),
        }),
    )
    .await
    .expect("ignored triage should succeed");
    assert_eq!(
        ignored.handling_disposition,
        agenthub_team_actor::ActorMessageHandlingDisposition::Ignored
    );

    let Json(snapshot_after) = get_team_run_snapshot(
        State(state),
        headers,
        Path(run.id),
        Query(TeamRunSnapshotQuery {
            event_limit: Some(100),
            message_limit: Some(100),
        }),
    )
    .await
    .expect("get snapshot after ignored triage");
    assert_eq!(snapshot_after.mailbox.open_reply_obligation_count, 0);
    assert!(snapshot_after.mailbox.open_reply_obligations.is_empty());
    let worker_after = snapshot_after
        .members
        .iter()
        .find(|member| member.member_id == "worker-1")
        .expect("find worker after ignored triage");
    assert_eq!(worker_after.reply_obligation_count, 0);
}

#[tokio::test]
async fn team_run_messages_api_escalation_reassigns_open_reply_obligation_to_coordinator() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "actor-mailbox-escalation-team".to_string(),
            description: Some("escalation reassigns reply obligation".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-message-escalation".to_string()),
            input: Some(json!({"prompt":"escalate reply obligation"})),
        }),
    )
    .await
    .expect("create run");

    let now = Utc::now().timestamp();
    let payload_json = json!({
        "type":"chat_message",
        "text":"Please confirm the final release owner.",
        "source_kind":"human",
        "source_surface":"conversation",
        "conversation_id":"conv-escalation-1",
        "requires_user_visible_reply": true,
        "reply_target": {
            "surface":"conversation",
            "conversation_id":"conv-escalation-1",
            "task_message_id": 1
        }
    })
    .to_string();
    let original_message_id = sqlx::query(
        r#"
        INSERT INTO team_actor_messages (
            run_id,
            from_actor_id,
            to_actor_id,
            channel,
            transport,
            route_json,
            payload_json,
            idempotency_key,
            status,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL, 'pending', ?7)
        "#,
    )
    .bind(&run.id)
    .bind("user")
    .bind("worker-1")
    .bind("default")
    .bind("local")
    .bind(&payload_json)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert human mailbox message")
    .last_insert_rowid();

    let Json(escalated) = escalate_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), original_message_id)),
        Json(EscalateTeamRunMessageRequest {
            actor_id: "worker-1".to_string(),
        }),
    )
    .await
    .expect("escalate mailbox obligation");
    assert_eq!(escalated.to_actor_id, "planner");
    assert_eq!(escalated.status, agenthub_team_actor::ActorMessageStatus::Pending);

    let Json(worker_pending_after) = list_team_run_inbox(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunInboxQuery {
            actor_id: "worker-1".to_string(),
            limit: Some(100),
            after_id: None,
            include_delivered: Some(false),
        }),
    )
    .await
    .expect("list worker inbox after escalation");
    assert!(worker_pending_after.is_empty());

    let Json(planner_pending_after) = list_team_run_inbox(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunInboxQuery {
            actor_id: "planner".to_string(),
            limit: Some(100),
            after_id: None,
            include_delivered: Some(false),
        }),
    )
    .await
    .expect("list planner inbox after escalation");
    assert_eq!(planner_pending_after.len(), 1);
    assert_eq!(planner_pending_after[0].message_id, escalated.message_id);

    let Json(snapshot_after) = get_team_run_snapshot(
        State(state),
        headers,
        Path(run.id),
        Query(TeamRunSnapshotQuery {
            event_limit: Some(100),
            message_limit: Some(100),
        }),
    )
    .await
    .expect("get snapshot after escalation");
    assert_eq!(snapshot_after.mailbox.open_reply_obligation_count, 1);
    assert_eq!(snapshot_after.mailbox.open_reply_obligations.len(), 1);
    assert_eq!(
        snapshot_after.mailbox.open_reply_obligations[0].agent_actor_id,
        "planner"
    );
    let planner_after = snapshot_after
        .members
        .iter()
        .find(|member| member.member_id == "planner")
        .expect("find planner after escalation");
    assert_eq!(planner_after.reply_obligation_count, 1);
    let worker_after = snapshot_after
        .members
        .iter()
        .find(|member| member.member_id == "worker-1")
        .expect("find worker after escalation");
    assert_eq!(worker_after.reply_obligation_count, 0);
}

#[tokio::test]
async fn team_run_messages_api_triage_surfaces_takeover_state() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "actor-mailbox-takeover-team".to_string(),
            description: Some("triage takeover coverage".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-message-takeover".to_string()),
            input: Some(json!({"prompt":"triage takeover"})),
        }),
    )
    .await
    .expect("create run");

    let now = Utc::now().timestamp();
    let payload_json = json!({
        "type":"chat_message",
        "text":"Please take over the deployment thread.",
        "source_kind":"human",
        "source_surface":"thread",
        "task_id":"task-mailbox-1",
        "task_message_id": 77,
        "thread_root_message_id": 42,
        "conversation_id":"conv-takeover-1",
        "requires_user_visible_reply": true,
        "reply_target": {
            "surface":"thread",
            "task_id":"task-mailbox-1",
            "conversation_id":"conv-takeover-1",
            "task_message_id": 77,
            "thread_root_message_id": 42
        }
    })
    .to_string();
    let message_id = sqlx::query(
        r#"
        INSERT INTO team_actor_messages (
            run_id,
            from_actor_id,
            to_actor_id,
            channel,
            transport,
            route_json,
            payload_json,
            idempotency_key,
            status,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL, 'pending', ?7)
        "#,
    )
    .bind(&run.id)
    .bind("user")
    .bind("worker-1")
    .bind("default")
    .bind("local")
    .bind(&payload_json)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert human takeover message")
    .last_insert_rowid();

    let Json(claimed) = triage_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), message_id)),
        Json(TriageTeamRunMessageRequest {
            actor_id: "worker-1".to_string(),
            disposition: "claimed".to_string(),
        }),
    )
    .await
    .expect("claim mailbox topic");
    assert_eq!(
        claimed.handling_disposition,
        agenthub_team_actor::ActorMessageHandlingDisposition::Claimed
    );
    assert_eq!(
        claimed.thread_claim_status,
        Some(agenthub_team_actor::ActorThreadClaimStatus::Claimed)
    );
    assert_eq!(claimed.thread_owner_actor_id.as_deref(), Some("worker-1"));

    let Json(snapshot_claimed) = get_team_run_snapshot(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(TeamRunSnapshotQuery {
            event_limit: Some(100),
            message_limit: Some(100),
        }),
    )
    .await
    .expect("get snapshot after claim");
    assert_eq!(snapshot_claimed.mailbox.open_reply_obligation_count, 1);
    let claimed_message = snapshot_claimed
        .mailbox
        .recent_messages
        .iter()
        .find(|message| message.message_id == message_id)
        .expect("find claimed snapshot message");
    assert_eq!(
        claimed_message.handling_disposition,
        agenthub_team_actor::ActorMessageHandlingDisposition::Claimed
    );
    assert_eq!(
        claimed_message.thread_claim_status,
        Some(agenthub_team_actor::ActorThreadClaimStatus::Claimed)
    );
    assert_eq!(claimed_message.thread_owner_actor_id.as_deref(), Some("worker-1"));

    let Json(released) = triage_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), message_id)),
        Json(TriageTeamRunMessageRequest {
            actor_id: "worker-1".to_string(),
            disposition: "released".to_string(),
        }),
    )
    .await
    .expect("release mailbox topic");
    assert_eq!(
        released.handling_disposition,
        agenthub_team_actor::ActorMessageHandlingDisposition::Released
    );
    assert_eq!(released.thread_claim_status, None);
    assert_eq!(released.thread_owner_actor_id, None);

    let Json(snapshot_released) = get_team_run_snapshot(
        State(state),
        headers,
        Path(run.id),
        Query(TeamRunSnapshotQuery {
            event_limit: Some(100),
            message_limit: Some(100),
        }),
    )
    .await
    .expect("get snapshot after release");
    assert_eq!(snapshot_released.mailbox.open_reply_obligation_count, 1);
    let released_message = snapshot_released
        .mailbox
        .recent_messages
        .iter()
        .find(|message| message.message_id == message_id)
        .expect("find released snapshot message");
    assert_eq!(
        released_message.handling_disposition,
        agenthub_team_actor::ActorMessageHandlingDisposition::Released
    );
    assert_eq!(released_message.thread_claim_status, None);
    assert_eq!(released_message.thread_owner_actor_id, None);
}

#[tokio::test]
async fn team_run_messages_api_supports_human_actor_list_and_ack_fallback() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;
    let user = crate::api::authz::require_user(&headers, &state)
        .await
        .expect("require auth user");
    let canonical_user_actor_id = super::canonical_user_actor_id(&user);

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "actor-mailbox-human-alias-team".to_string(),
            description: None,
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner","role":"coordinator"}]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-message-human-alias".to_string()),
            input: Some(json!({"prompt":"human alias mailbox flow"})),
        }),
    )
    .await
    .expect("create run");

    let now = Utc::now().timestamp();
    let payload_json = json!({"type":"chat_message","text":"hello human"}).to_string();
    let alias_message_id = sqlx::query(
        r#"
        INSERT INTO team_actor_messages (
            run_id,
            from_actor_id,
            to_actor_id,
            channel,
            transport,
            route_json,
            payload_json,
            idempotency_key,
            status,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL, 'pending', ?7)
        "#,
    )
    .bind(&run.id)
    .bind("planner")
    .bind("user")
    .bind("default")
    .bind("local")
    .bind(&payload_json)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert alias human message")
    .last_insert_rowid();

    let canonical_message_id = sqlx::query(
        r#"
        INSERT INTO team_actor_messages (
            run_id,
            from_actor_id,
            to_actor_id,
            channel,
            transport,
            route_json,
            payload_json,
            idempotency_key,
            status,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL, 'pending', ?7)
        "#,
    )
    .bind(&run.id)
    .bind("planner")
    .bind(canonical_user_actor_id.as_str())
    .bind("default")
    .bind("local")
    .bind(&payload_json)
    .bind(now + 1)
    .execute(&state.db)
    .await
    .expect("insert canonical human message")
    .last_insert_rowid();

    let Json(inbox_by_alias) = list_team_run_inbox(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunInboxQuery {
            actor_id: "user".to_string(),
            limit: Some(100),
            after_id: None,
            include_delivered: None,
        }),
    )
    .await
    .expect("list inbox by user alias");
    assert_eq!(inbox_by_alias.len(), 2);
    assert_eq!(inbox_by_alias[0].message_id, alias_message_id);
    assert_eq!(inbox_by_alias[1].message_id, canonical_message_id);

    let Json(inbox_by_canonical) = list_team_run_inbox(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunInboxQuery {
            actor_id: canonical_user_actor_id.clone(),
            limit: Some(100),
            after_id: None,
            include_delivered: Some(true),
        }),
    )
    .await
    .expect("list inbox by canonical user actor id");
    assert_eq!(inbox_by_canonical.len(), 2);
    assert!(
        inbox_by_canonical
            .iter()
            .all(|message| message.status == crate::team::TeamActorMessageStatus::Pending)
    );

    let Json(alias_acked_via_canonical) = ack_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), alias_message_id)),
        Json(AckTeamRunMessageRequest {
            actor_id: canonical_user_actor_id.clone(),
        }),
    )
    .await
    .expect("ack alias-targeted message via canonical actor id");
    assert_eq!(
        alias_acked_via_canonical.status,
        crate::team::TeamActorMessageStatus::Delivered
    );

    let Json(canonical_acked_via_alias) = ack_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), canonical_message_id)),
        Json(AckTeamRunMessageRequest {
            actor_id: "user".to_string(),
        }),
    )
    .await
    .expect("ack canonical-targeted message via alias actor id");
    assert_eq!(
        canonical_acked_via_alias.status,
        crate::team::TeamActorMessageStatus::Delivered
    );

    let Json(inbox_with_delivered) = list_team_run_inbox(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunInboxQuery {
            actor_id: "user".to_string(),
            limit: Some(100),
            after_id: None,
            include_delivered: Some(true),
        }),
    )
    .await
    .expect("list delivered by alias actor id");
    assert_eq!(inbox_with_delivered.len(), 2);
    assert!(
        inbox_with_delivered
            .iter()
            .all(|message| message.status == crate::team::TeamActorMessageStatus::Delivered)
    );

    let invalid_user_actor_err = list_team_run_inbox(
        State(state),
        headers,
        Path(run.id),
        Query(ListTeamRunInboxQuery {
            actor_id: format!("user:{}", Uuid::new_v4()),
            limit: Some(100),
            after_id: None,
            include_delivered: None,
        }),
    )
    .await
    .expect_err("mismatched authenticated user actor should fail");
    assert_eq!(
        invalid_user_actor_err.into_response().status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn team_run_messages_api_supports_idempotency_key() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "actor-mailbox-idempotent-team".to_string(),
            description: None,
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner","role":"coordinator"},{"member_id":"reviewer","role":"worker"}]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-message-idempotent-api".to_string()),
            input: Some(json!({"prompt":"mailbox flow"})),
        }),
    )
    .await
    .expect("create run");

    let Json(first_message) = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "reviewer".to_string(),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: json!({"text":"review this"}),
            idempotency_key: Some("msg-1".to_string()),
        }),
    )
    .await
    .expect("send first message");

    let Json(second_message) = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "reviewer".to_string(),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: json!({"text":"review this"}),
            idempotency_key: Some("msg-1".to_string()),
        }),
    )
    .await
    .expect("send retry message");
    assert_eq!(first_message.message_id, second_message.message_id);

    let mismatch_conflict_err = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "reviewer".to_string(),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: json!({"text":"changed payload"}),
            idempotency_key: Some("msg-1".to_string()),
        }),
    )
    .await
    .expect_err("same idempotency key with changed payload should conflict");
    assert_eq!(
        mismatch_conflict_err.into_response().status(),
        StatusCode::CONFLICT
    );

    let Json(events) = list_team_run_events(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunEventsQuery {
            limit: Some(100),
            before_id: None,
        }),
    )
    .await
    .expect("list run events");
    let sent_count = events
        .iter()
        .filter(|event| event.event_type == "actor_message_sent")
        .count();
    assert_eq!(sent_count, 1);

    let invalid_idempotency_err = send_team_run_message(
        State(state.clone()),
        headers,
        Path(run.id),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "reviewer".to_string(),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: json!({"text":"bad idempotency key"}),
            idempotency_key: Some("   ".to_string()),
        }),
    )
    .await
    .expect_err("blank idempotency key should fail");
    assert_eq!(
        invalid_idempotency_err.into_response().status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn team_run_messages_api_chat_type_hints_repeat_while_other_types_still_suppress() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "actor-mailbox-type-hint-team".to_string(),
            description: None,
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"reviewer","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-message-type-hint-api".to_string()),
            input: Some(json!({"prompt":"mailbox type hint flow"})),
        }),
    )
    .await
    .expect("create run");

    let Json(first_chat) = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "reviewer".to_string(),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: json!({"type":"chat_message","text":"first chat"}),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send first chat message");

    let Json(second_chat) = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "reviewer".to_string(),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: json!({"type":"chat_message","text":"second chat"}),
            idempotency_key: Some("chat-msg-2".to_string()),
        }),
    )
    .await
    .expect("send second chat message");

    let Json(second_chat_retry) = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "reviewer".to_string(),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: json!({"type":"chat_message","text":"second chat"}),
            idempotency_key: Some("chat-msg-2".to_string()),
        }),
    )
    .await
    .expect("retry second chat message");
    assert_eq!(second_chat_retry.message_id, second_chat.message_id);

    let Json(worker_status) = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "reviewer".to_string(),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: json!({"type":"worker_status","status":"done"}),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send worker status message");

    let Json(worker_status_repeat) = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "reviewer".to_string(),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: json!({"type":"worker_status","status":"done-again"}),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send repeated worker status message");

    let Json(events) = list_team_run_events(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunEventsQuery {
            limit: Some(200),
            before_id: None,
        }),
    )
    .await
    .expect("list run events");

    let hint_events = events
        .iter()
        .filter(|event| event.event_type == "actor_mailbox_type_hint")
        .collect::<Vec<_>>();
    assert_eq!(hint_events.len(), 4);

    let first_hint = hint_events
        .iter()
        .find(|event| event.payload["message_id"] == json!(first_chat.message_id))
        .expect("first chat hint event");
    assert!(
        matches!(
            first_hint.payload["status"].as_str(),
            Some("sent" | "send_failed")
        ),
        "unexpected first hint status: {:?}",
        first_hint.payload["status"]
    );
    assert_eq!(first_hint.payload["reason"], json!("direct_agent_message"));
    assert_eq!(first_hint.payload["target_actor_ids"], json!(["reviewer"]));

    let second_hint = hint_events
        .iter()
        .find(|event| event.payload["message_id"] == json!(second_chat.message_id))
        .expect("second chat hint event");
    assert!(
        matches!(
            second_hint.payload["status"].as_str(),
            Some("sent" | "send_failed")
        ),
        "unexpected second hint status: {:?}",
        second_hint.payload["status"]
    );
    assert_eq!(second_hint.payload["reason"], json!("direct_agent_message"));
    assert_eq!(second_hint.payload["target_actor_ids"], json!(["reviewer"]));

    let worker_status_hint = hint_events
        .iter()
        .find(|event| event.payload["message_id"] == json!(worker_status.message_id))
        .expect("worker status hint event");
    assert!(
        matches!(
            worker_status_hint.payload["status"].as_str(),
            Some("sent" | "send_failed")
        ),
        "unexpected worker status hint status: {:?}",
        worker_status_hint.payload["status"]
    );
    assert_eq!(
        worker_status_hint.payload["reason"],
        json!("direct_agent_message")
    );
    assert_eq!(
        worker_status_hint.payload["target_actor_ids"],
        json!(["reviewer"])
    );

    let repeated_worker_status_hint = hint_events
        .iter()
        .find(|event| event.payload["message_id"] == json!(worker_status_repeat.message_id))
        .expect("repeated worker status hint event");
    assert!(
        matches!(
            repeated_worker_status_hint.payload["status"].as_str(),
            Some("sent" | "send_failed")
        ),
        "unexpected repeated worker status hint status: {:?}",
        repeated_worker_status_hint.payload["status"]
    );
    assert_eq!(
        repeated_worker_status_hint.payload["reason"],
        json!("direct_agent_message")
    );
    assert_eq!(
        repeated_worker_status_hint.payload["target_actor_ids"],
        json!(["reviewer"])
    );

    let second_hint_count = hint_events
        .iter()
        .filter(|event| event.payload["message_id"] == json!(second_chat.message_id))
        .count();
    assert_eq!(second_hint_count, 1);
}

#[tokio::test]
async fn team_run_messages_profile_patch_proposal_updates_team_spec_and_is_idempotent() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "actor-profile-patch-team".to_string(),
            description: None,
            spec: json!({
                "entrypoint":"planner",
                "coordinator_member_id":"planner",
                "members":[
                    {
                        "member_id":"planner",
                        "role":"coordinator",
                        "description":"Existing planning lead",
                        "prompt":"Lead with checkpoints.",
                        "skills":["planning"]
                    },
                    {
                        "member_id":"reviewer",
                        "role":"worker",
                        "description":"Review specialist",
                        "prompt":"Review implementation.",
                        "skills":["review"]
                    }
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-profile-patch-team".to_string()),
            input: Some(json!({"prompt":"mailbox profile patch"})),
        }),
    )
    .await
    .expect("create run");

    let request_payload = json!({
        "type":"profile_patch_proposal",
        "target":"team",
        "member_id":"planner",
        "prompt_append":"Escalate blockers in a dedicated section.",
        "description":"Lead planner and review owner."
    });

    let Json(_first_message) = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "reviewer".to_string(),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: request_payload.clone(),
            idempotency_key: Some("profile-team-1".to_string()),
        }),
    )
    .await
    .expect("send team profile patch");

    let Json(_retry_message) = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "reviewer".to_string(),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: request_payload,
            idempotency_key: Some("profile-team-1".to_string()),
        }),
    )
    .await
    .expect("retry team profile patch");

    let Json(updated_team) = get_team(State(state.clone()), headers.clone(), Path(team.id.clone()))
        .await
        .expect("get updated team");
    let planner = updated_team
        .spec
        .get("members")
        .and_then(Value::as_array)
        .and_then(|members| {
            members.iter().find(|member| {
                member
                    .get("member_id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id == "planner")
            })
        })
        .cloned()
        .expect("planner member exists");
    let planner_prompt = planner
        .get("prompt")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(planner_prompt.contains("Lead with checkpoints."));
    assert!(planner_prompt.contains("Escalate blockers in a dedicated section."));
    assert_eq!(
        planner.get("description").and_then(Value::as_str),
        Some("Lead planner and review owner.")
    );
    let Json(events) = list_team_run_events(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(ListTeamRunEventsQuery {
            limit: Some(200),
            before_id: None,
        }),
    )
    .await
    .expect("list run events");
    let applied_events = events
        .iter()
        .filter(|event| event.event_type == "profile_patch_applied")
        .collect::<Vec<_>>();
    assert_eq!(applied_events.len(), 1);
    assert_eq!(applied_events[0].payload["target"], Value::from("team"));
    assert_eq!(
        applied_events[0].payload["member_id"],
        Value::from("planner")
    );
    assert_eq!(
        applied_events[0].payload["description"],
        Value::from("Lead planner and review owner.")
    );
    assert_eq!(
        applied_events[0].payload["before"]["description"],
        Value::from("Existing planning lead")
    );
    assert_eq!(
        applied_events[0].payload["after"]["description"],
        Value::from("Lead planner and review owner.")
    );
}

#[tokio::test]
async fn team_run_messages_profile_patch_proposal_updates_run_overrides_and_snapshot_view() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "actor-profile-patch-run".to_string(),
            description: None,
            spec: json!({
                "entrypoint":"coordinator-agent",
                "coordinator_member_id":"coordinator-agent",
                "members":[
                    {
                        "member_id":"coordinator-agent",
                        "role":"coordinator",
                        "description":"Run lead",
                        "prompt":"Lead the run.",
                        "skills":["planning"]
                    },
                    {
                        "member_id":"worker-agent",
                        "role":"worker",
                        "description":"Baseline execution specialist",
                        "prompt":"Execute baseline tasks.",
                        "skills":["coding"]
                    }
                ]
            }),
        }),
    )
    .await
    .expect("create run patch team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-profile-patch-run".to_string()),
            input: Some(json!({"prompt":"run-specific patch"})),
        }),
    )
    .await
    .expect("create run");

    let Json(_message) = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "coordinator-agent".to_string(),
            from_peer_id: None,
            to_actor_id: "worker-agent".to_string(),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: json!({
                "type":"profile_patch_proposal",
                "target":"run",
                "member_id":"worker-agent",
                "prompt_append":"Ask one clarification question before coding when requirements are incomplete.",
                "description":"Focused implementation specialist."
            }),
            idempotency_key: Some("profile-run-1".to_string()),
        }),
    )
    .await
    .expect("send run profile patch");

    let Json(updated_run) =
        get_team_run(State(state.clone()), headers.clone(), Path(run.id.clone()))
            .await
            .expect("get updated run");
    assert_eq!(
        updated_run.input["profile_overrides"]["members"]["worker-agent"]["description"],
        Value::from("Focused implementation specialist.")
    );

    let Json(unchanged_team) =
        get_team(State(state.clone()), headers.clone(), Path(team.id.clone()))
            .await
            .expect("get unchanged team");
    let worker_member = unchanged_team
        .spec
        .get("members")
        .and_then(Value::as_array)
        .and_then(|members| {
            members.iter().find(|member| {
                member
                    .get("member_id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id == "worker-agent")
            })
        })
        .cloned()
        .expect("worker member exists");
    assert_eq!(
        worker_member.get("prompt").and_then(Value::as_str),
        Some("Execute baseline tasks.")
    );
    assert_eq!(
        worker_member.get("description").and_then(Value::as_str),
        Some("Baseline execution specialist")
    );

    let Json(snapshot) = get_team_run_snapshot(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(TeamRunSnapshotQuery {
            event_limit: Some(200),
            message_limit: Some(200),
        }),
    )
    .await
    .expect("get snapshot");
    let worker_snapshot = snapshot
        .members
        .iter()
        .find(|member| member.member_id == "worker-agent")
        .expect("worker snapshot member exists");
    let worker_prompt = worker_snapshot.prompt.clone().unwrap_or_default();
    assert!(worker_prompt.contains("Execute baseline tasks."));
    assert!(worker_prompt.contains("Ask one clarification question before coding"));
    assert_eq!(
        worker_snapshot.description.as_deref(),
        Some("Focused implementation specialist.")
    );
    assert!(
        worker_snapshot
            .skills
            .iter()
            .any(|skill| skill == "team-worker-executor")
    );
    assert!(
        worker_snapshot
            .skills
            .iter()
            .any(|skill| skill == "team-actor-mailbox")
    );

    let Json(events) = list_team_run_events(
        State(state),
        headers,
        Path(run.id),
        Query(ListTeamRunEventsQuery {
            limit: Some(200),
            before_id: None,
        }),
    )
    .await
    .expect("list run events");
    let applied = events
        .iter()
        .find(|event| event.event_type == "profile_patch_applied")
        .expect("profile_patch_applied event");
    assert_eq!(applied.payload["target"], Value::from("run"));
    assert_eq!(applied.payload["member_id"], Value::from("worker-agent"));
    assert_eq!(
        applied.payload["description"],
        Value::from("Focused implementation specialist.")
    );
    assert_eq!(applied.payload["before"]["description"], Value::Null);
    assert_eq!(
        applied.payload["after"]["description"],
        Value::from("Focused implementation specialist.")
    );
}

#[tokio::test]
async fn team_run_messages_profile_patch_proposal_rejects_skills_add() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "actor-profile-patch-skill-reject".to_string(),
            description: None,
            spec: json!({
                "entrypoint":"coordinator-agent",
                "coordinator_member_id":"coordinator-agent",
                "members":[
                    {
                        "member_id":"coordinator-agent",
                        "role":"coordinator",
                        "description":"Run lead"
                    },
                    {
                        "member_id":"worker-agent",
                        "role":"worker",
                        "description":"Execution specialist"
                    }
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-profile-patch-skill-reject".to_string()),
            input: Some(json!({"prompt":"reject skill patch"})),
        }),
    )
    .await
    .expect("create run");

    let err = send_team_run_message(
        State(state),
        headers,
        Path(run.id),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "coordinator-agent".to_string(),
            from_peer_id: None,
            to_actor_id: "worker-agent".to_string(),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: json!({
                "type":"profile_patch_proposal",
                "target":"run",
                "member_id":"worker-agent",
                "skills_add":["team-actor-mailbox"]
            }),
            idempotency_key: Some("profile-run-skill-reject".to_string()),
        }),
    )
    .await
    .expect_err("reject unsupported skills_add");
    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = decode_json_body(response).await;
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|message| message.contains("system-managed from role")),
        "unexpected error body: {body}",
    );
}

#[tokio::test]
async fn team_run_snapshot_api_returns_member_status_and_mailbox_summary() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "snapshot-team".to_string(),
            description: Some("team snapshot coverage".to_string()),
            spec: json!({
                "entrypoint":"coordinator-agent",
                "coordinator_member_id":"coordinator-agent",
                "members":[
                    {
                        "member_id":"coordinator-agent",
                        "role":"coordinator",
                        "description":"Team architect and integration owner",
                        "model":"gpt-5",
                        "prompt":"Lead the plan",
                        "skills":["planning","review"]
                    },
                    {
                        "member_id":"worker-agent",
                        "role":"worker",
                        "description":"Primary implementation specialist",
                        "model":"gpt-4.1",
                        "prompt":"Execute tasks",
                        "skills":["coding"]
                    }
                ]
            }),
        }),
    )
    .await
    .expect("create snapshot team");

    let Json(run) = create_team_run(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamRunRequest {
            context_id: Some("ctx-snapshot".to_string()),
            input: Some(json!({"goal":"ship feature"})),
        }),
    )
    .await
    .expect("create snapshot run");

    let Json(step) = submit_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SubmitTeamRunStepRequest {
            step_key: "plan-step".to_string(),
            member_id: "coordinator-agent".to_string(),
            depends_on: Some(vec![]),
            input: Some(json!({"task":"plan"})),
        }),
    )
    .await
    .expect("submit snapshot step");

    let Json(_started) = start_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), step.id.clone())),
        Json(StartTeamRunStepRequest {
            runtime_handle_id: Some("session-coordinator-1".to_string()),
        }),
    )
    .await
    .expect("start snapshot step");

    let now = Utc::now().timestamp();
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9)
        "#,
    )
    .bind("coordinator-agent")
    .bind("coordinator-agent")
    .bind("/tmp")
    .bind("/usr/bin/env")
    .bind("[]")
    .bind("use_existing")
    .bind("running")
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert coordinator agent row");

    sqlx::query(
        r#"
        INSERT OR REPLACE INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9)
        "#,
    )
    .bind("worker-agent")
    .bind("worker-agent")
    .bind("/tmp")
    .bind("/usr/bin/env")
    .bind("[]")
    .bind("create_worktree")
    .bind("idle")
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert worker agent row");

    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at)
        VALUES (?1, ?2, ?3, ?4)
        "#,
    )
    .bind("session-coordinator-1")
    .bind("coordinator-agent")
    .bind("working")
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert agent session for snapshot");

    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at)
        VALUES (?1, ?2, ?3, ?4)
        "#,
    )
    .bind("session-worker-1")
    .bind("worker-agent")
    .bind("running")
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert worker session for snapshot");

    let Json(_message) = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "coordinator-agent".to_string(),
            from_peer_id: None,
            to_actor_id: "worker-agent".to_string(),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: json!({"kind":"assign","text":"implement"}),
            idempotency_key: Some("snapshot-msg-1".to_string()),
        }),
    )
    .await
    .expect("send snapshot message");

    let Json(snapshot) = get_team_run_snapshot(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Query(TeamRunSnapshotQuery {
            event_limit: Some(100),
            message_limit: Some(100),
        }),
    )
    .await
    .expect("get team run snapshot");

    assert_eq!(snapshot.run.id, run.id);
    assert_eq!(snapshot.team.id, team.id);
    assert_eq!(snapshot.coordinator_member_id.as_deref(), Some("coordinator-agent"));
    assert_eq!(snapshot.members.len(), 2);
    assert!(snapshot.latest_events.len() >= 3);
    assert_eq!(snapshot.mailbox.pending, 1);
    assert_eq!(snapshot.mailbox.delivered, 0);
    assert_eq!(snapshot.mailbox.dead_letter, 0);
    assert_eq!(snapshot.mailbox.recent_messages.len(), 1);

    let coordinator = snapshot
        .members
        .iter()
        .find(|member| member.member_id == "coordinator-agent")
        .expect("find coordinator");
    assert_eq!(coordinator.role, "coordinator");
    assert_eq!(
        coordinator.description.as_deref(),
        Some("Team architect and integration owner")
    );
    assert_eq!(coordinator.model.as_deref(), Some("gpt-5"));
    assert_eq!(coordinator.prompt.as_deref(), Some("Lead the plan"));
    assert_eq!(
        coordinator.skills,
        crate::team::effective_team_member_skills("coordinator")
    );
    assert_eq!(coordinator.pending_inbox_count, 0);
    assert_eq!(coordinator.status, "working");
    assert_eq!(coordinator.session_id.as_deref(), Some("session-coordinator-1"));
    assert_eq!(coordinator.session_status.as_deref(), Some("working"));
    assert_eq!(
        coordinator
            .latest_step
            .as_ref()
            .and_then(|step| step.runtime_handle_id.as_deref()),
        Some("session-coordinator-1")
    );

    let worker = snapshot
        .members
        .iter()
        .find(|member| member.member_id == "worker-agent")
        .expect("find worker");
    assert_eq!(worker.role, "worker");
    assert_eq!(
        worker.description.as_deref(),
        Some("Primary implementation specialist")
    );
    assert_eq!(worker.model.as_deref(), Some("gpt-4.1"));
    assert_eq!(worker.pending_inbox_count, 1);
    assert_eq!(worker.status, "idle");
    assert_eq!(worker.session_id.as_deref(), Some("session-worker-1"));
    assert_eq!(worker.session_status.as_deref(), Some("running"));
    assert!(worker.latest_step.is_none());
}

#[tokio::test]
async fn team_task_api_lists_gets_and_redacts_context() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "task-api-team".to_string(),
            description: Some("task api coverage".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Kickoff migration".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({
                "source":"ui",
                "token":"do-not-store",
                "nested":{"secret":"x"}
            })),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("kickoff".to_string()),
        },
    )
    .await
    .expect("seed task");
    assert_eq!(created.task.team_id, team.id);
    assert_eq!(created.task.title, "Kickoff migration");
    assert!(created.task.created_by_actor_id.starts_with("user:"));
    assert_eq!(created.task.assigned_member_id.as_deref(), Some("planner"));
    assert_eq!(created.task.context["token"], json!("[redacted]"));
    assert_eq!(
        created.task.context["nested"]["secret"],
        json!("[redacted]")
    );
    assert_eq!(created.conversation.mode, "group_chat");
    assert_eq!(created.conversation.task_id, created.task.id);
    assert_eq!(created.task.status, crate::team::TeamTaskStatus::Open);
    assert!(created.latest_run.is_none());

    let Json(listed) = list_team_tasks(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Query(ListTeamTasksQuery {
            limit: Some(20),
            priority: None,
            include_shared_thread: false,
        }),
    )
    .await
    .expect("list tasks");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.task.id);
    assert_eq!(listed[0].assigned_member_id.as_deref(), Some("planner"));

    let Json(found) = get_team_task(
        State(state),
        headers,
        Path((team.id, created.task.id.clone())),
    )
    .await
    .expect("get task");
    assert_eq!(found.task.id, created.task.id);
    assert_eq!(found.conversation.id, created.conversation.id);
    assert_eq!(found.task.assigned_member_id.as_deref(), Some("planner"));
    assert!(found.latest_run.is_none());
}

#[tokio::test]
async fn team_task_api_keeps_shared_thread_tasks_without_auto_run() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "shared-thread-team".to_string(),
            description: Some("shared thread task coverage".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "All".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({
                "bootstrap_kind":"shared_thread",
                "bootstrap_source":"teams_all"
            })),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("all".to_string()),
        },
    )
    .await
    .expect("create shared thread task");

    assert_eq!(created.task.status, crate::team::TeamTaskStatus::Open);
    assert!(created.latest_run.is_none());
}

#[tokio::test]
async fn team_task_list_api_can_include_shared_thread_when_requested() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "shared-thread-listing-team".to_string(),
            description: Some("shared thread listing coverage".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let shared_created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "All".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({
                "bootstrap_kind":"shared_thread",
                "bootstrap_source":"teams_all"
            })),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("all".to_string()),
        },
    )
    .await
    .expect("create shared thread task");

    let workspace_created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Investigate regression".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({
                "bootstrap_kind":"task_workspace"
            })),
            conversation_mode: Some("to_coordinator".to_string()),
            topic: Some("Investigate regression".to_string()),
        },
    )
    .await
    .expect("create workspace task");

    let Json(default_list) = list_team_tasks(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Query(ListTeamTasksQuery {
            limit: Some(100),
            priority: None,
            include_shared_thread: false,
        }),
    )
    .await
    .expect("list tasks without shared thread");
    assert_eq!(default_list.len(), 1);
    assert_eq!(default_list[0].id, workspace_created.task.id);

    let Json(with_shared_thread) = list_team_tasks(
        State(state),
        headers,
        Path(team.id),
        Query(ListTeamTasksQuery {
            limit: Some(100),
            priority: None,
            include_shared_thread: true,
        }),
    )
    .await
    .expect("list tasks with shared thread");
    assert_eq!(with_shared_thread.len(), 2);
    let listed_ids = with_shared_thread
        .iter()
        .map(|task| task.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        listed_ids,
        std::collections::HashSet::from([
            workspace_created.task.id.as_str(),
            shared_created.task.id.as_str(),
        ])
    );
}

#[tokio::test]
async fn team_shared_thread_api_returns_not_found_when_missing() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "shared-thread-missing-team".to_string(),
            description: Some("shared thread missing coverage".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let err = get_team_shared_thread(State(state), headers, Path(team.id))
        .await
        .expect_err("shared thread should be missing");
    assert_eq!(err.into_response().status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn team_shared_thread_api_ensures_canonical_thread_and_is_idempotent() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "shared-thread-ensure-team".to_string(),
            description: Some("shared thread ensure coverage".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let Json(created) =
        ensure_team_shared_thread(State(state.clone()), headers.clone(), Path(team.id.clone()))
            .await
            .expect("ensure shared thread");

    assert_eq!(created.task.title.to_lowercase(), "all");
    assert_eq!(created.conversation.task_id, created.task.id);

    let Json(found) =
        get_team_shared_thread(State(state.clone()), headers.clone(), Path(team.id.clone()))
            .await
            .expect("get ensured shared thread");
    assert_eq!(found.task.id, created.task.id);

    let Json(second) =
        ensure_team_shared_thread(State(state.clone()), headers, Path(team.id.clone()))
            .await
            .expect("ensure shared thread twice");
    assert_eq!(second.task.id, created.task.id);

    let shared_thread_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_tasks
        WHERE team_id = ?1
          AND (
            lower(trim(title)) = 'all'
            OR lower(trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), ''))) = 'shared_thread'
          )
        "#,
    )
    .bind(&team.id)
    .fetch_one(&state.db)
    .await
    .expect("count shared thread tasks");
    assert_eq!(shared_thread_count, 1);
}

#[tokio::test]
async fn team_shared_thread_api_prefers_thread_with_latest_conversation_message() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "shared-thread-canonical-team".to_string(),
            description: Some("shared thread canonical coverage".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let first = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "All".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({
                "bootstrap_kind":"shared_thread",
                "bootstrap_source":"teams_all"
            })),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("all".to_string()),
        },
    )
    .await
    .expect("create first shared thread");

    let second = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "All".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({
                "bootstrap_kind":"shared_thread",
                "bootstrap_source":"teams_all"
            })),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("all".to_string()),
        },
    )
    .await
    .expect("create second shared thread");

    let _ = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), second.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("user".to_string()),
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({
                "type":"chat_message",
                "text":"older shared thread message"
            }),
            idempotency_key: None,
        }),
    )
    .await
    .expect("append older message");

    let _ = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), first.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("user".to_string()),
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({
                "type":"chat_message",
                "text":"latest shared thread message"
            }),
            idempotency_key: None,
        }),
    )
    .await
    .expect("append latest message");

    let Json(found) =
        get_team_shared_thread(State(state.clone()), headers.clone(), Path(team.id.clone()))
            .await
            .expect("get canonical shared thread");
    assert_eq!(found.task.id, first.task.id);

    let Json(ensured) = ensure_team_shared_thread(State(state), headers, Path(team.id))
        .await
        .expect("ensure canonical shared thread");
    assert_eq!(ensured.task.id, first.task.id);
}

#[tokio::test]
async fn team_thread_reply_api_appends_reply_metadata_for_root_message() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "thread-reply-team".to_string(),
            description: Some("thread reply coverage".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let Json(shared_thread) =
        ensure_team_shared_thread(State(state.clone()), headers.clone(), Path(team.id.clone()))
            .await
            .expect("ensure shared thread");

    let root_message = state
        .teams
        .append_task_conversation_message(
            &shared_thread.task.id,
            "user:root",
            None,
            "group_chat",
            json!({
                "type":"chat_message",
                "text":"Please discuss this in thread"
            }),
        )
        .await
        .expect("append shared root message");

    let Json(reply) = reply_team_thread(
        State(state.clone()),
        headers,
        Path((team.id.clone(), "all".to_string(), root_message.message_id)),
        Json(ReplyTeamThreadRequest {
            text: "Threaded follow-up".to_string(),
            mention_actor_ids: vec![],
        }),
    )
    .await
    .expect("reply team thread");

    assert_eq!(reply.thread.team_id, team.id);
    assert_eq!(reply.thread.channel_id, "all");
    assert_eq!(reply.thread.root_message_id, root_message.message_id);
    assert_eq!(reply.message.route, "team_thread_reply");
    assert_eq!(reply.message.payload["type"], json!("chat_message"));
    assert_eq!(
        reply.message.payload["thread_root_message_id"],
        json!(root_message.message_id)
    );
    assert_eq!(reply.message.payload["text"], json!("Threaded follow-up"));
    assert_eq!(reply.message.payload["mention_actor_ids"], json!([]));
}

#[tokio::test]
async fn team_channel_api_lists_creates_and_deletes_non_default_channels() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "team-channel-api-team".to_string(),
            description: Some("channel api coverage".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let Json(initial_channels) =
        list_team_channels(State(state.clone()), headers.clone(), Path(team.id.clone()))
            .await
            .expect("list initial channels");
    assert!(initial_channels.is_empty());

    let Json(created) = create_team_channel(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamChannelRequest {
            channel_id: "Review".to_string(),
            description: Some("Review lane".to_string()),
        }),
    )
    .await
    .expect("create review channel");
    assert_eq!(created.channel_id, "review");
    assert_eq!(created.description.as_deref(), Some("Review lane"));

    let Json(listed) =
        list_team_channels(State(state.clone()), headers.clone(), Path(team.id.clone()))
            .await
            .expect("list created channels");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].channel_id, "review");

    let Json(deleted) = delete_team_channel(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), " review ".to_string())),
    )
    .await
    .expect("delete review channel");
    assert_eq!(deleted.channel_id, "review");

    let Json(final_channels) =
        list_team_channels(State(state.clone()), headers, Path(team.id.clone()))
            .await
            .expect("list final channels");
    assert!(final_channels.is_empty());
}

#[tokio::test]
async fn team_channel_api_maps_duplicates_and_missing_channel_errors() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "team-channel-api-errors".to_string(),
            description: Some("channel api error coverage".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let _ = create_team_channel(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamChannelRequest {
            channel_id: "review".to_string(),
            description: Some("Review lane".to_string()),
        }),
    )
    .await
    .expect("create review channel");

    let duplicate_err = create_team_channel(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamChannelRequest {
            channel_id: " REVIEW ".to_string(),
            description: Some("Duplicate".to_string()),
        }),
    )
    .await
    .expect_err("duplicate channel should fail");
    assert_eq!(duplicate_err.into_response().status(), StatusCode::CONFLICT);

    let missing_err = delete_team_channel(
        State(state.clone()),
        headers,
        Path((team.id.clone(), "research".to_string())),
    )
    .await
    .expect_err("missing channel should fail");
    assert_eq!(missing_err.into_response().status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn team_thread_reply_api_maps_missing_channel_and_root_to_not_found() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "thread-reply-not-found-team".to_string(),
            description: Some("thread reply not found coverage".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create team");

    let missing_channel_err = reply_team_thread(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), "review".to_string(), 17)),
        Json(ReplyTeamThreadRequest {
            text: "Missing channel".to_string(),
            mention_actor_ids: vec![],
        }),
    )
    .await
    .expect_err("missing channel should fail");
    assert_eq!(missing_channel_err.into_response().status(), StatusCode::NOT_FOUND);

    let Json(shared_thread) =
        ensure_team_shared_thread(State(state.clone()), headers.clone(), Path(team.id.clone()))
            .await
            .expect("ensure shared thread");
    let missing_root_err = reply_team_thread(
        State(state.clone()),
        headers,
        Path((team.id.clone(), "all".to_string(), 99999)),
        Json(ReplyTeamThreadRequest {
            text: "Missing root".to_string(),
            mention_actor_ids: vec![],
        }),
    )
    .await
    .expect_err("missing root should fail");
    assert_eq!(missing_root_err.into_response().status(), StatusCode::NOT_FOUND);

    let shared_task = state
        .teams
        .get_task(&shared_thread.task.id)
        .await
        .expect("shared thread task still exists");
    assert_eq!(shared_task.team_id, team.id);
}

#[tokio::test]
async fn team_thread_reply_api_notifies_existing_thread_participants() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "thread-reply-participants-team".to_string(),
            description: Some("thread participant mailbox forwarding coverage".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"},
                    {"member_id":"worker-2","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(shared_thread) =
        ensure_team_shared_thread(State(state.clone()), headers.clone(), Path(team.id.clone()))
            .await
            .expect("ensure shared thread");

    let root_message = state
        .teams
        .append_task_conversation_message(
            &shared_thread.task.id,
            "planner",
            None,
            "group_chat",
            json!({
                "type":"chat_message",
                "text":"Track the follow-up in this thread"
            }),
        )
        .await
        .expect("append shared root message");

    state
        .teams
        .reply_thread(
            &team.id,
            "all",
            root_message.message_id,
            "worker-1",
            "I am already looking into it.",
            &[],
        )
        .await
        .expect("append worker thread reply");

    let Json(reply) = reply_team_thread(
        State(state.clone()),
        headers,
        Path((team.id.clone(), "all".to_string(), root_message.message_id)),
        Json(ReplyTeamThreadRequest {
            text: "Please keep this thread updated.".to_string(),
            mention_actor_ids: vec!["worker-2".to_string()],
        }),
    )
    .await
    .expect("reply team thread");

    let mailbox_run = state
        .teams
        .get_latest_run_for_task(&team.id, &shared_thread.task.id)
        .await
        .expect("load shared thread mailbox run")
        .expect("shared thread mailbox run should exist");

    let rows = sqlx::query(
        r#"
        SELECT from_actor_id, to_actor_id, payload_json
        FROM team_actor_messages
        WHERE run_id = ?1
        ORDER BY id ASC
        "#,
    )
    .bind(&mailbox_run.id)
    .fetch_all(&state.db)
    .await
    .expect("load mailbox rows");

    let recipients = rows
        .iter()
        .map(|row| row.get::<String, _>("to_actor_id"))
        .collect::<Vec<_>>();
    assert_eq!(recipients, vec!["worker-1".to_string(), "worker-2".to_string()]);
    for row in &rows {
        assert_eq!(row.get::<String, _>("from_actor_id"), "planner");
        let forwarded_payload: Value =
            serde_json::from_str(row.get::<String, _>("payload_json").as_str())
                .expect("parse forwarded payload");
        assert_eq!(
            forwarded_payload["delivery_scope"],
            Value::from("thread_participants")
        );
        assert_eq!(
            forwarded_payload["task_message_id"],
            Value::from(reply.message.message_id)
        );
        assert_eq!(
            forwarded_payload["thread_root_message_id"],
            Value::from(root_message.message_id)
        );
        assert_eq!(forwarded_payload["mention_actor_ids"], json!(["worker-2"]));
        assert_eq!(forwarded_payload["source_kind"], json!("human"));
        assert_eq!(forwarded_payload["source_surface"], json!("thread"));
        assert_eq!(forwarded_payload["requires_user_visible_reply"], json!(true));
        assert_eq!(
            forwarded_payload["reply_target"],
            json!({
                "surface":"thread",
                "task_id": shared_thread.task.id,
                "conversation_id": reply.message.conversation_id,
                "task_message_id": reply.message.message_id,
                "thread_root_message_id": root_message.message_id
            })
        );
    }
}

#[tokio::test]
async fn team_task_messages_api_forwards_shared_thread_human_chat_without_active_run() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "shared-thread-mailbox-forward-team".to_string(),
            description: Some("shared thread mailbox forwarding coverage".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"},
                    {"member_id":"worker-2","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let task_created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "All".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({
                "bootstrap_kind":"shared_thread",
                "bootstrap_source":"teams_all"
            })),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("all".to_string()),
        },
    )
    .await
    .expect("create shared thread task");
    assert!(task_created.latest_run.is_none());

    let Json(directed_message) = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), task_created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: None,
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({
                "type":"chat_message",
                "text":"<at>worker-1</at> please inspect the channel delivery path"
            }),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send shared thread message");

    let mailbox_run = state
        .teams
        .get_latest_run_for_task(&team.id, &task_created.task.id)
        .await
        .expect("load shared thread mailbox run")
        .expect("shared thread mailbox run should exist");
    assert_eq!(mailbox_run.status, crate::team::TeamRunStatus::Completed);
    assert_eq!(
        mailbox_run.input["bootstrap_kind"],
        Value::from("shared_thread_mailbox")
    );
    assert_eq!(mailbox_run.input["channel"], Value::from("all"));
    assert_eq!(
        mailbox_run.input["task_id"],
        Value::from(task_created.task.id.clone())
    );

    let rows = sqlx::query(
        r#"
        SELECT from_actor_id, to_actor_id, payload_json
        FROM team_actor_messages
        WHERE run_id = ?1
        ORDER BY id ASC
        "#,
    )
    .bind(&mailbox_run.id)
    .fetch_all(&state.db)
    .await
    .expect("load mailbox rows");
    assert_eq!(rows.len(), 3);
    let recipients = rows
        .iter()
        .map(|row| row.get::<String, _>("to_actor_id"))
        .collect::<Vec<_>>();
    assert_eq!(
        recipients,
        vec![
            "planner".to_string(),
            "worker-1".to_string(),
            "worker-2".to_string()
        ]
    );
    for row in &rows {
        assert_eq!(row.get::<String, _>("from_actor_id"), "planner");
        let forwarded_payload: Value =
            serde_json::from_str(row.get::<String, _>("payload_json").as_str())
                .expect("parse forwarded payload");
        assert_eq!(
            forwarded_payload["delivery_scope"],
            Value::from("broadcast")
        );
        assert_eq!(
            forwarded_payload["task_message_id"],
            Value::from(directed_message.message_id)
        );
        assert_eq!(
            forwarded_payload["task_conversation_id"],
            Value::from(directed_message.conversation_id.clone())
        );
        assert_eq!(forwarded_payload["mention_actor_ids"], json!(["worker-1"]));
        assert_eq!(
            forwarded_payload["mentioned_actor_ids"],
            json!(["worker-1"])
        );
    }

    let service = state.teams.actor_mailbox_service();
    let inbox = service
        .actor_inbox(ActorInboxRequest {
            run_id: mailbox_run.id.clone(),
            actor_id: "worker-1".to_string(),
            cursor: None,
            limit: Some(50),
            states: None,
        })
        .await
        .expect("load worker inbox");
    assert_eq!(inbox.messages.len(), 1);
    assert_eq!(
        inbox.messages[0].payload["task_message_id"],
        Value::from(directed_message.message_id)
    );

    let acked = service
        .actor_ack(ActorAckRequest {
            run_id: mailbox_run.id.clone(),
            actor_id: "worker-1".to_string(),
            message_id: inbox.messages[0].message_id,
            ack_token: None,
            result: None,
        })
        .await
        .expect("ack mailbox message");
    assert_eq!(acked.state, crate::team::TeamActorMessageStatus::Delivered);

    let _reply = service
        .actor_send(ActorSendRequest {
            run_id: mailbox_run.id.clone(),
            from_actor_id: "worker-1".to_string(),
            from_peer_id: None,
            to_actor_id: Some("user".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"@planner delivery path looks healthy",
                "correlation_id":"corr-shared-thread-forward"
            }),
            idempotency_key: Some("shared-thread-forward-reply".to_string()),
            message_kind: None,
        })
        .await
        .expect("send shared thread reply");

    let Json(messages) = list_team_task_messages(
        State(state),
        headers,
        Path((team.id, task_created.task.id)),
        Query(ListTeamTaskMessagesQuery {
            limit: Some(50),
            before_id: None,
        }),
    )
    .await
    .expect("list shared thread messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].from_actor_id, directed_message.from_actor_id);
    assert_eq!(messages[1].from_actor_id, "worker-1");
    assert_eq!(messages[1].route, "group_chat");
    assert_eq!(messages[1].to_actor_id, None);
    assert_eq!(messages[1].payload["type"], Value::from("chat_message"));
    assert_eq!(
        messages[1].payload["text"],
        Value::from("@planner delivery path looks healthy")
    );
    assert_eq!(
        messages[1].payload["correlation_id"],
        Value::from("corr-shared-thread-forward")
    );
}

#[tokio::test]
async fn team_message_search_api_uses_archive_with_team_scope() {
    let archive = Arc::new(RecordingSearchArchive {
        queries: tokio::sync::Mutex::new(Vec::new()),
        hits: vec![MessageSearchHit {
            document_id: "team_conversation_message:conversation-1:42".to_string(),
            source_kind: MessageDocumentKind::TeamConversationMessage,
            body_text: "archive search result".to_string(),
            score: Some(0.75),
            authority_message_id: Some(42),
            correlation_id: Some("corr-search".to_string()),
            group_id: Some("group-search".to_string()),
            team_id: Some("team-from-archive".to_string()),
            run_id: None,
            conversation_id: Some("conversation-1".to_string()),
            task_id: Some("task-1".to_string()),
            agent_id: None,
            session_id: None,
        }],
    });
    let state = build_test_state_with_message_archive(archive.clone()).await;
    let headers = auth_headers(&state).await;
    let team = state
        .teams
        .create_team_with_owner(
            TeamDefinitionConfig {
            name: "message-search-api-team".to_string(),
            description: Some("archive search".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner","role":"coordinator"}]
            }),
            },
            None,
        )
        .await
        .expect("create team");

    let Json(hits) = search_team_messages(
        State(state),
        headers,
        Path(team.id.clone()),
        Query(SearchTeamMessagesQuery {
            query: " archive ".to_string(),
            limit: Some(5),
            authority_message_id: None,
            correlation_id: Some(" corr-search ".to_string()),
            group_id: Some(" group-search ".to_string()),
            run_id: None,
            conversation_id: None,
            task_id: Some(" task-1 ".to_string()),
            agent_id: None,
            session_id: None,
            source_kind: Some("team_conversation_message".to_string()),
        }),
    )
    .await
    .expect("search team messages");

    assert_eq!(
        hits,
        vec![TeamMessageSearchHitResponse {
            document_id: "team_conversation_message:conversation-1:42".to_string(),
            source_kind: MessageDocumentKind::TeamConversationMessage,
            body_text: "archive search result".to_string(),
            score: Some(0.75),
            authority_message_id: Some(42),
            correlation_id: Some("corr-search".to_string()),
            group_id: Some("group-search".to_string()),
            team_id: Some("team-from-archive".to_string()),
            run_id: None,
            conversation_id: Some("conversation-1".to_string()),
            task_id: Some("task-1".to_string()),
            agent_id: None,
            session_id: None,
        }]
    );

    let queries = archive.queries.lock().await;
    assert_eq!(queries.len(), 1);
    assert_eq!(queries[0].query_text, "archive");
    assert_eq!(queries[0].limit, 5);
    assert_eq!(queries[0].team_id.as_deref(), Some(team.id.as_str()));
    assert_eq!(queries[0].correlation_id.as_deref(), Some("corr-search"));
    assert_eq!(queries[0].group_id.as_deref(), Some("group-search"));
    assert_eq!(queries[0].task_id.as_deref(), Some("task-1"));
    assert_eq!(
        queries[0].source_kind,
        Some(MessageDocumentKind::TeamConversationMessage)
    );
}

#[tokio::test]
async fn team_message_search_api_rejects_blank_query() {
    let state = build_test_state_with_message_archive(Arc::new(RecordingSearchArchive {
        queries: tokio::sync::Mutex::new(Vec::new()),
        hits: Vec::new(),
    }))
    .await;
    let headers = auth_headers(&state).await;
    let team = state
        .teams
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "message-search-blank-query-team".to_string(),
                description: Some("archive search".to_string()),
                spec: json!({
                    "entrypoint":"planner",
                    "members":[{"member_id":"planner","role":"coordinator"}]
                }),
            },
            None,
        )
        .await
        .expect("create team");

    let err = search_team_messages(
        State(state),
        headers,
        Path(team.id),
        Query(SearchTeamMessagesQuery {
            query: "   ".to_string(),
            limit: None,
            authority_message_id: None,
            correlation_id: None,
            group_id: None,
            run_id: None,
            conversation_id: None,
            task_id: None,
            agent_id: None,
            session_id: None,
            source_kind: None,
        }),
    )
    .await
    .expect_err("blank archive query should fail");

    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = decode_json_body(response).await;
    assert_eq!(body["error"], Value::from("query is required"));
}

#[tokio::test]
async fn message_archive_source_kind_error_lists_supported_values() {
    let err = parse_message_archive_source_kind("bogus_kind")
        .expect_err("unsupported archive source kind should fail");
    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = decode_json_body(response).await;
    let message = body["error"].as_str().expect("error message");
    assert!(message.contains("bogus_kind"));
    assert!(message.contains("agent_event"));
    assert!(message.contains("team_conversation_message"));
    assert!(message.contains("team_run_event"));
    assert!(message.contains("team_actor_message"));
    assert!(message.contains("aggregated_acp_message"));
}

#[tokio::test]
async fn teams_api_rejects_human_task_status_and_owner_updates() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "task-status-api-team".to_string(),
            description: Some("status update".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Promote kanban card".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({"source":"ui"})),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("status".to_string()),
        },
    )
    .await
    .expect("create task");

    let status_err = update_team_task(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(UpdateTeamTaskRequest {
            status: Some("in_progress".to_string()),
            assigned_member_id: None,
        }),
    )
    .await
    .expect_err("human task status update should fail");
    let status_body = decode_json_body(status_err.into_response()).await;
    assert_eq!(
        status_body["error"],
        Value::from(
            "canonical Team task status/owner updates are agent-only; use actor runtime controls"
        )
    );

    let owner_err = update_team_task(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(UpdateTeamTaskRequest {
            status: None,
            assigned_member_id: Some(Some("worker-1".to_string())),
        }),
    )
    .await
    .expect_err("human task owner update should fail");
    let owner_body = decode_json_body(owner_err.into_response()).await;
    assert_eq!(
        owner_body["error"],
        Value::from(
            "canonical Team task status/owner updates are agent-only; use actor runtime controls"
        )
    );

    let reloaded = state
        .teams
        .get_task(&created.task.id)
        .await
        .expect("reload task");
    assert_eq!(reloaded.status, crate::team::TeamTaskStatus::Open);
    assert_eq!(reloaded.assigned_member_id.as_deref(), Some("planner"));
}

#[tokio::test]
async fn team_task_api_enforces_team_owner_access_for_existing_tasks() {
    let state = build_test_state().await;
    let owner_headers = auth_headers(&state).await;
    let outsider_headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        owner_headers.clone(),
        Json(CreateTeamRequest {
            name: "task-owner-enforcement-team".to_string(),
            description: Some("owner enforcement".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner","role":"coordinator"}]
            }),
        }),
    )
    .await
    .expect("create team");

    let created = create_team_task(
        &state,
        &owner_headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Owner only planning".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({})),
            conversation_mode: Some("group_chat".to_string()),
            topic: None,
        },
    )
    .await
    .expect("create task");

    let send_err = send_team_task_message(
        State(state.clone()),
        outsider_headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("user".to_string()),
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({"text":"malicious broadcast"}),
            idempotency_key: None,
        }),
    )
    .await
    .expect_err("outsider should not send task message");
    assert_eq!(send_err.into_response().status(), StatusCode::NOT_FOUND);

    let compile_err = compile_team_task_run_preview(
        State(state),
        outsider_headers,
        Path((team.id, created.task.id)),
        Json(CompileTeamTaskRunPreviewRequest { context_id: None }),
    )
    .await
    .expect_err("outsider should not compile preview");
    assert_eq!(compile_err.into_response().status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn team_task_messages_api_supports_route_and_redaction() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "task-msg-team".to_string(),
            description: Some("task message api coverage".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Discuss rollout".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({})),
            conversation_mode: Some("group_chat".to_string()),
            topic: None,
        },
    )
    .await
    .expect("create task");
    assert!(created.task.created_by_actor_id.starts_with("user:"));

    let missing_to_member_err = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("user".to_string()),
            to_actor_id: None,
            route: Some("to_member".to_string()),
            payload: json!({"text":"assign"}),
            idempotency_key: None,
        }),
    )
    .await
    .expect_err("route=to_member should require to_actor_id");
    assert_eq!(
        missing_to_member_err.into_response().status(),
        StatusCode::BAD_REQUEST
    );

    let invalid_sender_err = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("outsider".to_string()),
            to_actor_id: Some("worker-1".to_string()),
            route: Some("to_member".to_string()),
            payload: json!({"text":"assign"}),
            idempotency_key: None,
        }),
    )
    .await
    .expect_err("unknown sender should be rejected");
    assert_eq!(
        invalid_sender_err.into_response().status(),
        StatusCode::BAD_REQUEST
    );

    let invalid_group_chat_target_err = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("user".to_string()),
            to_actor_id: Some("planner".to_string()),
            route: Some("group_chat".to_string()),
            payload: json!({"text":"broadcast"}),
            idempotency_key: None,
        }),
    )
    .await
    .expect_err("group_chat should not allow to_actor_id");
    assert_eq!(
        invalid_group_chat_target_err.into_response().status(),
        StatusCode::BAD_REQUEST
    );

    let Json(message) = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: None,
            to_actor_id: Some("worker-1".to_string()),
            route: Some("to_member".to_string()),
            payload: json!({
                "text":"assign",
                "authorization":"Bearer abc",
                "nested":{"api_key":"123"}
            }),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send task message");
    assert_eq!(message.route, "to_member");
    assert!(message.from_actor_id.starts_with("user:"));
    assert_eq!(message.to_actor_id.as_deref(), Some("worker-1"));
    assert!(
        message
            .payload
            .get("correlation_id")
            .and_then(Value::as_str)
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
    );
    assert_eq!(message.payload["authorization"], json!("[redacted]"));
    assert_eq!(message.payload["nested"]["api_key"], json!("[redacted]"));

    let Json(to_coordinator_message) = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("worker-1".to_string()),
            to_actor_id: None,
            route: Some("to_coordinator".to_string()),
            payload: json!({"text":"need clarification"}),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send to coordinator message");
    assert_eq!(to_coordinator_message.route, "to_coordinator");
    assert_eq!(to_coordinator_message.to_actor_id.as_deref(), Some("planner"));

    let Json(group_message) = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("planner".to_string()),
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({
                "text":"status update",
                "correlation_id":"corr-group-status-update"
            }),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send group chat message");
    assert_eq!(group_message.route, "group_chat");
    assert_eq!(group_message.to_actor_id, None);
    assert_eq!(
        group_message.payload["correlation_id"],
        Value::from("corr-group-status-update")
    );

    let Json(messages) = list_team_task_messages(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Query(ListTeamTaskMessagesQuery {
            limit: Some(50),
            before_id: None,
        }),
    )
    .await
    .expect("list messages");
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].message_id, message.message_id);
    assert_eq!(messages[1].message_id, to_coordinator_message.message_id);
    assert_eq!(messages[2].message_id, group_message.message_id);
    assert_eq!(messages[0].route, "to_member");
    assert_eq!(messages[1].route, "to_coordinator");
    assert_eq!(messages[2].route, "group_chat");

    let Json(empty_page) = list_team_task_messages(
        State(state),
        headers,
        Path((team.id, created.task.id)),
        Query(ListTeamTaskMessagesQuery {
            limit: Some(50),
            before_id: Some(message.message_id),
        }),
    )
    .await
    .expect("list messages with before_id");
    assert!(empty_page.is_empty());
}

#[tokio::test]
async fn team_task_messages_api_supports_idempotency_key_and_dedupes_mailbox_forwarding() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "task-msg-idempotent-team".to_string(),
            description: Some("task message idempotency coverage".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Retry-safe chat".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({})),
            conversation_mode: Some("group_chat".to_string()),
            topic: None,
        },
    )
    .await
    .expect("create task");

    let run = state
        .teams
        .create_run(
            &team.id,
            Some(created.task.id.as_str()),
            json!({
                "task_id": created.task.id.clone(),
                "conversation_id": created.conversation.id.clone(),
            }),
        )
        .await
        .expect("create task run");

    let Json(first_message) = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: None,
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({
                "type":"chat_message",
                "text":"@worker-1 please verify the retry-safe path"
            }),
            idempotency_key: Some("task-msg-1".to_string()),
        }),
    )
    .await
    .expect("send first task message");

    let Json(retry_message) = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: None,
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({
                "type":"chat_message",
                "text":"@worker-1 please verify the retry-safe path"
            }),
            idempotency_key: Some("task-msg-1".to_string()),
        }),
    )
    .await
    .expect("retry task message");

    assert_eq!(first_message.message_id, retry_message.message_id);
    assert_eq!(
        first_message.payload["correlation_id"],
        retry_message.payload["correlation_id"]
    );

    let conversation_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM team_conversation_messages WHERE task_id = ?1")
            .bind(&created.task.id)
            .fetch_one(&state.db)
            .await
            .expect("count deduped conversation messages");
    assert_eq!(conversation_count, 1);

    let mailbox_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM team_actor_messages WHERE run_id = ?1")
            .bind(&run.id)
            .fetch_one(&state.db)
            .await
            .expect("count deduped mailbox messages");
    assert_eq!(mailbox_count, 2);

    let conflict_err = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: None,
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({
                "type":"chat_message",
                "text":"different payload should conflict"
            }),
            idempotency_key: Some("task-msg-1".to_string()),
        }),
    )
    .await
    .expect_err("same idempotency_key with different payload should conflict");
    assert_eq!(conflict_err.into_response().status(), StatusCode::CONFLICT);

    let invalid_idempotency_err = send_team_task_message(
        State(state),
        headers,
        Path((team.id, created.task.id)),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: None,
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({
                "type":"chat_message",
                "text":"blank idempotency key should fail"
            }),
            idempotency_key: Some("   ".to_string()),
        }),
    )
    .await
    .expect_err("blank idempotency_key should fail");
    assert_eq!(
        invalid_idempotency_err.into_response().status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn team_task_messages_api_forwards_human_chat_to_active_run_mailbox() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let team = state
        .teams
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "task-mailbox-forward-team".to_string(),
                description: Some("task to mailbox forwarding coverage".to_string()),
                spec: json!({
                    "entrypoint":"planner",
                    "members":[
                        {"member_id":"planner","role":"coordinator"},
                        {"member_id":"worker-1","role":"worker"},
                        {"member_id":"worker-2","role":"worker"}
                    ]
                }),
            },
            None,
        )
        .await
        .expect("create team");

    let task_created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Mailbox forwarding".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({})),
            conversation_mode: Some("group_chat".to_string()),
            topic: None,
        },
    )
    .await
    .expect("create task");
    assert!(task_created.latest_run.is_none());

    let run = state
        .teams
        .create_run(
            &team.id,
            Some(task_created.task.id.as_str()),
            json!({
                "task_id": task_created.task.id.clone(),
                "conversation_id": task_created.conversation.id.clone(),
            }),
        )
        .await
        .expect("create explicit task run");

    let Json(directed_message) = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), task_created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: None,
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({
                "type":"chat_message",
                "text":"@worker-1 please validate api contract"
            }),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send mention task message");

    assert!(directed_message.from_actor_id.starts_with("user:"));
    let directed_rows = sqlx::query(
        r#"
        SELECT from_actor_id, to_actor_id, payload_json
        FROM team_actor_messages
        WHERE run_id = ?1
        ORDER BY id ASC
        "#,
    )
    .bind(&run.id)
    .fetch_all(&state.db)
    .await
    .expect("load directed mailbox rows");
    assert_eq!(directed_rows.len(), 3);
    let directed_recipients = directed_rows
        .iter()
        .map(|row| row.get::<String, _>("to_actor_id"))
        .collect::<Vec<_>>();
    assert_eq!(
        directed_recipients,
        vec![
            "planner".to_string(),
            "worker-1".to_string(),
            "worker-2".to_string()
        ]
    );
    for row in &directed_rows {
        assert_eq!(row.get::<String, _>("from_actor_id"), "planner".to_string());
        let directed_payload: Value =
            serde_json::from_str(row.get::<String, _>("payload_json").as_str())
                .expect("parse directed payload json");
        assert_eq!(directed_payload["delivery_scope"], Value::from("broadcast"));
        assert_eq!(
            directed_payload["task_id"],
            Value::from(task_created.task.id.clone())
        );
        assert_eq!(
            directed_payload["task_message_id"],
            Value::from(directed_message.message_id)
        );
        assert_eq!(
            directed_payload["task_conversation_id"],
            Value::from(directed_message.conversation_id.clone())
        );
        assert_eq!(
            directed_payload["mention_actor_ids"],
            Value::from(vec!["worker-1"])
        );
        assert_eq!(
            directed_payload["mentioned_actor_ids"],
            Value::from(vec!["worker-1"])
        );
        assert_eq!(directed_payload["source_kind"], json!("human"));
        assert_eq!(directed_payload["source_surface"], json!("conversation"));
        assert_eq!(directed_payload["requires_user_visible_reply"], json!(true));
        assert_eq!(
            directed_payload["reply_target"],
            json!({
                "surface":"conversation",
                "task_id": task_created.task.id,
                "conversation_id": directed_message.conversation_id,
                "task_message_id": directed_message.message_id
            })
        );
        assert_eq!(
            directed_payload["text"],
            Value::from("@worker-1 please validate api contract")
        );
    }

    let _ = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), task_created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: None,
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({
                "type":"chat_message",
                "text":"status update to everyone"
            }),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send broadcast task message");

    let broadcast_rows = sqlx::query(
        r#"
        SELECT from_actor_id, to_actor_id, payload_json
        FROM team_actor_messages
        WHERE run_id = ?1
        ORDER BY id ASC
        "#,
    )
    .bind(&run.id)
    .fetch_all(&state.db)
    .await
    .expect("load broadcast mailbox rows");
    assert_eq!(broadcast_rows.len(), 6);
    let recipients = broadcast_rows
        .iter()
        .skip(3)
        .map(|row| row.get::<String, _>("to_actor_id"))
        .collect::<Vec<_>>();
    assert_eq!(
        recipients,
        vec![
            "planner".to_string(),
            "worker-1".to_string(),
            "worker-2".to_string()
        ]
    );
    for row in broadcast_rows.iter().skip(3) {
        let payload: Value = serde_json::from_str(row.get::<String, _>("payload_json").as_str())
            .expect("parse broadcast payload");
        assert_eq!(payload["delivery_scope"], Value::from("broadcast"));
        assert_eq!(payload["mention_actor_ids"], Value::Array(Vec::new()));
        assert_eq!(payload["mentioned_actor_ids"], Value::Array(Vec::new()));
        assert!(
            payload
                .get("correlation_id")
                .and_then(Value::as_str)
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
        );
        assert_eq!(payload["source_kind"], json!("human"));
        assert_eq!(payload["source_surface"], json!("conversation"));
        assert_eq!(payload["requires_user_visible_reply"], json!(true));
    }
}

#[tokio::test]
async fn team_task_messages_api_infers_direct_route_for_single_mention_and_normalizes_detail_ref() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "task-direct-default-team".to_string(),
            description: Some("single mention should default to direct mailbox".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"},
                    {"member_id":"worker-2","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let task_created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Direct by default".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({})),
            conversation_mode: Some("group_chat".to_string()),
            topic: None,
        },
    )
    .await
    .expect("create task");

    let run = state
        .teams
        .create_run(
            &team.id,
            Some(task_created.task.id.as_str()),
            json!({
                "task_id": task_created.task.id.clone(),
                "conversation_id": task_created.conversation.id.clone(),
            }),
        )
        .await
        .expect("create explicit task run");

    let Json(message) = send_team_task_message(
        State(state.clone()),
        headers,
        Path((team.id.clone(), task_created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: None,
            to_actor_id: None,
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"<at>worker-1</at> review the concise summary first",
                "detail_ref":"artifact://task-direct-default/full-review-1"
            }),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send inferred direct message");

    assert_eq!(message.route, "to_member");
    assert_eq!(message.to_actor_id.as_deref(), Some("worker-1"));
    assert_eq!(
        message.payload["summary"],
        json!("<at>worker-1</at> review the concise summary first")
    );
    assert_eq!(
        message.payload["detail_ref"]["uri"],
        json!("artifact://task-direct-default/full-review-1")
    );

    let rows = sqlx::query(
        r#"
        SELECT from_actor_id, to_actor_id, payload_json
        FROM team_actor_messages
        WHERE run_id = ?1
        ORDER BY id ASC
        "#,
    )
    .bind(&run.id)
    .fetch_all(&state.db)
    .await
    .expect("load inferred direct mailbox rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<String, _>("from_actor_id"), "planner");
    assert_eq!(rows[0].get::<String, _>("to_actor_id"), "worker-1");

    let payload: Value = serde_json::from_str(rows[0].get::<String, _>("payload_json").as_str())
        .expect("parse inferred direct payload");
    assert_eq!(payload["delivery_scope"], json!("direct"));
    assert_eq!(
        payload["summary"],
        json!("<at>worker-1</at> review the concise summary first")
    );
    assert_eq!(
        payload["detail_ref"]["uri"],
        json!("artifact://task-direct-default/full-review-1")
    );
    assert_eq!(payload["mention_actor_ids"], json!(["worker-1"]));
    assert_eq!(payload["source_kind"], json!("human"));
    assert_eq!(payload["source_surface"], json!("conversation"));
    assert_eq!(payload["requires_user_visible_reply"], json!(true));
    assert_eq!(
        payload["reply_target"],
        json!({
            "surface":"conversation",
            "task_id": task_created.task.id,
            "conversation_id": message.conversation_id,
            "task_message_id": message.message_id
        })
    );
}

#[tokio::test]
async fn team_task_messages_api_infers_to_coordinator_from_single_coordinator_mention() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "task-to-coordinator-default-team".to_string(),
            description: Some("single coordinator mention should infer to_coordinator".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Coordinator inference".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({})),
            conversation_mode: Some("group_chat".to_string()),
            topic: None,
        },
    )
    .await
    .expect("create task");

    let Json(message) = send_team_task_message(
        State(state),
        headers,
        Path((team.id, created.task.id)),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("worker-1".to_string()),
            to_actor_id: None,
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"<at>planner</at> please review the latest patch"
            }),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send inferred to_coordinator message");

    assert_eq!(message.route, "to_coordinator");
    assert_eq!(message.to_actor_id.as_deref(), Some("planner"));
}

#[tokio::test]
async fn team_task_messages_api_normalizes_detail_ref_objects_and_caps_summary_length() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "detail-ref-object-team".to_string(),
            description: Some("detail_ref object normalization coverage".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner","role":"coordinator"}]
            }),
        }),
    )
    .await
    .expect("create team");

    let created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Object detail_ref normalization".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({})),
            conversation_mode: Some("group_chat".to_string()),
            topic: None,
        },
    )
    .await
    .expect("create task");

    let long_text = format!("Summary {}", "x".repeat(400));
    let Json(message) = send_team_task_message(
        State(state),
        headers,
        Path((team.id, created.task.id)),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("planner".to_string()),
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({
                "type":"chat_message",
                "text": long_text,
                "detail_ref": {
                    "uri":" artifact://team/detail-1 ",
                    "label":" full evidence ",
                    "kind":" artifact ",
                    "content_type":" application/json ",
                    "ignored":["nested"]
                }
            }),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send normalized detail_ref message");

    let summary = message.payload["summary"]
        .as_str()
        .expect("summary should be injected");
    assert_eq!(summary.chars().count(), 240);
    assert!(summary.ends_with("..."));
    assert_eq!(
        message.payload["detail_ref"],
        json!({
            "uri":"artifact://team/detail-1",
            "label":"full evidence",
            "kind":"artifact",
            "content_type":"application/json"
        })
    );
}

#[tokio::test]
async fn team_task_messages_api_drops_invalid_detail_ref_objects_before_summary_injection() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "invalid-detail-ref-object-team".to_string(),
            description: Some("invalid detail_ref object coverage".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner","role":"coordinator"}]
            }),
        }),
    )
    .await
    .expect("create team");

    let created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Invalid detail_ref object".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({})),
            conversation_mode: Some("group_chat".to_string()),
            topic: None,
        },
    )
    .await
    .expect("create task");

    let Json(message) = send_team_task_message(
        State(state),
        headers,
        Path((team.id, created.task.id)),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("planner".to_string()),
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({
                "type":"chat_message",
                "text":"short evidence summary",
                "detail_ref": {
                    "uri":"   ",
                    "label":" full evidence "
                }
            }),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send invalid detail_ref message");

    let payload = message
        .payload
        .as_object()
        .expect("message payload should remain an object");
    assert!(!payload.contains_key("detail_ref"));
    assert!(!payload.contains_key("summary"));
}

#[tokio::test]
async fn team_task_compile_preview_builds_deterministic_role_bound_payload() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "task-compile-team".to_string(),
            description: Some("task compile preview coverage".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-dev","role":"worker"},
                    {"member_id":"qa-review","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Implement chat-first compile".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({
                "task_list":["Bootstrap compile endpoint"],
                "acceptance_criteria":["Compile preview API returns deterministic payload"]
            })),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("planning".to_string()),
        },
    )
    .await
    .expect("create task");

    let Json(plan_update_one) = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("planner".to_string()),
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({
                "plan_update":{
                    "task_list":["Implement compile endpoint","Add API tests"],
                    "acceptance_criteria":["All team task API tests pass","Route-level contract is covered"],
                    "deadline":"2026-03-05"
                }
            }),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send first plan update");

    let _ = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("worker-dev".to_string()),
            to_actor_id: Some("planner".to_string()),
            route: Some("to_coordinator".to_string()),
            payload: json!({"text":"working on compile details"}),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send non-plan message");

    let Json(plan_update_two) = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("planner".to_string()),
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({
                "acceptance":[
                    "All team task API tests pass",
                    "Route-level contract is covered",
                    "Feature note and todo are updated"
                ]
            }),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send second plan update");

    let Json(preview_a) = compile_team_task_run_preview(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(CompileTeamTaskRunPreviewRequest { context_id: None }),
    )
    .await
    .expect("compile preview first pass");
    let Json(preview_b) = compile_team_task_run_preview(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(CompileTeamTaskRunPreviewRequest { context_id: None }),
    )
    .await
    .expect("compile preview second pass");

    assert_eq!(preview_a, preview_b);
    assert_eq!(preview_a.task_id, created.task.id);
    assert_eq!(preview_a.conversation_id, created.conversation.id);
    assert_eq!(preview_a.run_payload.context_id, created.task.id);
    assert_eq!(
        preview_a.run_payload.input["task_compile_version"],
        Value::from(1)
    );
    assert_eq!(
        preview_a.run_payload.input["task_id"],
        Value::from(created.task.id.clone())
    );
    assert_eq!(
        preview_a.run_payload.input["conversation_id"],
        Value::from(created.conversation.id.clone())
    );
    assert_eq!(
        preview_a.plan.task_list,
        vec![
            "Implement compile endpoint".to_string(),
            "Add API tests".to_string(),
        ]
    );
    assert_eq!(
        preview_a.plan.acceptance_criteria,
        vec![
            "All team task API tests pass".to_string(),
            "Route-level contract is covered".to_string(),
            "Feature note and todo are updated".to_string(),
        ]
    );
    assert_eq!(preview_a.plan.deadline.as_deref(), Some("2026-03-05"));
    assert_eq!(
        preview_a.plan.source_message_id,
        Some(plan_update_two.message_id)
    );
    assert!(plan_update_one.message_id < plan_update_two.message_id);
    assert_eq!(preview_a.plan.step_template.len(), 4);

    let planner_assignment = preview_a
        .plan
        .role_assignments
        .iter()
        .find(|item| item.member_id == "planner")
        .expect("find planner assignment");
    assert_eq!(planner_assignment.role, "coordinator");
    assert_eq!(
        planner_assignment.step_keys,
        vec!["coordinator_plan".to_string(), "coordinator_synthesize".to_string(),]
    );

    let dev_assignment = preview_a
        .plan
        .role_assignments
        .iter()
        .find(|item| item.member_id == "worker-dev")
        .expect("find worker-dev assignment");
    assert_eq!(dev_assignment.role, "worker");
    assert_eq!(
        dev_assignment.step_keys,
        vec!["worker_1_worker_dev".to_string()]
    );

    let qa_assignment = preview_a
        .plan
        .role_assignments
        .iter()
        .find(|item| item.member_id == "qa-review")
        .expect("find qa-review assignment");
    assert_eq!(qa_assignment.role, "worker");
    assert_eq!(
        qa_assignment.step_keys,
        vec!["worker_2_qa_review".to_string()]
    );
}

#[tokio::test]
async fn team_task_compile_preview_sanitizes_plan_updates() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "task-compile-sanitize-team".to_string(),
            description: Some("task compile sanitize coverage".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner","role":"coordinator"}]
            }),
        }),
    )
    .await
    .expect("create team");

    let created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Sanitize compile updates".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({})),
            conversation_mode: Some("group_chat".to_string()),
            topic: None,
        },
    )
    .await
    .expect("create task");

    let long_text = "x".repeat(500);
    let Json(_) = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("planner".to_string()),
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({
                "plan_update": {
                    "task_list": [
                        "  normalize   spaces  ",
                        "`rm -rf /` {inject}",
                        long_text
                    ],
                    "acceptance_criteria": [
                        "keep-safe ✅",
                        "keep-safe ✅",
                        "check {unsafe} symbols"
                    ],
                    "deadline": "2026-99-77"
                }
            }),
            idempotency_key: None,
        }),
    )
    .await
    .expect("send sanitize patch");

    let Json(preview) = compile_team_task_run_preview(
        State(state),
        headers,
        Path((team.id, created.task.id)),
        Json(CompileTeamTaskRunPreviewRequest { context_id: None }),
    )
    .await
    .expect("compile sanitized preview");

    assert_eq!(preview.plan.deadline, None);
    assert_eq!(
        preview.plan.task_list.first().map(String::as_str),
        Some("normalize spaces")
    );
    assert!(
        preview
            .plan
            .task_list
            .iter()
            .all(|item| item.len() <= 280 && !item.contains('`') && !item.contains('{'))
    );
    assert_eq!(preview.plan.acceptance_criteria.len(), 2);
    assert!(
        preview
            .plan
            .acceptance_criteria
            .iter()
            .all(|item| !item.contains('{') && !item.contains('`'))
    );
}

#[tokio::test]
async fn team_task_compile_preview_prefers_task_execution_plan_steps() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "task-execution-plan-preview-team".to_string(),
            description: Some("task execution plan preview coverage".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-dev","role":"worker"},
                    {"member_id":"qa-review","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Use task execution plan".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({
                "execution_plan": {
                    "steps": [
                        {
                            "step_key":"design-plan",
                            "member_id":"planner",
                            "goal":"produce an implementation plan",
                            "acceptance":["plan is reviewable"],
                            "execution":{"mode":"reconcile_loop","max_rounds":3}
                        },
                        {
                            "step_key":"implement-worker",
                            "member_id":"worker-dev",
                            "depends_on":["design-plan"],
                            "goal":"   ",
                            "execution":{"mode":"single_pass"}
                        },
                        {
                            "step_key":"qa-review",
                            "member_id":"qa-review",
                            "depends_on":["implement-worker"],
                            "execution":{"mode":"single_pass"}
                        }
                    ]
                }
            })),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("execution-plan".to_string()),
        },
    )
    .await
    .expect("create task");

    let Json(preview) = compile_team_task_run_preview(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(CompileTeamTaskRunPreviewRequest { context_id: None }),
    )
    .await
    .expect("compile preview");

    let step_keys = preview
        .plan
        .step_template
        .iter()
        .map(|step| step.step_key.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        step_keys,
        vec![
            "design-plan".to_string(),
            "implement-worker".to_string(),
            "qa-review".to_string()
        ]
    );
    assert_eq!(preview.plan.step_template.len(), 3);
    assert_eq!(
        preview.plan.step_template[0].goal.as_deref(),
        Some("produce an implementation plan")
    );
    assert_eq!(
        preview.plan.step_template[0].acceptance,
        vec!["plan is reviewable".to_string()]
    );
    assert_eq!(preview.plan.step_template[1].goal, None);
    assert_eq!(
        preview.plan.step_template[0].execution,
        crate::team::TeamTaskStepExecutionSpec {
            mode: crate::team::TeamTaskStepExecutionMode::ReconcileLoop,
            max_rounds: Some(3),
        }
    );
    assert_eq!(
        preview.run_payload.input["step_template"][0]["execution"],
        json!({"mode":"reconcile_loop","max_rounds":3})
    );
}

#[tokio::test]
async fn team_task_compile_preview_rejects_invalid_execution_plan_payload() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "task-execution-plan-invalid-preview-team".to_string(),
            description: Some("invalid execution plan preview coverage".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner","role":"coordinator"}]
            }),
        }),
    )
    .await
    .expect("create team");

    let created = create_team_task(
        &state,
        &headers,
        &team.id,
        CreateTeamTaskRequest {
            title: "Use invalid task execution plan".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({
                "execution_plan": {
                    "steps": [
                        {
                            "step_key": "plan",
                            "member_id": "planner",
                            "goal": "produce a run plan",
                            "acceptance": ["plan is available"],
                            "execution": {"mode": "single_pass"}
                        }
                    ]
                }
            })),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("execution-plan-invalid".to_string()),
        },
    )
    .await
    .expect("create task");

    sqlx::query("UPDATE team_tasks SET context_json = ?1 WHERE id = ?2")
        .bind(json!({
            "execution_plan": {
                "steps": "invalid"
            }
        }))
        .bind(&created.task.id)
        .execute(&state.db)
        .await
        .expect("corrupt task execution plan context");

    let err = compile_team_task_run_preview(
        State(state),
        headers,
        Path((team.id, created.task.id)),
        Json(CompileTeamTaskRunPreviewRequest { context_id: None }),
    )
    .await
    .expect_err("invalid execution plan should fail with bad request");
    let body = decode_json_body(err.into_response()).await;
    assert_eq!(
        body["error"],
        Value::from("task context contains an invalid execution_plan")
    );
}

#[test]
fn mailbox_type_hint_helpers_build_prompt_contains_context() {
    let prompt =
        super::build_actor_mailbox_immediate_hint_prompt_for_test("run-42", "direct_agent_message");
    assert!(prompt.contains("run-42"));
    assert!(prompt.contains("Direct mailbox message pending"));
    assert!(prompt.contains("agenthub actor inbox"));
    assert!(prompt.contains("actor inbox"));
}
