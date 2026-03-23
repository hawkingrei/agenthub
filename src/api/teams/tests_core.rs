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
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"leader"}]}),
        }),
    )
    .await
    .expect("create team");
    assert_eq!(created.spec["spec_version"], Value::from(1));
    assert_eq!(created.spec["leader_member_id"], Value::from("planner"));
    assert_eq!(created.spec["entrypoint"], Value::from("leader_plan"));
    assert_eq!(
        created.spec["steps"][0]["step_key"],
        Value::from("leader_plan")
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
        created.spec["members"][0]["skills"]
            .as_array()
            .is_some_and(|skills| !skills.is_empty())
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
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"leader"}]}),
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
    assert!(created.spec.get("leader_member_id").is_none());
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
                        "role": "leader",
                    }
                ],
            }),
        }),
    )
    .await
    .expect("update team spec");

    assert_eq!(updated.spec["leader_member_id"], Value::from("planner"));
    assert_eq!(updated.spec["entrypoint"], Value::from("leader_plan"));
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

    let Json(task_detail) = create_team_task(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamTaskRequest {
            title: "Investigate".to_string(),
            created_by_actor_id: None,
            context: None,
            conversation_mode: None,
            topic: None,
        }),
    )
    .await
    .expect("create task without members");

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

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "delete-team".to_string(),
            description: Some("delete target".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"leader"}]}),
        }),
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
        VALUES (?1, 'leader', 'worker', 'default', 'local', NULL, '{}', NULL, 'pending', ?2)
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

    let member_event_db = state
        .agents
        .test_event_pool_for_agent(member_agent_id)
        .await
        .expect("reopen member event db");
    let event_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM agent_events WHERE session_id = ?1")
            .bind(&session_id)
            .fetch_one(&member_event_db)
            .await
            .expect("count member events");
    assert_eq!(event_count, 0);

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
                    {"member_id":"planner","role":"leader"},
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
async fn team_member_runtime_startup_supports_leader_and_worker_roles() {
    let state = build_test_state().await;
    configure_worker_team_member_agent(&state, "reviewer").await;
    let actor_cli_path = default_actor_cli_path().expect("resolve actor cli path");

    let planner_session = state
        .agents
        .start_agent_with_actor_context(
            "planner",
            Some(AcpActorSkillContext {
                team_id: Some("team-runtime-startup".to_string()),
                current_run_id: None,
                actor_id: "planner".to_string(),
                default_channel: "default".to_string(),
                actor_cli_path: actor_cli_path.clone(),
                member_role: Some("leader".to_string()),
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
                actor_cli_path,
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
                    {"member_id":"planner","role":"leader"},
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
                        {"member_id":"planner","role":"leader"},
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
                        {"member_id":"planner","role":"leader"},
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
            role: "leader".to_string(),
            model: None,
            description: None,
            skills: vec!["team-leader-orchestrator".to_string()],
            prompt: None,
        },
    )
    .expect("expected team member actor context");

    let mismatched = AcpActorSkillContext {
        team_id: Some("other-team".to_string()),
        current_run_id: None,
        actor_id: "planner".to_string(),
        default_channel: "default".to_string(),
        actor_cli_path: default_actor_cli_path().expect("actor cli path"),
        member_role: Some("leader".to_string()),
        member_skills: vec!["team-leader-orchestrator".to_string()],
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
async fn teams_api_enforces_required_role_skills() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(created) = create_team(
        State(state),
        headers,
        Json(CreateTeamRequest {
            name: "required-role-skills-team".to_string(),
            description: Some("role skill enforcement".to_string()),
            spec: json!({
                "entrypoint":"leader-agent",
                "leader_member_id":"leader-agent",
                "members":[
                    {
                        "member_id":"leader-agent",
                        "role":"leader",
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

    let resolve_skills = |member_id: &str| -> Vec<String> {
        members
            .iter()
            .find(|member| {
                member
                    .get("member_id")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value == member_id)
            })
            .and_then(|member| member.get("skills"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect::<Vec<_>>()
    };

    let leader_skills = resolve_skills("leader-agent");
    assert!(
        leader_skills
            .iter()
            .any(|item| item == "agenthub-actor-runtime")
    );
    assert!(
        leader_skills
            .iter()
            .any(|item| item == "team-leader-orchestrator")
    );
    assert!(leader_skills.iter().any(|item| item == "planning"));
    assert!(
        !leader_skills
            .iter()
            .any(|item| item == "team-worker-executor")
    );

    let worker_skills = resolve_skills("worker-agent");
    assert!(
        worker_skills
            .iter()
            .any(|item| item == "agenthub-actor-runtime")
    );
    assert!(
        worker_skills
            .iter()
            .any(|item| item == "team-worker-executor")
    );
    assert!(worker_skills.iter().any(|item| item == "coding"));
    assert!(
        !worker_skills
            .iter()
            .any(|item| item == "team-leader-orchestrator")
    );
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
                "entrypoint":"leader-agent",
                "leader_member_id":"leader-agent",
                "members":[
                    {"member_id":"leader-agent","role":"leader"},
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
    let leader_prompt = members
        .iter()
        .find(|member| member.get("member_id").and_then(Value::as_str) == Some("leader-agent"))
        .and_then(|member| member.get("prompt"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(leader_prompt.contains("Do not implement feature code directly."));
    assert!(leader_prompt.contains("perform targeted technical research"));
    assert!(leader_prompt.contains("Start from an empty workspace."));

    let worker_prompt = members
        .iter()
        .find(|member| member.get("member_id").and_then(Value::as_str) == Some("worker-agent"))
        .and_then(|member| member.get("prompt"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(worker_prompt.contains("Work in your own git worktree only."));
    assert!(worker_prompt.contains("Create a random branch at start"));
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
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(created) = create_team(
        State(state),
        headers,
        Json(CreateTeamRequest {
            name: "default-steps-team".to_string(),
            description: None,
            spec: json!({
                "entrypoint":"leader-agent",
                "members":[
                    {"member_id":"leader-agent","role":"leader"},
                    {"member_id":"worker-agent-a","role":"worker"},
                    {"member_id":"worker-agent-b","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team with generated defaults");

    assert_eq!(created.spec["entrypoint"], Value::from("leader_plan"));
    let steps = created.spec["steps"]
        .as_array()
        .expect("generated steps array");
    assert_eq!(steps.len(), 4);
    assert_eq!(steps[0]["step_key"], Value::from("leader_plan"));
    assert_eq!(steps[0]["member_id"], Value::from("leader-agent"));
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
        .find(|step| step.get("step_key").and_then(Value::as_str) == Some("leader_synthesize"))
        .expect("leader_synthesize step");
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
        json!({"entrypoint":"planner","leader_member_id":"missing","members":[{"member_id":"planner"}]}),
        json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"captain"}]}),
        json!({"entrypoint":"planner","members":[{"member_id":"planner","skills":["a","a"]}]}),
        json!({"entrypoint":"planner","leader_member_id":"leader","members":[{"member_id":"planner"},{"member_id":"leader"}]}),
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
                "members": [{"member_id":"planner","role":"leader"}],
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
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"leader"}]}),
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
            spec: json!({"entrypoint":"executor","members":[{"member_id":"executor","role":"leader"}]}),
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
            spec: json!({"entrypoint":"executor","members":[{"member_id":"executor","role":"leader"}]}),
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
            spec: json!({"entrypoint":"executor","members":[{"member_id":"executor","role":"leader"}]}),
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
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"leader"}]}),
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
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"leader"}]}),
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
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"leader"}]}),
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
                "members":[{"member_id":"planner","role":"leader"}]
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
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"leader"}]}),
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
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"leader"}]}),
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
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"leader"}]}),
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
        Some(json!({"question":"approve?"}))
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
    assert_eq!(resumed_step.input, Some(json!({"answer":"approved"})));

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
                "members":[{"member_id":"planner","role":"leader"},{"member_id":"reviewer","role":"worker"}]
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
async fn team_run_messages_api_supports_human_actor_auto_load_and_ack_fallback() {
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
                "members":[{"member_id":"planner","role":"leader"}]
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
            .all(|message| message.status == crate::team::TeamActorMessageStatus::Delivered)
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
                "members":[{"member_id":"planner","role":"leader"},{"member_id":"reviewer","role":"worker"}]
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
                    {"member_id":"planner","role":"leader"},
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
    assert_eq!(first_hint.payload["status"], json!("sent"));
    assert_eq!(
        first_hint.payload["reason"],
        json!("direct_agent_message")
    );
    assert_eq!(first_hint.payload["target_actor_ids"], json!(["reviewer"]));

    let second_hint = hint_events
        .iter()
        .find(|event| event.payload["message_id"] == json!(second_chat.message_id))
        .expect("second chat hint event");
    assert_eq!(second_hint.payload["status"], json!("sent"));
    assert_eq!(
        second_hint.payload["reason"],
        json!("direct_agent_message")
    );
    assert_eq!(second_hint.payload["target_actor_ids"], json!(["reviewer"]));

    let worker_status_hint = hint_events
        .iter()
        .find(|event| event.payload["message_id"] == json!(worker_status.message_id))
        .expect("worker status hint event");
    assert_eq!(worker_status_hint.payload["status"], json!("sent"));
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
    assert_eq!(repeated_worker_status_hint.payload["status"], json!("sent"));
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
                "leader_member_id":"planner",
                "members":[
                    {
                        "member_id":"planner",
                        "role":"leader",
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
        "description":"Lead planner and review owner.",
        "skills_add":["risk-analysis","planning"]
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
    let planner_skills = planner
        .get("skills")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    assert!(planner_skills.iter().any(|skill| skill == "planning"));
    assert!(planner_skills.iter().any(|skill| skill == "risk-analysis"));

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
                "entrypoint":"leader-agent",
                "leader_member_id":"leader-agent",
                "members":[
                    {
                        "member_id":"leader-agent",
                        "role":"leader",
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
            from_actor_id: "leader-agent".to_string(),
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
                "description":"Focused implementation specialist.",
                "skills_add":["actor-mailbox"]
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
        updated_run.input["profile_overrides"]["members"]["worker-agent"]["skills_add"][0],
        Value::from("actor-mailbox")
    );
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
    assert!(worker_snapshot.skills.iter().any(|skill| skill == "coding"));
    assert!(
        worker_snapshot
            .skills
            .iter()
            .any(|skill| skill == "actor-mailbox")
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
                "entrypoint":"leader-agent",
                "leader_member_id":"leader-agent",
                "members":[
                    {
                        "member_id":"leader-agent",
                        "role":"leader",
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
            member_id: "leader-agent".to_string(),
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
            runtime_handle_id: Some("session-leader-1".to_string()),
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
    .bind("leader-agent")
    .bind("leader-agent")
    .bind("/tmp")
    .bind("/usr/bin/env")
    .bind("[]")
    .bind("use_existing")
    .bind("running")
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert leader agent row");

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
    .bind("session-leader-1")
    .bind("leader-agent")
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
            from_actor_id: "leader-agent".to_string(),
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
    assert_eq!(snapshot.leader_member_id.as_deref(), Some("leader-agent"));
    assert_eq!(snapshot.members.len(), 2);
    assert!(snapshot.latest_events.len() >= 3);
    assert_eq!(snapshot.mailbox.pending, 1);
    assert_eq!(snapshot.mailbox.delivered, 0);
    assert_eq!(snapshot.mailbox.dead_letter, 0);
    assert_eq!(snapshot.mailbox.recent_messages.len(), 1);

    let leader = snapshot
        .members
        .iter()
        .find(|member| member.member_id == "leader-agent")
        .expect("find leader");
    assert_eq!(leader.role, "leader");
    assert_eq!(
        leader.description.as_deref(),
        Some("Team architect and integration owner")
    );
    assert_eq!(leader.model.as_deref(), Some("gpt-5"));
    assert_eq!(leader.prompt.as_deref(), Some("Lead the plan"));
    assert_eq!(
        leader.skills,
        vec![
            "agenthub-actor-runtime".to_string(),
            "team-agents-index".to_string(),
            "team-task-lifecycle".to_string(),
            "team-leader-orchestrator".to_string(),
            "team-actor-mailbox".to_string(),
            "planning".to_string(),
            "review".to_string()
        ]
    );
    assert_eq!(leader.pending_inbox_count, 0);
    assert_eq!(leader.status, "working");
    assert_eq!(leader.session_status.as_deref(), Some("working"));
    assert_eq!(
        leader
            .latest_step
            .as_ref()
            .and_then(|step| step.runtime_handle_id.as_deref()),
        Some("session-leader-1")
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
    assert_eq!(worker.session_status.as_deref(), Some("running"));
    assert!(worker.latest_step.is_none());
}

#[tokio::test]
async fn team_task_api_creates_lists_and_redacts_context() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "task-api-team".to_string(),
            description: Some("task api coverage".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"leader"}]}),
        }),
    )
    .await
    .expect("create team");

    let Json(created) = create_team_task(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamTaskRequest {
            title: "Kickoff migration".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({
                "source":"ui",
                "token":"do-not-store",
                "nested":{"secret":"x"}
            })),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("kickoff".to_string()),
        }),
    )
    .await
    .expect("create task");
    assert_eq!(created.task.team_id, team.id);
    assert_eq!(created.task.title, "Kickoff migration");
    assert!(created.task.created_by_actor_id.starts_with("user:"));
    assert_eq!(created.task.assigned_member_id, None);
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
        Query(ListTeamTasksQuery { limit: Some(20) }),
    )
    .await
    .expect("list tasks");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.task.id);
    assert_eq!(listed[0].assigned_member_id, None);

    let Json(found) = get_team_task(
        State(state),
        headers,
        Path((team.id, created.task.id.clone())),
    )
    .await
    .expect("get task");
    assert_eq!(found.task.id, created.task.id);
    assert_eq!(found.conversation.id, created.conversation.id);
    assert_eq!(found.task.assigned_member_id, None);
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
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"leader"}]}),
        }),
    )
    .await
    .expect("create team");

    let Json(created) = create_team_task(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamTaskRequest {
            title: "All".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({
                "bootstrap_kind":"shared_thread",
                "bootstrap_source":"teams_all"
            })),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("all".to_string()),
        }),
    )
    .await
    .expect("create shared thread task");

    assert_eq!(created.task.status, crate::team::TeamTaskStatus::Open);
    assert!(created.latest_run.is_none());
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
                    {"member_id":"planner","role":"leader"},
                    {"member_id":"worker-1","role":"worker"},
                    {"member_id":"worker-2","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(task_created) = create_team_task(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamTaskRequest {
            title: "All".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({
                "bootstrap_kind":"shared_thread",
                "bootstrap_source":"teams_all"
            })),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("all".to_string()),
        }),
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
        assert_eq!(forwarded_payload["delivery_scope"], Value::from("broadcast"));
        assert_eq!(
            forwarded_payload["task_message_id"],
            Value::from(directed_message.message_id)
        );
        assert_eq!(
            forwarded_payload["task_conversation_id"],
            Value::from(directed_message.conversation_id.clone())
        );
        assert_eq!(
            forwarded_payload["mention_actor_ids"],
            json!(["worker-1"])
        );
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
async fn teams_api_updates_task_status_and_rejects_invalid_values() {
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
                "members":[{"member_id":"planner","role":"leader"}]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(created) = create_team_task(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamTaskRequest {
            title: "Promote kanban card".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({"source":"ui"})),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("status".to_string()),
        }),
    )
    .await
    .expect("create task");

    let Json(updated) = update_team_task(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(UpdateTeamTaskRequest {
            status: "in_progress".to_string(),
        }),
    )
    .await
    .expect("update task status");
    assert_eq!(updated.id, created.task.id);
    assert_eq!(updated.status, crate::team::TeamTaskStatus::InProgress);

    let Json(reviewing) = update_team_task(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(UpdateTeamTaskRequest {
            status: "in_review".to_string(),
        }),
    )
    .await
    .expect("move task into review");
    assert_eq!(reviewing.status, crate::team::TeamTaskStatus::InReview);

    let err = update_team_task(
        State(state),
        headers,
        Path((team.id, created.task.id)),
        Json(UpdateTeamTaskRequest {
            status: "paused".to_string(),
        }),
    )
    .await
    .expect_err("invalid status should fail");
    let body = decode_json_body(err.into_response()).await;
    assert_eq!(
        body["error"],
        Value::from("status must be one of: open, in_progress, in_review, completed, canceled")
    );
}

