use super::*;

#[tokio::test]
async fn list_active_runs_returns_non_terminal_runs_only() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "active-runs-team".to_string(),
            description: Some("team to verify active run listing".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let submitted_run = manager
        .create_run(
            &team.id,
            Some("ctx-submitted"),
            json!({"payload":"submitted"}),
        )
        .await
        .expect("create submitted run");

    let canceled_run = manager
        .create_run(
            &team.id,
            Some("ctx-canceled"),
            json!({"payload":"canceled"}),
        )
        .await
        .expect("create canceled run");
    let _ = manager
        .cancel_run(&canceled_run.id)
        .await
        .expect("cancel run");

    let working_run = manager
        .create_run(&team.id, Some("ctx-working"), json!({"payload":"working"}))
        .await
        .expect("create working run");
    let working_step = manager
        .submit_step(
            &working_run.id,
            "work",
            "planner",
            Vec::new(),
            Some(json!({"goal":"start"})),
        )
        .await
        .expect("submit working step");
    let _ = manager
        .start_step(&working_step.id, Some("remote-working"))
        .await
        .expect("start working step");

    let active_runs = manager
        .list_active_runs(100)
        .await
        .expect("list active runs");
    let active_ids: Vec<&str> = active_runs.iter().map(|run| run.id.as_str()).collect();
    assert!(active_ids.contains(&submitted_run.id.as_str()));
    assert!(active_ids.contains(&working_run.id.as_str()));
    assert!(!active_ids.contains(&canceled_run.id.as_str()));
}

#[tokio::test]
async fn list_active_runs_for_team_excludes_shared_thread_mailbox_runs() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "active-runs-team-filtered".to_string(),
            description: Some("team to verify per-team active run listing".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let visible_run = manager
        .create_run(&team.id, Some("ctx-visible"), json!({"payload":"visible"}))
        .await
        .expect("create visible run");
    let shared_mailbox_run = manager
        .ensure_shared_thread_mailbox_run(&team.id, "shared-thread-task", "conversation-all")
        .await
        .expect("create shared mailbox run");
    sqlx::query(
        "UPDATE team_runs SET status = 'working', started_at = COALESCE(started_at, ?1) WHERE id = ?2",
    )
    .bind(chrono::Utc::now().timestamp())
    .bind(&shared_mailbox_run.id)
    .execute(&db)
    .await
    .expect("promote shared mailbox run to active status");

    let active_runs = manager
        .list_active_runs_for_team(&team.id, 20)
        .await
        .expect("list active runs for team");
    let active_ids: Vec<&str> = active_runs.iter().map(|run| run.id.as_str()).collect();
    assert_eq!(active_ids, vec![visible_run.id.as_str()]);
}

#[tokio::test]
async fn cancel_active_runs_on_startup_requires_manual_restart() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "startup-cancel-team".to_string(),
            description: Some("team to verify startup active-run cancellation".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let submitted_run = manager
        .create_run(
            &team.id,
            Some("ctx-startup-submitted"),
            json!({"payload":"submitted"}),
        )
        .await
        .expect("create submitted run");
    let working_run = manager
        .create_run(
            &team.id,
            Some("ctx-startup-working"),
            json!({"payload":"working"}),
        )
        .await
        .expect("create working run");
    let working_step = manager
        .submit_step(
            &working_run.id,
            "work",
            "planner",
            Vec::new(),
            Some(json!({"goal":"start"})),
        )
        .await
        .expect("submit working step");
    let _ = manager
        .start_step(&working_step.id, Some("remote-startup-working"))
        .await
        .expect("start working step");

    let canceled_count = manager
        .cancel_active_runs_on_startup()
        .await
        .expect("cancel active runs on startup");
    assert_eq!(canceled_count, 2);

    let submitted_after = manager
        .get_run(&submitted_run.id)
        .await
        .expect("get submitted run after startup cancel");
    assert_eq!(submitted_after.status, TeamRunStatus::Canceled);

    let working_after = manager
        .get_run(&working_run.id)
        .await
        .expect("get working run after startup cancel");
    assert_eq!(working_after.status, TeamRunStatus::Canceled);

    let working_step_after = manager
        .get_step(&working_step.id)
        .await
        .expect("get working step after startup cancel");
    assert_eq!(working_step_after.status, TeamStepStatus::Canceled);

    let active_after = manager
        .list_active_runs(100)
        .await
        .expect("list active runs after startup cancel");
    assert!(active_after.is_empty());

    let startup_events = manager
        .list_run_events(&working_run.id, 200, None)
        .await
        .expect("list working run events")
        .into_iter()
        .filter(|event| event.event_type == "run_startup_canceled")
        .collect::<Vec<_>>();
    assert_eq!(startup_events.len(), 1);
}

