use chrono::Utc;
use sqlx::SqlitePool;
use tokio::runtime::Handle;
use tokio::sync::broadcast;
use uuid::Uuid;

use agenthub_acp::{AcpEventSink, AcpStream};

use crate::agent::{AgentOutput, OutputStream};

#[derive(Clone)]
pub struct AgenthubAcpEventSink {
    db: SqlitePool,
    output_tx: broadcast::Sender<AgentOutput>,
    agent_id: String,
    session_id: String,
    runtime_handle: Handle,
}

impl AgenthubAcpEventSink {
    pub fn new(
        db: SqlitePool,
        output_tx: broadcast::Sender<AgentOutput>,
        agent_id: String,
        session_id: String,
    ) -> Self {
        Self {
            db,
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
        let db = self.db.clone();
        let agent_id = self.agent_id.clone();
        let session_id = self.session_id.clone();
        let stream_name = stream_to_str(&output_stream).to_string();
        let message_for_db = message.clone();
        let seq_for_db = seq.clone();
        let result = self
            .runtime_handle
            .spawn(async move {
                sqlx::query(
                    r#"
                    INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    "#,
                )
                .bind(agent_id)
                .bind(session_id)
                .bind(seq_for_db)
                .bind(ts)
                .bind(stream_name)
                .bind(message_for_db)
                .execute(&db)
                .await
            })
            .await;
        let result = match result {
            Ok(Ok(result)) => result,
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
            event_id: result.last_insert_rowid(),
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

fn stream_to_str(stream: &OutputStream) -> &'static str {
    match stream {
        OutputStream::Stdout => "stdout",
        OutputStream::Stderr => "stderr",
        OutputStream::System => "system",
        OutputStream::Acp => "acp",
    }
}
