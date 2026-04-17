use std::{collections::BTreeSet, convert::Infallible, time::Duration};

use axum::response::sse::Event;
use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderName, HeaderValue, StatusCode, header},
    response::{IntoResponse, Sse},
    routing::get,
};
use futures::stream::Stream;
use sqlx::Error as SqlxError;
use tokio::{
    sync::{broadcast, mpsc, watch},
    task::JoinHandle,
};

use crate::agent::AgentOutput;
use crate::api::{ApiError, load_team_for_user};
use crate::state::AppState;
use crate::team::TeamConversationStreamEvent;

#[derive(Debug, serde::Deserialize)]
struct SseTokenQuery {
    token: String,
}

#[derive(Debug, serde::Deserialize)]
struct SseAgentsQuery {
    token: String,
    ids: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/agents", get(sse_agents))
        .route("/agents/{id}", get(sse_agent))
        .route(
            "/teams/{team_id}/tasks/{task_id}/messages",
            get(sse_team_task_messages),
        )
        .with_state(state)
}

async fn sse_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Query(query): Query<SseTokenQuery>,
) -> impl IntoResponse {
    if state.auth.validate_session(&query.token).await.is_err() {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }

    let output_rx = match state.agents.subscribe_output(&agent_id).await {
        Ok(rx) => rx,
        Err(err) => {
            if let Err(reconcile_err) = state.agents.reconcile_runtime_absence(&agent_id).await {
                tracing::warn!(
                    agent_id = %agent_id,
                    error = %reconcile_err,
                    "sse agent route failed to reconcile stale running state"
                );
            }
            return (StatusCode::NOT_FOUND, err.to_string()).into_response();
        }
    };

    sse_response(vec![output_rx])
}

async fn sse_agents(
    State(state): State<AppState>,
    Query(query): Query<SseAgentsQuery>,
) -> impl IntoResponse {
    if state.auth.validate_session(&query.token).await.is_err() {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }

    let agent_ids = parse_agent_ids(&query.ids);
    if agent_ids.is_empty() {
        return (StatusCode::BAD_REQUEST, "at least one agent id is required").into_response();
    }

    let mut output_rxs = Vec::with_capacity(agent_ids.len());
    for agent_id in agent_ids {
        match state.agents.subscribe_output(&agent_id).await {
            Ok(rx) => output_rxs.push(rx),
            Err(_) => {
                if let Err(reconcile_err) = state.agents.reconcile_runtime_absence(&agent_id).await
                {
                    tracing::warn!(
                        agent_id = %agent_id,
                        error = %reconcile_err,
                        "sse agents route failed to reconcile stale running state"
                    );
                }
            }
        }
    }

    if output_rxs.is_empty() {
        return (StatusCode::NOT_FOUND, "agent not running").into_response();
    }

    sse_response(output_rxs)
}

async fn sse_team_task_messages(
    State(state): State<AppState>,
    Path((team_id, task_id)): Path<(String, String)>,
    Query(query): Query<SseTokenQuery>,
) -> impl IntoResponse {
    let user = match state.auth.validate_session(&query.token).await {
        Ok(user) => user,
        Err(_) => return (StatusCode::UNAUTHORIZED, "unauthorized").into_response(),
    };
    if let Err(error) = load_team_for_user(&state, &team_id, &user).await {
        return error.into_response();
    }
    let task = match state.teams.get_task(&task_id).await {
        Ok(task) if task.team_id == team_id => task,
        Ok(_) => return (StatusCode::NOT_FOUND, "task not found").into_response(),
        Err(error) => return map_sse_not_found_error(error, "task not found").into_response(),
    };
    let conversation = match state.teams.get_task_conversation(&task.id).await {
        Ok(conversation) => conversation,
        Err(error) => {
            return map_sse_not_found_error(error, "conversation not found").into_response();
        }
    };
    let event_rx = state.teams.subscribe_conversation_events();
    team_conversation_sse_response(event_rx, team_id, task_id, conversation.id)
}

