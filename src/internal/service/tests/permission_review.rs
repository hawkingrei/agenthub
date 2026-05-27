use super::*;

#[tokio::test]
async fn internal_grpc_permission_review_respond_updates_pending_request() {
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Coordinator, Some("planner"), None);
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );
    let now = chrono::Utc::now().timestamp();
    sqlx::query(
            r#"
            INSERT OR IGNORE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'running', ?7, ?8)
            "#,
        )
        .bind("worker-agent")
        .bind("worker-agent")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker agent");
    sqlx::query(
        r#"
            INSERT OR IGNORE INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
    )
    .bind("worker-session")
    .bind("worker-agent")
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert worker session");
    sqlx::query(
        r#"
            INSERT INTO acp_permission_requests (
                id,
                agent_id,
                session_id,
                acp_session_id,
                team_id,
                requester_actor_id,
                requester_role,
                tool_call_id,
                options_json,
                tool_call_json,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11)
            "#,
    )
    .bind("perm-internal-1")
    .bind("worker-agent")
    .bind("worker-session")
    .bind("acp-session-1")
    .bind(&run.team_id)
    .bind("reviewer")
    .bind("worker")
    .bind("tool-call-1")
    .bind(
        json!([
            {
                "option_id": "allow",
                "name": "Allow once",
                "kind": "allow_once"
            }
        ])
        .to_string(),
    )
    .bind(json!({"tool":{"name":"mcp__fs__read"}}).to_string())
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert permission request");

    let response = TeamInternalControl::respond_permission_review(
        &service,
        authenticated_request(
            RespondPermissionReviewRequest {
                team_id: run.team_id.clone(),
                actor_id: "planner".to_string(),
                permission_id: "perm-internal-1".to_string(),
                option_id: "allow".to_string(),
                outcome: String::new(),
            },
            &token,
        ),
    )
    .await
    .expect("respond permission review")
    .into_inner();

    assert_eq!(response.status, "ok");
    assert_eq!(response.permission_id, "perm-internal-1");
    assert_eq!(response.request_status, "responded");
    assert_eq!(response.reviewed_by_actor_id, "planner");

    let row = sqlx::query(
            "SELECT status, selected_option_id, reviewed_by_actor_id FROM acp_permission_requests WHERE id = ?1",
        )
        .bind("perm-internal-1")
        .fetch_one(&state.db)
        .await
        .expect("load permission request");
    assert_eq!(row.get::<String, _>("status"), "responded");
    assert_eq!(row.get::<String, _>("selected_option_id"), "allow");
    assert_eq!(row.get::<String, _>("reviewed_by_actor_id"), "planner");
}

#[tokio::test]
async fn internal_grpc_permission_review_respond_accepts_legacy_team_coordinator_fallback() {
    let fixture = setup_permission_review_fixture_with_spec(
        "legacy-coordinator-fallback",
        "validate legacy coordinator fallback",
        json!({
            "entrypoint":"planner",
            "coordinator_member_id":"planner",
            "members":[
                {"member_id":"planner","role":"coordinator"},
                {"member_id":"reviewer","role":"worker"}
            ]
        }),
        InternalRole::Coordinator,
        "planner",
    )
    .await;
    seed_permission_review_request(
        &fixture.state,
        &fixture.run,
        PermissionReviewSeed {
            request_id: "perm-legacy-coordinator-1",
            agent_id: "legacy-worker-agent",
            session_id: "legacy-worker-session",
            acp_session_id: "acp-session-legacy-1",
            requester_actor_id: "reviewer",
            requester_role: "worker",
            review_target_actor_id: None,
            tool_call_id: "tool-call-legacy-1",
            status: "pending",
        },
        fixture.now,
    )
    .await;

    let response = TeamInternalControl::respond_permission_review(
        &fixture.service,
        authenticated_request(
            RespondPermissionReviewRequest {
                team_id: fixture.run.team_id.clone(),
                actor_id: "planner".to_string(),
                permission_id: "perm-legacy-coordinator-1".to_string(),
                option_id: "allow".to_string(),
                outcome: String::new(),
            },
            &fixture.token,
        ),
    )
    .await
    .expect("respond permission review")
    .into_inner();

    assert_eq!(response.status, "ok");
    assert_eq!(response.request_status, "responded");
    assert_eq!(response.reviewed_by_actor_id, "planner");
}

