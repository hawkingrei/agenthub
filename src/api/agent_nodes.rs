use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};

use crate::agent::{AgentNodeConfig, AgentNodeJoinBootstrapInfo, AgentNodeRecord, AgentNodeUpdate};
use crate::api::authz::require_root;
use crate::api::error::ApiError;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", post(create_agent_node).get(list_agent_nodes))
        .route("/bootstrap", get(get_agent_node_bootstrap))
        .route(
            "/{id}",
            get(get_agent_node)
                .patch(update_agent_node)
                .delete(delete_agent_node),
        )
        .with_state(state)
}

fn map_agent_node_error(err: anyhow::Error, missing_msg: &str) -> ApiError {
    let message = err.to_string();
    if message.contains("no rows returned by a query") || message.contains("not found") {
        return ApiError::not_found(missing_msg);
    }
    if message.contains("UNIQUE constraint failed") {
        return ApiError::conflict("agent node id or name already exists");
    }
    if message.contains("still referenced by") {
        return ApiError::conflict(&message);
    }
    if message.contains("required")
        || message.contains("must ")
        || message.contains("invalid ")
        || message.contains("reserved")
    {
        return ApiError::bad_request(&message);
    }
    ApiError::from(err)
}

async fn create_agent_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AgentNodeConfig>,
) -> Result<Json<AgentNodeRecord>, ApiError> {
    let _user = require_root(&headers, &state).await?;
    let node = state
        .agents
        .create_agent_node(payload)
        .await
        .map_err(|err| map_agent_node_error(err, "agent node not found"))?;
    Ok(Json(node))
}

async fn list_agent_nodes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentNodeRecord>>, ApiError> {
    let _user = require_root(&headers, &state).await?;
    let nodes = state.agents.list_agent_nodes().await?;
    Ok(Json(nodes))
}

async fn get_agent_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<String>,
) -> Result<Json<AgentNodeRecord>, ApiError> {
    let _user = require_root(&headers, &state).await?;
    let node = state
        .agents
        .get_agent_node(&node_id)
        .await
        .map_err(|err| map_agent_node_error(err, "agent node not found"))?;
    Ok(Json(node))
}

async fn get_agent_node_bootstrap(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AgentNodeJoinBootstrapInfo>, ApiError> {
    let _user = require_root(&headers, &state).await?;
    Ok(Json(state.agent_node_join_bootstrap.clone()))
}

async fn update_agent_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<String>,
    Json(payload): Json<AgentNodeUpdate>,
) -> Result<Json<AgentNodeRecord>, ApiError> {
    let _user = require_root(&headers, &state).await?;
    let node = state
        .agents
        .update_agent_node(&node_id, payload)
        .await
        .map_err(|err| map_agent_node_error(err, "agent node not found"))?;
    Ok(Json(node))
}

