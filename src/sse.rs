use std::{convert::Infallible, time::Duration};

use axum::response::sse::Event;
use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderName, HeaderValue, StatusCode, header},
    response::{IntoResponse, Sse},
    routing::get,
};
use futures::stream::Stream;

use crate::agent::AgentOutput;
use crate::state::AppState;

#[derive(Debug, serde::Deserialize)]
struct SseQuery {
    token: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/agents/:id", get(sse_agent))
        .with_state(state)
}

async fn sse_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Query(query): Query<SseQuery>,
) -> impl IntoResponse {
    if state.auth.validate_session(&query.token).await.is_err() {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }

    let output_rx = match state.agents.subscribe_output(&agent_id).await {
        Ok(rx) => rx,
        Err(err) => return (StatusCode::NOT_FOUND, err.to_string()).into_response(),
    };

    let stream = output_stream(output_rx);
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

fn output_stream(
    output_rx: tokio::sync::broadcast::Receiver<AgentOutput>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let heartbeat_interval = Duration::from_secs(15);
    futures::stream::unfold(
        (output_rx, tokio::time::interval(heartbeat_interval)),
        |(mut rx, mut heartbeat)| async move {
            loop {
                tokio::select! {
                    _ = heartbeat.tick() => {
                        let event = Event::default().data("heartbeat");
                        return Some((Ok(event), (rx, heartbeat)));
                    }
                    msg = rx.recv() => {
                        match msg {
                            Ok(output) => {
                                if let Ok(text) = serde_json::to_string(&output_to_message(&output)) {
                                    let event = Event::default().data(text);
                                    return Some((Ok(event), (rx, heartbeat)));
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                continue;
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
