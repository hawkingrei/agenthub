use std::sync::Arc;

use axum::Json;
use axum::body::{Body, to_bytes};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header};
use axum::response::IntoResponse;
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tower::util::ServiceExt;
use uuid::Uuid;

use crate::acp::AcpPermissionService;
use crate::agent::AgentManager;
use crate::auth::AuthService;
use crate::config::{AppConfig, PushConfig, WebConfig};
use crate::push::PushService;
use crate::state::AppState;
use crate::team::TeamManager;

use super::{
    AckTeamRunMessageRequest, CompleteTeamRunStepRequest, CreateTeamRequest, CreateTeamRunRequest,
    FailTeamRunStepRequest, ListTeamRunEventsQuery, ListTeamRunInboxQuery,
    ResumeTeamRunStepRequest, SendTeamRunMessageRequest, SetTeamRunStepInputRequiredRequest,
    StartTeamRunStepRequest, SubmitTeamRunStepRequest, ack_team_run_message, cancel_team_run,
    complete_team_run_step, create_team, create_team_run, fail_team_run_step, get_team,
    get_team_run, list_team_run_events, list_team_run_inbox, list_team_run_steps, list_teams,
    resume_team_run_step, send_team_run_message, set_team_run_step_input_required,
    start_team_run_step, submit_team_run_step,
};

async fn build_test_state() -> AppState {
    let db = create_test_db().await;
    init_test_schema(&db).await;
    let keys_dir = std::env::temp_dir().join(format!("agenthub-a2a-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&keys_dir).expect("create keys dir");
    let keys_path = keys_dir.join("vapid.json");
    let config = AppConfig {
        web: Some(WebConfig {
            rp_id: Some("localhost".to_string()),
            rp_origin: Some("http://localhost:8080".to_string()),
            rp_name: Some("AgentHub Test".to_string()),
        }),
        push: Some(PushConfig {
            subject: Some("mailto:test@example.com".to_string()),
            keys_path: Some(keys_path.to_string_lossy().to_string()),
        }),
        ..Default::default()
    };
    let push = Arc::new(PushService::new(db.clone(), &config).expect("create push service"));
    let _ = std::fs::remove_dir_all(&keys_dir);
    let auth = Arc::new(
        AuthService::new(db.clone(), &config)
            .await
            .expect("create auth"),
    );
    let permissions = Arc::new(AcpPermissionService::new(db.clone()));
    let agents = Arc::new(AgentManager::new(
        db.clone(),
        push.clone(),
        Vec::new(),
        "agenthub-codex-acp".to_string(),
        None,
        permissions.clone(),
        auth.clone(),
    ));
    let teams = Arc::new(TeamManager::new(db.clone()));
    AppState {
        db,
        agents,
        teams,
        push,
        auth,
        acp_permissions: permissions,
    }
}

async fn create_test_db() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect sqlite")
}

async fn init_test_schema(db: &SqlitePool) {
    sqlx::query(
        r#"
        CREATE TABLE users (
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL,
            role TEXT NOT NULL,
            password_hash TEXT,
            created_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create users");

    sqlx::query(
        r#"
        CREATE TABLE auth_sessions (
            token TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL,
            revoked_at INTEGER,
            FOREIGN KEY(user_id) REFERENCES users(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create auth_sessions");

    sqlx::query(
        r#"
        CREATE TABLE devices (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            name TEXT NOT NULL,
            user_agent TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            last_login_at INTEGER,
            FOREIGN KEY(user_id) REFERENCES users(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create devices");

    sqlx::query(
        r#"
        CREATE TABLE agents (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            workdir TEXT NOT NULL,
            command TEXT NOT NULL,
            args TEXT NOT NULL,
            worktree_mode TEXT NOT NULL,
            worktree_repo TEXT,
            worktree_ref TEXT,
            code_mode INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create agents");

    sqlx::query(
        r#"
        CREATE TABLE safe_paths (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            created_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create safe_paths");

    sqlx::query(
        r#"
        CREATE TABLE agent_sessions (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            status TEXT NOT NULL,
            started_at INTEGER NOT NULL,
            ended_at INTEGER,
            FOREIGN KEY(agent_id) REFERENCES agents(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create agent_sessions");

    sqlx::query(
        r#"
        CREATE TABLE agent_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            seq TEXT NOT NULL,
            ts INTEGER NOT NULL,
            stream TEXT NOT NULL,
            message TEXT NOT NULL,
            FOREIGN KEY(agent_id) REFERENCES agents(id),
            FOREIGN KEY(session_id) REFERENCES agent_sessions(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create agent_events");

    sqlx::query(
        r#"
        CREATE TABLE team_definitions (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            description TEXT,
            spec_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_definitions");

    sqlx::query(
        r#"
        CREATE TABLE team_runs (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            context_id TEXT NOT NULL,
            status TEXT NOT NULL,
            input_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            started_at INTEGER,
            ended_at INTEGER,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_runs");

    sqlx::query(
        r#"
        CREATE TABLE team_steps (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            step_key TEXT NOT NULL,
            member_id TEXT NOT NULL,
            remote_task_id TEXT,
            status TEXT NOT NULL,
            attempt INTEGER NOT NULL DEFAULT 0,
            depends_on_json TEXT NOT NULL DEFAULT '[]',
            input_json TEXT,
            output_json TEXT,
            error_text TEXT,
            started_at INTEGER,
            ended_at INTEGER,
            UNIQUE(run_id, step_key, attempt),
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_steps");

    sqlx::query(
        r#"
        CREATE TABLE team_run_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            step_id TEXT,
            event_type TEXT NOT NULL,
            ts INTEGER NOT NULL,
            payload_json TEXT NOT NULL,
            FOREIGN KEY(run_id) REFERENCES team_runs(id),
            FOREIGN KEY(step_id) REFERENCES team_steps(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_run_events");

    sqlx::query(
        r#"
        CREATE TABLE team_actor_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            from_actor_id TEXT NOT NULL,
            to_actor_id TEXT NOT NULL,
            channel TEXT NOT NULL,
            transport TEXT NOT NULL,
            route_json TEXT,
            payload_json TEXT NOT NULL,
            idempotency_key TEXT,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            delivered_at INTEGER,
            relay_attempt INTEGER NOT NULL DEFAULT 0,
            relay_next_retry_at INTEGER,
            relay_last_error TEXT,
            dead_letter_at INTEGER,
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_actor_messages");

    sqlx::query(
        r#"
        CREATE UNIQUE INDEX idx_team_actor_messages_idempotency
        ON team_actor_messages(run_id, from_actor_id, idempotency_key)
        WHERE idempotency_key IS NOT NULL
        "#,
    )
    .execute(db)
    .await
    .expect("create team_actor_messages idempotency index");
}

async fn auth_headers(state: &AppState) -> HeaderMap {
    let token = create_auth_token(state).await;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).expect("auth header"),
    );
    headers
}

async fn create_auth_token(state: &AppState) -> String {
    let user_id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    sqlx::query(
        r#"
        INSERT INTO users (id, username, display_name, role, password_hash, created_at)
        VALUES (?1, ?2, ?3, 'root', NULL, ?4)
        "#,
    )
    .bind(&user_id)
    .bind(format!("root-{}", Uuid::new_v4()))
    .bind("Root")
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert user");

    let token = state
        .auth
        .create_session(&user_id)
        .await
        .expect("create token");
    token
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
    serde_json::from_slice(&bytes).expect("decode json response")
}

include!("tests_core.rs");
include!("tests_router.rs");
