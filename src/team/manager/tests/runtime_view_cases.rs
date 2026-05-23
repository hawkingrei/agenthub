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

#[tokio::test]
async fn run_context_read_models_reflect_actor_and_session_state() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "run-context-read-model-team".to_string(),
            description: Some("team for run context read models".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
                    {"member_id":"worker","role":"worker"},
                    {"member_id":"reviewer","role":"reviewer"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-read-models"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");
    manager
        .append_run_event(&run.id, "operator_note", json!({"text":"checkpoint"}))
        .await
        .expect("append run event");

    let pending = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "coordinator",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "worker",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "all",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"type":"chat_message","text":"please take this"}),
            idempotency_key: Some("read-model-pending"),
            message_kind: None,
        })
        .await
        .expect("send pending actor message");
    let delivered = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "coordinator",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "all",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"type":"chat_message","text":"please review this"}),
            idempotency_key: Some("read-model-delivered"),
            message_kind: None,
        })
        .await
        .expect("send delivered actor message");
    manager
        .ack_actor_message(&run.id, "reviewer", delivered.message_id)
        .await
        .expect("ack reviewer message");

    sqlx::query("INSERT INTO agents (id, name, workdir, command, args, worktree_mode, code_mode, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)")
        .bind("worker")
        .bind("Worker")
        .bind("/tmp/worker")
        .bind("agent")
        .bind("[]")
        .bind("off")
        .bind(1_i64)
        .bind("running")
        .bind(10_i64)
        .bind(10_i64)
        .execute(&db)
        .await
        .expect("insert worker agent");
    sqlx::query("INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at) VALUES (?1, ?2, ?3, ?4, NULL)")
        .bind("session-worker-live")
        .bind("worker")
        .bind("running")
        .bind(10_i64)
        .execute(&db)
        .await
        .expect("insert live worker session");

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let latest_event_id = events
        .iter()
        .map(|event| event.event_id)
        .max()
        .expect("run should have events");

    let fingerprint = manager
        .read_run_context_fingerprint(&run.id)
        .await
        .expect("read run fingerprint");
    assert_eq!(fingerprint.team_id, team.id);
    assert_eq!(fingerprint.run_id, run.id);
    assert_eq!(fingerprint.run_status, "submitted");
    assert_eq!(fingerprint.latest_event_id, latest_event_id);
    assert_eq!(fingerprint.latest_mailbox_message_id, delivered.message_id);
    assert_eq!(fingerprint.mailbox_pending, 1);
    assert_eq!(fingerprint.mailbox_delivered, 1);
    assert_eq!(fingerprint.mailbox_dead_letter, 0);

    let pending_by_actor = manager
        .list_actor_pending_counts_by_actor(&run.id)
        .await
        .expect("list pending counts by actor");
    assert_eq!(pending_by_actor.get("worker"), Some(&1));
    assert_eq!(pending_by_actor.get("reviewer"), None);

    let all_pending = manager
        .list_pending_actor_unread_counts()
        .await
        .expect("list pending unread counts");
    assert!(all_pending.iter().any(|record| {
        record.run_id == run.id && record.actor_id == "worker" && record.unread_count == 1
    }));
    assert!(
        !all_pending
            .iter()
            .any(|record| { record.run_id == run.id && record.actor_id == "reviewer" })
    );

    assert_eq!(
        manager
            .member_role_for_run(&run.id, "worker")
            .await
            .expect("read worker role"),
        Some("worker".to_string())
    );
    assert_eq!(
        manager
            .member_role_for_run(&run.id, "missing")
            .await
            .expect("read missing role"),
        None
    );
    assert_eq!(
        manager
            .member_role_for_run("missing-run", "worker")
            .await
            .expect("read missing run role"),
        None
    );

    assert_eq!(
        manager
            .get_agent_session_status("session-worker-live")
            .await
            .expect("read session status"),
        Some("running".to_string())
    );
    assert_eq!(
        manager
            .get_agent_session_status("missing-session")
            .await
            .expect("read missing session status"),
        None
    );
    assert_eq!(
        manager
            .get_live_member_session("worker")
            .await
            .expect("read live worker session"),
        Some(("session-worker-live".to_string(), "running".to_string()))
    );
    assert_eq!(
        manager
            .get_live_member_session("missing-worker")
            .await
            .expect("read missing live session"),
        None
    );

    let mismatch = manager
        .describe_team_context(Some("wrong-team"), Some(&run.id))
        .await
        .expect_err("explicit team mismatch should be rejected");
    assert!(mismatch.to_string().contains("wrong-team"));
    assert_eq!(pending.status, TeamActorMessageStatus::Pending);
}
