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
use tokio::{
    sync::{broadcast, mpsc, watch},
    task::JoinHandle,
};

use crate::agent::AgentOutput;
use crate::state::AppState;

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
        .route("/agents/:id", get(sse_agent))
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
        Err(err) => return (StatusCode::NOT_FOUND, err.to_string()).into_response(),
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
        if let Ok(rx) = state.agents.subscribe_output(&agent_id).await {
            output_rxs.push(rx);
        }
    }

    if output_rxs.is_empty() {
        return (StatusCode::NOT_FOUND, "agent not running").into_response();
    }

    sse_response(output_rxs)
}

fn sse_response(output_rxs: Vec<broadcast::Receiver<AgentOutput>>) -> axum::response::Response {
    let stream = output_stream(output_rxs);
    let mut response = Sse::new(stream).into_response();
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
            shutdown_tx,
            disconnect_rx,
            disconnect_watch_active: true,
            forwarders,
        },
        |mut state| async move {
            loop {
                if *state.disconnect_rx.borrow() {
                    return None;
                }
                tokio::select! {
                    _ = state.heartbeat.tick() => {
                        let event = Event::default().data("heartbeat");
                        return Some((Ok(event), state));
                    }
                    changed = state.disconnect_rx.changed(), if state.disconnect_watch_active => {
                        match changed {
                            Ok(()) => {
                                if *state.disconnect_rx.borrow() {
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
                    msg = state.output_rx.recv() => {
                        match msg {
                            Some(output) => {
                                if let Ok(text) = serde_json::to_string(&output_to_message(&output)) {
                                    let event = Event::default().data(text);
                                    return Some((Ok(event), state));
                                }
                            }
                            None => return None,
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
    shutdown_tx: watch::Sender<bool>,
    disconnect_rx: watch::Receiver<bool>,
    disconnect_watch_active: bool,
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
    };
    use chrono::Utc;
    use sqlx::SqlitePool;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use tower::util::ServiceExt;
    use uuid::Uuid;

    use crate::{
        acp::AcpPermissionService,
        agent::{AgentOutput, OutputStream},
        auth::AuthService,
        config::{AppConfig, PushConfig, WebConfig},
        push::PushService,
        state::AppState,
        team::TeamManager,
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
        let event_dbs = crate::db::AgentEventDbRouter::new(
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

    fn sample_output(stream: OutputStream) -> AgentOutput {
        AgentOutput {
            event_id: 42,
            agent_id: "agent-x".to_string(),
            session_id: "session-x".to_string(),
            seq: "0001".to_string(),
            ts: 123,
            stream,
            message: "hello".to_string(),
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

        tx.send(sample_output(OutputStream::Stdout))
            .expect("send first output");
        tx.send(sample_output(OutputStream::Stdout))
            .expect("send second output");

        tokio::time::sleep(std::time::Duration::from_millis(80)).await;

        let next = tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
            .await
            .expect("poll stream after backpressure");
        assert!(
            next.is_none(),
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
    }
}
