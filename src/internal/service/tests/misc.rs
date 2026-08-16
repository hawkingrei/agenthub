use super::*;

#[test]
fn resolve_team_coordinator_member_id_supports_legacy_fallbacks() {
    assert_eq!(
        super::resolve_team_coordinator_member_id(&json!({
            "members":[{"member_id":"planner","role":"coordinator"}]
        }))
        .expect("resolve from role"),
        "planner"
    );
    assert_eq!(
        super::resolve_team_coordinator_member_id(&json!({
            "entrypoint":"planner"
        }))
        .expect("resolve from entrypoint"),
        "planner"
    );
}

#[test]
fn actor_service_error_code_maps_to_expected_grpc_status() {
    let cases = [
        (
            agenthub_team_actor::ActorServiceErrorCode::BadRequest,
            Code::InvalidArgument,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::UnprocessableEntity,
            Code::InvalidArgument,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::Unauthorized,
            Code::Unauthenticated,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::Forbidden,
            Code::PermissionDenied,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::NotFound,
            Code::NotFound,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::Conflict,
            Code::AlreadyExists,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::Gone,
            Code::FailedPrecondition,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::TooManyRequests,
            Code::ResourceExhausted,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::Internal,
            Code::Internal,
        ),
    ];

    for (actor_code, grpc_code) in cases {
        let status = map_actor_service_status(agenthub_team_actor::ActorServiceError::new(
            actor_code, "boom",
        ));
        assert_eq!(status.code(), grpc_code);
    }
}

#[test]
fn parse_team_task_status_reports_trimmed_input() {
    let err = super::super::parse_team_task_status("  paused  ").expect_err("invalid status");
    assert_eq!(
        err.message(),
        "invalid task status 'paused', expected one of: open, in_progress, waiting, in_review, completed, canceled"
    );
}

async fn build_started_reconcile_transition_fixture(
    name_suffix: &str,
) -> (
    crate::state::AppState,
    TeamInternalControlService,
    String,
    String,
    String,
    String,
) {
    let state = build_test_state_without_seeded_team_member_agents().await;
    let now = chrono::Utc::now().timestamp();
    let workdir = std::env::temp_dir().join(format!(
        "agenthub-internal-{name_suffix}-{}",
        Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workdir).expect("create reconcile transition test workdir");
    let workdir = workdir.to_string_lossy().to_string();
    sqlx::query("INSERT OR IGNORE INTO safe_paths (path, created_at) VALUES (?1, ?2)")
        .bind(&workdir)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert reconcile transition safe path");
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
        .expect("insert reconcile transition test agent");
    }

    let team = state
        .teams
        .create_team(TeamDefinitionConfig {
            name: format!("internal-{name_suffix}-{}", Uuid::new_v4()),
            description: Some(format!("internal {name_suffix} reconcile transition test")),
            spec: json!({
                "entrypoint":"planner",
                "coordinator_member_id":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let (task, _) = state
        .teams
        .create_task(
            &team.id,
            "Internal reconcile task",
            "planner",
            json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"worker-1",
                        "goal":"finish the patch",
                        "acceptance":["tests pass"],
                        "execution":{"mode":"reconcile_loop","max_rounds":3}
                    }]
                }
            }),
            "group_chat",
            Some(name_suffix),
        )
        .await
        .expect("create task");
    let run = state
        .teams
        .create_run(
            &team.id,
            Some(&format!("ctx-{name_suffix}")),
            json!({"task_id": task.id, "prompt":"execute reconcile loop"}),
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
        .expect("step");
    state
        .teams
        .start_step(&step.id, Some("missing-session"))
        .await
        .expect("start step");

    let authz = build_authz();
    let (token, _expires_at) = authz
        .issue_access_token(
            InternalRole::Coordinator,
            Some("planner"),
            Some(&run.id),
            vec![InternalAction::StepTransition.as_str().to_string()],
            600,
        )
        .expect("issue step transition token");
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    (state, service, token, task.id, run.id, step.id)
}

