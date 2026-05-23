use super::*;

#[tokio::test]
async fn list_steps_returns_sorted_steps_for_a_run() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "list-steps-team".to_string(),
            description: Some("team for step listing".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-list"), json!({"payload":"list"}))
        .await
        .expect("create run");
    let run_2 = manager
        .create_run(&team.id, Some("ctx-list-2"), json!({"payload":"list-2"}))
        .await
        .expect("create second run");

    let _ = manager
        .submit_step(
            &run.id,
            "z-step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"z"})),
        )
        .await
        .expect("submit z step");
    let _ = manager
        .submit_step(
            &run.id,
            "a-step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"a"})),
        )
        .await
        .expect("submit a step");
    let _ = manager
        .submit_step(
            &run_2.id,
            "other-run-step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"other"})),
        )
        .await
        .expect("submit step in other run");

    let listed = manager.list_steps(&run.id).await.expect("list steps");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].run_id, run.id);
    assert_eq!(listed[1].run_id, run.id);
    assert_eq!(
        listed
            .iter()
            .map(|step| step.step_key.as_str())
            .collect::<Vec<_>>(),
        vec!["a-step", "z-step"]
    );
}

#[tokio::test]
async fn run_completes_only_after_all_steps_complete() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "multi-step-team".to_string(),
            description: Some("team with two parallel steps".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"},{"member_id":"reviewer"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-multi"), json!({"payload":"start"}))
        .await
        .expect("create run");

    let step_1 = manager
        .submit_step(
            &run.id,
            "plan_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"draft"})),
        )
        .await
        .expect("submit step 1");
    let step_2 = manager
        .submit_step(
            &run.id,
            "review_step",
            "reviewer",
            vec!["plan_step".to_string()],
            Some(json!({"goal":"review"})),
        )
        .await
        .expect("submit step 2");

    let _ = manager
        .start_step(&step_1.id, Some("remote-task-1"))
        .await
        .expect("start step 1");
    let _ = manager
        .start_step(&step_2.id, Some("remote-task-2"))
        .await
        .expect("start step 2");

    let _ = manager
        .complete_step(&step_1.id, Some(json!({"result":"done-1"})))
        .await
        .expect("complete step 1");
    let run_after_first_complete = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_first_complete.status, TeamRunStatus::Working);
    assert!(run_after_first_complete.ended_at.is_none());

    let _ = manager
        .complete_step(&step_2.id, Some(json!({"result":"done-2"})))
        .await
        .expect("complete step 2");
    let run_after_second_complete = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_second_complete.status, TeamRunStatus::Completed);
    assert!(run_after_second_complete.ended_at.is_some());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let run_completed_count = events
        .iter()
        .filter(|event| event.event_type == "run_completed")
        .count();
    assert_eq!(run_completed_count, 1);
    assert_eq!(
        events.last().map(|event| event.event_type.as_str()),
        Some("run_completed")
    );
}

#[tokio::test]
async fn fail_step_updates_status_and_emits_event() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "fail-step-team".to_string(),
            description: Some("team with failure".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-fail"), json!({"payload":"start"}))
        .await
        .expect("create run");
    let step = manager
        .submit_step(
            &run.id,
            "failing_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"can fail"})),
        )
        .await
        .expect("submit step");

    let _ = manager
        .start_step(&step.id, Some("remote-task-fail"))
        .await
        .expect("start step");
    let failed = manager
        .fail_step(&step.id, "remote task failed")
        .await
        .expect("fail step");
    assert_eq!(failed.status, TeamStepStatus::Failed);
    assert_eq!(failed.error_text.as_deref(), Some("remote task failed"));

    let run_after_fail = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_fail.status, TeamRunStatus::Failed);
    assert!(run_after_fail.ended_at.is_some());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
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
            "step_failed",
            "run_failed"
        ]
    );

    let documents =
        wait_for_archive_run_event_documents(&archive, &run.id, event_types.len()).await;
    let archived_event_types = archived_run_event_types(&documents, &events);
    assert!(
        archived_event_types.contains(&"step_failed"),
        "step_failed should be archived after transaction commit"
    );
    assert!(
        archived_event_types.contains(&"run_failed"),
        "run_failed should be archived after transaction commit"
    );
}