#[tokio::test]
async fn internal_grpc_permission_review_respond_accepts_legacy_team_peer_worker_fallback() {
    let fixture = setup_permission_review_fixture_with_spec(
        "legacy-peer-worker-fallback",
        "validate legacy peer worker fallback",
        json!({
            "entrypoint":"planner",
            "coordinator_member_id":"planner",
            "members":[
                {"member_id":"planner","role":"coordinator"},
                {"member_id":"requester","role":"worker"},
                {"member_id":"reviewer","role":"worker"}
            ]
        }),
        InternalRole::Worker,
        "reviewer",
    )
    .await;
    seed_permission_review_request(
        &fixture.state,
        &fixture.run,
        PermissionReviewSeed {
            request_id: "perm-legacy-peer-worker-1",
            agent_id: "legacy-peer-worker-agent",
            session_id: "legacy-peer-worker-session",
            acp_session_id: "acp-session-legacy-peer-worker-1",
            requester_actor_id: "requester",
            requester_role: "worker",
            review_target_actor_id: None,
            tool_call_id: "tool-call-legacy-peer-worker-1",
            status: "pending",
        },
        fixture.now,
    )
    .await;

    let response = TeamInternalControl::respond_permission_review(
        &fixture.service,
        authenticated_request(
            RespondPermissionReviewRequest {
                team_id: fixture.run.team_id.clone(),
                actor_id: "reviewer".to_string(),
                permission_id: "perm-legacy-peer-worker-1".to_string(),
                option_id: "allow".to_string(),
                outcome: String::new(),
            },
            &fixture.token,
        ),
    )
    .await
    .expect("respond permission review")
    .into_inner();

    assert_eq!(response.status, "ok");
    assert_eq!(response.request_status, "responded");
    assert_eq!(response.reviewed_by_actor_id, "reviewer");
}

#[tokio::test]
async fn internal_grpc_permission_review_respond_surfaces_legacy_reviewer_resolution_errors() {
    let fixture = setup_permission_review_fixture_with_spec(
        "legacy-reviewer-resolution-error",
        "validate legacy reviewer resolution errors",
        json!({
            "entrypoint":"reviewer",
            "coordinator_member_id":"reviewer",
            "members":[
                {"member_id":"reviewer","role":"worker"}
            ]
        }),
        InternalRole::Worker,
        "reviewer",
    )
    .await;
    seed_permission_review_request(
        &fixture.state,
        &fixture.run,
        PermissionReviewSeed {
            request_id: "perm-legacy-resolution-error-1",
            agent_id: "legacy-resolution-error-agent",
            session_id: "legacy-resolution-error-session",
            acp_session_id: "acp-session-legacy-resolution-error-1",
            requester_actor_id: "removed-planner",
            requester_role: "coordinator",
            review_target_actor_id: None,
            tool_call_id: "tool-call-legacy-resolution-error-1",
            status: "pending",
        },
        fixture.now,
    )
    .await;

    let err = TeamInternalControl::respond_permission_review(
        &fixture.service,
        authenticated_request(
            RespondPermissionReviewRequest {
                team_id: fixture.run.team_id.clone(),
                actor_id: "reviewer".to_string(),
                permission_id: "perm-legacy-resolution-error-1".to_string(),
                option_id: "allow".to_string(),
                outcome: String::new(),
            },
            &fixture.token,
        ),
    )
    .await
    .expect_err("legacy reviewer resolution should fail");

    assert_eq!(err.code(), tonic::Code::FailedPrecondition);
    assert!(
        err.message()
            .contains("failed to resolve active reviewer for permission request"),
        "unexpected error message: {err}"
    );
}

