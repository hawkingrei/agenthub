use std::{collections::BTreeSet, convert::Infallible, time::Duration};
#[cfg(debug_assertions)]
use std::{
    collections::HashMap,
    sync::{Mutex as StdMutex, OnceLock},
};

use axum::response::sse::Event;
use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderName, HeaderValue, StatusCode, header},
    response::{IntoResponse, Sse},
    routing::get,
};
#[cfg(debug_assertions)]
use chrono::Utc;
use futures::stream::Stream;
use sqlx::Error as SqlxError;
use tokio::{
    sync::{broadcast, mpsc, watch},
    task::JoinHandle,
};

use crate::agent::AgentOutput;
use crate::api::{ApiError, load_team_for_user};
use crate::state::AppState;
use crate::team::{
    TeamConversationStreamEvent, TeamRunContextFingerprint, TeamRunContextStreamEvent,
    TeamRuntimeRecord, TeamRuntimeStatus,
};

const ACP_SSE_INLINE_MESSAGE_LIMIT: usize = 16 * 1024;
const ACP_DEFERRED_FIELD_NAMES: &[&str] = &["content", "raw_input", "raw_output"];

#[derive(Debug, serde::Deserialize)]
struct SseTokenQuery {
    token: String,
}

#[derive(Debug, serde::Deserialize)]
struct SseAgentsQuery {
    token: String,
    ids: String,
}

#[cfg(debug_assertions)]
#[derive(Debug, Clone, serde::Serialize, Default)]
pub(crate) struct AgentSseDiagnosticsSnapshot {
    pub active_stream_count: usize,
    pub active_forwarder_count: usize,
    pub last_forwarded_event_id: Option<i64>,
    pub last_forwarded_at: Option<i64>,
    pub last_emitted_event_id: Option<i64>,
    pub last_emitted_at: Option<i64>,
    pub last_error: Option<String>,
    pub last_error_at: Option<i64>,
}

#[cfg(debug_assertions)]
#[derive(Debug, Default)]
struct AgentSseDiagnosticsState {
    active_stream_count: usize,
    active_forwarder_count: usize,
    last_forwarded_event_id: Option<i64>,
    last_forwarded_at: Option<i64>,
    last_emitted_event_id: Option<i64>,
    last_emitted_at: Option<i64>,
    last_error: Option<String>,
    last_error_at: Option<i64>,
}

#[cfg(debug_assertions)]
static AGENT_SSE_DIAGNOSTICS: OnceLock<StdMutex<HashMap<String, AgentSseDiagnosticsState>>> =
    OnceLock::new();

#[cfg(debug_assertions)]
pub(crate) fn agent_sse_diagnostics(agent_id: &str) -> Option<AgentSseDiagnosticsSnapshot> {
    let registry = AGENT_SSE_DIAGNOSTICS.get_or_init(Default::default);
    let guard = registry.lock().expect("agent sse diagnostics poisoned");
    guard
        .get(agent_id)
        .map(|state| AgentSseDiagnosticsSnapshot {
            active_stream_count: state.active_stream_count,
            active_forwarder_count: state.active_forwarder_count,
            last_forwarded_event_id: state.last_forwarded_event_id,
            last_forwarded_at: state.last_forwarded_at,
            last_emitted_event_id: state.last_emitted_event_id,
            last_emitted_at: state.last_emitted_at,
            last_error: state.last_error.clone(),
            last_error_at: state.last_error_at,
        })
}

#[cfg(debug_assertions)]
fn update_agent_sse_diagnostics(
    agent_id: &str,
    update: impl FnOnce(&mut AgentSseDiagnosticsState),
) {
    let registry = AGENT_SSE_DIAGNOSTICS.get_or_init(Default::default);
    let mut guard = registry.lock().expect("agent sse diagnostics poisoned");
    update(guard.entry(agent_id.to_string()).or_default());
}