#[tokio::test]
async fn cancel_active_runs_on_startup_reopens_linked_tasks() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "startup-linked-task-team".to_string(),
            description: Some("team to verify startup cancel reopens linked tasks".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Restart-safe linked task",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("startup-linked-task"),
        )
        .await
        .expect("create task");
    assert_eq!(task.status, TeamTaskStatus::Open);

    let run = manager
        .create_run(
            &team.id,
            Some(task.id.as_str()),
            json!({"task_id": task.id, "payload":"linked"}),
        )
        .await
        .expect("create linked run");
    assert_eq!(run.status, TeamRunStatus::Submitted);

    let in_progress_task = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(in_progress_task.status, TeamTaskStatus::InProgress);

    let canceled_count = manager
        .cancel_active_runs_on_startup()
        .await
        .expect("cancel active runs on startup");
    assert_eq!(canceled_count, 1);

    let run_after = manager.get_run(&run.id).await.expect("reload canceled run");
    assert_eq!(run_after.status, TeamRunStatus::Canceled);

    let reopened_task = manager
        .get_task(&task.id)
        .await
        .expect("reload reopened task");
    assert_eq!(reopened_task.status, TeamTaskStatus::Open);
    assert_eq!(reopened_task.assigned_member_id, None);
}

#[tokio::test]
async fn resume_run_handles_active_terminal_and_completed_statuses() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "resume-run-team".to_string(),
            description: Some("team to verify run resume strategy".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let submitted_run = manager
        .create_run(
            &team.id,
            Some("ctx-resume-submitted"),
            json!({"payload":"submitted"}),
        )
        .await
        .expect("create submitted run");
    let resumed_submitted = manager
        .resume_run(&submitted_run.id)
        .await
        .expect("resume submitted run");
    assert_eq!(resumed_submitted.id, submitted_run.id);

    let failed_run = manager
        .create_run(
            &team.id,
            Some("ctx-resume-failed"),
            json!({"payload":"failed"}),
        )
        .await
        .expect("create failed run");
    let failed_step = manager
        .submit_step(
            &failed_run.id,
            "step_failed",
            "planner",
            Vec::new(),
            Some(json!({"goal":"fail"})),
        )
        .await
        .expect("submit failed step");
    let _ = manager
        .start_step(&failed_step.id, Some("remote-failed"))
        .await
        .expect("start failed step");
    let _ = manager
        .fail_step(&failed_step.id, "forced fail")
        .await
        .expect("fail step");
    let resumed_failed = manager
        .resume_run(&failed_run.id)
        .await
        .expect("resume failed run");
    assert_ne!(resumed_failed.id, failed_run.id);
    assert_eq!(resumed_failed.team_id, failed_run.team_id);
    assert_eq!(resumed_failed.context_id, failed_run.context_id);
    assert_eq!(resumed_failed.input, failed_run.input);
    assert_eq!(resumed_failed.status, TeamRunStatus::Submitted);
    let failed_after_resume = manager
        .get_run(&failed_run.id)
        .await
        .expect("get original failed run");
    assert_eq!(failed_after_resume.status, TeamRunStatus::Failed);

    let canceled_run = manager
        .create_run(
            &team.id,
            Some("ctx-resume-canceled"),
            json!({"payload":"canceled"}),
        )
        .await
        .expect("create canceled run");
    let _ = manager
        .cancel_run(&canceled_run.id)
        .await
        .expect("cancel run");
    let resumed_canceled = manager
        .resume_run(&canceled_run.id)
        .await
        .expect("resume canceled run");
    assert_ne!(resumed_canceled.id, canceled_run.id);
    assert_eq!(resumed_canceled.context_id, canceled_run.context_id);
    assert_eq!(resumed_canceled.input, canceled_run.input);
    assert_eq!(resumed_canceled.status, TeamRunStatus::Submitted);

    let completed_run = manager
        .create_run(
            &team.id,
            Some("ctx-resume-completed"),
            json!({"payload":"completed"}),
        )
        .await
        .expect("create completed run");
    let completed_step = manager
        .submit_step(
            &completed_run.id,
            "step_completed",
            "planner",
            Vec::new(),
            Some(json!({"goal":"done"})),
        )
        .await
        .expect("submit completed step");
    let _ = manager
        .start_step(&completed_step.id, Some("remote-completed"))
        .await
        .expect("start completed step");
    let _ = manager
        .complete_step(&completed_step.id, Some(json!({"ok":true})))
        .await
        .expect("complete step");
    let err = manager
        .resume_run(&completed_run.id)
        .await
        .expect_err("completed run should reject resume");
    assert_eq!(
        err.downcast_ref::<TeamRunResumeError>(),
        Some(&TeamRunResumeError::CompletedRun),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn restart_run_creates_new_submission_with_same_context_and_input() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "restart-run-team".to_string(),
            description: Some("team to verify run restart strategy".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let run = manager
        .create_run(
            &team.id,
            Some("ctx-restart"),
            json!({"payload":"restart-me"}),
        )
        .await
        .expect("create source run");
    let restarted = manager.restart_run(&run.id).await.expect("restart run");

    assert_ne!(restarted.id, run.id);
    assert_eq!(restarted.team_id, run.team_id);
    assert_eq!(restarted.context_id, run.context_id);
    assert_eq!(restarted.input, run.input);
    assert_eq!(restarted.status, TeamRunStatus::Submitted);

    let events = manager
        .list_run_events(&restarted.id, 10, None)
        .await
        .expect("list restarted run events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "run_submitted");
}

#[tokio::test]
async fn restart_run_keeps_linked_task_on_same_attempt_when_already_in_progress() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "restart-run-attempt-team".to_string(),
            description: Some("team to verify restart keeps attempt projection".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Restart-safe attempt projection",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("restart-attempt"),
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"active execution push"}),
        )
        .await
        .expect("create linked run");
    let after_create = manager
        .get_task(&task.id)
        .await
        .expect("reload after create");
    assert_eq!(after_create.status, TeamTaskStatus::InProgress);
    assert_eq!(task_attempt_number(&after_create), Some(1));

    let _ = manager.restart_run(&run.id).await.expect("restart run");

    let after_restart = manager
        .get_task(&task.id)
        .await
        .expect("reload after restart");
    assert_eq!(after_restart.status, TeamTaskStatus::InProgress);
    assert_eq!(task_attempt_number(&after_restart), Some(1));
}