#[tokio::test]
async fn transition_step_continue_advances_reconcile_round_and_keeps_step_working() {
    let state = build_test_state_without_seeded_team_member_agents().await;
    let now = chrono::Utc::now().timestamp();
    let workdir = std::env::temp_dir().join(format!(
        "agenthub-internal-continue-step-{}",
        Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workdir).expect("create internal continue test workdir");
    let workdir = workdir.to_string_lossy().to_string();
    sqlx::query("INSERT OR IGNORE INTO safe_paths (path, created_at) VALUES (?1, ?2)")
        .bind(&workdir)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert continue test safe path");
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
        .expect("insert internal continue test agent");
    }

    let team = state
        .teams
        .create_team(TeamDefinitionConfig {
            name: format!("internal-transition-continue-{}", Uuid::new_v4()),
            description: Some("internal transition continue test".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "coordinator_member_id":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let (task, _) = state
        .teams
        .create_task(
            &team.id,
            "Internal continue task",
            "planner",
            json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"worker-1",
                        "goal":"finish the patch",
                        "acceptance":["tests pass"],
                        "execution":{"mode":"reconcile_loop","max_rounds":3}
                    }]
                }
            }),
            "group_chat",
            Some("internal-continue"),
        )
        .await
        .expect("create task");
    let run = state
        .teams
        .create_run(
            &team.id,
            Some("ctx-internal-continue"),
            json!({"task_id": task.id, "prompt":"execute reconcile loop"}),
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
        .expect("step");
    state
        .teams
        .start_step(&step.id, Some("missing-session"))
        .await
        .expect("start step");

    let authz = build_authz();
    let (token, _expires_at) = authz
        .issue_access_token(
            InternalRole::Coordinator,
            Some("planner"),
            Some(&run.id),
            vec![InternalAction::StepTransition.as_str().to_string()],
            600,
        )
        .expect("issue step transition token");
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let response = TeamInternalControl::transition_step(
        &service,
        authenticated_request(
            TransitionStepRequest {
                run_id: run.id.clone(),
                step_id: step.id.clone(),
                action: "continue".to_string(),
                remote_task_id: String::new(),
                output_json: json!({"summary":"need another round"}).to_string(),
                error_text: String::new(),
                input_json: String::new(),
                reason: String::new(),
            },
            &token,
        ),
    )
    .await
    .expect("continue step transition")
    .into_inner();
    assert_eq!(response.status, "working");
    assert_eq!(response.step_id, step.id);

    let step_after = state.teams.get_step(&step.id).await.expect("get step");
    assert_eq!(step_after.status, crate::team::TeamStepStatus::Working);
    assert_eq!(
        step_after.input,
        Some(json!({
            "task_execution_step": {
                "goal":"finish the patch",
                "acceptance":["tests pass"],
                "execution":{"mode":"reconcile_loop","max_rounds":3},
                "round_state":{
                    "current_round":2,
                    "latest_status":"working"
                }
            }
        }))
    );

    let events = state
        .teams
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list events");
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "step_continued"),
        "expected step_continued event: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "step_reconcile_prompt_requested"),
        "expected reconcile prompt request after continue: {events:?}"
    );
}

