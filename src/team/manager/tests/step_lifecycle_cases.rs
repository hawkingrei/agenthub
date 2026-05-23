use super::*;

#[tokio::test]
async fn step_lifecycle_transitions_persist_and_emit_events() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "step-team".to_string(),
            description: Some("team with step lifecycle".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-step"), json!({"payload":"start"}))
        .await
        .expect("create run");

    let step = manager
        .submit_step(
            &run.id,
            "plan_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"draft plan"})),
        )
        .await
        .expect("submit step");
    assert_eq!(step.status, TeamStepStatus::Submitted);

    let working = manager
        .start_step(&step.id, Some("remote-task-1"))
        .await
        .expect("start step");
    assert_eq!(working.status, TeamStepStatus::Working);
    assert_eq!(working.runtime_handle_id.as_deref(), Some("remote-task-1"));
    assert!(working.started_at.is_some());

    let run_after_start = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_start.status, TeamRunStatus::Working);
    assert!(run_after_start.started_at.is_some());

    let completed = manager
        .complete_step(&step.id, Some(json!({"result":"ok"})))
        .await
        .expect("complete step");
    assert_eq!(completed.status, TeamStepStatus::Completed);
    assert_eq!(completed.output, Some(json!({"result":"ok"})));
    assert!(completed.ended_at.is_some());

    let continuity = manager
        .get_member_continuity_state(&team.id, "planner")
        .await
        .expect("get continuity state")
        .expect("continuity state should exist");
    assert_eq!(continuity.team_id, team.id);
    assert_eq!(continuity.member_id, "planner");
    assert_eq!(continuity.source_run_id, run.id);
    assert_eq!(
        continuity.source_session_id.as_deref(),
        Some("remote-task-1")
    );
    assert!(continuity.summary_text.contains("ok"));
    assert_eq!(continuity.history_window["schema_version"], json!(1));

    let run_after_complete = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_complete.status, TeamRunStatus::Completed);
    assert!(run_after_complete.ended_at.is_some());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let step_working_event = events
        .iter()
        .find(|event| event.event_type == "step_working")
        .expect("step_working event should exist");
    assert_eq!(
        step_working_event.payload["runtime_handle_id"],
        json!("remote-task-1")
    );
    assert!(
        step_working_event.payload.get("remote_task_id").is_none(),
        "step_working payload should not expose legacy remote_task_id"
    );
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
            "continuity_state_updated",
            "run_completed"
        ]
    );

    let documents =
        wait_for_archive_run_event_documents(&archive, &run.id, event_types.len()).await;
    let mut archived_event_types = archived_run_event_types(&documents, &events);
    let mut expected_event_types = event_types.clone();
    archived_event_types.sort_unstable();
    expected_event_types.sort_unstable();
    assert_eq!(archived_event_types, expected_event_types);
}

