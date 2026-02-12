use chrono::Utc;
use sqlx::SqlitePool;
use tokio::sync::broadcast;
use uuid::Uuid;

use agenthub_acp::{AcpEventSink, AcpStream};

use crate::agent::{AgentOutput, OutputStream};

pub use agenthub_acp::*;

#[derive(Clone)]
pub struct AgenthubAcpEventSink {
    db: SqlitePool,
    output_tx: broadcast::Sender<AgentOutput>,
    agent_id: String,
    session_id: String,
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
        let result = sqlx::query(
            r#"
            INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&self.agent_id)
        .bind(&self.session_id)
        .bind(&seq)
        .bind(ts)
        .bind(stream_to_str(&output_stream))
        .bind(&message)
        .execute(&self.db)
        .await;
        let result = match result {
            Ok(result) => result,
            Err(err) => {
                tracing::error!(
                    "acp emit_raw: failed to persist event: agent_id={} session_id={} error={}",
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
