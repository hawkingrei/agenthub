use super::*;
use crate::acp::AcpActorSkillContext;

#[tokio::test]
async fn internal_grpc_team_context_and_task_controls_are_wire_compatible() {
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Leader, Some("planner"), Some(&run.id));
    let service = TeamInternalControlService::new(
        state,
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let context = TeamInternalControl::describe_team_context(
        &service,
        authenticated_request(
            DescribeTeamContextRequest {
                team_id: String::new(),
                run_id: run.id.clone(),
                actor_id: "planner".to_string(),
            },
            &token,
        ),
    )
    .await
    .expect("describe team context")
    .into_inner();
    let context_json: serde_json::Value =
        serde_json::from_str(&context.context_json).expect("decode team context");
    assert_eq!(context_json["team_id"], json!(run.team_id));
    assert_eq!(context_json["run"]["run_id"], json!(run.id));
    assert_eq!(context_json["runtime"]["member_count"], json!(2));
    assert!(
        context_json["members"]
            .as_array()
            .expect("members array")
            .iter()
            .any(|member| member["member_id"] == json!("planner"))
    );
    assert!(
        context_json["members"]
            .as_array()
            .expect("members array")
            .iter()
            .any(|member| member["member_id"] == json!("reviewer"))
    );

    let created = TeamInternalControl::create_team_task(
        &service,
        authenticated_request(
            CreateTeamTaskRequest {
                team_id: run.team_id.clone(),
                actor_id: "planner".to_string(),
                title: "Investigate authority-only actor CLI".to_string(),
                status: "in_progress".to_string(),
                topic: "actor-cli".to_string(),
                context_json: json!({"goal":"remove sqlite fallback"}).to_string(),
            },
            &token,
        ),
    )
    .await
    .expect("create team task")
    .into_inner();
    let created_json: serde_json::Value =
        serde_json::from_str(&created.output_json).expect("decode created task output");
    let task_id = created_json["task"]["id"]
        .as_str()
        .expect("task id")
        .to_string();
    assert_eq!(created_json["task"]["status"], json!("in_progress"));
    assert_eq!(
        created_json["task"]["created_by_actor_id"],
        json!("planner")
    );
    assert_eq!(
        created_json["task"]["assigned_member_id"],
        serde_json::Value::Null
    );
    assert_eq!(created_json["conversation"]["topic"], json!("actor-cli"));

    let listed = TeamInternalControl::list_team_tasks(
        &service,
        authenticated_request(
            ListTeamTasksRequest {
                team_id: run.team_id.clone(),
                actor_id: "planner".to_string(),
                limit: 20,
                status: "in_progress".to_string(),
                include_shared_thread: false,
                run_id: String::new(),
                task_id: task_id.clone(),
                assigned_member_id: String::new(),
                topic: "actor-cli".to_string(),
            },
            &token,
        ),
    )
    .await
    .expect("list team tasks")
    .into_inner();
    let listed_tasks: Vec<TeamTaskRecord> =
        serde_json::from_str(&listed.tasks_json).expect("decode task list");
    let created_task = listed_tasks
        .iter()
        .find(|task| task.id == task_id)
        .expect("created task in filtered list");
    assert_eq!(created_task.status, crate::team::TeamTaskStatus::InProgress);
    assert!(created_task.assigned_member_id.is_none());

    let updated = TeamInternalControl::update_team_task(
        &service,
        authenticated_request(
            UpdateTeamTaskRequest {
                team_id: run.team_id.clone(),
                actor_id: "planner".to_string(),
                task_id: task_id.clone(),
                status: Some("completed".to_string()),
                assigned_member_id: Some("reviewer".to_string()),
                clear_assigned_member_id: false,
                context_json: None,
                context_merge_json: Some(json!({"repo":"agenthub","issue":128}).to_string()),
            },
            &token,
        ),
    )
    .await
    .expect("update team task")
    .into_inner();
    let updated_task: TeamTaskRecord =
        serde_json::from_str(&updated.task_json).expect("decode updated task");
    assert_eq!(updated_task.id, task_id);
    assert_eq!(updated_task.status, crate::team::TeamTaskStatus::Completed);
    assert_eq!(updated_task.assigned_member_id.as_deref(), Some("reviewer"));
    assert_eq!(updated_task.context["repo"], json!("agenthub"));
    assert_eq!(updated_task.context["issue"], json!(128));

    let detail = TeamInternalControl::get_team_task(
        &service,
        authenticated_request(
            GetTeamTaskRequest {
                team_id: String::new(),
                run_id: run.id.clone(),
                actor_id: "planner".to_string(),
                task_id: task_id.clone(),
                message_limit: 10,
            },
            &token,
        ),
    )
    .await
    .expect("get team task detail")
    .into_inner();
    let detail: TeamTaskDetailRecord =
        serde_json::from_str(&detail.detail_json).expect("decode team task detail");
    assert_eq!(detail.task.id, task_id);
    assert_eq!(detail.conversation.topic.as_deref(), Some("actor-cli"));
    assert_eq!(detail.latest_run.as_ref().map(|run| run.id.as_str()), None);
    assert!(detail.recent_messages.is_empty());

    let note = TeamInternalControl::append_team_task_note(
        &service,
        authenticated_request(
            AppendTeamTaskNoteRequest {
                team_id: String::new(),
                run_id: "missing-run".to_string(),
                actor_id: "planner".to_string(),
                task_id: detail.task.id.clone(),
                kind: "result".to_string(),
                text: "implemented".to_string(),
            },
            &token,
        ),
    )
    .await
    .expect_err("run_id must reference a run when team_id is omitted");
    assert_eq!(note.code(), Code::NotFound);

    let note = TeamInternalControl::append_team_task_note(
        &service,
        authenticated_request(
            AppendTeamTaskNoteRequest {
                team_id: detail.task.team_id.clone(),
                run_id: String::new(),
                actor_id: "planner".to_string(),
                task_id: detail.task.id.clone(),
                kind: "result".to_string(),
                text: "implemented".to_string(),
            },
            &token,
        ),
    )
    .await
    .expect("append team task note")
    .into_inner();
    let note_json: serde_json::Value =
        serde_json::from_str(&note.message_json).expect("decode note");
    assert_eq!(note_json["payload"]["kind"], json!("result"));
    assert_eq!(note_json["payload"]["text"], json!("implemented"));

    let cleared = TeamInternalControl::update_team_task(
        &service,
        authenticated_request(
            UpdateTeamTaskRequest {
                team_id: updated_task.team_id.clone(),
                actor_id: "planner".to_string(),
                task_id: updated_task.id.clone(),
                status: None,
                assigned_member_id: None,
                clear_assigned_member_id: true,
                context_json: Some(json!({"owner":"leader"}).to_string()),
                context_merge_json: None,
            },
            &token,
        ),
    )
    .await
    .expect("clear task assignee")
    .into_inner();
    let cleared_task: TeamTaskRecord =
        serde_json::from_str(&cleared.task_json).expect("decode cleared task");
    assert_eq!(cleared_task.assigned_member_id, None);
    assert_eq!(cleared_task.context["owner"], json!("leader"));
}

