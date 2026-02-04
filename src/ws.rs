use axum::extract::ws::{Message, WebSocket};
use axum::{
    Router,
    extract::{Path, Query, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};
use futures::{SinkExt, StreamExt};

use crate::agent::AgentOutput;
use crate::state::AppState;

#[derive(Debug, serde::Deserialize)]
struct WsClientMessage {
    r#type: String,
    data: Option<String>,
}

#[derive(Debug, serde::Serialize)]
struct WsServerMessage {
    r#type: String,
    payload: serde_json::Value,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/agents/:id", get(ws_agent))
        .with_state(state)
}

#[derive(Debug, serde::Deserialize)]
struct WsQuery {
    token: String,
}

async fn ws_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(state, agent_id, query.token, socket))
}

async fn handle_socket(state: AppState, agent_id: String, token: String, socket: WebSocket) {
    if state.auth.validate_session(&token).await.is_err() {
        let mut socket = socket;
        let _ = socket.send(Message::Text("unauthorized".to_string())).await;
        return;
    }

    let mut output_rx = match state.agents.subscribe_output(&agent_id).await {
        Ok(rx) => rx,
        Err(err) => {
            let mut socket = socket;
            let _ = socket.send(Message::Text(err.to_string())).await;
            return;
        }
    };

    let (mut sender, mut receiver) = socket.split();

    let outbound = tokio::spawn(async move {
        while let Ok(output) = output_rx.recv().await {
            if let Ok(text) = serde_json::to_string(&output_to_message(&output)) {
                if sender.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
        }
    });

    let inbound_state = state.clone();
    let inbound_agent = agent_id.clone();
    let inbound = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(parsed) = serde_json::from_str::<WsClientMessage>(&text) {
                    if parsed.r#type == "input" {
                        if let Some(data) = parsed.data {
                            let _ = inbound_state.agents.send_input(&inbound_agent, &data).await;
                        }
                    }
                }
            }
        }
    });

    let _ = tokio::join!(outbound, inbound);
}

fn output_to_message(output: &AgentOutput) -> WsServerMessage {
    let msg_type = if matches!(output.stream, crate::agent::OutputStream::Acp) {
        "acp"
    } else {
        "output"
    };
    WsServerMessage {
        r#type: msg_type.to_string(),
        payload: serde_json::json!(output),
    }
}