#[tokio::test]
async fn team_task_api_enforces_team_owner_access() {
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
                "members":[{"member_id":"planner","role":"leader"}]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(created) = create_team_task(
        State(state.clone()),
        owner_headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamTaskRequest {
            title: "Owner only planning".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({})),
            conversation_mode: Some("group_chat".to_string()),
            topic: None,
        }),
    )
    .await
    .expect("create task");

    let create_err = create_team_task(
        State(state.clone()),
        outsider_headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamTaskRequest {
            title: "Outsider task".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({})),
            conversation_mode: Some("group_chat".to_string()),
            topic: None,
        }),
    )
    .await
    .expect_err("outsider should not create task");
    assert_eq!(create_err.into_response().status(), StatusCode::NOT_FOUND);

    let send_err = send_team_task_message(
        State(state.clone()),
        outsider_headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("user".to_string()),
            to_actor_id: None,
            route: Some("group_chat".to_string()),
            payload: json!({"text":"malicious broadcast"}),
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
                    {"member_id":"planner","role":"leader"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(created) = create_team_task(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamTaskRequest {
            title: "Discuss rollout".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({})),
            conversation_mode: Some("group_chat".to_string()),
            topic: None,
        }),
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

    let Json(to_leader_message) = send_team_task_message(
        State(state.clone()),
        headers.clone(),
        Path((team.id.clone(), created.task.id.clone())),
        Json(SendTeamTaskMessageRequest {
            from_actor_id: Some("worker-1".to_string()),
            to_actor_id: None,
            route: Some("to_leader".to_string()),
            payload: json!({"text":"need clarification"}),
        }),
    )
    .await
    .expect("send to leader message");
    assert_eq!(to_leader_message.route, "to_leader");
    assert_eq!(to_leader_message.to_actor_id.as_deref(), Some("planner"));

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
    assert_eq!(messages[1].message_id, to_leader_message.message_id);
    assert_eq!(messages[2].message_id, group_message.message_id);
    assert_eq!(messages[0].route, "to_member");
    assert_eq!(messages[1].route, "to_leader");
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
async fn team_task_messages_api_forwards_human_chat_to_active_run_mailbox() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "task-mailbox-forward-team".to_string(),
            description: Some("task to mailbox forwarding coverage".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"leader"},
                    {"member_id":"worker-1","role":"worker"},
                    {"member_id":"worker-2","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(task_created) = create_team_task(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamTaskRequest {
            title: "Mailbox forwarding".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({})),
            conversation_mode: Some("group_chat".to_string()),
            topic: None,
        }),
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
    }
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
                    {"member_id":"planner","role":"leader"},
                    {"member_id":"worker-dev","role":"worker"},
                    {"member_id":"qa-review","role":"worker"}
                ]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(created) = create_team_task(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamTaskRequest {
            title: "Implement chat-first compile".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({
                "task_list":["Bootstrap compile endpoint"],
                "acceptance_criteria":["Compile preview API returns deterministic payload"]
            })),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("planning".to_string()),
        }),
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
            route: Some("to_leader".to_string()),
            payload: json!({"text":"working on compile details"}),
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
    assert_eq!(planner_assignment.role, "leader");
    assert_eq!(
        planner_assignment.step_keys,
        vec!["leader_plan".to_string(), "leader_synthesize".to_string(),]
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
                "members":[{"member_id":"planner","role":"leader"}]
            }),
        }),
    )
    .await
    .expect("create team");

    let Json(created) = create_team_task(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamTaskRequest {
            title: "Sanitize compile updates".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({})),
            conversation_mode: Some("group_chat".to_string()),
            topic: None,
        }),
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

#[test]
fn mailbox_type_hint_helpers_build_prompt_contains_context() {
    let prompt =
        super::build_actor_mailbox_immediate_hint_prompt_for_test("run-42", "direct_agent_message");
    assert!(prompt.contains("run-42"));
    assert!(prompt.contains("Direct mailbox message pending"));
    assert!(prompt.contains("agenthub actor inbox"));
    assert!(prompt.contains("actor inbox"));
}