#[tokio::test]
async fn internal_grpc_permission_review_respond_reports_timeout_before_reviewer_check() {
    let fixture =
        setup_permission_review_fixture("timeout-review", "validate resolved review precedence")
            .await;
    seed_permission_review_request(
        &fixture.state,
        &fixture.run,
        PermissionReviewSeed {
            request_id: "perm-timeout-review-1",
            agent_id: "timeout-worker-agent",
            session_id: "timeout-worker-session",
            acp_session_id: "acp-session-timeout-1",
            requester_actor_id: "planner",
            requester_role: "coordinator",
            review_target_actor_id: None,
            tool_call_id: "tool-call-timeout-1",
            status: "timeout",
        },
        fixture.now,
    )
    .await;

    let response = TeamInternalControl::respond_permission_review(
        &fixture.service,
        authenticated_request(
            RespondPermissionReviewRequest {
                team_id: fixture.run.team_id.clone(),
                actor_id: "observer".to_string(),
                permission_id: "perm-timeout-review-1".to_string(),
                option_id: "allow".to_string(),
                outcome: String::new(),
            },
            &fixture.token,
        ),
    )
    .await
    .expect("timeout permission review should report already resolved")
    .into_inner();

    assert_eq!(response.status, "already_resolved");
    assert_eq!(response.request_status, "timeout");
    assert!(
        response.reviewed_by_actor_id.is_empty(),
        "expected no reviewer for timed-out request"
    );
}

#[tokio::test]
async fn internal_grpc_permission_review_respond_reports_persisted_reviewer_for_resolved_request() {
    let fixture =
        setup_permission_review_fixture("resolved-review", "validate resolved reviewer").await;
    seed_permission_review_request(
        &fixture.state,
        &fixture.run,
        PermissionReviewSeed {
            request_id: "perm-resolved-review-1",
            agent_id: "resolved-worker-agent",
            session_id: "resolved-worker-session",
            acp_session_id: "acp-session-resolved-1",
            requester_actor_id: "planner",
            requester_role: "coordinator",
            review_target_actor_id: None,
            tool_call_id: "tool-call-resolved-1",
            status: "responded",
        },
        fixture.now,
    )
    .await;
    sqlx::query(
        r#"
        UPDATE acp_permission_requests
        SET reviewed_by_actor_id = ?2
        WHERE id = ?1
        "#,
    )
    .bind("perm-resolved-review-1")
    .bind("reviewer")
    .execute(&fixture.state.db)
    .await
    .expect("set resolved reviewer");

    let response = TeamInternalControl::respond_permission_review(
        &fixture.service,
        authenticated_request(
            RespondPermissionReviewRequest {
                team_id: fixture.run.team_id.clone(),
                actor_id: "observer".to_string(),
                permission_id: "perm-resolved-review-1".to_string(),
                option_id: "allow".to_string(),
                outcome: String::new(),
            },
            &fixture.token,
        ),
    )
    .await
    .expect("resolved permission review should report persisted reviewer")
    .into_inner();

    assert_eq!(response.status, "already_resolved");
    assert_eq!(response.request_status, "responded");
    assert_eq!(response.reviewed_by_actor_id, "reviewer");
}

