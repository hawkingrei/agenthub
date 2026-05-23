use super::*;

#[tokio::test]
async fn describe_run_members_returns_live_roster_and_session_state() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "describe-run-members-team".to_string(),
            description: Some("team to verify run member roster".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator","description":"Lead planner"},
                    {"member_id":"worker","role":"worker","description":"Implements changes"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-team-members"), json!({"prompt":"go"}))
        .await
        .expect("create run");
    let coordinator_step = manager
        .submit_step(&run.id, "coordinator_plan", "coordinator", Vec::new(), None)
        .await
        .expect("submit coordinator step");
    let worker_step = manager
        .submit_step(
            &run.id,
            "worker_exec",
            "worker",
            vec!["coordinator_plan".to_string()],
            None,
        )
        .await
        .expect("submit worker step");

    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("coordinator")
    .bind("Coordinator Agent")
    .bind("/tmp/coordinator")
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("running")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert coordinator agent");

    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("worker")
    .bind("Worker Agent")
    .bind("/tmp/worker")
    .bind("codex")
    .bind("[]")
    .bind("create_worktree")
    .bind("manual")
    .bind("idle")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert worker agent");

    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
        VALUES (?1, ?2, ?3, ?4, NULL)
        "#,
    )
    .bind("session-coordinator")
    .bind("coordinator")
    .bind("running")
    .bind(10_i64)
    .execute(&db)
    .await
    .expect("insert coordinator session");

    manager
        .start_step(&coordinator_step.id, Some("session-coordinator"))
        .await
        .expect("start coordinator step");

    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
        VALUES (?1, ?2, ?3, ?4, NULL)
        "#,
    )
    .bind("session-worker")
    .bind("worker")
    .bind("running")
    .bind(11_i64)
    .execute(&db)
    .await
    .expect("insert worker session");

    let roster = manager
        .describe_run_members(&run.id)
        .await
        .expect("describe run members");

    assert_eq!(roster.team_id, team.id);
    assert_eq!(roster.run_id, run.id);
    assert_eq!(roster.members.len(), 2);

    let coordinator = &roster.members[0];
    assert_eq!(coordinator.member_id, "coordinator");
    assert_eq!(coordinator.display_name, "Coordinator Agent");
    assert_eq!(coordinator.role, "coordinator");
    assert_eq!(coordinator.description.as_deref(), Some("Lead planner"));
    assert_eq!(coordinator.agent_status.as_deref(), Some("running"));
    assert_eq!(
        coordinator.session_id.as_deref(),
        Some("session-coordinator")
    );
    assert_eq!(coordinator.session_status.as_deref(), Some("running"));
    assert_eq!(coordinator.card.description, "Lead planner");
    assert_eq!(coordinator.steps.len(), 1);
    assert_eq!(coordinator.steps[0].step_id, coordinator_step.id);
    assert_eq!(coordinator.steps[0].status, TeamStepStatus::Working);
    assert_eq!(
        coordinator.steps[0].session_id.as_deref(),
        Some("session-coordinator")
    );
    assert_eq!(
        coordinator.steps[0].session_status.as_deref(),
        Some("running")
    );

    let worker = &roster.members[1];
    assert_eq!(worker.member_id, "worker");
    assert_eq!(worker.display_name, "Worker Agent");
    assert_eq!(worker.role, "worker");
    assert_eq!(worker.description.as_deref(), Some("Implements changes"));
    assert_eq!(worker.agent_status.as_deref(), Some("idle"));
    assert_eq!(worker.session_id.as_deref(), Some("session-worker"));
    assert_eq!(worker.session_status.as_deref(), Some("running"));
    assert_eq!(worker.steps.len(), 1);
    assert_eq!(worker.steps[0].step_id, worker_step.id);
    assert_eq!(worker.steps[0].status, TeamStepStatus::Submitted);
    assert!(worker.steps[0].session_id.is_none());
    assert!(worker.steps[0].session_status.is_none());
}