#[tokio::test]
async fn internal_grpc_describe_team_context_reconciles_stale_running_member_sessions() {
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Leader, Some("planner"), Some(&run.id));
    let service = TeamInternalControlService::new(
        state.clone(),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let now = chrono::Utc::now().timestamp();
    sqlx::query("INSERT OR IGNORE INTO safe_paths (path, created_at) VALUES (?1, ?2)")
        .bind("/tmp/internal-stale-planner")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert safe path for stale planner");
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref,
            code_mode, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, 'use_existing', NULL, NULL, 0, 'running', ?6, ?7)
        "#,
    )
    .bind("planner")
    .bind("planner")
    .bind("/tmp/internal-stale-planner")
    .bind("agenthub-codex-acp")
    .bind("[]")
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert stale planner agent");
    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
        VALUES (?1, ?2, 'running', ?3, NULL)
        "#,
    )
    .bind("internal-stale-planner-session")
    .bind("planner")
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert stale planner session");

    let response = TeamInternalControl::describe_team_context(
        &service,
        authenticated_request(
            DescribeTeamContextRequest {
                team_id: String::new(),
                run_id: run.id.clone(),
                actor_id: "planner".to_string(),
            },
            &token,
        ),
    )
    .await
    .expect("describe reconciled team context")
    .into_inner();
    let context_json: serde_json::Value =
        serde_json::from_str(&response.context_json).expect("decode reconciled team context");
    assert_eq!(context_json["runtime"]["status"], json!("stopped"));
    let planner = context_json["members"]
        .as_array()
        .expect("members array")
        .iter()
        .find(|member| member["member_id"] == json!("planner"))
        .expect("planner member record");
    assert_eq!(planner["agent_status"], json!("exited"));
    assert_eq!(planner["session_id"], serde_json::Value::Null);
    assert_eq!(planner["session_status"], serde_json::Value::Null);

    let agent_status: String = sqlx::query_scalar("SELECT status FROM agents WHERE id = ?1")
        .bind("planner")
        .fetch_one(&state.db)
        .await
        .expect("load reconciled internal planner status");
    assert_eq!(agent_status, "exited");
}

