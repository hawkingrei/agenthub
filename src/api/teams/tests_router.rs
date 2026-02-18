#[tokio::test]
async fn teams_router_http_contract() {
    let state = build_test_state().await;
    let token = create_auth_token(&state).await;
    let app = super::router(state);

    let unauthorized = app
        .clone()
        .oneshot(build_json_request(Method::GET, "/", None, None))
        .await
        .expect("run unauthorized request");
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let invalid_spec_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            "/",
            Some(&token),
            Some(json!({
                "name": "invalid-router-team",
                "description": null,
                "spec": {"entrypoint":"planner","members":[]}
            })),
        ))
        .await
        .expect("create invalid team via router");
    assert_eq!(invalid_spec_resp.status(), StatusCode::BAD_REQUEST);

    let create_team_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            "/",
            Some(&token),
            Some(json!({
                "name": "router-team",
                "description": "router-level contract",
                "spec": {"entrypoint":"planner","members":[{"member_id":"planner","role":"leader"}]}
            })),
        ))
        .await
        .expect("create team via router");
    assert_eq!(create_team_resp.status(), StatusCode::OK);
    let created_team = decode_json_body(create_team_resp).await;
    let team_id = created_team["id"].as_str().expect("team id").to_string();
    assert_eq!(created_team["spec"]["spec_version"], Value::from(1));

    let list_teams_resp = app
        .clone()
        .oneshot(build_json_request(Method::GET, "/", Some(&token), None))
        .await
        .expect("list teams via router");
    assert_eq!(list_teams_resp.status(), StatusCode::OK);
    let listed = decode_json_body(list_teams_resp).await;
    let listed = listed.as_array().expect("teams array");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["id"], team_id);

    let duplicate_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            "/",
            Some(&token),
            Some(json!({
                "name": "router-team",
                "description": null,
                "spec": {"entrypoint":"planner","members":[{"member_id":"planner","role":"leader"}]}
            })),
        ))
        .await
        .expect("duplicate create");
    assert_eq!(duplicate_resp.status(), StatusCode::CONFLICT);

    let get_team_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/{team_id}"),
            Some(&token),
            None,
        ))
        .await
        .expect("get team via router");
    assert_eq!(get_team_resp.status(), StatusCode::OK);

    let create_run_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/runs"),
            Some(&token),
            Some(json!({
                "context_id": "ctx-router",
                "input": {"prompt":"review this run"}
            })),
        ))
        .await
        .expect("create run via router");
    assert_eq!(create_run_resp.status(), StatusCode::OK);
    let run = decode_json_body(create_run_resp).await;
    let run_id = run["id"].as_str().expect("run id").to_string();
    assert_eq!(run["status"], "submitted");

    let snapshot_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/runs/{run_id}/snapshot?event_limit=100&message_limit=100"),
            Some(&token),
            None,
        ))
        .await
        .expect("get run snapshot via router");
    assert_eq!(snapshot_resp.status(), StatusCode::OK);
    let snapshot = decode_json_body(snapshot_resp).await;
    assert_eq!(snapshot["run"]["id"], run_id);
    assert_eq!(snapshot["mailbox"]["pending"], Value::from(0));
    assert_eq!(snapshot["members"][0]["member_id"], "planner");

    let list_runs_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/{team_id}/runs?limit=100"),
            Some(&token),
            None,
        ))
        .await
        .expect("list runs via router");
    assert_eq!(list_runs_resp.status(), StatusCode::OK);
    let listed_runs = decode_json_body(list_runs_resp).await;
    let listed_runs = listed_runs.as_array().expect("runs array");
    assert_eq!(listed_runs.len(), 1);
    assert_eq!(listed_runs[0]["id"], run_id);

    let invalid_status_runs_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/{team_id}/runs?status=invalid"),
            Some(&token),
            None,
        ))
        .await
        .expect("invalid runs status request");
    assert_eq!(invalid_status_runs_resp.status(), StatusCode::BAD_REQUEST);

    let cancel_run_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{run_id}/cancel"),
            Some(&token),
            None,
        ))
        .await
        .expect("cancel run via router");
    assert_eq!(cancel_run_resp.status(), StatusCode::OK);
    let canceled = decode_json_body(cancel_run_resp).await;
    assert_eq!(canceled["status"], "canceled");

    let events_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/runs/{run_id}/events?limit=100"),
            Some(&token),
            None,
        ))
        .await
        .expect("list events via router");
    assert_eq!(events_resp.status(), StatusCode::OK);
    let events = decode_json_body(events_resp).await;
    let events = events.as_array().expect("events array");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["event_type"], "run_submitted");
    assert_eq!(events[1]["event_type"], "run_canceled");
    let first_id = events[0]["event_id"].as_i64().expect("first event id");
    let second_id = events[1]["event_id"].as_i64().expect("second event id");
    assert!(first_id < second_id);

    let paged_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/runs/{run_id}/events?limit=1&before_id={second_id}"),
            Some(&token),
            None,
        ))
        .await
        .expect("page events via router");
    assert_eq!(paged_resp.status(), StatusCode::OK);
    let paged = decode_json_body(paged_resp).await;
    let paged = paged.as_array().expect("paged events array");
    assert_eq!(paged.len(), 1);
    assert_eq!(paged[0]["event_type"], "run_submitted");

    let create_step_run_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/runs"),
            Some(&token),
            Some(json!({
                "context_id": "ctx-router-steps",
                "input": {"prompt":"run step lifecycle bridge"}
            })),
        ))
        .await
        .expect("create run for step bridge");
    assert_eq!(create_step_run_resp.status(), StatusCode::OK);
    let step_run = decode_json_body(create_step_run_resp).await;
    let step_run_id = step_run["id"].as_str().expect("step run id").to_string();

    let submit_step_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{step_run_id}/steps"),
            Some(&token),
            Some(json!({
                "step_key": "router-step",
                "member_id": "planner",
                "depends_on": [],
                "input": {"goal":"plan"}
            })),
        ))
        .await
        .expect("submit step via router");
    assert_eq!(submit_step_resp.status(), StatusCode::OK);
    let submitted_step = decode_json_body(submit_step_resp).await;
    let step_id = submitted_step["id"].as_str().expect("step id").to_string();
    assert_eq!(submitted_step["status"], "submitted");
    assert_eq!(submitted_step["step_key"], "router-step");
    assert_eq!(submitted_step["member_id"], "planner");

    let list_steps_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/runs/{step_run_id}/steps"),
            Some(&token),
            None,
        ))
        .await
        .expect("list steps via router");
    assert_eq!(list_steps_resp.status(), StatusCode::OK);
    let steps = decode_json_body(list_steps_resp).await;
    let steps = steps.as_array().expect("steps array");
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0]["status"], "submitted");

    let start_step_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{step_run_id}/steps/{step_id}/start"),
            Some(&token),
            Some(json!({"remote_task_id":"router-remote-task"})),
        ))
        .await
        .expect("start step via router");
    assert_eq!(start_step_resp.status(), StatusCode::OK);
    let started_step = decode_json_body(start_step_resp).await;
    assert_eq!(started_step["status"], "working");
    assert_eq!(started_step["remote_task_id"], "router-remote-task");

    let input_required_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{step_run_id}/steps/{step_id}/input_required"),
            Some(&token),
            Some(json!({
                "reason": "need approval",
                "input": {"question":"approve?"}
            })),
        ))
        .await
        .expect("set input required via router");
    assert_eq!(input_required_resp.status(), StatusCode::OK);
    let input_required_step = decode_json_body(input_required_resp).await;
    assert_eq!(input_required_step["status"], "input_required");
    assert_eq!(input_required_step["error_text"], "need approval");

    let invalid_input_required_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{step_run_id}/steps/{step_id}/input_required"),
            Some(&token),
            Some(json!({
                "reason": "   ",
                "input": null
            })),
        ))
        .await
        .expect("invalid input required request");
    assert_eq!(
        invalid_input_required_resp.status(),
        StatusCode::BAD_REQUEST
    );

    let run_after_input_required_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/runs/{step_run_id}"),
            Some(&token),
            None,
        ))
        .await
        .expect("get run after input required");
    assert_eq!(run_after_input_required_resp.status(), StatusCode::OK);
    let run_after_input_required = decode_json_body(run_after_input_required_resp).await;
    assert_eq!(run_after_input_required["status"], "input_required");

    let resume_step_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{step_run_id}/steps/{step_id}/resume"),
            Some(&token),
            Some(json!({"input":{"answer":"approved"}})),
        ))
        .await
        .expect("resume step via router");
    assert_eq!(resume_step_resp.status(), StatusCode::OK);
    let resumed_step = decode_json_body(resume_step_resp).await;
    assert_eq!(resumed_step["status"], "working");
    assert_eq!(resumed_step["input"]["answer"], "approved");

    let complete_step_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{step_run_id}/steps/{step_id}/complete"),
            Some(&token),
            Some(json!({"output":{"result":"ok"}})),
        ))
        .await
        .expect("complete step via router");
    assert_eq!(complete_step_resp.status(), StatusCode::OK);
    let completed_step = decode_json_body(complete_step_resp).await;
    assert_eq!(completed_step["status"], "completed");

    let get_step_run_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/runs/{step_run_id}"),
            Some(&token),
            None,
        ))
        .await
        .expect("get step run via router");
    assert_eq!(get_step_run_resp.status(), StatusCode::OK);
    let step_run_after_complete = decode_json_body(get_step_run_resp).await;
    assert_eq!(step_run_after_complete["status"], "completed");

    let create_message_run_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/runs"),
            Some(&token),
            Some(json!({"context_id":"ctx-router-msg","input":{"prompt":"msg flow"}})),
        ))
        .await
        .expect("create run for actor messages");
    assert_eq!(create_message_run_resp.status(), StatusCode::OK);
    let message_run = decode_json_body(create_message_run_resp).await;
    let message_run_id = message_run["id"]
        .as_str()
        .expect("message run id")
        .to_string();

    let send_local_message_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{message_run_id}/messages/send"),
            Some(&token),
            Some(json!({
                "from_actor_id":"planner",
                "to_actor_id":"planner",
                "channel":"coordination",
                "transport":"local",
                "route":null,
                "payload":{"text":"self-check"}
            })),
        ))
        .await
        .expect("send local actor message via router");
    assert_eq!(send_local_message_resp.status(), StatusCode::OK);
    let local_message = decode_json_body(send_local_message_resp).await;
    let local_message_id = local_message["message_id"]
        .as_i64()
        .expect("local message id");
    assert_eq!(local_message["transport"], "local");
    assert_eq!(local_message["status"], "pending");

    let send_remote_message_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{message_run_id}/messages/send"),
            Some(&token),
            Some(json!({
                "from_actor_id":"planner",
                "to_actor_id":"remote-reviewer",
                "channel":"federation",
                "transport":"remote",
                "route":{"endpoint":"https://remote.example/a2a"},
                "payload":{"text":"remote request"}
            })),
        ))
        .await
        .expect("send remote actor message via router");
    assert_eq!(send_remote_message_resp.status(), StatusCode::OK);
    let remote_message = decode_json_body(send_remote_message_resp).await;
    let remote_message_id = remote_message["message_id"]
        .as_i64()
        .expect("remote message id");
    assert_eq!(remote_message["transport"], "remote");
    assert_eq!(
        remote_message["route"]["endpoint"],
        "https://remote.example/a2a"
    );

    let send_idempotent_first_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{message_run_id}/messages/send"),
            Some(&token),
            Some(json!({
                "from_actor_id":"planner",
                "to_actor_id":"planner",
                "channel":"coordination",
                "transport":"local",
                "route":null,
                "payload":{"text":"idempotent message"},
                "idempotency_key":"router-msg-1"
            })),
        ))
        .await
        .expect("send idempotent actor message via router");
    assert_eq!(send_idempotent_first_resp.status(), StatusCode::OK);
    let idempotent_first = decode_json_body(send_idempotent_first_resp).await;
    let idempotent_message_id = idempotent_first["message_id"]
        .as_i64()
        .expect("idempotent message id");

    let send_idempotent_retry_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{message_run_id}/messages/send"),
            Some(&token),
            Some(json!({
                "from_actor_id":"planner",
                "to_actor_id":"planner",
                "channel":"coordination",
                "transport":"local",
                "route":null,
                "payload":{"text":"idempotent message"},
                "idempotency_key":"router-msg-1"
            })),
        ))
        .await
        .expect("retry idempotent actor message via router");
    assert_eq!(send_idempotent_retry_resp.status(), StatusCode::OK);
    let idempotent_retry = decode_json_body(send_idempotent_retry_resp).await;
    assert_eq!(
        idempotent_retry["message_id"],
        Value::from(idempotent_message_id)
    );

    let send_idempotent_conflict_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{message_run_id}/messages/send"),
            Some(&token),
            Some(json!({
                "from_actor_id":"planner",
                "to_actor_id":"planner",
                "channel":"coordination",
                "transport":"local",
                "route":null,
                "payload":{"text":"changed message"},
                "idempotency_key":"router-msg-1"
            })),
        ))
        .await
        .expect("send conflicting idempotent actor message via router");
    assert_eq!(send_idempotent_conflict_resp.status(), StatusCode::CONFLICT);

    let invalid_remote_message_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{message_run_id}/messages/send"),
            Some(&token),
            Some(json!({
                "from_actor_id":"planner",
                "to_actor_id":"remote-reviewer-2",
                "channel":"federation",
                "transport":"remote",
                "route":null,
                "payload":{"text":"missing route"}
            })),
        ))
        .await
        .expect("send invalid remote actor message via router");
    assert_eq!(
        invalid_remote_message_resp.status(),
        StatusCode::BAD_REQUEST
    );

    let list_inbox_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/runs/{message_run_id}/messages/inbox?actor_id=planner&limit=100"),
            Some(&token),
            None,
        ))
        .await
        .expect("list actor inbox via router");
    assert_eq!(list_inbox_resp.status(), StatusCode::OK);
    let inbox = decode_json_body(list_inbox_resp).await;
    let inbox = inbox.as_array().expect("inbox array");
    assert_eq!(inbox.len(), 2);
    let message_ids = inbox
        .iter()
        .filter_map(|message| message["message_id"].as_i64())
        .collect::<Vec<_>>();
    assert!(message_ids.contains(&local_message_id));
    assert!(message_ids.contains(&idempotent_message_id));

    let ack_local_message_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{message_run_id}/messages/{local_message_id}/ack"),
            Some(&token),
            Some(json!({"actor_id":"planner"})),
        ))
        .await
        .expect("ack actor message via router");
    assert_eq!(ack_local_message_resp.status(), StatusCode::OK);
    let acked = decode_json_body(ack_local_message_resp).await;
    assert_eq!(acked["status"], "delivered");

    let wrong_actor_ack_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{message_run_id}/messages/{remote_message_id}/ack"),
            Some(&token),
            Some(json!({"actor_id":"planner"})),
        ))
        .await
        .expect("ack remote message by wrong actor via router");
    assert_eq!(wrong_actor_ack_resp.status(), StatusCode::NOT_FOUND);

    let create_fail_run_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/runs"),
            Some(&token),
            Some(json!({"context_id":"ctx-router-fail","input":{"prompt":"fail path"}})),
        ))
        .await
        .expect("create run for fail path");
    assert_eq!(create_fail_run_resp.status(), StatusCode::OK);
    let fail_run = decode_json_body(create_fail_run_resp).await;
    let fail_run_id = fail_run["id"].as_str().expect("fail run id").to_string();

    let submit_fail_step_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{fail_run_id}/steps"),
            Some(&token),
            Some(json!({
                "step_key": "router-fail-step",
                "member_id": "planner",
                "depends_on": [],
                "input": null
            })),
        ))
        .await
        .expect("submit fail step via router");
    assert_eq!(submit_fail_step_resp.status(), StatusCode::OK);
    let fail_step = decode_json_body(submit_fail_step_resp).await;
    let fail_step_id = fail_step["id"].as_str().expect("fail step id").to_string();

    let start_fail_step_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{fail_run_id}/steps/{fail_step_id}/start"),
            Some(&token),
            Some(json!({"remote_task_id":"router-fail-task"})),
        ))
        .await
        .expect("start fail step via router");
    assert_eq!(start_fail_step_resp.status(), StatusCode::OK);

    let invalid_fail_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{fail_run_id}/steps/{fail_step_id}/fail"),
            Some(&token),
            Some(json!({"error_text":"  "})),
        ))
        .await
        .expect("invalid fail step via router");
    assert_eq!(invalid_fail_resp.status(), StatusCode::BAD_REQUEST);

    let fail_step_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{fail_run_id}/steps/{fail_step_id}/fail"),
            Some(&token),
            Some(json!({"error_text":"worker error"})),
        ))
        .await
        .expect("fail step via router");
    assert_eq!(fail_step_resp.status(), StatusCode::OK);
    let failed_step = decode_json_body(fail_step_resp).await;
    assert_eq!(failed_step["status"], "failed");

    let missing_step_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{step_run_id}/steps/missing-step/start"),
            Some(&token),
            Some(json!({"remote_task_id":null})),
        ))
        .await
        .expect("missing step request");
    assert_eq!(missing_step_resp.status(), StatusCode::NOT_FOUND);

    let missing_team_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            "/missing-team/runs",
            Some(&token),
            Some(json!({"input": {}})),
        ))
        .await
        .expect("missing team request");
    assert_eq!(missing_team_resp.status(), StatusCode::NOT_FOUND);

    let missing_team_list_runs_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            "/missing-team/runs",
            Some(&token),
            None,
        ))
        .await
        .expect("missing team list runs request");
    assert_eq!(missing_team_list_runs_resp.status(), StatusCode::NOT_FOUND);

    let missing_run_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            "/runs/missing-run/events",
            Some(&token),
            None,
        ))
        .await
        .expect("missing run request");
    assert_eq!(missing_run_resp.status(), StatusCode::NOT_FOUND);

    let unsupported_version_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            "/",
            Some(&token),
            Some(json!({
                "name": "unsupported-version-team",
                "description": null,
                "spec": {
                    "spec_version": 2,
                    "entrypoint": "planner",
                    "members": [{"member_id":"planner"}]
                }
            })),
        ))
        .await
        .expect("unsupported version request");
    assert_eq!(unsupported_version_resp.status(), StatusCode::BAD_REQUEST);

    let delete_team_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::DELETE,
            &format!("/{team_id}"),
            Some(&token),
            None,
        ))
        .await
        .expect("delete team via router");
    assert_eq!(delete_team_resp.status(), StatusCode::OK);
    let deleted_team = decode_json_body(delete_team_resp).await;
    assert_eq!(deleted_team["id"], team_id);

    let deleted_get_team_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/{team_id}"),
            Some(&token),
            None,
        ))
        .await
        .expect("get deleted team via router");
    assert_eq!(deleted_get_team_resp.status(), StatusCode::NOT_FOUND);

    let deleted_run_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/runs/{run_id}"),
            Some(&token),
            None,
        ))
        .await
        .expect("get run after team deletion");
    assert_eq!(deleted_run_resp.status(), StatusCode::NOT_FOUND);

    let delete_missing_team_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::DELETE,
            "/missing-team",
            Some(&token),
            None,
        ))
        .await
        .expect("delete missing team via router");
    assert_eq!(delete_missing_team_resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn teams_router_orchestrator_converges_with_real_executor() {
    let state = build_test_state().await;
    let token = create_auth_token(&state).await;
    let app = super::router(state.clone());

    let workdir = std::env::temp_dir().join(format!("agenthub-team-exec-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&workdir).expect("create workdir");
    let workdir_str = workdir.to_string_lossy().to_string();

    sqlx::query(
        r#"
        INSERT INTO safe_paths (path, created_at)
        VALUES (?1, ?2)
        "#,
    )
    .bind(&workdir_str)
    .bind(chrono::Utc::now().timestamp())
    .execute(&state.db)
    .await
    .expect("insert safe path");

    let member_agent = state
        .agents
        .create_agent(crate::agent::AgentConfig {
            name: "router-orchestrator-member".to_string(),
            workdir: workdir_str.clone(),
            command: "/bin/sh".to_string(),
            args: vec!["-lc".to_string(), "sleep 2".to_string()],
            worktree_mode: crate::agent::WorktreeMode::UseExisting,
            worktree_repo: None,
            worktree_ref: None,
            code_mode: false,
        })
        .await
        .expect("create member agent");

    let create_team_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            "/",
            Some(&token),
            Some(json!({
                "name": "router-orchestrator-real-executor-team",
                "description": "router orchestrator convergence with real executor",
                "spec": {
                    "entrypoint":"step_run",
                    "members":[{"member_id":member_agent.id,"role":"leader"}],
                    "steps":[{"step_key":"step_run","member_id":member_agent.id,"depends_on":[]}]
                }
            })),
        ))
        .await
        .expect("create team");
    assert_eq!(create_team_resp.status(), StatusCode::OK);
    let team = decode_json_body(create_team_resp).await;
    let team_id = team["id"].as_str().expect("team id").to_string();

    let create_run_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/runs"),
            Some(&token),
            Some(json!({
                "context_id": "ctx-router-orchestrator-real-executor",
                "input": {"prompt":"execute member agent"}
            })),
        ))
        .await
        .expect("create run");
    assert_eq!(create_run_resp.status(), StatusCode::OK);
    let run = decode_json_body(create_run_resp).await;
    let run_id = run["id"].as_str().expect("run id").to_string();
    assert_eq!(run["status"], "submitted");

    let worker =
        crate::team::TeamOrchestratorWorker::new(state.teams.clone(), state.agents.clone());
    let first_summary = worker.dispatch_once(64).await.expect("first dispatch");
    assert_eq!(first_summary.dispatched, 1);

    let mut step_remote_task_id = None;
    for _ in 0..20 {
        let db_steps = state.teams.list_steps(&run_id).await.expect("list db steps");
        if let Some(step) = db_steps.first()
            && step.status == crate::team::TeamStepStatus::Working {
                step_remote_task_id = step.remote_task_id.clone();
                break;
            }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let remote_task_id = step_remote_task_id.expect("step should reach working state");
    assert!(!remote_task_id.is_empty());

    state
        .agents
        .stop_agent(&member_agent.id)
        .await
        .expect("stop member agent");

    let mut converged_failed = false;
    let mut last_run_status = String::new();
    for _ in 0..200 {
        let _ = worker.dispatch_once(64).await.expect("tick dispatch");
        let run_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::GET,
                &format!("/runs/{run_id}"),
                Some(&token),
                None,
            ))
            .await
            .expect("get run");
        assert_eq!(run_resp.status(), StatusCode::OK);
        let run_payload = decode_json_body(run_resp).await;
        last_run_status = run_payload["status"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        if last_run_status == "failed" {
            converged_failed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    if !converged_failed {
        let db_steps = state.teams.list_steps(&run_id).await.expect("list db steps");
        let db_step = db_steps.first().expect("first db step");
        let session_status = match db_step.remote_task_id.as_deref() {
            Some(session_id) => state
                .teams
                .get_agent_session_status(session_id)
                .await
                .expect("query session status"),
            None => None,
        };
        panic!(
            "run did not converge to failed in time: run_status={}, step_status={:?}, remote_task_id={:?}, session_status={:?}",
            last_run_status, db_step.status, db_step.remote_task_id, session_status
        );
    }

    let steps_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/runs/{run_id}/steps"),
            Some(&token),
            None,
        ))
        .await
        .expect("list steps");
    assert_eq!(steps_resp.status(), StatusCode::OK);
    let steps_payload = decode_json_body(steps_resp).await;
    let steps = steps_payload.as_array().expect("steps array");
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0]["status"], "failed");
    let final_remote_task_id = steps[0]["remote_task_id"]
        .as_str()
        .expect("remote task id");
    assert_eq!(final_remote_task_id, remote_task_id);

    let events_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/runs/{run_id}/events?limit=100"),
            Some(&token),
            None,
        ))
        .await
        .expect("list events");
    assert_eq!(events_resp.status(), StatusCode::OK);
    let events_payload = decode_json_body(events_resp).await;
    let events = events_payload.as_array().expect("events array");
    let event_types = events
        .iter()
        .filter_map(|event| event["event_type"].as_str())
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"run_submitted"));
    assert!(event_types.contains(&"step_submitted"));
    assert!(event_types.contains(&"run_working"));
    assert!(event_types.contains(&"step_working"));
    assert!(event_types.contains(&"step_failed"));
    assert!(event_types.contains(&"run_failed"));

    let _ = std::fs::remove_dir_all(&workdir);
}