#[tokio::test]
async fn describe_team_runtime_returns_member_runtime_status() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "describe-team-runtime".to_string(),
            description: Some("team to verify runtime status".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator","description":"Lead planner"},
                    {"member_id":"worker","role":"worker","description":"Implements changes"}
                ]
            }),
        })
        .await
        .expect("create team");

    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("coordinator")
    .bind("Coordinator Agent")
    .bind("/tmp/coordinator")
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("running")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert coordinator agent");

    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("worker")
    .bind("Worker Agent")
    .bind("/tmp/worker")
    .bind("codex")
    .bind("[]")
    .bind("create_worktree")
    .bind("manual")
    .bind("idle")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert worker agent");

    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
        VALUES (?1, ?2, ?3, ?4, NULL)
        "#,
    )
    .bind("session-coordinator")
    .bind("coordinator")
    .bind("running")
    .bind(10_i64)
    .execute(&db)
    .await
    .expect("insert coordinator session");

    let runtime = manager
        .describe_team_runtime(&team.id)
        .await
        .expect("describe team runtime");

    assert_eq!(runtime.team_id, team.id);
    assert_eq!(runtime.team_name, team.name);
    assert_eq!(runtime.status, crate::team::TeamRuntimeStatus::Degraded);
    assert_eq!(runtime.members.len(), 2);

    let coordinator = &runtime.members[0];
    assert_eq!(coordinator.member_id, "coordinator");
    assert_eq!(coordinator.display_name, "Coordinator Agent");
    assert_eq!(
        coordinator.session_id.as_deref(),
        Some("session-coordinator")
    );
    assert_eq!(coordinator.session_status.as_deref(), Some("running"));
    assert_eq!(coordinator.card.description, "Lead planner");

    let worker = &runtime.members[1];
    assert_eq!(worker.member_id, "worker");
    assert_eq!(worker.display_name, "Worker Agent");
    assert!(worker.session_id.is_none());
    assert!(worker.session_status.is_none());
    assert_eq!(worker.card.description, "Implements changes");
}

#[tokio::test]
async fn describe_team_context_merges_runtime_summary_and_optional_run_overlay() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "describe-team-context".to_string(),
            description: Some("team to verify merged context view".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator","description":"Lead planner"},
                    {"member_id":"worker","role":"worker","description":"Implements changes"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-team-context"), json!({"prompt":"go"}))
        .await
        .expect("create run");
    let coordinator_step = manager
        .submit_step(&run.id, "coordinator_plan", "coordinator", Vec::new(), None)
        .await
        .expect("submit coordinator step");

    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("coordinator")
    .bind("Coordinator Agent")
    .bind("/tmp/coordinator")
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("running")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert coordinator agent");

    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
        VALUES (?1, ?2, ?3, ?4, NULL)
        "#,
    )
    .bind("session-coordinator")
    .bind("coordinator")
    .bind("running")
    .bind(10_i64)
    .execute(&db)
    .await
    .expect("insert coordinator session");

    manager
        .start_step(&coordinator_step.id, Some("session-coordinator"))
        .await
        .expect("start coordinator step");

    manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "coordinator",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "worker",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!("## Review request\n\nPlease inspect the patch."),
            idempotency_key: Some("ctx-unread-worker"),
            message_kind: None,
        })
        .await
        .expect("send unread worker message");

    let team_context = manager
        .describe_team_context(Some(&team.id), Some(&run.id))
        .await
        .expect("describe team context");

    assert_eq!(team_context.team_id, team.id);
    assert_eq!(
        team_context.runtime.status,
        crate::team::TeamRuntimeStatus::Degraded
    );
    assert_eq!(team_context.runtime.online_count, 1);
    assert_eq!(team_context.runtime.member_count, 2);
    assert_eq!(
        team_context
            .run
            .as_ref()
            .map(|overlay| overlay.run_id.as_str()),
        Some(run.id.as_str())
    );
    assert_eq!(team_context.members.len(), 2);
    assert_eq!(team_context.members[0].display_name, "Coordinator Agent");
    assert_eq!(team_context.members[0].pending_inbox_count, 0);
    assert_eq!(team_context.members[0].steps.len(), 1);
    assert_eq!(team_context.members[1].pending_inbox_count, 1);

    let runtime_only_context = manager
        .describe_team_context(Some(&team.id), None)
        .await
        .expect("describe runtime-only team context");
    assert_eq!(runtime_only_context.team_id, team.id);
    assert_eq!(
        runtime_only_context.runtime.status,
        crate::team::TeamRuntimeStatus::Degraded
    );
    assert!(runtime_only_context.run.is_none());
    assert_eq!(runtime_only_context.members.len(), 2);
    assert_eq!(runtime_only_context.members[0].pending_inbox_count, 0);
    assert_eq!(runtime_only_context.members[1].pending_inbox_count, 0);
    assert!(runtime_only_context.members[0].steps.is_empty());
}
