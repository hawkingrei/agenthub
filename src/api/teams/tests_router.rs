#[tokio::test]
async fn teams_router_http_contract() {
    let state = build_test_state().await;
    let token = create_auth_token(&state).await;
    let outsider_token = create_auth_token(&state).await;
    let app = super::router(state.clone());

    let unauthorized = app
        .clone()
        .oneshot(build_json_request(Method::GET, "/", None, None))
        .await
        .expect("run unauthorized request");
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let prompt_defaults_unauthorized = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            "/prompt_defaults",
            None,
            None,
        ))
        .await
        .expect("run unauthorized prompt defaults request");
    assert_eq!(prompt_defaults_unauthorized.status(), StatusCode::UNAUTHORIZED);

    let prompt_defaults_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            "/prompt_defaults",
            Some(&token),
            None,
        ))
        .await
        .expect("get team prompt defaults via router");
    assert_eq!(prompt_defaults_resp.status(), StatusCode::OK);
    let prompt_defaults = decode_json_body(prompt_defaults_resp).await;
    assert!(
        prompt_defaults["coordinator_prompt"]
            .as_str()
            .is_some_and(|value| value.contains("Inspect inbox regularly")),
        "unexpected coordinator prompt contract: {}",
        prompt_defaults["coordinator_prompt"]
    );
    assert!(
        prompt_defaults["worker_prompt"]
            .as_str()
            .is_some_and(|value| value.contains("Receive inbox work")),
        "unexpected worker prompt contract: {}",
        prompt_defaults["worker_prompt"]
    );

    let empty_team_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            "/",
            Some(&token),
            Some(json!({
                "name": "empty-router-team",
                "description": null,
                "spec": {"entrypoint":"planner","members":[]}
            })),
        ))
        .await
        .expect("create empty team via router");
    assert_eq!(empty_team_resp.status(), StatusCode::OK);
    let empty_team = decode_json_body(empty_team_resp).await;
    assert_eq!(empty_team["spec"]["spec_version"], Value::from(1));
    assert_eq!(empty_team["spec"]["members"], json!([]));
    assert_eq!(empty_team.get("spec").and_then(|spec| spec.get("entrypoint")), None);
    assert_eq!(
        empty_team
            .get("spec")
            .and_then(|spec| spec.get("coordinator_member_id")),
        None
    );
    assert_eq!(empty_team.get("spec").and_then(|spec| spec.get("steps")), None);

    let create_team_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            "/",
            Some(&token),
            Some(json!({
                "name": "router-team",
                "description": "router-level contract",
                "spec": {
                    "entrypoint":"planner",
                    "members":[
                        {"member_id":"planner","role":"coordinator"},
                        {"member_id":"worker-1","role":"worker"}
                    ]
                }
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
    assert_eq!(listed.len(), 2);
    let mut listed_ids = listed
        .iter()
        .map(|team| team["id"].as_str().expect("team id").to_string())
        .collect::<Vec<_>>();
    listed_ids.sort();
    let mut expected_ids = vec![
        empty_team["id"].as_str().expect("empty team id").to_string(),
        team_id.clone(),
    ];
    expected_ids.sort();
    assert_eq!(listed_ids, expected_ids);

    let duplicate_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            "/",
            Some(&token),
            Some(json!({
                "name": "router-team",
                "description": null,
                "spec": {
                    "entrypoint":"planner",
                    "members":[
                        {"member_id":"planner","role":"coordinator"},
                        {"member_id":"worker-1","role":"worker"}
                    ]
                }
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

    let get_runtime_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/{team_id}/runtime"),
            Some(&token),
            None,
        ))
        .await
        .expect("get team runtime via router");
    assert_eq!(get_runtime_resp.status(), StatusCode::OK);
    let runtime = decode_json_body(get_runtime_resp).await;
    assert_eq!(runtime["team_id"], team_id);
    let runtime_status = runtime["status"].as_str().expect("runtime status");
    assert!(
        matches!(runtime_status, "running" | "degraded" | "stopped"),
        "unexpected runtime status: {runtime_status}"
    );
    // This router-level test validates the HTTP contract shape only. Auto-start semantics are
    // covered in tests_core, while a just-started member runtime can legitimately settle back to
    // stopped before this follow-up read if the subprocess exits quickly.
    assert_eq!(runtime["members"].as_array().map(|items| items.len()), Some(2));

    let outsider_runtime_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/{team_id}/runtime"),
            Some(&outsider_token),
            None,
        ))
        .await
        .expect("outsider get runtime");
    assert_eq!(outsider_runtime_resp.status(), StatusCode::NOT_FOUND);

    let create_task_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/tasks"),
            Some(&token),
            Some(json!({
                "title": "router task",
                "created_by_actor_id": "user",
                "context": {"token":"should-redact"},
                "conversation_mode": "group_chat",
                "topic": "kickoff"
            })),
        ))
        .await
        .expect("create task via router");
    assert_eq!(create_task_resp.status(), StatusCode::METHOD_NOT_ALLOWED);

    let mut token_headers = HeaderMap::new();
    token_headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).expect("router auth header"),
    );
    let seeded_task = create_team_task(
        &state,
        &token_headers,
        &team_id,
        CreateTeamTaskRequest {
            title: "router task".to_string(),
            priority: "high".to_string(),
            assigned_member_id: "planner".to_string(),
            created_by_actor_id: Some("user".to_string()),
            context: Some(json!({
                "token":"should-redact"
            })),
            conversation_mode: Some("group_chat".to_string()),
            topic: Some("kickoff".to_string()),
        },
    )
    .await
    .expect("seed canonical task for router contract");
    let task_id = seeded_task.task.id.clone();

    let list_tasks_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/{team_id}/tasks?limit=20"),
            Some(&token),
            None,
        ))
        .await
        .expect("list tasks via router");
    assert_eq!(list_tasks_resp.status(), StatusCode::OK);
    let listed_tasks = decode_json_body(list_tasks_resp).await;
    assert_eq!(listed_tasks.as_array().map(Vec::len), Some(1));
    let get_task_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/{team_id}/tasks/{task_id}"),
            Some(&token),
            None,
        ))
        .await
        .expect("get task via router");
    assert_eq!(get_task_resp.status(), StatusCode::OK);
    let get_task_body = decode_json_body(get_task_resp).await;
    assert_eq!(
        get_task_body["task"]["context"]["token"],
        Value::from("[redacted]")
    );
    assert_eq!(get_task_body["task"]["status"], Value::from("open"));
    assert_eq!(get_task_body["task"]["assigned_member_id"], Value::from("planner"));
    assert_eq!(get_task_body["latest_run"], Value::Null);
    assert!(
        get_task_body["task"]["created_by_actor_id"]
            .as_str()
            .map(|value| value.starts_with("user:"))
            .unwrap_or(false)
    );

    let update_task_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::PATCH,
            &format!("/{team_id}/tasks/{task_id}"),
            Some(&token),
            Some(json!({
                "status": "in_progress"
            })),
        ))
        .await
        .expect("update task via router");
    assert_eq!(update_task_resp.status(), StatusCode::FORBIDDEN);
    let update_task_err = decode_json_body(update_task_resp).await;
    assert_eq!(
        update_task_err["error"],
        Value::from(
            "canonical Team task status/owner updates are agent-only; use actor runtime controls",
        )
    );

    let invalid_update_task_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::PATCH,
            &format!("/{team_id}/tasks/{task_id}"),
            Some(&token),
            Some(json!({
                "status": "paused"
            })),
        ))
        .await
        .expect("invalid task status via router");
    assert_eq!(invalid_update_task_resp.status(), StatusCode::FORBIDDEN);

    let send_human_task_message_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/tasks/{task_id}/messages"),
            Some(&token),
            Some(json!({
                "route": "group_chat",
                "payload": {"text":"human message without explicit actor id"}
            })),
        ))
        .await
        .expect("send human task message without explicit actor id via router");
    assert_eq!(send_human_task_message_resp.status(), StatusCode::OK);
    let human_task_message = decode_json_body(send_human_task_message_resp).await;
    assert!(
        human_task_message["from_actor_id"]
            .as_str()
            .map(|value| value.starts_with("user:"))
            .unwrap_or(false)
    );
    assert!(
        human_task_message["payload"]["correlation_id"]
            .as_str()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
    );

    let create_channel_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/channels"),
            Some(&token),
            Some(json!({
                "channel_id": "review",
                "description": "Review lane"
            })),
        ))
        .await
        .expect("create channel via router");
    assert_eq!(create_channel_resp.status(), StatusCode::OK);
    let channel = decode_json_body(create_channel_resp).await;
    let channel_task_id = channel["task_id"]
        .as_str()
        .expect("channel task id")
        .to_string();
    let source_message_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/tasks/{channel_task_id}/messages"),
            Some(&token),
            Some(json!({
                "route": "group_chat",
                "payload": {"type":"chat_message","text":"Turn this channel message into a tracked task"}
            })),
        ))
        .await
        .expect("send channel message via router");
    assert_eq!(source_message_resp.status(), StatusCode::OK);
    let source_message = decode_json_body(source_message_resp).await;
    let source_message_id = source_message["message_id"]
        .as_i64()
        .expect("source message id");
    let create_from_message_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/channels/review/messages/{source_message_id}/tasks"),
            Some(&token),
            Some(json!({
                "priority": "high",
                "context": {"token":"should-redact"}
            })),
        ))
        .await
        .expect("create task from channel message via router");
    assert_eq!(create_from_message_resp.status(), StatusCode::OK);
    let created_from_message = decode_json_body(create_from_message_resp).await;
    assert_eq!(
        created_from_message["task"]["title"],
        Value::from("Turn this channel message into a tracked task")
    );
    assert_eq!(created_from_message["task"]["priority"], Value::from("high"));
    assert_eq!(
        created_from_message["task"]["context"]["bootstrap_kind"],
        Value::from("channel_message_task")
    );
    assert_eq!(
        created_from_message["task"]["context"]["source"]["channel_id"],
        Value::from("review")
    );
    assert_eq!(
        created_from_message["task"]["context"]["source"]["message_id"],
        Value::from(source_message_id)
    );
    assert_eq!(
        created_from_message["task"]["context"]["token"],
        Value::from("[redacted]")
    );

    let send_task_message_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/tasks/{task_id}/messages"),
            Some(&token),
            Some(json!({
                "from_actor_id": "planner",
                "to_actor_id": "worker-1",
                "route": "to_member",
                "payload": {"authorization":"Bearer x","text":"assign"}
            })),
        ))
        .await
        .expect("send task message via router");
    assert_eq!(send_task_message_resp.status(), StatusCode::OK);
    let task_message = decode_json_body(send_task_message_resp).await;
    assert_eq!(
        task_message["payload"]["authorization"],
        Value::from("[redacted]")
    );

    let send_to_coordinator_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/tasks/{task_id}/messages"),
            Some(&token),
            Some(json!({
                "from_actor_id": "worker-1",
                "route": "to_coordinator",
                "payload": {"text":"need decision"}
            })),
        ))
        .await
        .expect("send to coordinator via router");
    assert_eq!(send_to_coordinator_resp.status(), StatusCode::OK);
    let to_coordinator_message = decode_json_body(send_to_coordinator_resp).await;
    assert_eq!(to_coordinator_message["to_actor_id"], Value::from("planner"));

    let list_task_messages_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/{team_id}/tasks/{task_id}/messages?limit=20"),
            Some(&token),
            None,
        ))
        .await
        .expect("list task messages via router");
    assert_eq!(list_task_messages_resp.status(), StatusCode::OK);
    let listed_task_messages = decode_json_body(list_task_messages_resp).await;
    assert_eq!(listed_task_messages.as_array().map(Vec::len), Some(3));

    let compile_preview_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/tasks/{task_id}/compile_run_preview"),
            Some(&token),
            Some(json!({})),
        ))
        .await
        .expect("compile run preview via router");
    assert_eq!(compile_preview_resp.status(), StatusCode::OK);
    let compile_preview = decode_json_body(compile_preview_resp).await;
    assert_eq!(compile_preview["task_id"], Value::from(task_id.clone()));
    assert_eq!(
        compile_preview["run_payload"]["context_id"],
        Value::from(task_id.clone())
    );
    assert_eq!(
        compile_preview["run_payload"]["input"]["task_compile_version"],
        Value::from(1)
    );
    assert_eq!(
        compile_preview["plan"]["role_assignments"][0]["member_id"],
        Value::from("planner")
    );

    let outsider_compile_preview_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/tasks/{task_id}/compile_run_preview"),
            Some(&outsider_token),
            Some(json!({})),
        ))
        .await
        .expect("outsider compile preview via router");
    assert_eq!(
        outsider_compile_preview_resp.status(),
        StatusCode::NOT_FOUND
    );

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
    let mut listed_run_ids = listed_runs
        .iter()
        .map(|run| run["id"].as_str().expect("run id").to_string())
        .collect::<Vec<_>>();
    listed_run_ids.sort();
    let mut expected_run_ids = vec![run_id.clone()];
    expected_run_ids.sort();
    assert_eq!(listed_run_ids, expected_run_ids);

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

    let resume_run_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{run_id}/resume"),
            Some(&token),
            None,
        ))
        .await
        .expect("resume run via router");
    assert_eq!(resume_run_resp.status(), StatusCode::OK);
    let resumed = decode_json_body(resume_run_resp).await;
    assert_eq!(resumed["status"], "submitted");
    assert_ne!(resumed["id"].as_str(), Some(run_id.as_str()));
    assert_eq!(resumed["team_id"], team_id);

    let restart_run_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{run_id}/restart"),
            Some(&token),
            None,
        ))
        .await
        .expect("restart run via router");
    assert_eq!(restart_run_resp.status(), StatusCode::OK);
    let restarted = decode_json_body(restart_run_resp).await;
    assert_eq!(restarted["status"], "submitted");
    assert_ne!(restarted["id"].as_str(), Some(run_id.as_str()));
    assert_eq!(restarted["team_id"], team_id);

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

    for index in 0..25 {
        sqlx::query(
            r#"
            INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
            VALUES (?1, NULL, 'agent_message', ?2, ?3)
            "#,
        )
        .bind(&run_id)
        .bind(2_000 + i64::from(index))
        .bind(format!("{{\"text\":\"bulk-{index}\"}}"))
        .execute(&state.db)
        .await
        .expect("insert extra run events");
    }

    let clamped_events_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/runs/{run_id}/events?limit=100"),
            Some(&token),
            None,
        ))
        .await
        .expect("list clamped events via router");
    assert_eq!(clamped_events_resp.status(), StatusCode::OK);
    let clamped_events = decode_json_body(clamped_events_resp).await;
    let clamped_events = clamped_events.as_array().expect("clamped events array");
    assert_eq!(clamped_events.len(), 20);

    let snapshot_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/runs/{run_id}/snapshot?event_limit=100&message_limit=100"),
            Some(&token),
            None,
        ))
        .await
        .expect("snapshot via router");
    assert_eq!(snapshot_resp.status(), StatusCode::OK);
    let snapshot = decode_json_body(snapshot_resp).await;
    assert_eq!(snapshot["latest_events"].as_array().map(Vec::len), Some(20));

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
            Some(json!({"runtime_handle_id":"router-remote-task"})),
        ))
        .await
        .expect("start step via router");
    assert_eq!(start_step_resp.status(), StatusCode::OK);
    let started_step = decode_json_body(start_step_resp).await;
    assert_eq!(started_step["status"], "working");
    assert_eq!(started_step["runtime_handle_id"], "router-remote-task");
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
    for message in inbox.iter() {
        assert_eq!(message["status"], "pending");
    }
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
async fn teams_router_delete_team_cleans_member_session_dependents_without_500() {
    let state = build_test_state().await;
    let token = create_auth_token(&state).await;
    let app = super::router(state.clone());
    let member_agent_id = "planner";
    let now = Utc::now().timestamp();

    sqlx::query("UPDATE agents SET status = 'running', updated_at = ?2 WHERE id = ?1")
        .bind(member_agent_id)
        .bind(now)
    .execute(&state.db)
    .await
    .expect("mark seeded member agent running");

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

    let create_team_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            "/",
            Some(&token),
            Some(json!({
                "name": format!("router-delete-fk-{}", Uuid::new_v4()),
                "description": "delete fk regression",
                "spec": {
                    "entrypoint":"planner",
                    "members":[{"member_id":"planner","role":"coordinator"}]
                }
            })),
        ))
        .await
        .expect("create team via router");
    assert_eq!(create_team_resp.status(), StatusCode::OK);
    let created_team = decode_json_body(create_team_resp).await;
    let team_id = created_team["id"].as_str().expect("team id").to_string();

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

    let session_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM agent_sessions WHERE agent_id = ?1")
            .bind(member_agent_id)
            .fetch_one(&state.db)
            .await
            .expect("count member sessions");
    assert_eq!(session_count, 0);

    let permission_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM acp_permission_requests WHERE agent_id = ?1")
            .bind(member_agent_id)
            .fetch_one(&state.db)
            .await
            .expect("count permission requests");
    assert_eq!(permission_count, 0);
}