async fn delete_agent_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_root(&headers, &state).await?;
    state
        .agents
        .delete_agent_node(&node_id)
        .await
        .map_err(|err| map_agent_node_error(err, "agent node not found"))?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode, header};
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::router;
    use crate::api::team_tests::{build_test_state, create_auth_token};

    async fn create_non_root_auth_token(state: &crate::state::AppState) -> String {
        let user_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO users (id, username, display_name, role, password_hash, created_at)
            VALUES (?1, ?2, ?3, 'user', NULL, ?4)
            "#,
        )
        .bind(&user_id)
        .bind(format!("user-{}", Uuid::new_v4()))
        .bind("User")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert non-root user");
        state
            .auth
            .create_session(&user_id)
            .await
            .expect("create non-root session")
    }

    fn build_json_request(
        method: Method,
        path: &str,
        token: Option<&str>,
        payload: Option<Value>,
    ) -> Request<Body> {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        match payload {
            Some(value) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(value.to_string()))
                .expect("build json request"),
            None => builder.body(Body::empty()).expect("build request"),
        }
    }

    async fn decode_json_body(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        serde_json::from_slice(&bytes).expect("decode response json")
    }

    #[tokio::test]
    async fn agent_node_routes_require_root() {
        let state = build_test_state().await;
        let token = create_non_root_auth_token(&state).await;
        let app = router(state);

        let response = app
            .oneshot(build_json_request(Method::GET, "/", Some(&token), None))
            .await
            .expect("run non-root list agent nodes request");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = decode_json_body(response).await;
        assert_eq!(body["error"], json!("root required"));
    }

    #[tokio::test]
    async fn get_agent_node_bootstrap_requires_root() {
        let state = build_test_state().await;
        let token = create_non_root_auth_token(&state).await;
        let app = router(state);

        let response = app
            .oneshot(build_json_request(
                Method::GET,
                "/bootstrap",
                Some(&token),
                None,
            ))
            .await
            .expect("run non-root get agent node bootstrap request");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = decode_json_body(response).await;
        assert_eq!(body["error"], json!("root required"));
    }

    #[tokio::test]
    async fn get_agent_node_bootstrap_returns_root_only_join_info() {
        let mut state = build_test_state().await;
        state.agent_node_join_bootstrap = crate::agent::AgentNodeJoinBootstrapInfo {
            enabled: true,
            bootstrap_token: Some("bootstrap-token".to_string()),
            grpc_listen_addr: Some("0.0.0.0:50051".to_string()),
            security_mode: Some("tls".to_string()),
            cert_dir: Some("/etc/agenthub/internal-grpc".to_string()),
            issuer: Some("agenthub".to_string()),
            audience: Some("agenthub-internal".to_string()),
        };
        let token = create_auth_token(&state).await;
        let app = router(state);

        let response = app
            .oneshot(build_json_request(
                Method::GET,
                "/bootstrap",
                Some(&token),
                None,
            ))
            .await
            .expect("run get agent node bootstrap request");
        assert_eq!(response.status(), StatusCode::OK);
        let body = decode_json_body(response).await;
        assert_eq!(body["enabled"], json!(true));
        assert_eq!(body["bootstrap_token"], json!("bootstrap-token"));
        assert_eq!(body["grpc_listen_addr"], json!("0.0.0.0:50051"));
        assert_eq!(body["security_mode"], json!("tls"));
        assert_eq!(body["cert_dir"], json!("/etc/agenthub/internal-grpc"));
        assert_eq!(body["issuer"], json!("agenthub"));
        assert_eq!(body["audience"], json!("agenthub-internal"));
    }

    #[tokio::test]
    async fn delete_agent_node_maps_still_referenced_to_conflict() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());

        sqlx::query(
            r#"
            CREATE TABLE agent_nodes (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                grpc_target TEXT NOT NULL,
                tls_server_name TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&state.db)
        .await
        .expect("create agent_nodes table");
        sqlx::query("ALTER TABLE agents ADD COLUMN target_node_id TEXT")
            .execute(&state.db)
            .await
            .expect("add agents.target_node_id column");

        state
            .agents
            .create_agent_node(crate::agent::AgentNodeConfig {
                id: "node-east".to_string(),
                name: "Node East".to_string(),
                grpc_target: "https://node-east.internal:50051".to_string(),
                tls_server_name: Some("node-east.internal".to_string()),
                default_worktree_root: None,
            })
            .await
            .expect("create agent node");

        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO agents (
                id,
                name,
                workdir,
                command,
                args,
                target_node_id,
                worktree_mode,
                worktree_repo,
                worktree_ref,
                code_mode,
                status,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, 1, 'created', ?8, ?9)
            "#,
        )
        .bind("agent-node-ref")
        .bind("Agent Node Ref")
        .bind("/tmp/agent-node-ref")
        .bind("/bin/sh")
        .bind("[]")
        .bind("node-east")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert referencing agent");

        let response = app
            .oneshot(build_json_request(
                Method::DELETE,
                "/node-east",
                Some(&token),
                None,
            ))
            .await
            .expect("run delete agent node request");
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = decode_json_body(response).await;
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|value| value.contains("still referenced by 1 agent(s)")),
            "unexpected error body: {body}"
        );
    }

    #[tokio::test]
    async fn patch_agent_node_updates_default_worktree_root() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());

        sqlx::query(
            r#"
            CREATE TABLE agent_nodes (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                grpc_target TEXT NOT NULL,
                tls_server_name TEXT,
                default_worktree_root TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&state.db)
        .await
        .expect("create agent_nodes table");

        state
            .agents
            .create_agent_node(crate::agent::AgentNodeConfig {
                id: "node-east".to_string(),
                name: "Node East".to_string(),
                grpc_target: "https://node-east.internal:50051".to_string(),
                tls_server_name: Some("node-east.internal".to_string()),
                default_worktree_root: None,
            })
            .await
            .expect("create agent node");

        let response = app
            .oneshot(build_json_request(
                Method::PATCH,
                "/node-east",
                Some(&token),
                Some(json!({
                    "name": "Node East 2",
                    "grpc_target": "https://node-east-2.internal:50051",
                    "tls_server_name": "node-east-2.internal",
                    "default_worktree_root": "~/.agenthub/worktrees/node-east"
                })),
            ))
            .await
            .expect("run patch agent node request");
        assert_eq!(response.status(), StatusCode::OK);
        let body = decode_json_body(response).await;
        assert_eq!(body["name"], json!("Node East 2"));
        assert_eq!(
            body["grpc_target"],
            json!("https://node-east-2.internal:50051")
        );
        assert_eq!(
            body["default_worktree_root"],
            json!("~/.agenthub/worktrees/node-east")
        );
    }

    #[tokio::test]
    async fn delete_missing_agent_node_returns_not_found() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());

        sqlx::query(
            r#"
            CREATE TABLE agent_nodes (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                grpc_target TEXT NOT NULL,
                tls_server_name TEXT,
                default_worktree_root TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&state.db)
        .await
        .expect("create agent_nodes table");

        let response = app
            .oneshot(build_json_request(
                Method::DELETE,
                "/node-missing",
                Some(&token),
                None,
            ))
            .await
            .expect("run delete missing agent node request");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = decode_json_body(response).await;
        assert_eq!(body["error"], json!("agent node not found"));
    }

    #[tokio::test]
    async fn delete_main_agent_node_returns_bad_request() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state);

        let response = app
            .oneshot(build_json_request(
                Method::DELETE,
                "/main",
                Some(&token),
                None,
            ))
            .await
            .expect("run delete main agent node request");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = decode_json_body(response).await;
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|value| value.contains("reserved")),
            "unexpected error body: {body}"
        );
    }
}