fn map_sse_not_found_error(error: anyhow::Error, msg: &str) -> ApiError {
    if matches!(
        error.downcast_ref::<SqlxError>(),
        Some(SqlxError::RowNotFound)
    ) {
        return ApiError::not_found(msg);
    }
    tracing::error!("team task message sse internal error: {}", error);
    ApiError::from(anyhow::anyhow!("internal server error"))
}

fn sse_response(output_rxs: Vec<broadcast::Receiver<AgentOutput>>) -> axum::response::Response {
    let stream = output_stream(output_rxs);
    decorate_sse_response(Sse::new(stream).into_response())
}

fn team_conversation_sse_response(
    event_rx: broadcast::Receiver<TeamConversationStreamEvent>,
    team_id: String,
    task_id: String,
    conversation_id: String,
) -> axum::response::Response {
    let stream = team_conversation_stream(event_rx, team_id, task_id, conversation_id);
    decorate_sse_response(Sse::new(stream).into_response())
}

fn decorate_sse_response(mut response: axum::response::Response) -> axum::response::Response {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));
    headers.insert(
        HeaderName::from_static("x-accel-buffering"),
        HeaderValue::from_static("no"),
    );
    response
}

fn parse_agent_ids(raw_ids: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut ids = Vec::new();
    for raw in raw_ids.split(',') {
        let id = raw.trim();
        if id.is_empty() {
            continue;
        }
        if seen.insert(id.to_string()) {
            ids.push(id.to_string());
        }
    }
    ids
}

const OUTPUT_STREAM_BUFFER_CAPACITY: usize = 512;
const OUTPUT_STREAM_SEND_TIMEOUT: Duration = Duration::from_secs(2);
const OUTPUT_STREAM_BATCH_MAX_EVENTS: usize = 32;
const OUTPUT_STREAM_BATCH_MAX_BYTES: usize = 64 * 1024;
const OUTPUT_STREAM_BATCH_FLUSH_INTERVAL: Duration = Duration::from_millis(50);
const TEAM_CONVERSATION_STREAM_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

fn output_stream(
    output_rxs: Vec<broadcast::Receiver<AgentOutput>>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    output_stream_with_limits(
        output_rxs,
        OUTPUT_STREAM_BUFFER_CAPACITY,
        OUTPUT_STREAM_SEND_TIMEOUT,
    )
}

fn output_stream_with_limits(
    output_rxs: Vec<broadcast::Receiver<AgentOutput>>,
    buffer_capacity: usize,
    send_timeout: Duration,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let (output_tx, output_rx) = mpsc::channel::<AgentOutput>(buffer_capacity);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (disconnect_tx, disconnect_rx) = watch::channel(false);
    let forwarders = output_rxs
        .into_iter()
        .map(|rx| {
            spawn_output_forwarder(
                rx,
                output_tx.clone(),
                shutdown_rx.clone(),
                disconnect_tx.clone(),
                send_timeout,
                buffer_capacity,
            )
        })
        .collect::<Vec<_>>();
    drop(output_tx);
    drop(disconnect_tx);

    let heartbeat_interval = Duration::from_secs(15);
    futures::stream::unfold(
        OutputStreamState {
            output_rx,
            heartbeat: tokio::time::interval(heartbeat_interval),
            batch_flush: tokio::time::interval(OUTPUT_STREAM_BATCH_FLUSH_INTERVAL),
            batch: Vec::new(),
            batch_bytes: 0,
            shutdown_tx,
            disconnect_rx,
            disconnect_watch_active: true,
            output_rx_open: true,
            forwarders,
        },
        |mut state| async move {
            loop {
                if *state.disconnect_rx.borrow() {
                    if let Some(event) = take_disconnect_recovery_event(&mut state) {
                        return Some((Ok(event), state));
                    }
                    return None;
                }
                if !state.output_rx_open && state.batch.is_empty() {
                    return None;
                }
                tokio::select! {
                    _ = state.heartbeat.tick() => {
                        let event = Event::default().data("heartbeat");
                        return Some((Ok(event), state));
                    }
                    _ = state.batch_flush.tick() => {
                        if let Some(event) = take_batched_event(&mut state) {
                            return Some((Ok(event), state));
                        }
                    }
                    changed = state.disconnect_rx.changed(), if state.disconnect_watch_active => {
                        match changed {
                            Ok(()) => {
                                if *state.disconnect_rx.borrow() {
                                    if let Some(event) = take_disconnect_recovery_event(&mut state) {
                                        return Some((Ok(event), state));
                                    }
                                    return None;
                                }
                            }
                            Err(_) => {
                                // All disconnect senders are dropped. Keep draining output_rx
                                // until it is naturally exhausted.
                                state.disconnect_watch_active = false;
                            }
                        }
                    }
                    msg = state.output_rx.recv(), if state.output_rx_open => {
                        match msg {
                            Some(output) => {
                                push_batched_output(&mut state, output);
                                if should_flush_batch(&state)
                                    && let Some(event) = take_batched_event(&mut state)
                                {
                                    return Some((Ok(event), state));
                                }
                            }
                            None => {
                                state.output_rx_open = false;
                                if let Some(event) = take_batched_event(&mut state) {
                                    return Some((Ok(event), state));
                                }
                                return None;
                            }
                        }
                    }
                }
            }
        },
    )
}

