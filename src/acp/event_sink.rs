use chrono::Utc;
use tokio::runtime::Handle;
use tokio::sync::broadcast;
use uuid::Uuid;

use agenthub_acp::{AcpEventSink, AcpStream};

use crate::agent::event_message_codec::persist_agent_event;
use crate::agent::{AgentOutput, OutputStream};
use crate::db::{AgentEventDbRouter, AgentEventIdleGc};

#[derive(Clone)]
pub struct AgenthubAcpEventSink {
    event_dbs: AgentEventDbRouter,
    idle_gc: Option<AgentEventIdleGc>,
    output_tx: broadcast::Sender<AgentOutput>,
    agent_id: String,
    session_id: String,
    runtime_handle: Handle,
}

impl AgenthubAcpEventSink {
    pub fn new(
        event_dbs: AgentEventDbRouter,
        idle_gc: Option<AgentEventIdleGc>,
        output_tx: broadcast::Sender<AgentOutput>,
        agent_id: String,
        session_id: String,
    ) -> Self {
        Self {
            event_dbs,
            idle_gc,
            output_tx,
            agent_id,
            session_id,
            runtime_handle: Handle::current(),
        }
    }

    fn map_stream(stream: AcpStream) -> OutputStream {
        match stream {
            AcpStream::Acp => OutputStream::Acp,
            AcpStream::System => OutputStream::System,
        }
    }
}

#[async_trait::async_trait]
impl AcpEventSink for AgenthubAcpEventSink {
    async fn emit_raw(&self, stream: AcpStream, message: String) {
        let seq = Uuid::now_v7().to_string();
        let ts = Utc::now().timestamp();
        let output_stream = Self::map_stream(stream);
        let event_dbs = self.event_dbs.clone();
        let idle_gc = self.idle_gc.clone();
        let agent_id = self.agent_id.clone();
        let session_id = self.session_id.clone();
        let stream_for_db = output_stream.clone();
        let message_for_db = message.clone();
        let seq_for_db = seq.clone();
        let result = self
            .runtime_handle
            .spawn(async move {
                persist_agent_event(
                    &event_dbs,
                    idle_gc.as_ref(),
                    &agent_id,
                    &session_id,
                    &seq_for_db,
                    ts,
                    &stream_for_db,
                    &message_for_db,
                )
                .await
            })
            .await;
        let event_id = match result {
            Ok(Ok(event_id)) => event_id,
            Ok(Err(err)) => {
                tracing::error!(
                    "acp emit_raw: failed to persist event: agent_id={} session_id={} error={}",
                    self.agent_id,
                    self.session_id,
                    err
                );
                return;
            }
            Err(err) => {
                tracing::error!(
                    "acp emit_raw: db task join failed: agent_id={} session_id={} error={}",
                    self.agent_id,
                    self.session_id,
                    err
                );
                return;
            }
        };
        let output = AgentOutput {
            event_id,
            agent_id: self.agent_id.clone(),
            session_id: self.session_id.clone(),
            seq,
            ts,
            stream: output_stream,
            message,
        };
        let _ = self.output_tx.send(output);
    }
}