#[cfg(debug_assertions)]
fn record_sse_stream_open(agent_id: &str) {
    update_agent_sse_diagnostics(agent_id, |state| {
        state.active_stream_count = state.active_stream_count.saturating_add(1);
    });
}

#[cfg(debug_assertions)]
fn record_sse_stream_close(agent_id: &str) {
    update_agent_sse_diagnostics(agent_id, |state| {
        state.active_stream_count = state.active_stream_count.saturating_sub(1);
    });
}

#[cfg(debug_assertions)]
fn record_sse_forwarder_open(agent_id: &str) {
    update_agent_sse_diagnostics(agent_id, |state| {
        state.active_forwarder_count = state.active_forwarder_count.saturating_add(1);
    });
}

#[cfg(debug_assertions)]
fn record_sse_forwarder_close(agent_id: &str) {
    update_agent_sse_diagnostics(agent_id, |state| {
        state.active_forwarder_count = state.active_forwarder_count.saturating_sub(1);
    });
}

#[cfg(debug_assertions)]
fn record_sse_forwarded(output: &AgentOutput) {
    update_agent_sse_diagnostics(&output.agent_id, |state| {
        state.last_forwarded_event_id = Some(output.event_id);
        state.last_forwarded_at = Some(Utc::now().timestamp());
    });
}

#[cfg(debug_assertions)]
fn record_sse_emitted(output: &AgentOutput) {
    update_agent_sse_diagnostics(&output.agent_id, |state| {
        state.last_emitted_event_id = Some(output.event_id);
        state.last_emitted_at = Some(Utc::now().timestamp());
    });
}

