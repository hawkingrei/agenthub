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
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        }),
    )
    .await
    .expect("create team");
    assert_eq!(created.spec["spec_version"], Value::from(1));

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
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        }),
    )
    .await
    .expect_err("duplicate team name should fail");
    assert_eq!(err.into_response().status(), StatusCode::CONFLICT);
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
async fn teams_api_internal_errors_are_sanitized() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "internal-error-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
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
            spec: json!({"entrypoint":"executor","members":[{"member_id":"executor"}]}),
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

    let Json(found_run) =
        get_team_run(State(state.clone()), headers.clone(), Path(run.id.clone()))
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
async fn team_run_steps_api_supports_scheduler_lifecycle_bridge() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "scheduler-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
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
    assert_eq!(
        duplicate_submit_err.into_response().status(),
        StatusCode::CONFLICT
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
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
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
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
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
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
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
    assert_eq!(mismatch_conflict_err.into_response().status(), StatusCode::CONFLICT);

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
