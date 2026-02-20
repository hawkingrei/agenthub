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
async fn teams_api_delete_team_cascades_related_run_data() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;
    let now = Utc::now().timestamp();
    let member_agent_id = "planner";

    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref,
            code_mode, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9)
        "#,
    )
    .bind(member_agent_id)
    .bind("planner-agent")
    .bind("/tmp")
    .bind("/bin/sh")
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

    let Json(deleted) = delete_team(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
    )
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

    let session_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_sessions WHERE agent_id = ?1",
    )
    .bind(member_agent_id)
    .fetch_one(&state.db)
    .await
    .expect("count member sessions");
    assert_eq!(session_count, 0);
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
    assert!(leader_skills.iter().any(|item| item == "agenthub-actor-runtime"));
    assert!(leader_skills
        .iter()
        .any(|item| item == "team-leader-orchestrator"));
    assert!(leader_skills.iter().any(|item| item == "planning"));
    assert!(!leader_skills
        .iter()
        .any(|item| item == "team-worker-executor"));

    let worker_skills = resolve_skills("worker-agent");
    assert!(worker_skills.iter().any(|item| item == "agenthub-actor-runtime"));
    assert!(worker_skills
        .iter()
        .any(|item| item == "team-worker-executor"));
    assert!(worker_skills.iter().any(|item| item == "coding"));
    assert!(!worker_skills
        .iter()
        .any(|item| item == "team-leader-orchestrator"));
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

    let err = delete_team(
        State(state),
        headers,
        Path("missing-team".to_string()),
    )
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
        json!({"entrypoint":"planner","members":[]}),
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
        State(state),
        headers,
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

    let completed_resume_err = resume_team_run(
        State(state),
        headers,
        Path(completed_run.id.clone()),
    )
    .await
    .expect_err("completed run should reject resume");
    assert_eq!(
        completed_resume_err.into_response().status(),
        StatusCode::CONFLICT
    );
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
            let _ = cancel_team_run(
                State(state.clone()),
                headers.clone(),
                Path(run.id.clone()),
            )
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
                page.iter()
                    .all(|run| run.created_at < before_created_at),
                "cursor filter should only return older runs"
            );
        }

        for pair in page.windows(2) {
            let current = &pair[0];
            let next = &pair[1];
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
        collected_ids.iter().collect::<std::collections::HashSet<_>>().len(),
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
        assert!(page.iter().all(|run| run.status == crate::team::TeamRunStatus::Canceled));
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
            remote_task_id: Some("remote-task-bridge".to_string()),
        }),
    )
    .await
    .expect("start step");
    assert_eq!(step_working.status, crate::team::TeamStepStatus::Working);
    assert_eq!(
        step_working.remote_task_id.as_deref(),
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
            "run_completed"
        ]
    );

    let missing_step_err = start_team_run_step(
        State(state.clone()),
        headers.clone(),
        Path((run.id.clone(), "missing-step".to_string())),
        Json(StartTeamRunStepRequest {
            remote_task_id: None,
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
            remote_task_id: Some("remote-task-fail".to_string()),
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
            remote_task_id: Some("remote-task-input-required".to_string()),
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
            to_actor_id: "reviewer".to_string(),
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
            to_actor_id: "remote-reviewer".to_string(),
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
            to_actor_id: "remote-reviewer-2".to_string(),
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
            to_actor_id: "unknown-local".to_string(),
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
            to_actor_id: "reviewer".to_string(),
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
            to_actor_id: "reviewer".to_string(),
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
            to_actor_id: "reviewer".to_string(),
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
            to_actor_id: "reviewer".to_string(),
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
                        "prompt":"Lead with checkpoints.",
                        "skills":["planning"]
                    },
                    {
                        "member_id":"reviewer",
                        "role":"worker",
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
        "skills_add":["risk-analysis","planning"]
    });

    let Json(_first_message) = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "planner".to_string(),
            to_actor_id: "reviewer".to_string(),
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
            to_actor_id: "reviewer".to_string(),
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
                        "prompt":"Lead the run.",
                        "skills":["planning"]
                    },
                    {
                        "member_id":"worker-agent",
                        "role":"worker",
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
            to_actor_id: "worker-agent".to_string(),
            channel: Some("coordination".to_string()),
            transport: Some("local".to_string()),
            route: None,
            payload: json!({
                "type":"profile_patch_proposal",
                "target":"run",
                "member_id":"worker-agent",
                "prompt_append":"Ask one clarification question before coding when requirements are incomplete.",
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
                        "model":"gpt-5",
                        "prompt":"Lead the plan",
                        "skills":["planning","review"]
                    },
                    {
                        "member_id":"worker-agent",
                        "role":"worker",
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
            remote_task_id: Some("session-leader-1".to_string()),
        }),
    )
    .await
    .expect("start snapshot step");

    let now = Utc::now().timestamp();
    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9)
        "#,
    )
    .bind("leader-agent")
    .bind("leader-agent")
    .bind("/tmp")
    .bind("/bin/sh")
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

    let Json(_message) = send_team_run_message(
        State(state.clone()),
        headers.clone(),
        Path(run.id.clone()),
        Json(SendTeamRunMessageRequest {
            from_actor_id: "leader-agent".to_string(),
            to_actor_id: "worker-agent".to_string(),
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
    assert_eq!(leader.model.as_deref(), Some("gpt-5"));
    assert_eq!(leader.prompt.as_deref(), Some("Lead the plan"));
    assert_eq!(
        leader.skills,
        vec![
            "agenthub-actor-runtime".to_string(),
            "team-leader-orchestrator".to_string(),
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
            .and_then(|step| step.remote_task_id.as_deref()),
        Some("session-leader-1")
    );

    let worker = snapshot
        .members
        .iter()
        .find(|member| member.member_id == "worker-agent")
        .expect("find worker");
    assert_eq!(worker.role, "worker");
    assert_eq!(worker.model.as_deref(), Some("gpt-4.1"));
    assert_eq!(worker.pending_inbox_count, 1);
    assert_eq!(worker.status, "idle");
    assert!(worker.latest_step.is_none());
}
