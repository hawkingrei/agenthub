use super::*;

#[tokio::test]
async fn linked_run_failure_keeps_task_in_progress() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-fail-team".to_string(),
            description: Some("team with linked run failure".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Handle linked failure",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("failure"),
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"fail linked task"}),
        )
        .await
        .expect("create linked run");
    let step = manager
        .submit_step(
            &run.id,
            "planner_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"fail linked task"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("linked-session-fail"))
        .await
        .expect("start step");
    let _ = manager
        .fail_step(&step.id, "linked run failed")
        .await
        .expect("fail step");

    let reloaded = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(reloaded.status, TeamTaskStatus::InProgress);
}

#[tokio::test]
async fn cancel_run_marks_linked_task_canceled() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-cancel-team".to_string(),
            description: Some("team with linked run cancellation".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Cancel linked run",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("cancel"),
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"cancel linked task"}),
        )
        .await
        .expect("create linked run");
    let _ = manager.cancel_run(&run.id).await.expect("cancel run");

    let reloaded = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(reloaded.status, TeamTaskStatus::Canceled);
}

#[tokio::test]
async fn startup_cancellation_reopens_linked_task() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-startup-cancel-team".to_string(),
            description: Some("team with startup run cancellation".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Resume after restart",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("startup-cancel"),
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"restart-sensitive task"}),
        )
        .await
        .expect("create linked run");
    assert_eq!(
        manager
            .get_task(&task.id)
            .await
            .expect("reload in-progress task")
            .status,
        TeamTaskStatus::InProgress
    );

    let canceled_count = manager
        .cancel_active_runs_on_startup()
        .await
        .expect("cancel active runs on startup");
    assert_eq!(canceled_count, 1);
    assert_eq!(
        manager
            .get_run(&run.id)
            .await
            .expect("reload canceled run")
            .status,
        TeamRunStatus::Canceled
    );
    assert_eq!(
        manager
            .get_task(&task.id)
            .await
            .expect("reload reopened task")
            .status,
        TeamTaskStatus::Open
    );
}

#[tokio::test]
async fn linked_run_completion_marks_task_in_review() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-complete-team".to_string(),
            description: Some("team with linked run completion".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Ship linked completion",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("complete"),
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"finish linked task"}),
        )
        .await
        .expect("create linked run");
    let step = manager
        .submit_step(
            &run.id,
            "planner_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"complete linked task"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("linked-session-complete"))
        .await
        .expect("start step");
    let _ = manager
        .complete_step(&step.id, Some(json!({"result":"done"})))
        .await
        .expect("complete step");

    let reloaded = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(reloaded.status, TeamTaskStatus::InReview);
}

#[tokio::test]
async fn linked_run_sync_preserves_waiting_tasks() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-waiting-team".to_string(),
            description: Some("team with sticky waiting tasks".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Wait for review",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            None,
        )
        .await
        .expect("create task");
    let task = manager
        .update_task_status(&task.id, TeamTaskStatus::Waiting)
        .await
        .expect("move task to waiting");
    assert_eq!(task.status, TeamTaskStatus::Waiting);

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"check for review updates"}),
        )
        .await
        .expect("create linked run");
    let after_create = manager
        .get_task(&task.id)
        .await
        .expect("reload after create");
    assert_eq!(after_create.status, TeamTaskStatus::Waiting);

    let step = manager
        .submit_step(
            &run.id,
            "planner_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"check waiting dependency"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("linked-session-waiting"))
        .await
        .expect("start step");
    let _ = manager
        .complete_step(&step.id, Some(json!({"result":"still waiting"})))
        .await
        .expect("complete step");

    let reloaded = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(reloaded.status, TeamTaskStatus::Waiting);
}

#[tokio::test]
async fn linked_run_input_required_and_resume_sync_task_waiting_transitions() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-waiting-transition-team".to_string(),
            description: Some("team with linked waiting/resume transitions".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Wait for approval and resume",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("waiting-transition"),
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"pause for approval then resume"}),
        )
        .await
        .expect("create linked run");
    let step = manager
        .submit_step(
            &run.id,
            "planner_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"wait for approval"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("linked-session-waiting-transition"))
        .await
        .expect("start step");

    let after_start = manager
        .get_task(&task.id)
        .await
        .expect("reload after start");
    assert_eq!(after_start.status, TeamTaskStatus::InProgress);
    assert_eq!(task_attempt_number(&after_start), Some(1));

    let _ = manager
        .set_step_input_required(
            &step.id,
            Some("approval is required"),
            Some(json!({"question":"approve?"})),
        )
        .await
        .expect("mark input required");
    let after_input_required = manager
        .get_task(&task.id)
        .await
        .expect("reload after input required");
    assert_eq!(after_input_required.status, TeamTaskStatus::Waiting);
    assert_eq!(task_attempt_number(&after_input_required), Some(1));

    let _ = manager
        .resume_step(&step.id, Some(json!({"answer":"approved"})))
        .await
        .expect("resume step");
    let after_resume = manager
        .get_task(&task.id)
        .await
        .expect("reload after resume");
    assert_eq!(after_resume.status, TeamTaskStatus::InProgress);
    assert_eq!(task_attempt_number(&after_resume), Some(2));

    let _ = manager
        .complete_step(&step.id, Some(json!({"result":"done"})))
        .await
        .expect("complete step");
    let after_complete = manager
        .get_task(&task.id)
        .await
        .expect("reload after complete");
    assert_eq!(after_complete.status, TeamTaskStatus::InReview);
    assert_eq!(task_attempt_number(&after_complete), Some(2));
}

#[tokio::test]
async fn cancel_run_preserves_waiting_task() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-waiting-cancel-team".to_string(),
            description: Some("team with sticky waiting cancellation".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Wait for approval",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            None,
        )
        .await
        .expect("create task");
    let task = manager
        .update_task_status(&task.id, TeamTaskStatus::Waiting)
        .await
        .expect("move task to waiting");
    assert_eq!(task.status, TeamTaskStatus::Waiting);

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"check waiting dependency"}),
        )
        .await
        .expect("create linked run");
    let _ = manager.cancel_run(&run.id).await.expect("cancel run");

    let reloaded = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(reloaded.status, TeamTaskStatus::Waiting);
    assert_eq!(task_attempt_number(&reloaded), None);
}

#[tokio::test]
async fn linked_run_create_sets_first_attempt_number() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-attempt-create-team".to_string(),
            description: Some("team with linked task attempt projection".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Attempt-number task",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("attempt-create"),
        )
        .await
        .expect("create task");
    assert_eq!(task_attempt_number(&task), None);

    let _ = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"start linked execution"}),
        )
        .await
        .expect("create linked run");

    let reloaded = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(reloaded.status, TeamTaskStatus::InProgress);
    assert_eq!(task_attempt_number(&reloaded), Some(1));
}