#[tokio::test]
async fn internal_grpc_permission_review_respond_keeps_pending_reviewer_guard() {
    let fixture =
        setup_permission_review_fixture("pending-review", "validate pending reviewer guard").await;
    seed_permission_review_request(
        &fixture.state,
        &fixture.run,
        PermissionReviewSeed {
            request_id: "perm-pending-review-1",
            agent_id: "pending-worker-agent",
            session_id: "pending-worker-session",
            acp_session_id: "acp-session-pending-1",
            requester_actor_id: "planner",
            requester_role: "coordinator",
            review_target_actor_id: Some("reviewer"),
            tool_call_id: "tool-call-pending-1",
            status: "pending",
        },
        fixture.now,
    )
    .await;

    let err = TeamInternalControl::respond_permission_review(
        &fixture.service,
        authenticated_request(
            RespondPermissionReviewRequest {
                team_id: fixture.run.team_id.clone(),
                actor_id: "observer".to_string(),
                permission_id: "perm-pending-review-1".to_string(),
                option_id: "allow".to_string(),
                outcome: String::new(),
            },
            &fixture.token,
        ),
    )
    .await
    .expect_err("pending permission review should reject non-reviewer actor");

    assert_eq!(err.code(), tonic::Code::PermissionDenied);
    assert!(
        err.message()
            .contains("current actor is not the active reviewer for this permission request"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn internal_grpc_permission_review_respond_rejects_requester_self_review() {
    let fixture = setup_permission_review_fixture_with_spec(
        "self-review-guard",
        "validate requester self-review guard",
        json!({
            "entrypoint":"planner",
            "coordinator_member_id":"planner",
            "members":[
                {"member_id":"planner","role":"coordinator"},
                {"member_id":"reviewer","role":"worker"},
                {"member_id":"requester","role":"worker"}
            ]
        }),
        InternalRole::Worker,
        "requester",
    )
    .await;
    seed_permission_review_request(
        &fixture.state,
        &fixture.run,
        PermissionReviewSeed {
            request_id: "perm-self-review-1",
            agent_id: "self-review-worker-agent",
            session_id: "self-review-worker-session",
            acp_session_id: "acp-session-self-review-1",
            requester_actor_id: "requester",
            requester_role: "worker",
            review_target_actor_id: Some("reviewer"),
            tool_call_id: "tool-call-self-review-1",
            status: "pending",
        },
        fixture.now,
    )
    .await;

    let err = TeamInternalControl::respond_permission_review(
        &fixture.service,
        authenticated_request(
            RespondPermissionReviewRequest {
                team_id: fixture.run.team_id.clone(),
                actor_id: "requester".to_string(),
                permission_id: "perm-self-review-1".to_string(),
                option_id: "allow".to_string(),
                outcome: String::new(),
            },
            &fixture.token,
        ),
    )
    .await
    .expect_err("requester should not be allowed to review its own permission request");

    assert_eq!(err.code(), tonic::Code::PermissionDenied);
    assert!(
        err.message()
            .contains("requester cannot review its own permission request"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn internal_grpc_permission_review_respond_rejects_conflicting_outcome_fields() {
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let authz = build_authz();
    let token = issue_token(&authz, InternalRole::Coordinator, Some("planner"), None);
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );
    let now = chrono::Utc::now().timestamp();

    sqlx::query(
            r#"
            INSERT OR IGNORE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'running', ?7, ?8)
            "#,
        )
        .bind("conflict-worker-agent")
        .bind("conflict-worker-agent")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker agent");
    sqlx::query(
        r#"
            INSERT OR IGNORE INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
    )
    .bind("conflict-worker-session")
    .bind("conflict-worker-agent")
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert worker session");
    sqlx::query(
        r#"
            INSERT INTO acp_permission_requests (
                id,
                agent_id,
                session_id,
                acp_session_id,
                team_id,
                requester_actor_id,
                requester_role,
                tool_call_id,
                options_json,
                tool_call_json,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11)
            "#,
    )
    .bind("perm-conflict-review-1")
    .bind("conflict-worker-agent")
    .bind("conflict-worker-session")
    .bind("acp-session-conflict-1")
    .bind(&run.team_id)
    .bind("reviewer")
    .bind("worker")
    .bind("tool-call-conflict-1")
    .bind(
        json!([
            {
                "option_id": "allow",
                "name": "Allow once",
                "kind": "allow_once"
            }
        ])
        .to_string(),
    )
    .bind(json!({"tool":{"name":"mcp__fs__read"}}).to_string())
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert permission request");

    let err = TeamInternalControl::respond_permission_review(
        &service,
        authenticated_request(
            RespondPermissionReviewRequest {
                team_id: run.team_id.clone(),
                actor_id: "planner".to_string(),
                permission_id: "perm-conflict-review-1".to_string(),
                option_id: "allow".to_string(),
                outcome: "cancelled".to_string(),
            },
            &token,
        ),
    )
    .await
    .expect_err("conflicting response fields should be rejected");

    assert_eq!(err.code(), tonic::Code::InvalidArgument);
    assert!(
        err.message()
            .contains("option_id and outcome cannot be set together"),
        "unexpected error: {err}"
    );
}
