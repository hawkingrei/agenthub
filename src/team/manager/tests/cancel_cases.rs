use super::*;

#[tokio::test]
async fn cancel_run_updates_status_and_emits_event() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "cancel-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"main","members":[]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-1"), json!({"payload":1}))
        .await
        .expect("create run");

    let canceled = manager.cancel_run(&run.id).await.expect("cancel run");
    assert_eq!(canceled.status, crate::team::TeamRunStatus::Canceled);
    assert!(canceled.ended_at.is_some());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, "run_submitted");
    assert_eq!(events[1].event_type, "run_canceled");
}

#[tokio::test]
async fn cancel_run_only_cancels_active_steps() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "cancel-active-step-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-cancel-steps"), json!({"payload":1}))
        .await
        .expect("create run");

    let completed_step = manager
        .submit_step(
            &run.id,
            "already_done",
            "planner",
            Vec::new(),
            Some(json!({"goal":"done"})),
        )
        .await
        .expect("submit completed step");
    let active_step = manager
        .submit_step(
            &run.id,
            "still_running",
            "planner",
            Vec::new(),
            Some(json!({"goal":"running"})),
        )
        .await
        .expect("submit active step");
    let _ = manager
        .start_step(&completed_step.id, Some("remote-completed"))
        .await
        .expect("start completed step");
    let _ = manager
        .start_step(&active_step.id, Some("remote-active"))
        .await
        .expect("start active step");
    let _ = manager
        .complete_step(&completed_step.id, Some(json!({"result":"ok"})))
        .await
        .expect("complete step");

    let canceled_run = manager.cancel_run(&run.id).await.expect("cancel run");
    assert_eq!(canceled_run.status, TeamRunStatus::Canceled);

    let completed_after_cancel = manager
        .get_step(&completed_step.id)
        .await
        .expect("get completed step");
    assert_eq!(completed_after_cancel.status, TeamStepStatus::Completed);

    let active_after_cancel = manager
        .get_step(&active_step.id)
        .await
        .expect("get active step");
    assert_eq!(active_after_cancel.status, TeamStepStatus::Canceled);
    assert!(active_after_cancel.ended_at.is_some());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let canceled_step_ids: Vec<String> = events
        .iter()
        .filter(|event| event.event_type == "step_canceled")
        .filter_map(|event| event.step_id.clone())
        .collect();
    assert_eq!(canceled_step_ids, vec![active_step.id]);

    let documents = wait_for_archive_run_event_documents(&archive, &run.id, events.len()).await;
    let archived_event_types = archived_run_event_types(&documents, &events);
    assert!(
        archived_event_types.contains(&"step_canceled"),
        "step_canceled should be archived after transaction commit"
    );
    assert!(
        archived_event_types.contains(&"run_canceled"),
        "run_canceled should be archived after transaction commit"
    );
}
