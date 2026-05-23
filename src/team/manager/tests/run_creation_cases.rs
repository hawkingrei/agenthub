use super::*;

#[tokio::test]
async fn create_run_marks_linked_task_in_progress() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-run-team".to_string(),
            description: Some("team with linked task run".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Compile linked plan",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("linked-run"),
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"run linked task"}),
        )
        .await
        .expect("create linked run");
    assert_eq!(run.status, TeamRunStatus::Submitted);

    let reloaded = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(reloaded.status, TeamTaskStatus::InProgress);
}

#[tokio::test]
async fn create_run_materializes_input_step_template_into_run_steps() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "input-step-template-run-team".to_string(),
            description: Some("team with run input step template".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let run = manager
        .create_run(
            &team.id,
            Some("ctx-step-template"),
            json!({
                "step_template": [
                    {
                        "step_key":"coordinator-plan",
                        "member_id":"coordinator",
                        "execution":{"mode":"single_pass"}
                    },
                    {
                        "step_key":"worker-implement",
                        "member_id":"worker-1",
                        "depends_on":["coordinator-plan"],
                        "goal":"finish the patch",
                        "acceptance":["tests pass"],
                        "execution":{"mode":"reconcile_loop","max_rounds":5}
                    }
                ]
            }),
        )
        .await
        .expect("create run");

    let steps = manager
        .list_steps(&run.id)
        .await
        .expect("list materialized steps");
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].step_key, "coordinator-plan");
    assert_eq!(steps[0].member_id, "coordinator");
    assert!(steps[0].depends_on.is_empty());
    assert_eq!(steps[0].input, None);
    assert_eq!(steps[1].step_key, "worker-implement");
    assert_eq!(steps[1].member_id, "worker-1");
    assert_eq!(steps[1].depends_on, vec!["coordinator-plan".to_string()]);
    assert_eq!(
        steps[1].input,
        Some(json!({
            "task_execution_step": {
                "goal":"finish the patch",
                "acceptance":["tests pass"],
                "execution":{"mode":"reconcile_loop","max_rounds":5},
                "round_state":{"current_round":0}
            }
        }))
    );

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let documents = wait_for_archive_run_event_documents(&archive, &run.id, events.len()).await;
    let archived_event_types = archived_run_event_types(&documents, &events);
    assert!(
        archived_event_types.contains(&"run_submitted"),
        "run_submitted should be archived after run creation commits"
    );
    assert_eq!(
        archived_event_types
            .iter()
            .filter(|event_type| **event_type == "step_submitted")
            .count(),
        2,
        "materialized step_submitted events should be archived after run creation commits"
    );
}

#[tokio::test]
async fn create_run_materializes_linked_task_execution_plan_when_input_has_no_step_template() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-execution-plan-run-team".to_string(),
            description: Some("team with linked task execution plan".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Execution-plan task",
            "coordinator",
            json!({
                "execution_plan": {
                    "steps": [
                        {
                            "step_key":"coordinator-plan",
                            "member_id":"coordinator",
                            "execution":{"mode":"single_pass"}
                        },
                        {
                            "step_key":"worker-implement",
                            "member_id":"worker-1",
                            "depends_on":["coordinator-plan"],
                            "goal":"finish implementation",
                            "acceptance":["tests pass","review notes addressed"],
                            "execution":{"mode":"reconcile_loop","max_rounds":4}
                        }
                    ]
                }
            }),
            "group_chat",
            Some("execution-plan"),
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some("ctx-linked-task-plan"),
            json!({"task_id": task.id, "prompt":"run linked task"}),
        )
        .await
        .expect("create run");

    let steps = manager.list_steps(&run.id).await.expect("list steps");
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].step_key, "coordinator-plan");
    assert_eq!(steps[1].step_key, "worker-implement");
    assert_eq!(steps[1].depends_on, vec!["coordinator-plan".to_string()]);
    assert_eq!(
        steps[1].input,
        Some(json!({
            "task_execution_step": {
                "goal":"finish implementation",
                "acceptance":["tests pass","review notes addressed"],
                "execution":{"mode":"reconcile_loop","max_rounds":4},
                "round_state":{"current_round":0}
            }
        }))
    );
}