#[tokio::test]
async fn complete_step_offloads_large_output_to_workspace_context_artifact() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!("agenthub-context-artifact-{unique_suffix}"));
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
    .bind("planner")
    .bind("planner")
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
    .expect("insert planner agent");

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "artifact-team".to_string(),
            description: Some("team with large continuity output".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-artifact"), json!({"payload":"start"}))
        .await
        .expect("create run");
    let step = manager
        .submit_step(
            &run.id,
            "large_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"emit large output"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("remote-task-artifact"))
        .await
        .expect("start step");

    let large_text = "x".repeat(12_000);
    manager
        .complete_step(
            &step.id,
            Some(json!({
                "summary":"large payload",
                "details": large_text,
                "api_key":"secret-value"
            })),
        )
        .await
        .expect("complete step");

    let continuity = manager
        .get_member_continuity_state(&team.id, "planner")
        .await
        .expect("get continuity state")
        .expect("continuity state should exist");
    let pointer = continuity
        .history_window
        .get("artifact_pointer")
        .expect("artifact pointer should exist for oversized output");
    let pointer_path = pointer
        .get("path")
        .and_then(|value| value.as_str())
        .expect("artifact pointer path should be string");
    assert!(
        pointer_path.starts_with(&format!(".cache/context/run/{}/artifact-", run.id)),
        "unexpected pointer path: {pointer_path}"
    );

    let artifact_row = sqlx::query(
        r#"
        SELECT artifact_path, artifact_size_bytes
        FROM team_context_artifacts
        WHERE run_id = ?1 AND member_id = ?2
        ORDER BY artifact_seq DESC
        LIMIT 1
        "#,
    )
    .bind(&run.id)
    .bind("planner")
    .fetch_one(&db)
    .await
    .expect("fetch context artifact row");
    let artifact_path: String = artifact_row.get("artifact_path");
    let artifact_size: i64 = artifact_row.get("artifact_size_bytes");
    assert!(artifact_size > 0);
    assert!(
        std::path::Path::new(&artifact_path).exists(),
        "artifact path should exist: {artifact_path}"
    );
    let artifact_content =
        std::fs::read_to_string(&artifact_path).expect("read persisted artifact content");
    assert!(
        artifact_content.contains("[redacted]"),
        "sensitive keys should be redacted in persisted artifact"
    );
    let state_path = workspace.join(".cache/context/state.md");
    let state_text = std::fs::read_to_string(&state_path).expect("read runtime state snapshot");
    let note_relative_path = format!(
        ".cache/context/run/{}/continuity.md",
        continuity.source_run_id
    );
    let note_path = workspace
        .join(".cache/context/run")
        .join(&continuity.source_run_id)
        .join("continuity.md");
    let note_text = std::fs::read_to_string(&note_path).expect("read runtime continuity note");
    assert!(state_text.contains("# Team Runtime State"));
    assert!(state_text.contains("- schema_family: team_runtime_state"));
    assert!(state_text.contains("- schema_version: 1"));
    assert!(state_text.contains("- team_id:"));
    assert!(state_text.contains("- member_id: planner"));
    assert!(state_text.contains("- current_execution_run_id:"));
    assert!(state_text.contains("- continuity_mode: inherit_recent"));
    assert!(state_text.contains(format!("- continuity_note_path: {note_relative_path}").as_str()));
    assert!(state_text.contains(pointer_path));
    assert!(note_text.contains("# Team Continuity Note"));
    assert!(note_text.contains("- schema_family: team_continuity_note"));
    assert!(note_text.contains("- schema_version: 1"));
    assert!(note_text.contains("- current_execution_run_id:"));
    assert!(note_text.contains("- continuity_source_execution_run_id:"));
    assert!(note_text.contains("## Summary"));
    assert!(note_text.contains("large payload"));
    assert!(note_text.contains("## History Window"));

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let continuity_event = events
        .iter()
        .find(|event| event.event_type == "continuity_state_updated")
        .expect("continuity_state_updated event should exist");
    assert_eq!(
        continuity_event.payload["source_runtime_handle_id"],
        json!("remote-task-artifact")
    );
    assert!(
        continuity_event.payload.get("source_session_id").is_none(),
        "continuity_state_updated should not expose legacy source_session_id"
    );
    assert_eq!(
        continuity_event.payload["artifact_offload_status"],
        json!("persisted")
    );
    assert!(
        continuity_event.payload.get("artifact_pointer").is_some(),
        "continuity_state_updated should include artifact pointer metadata"
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn complete_step_keeps_success_when_runtime_state_snapshot_write_fails() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let workspace =
        std::env::temp_dir().join(format!("agenthub-context-state-write-fail-{unique_suffix}"));
    std::fs::create_dir_all(workspace.join(".cache/context"))
        .expect("create workspace context directory");
    std::fs::create_dir_all(workspace.join(".cache/context/state.md"))
        .expect("create conflicting state snapshot path");
    let workspace_text = workspace.to_string_lossy().to_string();
    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("planner")
    .bind("planner")
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
    .expect("insert planner agent");

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "state-write-fail-team".to_string(),
            description: Some("team with conflicting runtime state path".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-write-fail"), json!({"payload":"start"}))
        .await
        .expect("create run");
    let step = manager
        .submit_step(
            &run.id,
            "state_write_fail_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"exercise best-effort state snapshot write"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("remote-task-state-write-fail"))
        .await
        .expect("start step");

    let completed = manager
        .complete_step(
            &step.id,
            Some(json!({
                "summary":"best effort snapshot",
                "details":"state snapshot should not block completion"
            })),
        )
        .await
        .expect("complete step should succeed despite snapshot write failure");

    assert_eq!(completed.status, TeamStepStatus::Completed);
    let continuity = manager
        .get_member_continuity_state(&team.id, "planner")
        .await
        .expect("get continuity state")
        .expect("continuity state should exist");
    assert_eq!(continuity.summary_text, "best effort snapshot");

    let run_after_complete = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_complete.status, TeamRunStatus::Completed);
    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn complete_step_offloads_large_output_to_coordinator_runtime_workspace_context_artifact() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!(
        "agenthub-coordinator-context-artifact-{unique_suffix}"
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
    .bind("planner")
    .bind("planner")
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
    .expect("insert planner agent");

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "coordinator-artifact-team".to_string(),
            description: Some("team with coordinator continuity output".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner","role":"coordinator"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-artifact"), json!({"payload":"start"}))
        .await
        .expect("create run");
    let step = manager
        .submit_step(
            &run.id,
            "large_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"emit large output"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("remote-task-artifact"))
        .await
        .expect("start step");

    manager
        .complete_step(
            &step.id,
            Some(json!({
                "summary":"large payload",
                "details": "x".repeat(12_000),
                "api_key":"secret-value"
            })),
        )
        .await
        .expect("complete step");

    let artifact_row = sqlx::query(
        r#"
        SELECT artifact_path
        FROM team_context_artifacts
        WHERE run_id = ?1 AND member_id = ?2
        ORDER BY artifact_seq DESC
        LIMIT 1
        "#,
    )
    .bind(&run.id)
    .bind("planner")
    .fetch_one(&db)
    .await
    .expect("fetch context artifact row");
    let artifact_path: String = artifact_row.get("artifact_path");
    let expected_runtime_workdir = derive_team_runtime_workdir(
        &workspace_text,
        &AcpActorSkillContext {
            team_id: Some(team.id.clone()),
            current_run_id: None,
            actor_id: "planner".to_string(),
            default_channel: DEFAULT_ACTOR_CHANNEL.to_string(),
            member_role: Some("coordinator".to_string()),
            member_skills: Vec::new(),
            contract_version: None,
            continuity: None,
        },
        &WorktreeMode::UseExisting,
    );
    let expected_prefix = std::path::Path::new(&expected_runtime_workdir)
        .join(".cache/context/run")
        .to_string_lossy()
        .to_string();
    assert!(
        artifact_path.starts_with(&expected_prefix),
        "artifact path should be under derived coordinator runtime workspace: {artifact_path}"
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn flush_run_context_persists_artifact_and_then_noops_with_checkpoint() {
    let db = setup_test_db().await;
    let event_dbs = AgentEventDbRouter::new(std::env::temp_dir().join(format!(
        "agenthub-team-flush-eventdb-{}",
        uuid::Uuid::new_v4()
    )));
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        event_dbs.clone(),
        Some(archive.clone()),
    );

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!("agenthub-memory-flush-{unique_suffix}"));
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
    .bind("planner")
    .bind("planner")
    .bind(&workspace_text)
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("running")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert planner agent");

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "flush-team".to_string(),
            description: Some("team with flushable context".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-flush"), json!({"payload":"start"}))
        .await
        .expect("create run");
    let step = manager
        .submit_step(
            &run.id,
            "flush_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"flush"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("session-flush-1"))
        .await
        .expect("start step");

    let event_db = event_dbs
        .pool_for_agent("planner")
        .await
        .expect("open planner event db");
    sqlx::query(
        r#"
        INSERT INTO agent_events (session_id, seq, ts, stream, message)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind("session-flush-1")
    .bind("1")
    .bind(100_i64)
    .bind("acp")
    .bind(r#"{"type":"agent_message","content":"first signal","api_key":"secret"}"#)
    .execute(&event_db)
    .await
    .expect("insert first agent event");
    sqlx::query(
        r#"
        INSERT INTO agent_events (session_id, seq, ts, stream, message)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind("session-flush-1")
    .bind("2")
    .bind(101_i64)
    .bind("system")
    .bind("plain text event")
    .execute(&event_db)
    .await
    .expect("insert second agent event");

    let first = manager
        .flush_run_context(
            &run.id,
            crate::team::TeamMemoryFlushRequest {
                member_id: "planner".to_string(),
                session_id: None,
                trigger: "manual".to_string(),
                max_events: None,
            },
        )
        .await
        .expect("flush context first time");
    assert_eq!(first.status, "persisted");
    assert_eq!(first.flushed_events, 2);
    assert!(first.artifact_pointer.is_some());
    assert_eq!(first.reason, None);

    let checkpoint_event_id: i64 = sqlx::query_scalar(
        r#"
        SELECT last_event_id
        FROM team_context_flush_checkpoint
        WHERE run_id = ?1 AND member_id = ?2 AND session_id = ?3
        "#,
    )
    .bind(&run.id)
    .bind("planner")
    .bind("session-flush-1")
    .fetch_one(&db)
    .await
    .expect("fetch checkpoint");
    assert!(checkpoint_event_id > 0);

    let second = manager
        .flush_run_context(
            &run.id,
            crate::team::TeamMemoryFlushRequest {
                member_id: "planner".to_string(),
                session_id: Some("session-flush-1".to_string()),
                trigger: "manual".to_string(),
                max_events: None,
            },
        )
        .await
        .expect("flush context second time");
    assert_eq!(second.status, "noop");
    assert_eq!(second.reason.as_deref(), Some("no_new_events"));
    assert_eq!(second.flushed_events, 0);

    let events = manager
        .list_run_events(&run.id, 200, None)
        .await
        .expect("list run events");
    let event_types = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>();
    assert!(
        event_types.contains(&"memory_flush_started"),
        "memory_flush_started event should be recorded"
    );
    assert!(
        event_types.contains(&"memory_flush_persisted"),
        "memory_flush_persisted event should be recorded"
    );
    assert!(
        event_types.contains(&"memory_flush_noop"),
        "memory_flush_noop event should be recorded"
    );

    let documents = wait_for_archive_run_event_documents(&archive, &run.id, events.len()).await;
    let archived_event_types = archived_run_event_types(&documents, &events);
    assert!(
        archived_event_types.contains(&"run_submitted"),
        "run_submitted should still be archived"
    );
    assert!(
        archived_event_types.contains(&"memory_flush_started"),
        "memory_flush_started should be archived after transaction commit"
    );
    assert_eq!(
        archived_event_types
            .iter()
            .filter(|event_type| **event_type == "memory_flush_started")
            .count(),
        2,
        "each flush attempt should archive its own memory_flush_started event"
    );
    assert!(
        archived_event_types.contains(&"memory_flush_persisted"),
        "memory_flush_persisted should be archived after transaction commit"
    );
    assert!(
        archived_event_types.contains(&"memory_flush_noop"),
        "memory_flush_noop should be archived after transaction commit"
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn flush_run_context_fails_when_session_mapping_missing() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "flush-missing-session-team".to_string(),
            description: Some("team with no session mapping".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-missing-session"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let result = manager
        .flush_run_context(
            &run.id,
            crate::team::TeamMemoryFlushRequest {
                member_id: "planner".to_string(),
                session_id: None,
                trigger: "manual".to_string(),
                max_events: None,
            },
        )
        .await
        .expect("flush context should return failed result");
    assert_eq!(result.status, "failed");
    assert_eq!(result.reason.as_deref(), Some("session_mapping_missing"));
    assert!(result.artifact_pointer.is_none());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let event_types = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>();
    assert!(
        event_types.contains(&"memory_flush_started"),
        "memory_flush_started event should be recorded"
    );
    assert!(
        event_types.contains(&"memory_flush_failed"),
        "memory_flush_failed event should be recorded"
    );

    let documents = wait_for_archive_documents(&archive, 3).await;
    let archived_event_types = documents
        .iter()
        .filter(|document| {
            document.source_kind == MessageDocumentKind::TeamRunEvent
                && document.run_id.as_deref() == Some(run.id.as_str())
        })
        .map(|document| document.body_text.as_str())
        .collect::<Vec<_>>();
    assert!(
        archived_event_types.contains(&"memory_flush_started"),
        "memory_flush_started should be archived for failed flush attempts"
    );
    assert!(
        archived_event_types.contains(&"memory_flush_failed"),
        "memory_flush_failed should be archived after transaction commit"
    );
}

#[tokio::test]
async fn input_required_and_resume_transitions_update_run_and_emit_events() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "input-required-team".to_string(),
            description: Some("team requiring manual input".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-input"), json!({"payload":"start"}))
        .await
        .expect("create run");
    let step = manager
        .submit_step(
            &run.id,
            "input_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"collect feedback"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("remote-task-input"))
        .await
        .expect("start step");

    let input_required = manager
        .set_step_input_required(
            &step.id,
            Some("approval is required"),
            Some(json!({"question":"approve?"})),
        )
        .await
        .expect("set input required");
    assert_eq!(input_required.status, TeamStepStatus::InputRequired);
    assert_eq!(
        input_required.error_text.as_deref(),
        Some("approval is required")
    );
    assert_eq!(
        input_required.input,
        Some(json!({"goal":"collect feedback","question":"approve?"}))
    );

    let run_after_input_required = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(
        run_after_input_required.status,
        TeamRunStatus::InputRequired
    );

    let resumed = manager
        .resume_step(&step.id, Some(json!({"answer":"approved"})))
        .await
        .expect("resume step");
    assert_eq!(resumed.status, TeamStepStatus::Working);
    assert!(resumed.error_text.is_none());
    assert_eq!(
        resumed.input,
        Some(json!({
            "goal":"collect feedback",
            "question":"approve?",
            "answer":"approved"
        }))
    );

    let run_after_resume = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_resume.status, TeamRunStatus::Working);

    let _ = manager
        .complete_step(&step.id, Some(json!({"result":"done"})))
        .await
        .expect("complete step");
    let run_after_complete = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_complete.status, TeamRunStatus::Completed);

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let step_resumed_event = events
        .iter()
        .find(|event| event.event_type == "step_resumed")
        .expect("step_resumed event should exist");
    assert_eq!(
        step_resumed_event.payload["runtime_handle_id"],
        json!("remote-task-input")
    );
    assert!(
        step_resumed_event.payload.get("remote_task_id").is_none(),
        "step_resumed payload should not expose legacy remote_task_id"
    );

    let continuity_event = events
        .iter()
        .find(|event| event.event_type == "continuity_state_updated")
        .expect("continuity_state_updated event should exist");
    assert_eq!(
        continuity_event.payload["source_runtime_handle_id"],
        json!("remote-task-input")
    );
    assert!(
        continuity_event.payload.get("source_session_id").is_none(),
        "continuity_state_updated should not expose legacy source_session_id"
    );

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
            "continuity_state_updated",
            "run_completed"
        ]
    );

    let documents =
        wait_for_archive_run_event_documents(&archive, &run.id, event_types.len()).await;
    let archived_event_types = archived_run_event_types(&documents, &events);
    assert!(
        archived_event_types.contains(&"run_input_required"),
        "run_input_required should be archived after transaction commit"
    );
    assert!(
        archived_event_types.contains(&"step_input_required"),
        "step_input_required should be archived after transaction commit"
    );
    assert!(
        archived_event_types.contains(&"step_resumed"),
        "step_resumed should be archived after transaction commit"
    );
}
