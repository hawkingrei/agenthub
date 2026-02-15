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

fn output_stream(
    output_rxs: Vec<broadcast::Receiver<AgentOutput>>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let (output_tx, output_rx) = mpsc::unbounded_channel::<AgentOutput>();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let forwarders = output_rxs
        .into_iter()
        .map(|rx| spawn_output_forwarder(rx, output_tx.clone(), shutdown_rx.clone()))
        .collect::<Vec<_>>();
    drop(output_tx);

    let heartbeat_interval = Duration::from_secs(15);
    futures::stream::unfold(
        OutputStreamState {
            output_rx,
            heartbeat: tokio::time::interval(heartbeat_interval),
            shutdown_tx,
            forwarders,
        },
        |mut state| async move {
            loop {
                tokio::select! {
                    _ = state.heartbeat.tick() => {
                        let event = Event::default().data("heartbeat");
                        return Some((Ok(event), state));
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
    output_rx: mpsc::UnboundedReceiver<AgentOutput>,
    heartbeat: tokio::time::Interval,
    shutdown_tx: watch::Sender<bool>,
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
    output_tx: mpsc::UnboundedSender<AgentOutput>,
    mut shutdown_rx: watch::Receiver<bool>,
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
                            if output_tx.send(output).is_err() {
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
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
    use super::parse_agent_ids;

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
}