#[tokio::test]
async fn list_runs_supports_status_filter_and_cursor() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "list-runs-team".to_string(),
            description: Some("team to verify run listing".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let first_run = manager
        .create_run(&team.id, Some("ctx-list-runs-1"), json!({"seq": 1}))
        .await
        .expect("create first run");
    let second_run = manager
        .create_run(&team.id, Some("ctx-list-runs-2"), json!({"seq": 2}))
        .await
        .expect("create second run");
    let _ = manager
        .cancel_run(&first_run.id)
        .await
        .expect("cancel first run");
    let shared_thread_run = manager
        .ensure_shared_thread_mailbox_run(&team.id, "shared-thread-task", "conversation-all")
        .await
        .expect("create hidden shared thread mailbox run");

    sqlx::query("UPDATE team_runs SET created_at = ?1 WHERE id = ?2")
        .bind(100_i64)
        .bind(&first_run.id)
        .execute(&db)
        .await
        .expect("set first run created_at");
    sqlx::query("UPDATE team_runs SET created_at = ?1 WHERE id = ?2")
        .bind(200_i64)
        .bind(&second_run.id)
        .execute(&db)
        .await
        .expect("set second run created_at");
    sqlx::query("UPDATE team_runs SET created_at = ?1 WHERE id = ?2")
        .bind(300_i64)
        .bind(&shared_thread_run.id)
        .execute(&db)
        .await
        .expect("set shared thread run created_at");

    let all_runs = manager
        .list_runs(&team.id, 100, None, None)
        .await
        .expect("list all runs");
    assert_eq!(all_runs.len(), 2);
    assert_eq!(all_runs[0].id, second_run.id);
    assert_eq!(all_runs[0].summary, None);
    assert_eq!(all_runs[1].id, first_run.id);
    assert_eq!(
        all_runs[1].summary.as_deref(),
        Some("Run was canceled before completion.")
    );

    let canceled_runs = manager
        .list_runs(&team.id, 100, Some("canceled"), None)
        .await
        .expect("list canceled runs");
    assert_eq!(canceled_runs.len(), 1);
    assert_eq!(canceled_runs[0].id, first_run.id);

    let cursor_runs = manager
        .list_runs(&team.id, 100, None, Some(200))
        .await
        .expect("list runs with cursor");
    assert_eq!(cursor_runs.len(), 1);
    assert_eq!(cursor_runs[0].id, first_run.id);

    let limited_runs = manager
        .list_runs(&team.id, 1, None, None)
        .await
        .expect("list limited visible runs");
    assert_eq!(limited_runs.len(), 1);
    assert_eq!(limited_runs[0].id, second_run.id);

    let limited_cursor_runs = manager
        .list_runs(&team.id, 1, None, Some(200))
        .await
        .expect("list limited cursor visible runs");
    assert_eq!(limited_cursor_runs.len(), 1);
    assert_eq!(limited_cursor_runs[0].id, first_run.id);

    let hidden_run = manager
        .get_latest_run_for_task(&team.id, "shared-thread-task")
        .await
        .expect("load hidden shared thread run")
        .expect("hidden shared thread run should exist");
    assert_eq!(hidden_run.id, shared_thread_run.id);
    assert_eq!(
        hidden_run.input["bootstrap_kind"],
        Value::from("shared_thread_mailbox")
    );
}

#[tokio::test]
async fn ensure_shared_thread_mailbox_run_is_idempotent() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "shared-thread-mailbox-idempotent-team".to_string(),
            description: Some("team to verify shared thread mailbox idempotency".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let first = manager
        .ensure_shared_thread_mailbox_run(&team.id, "shared-thread-task", "conversation-all")
        .await
        .expect("create first shared thread mailbox run");
    let second = manager
        .ensure_shared_thread_mailbox_run(&team.id, "shared-thread-task", "conversation-all")
        .await
        .expect("reuse shared thread mailbox run");

    assert_eq!(first.id, second.id);

    let run_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_runs
        WHERE team_id = ?1
          AND trim(COALESCE(json_extract(input_json, '$.bootstrap_kind'), '')) = 'shared_thread_mailbox'
          AND trim(COALESCE(json_extract(input_json, '$.task_id'), '')) = 'shared-thread-task'
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count shared thread mailbox runs");
    assert_eq!(run_count, 1);

    let event_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM team_run_events WHERE run_id = ?1")
            .bind(&first.id)
            .fetch_one(&db)
            .await
            .expect("count shared thread mailbox run events");
    assert_eq!(event_count, 2);
}
