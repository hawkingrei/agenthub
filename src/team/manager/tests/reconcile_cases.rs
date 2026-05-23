use super::*;

#[tokio::test]
async fn reconcile_loop_step_tracks_round_state_and_events() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "reconcile-round-team".to_string(),
            description: Some("team for reconcile round tracking".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "Reconcile task",
            "planner",
            json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"worker",
                        "goal":"finish the patch",
                        "acceptance":["tests pass"],
                        "execution":{"mode":"reconcile_loop","max_rounds":3}
                    }]
                }
            }),
            "group_chat",
            Some("reconcile"),
        )
        .await
        .expect("create task");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-reconcile"),
            json!({"task_id": task.id, "prompt":"execute reconcile step"}),
        )
        .await
        .expect("create run");
    let step = manager
        .list_steps(&run.id)
        .await
        .expect("list steps")
        .into_iter()
        .next()
        .expect("materialized step");

    let started = manager
        .start_step(&step.id, Some("session-reconcile"))
        .await
        .expect("start reconcile step");
    assert_eq!(
        started.input,
        Some(json!({
            "task_execution_step": {
                "goal":"finish the patch",
                "acceptance":["tests pass"],
                "execution":{"mode":"reconcile_loop","max_rounds":3},
                "round_state":{
                    "current_round":1,
                    "latest_status":"working"
                }
            }
        }))
    );

    let waiting = manager
        .set_step_input_required(
            &step.id,
            Some("need review"),
            Some(json!({"question":"approve?"})),
        )
        .await
        .expect("input required");
    assert_eq!(
        waiting.input,
        Some(json!({
            "task_execution_step": {
                "goal":"finish the patch",
                "acceptance":["tests pass"],
                "execution":{"mode":"reconcile_loop","max_rounds":3},
                "round_state":{
                    "current_round":1,
                    "latest_status":"input_required",
                    "latest_outcome":"input_required",
                    "latest_summary":"need review"
                }
            },
            "question":"approve?"
        }))
    );

    let resumed = manager
        .resume_step(&step.id, Some(json!({"answer":"approved"})))
        .await
        .expect("resume step");
    assert_eq!(
        resumed.input,
        Some(json!({
            "task_execution_step": {
                "goal":"finish the patch",
                "acceptance":["tests pass"],
                "execution":{"mode":"reconcile_loop","max_rounds":3},
                "round_state":{
                    "current_round":2,
                    "latest_status":"working"
                }
            },
            "question":"approve?",
            "answer":"approved"
        }))
    );

    let completed = manager
        .complete_step(
            &step.id,
            Some(json!({"summary":"patch is merge-ready","result":"done"})),
        )
        .await
        .expect("complete step");
    assert_eq!(
        completed.input,
        Some(json!({
            "task_execution_step": {
                "goal":"finish the patch",
                "acceptance":["tests pass"],
                "execution":{"mode":"reconcile_loop","max_rounds":3},
                "round_state":{
                    "current_round":2,
                    "latest_status":"completed",
                    "latest_outcome":"completed",
                    "latest_summary":"patch is merge-ready"
                }
            },
            "question":"approve?",
            "answer":"approved"
        }))
    );

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list events");
    let reconcile_events = events
        .iter()
        .filter(|event| event.event_type.starts_with("step_reconcile_round_"))
        .map(|event| (event.event_type.as_str(), event.payload.clone()))
        .collect::<Vec<_>>();
    assert_eq!(reconcile_events.len(), 4);
    assert_eq!(reconcile_events[0].0, "step_reconcile_round_started");
    assert_eq!(reconcile_events[0].1["round"], json!(1));
    assert_eq!(reconcile_events[1].0, "step_reconcile_round_finished");
    assert_eq!(reconcile_events[1].1["status"], json!("input_required"));
    assert_eq!(reconcile_events[2].0, "step_reconcile_round_started");
    assert_eq!(reconcile_events[2].1["round"], json!(2));
    assert_eq!(reconcile_events[3].0, "step_reconcile_round_finished");
    assert_eq!(reconcile_events[3].1["status"], json!("completed"));
}