#[tokio::test]
async fn internal_grpc_describe_team_context_rejects_non_member_before_reconcile() {
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let authz = build_authz();
    let token = issue_token(
        &authz,
        InternalRole::Worker,
        Some("intruder"),
        Some(&run.id),
    );
    let service = TeamInternalControlService::new(
        state.clone(),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let now = chrono::Utc::now().timestamp();
    sqlx::query("INSERT OR IGNORE INTO safe_paths (path, created_at) VALUES (?1, ?2)")
        .bind("/tmp/internal-unauthorized-stale-planner")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert safe path for unauthorized stale planner");
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref,
            code_mode, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, 'use_existing', NULL, NULL, 0, 'running', ?6, ?7)
        "#,
    )
    .bind("planner")
    .bind("planner")
    .bind("/tmp/internal-unauthorized-stale-planner")
    .bind("agenthub-codex-acp")
    .bind("[]")
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert unauthorized stale planner agent");
    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
        VALUES (?1, ?2, 'running', ?3, NULL)
        "#,
    )
    .bind("internal-unauthorized-stale-planner-session")
    .bind("planner")
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert unauthorized stale planner session");

    let err = TeamInternalControl::describe_team_context(
        &service,
        authenticated_request(
            DescribeTeamContextRequest {
                team_id: String::new(),
                run_id: run.id,
                actor_id: "intruder".to_string(),
            },
            &token,
        ),
    )
    .await
    .expect_err("non-member actor should be rejected before reconciliation");
    assert_eq!(err.code(), Code::PermissionDenied);
    assert_eq!(err.message(), "current actor is not a member of this team");

    let agent_status: String = sqlx::query_scalar("SELECT status FROM agents WHERE id = ?1")
        .bind("planner")
        .fetch_one(&state.db)
        .await
        .expect("load planner status after unauthorized describe");
    assert_eq!(agent_status, "running");

    let session_status: String =
        sqlx::query_scalar("SELECT status FROM agent_sessions WHERE id = ?1")
            .bind("internal-unauthorized-stale-planner-session")
            .fetch_one(&state.db)
            .await
            .expect("load planner session after unauthorized describe");
    assert_eq!(session_status, "running");
}