#[tokio::test]
async fn teams_router_resume_restart_strategy_survives_state_reopen() {
    let base = std::env::temp_dir().join(format!("agenthub-team-run-recover-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&base).expect("create temp base");
    let db_path = base.join("teams.sqlite");

    let state = build_test_state_with_db_path(&db_path).await;
    let token = create_auth_token(&state).await;
    let app = super::router(state.clone());

    let create_team_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            "/",
            Some(&token),
            Some(json!({
                "name": "router-restart-strategy-team",
                "description": "verify run resume/restart across state reopen",
                "spec": {
                    "entrypoint":"planner",
                    "members":[{"member_id":"planner","role":"coordinator"}]
                }
            })),
        ))
        .await
        .expect("create team");
    assert_eq!(create_team_resp.status(), StatusCode::OK);
    let created_team = decode_json_body(create_team_resp).await;
    let team_id = created_team["id"].as_str().expect("team id").to_string();

    let create_failed_run_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/runs"),
            Some(&token),
            Some(json!({
                "context_id":"ctx-reopen-failed",
                "input":{"prompt":"failed run"}
            })),
        ))
        .await
        .expect("create failed run");
    assert_eq!(create_failed_run_resp.status(), StatusCode::OK);
    let failed_run = decode_json_body(create_failed_run_resp).await;
    let failed_run_id = failed_run["id"]
        .as_str()
        .expect("failed run id")
        .to_string();

    let submit_failed_step_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{failed_run_id}/steps"),
            Some(&token),
            Some(json!({
                "step_key":"failed-step",
                "member_id":"planner",
                "depends_on":[],
                "input":{"goal":"fail"}
            })),
        ))
        .await
        .expect("submit failed step");
    assert_eq!(submit_failed_step_resp.status(), StatusCode::OK);
    let failed_step = decode_json_body(submit_failed_step_resp).await;
    let failed_step_id = failed_step["id"].as_str().expect("failed step id");

    let start_failed_step_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{failed_run_id}/steps/{failed_step_id}/start"),
            Some(&token),
            Some(json!({"remote_task_id":"remote-failed"})),
        ))
        .await
        .expect("start failed step");
    assert_eq!(start_failed_step_resp.status(), StatusCode::OK);

    let fail_step_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{failed_run_id}/steps/{failed_step_id}/fail"),
            Some(&token),
            Some(json!({"error_text":"forced failure"})),
        ))
        .await
        .expect("fail step");
    assert_eq!(fail_step_resp.status(), StatusCode::OK);

    let failed_run_status_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/runs/{failed_run_id}"),
            Some(&token),
            None,
        ))
        .await
        .expect("get failed run");
    assert_eq!(failed_run_status_resp.status(), StatusCode::OK);
    let failed_run_status = decode_json_body(failed_run_status_resp).await;
    assert_eq!(failed_run_status["status"], "failed");

    let create_canceled_run_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/runs"),
            Some(&token),
            Some(json!({
                "context_id":"ctx-reopen-canceled",
                "input":{"prompt":"canceled run"}
            })),
        ))
        .await
        .expect("create canceled run");
    assert_eq!(create_canceled_run_resp.status(), StatusCode::OK);
    let canceled_run = decode_json_body(create_canceled_run_resp).await;
    let canceled_run_id = canceled_run["id"]
        .as_str()
        .expect("canceled run id")
        .to_string();

    let cancel_run_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{canceled_run_id}/cancel"),
            Some(&token),
            None,
        ))
        .await
        .expect("cancel run");
    assert_eq!(cancel_run_resp.status(), StatusCode::OK);
    let canceled_run_status = decode_json_body(cancel_run_resp).await;
    assert_eq!(canceled_run_status["status"], "canceled");

    let create_completed_run_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{team_id}/runs"),
            Some(&token),
            Some(json!({
                "context_id":"ctx-reopen-completed",
                "input":{"prompt":"completed run"}
            })),
        ))
        .await
        .expect("create completed run");
    assert_eq!(create_completed_run_resp.status(), StatusCode::OK);
    let completed_run = decode_json_body(create_completed_run_resp).await;
    let completed_run_id = completed_run["id"]
        .as_str()
        .expect("completed run id")
        .to_string();

    let submit_completed_step_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{completed_run_id}/steps"),
            Some(&token),
            Some(json!({
                "step_key":"completed-step",
                "member_id":"planner",
                "depends_on":[],
                "input":{"goal":"finish"}
            })),
        ))
        .await
        .expect("submit completed step");
    assert_eq!(submit_completed_step_resp.status(), StatusCode::OK);
    let completed_step = decode_json_body(submit_completed_step_resp).await;
    let completed_step_id = completed_step["id"].as_str().expect("completed step id");

    let start_completed_step_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{completed_run_id}/steps/{completed_step_id}/start"),
            Some(&token),
            Some(json!({"remote_task_id":"remote-completed"})),
        ))
        .await
        .expect("start completed step");
    assert_eq!(start_completed_step_resp.status(), StatusCode::OK);

    let complete_step_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{completed_run_id}/steps/{completed_step_id}/complete"),
            Some(&token),
            Some(json!({"output":{"result":"ok"}})),
        ))
        .await
        .expect("complete step");
    assert_eq!(complete_step_resp.status(), StatusCode::OK);

    let completed_run_status_resp = app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/runs/{completed_run_id}"),
            Some(&token),
            None,
        ))
        .await
        .expect("get completed run");
    assert_eq!(completed_run_status_resp.status(), StatusCode::OK);
    let completed_run_status = decode_json_body(completed_run_status_resp).await;
    assert_eq!(completed_run_status["status"], "completed");

    drop(app);
    drop(state);

    let reopened_state = reopen_test_state_with_db_path(&db_path).await;
    let reopened_app = super::router(reopened_state);

    let resumed_failed_resp = reopened_app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{failed_run_id}/resume"),
            Some(&token),
            None,
        ))
        .await
        .expect("resume failed run after reopen");
    assert_eq!(resumed_failed_resp.status(), StatusCode::OK);
    let resumed_failed = decode_json_body(resumed_failed_resp).await;
    assert_ne!(resumed_failed["id"], failed_run["id"]);
    assert_eq!(resumed_failed["status"], "submitted");
    assert_eq!(resumed_failed["context_id"], failed_run["context_id"]);
    assert_eq!(resumed_failed["input"], failed_run["input"]);

    let resumed_canceled_resp = reopened_app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{canceled_run_id}/resume"),
            Some(&token),
            None,
        ))
        .await
        .expect("resume canceled run after reopen");
    assert_eq!(resumed_canceled_resp.status(), StatusCode::OK);
    let resumed_canceled = decode_json_body(resumed_canceled_resp).await;
    assert_ne!(resumed_canceled["id"], canceled_run["id"]);
    assert_eq!(resumed_canceled["status"], "submitted");
    assert_eq!(resumed_canceled["context_id"], canceled_run["context_id"]);
    assert_eq!(resumed_canceled["input"], canceled_run["input"]);

    let resume_completed_resp = reopened_app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{completed_run_id}/resume"),
            Some(&token),
            None,
        ))
        .await
        .expect("resume completed run after reopen");
    assert_eq!(resume_completed_resp.status(), StatusCode::CONFLICT);

    let restarted_completed_resp = reopened_app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/runs/{completed_run_id}/restart"),
            Some(&token),
            None,
        ))
        .await
        .expect("restart completed run after reopen");
    assert_eq!(restarted_completed_resp.status(), StatusCode::OK);
    let restarted_completed = decode_json_body(restarted_completed_resp).await;
    assert_ne!(restarted_completed["id"], completed_run["id"]);
    assert_eq!(restarted_completed["status"], "submitted");
    assert_eq!(
        restarted_completed["context_id"],
        completed_run["context_id"]
    );
    assert_eq!(restarted_completed["input"], completed_run["input"]);

    let original_completed_resp = reopened_app
        .clone()
        .oneshot(build_json_request(
            Method::GET,
            &format!("/runs/{completed_run_id}"),
            Some(&token),
            None,
        ))
        .await
        .expect("get original completed run after restart");
    assert_eq!(original_completed_resp.status(), StatusCode::OK);
    let original_completed = decode_json_body(original_completed_resp).await;
    assert_eq!(original_completed["status"], "completed");

    let _ = std::fs::remove_dir_all(&base);
}