struct OutputStreamState {
    output_rx: mpsc::Receiver<AgentOutput>,
    heartbeat: tokio::time::Interval,
    batch_flush: tokio::time::Interval,
    batch: Vec<AgentOutput>,
    batch_bytes: usize,
    shutdown_tx: watch::Sender<bool>,
    disconnect_rx: watch::Receiver<bool>,
    disconnect_watch_active: bool,
    output_rx_open: bool,
    forwarders: Vec<JoinHandle<()>>,
}

impl Drop for OutputStreamState {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(true);
        for task in &self.forwarders {
            task.abort();
        }
    }
}

fn spawn_output_forwarder(
    mut output_rx: broadcast::Receiver<AgentOutput>,
    output_tx: mpsc::Sender<AgentOutput>,
    mut shutdown_rx: watch::Receiver<bool>,
    disconnect_tx: watch::Sender<bool>,
    send_timeout: Duration,
    buffer_capacity: usize,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                shutdown = shutdown_rx.changed() => {
                    if shutdown.is_err() || *shutdown_rx.borrow() {
                        break;
                    }
                }
                msg = output_rx.recv() => {
                    match msg {
                        Ok(output) => {
                            // Bounded fan-in avoids unbounded memory growth for slow/disconnected SSE clients.
                            // On sustained backpressure we close this SSE stream and let reconnect + DB replay catch up.
                            match tokio::time::timeout(send_timeout, output_tx.send(output)).await {
                                Ok(Ok(())) => {}
                                Ok(Err(_)) => break,
                                Err(_) => {
                                    let _ = disconnect_tx.send(true);
                                    tracing::warn!(
                                        ?send_timeout,
                                        buffer_capacity,
                                        "sse output stream backpressure timeout; closing stream"
                                    );
                                    break;
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            let _ = disconnect_tx.send(true);
                            tracing::warn!(
                                skipped,
                                "sse output stream lagged; closing stream for replay recovery"
                            );
                            break;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    })
}

#[derive(Debug, serde::Serialize)]
struct SseServerMessage {
    r#type: String,
    payload: serde_json::Value,
}

#[derive(Debug, serde::Serialize)]
struct SseServerBatchMessage {
    r#type: String,
    payload: Vec<AgentOutput>,
}

#[derive(Debug, serde::Serialize)]
struct TeamConversationSseMessage {
    r#type: String,
    payload: TeamConversationStreamEvent,
}

fn output_to_message(output: &AgentOutput) -> SseServerMessage {
    let msg_type = if matches!(output.stream, crate::agent::OutputStream::Acp) {
        "acp"
    } else {
        "output"
    };
    SseServerMessage {
        r#type: msg_type.to_string(),
        payload: serde_json::json!(output),
    }
}

fn team_conversation_event_to_message(
    event: TeamConversationStreamEvent,
) -> TeamConversationSseMessage {
    TeamConversationSseMessage {
        r#type: "team_conversation".to_string(),
        payload: event,
    }
}

fn team_conversation_stream(
    event_rx: broadcast::Receiver<TeamConversationStreamEvent>,
    team_id: String,
    task_id: String,
    conversation_id: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    futures::stream::unfold(
        (
            tokio::time::interval(TEAM_CONVERSATION_STREAM_HEARTBEAT_INTERVAL),
            event_rx,
            team_id,
            task_id,
            conversation_id,
        ),
        |(mut heartbeat, mut event_rx, team_id, task_id, conversation_id)| async move {
            loop {
                tokio::select! {
                    _ = heartbeat.tick() => {
                        return Some((
                            Ok(Event::default().data("heartbeat")),
                            (heartbeat, event_rx, team_id, task_id, conversation_id),
                        ));
                    }
                    msg = event_rx.recv() => {
                        match msg {
                            Ok(event)
                                if event.team_id == team_id
                                    && event.task_id == task_id
                                    && event.conversation_id == conversation_id =>
                            {
                                let text = match serde_json::to_string(
                                    &team_conversation_event_to_message(event),
                                ) {
                                    Ok(text) => text,
                                    Err(_) => continue,
                                };
                                return Some((
                                    Ok(Event::default().data(text)),
                                    (heartbeat, event_rx, team_id, task_id, conversation_id),
                                ));
                            }
                            Ok(_) => {}
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                let event = TeamConversationStreamEvent {
                                    team_id: team_id.clone(),
                                    task_id: task_id.clone(),
                                    conversation_id: conversation_id.clone(),
                                    message_id: None,
                                    source: "stream_replay_required".to_string(),
                                };
                                let text = match serde_json::to_string(
                                    &team_conversation_event_to_message(event),
                                ) {
                                    Ok(text) => text,
                                    Err(_) => continue,
                                };
                                return Some((
                                    Ok(Event::default().data(text)),
                                    (heartbeat, event_rx, team_id, task_id, conversation_id),
                                ));
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                        }
                    }
                }
            }
        },
    )
}

fn push_batched_output(state: &mut OutputStreamState, output: AgentOutput) {
    state.batch_bytes = state
        .batch_bytes
        .saturating_add(estimate_output_size(&output));
    state.batch.push(output);
}

fn should_flush_batch(state: &OutputStreamState) -> bool {
    state.batch.len() >= OUTPUT_STREAM_BATCH_MAX_EVENTS
        || state.batch_bytes >= OUTPUT_STREAM_BATCH_MAX_BYTES
}

fn take_batched_event(state: &mut OutputStreamState) -> Option<Event> {
    if state.batch.is_empty() {
        state.batch_bytes = 0;
        return None;
    }

    let outputs = std::mem::take(&mut state.batch);
    state.batch_bytes = 0;
    let text = if outputs.len() == 1 {
        serde_json::to_string(&output_to_message(&outputs[0])).ok()?
    } else {
        serde_json::to_string(&SseServerBatchMessage {
            r#type: "batch".to_string(),
            payload: outputs,
        })
        .ok()?
    };
    Some(Event::default().data(text))
}

fn take_disconnect_recovery_event(state: &mut OutputStreamState) -> Option<Event> {
    loop {
        match state.output_rx.try_recv() {
            Ok(output) => push_batched_output(state, output),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                state.output_rx_open = false;
                break;
            }
        }
    }
    take_batched_event(state)
}

fn estimate_output_size(output: &AgentOutput) -> usize {
    output.agent_id.len() + output.session_id.len() + output.seq.len() + output.message.len() + 64
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
    };
    use chrono::Utc;
    use sqlx::Row;
    use sqlx::SqlitePool;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use tower::util::ServiceExt;
    use uuid::Uuid;

    use crate::{
        acp::AcpPermissionService,
        agent::{AgentOutput, OutputStream},
        api::team_tests::build_test_state as build_team_test_state,
        auth::AuthService,
        config::{AppConfig, PushConfig, WebConfig},
        push::PushService,
        state::AppState,
        team::{TeamConversationStreamEvent, TeamDefinitionConfig, TeamManager},
    };

    use super::parse_agent_ids;

    fn build_sse_request(path: &str) -> Request<Body> {
        Request::builder()
            .method(Method::GET)
            .uri(path)
            .body(Body::empty())
            .expect("build sse request")
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
            )
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
            )
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
            )
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
                agent_loop_enabled INTEGER NOT NULL DEFAULT 0,
                agent_loop_idle_seconds INTEGER,
                agent_loop_prompt TEXT,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
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
            )
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
            )
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
                message BLOB NOT NULL,
                FOREIGN KEY(agent_id) REFERENCES agents(id),
                FOREIGN KEY(session_id) REFERENCES agent_sessions(id)
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create agent_events");
    }

    async fn build_test_state() -> AppState {
        let db = create_test_db().await;
        init_test_schema(&db).await;
        let keys_dir = std::env::temp_dir().join(format!("agenthub-sse-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&keys_dir).expect("create keys dir");
        let keys_path = keys_dir.join("vapid.json");
        let config = AppConfig {
            web: Some(WebConfig {
                rp_id: Some("localhost".to_string()),
                rp_origin: Some("http://localhost:8080".to_string()),
                rp_name: Some("AgentHub Test".to_string()),
                passkey_enabled: None,
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
        let event_dbs = agenthub_db::AgentEventDbRouter::new(
            std::env::temp_dir().join(format!("agenthub-sse-eventdb-{}", Uuid::new_v4())),
        );
        let agents = Arc::new(crate::agent::AgentManager::new(
            db.clone(),
            event_dbs.clone(),
            None,
            push.clone(),
            Vec::new(),
            "agenthub-codex-acp".to_string(),
            None,
            true,
            permissions.clone(),
            auth.clone(),
        ));
        let teams = Arc::new(TeamManager::new_with_event_dbs(db.clone(), event_dbs));
        AppState {
            db,
            agents,
            teams,
            push,
            auth,
            acp_permissions: permissions,
            agent_node_join_bootstrap: crate::agent::AgentNodeJoinBootstrapInfo::disabled(),
            default_worktree_root: config.default_worktree_root(),
        }
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
        state
            .auth
            .create_session(&user_id)
            .await
            .expect("create token")
    }

    async fn insert_running_agent_without_handle(
        state: &AppState,
        suffix: &str,
    ) -> (String, String) {
        let now = Utc::now().timestamp();
        let agent_id = format!("stale-agent-{suffix}-{}", Uuid::new_v4());
        let session_id = format!("stale-session-{suffix}-{}", Uuid::new_v4());
        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, 'use_existing', NULL, NULL, 0, 'running', ?6, ?7)
            "#,
        )
        .bind(&agent_id)
        .bind(format!("stale-{suffix}"))
        .bind("/tmp")
        .bind("cat")
        .bind("[]")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert stale running agent");
        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind(&session_id)
        .bind(&agent_id)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert stale running session");
        (agent_id, session_id)
    }

    fn sample_output(stream: OutputStream) -> AgentOutput {
        sample_output_with(42, stream, "hello")
    }

    fn sample_output_with(event_id: i64, stream: OutputStream, message: &str) -> AgentOutput {
        AgentOutput {
            event_id,
            agent_id: "agent-x".to_string(),
            session_id: "session-x".to_string(),
            seq: format!("{event_id:04}"),
            ts: 123,
            stream,
            message: message.to_string(),
        }
    }

    #[test]
    fn parse_agent_ids_dedupes_and_trims() {
        let parsed = parse_agent_ids(" alpha, beta ,alpha,, gamma ");
        assert_eq!(parsed, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn parse_agent_ids_ignores_empty_values() {
        let parsed = parse_agent_ids(" , , ");
        assert!(parsed.is_empty());
    }

    #[tokio::test]
    async fn sse_agents_requires_valid_token() {
        let state = build_test_state().await;
        let app = super::router(state);
        let response = app
            .oneshot(build_sse_request("/agents?ids=a&token=bad-token"))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn sse_agents_rejects_empty_ids() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = super::router(state);
        let response = app
            .oneshot(build_sse_request(&format!("/agents?ids=,,,&token={token}")))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn sse_agents_returns_not_found_when_no_running_agents_match() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = super::router(state);
        let response = app
            .oneshot(build_sse_request(&format!(
                "/agents?ids=missing-agent&token={token}"
            )))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn legacy_sse_agent_route_returns_not_found_when_agent_is_not_running() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = super::router(state);
        let response = app
            .oneshot(build_sse_request(&format!(
                "/agents/missing-agent?token={token}"
            )))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn sse_agents_reconciles_stale_running_agent_before_returning_not_found() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let (agent_id, session_id) = insert_running_agent_without_handle(&state, "sse").await;
        let app = super::router(state.clone());
        let response = app
            .oneshot(build_sse_request(&format!(
                "/agents?ids={agent_id}&token={token}"
            )))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let agent_row = sqlx::query("SELECT status FROM agents WHERE id = ?1")
            .bind(&agent_id)
            .fetch_one(&state.db)
            .await
            .expect("fetch reconciled agent");
        assert_eq!(agent_row.get::<String, _>("status"), "exited");

        let session_row = sqlx::query("SELECT status, ended_at FROM agent_sessions WHERE id = ?1")
            .bind(&session_id)
            .fetch_one(&state.db)
            .await
            .expect("fetch reconciled session");
        assert_eq!(session_row.get::<String, _>("status"), "exited");
        assert!(session_row.get::<Option<i64>, _>("ended_at").is_some());
    }

    #[tokio::test]
    async fn team_task_messages_sse_requires_valid_token() {
        let state = build_team_test_state().await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: "sse-team-auth".to_string(),
                description: Some("team conversation sse auth".to_string()),
                spec: serde_json::json!({
                    "entrypoint":"leader_plan",
                    "members":[{"member_id":"leader"}]
                }),
            })
            .await
            .expect("create team");
        let (task, _) = state
            .teams
            .create_task(
                &team.id,
                "all",
                "user",
                serde_json::json!({"bootstrap_kind":"shared_thread"}),
                "group_chat",
                Some("all"),
            )
            .await
            .expect("create shared thread task");
        let app = super::router(state);
        let response = app
            .oneshot(build_sse_request(&format!(
                "/teams/{}/tasks/{}/messages?token=bad-token",
                team.id, task.id
            )))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn team_task_messages_sse_returns_ok_for_accessible_team_task() {
        let state = build_team_test_state().await;
        let token = create_auth_token(&state).await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: "sse-team-ok".to_string(),
                description: Some("team conversation sse ok".to_string()),
                spec: serde_json::json!({
                    "entrypoint":"leader_plan",
                    "members":[{"member_id":"leader"}]
                }),
            })
            .await
            .expect("create team");
        let (task, _) = state
            .teams
            .create_task(
                &team.id,
                "all",
                "user",
                serde_json::json!({"bootstrap_kind":"shared_thread"}),
                "group_chat",
                Some("all"),
            )
            .await
            .expect("create shared thread task");
        let app = super::router(state);
        let response = app
            .oneshot(build_sse_request(&format!(
                "/teams/{}/tasks/{}/messages?token={}",
                team.id, task.id, token
            )))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn team_task_messages_sse_preserves_internal_errors_as_500() {
        let state = build_team_test_state().await;
        let token = create_auth_token(&state).await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: "sse-team-internal".to_string(),
                description: Some("team conversation sse internal error".to_string()),
                spec: serde_json::json!({
                    "entrypoint":"leader_plan",
                    "members":[{"member_id":"leader"}]
                }),
            })
            .await
            .expect("create team");
        let (task, _) = state
            .teams
            .create_task(
                &team.id,
                "all",
                "user",
                serde_json::json!({"bootstrap_kind":"shared_thread"}),
                "group_chat",
                Some("all"),
            )
            .await
            .expect("create shared thread task");
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&state.db)
            .await
            .expect("disable foreign keys");
        sqlx::query("DROP TABLE team_definitions")
            .execute(&state.db)
            .await
            .expect("drop team_definitions");
        let app = super::router(state);
        let response = app
            .oneshot(build_sse_request(&format!(
                "/teams/{}/tasks/{}/messages?token={}",
                team.id, task.id, token
            )))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn output_to_message_uses_output_type_for_non_acp_streams() {
        let msg = super::output_to_message(&sample_output(OutputStream::Stdout));
        assert_eq!(msg.r#type, "output");
    }

    #[test]
    fn output_to_message_uses_acp_type_for_acp_streams() {
        let msg = super::output_to_message(&sample_output(OutputStream::Acp));
        assert_eq!(msg.r#type, "acp");
    }

    #[tokio::test]
    async fn output_stream_emits_events_from_forwarders() {
        use futures::StreamExt;

        let (tx, rx) = tokio::sync::broadcast::channel(8);
        let mut stream = std::pin::pin!(super::output_stream(vec![rx]));

        let first = tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
            .await
            .expect("receive first event")
            .expect("stream should not end")
            .expect("stream event should be ok");
        assert!(format!("{first:?}").contains("heartbeat"));

        tx.send(sample_output(OutputStream::Stdout))
            .expect("send broadcast output");
        let _second = tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
            .await
            .expect("receive second event")
            .expect("stream should not end")
            .expect("stream event should be ok");
    }

    #[tokio::test]
    async fn team_conversation_stream_emits_only_matching_events() {
        use futures::StreamExt;

        let (tx, rx) = tokio::sync::broadcast::channel(8);
        let mut stream = std::pin::pin!(super::team_conversation_stream(
            rx,
            "team-1".to_string(),
            "task-1".to_string(),
            "conversation-1".to_string(),
        ));

        let first = tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
            .await
            .expect("receive heartbeat event")
            .expect("stream should not end")
            .expect("heartbeat should be ok");
        assert!(format!("{first:?}").contains("heartbeat"));

        tx.send(TeamConversationStreamEvent {
            team_id: "team-2".to_string(),
            task_id: "task-2".to_string(),
            conversation_id: "conversation-2".to_string(),
            message_id: Some(10),
            source: "conversation_message".to_string(),
        })
        .expect("send unrelated event");
        tx.send(TeamConversationStreamEvent {
            team_id: "team-1".to_string(),
            task_id: "task-1".to_string(),
            conversation_id: "conversation-1".to_string(),
            message_id: Some(11),
            source: "conversation_message".to_string(),
        })
        .expect("send matching event");

        let second = tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
            .await
            .expect("receive matching event")
            .expect("stream should not end")
            .expect("matching event should be ok");
        let second_debug = format!("{second:?}");
        assert!(second_debug.contains("team_conversation"));
        assert!(second_debug.contains("task-1"));
        assert!(second_debug.contains("message_id"));
    }

    #[tokio::test]
    async fn output_stream_closes_after_backpressure_timeout() {
        use futures::StreamExt;

        let (tx, rx) = tokio::sync::broadcast::channel(8);
        let mut stream = std::pin::pin!(super::output_stream_with_limits(
            vec![rx],
            1,
            std::time::Duration::from_millis(30),
        ));

        let first = tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
            .await
            .expect("receive first event")
            .expect("stream should not end")
            .expect("stream event should be ok");
        assert!(format!("{first:?}").contains("heartbeat"));

        tx.send(sample_output_with(1, OutputStream::Stdout, "alpha"))
            .expect("send first output");
        tx.send(sample_output_with(2, OutputStream::Stdout, "beta"))
            .expect("send second output");

        tokio::time::sleep(std::time::Duration::from_millis(80)).await;

        let next = tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
            .await
            .expect("poll stream after backpressure")
            .expect("stream should flush buffered output before termination")
            .expect("stream event should be ok");
        let debug = format!("{next:?}");
        assert!(
            debug.contains("alpha"),
            "expected buffered tail output before termination: {debug}"
        );
        assert!(
            !debug.contains("beta"),
            "timed-out output should not appear in final live flush: {debug}"
        );

        let final_event =
            tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
                .await
                .expect("poll stream termination after backpressure");
        assert!(
            final_event.is_none(),
            "stream should close after sustained backpressure timeout"
        );
    }

    #[tokio::test]
    async fn output_stream_closes_after_broadcast_lagged() {
        use futures::StreamExt;

        let (tx, rx) = tokio::sync::broadcast::channel(1);
        tx.send(sample_output(OutputStream::Stdout))
            .expect("send first output");
        tx.send(sample_output(OutputStream::Stdout))
            .expect("send second output");

        let mut stream = std::pin::pin!(super::output_stream_with_limits(
            vec![rx],
            8,
            std::time::Duration::from_secs(1),
        ));

        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        let next = tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
            .await
            .expect("poll stream after lagged");
        assert!(
            next.is_none(),
            "stream should close when broadcast receiver reports lagged"
        );
    }

    #[tokio::test]
    async fn output_stream_drains_buffered_messages_after_forwarder_shutdown() {
        use futures::StreamExt;

        let (tx, rx) = tokio::sync::broadcast::channel(8);
        let mut stream = std::pin::pin!(super::output_stream_with_limits(
            vec![rx],
            8,
            std::time::Duration::from_secs(1),
        ));

        let first = tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
            .await
            .expect("receive first event")
            .expect("stream should not end")
            .expect("stream event should be ok");
        assert!(format!("{first:?}").contains("heartbeat"));

        tx.send(sample_output(OutputStream::Stdout))
            .expect("send output");
        drop(tx);

        let second = tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
            .await
            .expect("receive buffered output after forwarder shutdown");
        assert!(
            second.is_some(),
            "stream should emit buffered output before termination"
        );

        let third = tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
            .await
            .expect("poll stream after buffered output");
        assert!(
            third.is_none(),
            "stream should terminate after flushing the final buffered output"
        );
    }

    #[tokio::test]
    async fn output_stream_batches_multiple_messages_into_single_sse_event() {
        use futures::StreamExt;

        let (tx, rx) = tokio::sync::broadcast::channel(8);
        let mut stream = std::pin::pin!(super::output_stream(vec![rx]));

        let first = tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
            .await
            .expect("receive heartbeat")
            .expect("stream should not end")
            .expect("stream event should be ok");
        assert!(format!("{first:?}").contains("heartbeat"));

        tx.send(sample_output_with(1, OutputStream::Stdout, "alpha"))
            .expect("send first output");
        tx.send(sample_output_with(2, OutputStream::Acp, "beta"))
            .expect("send second output");

        let second = tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
            .await
            .expect("receive batched output")
            .expect("stream should not end")
            .expect("stream event should be ok");
        let debug = format!("{second:?}");
        assert!(
            debug.contains("\\\"type\\\":\\\"batch\\\""),
            "expected batched SSE payload: {debug}"
        );
        assert!(
            debug.contains("alpha"),
            "expected first payload in batch: {debug}"
        );
        assert!(
            debug.contains("beta"),
            "expected second payload in batch: {debug}"
        );
    }
}