#[tokio::test]
async fn continue_step_advances_reconcile_round_without_coordinator_resume() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "continue-reconcile-team".to_string(),
            description: Some("team for reconcile continue".to_string()),
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
    let (task, _) = manager
        .create_task(
            &team.id,
            "Continue reconcile task",
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
            Some("continue-reconcile"),
        )
        .await
        .expect("create task");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-continue-reconcile"),
            json!({"task_id": task.id, "prompt":"execute reconcile continue"}),
        )
        .await
        .expect("create run");
    let step = manager
        .list_steps(&run.id)
        .await
        .expect("list steps")
        .into_iter()
        .next()
        .expect("materialized step");

    let started = manager
        .start_step(&step.id, Some("session-reconcile"))
        .await
        .expect("start reconcile step");
    assert_eq!(started.status, TeamStepStatus::Working);

    let continued = manager
        .continue_step(
            &step.id,
            Some(json!({"summary":"tests still failing on lint","artifact":"round-1.log"})),
        )
        .await
        .expect("continue reconcile step");
    assert_eq!(continued.status, TeamStepStatus::Working);
    assert_eq!(
        continued.input,
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
    assert_eq!(
        continued.output,
        Some(json!({"summary":"tests still failing on lint","artifact":"round-1.log"}))
    );

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let event_types = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        event_types,
        vec![
            "run_submitted",
            "step_submitted",
            "run_working",
            "step_working",
            "step_reconcile_round_started",
            "step_continued",
            "step_reconcile_round_finished",
            "step_reconcile_round_started",
        ]
    );
    let continue_event = events
        .iter()
        .find(|event| event.event_type == "step_continued")
        .expect("step_continued event");
    assert_eq!(continue_event.payload["continued_from_round"], json!(1));
    assert_eq!(continue_event.payload["continued_to_round"], json!(2));
    assert_eq!(
        continue_event.payload["summary"],
        json!("tests still failing on lint")
    );
    let reconcile_events = events
        .iter()
        .filter(|event| event.event_type.starts_with("step_reconcile_round_"))
        .map(|event| (event.event_type.as_str(), event.payload.clone()))
        .collect::<Vec<_>>();
    assert_eq!(reconcile_events.len(), 3);
    assert_eq!(reconcile_events[0].1["round"], json!(1));
    assert_eq!(reconcile_events[1].1["round"], json!(1));
    assert_eq!(reconcile_events[1].1["status"], json!("continued"));
    assert_eq!(reconcile_events[2].1["round"], json!(2));

    let documents =
        wait_for_archive_run_event_documents(&archive, &run.id, event_types.len()).await;
    let archived_event_types = archived_run_event_types(&documents, &events);
    assert!(
        archived_event_types.contains(&"step_continued"),
        "step_continued should be archived after transaction commit"
    );
    assert_eq!(
        archived_event_types
            .iter()
            .filter(|event_type| **event_type == "step_reconcile_round_started")
            .count(),
        2,
        "both reconcile round start events should be archived"
    );
    assert!(
        archived_event_types.contains(&"step_reconcile_round_finished"),
        "step_reconcile_round_finished should be archived after transaction commit"
    );
}