#[tokio::test]
async fn transition_step_input_required_resume_and_complete_follow_reconcile_contract() {
    let (state, service, token, task_id, run_id, step_id) =
        build_started_reconcile_transition_fixture("transition-input-resume-complete").await;

    let input_required = TeamInternalControl::transition_step(
        &service,
        authenticated_request(
            TransitionStepRequest {
                run_id: run_id.clone(),
                step_id: step_id.clone(),
                action: "input_required".to_string(),
                remote_task_id: String::new(),
                output_json: String::new(),
                error_text: String::new(),
                input_json: json!({"question":"approve?"}).to_string(),
                reason: "need review".to_string(),
            },
            &token,
        ),
    )
    .await
    .expect("input_required transition")
    .into_inner();
    assert_eq!(input_required.status, "input_required");

    let step_after_input_required = state.teams.get_step(&step_id).await.expect("get step");
    assert_eq!(
        step_after_input_required.status,
        crate::team::TeamStepStatus::InputRequired
    );
    let run_after_input_required = state.teams.get_run(&run_id).await.expect("get run");
    assert_eq!(
        run_after_input_required.status,
        crate::team::TeamRunStatus::InputRequired
    );
    let task_after_input_required = state.teams.get_task(&task_id).await.expect("get task");
    assert_eq!(
        task_after_input_required.status,
        crate::team::TeamTaskStatus::Waiting
    );

    let events_after_input_required = state
        .teams
        .list_run_events(&run_id, 100, None)
        .await
        .expect("list events after input_required");
    assert!(
        events_after_input_required
            .iter()
            .all(|event| event.event_type != "step_reconcile_prompt_requested"),
        "input_required should not auto-nudge another reconcile prompt: {events_after_input_required:?}"
    );

    let resumed = TeamInternalControl::transition_step(
        &service,
        authenticated_request(
            TransitionStepRequest {
                run_id: run_id.clone(),
                step_id: step_id.clone(),
                action: "resume".to_string(),
                remote_task_id: String::new(),
                output_json: String::new(),
                error_text: String::new(),
                input_json: json!({"answer":"approved"}).to_string(),
                reason: String::new(),
            },
            &token,
        ),
    )
    .await
    .expect("resume transition")
    .into_inner();
    assert_eq!(resumed.status, "working");

    let run_after_resume = state
        .teams
        .get_run(&run_id)
        .await
        .expect("get run after resume");
    assert_eq!(run_after_resume.status, crate::team::TeamRunStatus::Working);
    let task_after_resume = state
        .teams
        .get_task(&task_id)
        .await
        .expect("get task after resume");
    assert_eq!(
        task_after_resume.status,
        crate::team::TeamTaskStatus::InProgress
    );

    let events_after_resume = state
        .teams
        .list_run_events(&run_id, 100, None)
        .await
        .expect("list events after resume");
    let prompt_request_count_after_resume = events_after_resume
        .iter()
        .filter(|event| event.event_type == "step_reconcile_prompt_requested")
        .count();
    assert_eq!(prompt_request_count_after_resume, 1);

    let completed = TeamInternalControl::transition_step(
        &service,
        authenticated_request(
            TransitionStepRequest {
                run_id: run_id.clone(),
                step_id: step_id.clone(),
                action: "complete".to_string(),
                remote_task_id: String::new(),
                output_json: json!({"summary":"patch is merge-ready"}).to_string(),
                error_text: String::new(),
                input_json: String::new(),
                reason: String::new(),
            },
            &token,
        ),
    )
    .await
    .expect("complete transition")
    .into_inner();
    assert_eq!(completed.status, "completed");

    let step_after_complete = state
        .teams
        .get_step(&step_id)
        .await
        .expect("get completed step");
    assert_eq!(
        step_after_complete.status,
        crate::team::TeamStepStatus::Completed
    );
    let run_after_complete = state
        .teams
        .get_run(&run_id)
        .await
        .expect("get run after complete");
    assert_eq!(
        run_after_complete.status,
        crate::team::TeamRunStatus::Completed
    );
    let task_after_complete = state
        .teams
        .get_task(&task_id)
        .await
        .expect("get task after complete");
    assert_eq!(
        task_after_complete.status,
        crate::team::TeamTaskStatus::InReview
    );

    let events_after_complete = state
        .teams
        .list_run_events(&run_id, 100, None)
        .await
        .expect("list events after complete");
    let prompt_request_count_after_complete = events_after_complete
        .iter()
        .filter(|event| event.event_type == "step_reconcile_prompt_requested")
        .count();
    assert_eq!(prompt_request_count_after_complete, 1);
}