#[tokio::test]
async fn internal_grpc_describe_team_context_rejects_invalid_scope_inputs() {
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Leader, Some("planner"), None);
    let service = TeamInternalControlService::new(
        state.clone(),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let missing_selector_err = TeamInternalControl::describe_team_context(
        &service,
        authenticated_request(
            DescribeTeamContextRequest {
                team_id: String::new(),
                run_id: String::new(),
                actor_id: "planner".to_string(),
            },
            &token,
        ),
    )
    .await
    .expect_err("missing team/run selector should fail");
    assert_eq!(missing_selector_err.code(), Code::InvalidArgument);
    assert_eq!(
        missing_selector_err.message(),
        "team_id or run_id is required"
    );

    let other_team = state
        .teams
        .create_team(TeamDefinitionConfig {
            name: format!("other-team-{}", Uuid::new_v4()),
            description: Some("other team".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "leader_member_id":"planner",
                "members":[
                    {"member_id":"planner","role":"leader"},
                    {"member_id":"reviewer","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create other team");
    let mismatch_err = TeamInternalControl::describe_team_context(
        &service,
        authenticated_request(
            DescribeTeamContextRequest {
                team_id: other_team.id,
                run_id: run.id,
                actor_id: "planner".to_string(),
            },
            &token,
        ),
    )
    .await
    .expect_err("mismatched team/run should fail");
    assert_eq!(mismatch_err.code(), Code::InvalidArgument);
    assert!(mismatch_err.message().contains("belongs to team"));
}

#[tokio::test]
async fn internal_grpc_resolve_actor_run_scope_prefers_running_actor_context() {
    let state = build_test_state().await;
    let authz = build_authz();
    let service = TeamInternalControlService::new(
        state.clone(),
        authz.clone(),
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );
    let token = issue_token(&authz, InternalRole::Worker, Some("planner"), None);
    let actor_cli_path = crate::acp::default_actor_cli_path().expect("resolve actor cli path");

    let session_id = state
        .agents
        .start_agent_with_actor_context(
            "planner",
            Some(AcpActorSkillContext {
                team_id: Some("team-runtime-scope".to_string()),
                current_run_id: Some("run-runtime-scope".to_string()),
                actor_id: "planner".to_string(),
                default_channel: "default".to_string(),
                actor_cli_path,
                member_role: Some("leader".to_string()),
                member_skills: Vec::new(),
                contract_version: None,
                continuity: None,
            }),
        )
        .await
        .expect("start scoped planner runtime");

    let response = TeamInternalControl::resolve_actor_run_scope(
        &service,
        authenticated_request(
            ResolveActorRunScopeRequest {
                actor_id: "planner".to_string(),
                team_id: String::new(),
            },
            &token,
        ),
    )
    .await
    .expect("resolve actor run scope from runtime")
    .into_inner();
    assert_eq!(response.run_id, "run-runtime-scope");
    assert_eq!(response.team_id, "team-runtime-scope");
    assert_eq!(response.source, "actor_runtime");

    state
        .agents
        .stop_agent("planner")
        .await
        .expect("stop scoped planner runtime");
    let stopped_session = state
        .agents
        .live_session_id_for_agent("planner")
        .await
        .expect("load live session after stop");
    assert_ne!(stopped_session.as_deref(), Some(session_id.as_str()));
}

#[tokio::test]
async fn internal_grpc_resolve_actor_run_scope_falls_back_to_unique_active_team_run() {
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Worker, Some("planner"), None);
    let service = TeamInternalControlService::new(
        state,
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let response = TeamInternalControl::resolve_actor_run_scope(
        &service,
        authenticated_request(
            ResolveActorRunScopeRequest {
                actor_id: "planner".to_string(),
                team_id: run.team_id.clone(),
            },
            &token,
        ),
    )
    .await
    .expect("resolve unique team run scope")
    .into_inner();
    assert_eq!(response.run_id, run.id);
    assert_eq!(response.team_id, run.team_id);
    assert_eq!(response.source, "team_active_run");
}

#[tokio::test]
async fn internal_grpc_resolve_actor_run_scope_rejects_ambiguous_active_team_runs() {
    let state = build_test_state().await;
    let team = state
        .teams
        .create_team(TeamDefinitionConfig {
            name: format!("ambiguous-run-scope-{}", Uuid::new_v4()),
            description: Some("team to verify ambiguous run scope hints".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "leader_member_id":"planner",
                "members":[
                    {"member_id":"planner","role":"leader"},
                    {"member_id":"reviewer","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create test team");
    let first_run = state
        .teams
        .create_run(&team.id, Some("ctx-1"), json!({"prompt":"first"}))
        .await
        .expect("create first run");
    let second_run = state
        .teams
        .create_run(&team.id, Some("ctx-2"), json!({"prompt":"second"}))
        .await
        .expect("create second run");
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Worker, Some("planner"), None);
    let service = TeamInternalControlService::new(
        state,
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let err = TeamInternalControl::resolve_actor_run_scope(
        &service,
        authenticated_request(
            ResolveActorRunScopeRequest {
                actor_id: "planner".to_string(),
                team_id: team.id.clone(),
            },
            &token,
        ),
    )
    .await
    .expect_err("ambiguous team run scope should fail");
    assert_eq!(err.code(), Code::FailedPrecondition);
    assert!(err.message().contains(first_run.id.as_str()));
    assert!(err.message().contains(second_run.id.as_str()));
}
