//! A bounded context-lens consumer of the shared MCP proxy, not another upstream transport.

use std::{collections::HashSet, sync::Arc, time::Duration};

use agenthub_agent_domain::loop_runtime::LoopReservation;
use agenthub_mcp::{bridge::McpProxySession, protocol::parse_message};
use serde_json::{Value, json};
use tokio::{sync::mpsc, time::Instant};

use super::McpProxyHub;

pub(crate) const CONTEXT_DEADLINE: Duration = Duration::from_secs(30);
const MAX_CONTEXT_BYTES: usize = 65_536;
const MAX_DISCOVERY_PAGES: usize = 32;

pub(crate) enum MemContext {
    Ready(String),
    Unavailable,
    MissingTool,
    InvalidResponse,
}

impl MemContext {
    pub(crate) fn event_kind(&self) -> agenthub_agent_domain::loop_runtime::LoopEventKind {
        use agenthub_agent_domain::loop_runtime::LoopEventKind;
        match self {
            Self::Ready(_) => LoopEventKind::MemContextReady,
            Self::Unavailable => LoopEventKind::MemContextUnavailable,
            Self::MissingTool => LoopEventKind::MemContextMissing,
            Self::InvalidResponse => LoopEventKind::MemContextInvalid,
        }
    }
}

impl McpProxyHub {
    /// The daemon owns this future and the live operation guard until every admitted exchange
    /// settles, even if the activation stops waiting for its context deadline.
    pub(crate) async fn read_mem_context(
        &self,
        executor: &LoopReservation,
        space: &str,
        deadline: Instant,
    ) -> MemContext {
        let Ok(id) = self.open(executor, "nowledge-mem").await else {
            return MemContext::Unavailable;
        };
        let result = match self.session(executor, &id).await {
            Ok(session) => self.context_lens(executor, &session, space, deadline).await,
            Err(_) => Err(MemContext::Unavailable),
        };
        // Retire only this bootstrap session; the provider's independently negotiated session
        // continues to expose all authorized dynamic tools and native response shapes.
        let _ = self.close(executor, &id).await;
        result.unwrap_or_else(|error| error)
    }

    async fn context_lens(
        &self,
        executor: &LoopReservation,
        session: &Arc<McpProxySession>,
        space: &str,
        deadline: Instant,
    ) -> Result<MemContext, MemContext> {
        self.context_exchange(
            executor,
            session,
            json!({"jsonrpc":"2.0","id":1,
            "method":"initialize","params":{"protocolVersion":"2025-06-18",
            "capabilities":{},"clientInfo":{"name":"agenthub-context","version":"1"}}}),
            deadline,
        )
        .await?;
        self.context_exchange(
            executor,
            session,
            json!({"jsonrpc":"2.0",
            "method":"notifications/initialized"}),
            deadline,
        )
        .await?;
        let mut cursor = None;
        let mut seen = HashSet::new();
        let mut found = false;
        for page in 0..MAX_DISCOVERY_PAGES {
            let mut request =
                json!({"jsonrpc":"2.0","id":page + 2,"method":"tools/list","params":{}});
            if let Some(cursor) = cursor.take() {
                request["params"]["cursor"] = cursor;
            }
            let result = self
                .context_exchange(executor, session, request, deadline)
                .await?;
            let tools = result["tools"]
                .as_array()
                .ok_or(MemContext::InvalidResponse)?;
            found |= tools
                .iter()
                .any(|tool| tool["name"] == "read_context_bundle");
            cursor = result
                .get("nextCursor")
                .filter(|cursor| !cursor.is_null())
                .cloned();
            if cursor.is_none() {
                if !found {
                    return Err(MemContext::MissingTool);
                }
                let result = self
                    .context_exchange(
                        executor,
                        session,
                        json!({"jsonrpc":"2.0",
                    "id":MAX_DISCOVERY_PAGES + 2,"method":"tools/call",
                    "params":{"name":"read_context_bundle","arguments":{}}}),
                        deadline,
                    )
                    .await?;
                return decode_context(&result, space).map(MemContext::Ready);
            }
            let next = cursor
                .as_ref()
                .and_then(Value::as_str)
                .ok_or(MemContext::InvalidResponse)?;
            if next.len() > 4096 || !seen.insert(next.to_owned()) {
                return Err(MemContext::InvalidResponse);
            }
        }
        Err(MemContext::InvalidResponse)
    }

    async fn context_exchange(
        &self,
        executor: &LoopReservation,
        session: &Arc<McpProxySession>,
        message: Value,
        deadline: Instant,
    ) -> Result<Value, MemContext> {
        if Instant::now() >= deadline {
            return Err(MemContext::Unavailable);
        }
        let id = message.get("id").cloned();
        let prepared = session
            .prepare(executor, message)
            .await
            .map_err(|_| MemContext::Unavailable)?;
        let (output, mut receiver) = mpsc::channel::<agenthub_mcp::bridge::McpProxyFrame>(8);
        let receive = async move {
            let mut result = None;
            while let Some(frame) = receiver.recv().await {
                if !frame.message_json.is_empty() {
                    let response = parse_message(frame.message_json.as_bytes())
                        .map_err(|_| MemContext::InvalidResponse)?;
                    if response.get("id") == id.as_ref() && id.is_some() {
                        if response.get("error").is_some() {
                            return Err(MemContext::Unavailable);
                        }
                        if result.is_some() {
                            return Err(MemContext::InvalidResponse);
                        }
                        result = Some(
                            response
                                .get("result")
                                .cloned()
                                .ok_or(MemContext::InvalidResponse)?,
                        );
                    }
                }
            }
            if id.is_none() {
                Ok(Value::Null)
            } else {
                result.ok_or(MemContext::Unavailable)
            }
        };
        // The consumer deadline drops only its receiver. The journaled send still drains to a
        // factual terminal state under the daemon's guard, bounded by the transport deadline.
        let (_, response) = tokio::join!(
            prepared.run(self.journal.clone(), output),
            tokio::time::timeout_at(deadline, receive)
        );
        response.map_err(|_| MemContext::Unavailable)?
    }
}

fn decode_context(result: &Value, space: &str) -> Result<String, MemContext> {
    if result.get("isError").is_some_and(|value| value != false) || result.get("error").is_some() {
        return Err(MemContext::Unavailable);
    }
    // Cloud returns JSON in a native MCP text block. Structured output is also accepted, but
    // conflicting copies are never silently selected. The markdown is returned byte-for-byte.
    let text = result
        .get("content")
        .and_then(Value::as_array)
        .filter(|blocks| blocks.len() == 1)
        .and_then(|blocks| (blocks[0]["type"] == "text").then_some(&blocks[0]))
        .and_then(|block| block["text"].as_str());
    let parsed = text
        .map(serde_json::from_str::<Value>)
        .transpose()
        .map_err(|_| MemContext::InvalidResponse)?;
    let structured = result.get("structuredContent");
    if let (Some(parsed), Some(structured)) = (&parsed, structured)
        && parsed != structured
    {
        return Err(MemContext::InvalidResponse);
    }
    let bundle = structured
        .or(parsed.as_ref())
        .ok_or(MemContext::InvalidResponse)?;
    if bundle["format"] != "markdown" || bundle["space_id"] != space {
        return Err(MemContext::InvalidResponse);
    }
    let content = bundle["content"]
        .as_str()
        .filter(|content| !content.is_empty() && content.len() <= MAX_CONTEXT_BYTES)
        .ok_or(MemContext::InvalidResponse)?;
    Ok(content.to_owned())
}

#[cfg(test)]
mod tests;
