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
