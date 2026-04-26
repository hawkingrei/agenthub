use super::*;

#[tokio::test]
async fn issue_node_credential_returns_phase0_metadata() {
    let state = build_test_state().await;
    let before = chrono::Utc::now().timestamp();
    sqlx::query(
        r#"
        CREATE TABLE agent_nodes (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            grpc_target TEXT NOT NULL,
            tls_server_name TEXT,
            last_seen_at INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(&state.db)
    .await
    .expect("create agent_nodes table");
    sqlx::query(
        r#"
        INSERT INTO agent_nodes (
            id,
            name,
            grpc_target,
            tls_server_name,
            last_seen_at,
            created_at,
            updated_at
        )
        VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6)
        "#,
    )
    .bind("node-a")
    .bind("Node A")
    .bind("https://node-a.internal:50051")
    .bind("node-a.internal")
    .bind(before - 60)
    .bind(before - 60)
    .execute(&state.db)
    .await
    .expect("insert agent node");
    let authz = build_authz();
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );
    let mut request = Request::new(IssueNodeCredentialRequest {
        node_id: "node-a".to_string(),
        role: "leader".to_string(),
        actor_id: String::new(),
        run_id: String::new(),
        permissions: vec![InternalAction::AgentManage.as_str().to_string()],
        ttl_seconds: 600,
    });
    request.metadata_mut().insert(
        BOOTSTRAP_TOKEN_HEADER,
        MetadataValue::try_from("bootstrap-token").expect("bootstrap metadata"),
    );

    let response = TeamInternalControl::issue_node_credential(&service, request)
        .await
        .expect("issue node credential")
        .into_inner();
    assert_eq!(response.node_id, "node-a");
    assert_eq!(response.source_node_id, "node-a");
    assert_eq!(response.cluster_id, "agenthub");
    assert_eq!(response.scope, vec!["agent:manage", "node:p2p"]);
    assert_eq!(response.audience, vec!["agenthub-internal"]);
    assert!(response.kid.starts_with("shared-hs256-"));
    assert!(response.issued_at > 0);
    assert!(response.expires_at > response.issued_at);
    let last_seen_at: Option<i64> =
        sqlx::query_scalar("SELECT last_seen_at FROM agent_nodes WHERE id = ?1")
            .bind("node-a")
            .fetch_one(&state.db)
            .await
            .expect("load node last_seen_at");
    assert!(last_seen_at.is_some_and(|value| value >= before));
}

#[tokio::test]
async fn issue_node_credential_rejects_bootstrap_token_mismatch() {
    let state = build_test_state().await;
    sqlx::query(
        r#"
        CREATE TABLE agent_nodes (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            grpc_target TEXT NOT NULL,
            tls_server_name TEXT,
            last_seen_at INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(&state.db)
    .await
    .expect("create agent_nodes table");
    let authz = build_authz();
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );
    let mut request = Request::new(IssueNodeCredentialRequest {
        node_id: "node-a".to_string(),
        role: "leader".to_string(),
        actor_id: String::new(),
        run_id: String::new(),
        permissions: vec![InternalAction::AgentManage.as_str().to_string()],
        ttl_seconds: 600,
    });
    request.metadata_mut().insert(
        BOOTSTRAP_TOKEN_HEADER,
        MetadataValue::try_from("wrong-token").expect("bootstrap metadata"),
    );

    let err = TeamInternalControl::issue_node_credential(&service, request)
        .await
        .expect_err("mismatched bootstrap token should fail");
    assert_eq!(err.code(), Code::PermissionDenied);
    assert_eq!(err.message(), "bootstrap token mismatch");
}

#[tokio::test]
async fn issue_node_credential_requires_worker_actor_and_run() {
    let state = build_test_state().await;
    sqlx::query(
        r#"
        CREATE TABLE agent_nodes (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            grpc_target TEXT NOT NULL,
            tls_server_name TEXT,
            last_seen_at INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(&state.db)
    .await
    .expect("create agent_nodes table");
    let authz = build_authz();
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );

    let mut missing_actor_request = Request::new(IssueNodeCredentialRequest {
        node_id: "node-a".to_string(),
        role: "worker".to_string(),
        actor_id: String::new(),
        run_id: "run-1".to_string(),
        permissions: vec![InternalAction::AgentManage.as_str().to_string()],
        ttl_seconds: 600,
    });
    missing_actor_request.metadata_mut().insert(
        BOOTSTRAP_TOKEN_HEADER,
        MetadataValue::try_from("bootstrap-token").expect("bootstrap metadata"),
    );
    let missing_actor_err =
        TeamInternalControl::issue_node_credential(&service, missing_actor_request)
            .await
            .expect_err("worker bootstrap should require actor_id");
    assert_eq!(missing_actor_err.code(), Code::InvalidArgument);
    assert_eq!(
        missing_actor_err.message(),
        "worker bootstrap requires actor_id"
    );

    let mut missing_run_request = Request::new(IssueNodeCredentialRequest {
        node_id: "node-a".to_string(),
        role: "worker".to_string(),
        actor_id: "worker-a".to_string(),
        run_id: String::new(),
        permissions: vec![InternalAction::AgentManage.as_str().to_string()],
        ttl_seconds: 600,
    });
    missing_run_request.metadata_mut().insert(
        BOOTSTRAP_TOKEN_HEADER,
        MetadataValue::try_from("bootstrap-token").expect("bootstrap metadata"),
    );
    let missing_run_err = TeamInternalControl::issue_node_credential(&service, missing_run_request)
        .await
        .expect_err("worker bootstrap should require run_id");
    assert_eq!(missing_run_err.code(), Code::InvalidArgument);
    assert_eq!(
        missing_run_err.message(),
        "worker bootstrap requires run_id"
    );
}