#[tokio::test]
async fn continue_step_rejects_reconcile_loop_after_max_rounds() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "continue-reconcile-max-rounds-team".to_string(),
            description: Some("team for reconcile max rounds".to_string()),
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
    let (task, _) = manager
        .create_task(
            &team.id,
            "Continue reconcile max rounds task",
            "planner",
            json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"worker-1",
                        "goal":"finish the patch",
                        "acceptance":["tests pass"],
                        "execution":{"mode":"reconcile_loop","max_rounds":1}
                    }]
                }
            }),
            "group_chat",
            Some("continue-reconcile-max-rounds"),
        )
        .await
        .expect("create task");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-continue-reconcile-max-rounds"),
            json!({"task_id": task.id, "prompt":"execute reconcile continue"}),
        )
        .await
        .expect("create run");
    let step = manager
        .list_steps(&run.id)
        .await
        .expect("list steps")
        .into_iter()
        .next()
        .expect("materialized step");

    manager
        .start_step(&step.id, Some("session-reconcile"))
        .await
        .expect("start reconcile step");
    let err = manager
        .continue_step(&step.id, Some(json!({"summary":"still working"})))
        .await
        .expect_err("continue step should reject at max rounds");
    assert!(
        err.to_string().contains("max_rounds=1"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn continue_step_persists_reconcile_round_result_artifact() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!(
        "agenthub-reconcile-continue-artifact-{unique_suffix}"
    ));
    std::fs::create_dir_all(&workspace).expect("create workspace directory");
    let workspace_text = workspace.to_string_lossy().to_string();
    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("worker")
    .bind("Worker Agent")
    .bind(&workspace_text)
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("idle")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert worker agent");

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "continue-round-artifact-team".to_string(),
            description: Some("team for reconcile round artifact persistence".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "Round artifact task",
            "planner",
            json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"worker",
                        "goal":"finish the patch",
                        "acceptance":["tests pass"],
                        "execution":{"mode":"reconcile_loop","max_rounds":3}
                    }]
                }
            }),
            "group_chat",
            Some("round-artifact"),
        )
        .await
        .expect("create task");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-round-artifact"),
            json!({"task_id": task.id, "prompt":"execute reconcile continue"}),
        )
        .await
        .expect("create run");
    let step = manager
        .list_steps(&run.id)
        .await
        .expect("list steps")
        .into_iter()
        .next()
        .expect("materialized step");

    manager
        .start_step(&step.id, Some("session-reconcile"))
        .await
        .expect("start reconcile step");
    manager
        .continue_step(
            &step.id,
            Some(json!({
                "summary":"tests still failing on lint",
                "artifacts":["logs/round-1.txt"]
            })),
        )
        .await
        .expect("continue reconcile step");

    let artifact_row = sqlx::query(
        r#"
        SELECT artifact_path, artifact_size_bytes
        FROM team_context_artifacts
        WHERE run_id = ?1 AND member_id = ?2 AND artifact_kind = ?3
        ORDER BY artifact_seq DESC
        LIMIT 1
        "#,
    )
    .bind(&run.id)
    .bind("worker")
    .bind("reconcile_round_result")
    .fetch_one(&db)
    .await
    .expect("fetch reconcile round artifact row");
    let artifact_path: String = artifact_row.get("artifact_path");
    assert!(artifact_row.get::<i64, _>("artifact_size_bytes") > 0);
    let artifact_content =
        std::fs::read_to_string(&artifact_path).expect("read persisted artifact content");
    assert!(artifact_content.contains("\"status\":\"continued\""));
    assert!(artifact_content.contains("\"round\":1"));
    assert!(artifact_content.contains("tests still failing on lint"));
    assert!(
        artifact_path.starts_with(
            &workspace
                .join(".cache/context/run")
                .to_string_lossy()
                .to_string()
        ),
        "artifact path should be under worker runtime workspace: {artifact_path}"
    );

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let continue_event = events
        .iter()
        .find(|event| event.event_type == "step_continued")
        .expect("step_continued event");
    assert_eq!(
        continue_event.payload["artifact_offload_status"],
        json!("persisted")
    );
    assert!(continue_event.payload.get("artifact_pointer").is_some());
    assert!(
        continue_event.payload.get("output").is_none(),
        "step_continued should rely on artifact pointer instead of echoing full output"
    );
    let round_finished_event = events
        .iter()
        .find(|event| {
            event.event_type == "step_reconcile_round_finished"
                && event.payload["status"] == json!("continued")
        })
        .expect("continued round event");
    assert_eq!(
        round_finished_event.payload["artifact_offload_status"],
        json!("persisted")
    );
    assert!(
        round_finished_event
            .payload
            .get("artifact_pointer")
            .is_some()
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn input_required_persists_reconcile_round_result_artifact() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let workspace =
        std::env::temp_dir().join(format!("agenthub-reconcile-input-artifact-{unique_suffix}"));
    std::fs::create_dir_all(&workspace).expect("create workspace directory");
    let workspace_text = workspace.to_string_lossy().to_string();
    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("worker")
    .bind("Worker Agent")
    .bind(&workspace_text)
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("idle")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert worker agent");

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "input-round-artifact-team".to_string(),
            description: Some("team for reconcile input artifact persistence".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "Input artifact task",
            "planner",
            json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"worker",
                        "goal":"request review",
                        "acceptance":["review granted"],
                        "execution":{"mode":"reconcile_loop","max_rounds":3}
                    }]
                }
            }),
            "group_chat",
            Some("input-artifact"),
        )
        .await
        .expect("create task");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-input-artifact"),
            json!({"task_id": task.id, "prompt":"execute reconcile input-required"}),
        )
        .await
        .expect("create run");
    let step = manager
        .list_steps(&run.id)
        .await
        .expect("list steps")
        .into_iter()
        .next()
        .expect("materialized step");

    manager
        .start_step(&step.id, Some("session-reconcile"))
        .await
        .expect("start reconcile step");
    manager
        .set_step_input_required(
            &step.id,
            Some("need human review"),
            Some(json!({"question":"approve?"})),
        )
        .await
        .expect("mark input required");

    let artifact_row = sqlx::query(
        r#"
        SELECT artifact_path
        FROM team_context_artifacts
        WHERE run_id = ?1 AND member_id = ?2 AND artifact_kind = ?3
        ORDER BY artifact_seq DESC
        LIMIT 1
        "#,
    )
    .bind(&run.id)
    .bind("worker")
    .bind("reconcile_round_result")
    .fetch_one(&db)
    .await
    .expect("fetch reconcile round artifact row");
    let artifact_path: String = artifact_row.get("artifact_path");
    let artifact_content =
        std::fs::read_to_string(&artifact_path).expect("read persisted artifact content");
    assert!(artifact_content.contains("\"status\":\"input_required\""));
    assert!(artifact_content.contains("need human review"));
    assert!(artifact_content.contains("\"question\":\"approve?\""));

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let input_required_event = events
        .iter()
        .find(|event| event.event_type == "step_input_required")
        .expect("step_input_required event");
    assert_eq!(
        input_required_event.payload["artifact_offload_status"],
        json!("persisted")
    );
    assert!(
        input_required_event
            .payload
            .get("artifact_pointer")
            .is_some()
    );
    let round_finished_event = events
        .iter()
        .find(|event| {
            event.event_type == "step_reconcile_round_finished"
                && event.payload["status"] == json!("input_required")
        })
        .expect("input_required round event");
    assert_eq!(
        round_finished_event.payload["artifact_offload_status"],
        json!("persisted")
    );
    assert!(
        round_finished_event
            .payload
            .get("artifact_pointer")
            .is_some()
    );

    let _ = std::fs::remove_dir_all(workspace);
}