#[tokio::test]
async fn create_run_rejects_invalid_input_step_template_member_scope() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "invalid-step-template-run-team".to_string(),
            description: Some("team with invalid run input step template".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let err = manager
        .create_run(
            &team.id,
            Some("ctx-invalid-step-template"),
            json!({
                "step_template": [{
                    "step_key":"worker-implement",
                    "member_id":"worker-2",
                    "goal":"finish the patch",
                    "acceptance":["tests pass"],
                    "execution":{"mode":"reconcile_loop","max_rounds":5}
                }]
            }),
        )
        .await
        .expect_err("invalid step template should fail");
    assert!(
        err.to_string()
            .contains("run input step_template[].member_id must reference"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn create_run_hides_cross_team_linked_task_lookup_details() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team_a = manager
        .create_team(TeamDefinitionConfig {
            name: "run-team-a".to_string(),
            description: Some("requesting team".to_string()),
            spec: json!({"entrypoint":"coordinator","members":[{"member_id":"coordinator","role":"coordinator"}]}),
        })
        .await
        .expect("create team a");
    let team_b = manager
        .create_team(TeamDefinitionConfig {
            name: "run-team-b".to_string(),
            description: Some("foreign team".to_string()),
            spec: json!({"entrypoint":"coordinator","members":[{"member_id":"coordinator","role":"coordinator"}]}),
        })
        .await
        .expect("create team b");

    let (foreign_task, _) = manager
        .create_task(
            &team_b.id,
            "Foreign task",
            "coordinator",
            json!({"source":"foreign"}),
            "group_chat",
            Some("foreign-task"),
        )
        .await
        .expect("create foreign task");

    let wrong_team_err = manager
        .create_run(
            &team_a.id,
            Some("ctx-cross-team"),
            json!({"task_id": foreign_task.id, "prompt":"run foreign task"}),
        )
        .await
        .expect_err("cross-team task should fail");
    let missing_task_err = manager
        .create_run(
            &team_a.id,
            Some("ctx-missing-task"),
            json!({"task_id": "missing-task", "prompt":"run missing task"}),
        )
        .await
        .expect_err("missing task should fail");

    assert_eq!(
        wrong_team_err.to_string(),
        "linked task does not belong to the requested team"
    );
    assert_eq!(wrong_team_err.to_string(), missing_task_err.to_string());
}

#[tokio::test]
async fn create_team_and_run_records_submission_event() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "review-team".to_string(),
            description: Some("team for review tasks".to_string()),
            spec: json!({"entrypoint":"triage","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    assert_eq!(team.name, "review-team");

    let run = manager
        .create_run(&team.id, None, json!({"prompt":"check plan"}))
        .await
        .expect("create run");
    assert_eq!(run.status, crate::team::TeamRunStatus::Submitted);
    assert_eq!(run.input["continuity"]["mode"], json!("inherit_recent"));

    let row = sqlx::query(
        "SELECT event_type, run_id, payload_json FROM team_run_events WHERE run_id = ?1 ORDER BY id ASC LIMIT 1",
    )
    .bind(&run.id)
    .fetch_one(&db)
    .await
    .expect("read run event");
    let event_type: String = row.get("event_type");
    let run_id: String = row.get("run_id");
    let payload_json: String = row.get("payload_json");
    let payload: serde_json::Value =
        serde_json::from_str(&payload_json).expect("decode run_submitted payload");
    assert_eq!(event_type, "run_submitted");
    assert_eq!(run_id, run.id);
    assert_eq!(payload["continuity_mode"], json!("inherit_recent"));
}