#[tokio::test]
async fn transition_step_fail_keeps_reconcile_loop_terminal_without_auto_nudge() {
    let (state, service, token, task_id, run_id, step_id) =
        build_started_reconcile_transition_fixture("transition-fail").await;

    let failed = TeamInternalControl::transition_step(
        &service,
        authenticated_request(
            TransitionStepRequest {
                run_id: run_id.clone(),
                step_id: step_id.clone(),
                action: "fail".to_string(),
                remote_task_id: String::new(),
                output_json: String::new(),
                error_text: "lint failed".to_string(),
                input_json: String::new(),
                reason: String::new(),
            },
            &token,
        ),
    )
    .await
    .expect("fail transition")
    .into_inner();
    assert_eq!(failed.status, "failed");

    let step_after_fail = state
        .teams
        .get_step(&step_id)
        .await
        .expect("get failed step");
    assert_eq!(step_after_fail.status, crate::team::TeamStepStatus::Failed);
    assert_eq!(step_after_fail.error_text.as_deref(), Some("lint failed"));
    let run_after_fail = state.teams.get_run(&run_id).await.expect("get failed run");
    assert_eq!(run_after_fail.status, crate::team::TeamRunStatus::Failed);
    let task_after_fail = state
        .teams
        .get_task(&task_id)
        .await
        .expect("get linked task");
    assert_eq!(
        task_after_fail.status,
        crate::team::TeamTaskStatus::InProgress
    );

    let events = state
        .teams
        .list_run_events(&run_id, 100, None)
        .await
        .expect("list fail events");
    assert!(
        events
            .iter()
            .all(|event| event.event_type != "step_reconcile_prompt_requested"),
        "fail should not auto-nudge another reconcile prompt: {events:?}"
    );
    let round_finished_event = events
        .iter()
        .find(|event| {
            event.event_type == "step_reconcile_round_finished"
                && event.payload["status"] == json!("failed")
        })
        .expect("failed reconcile round event");
    assert_eq!(
        round_finished_event.payload["summary"],
        json!("lint failed")
    );
}