#[cfg(debug_assertions)]
fn record_sse_error(agent_id: &str, message: String) {
    update_agent_sse_diagnostics(agent_id, |state| {
        state.last_error = Some(message);
        state.last_error_at = Some(Utc::now().timestamp());
    });
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/agents", get(sse_agents))
        .route("/agents/{id}", get(sse_agent))
        .route(
            "/teams/{team_id}/runs/{run_id}/context",
            get(sse_team_run_context),
        )
        .route("/teams/{team_id}/runtime", get(sse_team_runtime))
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

    sse_response(vec![(agent_id, output_rx)])
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
            Ok(rx) => output_rxs.push((agent_id, rx)),
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

async fn sse_team_run_context(
    State(state): State<AppState>,
    Path((team_id, run_id)): Path<(String, String)>,
    Query(query): Query<SseTokenQuery>,
) -> impl IntoResponse {
    let user = match state.auth.validate_session(&query.token).await {
        Ok(user) => user,
        Err(_) => return (StatusCode::UNAUTHORIZED, "unauthorized").into_response(),
    };
    if let Err(error) = load_team_for_user(&state, &team_id, &user).await {
        return error.into_response();
    }
    let run = match state.teams.get_run(&run_id).await {
        Ok(run) if run.team_id == team_id => run,
        Ok(_) => return (StatusCode::NOT_FOUND, "run not found").into_response(),
        Err(error) => return map_sse_not_found_error(error, "run not found").into_response(),
    };
    let stream = team_run_context_stream(state, run.team_id, run.id);
    decorate_sse_response(Sse::new(stream).into_response())
}

async fn sse_team_runtime(
    State(state): State<AppState>,
    Path(team_id): Path<String>,
    Query(query): Query<SseTokenQuery>,
) -> impl IntoResponse {
    let user = match state.auth.validate_session(&query.token).await {
        Ok(user) => user,
        Err(_) => return (StatusCode::UNAUTHORIZED, "unauthorized").into_response(),
    };
    let team = match load_team_for_user(&state, &team_id, &user).await {
        Ok(team) => team,
        Err(error) => return error.into_response(),
    };
    let stream = team_runtime_stream(state, team.id);
    decorate_sse_response(Sse::new(stream).into_response())
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

fn sse_response(
    output_rxs: Vec<(String, broadcast::Receiver<AgentOutput>)>,
) -> axum::response::Response {
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
const TEAM_RUN_CONTEXT_STREAM_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const TEAM_RUN_CONTEXT_STREAM_POLL_INTERVAL: Duration = Duration::from_secs(2);
const TEAM_RUNTIME_STREAM_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const TEAM_RUNTIME_STREAM_POLL_INTERVAL: Duration = Duration::from_secs(2);

fn output_stream(
    output_rxs: Vec<(String, broadcast::Receiver<AgentOutput>)>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    output_stream_with_limits(
        output_rxs,
        OUTPUT_STREAM_BUFFER_CAPACITY,
        OUTPUT_STREAM_SEND_TIMEOUT,
    )
}

fn output_stream_with_limits(
    output_rxs: Vec<(String, broadcast::Receiver<AgentOutput>)>,
    buffer_capacity: usize,
    send_timeout: Duration,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let agent_ids = output_rxs
        .iter()
        .map(|(agent_id, _)| agent_id.clone())
        .collect::<Vec<_>>();
    #[cfg(debug_assertions)]
    for agent_id in &agent_ids {
        record_sse_stream_open(agent_id);
    }
    let (output_tx, output_rx) = mpsc::channel::<AgentOutput>(buffer_capacity);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (disconnect_tx, disconnect_rx) = watch::channel(false);
    let forwarders = output_rxs
        .into_iter()
        .map(|(agent_id, rx)| {
            spawn_output_forwarder(
                agent_id,
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
            agent_ids,
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
    agent_ids: Vec<String>,
}

impl Drop for OutputStreamState {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(true);
        #[cfg(debug_assertions)]
        for agent_id in &self.agent_ids {
            record_sse_stream_close(agent_id);
            record_sse_forwarder_close(agent_id);
        }
        for task in &self.forwarders {
            task.abort();
        }
    }
}

fn spawn_output_forwarder(
    agent_id: String,
    mut output_rx: broadcast::Receiver<AgentOutput>,
    output_tx: mpsc::Sender<AgentOutput>,
    mut shutdown_rx: watch::Receiver<bool>,
    disconnect_tx: watch::Sender<bool>,
    send_timeout: Duration,
    buffer_capacity: usize,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        #[cfg(debug_assertions)]
        record_sse_forwarder_open(&agent_id);
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
                            let output_for_diagnostics = output.clone();
                            match tokio::time::timeout(send_timeout, output_tx.send(output)).await {
                                Ok(Ok(())) => {
                                    #[cfg(debug_assertions)]
                                    record_sse_forwarded(&output_for_diagnostics);
                                }
                                Ok(Err(_)) => break,
                                Err(_) => {
                                    let _ = disconnect_tx.send(true);
                                    #[cfg(debug_assertions)]
                                    record_sse_error(
                                        &agent_id,
                                        format!(
                                            "backpressure_timeout:{}ms",
                                            send_timeout.as_millis()
                                        ),
                                    );
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
                            #[cfg(debug_assertions)]
                            record_sse_error(&agent_id, format!("broadcast_lagged:{skipped}"));
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
        #[cfg(debug_assertions)]
        record_sse_forwarder_close(&agent_id);
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
        payload: serde_json::json!(compact_output_for_sse(output)),
    }
}

fn compact_output_for_sse(output: &AgentOutput) -> AgentOutput {
    if !matches!(output.stream, crate::agent::OutputStream::Acp)
        || output.message.len() <= ACP_SSE_INLINE_MESSAGE_LIMIT
    {
        return output.clone();
    }

    let Some(message) = compact_acp_message_for_sse(&output.message, output.event_id) else {
        return output.clone();
    };

    AgentOutput {
        message,
        ..output.clone()
    }
}

fn compact_acp_message_for_sse(message: &str, event_id: i64) -> Option<String> {
    let mut value = serde_json::from_str::<serde_json::Value>(message).ok()?;
    let obj = value.as_object_mut()?;
    let event_type = obj.get("type").and_then(serde_json::Value::as_str)?;
    if !matches!(event_type, "tool_call" | "tool_call_update") {
        return None;
    }

    let mut deferred_fields = Vec::new();
    for field in ACP_DEFERRED_FIELD_NAMES {
        if obj.remove(*field).is_some() {
            deferred_fields.push(serde_json::Value::String((*field).to_string()));
        }
    }
    if deferred_fields.is_empty() {
        return None;
    }

    obj.insert(
        "deferred_event_id".to_string(),
        serde_json::Value::Number(serde_json::Number::from(event_id)),
    );
    obj.insert(
        "deferred_fields".to_string(),
        serde_json::Value::Array(deferred_fields),
    );
    obj.insert(
        "deferred_reason".to_string(),
        serde_json::Value::String("large_acp_payload".to_string()),
    );
    obj.insert(
        "preview".to_string(),
        serde_json::Value::String(format!(
            "Large ACP payload deferred from SSE event {event_id}"
        )),
    );

    serde_json::to_string(&value).ok()
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

#[derive(Debug, serde::Serialize)]
struct TeamRunContextSseMessage {
    r#type: String,
    payload: TeamRunContextStreamEvent,
}

fn team_run_context_event_to_message(event: TeamRunContextStreamEvent) -> TeamRunContextSseMessage {
    TeamRunContextSseMessage {
        r#type: "team_run_context".to_string(),
        payload: event,
    }
}

fn optional_id(id: i64) -> Option<i64> {
    (id > 0).then_some(id)
}

fn build_team_run_context_delta(
    previous: &TeamRunContextFingerprint,
    next: &TeamRunContextFingerprint,
) -> Option<TeamRunContextStreamEvent> {
    let refresh_run = previous.run_status != next.run_status;
    let refresh_events = previous.latest_event_id != next.latest_event_id;
    let refresh_mailbox = previous.latest_mailbox_message_id != next.latest_mailbox_message_id
        || previous.mailbox_pending != next.mailbox_pending
        || previous.mailbox_delivered != next.mailbox_delivered
        || previous.mailbox_dead_letter != next.mailbox_dead_letter;
    let refresh_snapshot = refresh_run || refresh_events || refresh_mailbox;
    if !(refresh_run || refresh_events || refresh_mailbox) {
        return None;
    }
    Some(TeamRunContextStreamEvent {
        team_id: next.team_id.clone(),
        run_id: next.run_id.clone(),
        refresh_run,
        refresh_events,
        refresh_snapshot,
        refresh_mailbox,
        latest_event_id: optional_id(next.latest_event_id),
        latest_mailbox_message_id: optional_id(next.latest_mailbox_message_id),
        source: "poll_delta".to_string(),
    })
}

fn team_run_context_stream(
    state: AppState,
    team_id: String,
    run_id: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let mut heartbeat = tokio::time::interval(TEAM_RUN_CONTEXT_STREAM_HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut poll = tokio::time::interval(TEAM_RUN_CONTEXT_STREAM_POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    futures::stream::unfold(
        (
            heartbeat,
            poll,
            None::<TeamRunContextFingerprint>,
            state,
            team_id,
            run_id,
        ),
        |(mut heartbeat, mut poll, mut previous, state, team_id, run_id)| async move {
            loop {
                tokio::select! {
                    _ = heartbeat.tick() => {
                        return Some((
                            Ok(Event::default().data("heartbeat")),
                            (heartbeat, poll, previous, state, team_id, run_id),
                        ));
                    }
                    _ = poll.tick() => {
                        // The Team workbench still reads snapshots/events over HTTP. This stream
                        // only ships a compact invalidation fingerprint so one SSE connection can
                        // replace the old 4s triple-poll loop without introducing a second
                        // snapshot-format contract to keep in sync.
                        let next = match state.teams.read_run_context_fingerprint(&run_id).await {
                            Ok(next) => next,
                            Err(error) if matches!(error.downcast_ref::<SqlxError>(), Some(SqlxError::RowNotFound)) => {
                                return None;
                            }
                            Err(error) => {
                                tracing::warn!(
                                    team_id = %team_id,
                                    run_id = %run_id,
                                    error = %error,
                                    "team run context sse fingerprint refresh failed"
                                );
                                continue;
                            }
                        };
                        if next.team_id != team_id {
                            return None;
                        }
                        let event = previous
                            .as_ref()
                            .and_then(|prev| build_team_run_context_delta(prev, &next));
                        previous = Some(next);
                        if let Some(event) = event {
                            let text = match serde_json::to_string(
                                &team_run_context_event_to_message(event),
                            ) {
                                Ok(text) => text,
                                Err(_) => continue,
                            };
                            return Some((
                                Ok(Event::default().data(text)),
                                (heartbeat, poll, previous, state, team_id, run_id),
                            ));
                        }
                    }
                }
            }
        },
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TeamRuntimeFingerprint {
    team_id: String,
    status: TeamRuntimeStatus,
    members: Vec<TeamRuntimeMemberFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TeamRuntimeMemberFingerprint {
    member_id: String,
    agent_status: Option<String>,
    session_id: Option<String>,
    session_status: Option<String>,
    pending_inbox_count: i64,
}

#[derive(Debug, serde::Serialize)]
struct TeamRuntimeSseMessage {
    r#type: String,
    payload: TeamRuntimeStreamEvent,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TeamRuntimeStreamEvent {
    team_id: String,
    source: String,
}

fn team_runtime_fingerprint(runtime: TeamRuntimeRecord) -> TeamRuntimeFingerprint {
    let mut members: Vec<TeamRuntimeMemberFingerprint> = runtime
        .members
        .into_iter()
        .map(|member| TeamRuntimeMemberFingerprint {
            member_id: member.member_id,
            agent_status: member.agent_status,
            session_id: member.session_id,
            session_status: member.session_status,
            pending_inbox_count: member.pending_inbox_count,
        })
        .collect();
    members.sort_by(|left, right| left.member_id.cmp(&right.member_id));

    TeamRuntimeFingerprint {
        team_id: runtime.team_id,
        status: runtime.status,
        members,
    }
}

fn build_team_runtime_delta(
    previous: &TeamRuntimeFingerprint,
    next: &TeamRuntimeFingerprint,
) -> Option<TeamRuntimeStreamEvent> {
    if previous == next {
        return None;
    }
    Some(TeamRuntimeStreamEvent {
        team_id: next.team_id.clone(),
        source: "poll_delta".to_string(),
    })
}

fn team_runtime_event_to_message(event: TeamRuntimeStreamEvent) -> TeamRuntimeSseMessage {
    TeamRuntimeSseMessage {
        r#type: "team_runtime".to_string(),
        payload: event,
    }
}

fn team_runtime_stream(
    state: AppState,
    team_id: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let mut heartbeat = tokio::time::interval(TEAM_RUNTIME_STREAM_HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut poll = tokio::time::interval(TEAM_RUNTIME_STREAM_POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    futures::stream::unfold(
        (
            heartbeat,
            poll,
            None::<TeamRuntimeFingerprint>,
            state,
            team_id,
        ),
        |(mut heartbeat, mut poll, mut previous, state, team_id)| async move {
            loop {
                tokio::select! {
                    _ = heartbeat.tick() => {
                        return Some((
                            Ok(Event::default().data("heartbeat")),
                            (heartbeat, poll, previous, state, team_id),
                        ));
                    }
                    _ = poll.tick() => {
                        let next = match state.teams.describe_team_runtime(&team_id).await {
                            Ok(runtime) => team_runtime_fingerprint(runtime),
                            Err(error) if matches!(error.downcast_ref::<SqlxError>(), Some(SqlxError::RowNotFound)) => {
                                return None;
                            }
                            Err(error) => {
                                tracing::warn!(
                                    team_id = %team_id,
                                    error = %error,
                                    "team runtime sse fingerprint refresh failed"
                                );
                                continue;
                            }
                        };
                        let event = previous
                            .as_ref()
                            .and_then(|prev| build_team_runtime_delta(prev, &next));
                        previous = Some(next);
                        if let Some(event) = event {
                            let text = match serde_json::to_string(
                                &team_runtime_event_to_message(event),
                            ) {
                                Ok(text) => text,
                                Err(_) => continue,
                            };
                            return Some((
                                Ok(Event::default().data(text)),
                                (heartbeat, poll, previous, state, team_id),
                            ));
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
    #[cfg(debug_assertions)]
    for output in &outputs {
        record_sse_emitted(output);
    }
    let text = if outputs.len() == 1 {
        serde_json::to_string(&output_to_message(&outputs[0])).ok()?
    } else {
        let outputs = outputs
            .iter()
            .map(compact_output_for_sse)
            .collect::<Vec<_>>();
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

    use super::{ACP_SSE_INLINE_MESSAGE_LIMIT, compact_output_for_sse};

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
        object_upload::ObjectUploadService,
        push::PushService,
        state::AppState,
        team::{
            TeamConversationStreamEvent, TeamDefinitionConfig, TeamManager,
            TeamRunContextFingerprint,
        },
    };

    use super::{
        TeamRuntimeFingerprint, TeamRuntimeMemberFingerprint, TeamRuntimeStatus, parse_agent_ids,
    };

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
        let object_uploads = Arc::new(test_object_upload_service(db.clone()));
        AppState {
            db,
            linker_http: crate::linkers::AppLinkerService::default_http_client(),
            agents,
            teams,
            push,
            auth,
            acp_permissions: permissions,
            object_uploads,
            agent_node_join_bootstrap: crate::agent::AgentNodeJoinBootstrapInfo::disabled(),
            default_worktree_root: config.default_worktree_root(),
            body_store: None,
        }
    }

    fn test_object_upload_service(db: SqlitePool) -> ObjectUploadService {
        let root = std::env::temp_dir()
            .join(format!("agenthub-sse-objects-{}", Uuid::new_v4()))
            .to_string_lossy()
            .to_string();
        let config = AppConfig {
            object_store: Some(agenthub_config::ObjectStoreConfig {
                backend: Some("fs".to_string()),
                root: Some(root),
                public_base_url: None,
                prefix: None,
                bucket: None,
                endpoint: None,
                region: None,
                access_key_id_env: None,
                secret_access_key_env: None,
            }),
            ..Default::default()
        };
        ObjectUploadService::from_config(db, &config).expect("create object upload service")
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
    fn compact_output_for_sse_defers_large_acp_tool_payload() {
        let message = serde_json::json!({
            "type": "tool_call_update",
            "id": "call-1",
            "title": "Run command",
            "status": "completed",
            "content": "x".repeat(ACP_SSE_INLINE_MESSAGE_LIMIT),
            "raw_input": {"cmd": "cargo test"},
            "raw_output": {"stdout": "y".repeat(ACP_SSE_INLINE_MESSAGE_LIMIT)}
        })
        .to_string();
        let output = sample_output_with(77, OutputStream::Acp, &message);

        let compact = compact_output_for_sse(&output);
        let parsed: serde_json::Value =
            serde_json::from_str(&compact.message).expect("compact message JSON");

        assert_eq!(parsed["type"], "tool_call_update");
        assert_eq!(parsed["id"], "call-1");
        assert_eq!(parsed["status"], "completed");
        assert_eq!(parsed["deferred_event_id"], 77);
        assert_eq!(parsed["deferred_reason"], "large_acp_payload");
        assert!(parsed.get("content").is_none());
        assert!(parsed.get("raw_input").is_none());
        assert!(parsed.get("raw_output").is_none());
        assert_eq!(
            parsed["deferred_fields"],
            serde_json::json!(["content", "raw_input", "raw_output"])
        );
    }

    #[test]
    fn compact_output_for_sse_keeps_small_acp_payload_inline() {
        let message = serde_json::json!({
            "type": "tool_call_update",
            "id": "call-1",
            "status": "completed",
            "raw_output": {"stdout": "ok"}
        })
        .to_string();
        let output = sample_output_with(78, OutputStream::Acp, &message);

        let compact = compact_output_for_sse(&output);

        assert_eq!(compact.message, output.message);
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
                    "entrypoint":"coordinator_plan",
                    "members":[{"member_id":"coordinator"}]
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
    async fn team_run_context_sse_requires_valid_token() {
        let state = build_team_test_state().await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: "sse-team-run-auth".to_string(),
                description: Some("team run context sse auth".to_string()),
                spec: serde_json::json!({
                    "entrypoint":"coordinator_plan",
                    "members":[{"member_id":"coordinator"}]
                }),
            })
            .await
            .expect("create team");
        let run = state
            .teams
            .create_run(&team.id, Some("ctx-run-auth"), serde_json::json!({}))
            .await
            .expect("create run");
        let app = super::router(state);
        let response = app
            .oneshot(build_sse_request(&format!(
                "/teams/{}/runs/{}/context?token=bad-token",
                team.id, run.id
            )))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn team_runtime_sse_requires_valid_token() {
        let state = build_team_test_state().await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: "sse-team-runtime-auth".to_string(),
                description: Some("team runtime sse auth".to_string()),
                spec: serde_json::json!({
                    "entrypoint":"coordinator_plan",
                    "members":[{"member_id":"coordinator"}]
                }),
            })
            .await
            .expect("create team");
        let app = super::router(state);
        let response = app
            .oneshot(build_sse_request(&format!(
                "/teams/{}/runtime?token=bad-token",
                team.id
            )))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn team_runtime_sse_returns_ok_for_accessible_team() {
        let state = build_team_test_state().await;
        let token = create_auth_token(&state).await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: "sse-team-runtime-ok".to_string(),
                description: Some("team runtime sse ok".to_string()),
                spec: serde_json::json!({
                    "entrypoint":"coordinator_plan",
                    "members":[{"member_id":"coordinator"}]
                }),
            })
            .await
            .expect("create team");
        let app = super::router(state);
        let response = app
            .oneshot(build_sse_request(&format!(
                "/teams/{}/runtime?token={token}",
                team.id
            )))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("cache-control")
                .and_then(|value| value.to_str().ok()),
            Some("no-cache")
        );
    }

    #[test]
    fn build_team_run_context_delta_marks_expected_refresh_targets() {
        let previous = TeamRunContextFingerprint {
            team_id: "team-1".to_string(),
            run_id: "run-1".to_string(),
            run_status: "working".to_string(),
            latest_event_id: 10,
            latest_mailbox_message_id: 5,
            mailbox_pending: 1,
            mailbox_delivered: 2,
            mailbox_dead_letter: 0,
        };
        let next = TeamRunContextFingerprint {
            team_id: "team-1".to_string(),
            run_id: "run-1".to_string(),
            run_status: "completed".to_string(),
            latest_event_id: 11,
            latest_mailbox_message_id: 6,
            mailbox_pending: 0,
            mailbox_delivered: 3,
            mailbox_dead_letter: 0,
        };

        let delta =
            super::build_team_run_context_delta(&previous, &next).expect("delta should be emitted");
        assert!(delta.refresh_run);
        assert!(delta.refresh_events);
        assert!(delta.refresh_snapshot);
        assert!(delta.refresh_mailbox);
        assert_eq!(delta.latest_event_id, Some(11));
        assert_eq!(delta.latest_mailbox_message_id, Some(6));
    }

    #[test]
    fn build_team_runtime_delta_marks_visible_status_changes() {
        let previous = TeamRuntimeFingerprint {
            team_id: "team-1".to_string(),
            status: TeamRuntimeStatus::Running,
            members: vec![TeamRuntimeMemberFingerprint {
                member_id: "worker-1".to_string(),
                agent_status: Some("running".to_string()),
                session_id: Some("session-1".to_string()),
                session_status: Some("running".to_string()),
                pending_inbox_count: 0,
            }],
        };
        let next = TeamRuntimeFingerprint {
            status: TeamRuntimeStatus::Degraded,
            members: vec![TeamRuntimeMemberFingerprint {
                agent_status: Some("failed".to_string()),
                pending_inbox_count: 1,
                ..previous.members[0].clone()
            }],
            ..previous.clone()
        };

        let delta =
            super::build_team_runtime_delta(&previous, &next).expect("delta should be emitted");

        assert_eq!(delta.team_id, "team-1");
        assert_eq!(delta.source, "poll_delta");
        assert!(super::build_team_runtime_delta(&next, &next).is_none());
    }

    #[tokio::test]
    async fn team_runtime_fingerprint_tracks_runtime_snapshot_fields() {
        let state = build_team_test_state().await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: "fingerprint-runtime-team".to_string(),
                description: Some("team runtime fingerprint".to_string()),
                spec: serde_json::json!({
                    "entrypoint":"coordinator_plan",
                    "members":[{"member_id":"worker-b"},{"member_id":"coordinator-a"}]
                }),
            })
            .await
            .expect("create team");
        let runtime = state
            .teams
            .describe_team_runtime(&team.id)
            .await
            .expect("describe runtime");

        let fingerprint = super::team_runtime_fingerprint(runtime);

        assert_eq!(fingerprint.team_id, team.id);
        assert_eq!(fingerprint.status, TeamRuntimeStatus::Stopped);
        assert_eq!(
            fingerprint.members,
            vec![
                TeamRuntimeMemberFingerprint {
                    member_id: "coordinator-a".to_string(),
                    agent_status: None,
                    session_id: None,
                    session_status: None,
                    pending_inbox_count: 0,
                },
                TeamRuntimeMemberFingerprint {
                    member_id: "worker-b".to_string(),
                    agent_status: None,
                    session_id: None,
                    session_status: None,
                    pending_inbox_count: 0,
                },
            ]
        );
    }

    #[test]
    fn team_runtime_event_message_uses_runtime_event_type() {
        let message = super::team_runtime_event_to_message(super::TeamRuntimeStreamEvent {
            team_id: "team-1".to_string(),
            source: "poll_delta".to_string(),
        });

        assert_eq!(message.r#type, "team_runtime");
        assert_eq!(message.payload.team_id, "team-1");
        assert_eq!(message.payload.source, "poll_delta");
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
                    "entrypoint":"coordinator_plan",
                    "members":[{"member_id":"coordinator"}]
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
                    "entrypoint":"coordinator_plan",
                    "members":[{"member_id":"coordinator"}]
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
        let mut stream = std::pin::pin!(super::output_stream(vec![("agent-x".to_string(), rx)]));

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
        #[cfg(debug_assertions)]
        {
            let diagnostics = super::agent_sse_diagnostics("agent-x")
                .expect("agent sse diagnostics should exist");
            assert_eq!(diagnostics.last_forwarded_event_id, Some(42));
            assert_eq!(diagnostics.last_emitted_event_id, Some(42));
            assert!(diagnostics.last_forwarded_at.is_some());
            assert!(diagnostics.last_emitted_at.is_some());
        }
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
            vec![("agent-x".to_string(), rx)],
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
            vec![("agent-x".to_string(), rx)],
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
            vec![("agent-x".to_string(), rx)],
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
        let mut stream = std::pin::pin!(super::output_stream(vec![("agent-x".to_string(), rx)]));

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