#[tokio::test]
async fn worker_token_can_continue_own_reconcile_step_but_not_other_members_step() {
    let state = build_test_state_without_seeded_team_member_agents().await;
    let now = chrono::Utc::now().timestamp();
    let workdir =
        std::env::temp_dir().join(format!("agenthub-internal-worker-step-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&workdir).expect("create internal worker step test workdir");
    let workdir = workdir.to_string_lossy().to_string();
    sqlx::query("INSERT OR IGNORE INTO safe_paths (path, created_at) VALUES (?1, ?2)")
        .bind(&workdir)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker step safe path");
    for agent_id in ["planner", "worker-1", "worker-2"] {
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
        .expect("insert internal worker step test agent");
    }

    let team = state
        .teams
        .create_team(TeamDefinitionConfig {
            name: format!("internal-worker-step-{}", Uuid::new_v4()),
            description: Some("internal worker step transition test".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "coordinator_member_id":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"},
                    {"member_id":"worker-2","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let (task, _) = state
        .teams
        .create_task(
            &team.id,
            "Internal worker continue task",
            "planner",
            json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"worker-1",
                        "goal":"finish the patch",
                        "acceptance":["tests pass"],
                        "execution":{"mode":"reconcile_loop","max_rounds":3}
                    }]
                }
            }),
            "group_chat",
            Some("internal-worker-continue"),
        )
        .await
        .expect("create task");
    let run = state
        .teams
        .create_run(
            &team.id,
            Some("ctx-internal-worker-continue"),
            json!({"task_id": task.id, "prompt":"execute reconcile loop"}),
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
        .expect("step");
    state
        .teams
        .start_step(&step.id, Some("missing-session"))
        .await
        .expect("start step");

    let authz = build_authz();
    let (worker_token, _expires_at) = authz
        .issue_access_token(
            InternalRole::Worker,
            Some("worker-1"),
            Some(&run.id),
            vec![InternalAction::StepTransition.as_str().to_string()],
            600,
        )
        .expect("issue worker step transition token");
    let (other_worker_token, _expires_at) = authz
        .issue_access_token(
            InternalRole::Worker,
            Some("worker-2"),
            Some(&run.id),
            vec![InternalAction::StepTransition.as_str().to_string()],
            600,
        )
        .expect("issue other worker token");
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let own_response = TeamInternalControl::transition_step(
        &service,
        authenticated_request(
            TransitionStepRequest {
                run_id: run.id.clone(),
                step_id: step.id.clone(),
                action: "continue".to_string(),
                remote_task_id: String::new(),
                output_json: json!({"summary":"worker keeps going"}).to_string(),
                error_text: String::new(),
                input_json: String::new(),
                reason: String::new(),
            },
            &worker_token,
        ),
    )
    .await
    .expect("worker should continue own step")
    .into_inner();
    assert_eq!(own_response.status, "working");

    let err = TeamInternalControl::transition_step(
        &service,
        authenticated_request(
            TransitionStepRequest {
                run_id: run.id.clone(),
                step_id: step.id.clone(),
                action: "continue".to_string(),
                remote_task_id: String::new(),
                output_json: json!({"summary":"other worker should fail"}).to_string(),
                error_text: String::new(),
                input_json: String::new(),
                reason: String::new(),
            },
            &other_worker_token,
        ),
    )
    .await
    .expect_err("other worker should not transition foreign step");
    assert_eq!(err.code(), Code::PermissionDenied);
    assert!(
        err.message()
            .contains("worker token cannot access step member"),
        "unexpected status: {err}"
    );
}

#[tokio::test]
async fn coordinator_transition_step_checks_run_scope_before_mutation() {
    let state = build_test_state_without_seeded_team_member_agents().await;
    let now = chrono::Utc::now().timestamp();
    let workdir =
        std::env::temp_dir().join(format!("agenthub-internal-run-scope-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&workdir).expect("create internal run-scope test workdir");
    let workdir = workdir.to_string_lossy().to_string();
    sqlx::query("INSERT OR IGNORE INTO safe_paths (path, created_at) VALUES (?1, ?2)")
        .bind(&workdir)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert run-scope safe path");
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
        .expect("insert run-scope test agent");
    }

    let team = state
        .teams
        .create_team(TeamDefinitionConfig {
            name: format!("internal-transition-run-scope-{}", Uuid::new_v4()),
            description: Some("team for run-scope step transition auth".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let (task, _) = state
        .teams
        .create_task(
            &team.id,
            "Internal scope task",
            "planner",
            json!({
                "execution_plan": {
                    "steps": [
                        {
                            "step_key":"worker-implement",
                            "member_id":"worker-1",
                            "goal":"finish implementation",
                            "acceptance":["tests pass"],
                            "execution":{"mode":"reconcile_loop","max_rounds":3}
                        }
                    ]
                }
            }),
            "group_chat",
            Some("internal-scope-task"),
        )
        .await
        .expect("create task");
    let run = state
        .teams
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"execute reconcile loop"}),
        )
        .await
        .expect("create run");
    let other_run = state
        .teams
        .create_run(
            &team.id,
            Some("other-context"),
            json!({"prompt":"other run"}),
        )
        .await
        .expect("create other run");
    let step = state
        .teams
        .list_steps(&run.id)
        .await
        .expect("list steps")
        .into_iter()
        .next()
        .expect("step");
    state
        .teams
        .start_step(&step.id, Some("missing-session"))
        .await
        .expect("start step");

    let authz = build_authz();
    let (token, _expires_at) = authz
        .issue_access_token(
            InternalRole::Coordinator,
            Some("planner"),
            Some(&other_run.id),
            vec![InternalAction::StepTransition.as_str().to_string()],
            600,
        )
        .expect("issue mismatched coordinator token");
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let err = TeamInternalControl::transition_step(
        &service,
        authenticated_request(
            TransitionStepRequest {
                run_id: other_run.id.clone(),
                step_id: step.id.clone(),
                action: "continue".to_string(),
                remote_task_id: String::new(),
                output_json: json!({"summary":"should not mutate"}).to_string(),
                error_text: String::new(),
                input_json: String::new(),
                reason: String::new(),
            },
            &token,
        ),
    )
    .await
    .expect_err("mismatched run scope should fail");
    assert_eq!(err.code(), Code::PermissionDenied);
    assert!(
        err.message()
            .contains("step does not belong to requested run scope"),
        "unexpected status: {err}"
    );

    let step_after = state.teams.get_step(&step.id).await.expect("reload step");
    assert_eq!(step_after.status, crate::team::TeamStepStatus::Working);
    assert_eq!(step_after.output, None);
}

#[test]
fn constant_time_eq_matches_ordinary_byte_equality() {
    use super::super::constant_time_eq;

    assert!(constant_time_eq(b"", b""));
    assert!(constant_time_eq(b"bootstrap-token", b"bootstrap-token"));
    assert!(!constant_time_eq(b"bootstrap-token", b"bootstrap-toke"));
    assert!(!constant_time_eq(b"bootstrap-token", b"bootstrap-tokeX"));
    assert!(!constant_time_eq(b"bootstrap-token", b"Xootstrap-token"));
    assert!(!constant_time_eq(b"short", b"much-longer-value"));
}
