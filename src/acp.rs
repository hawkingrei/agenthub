use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use agent_client_protocol::{
    Agent, CancelNotification, Client, ClientCapabilities, ClientSideConnection, ContentBlock,
    Implementation, InitializeRequest, LoadSessionRequest, NewSessionRequest, PromptRequest,
    ProtocolVersion, RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    SelectedPermissionOutcome, SessionNotification, SessionUpdate, SetSessionConfigOptionRequest,
    SetSessionModeRequest, SetSessionModelRequest, ToolCall, ToolCallUpdate, ToolCallUpdateFields,
};
use chrono::Utc;
use serde_json::{Map, Value};
use sqlx::{Row, SqlitePool};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::agent::{AgentOutput, OutputStream};
use uuid::Uuid;

#[derive(Clone)]
pub struct AcpEventSink {
    db: SqlitePool,
    output_tx: broadcast::Sender<AgentOutput>,
    agent_id: String,
    session_id: String,
}

impl AcpEventSink {
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

    async fn emit_json(&self, value: Value) {
        let message = value.to_string();
        self.emit_raw(OutputStream::Acp, message).await;
    }

    async fn emit_system(&self, message: String) {
        self.emit_raw(OutputStream::System, message).await;
    }

    async fn emit_raw(&self, stream: OutputStream, message: String) {
        let seq = Uuid::now_v7().to_string();
        let output = AgentOutput {
            agent_id: self.agent_id.clone(),
            session_id: self.session_id.clone(),
            seq: seq.clone(),
            ts: Utc::now().timestamp(),
            stream: stream.clone(),
            message: message.clone(),
        };
        let _ = self.output_tx.send(output);

        let _ = sqlx::query(
            r#"
            INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&self.agent_id)
        .bind(&self.session_id)
        .bind(seq)
        .bind(Utc::now().timestamp())
        .bind(stream_to_str(&stream))
        .bind(message)
        .execute(&self.db)
        .await;
    }

    async fn emit_update(&self, update: SessionUpdate) {
        if let Some(value) = update_to_event(update) {
            self.emit_json(value).await;
        }
    }
}

#[derive(Clone)]
pub struct AcpClient {
    sink: AcpEventSink,
    permissions: Arc<AcpPermissionService>,
}

impl AcpClient {
    pub fn new(sink: AcpEventSink, permissions: Arc<AcpPermissionService>) -> Self {
        Self { sink, permissions }
    }
}

#[async_trait::async_trait(?Send)]
impl Client for AcpClient {
    async fn request_permission(
        &self,
        args: RequestPermissionRequest,
    ) -> Result<RequestPermissionResponse, agent_client_protocol::Error> {
        let (request_id, response_rx) = self
            .permissions
            .create_request(&self.sink.agent_id, &args)
            .await
            .map_err(|err| agent_client_protocol::Error::internal_error().data(err.to_string()))?;

        let outcome =
            match tokio::time::timeout(std::time::Duration::from_secs(300), response_rx).await {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(_)) => RequestPermissionOutcome::Cancelled,
                Err(_) => {
                    let fallback = pick_allow_option(&args);
                    let _ = self
                        .permissions
                        .mark_timeout(&request_id, Some(&fallback))
                        .await;
                    fallback
                }
            };

        Ok(RequestPermissionResponse::new(outcome))
    }

    async fn session_notification(
        &self,
        args: SessionNotification,
    ) -> Result<(), agent_client_protocol::Error> {
        self.sink.emit_update(args.update).await;
        Ok(())
    }
}

fn pick_allow_option(args: &RequestPermissionRequest) -> RequestPermissionOutcome {
    let option_id = args
        .options
        .iter()
        .find(|opt| {
            matches!(
                opt.kind,
                agent_client_protocol::PermissionOptionKind::AllowAlways
                    | agent_client_protocol::PermissionOptionKind::AllowOnce
            )
        })
        .or_else(|| args.options.first())
        .map(|opt| opt.option_id.clone());

    match option_id {
        Some(option_id) => {
            RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option_id))
        }
        None => RequestPermissionOutcome::Cancelled,
    }
}

#[derive(Debug)]
enum AcpCommand {
    Prompt(String),
    SetMode(String),
    SetModel(String),
    SetConfig { config_id: String, value: String },
    Cancel,
}

#[derive(Clone)]
pub struct AcpHandle {
    pub session_id: String,
    tx: mpsc::Sender<AcpCommand>,
}

impl AcpHandle {
    pub async fn prompt(&self, input: String) -> anyhow::Result<()> {
        self.send(AcpCommand::Prompt(input)).await
    }

    pub async fn set_mode(&self, mode_id: String) -> anyhow::Result<()> {
        self.send(AcpCommand::SetMode(mode_id)).await
    }

    pub async fn set_model(&self, model_id: String) -> anyhow::Result<()> {
        self.send(AcpCommand::SetModel(model_id)).await
    }

    pub async fn set_config(&self, config_id: String, value: String) -> anyhow::Result<()> {
        self.send(AcpCommand::SetConfig { config_id, value }).await
    }

    pub async fn cancel(&self) -> anyhow::Result<()> {
        self.send(AcpCommand::Cancel).await
    }

    async fn send(&self, cmd: AcpCommand) -> anyhow::Result<()> {
        self.tx
            .send(cmd)
            .await
            .map_err(|_| anyhow::anyhow!("acp command channel closed"))
    }
}

pub async fn spawn_acp_session(
    db: SqlitePool,
    output_tx: broadcast::Sender<AgentOutput>,
    permissions: Arc<AcpPermissionService>,
    agent_id: String,
    agent_session_id: String,
    resume_session_id: Option<String>,
    workdir: String,
    stdout: ChildStdout,
    stdin: ChildStdin,
) -> anyhow::Result<AcpHandle> {
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<AcpCommand>(64);
    let (ready_tx, ready_rx) = oneshot::channel::<Result<String, String>>();

    let sink = AcpEventSink::new(db, output_tx, agent_id, agent_session_id);

    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(err) => {
                let _ = ready_tx.send(Err(format!("acp runtime init failed: {err}")));
                return;
            }
        };

        let local = tokio::task::LocalSet::new();
        runtime.block_on(local.run_until(async move {
            let client = AcpClient::new(sink.clone(), permissions);
            let outgoing = stdin.compat_write();
            let incoming = stdout.compat();
            let (conn, io_task) = ClientSideConnection::new(client, outgoing, incoming, |fut| {
                tokio::task::spawn_local(fut);
            });

            let io_sink = sink.clone();
            tokio::task::spawn_local(async move {
                if let Err(err) = io_task.await {
                    io_sink.emit_system(format!("acp io error: {err}")).await;
                }
            });

            let init = InitializeRequest::new(ProtocolVersion::V1)
                .client_capabilities(ClientCapabilities::default())
                .client_info(Implementation::new("agenthub", env!("CARGO_PKG_VERSION")));

            if let Err(err) = conn.initialize(init).await {
                let _ = ready_tx.send(Err(format!("acp initialize failed: {err}")));
                return;
            }

            let cwd = PathBuf::from(&workdir);
            let mut session_id = None;

            if let Some(resume_id) = resume_session_id.clone() {
                let request = LoadSessionRequest::new(resume_id.clone(), cwd.clone());
                match conn.load_session(request).await {
                    Ok(_) => {
                        sink.emit_system(format!("acp session resumed: {resume_id}"))
                            .await;
                        session_id = Some(resume_id);
                    }
                    Err(err) => {
                        sink.emit_system(format!("acp load_session failed: {err}"))
                            .await;
                    }
                }
            }

            if session_id.is_none() {
                let session = match conn.new_session(NewSessionRequest::new(cwd)).await {
                    Ok(session) => session,
                    Err(err) => {
                        let _ = ready_tx.send(Err(format!("acp new_session failed: {err}")));
                        return;
                    }
                };
                session_id = Some(session.session_id.to_string());
            }

            let session_id = session_id.unwrap_or_else(|| "unknown".to_string());
            let _ = ready_tx.send(Ok(session_id.clone()));

            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    AcpCommand::Prompt(prompt) => {
                        let request = PromptRequest::new(
                            session_id.clone(),
                            vec![ContentBlock::Text(agent_client_protocol::TextContent::new(
                                prompt,
                            ))],
                        );
                        if let Err(err) = conn.prompt(request).await {
                            sink.emit_system(format!("acp prompt error: {err}")).await;
                        }
                    }
                    AcpCommand::SetMode(mode_id) => {
                        let request = SetSessionModeRequest::new(session_id.clone(), mode_id);
                        if let Err(err) = conn.set_session_mode(request).await {
                            sink.emit_system(format!("acp set_mode error: {err}")).await;
                        }
                    }
                    AcpCommand::SetModel(model_id) => {
                        let request = SetSessionModelRequest::new(session_id.clone(), model_id);
                        if let Err(err) = conn.set_session_model(request).await {
                            sink.emit_system(format!("acp set_model error: {err}"))
                                .await;
                        }
                    }
                    AcpCommand::SetConfig { config_id, value } => {
                        let request = SetSessionConfigOptionRequest::new(
                            session_id.clone(),
                            config_id,
                            value,
                        );
                        if let Err(err) = conn.set_session_config_option(request).await {
                            sink.emit_system(format!("acp set_config error: {err}"))
                                .await;
                        }
                    }
                    AcpCommand::Cancel => {
                        let request = CancelNotification::new(session_id.clone());
                        if let Err(err) = conn.cancel(request).await {
                            sink.emit_system(format!("acp cancel error: {err}")).await;
                        }
                    }
                }
            }
        }));
    });

    match ready_rx.await {
        Ok(Ok(session_id)) => Ok(AcpHandle {
            session_id,
            tx: cmd_tx,
        }),
        Ok(Err(err)) => Err(anyhow::anyhow!(err)),
        Err(_) => Err(anyhow::anyhow!("acp session init cancelled")),
    }
}

fn update_to_event(update: SessionUpdate) -> Option<Value> {
    match &update {
        SessionUpdate::UserMessageChunk(chunk) => {
            Some(json_message("user_message", &chunk.content))
        }
        SessionUpdate::AgentMessageChunk(chunk) => {
            Some(json_message("agent_message", &chunk.content))
        }
        SessionUpdate::AgentThoughtChunk(chunk) => {
            Some(json_message("agent_thought", &chunk.content))
        }
        SessionUpdate::ToolCall(tool_call) => Some(json_tool_call(tool_call)),
        SessionUpdate::ToolCallUpdate(update) => Some(json_tool_call_update(update)),
        SessionUpdate::Plan(plan) => Some(serde_json::json!({
            "type": "plan",
            "plan": plan,
        })),
        SessionUpdate::AvailableCommandsUpdate(update) => Some(serde_json::json!({
            "type": "available_commands",
            "commands": update.available_commands,
            "meta": update.meta,
        })),
        SessionUpdate::CurrentModeUpdate(update) => Some(serde_json::json!({
            "type": "current_mode",
            "current_mode_id": update.current_mode_id,
            "meta": update.meta,
        })),
        _ => serde_json::to_value(&update)
            .ok()
            .map(|payload| serde_json::json!({ "type": "session_update", "payload": payload })),
    }
}

fn json_message(kind: &str, content: &ContentBlock) -> Value {
    let text = content_to_text(content);
    serde_json::json!({
        "type": kind,
        "text": text,
        "chunk": true
    })
}

fn content_to_text(content: &ContentBlock) -> String {
    match content {
        ContentBlock::Text(text) => text.text.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn json_tool_call(tool_call: &ToolCall) -> Value {
    serde_json::json!({
        "type": "tool_call",
        "id": tool_call.tool_call_id.to_string(),
        "title": tool_call.title,
        "kind": serde_json::to_value(&tool_call.kind).unwrap_or(Value::Null),
        "status": serde_json::to_value(&tool_call.status).unwrap_or(Value::Null),
        "content": serde_json::to_value(&tool_call.content).unwrap_or(Value::Null),
        "raw_input": tool_call.raw_input,
        "raw_output": tool_call.raw_output,
        "meta": tool_call.meta
    })
}

fn json_tool_call_update(update: &ToolCallUpdate) -> Value {
    let mut obj = Map::new();
    obj.insert(
        "type".to_string(),
        Value::String("tool_call_update".to_string()),
    );
    obj.insert(
        "id".to_string(),
        Value::String(update.tool_call_id.to_string()),
    );
    apply_tool_call_update_fields(&mut obj, &update.fields);
    if let Some(meta) = &update.meta {
        obj.insert(
            "meta".to_string(),
            serde_json::to_value(meta).unwrap_or(Value::Null),
        );
    }
    Value::Object(obj)
}

fn apply_tool_call_update_fields(obj: &mut Map<String, Value>, fields: &ToolCallUpdateFields) {
    if let Some(kind) = &fields.kind {
        obj.insert(
            "kind".to_string(),
            serde_json::to_value(kind).unwrap_or(Value::Null),
        );
    }
    if let Some(status) = &fields.status {
        obj.insert(
            "status".to_string(),
            serde_json::to_value(status).unwrap_or(Value::Null),
        );
    }
    if let Some(title) = &fields.title {
        obj.insert("title".to_string(), Value::String(title.clone()));
    }
    if let Some(content) = &fields.content {
        obj.insert(
            "content".to_string(),
            serde_json::to_value(content).unwrap_or(Value::Null),
        );
    }
    if let Some(locations) = &fields.locations {
        obj.insert(
            "locations".to_string(),
            serde_json::to_value(locations).unwrap_or(Value::Null),
        );
    }
    if let Some(raw_input) = &fields.raw_input {
        obj.insert("raw_input".to_string(), raw_input.clone());
    }
    if let Some(raw_output) = &fields.raw_output {
        obj.insert("raw_output".to_string(), raw_output.clone());
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

#[derive(Clone)]
pub struct AcpPermissionService {
    db: SqlitePool,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<RequestPermissionOutcome>>>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AcpPermissionRecord {
    pub id: String,
    pub agent_id: String,
    pub session_id: String,
    pub tool_call_id: Option<String>,
    pub options: Vec<agent_client_protocol::PermissionOption>,
    pub tool_call: Option<Value>,
    pub status: String,
    pub selected_option_id: Option<String>,
    pub created_at: i64,
    pub responded_at: Option<i64>,
}

impl AcpPermissionService {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            db,
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn create_request(
        &self,
        agent_id: &str,
        args: &RequestPermissionRequest,
    ) -> anyhow::Result<(String, oneshot::Receiver<RequestPermissionOutcome>)> {
        let id = uuid::Uuid::new_v4().to_string();
        let options_json = serde_json::to_string(&args.options)?;
        let tool_call_json = serde_json::to_string(&args.tool_call)?;
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO acp_permission_requests (
                id, agent_id, session_id, tool_call_id, options_json, tool_call_json,
                status, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7)
            "#,
        )
        .bind(&id)
        .bind(agent_id)
        .bind(args.session_id.to_string())
        .bind(args.tool_call.tool_call_id.to_string())
        .bind(options_json)
        .bind(tool_call_json)
        .bind(now)
        .execute(&self.db)
        .await?;

        let (tx, rx) = oneshot::channel();
        let mut pending = self.pending.lock().await;
        pending.insert(id.clone(), tx);
        Ok((id, rx))
    }

    pub async fn respond(
        &self,
        request_id: &str,
        outcome: RequestPermissionOutcome,
        selected_option_id: Option<String>,
    ) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            UPDATE acp_permission_requests
            SET status = 'responded', selected_option_id = ?1, responded_at = ?2
            WHERE id = ?3
            "#,
        )
        .bind(selected_option_id)
        .bind(now)
        .bind(request_id)
        .execute(&self.db)
        .await?;

        let mut pending = self.pending.lock().await;
        if let Some(sender) = pending.remove(request_id) {
            let _ = sender.send(outcome);
        }
        Ok(())
    }

    pub async fn mark_timeout(
        &self,
        request_id: &str,
        selected: Option<&RequestPermissionOutcome>,
    ) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        let selected_option_id = match selected {
            Some(RequestPermissionOutcome::Selected(selected)) => {
                Some(selected.option_id.to_string())
            }
            _ => None,
        };
        sqlx::query(
            r#"
            UPDATE acp_permission_requests
            SET status = 'timeout', selected_option_id = ?1, responded_at = ?2
            WHERE id = ?3
            "#,
        )
        .bind(selected_option_id)
        .bind(now)
        .bind(request_id)
        .execute(&self.db)
        .await?;
        let mut pending = self.pending.lock().await;
        pending.remove(request_id);
        Ok(())
    }

    pub async fn list(
        &self,
        agent_id: &str,
        status: Option<&str>,
    ) -> anyhow::Result<Vec<AcpPermissionRecord>> {
        let rows = if let Some(status) = status {
            sqlx::query(
                r#"
                SELECT id, agent_id, session_id, tool_call_id, options_json, tool_call_json,
                       status, selected_option_id, created_at, responded_at
                FROM acp_permission_requests
                WHERE agent_id = ?1 AND status = ?2
                ORDER BY created_at DESC
                "#,
            )
            .bind(agent_id)
            .bind(status)
            .fetch_all(&self.db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, agent_id, session_id, tool_call_id, options_json, tool_call_json,
                       status, selected_option_id, created_at, responded_at
                FROM acp_permission_requests
                WHERE agent_id = ?1
                ORDER BY created_at DESC
                "#,
            )
            .bind(agent_id)
            .fetch_all(&self.db)
            .await?
        };

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let options_json: String = row.get("options_json");
            let tool_call_json: Option<String> = row.try_get("tool_call_json").ok();
            let options = serde_json::from_str(&options_json).unwrap_or_default();
            let tool_call = tool_call_json.and_then(|raw| serde_json::from_str(&raw).ok());
            out.push(AcpPermissionRecord {
                id: row.get("id"),
                agent_id: row.get("agent_id"),
                session_id: row.get("session_id"),
                tool_call_id: row.try_get("tool_call_id").ok(),
                options,
                tool_call,
                status: row.get("status"),
                selected_option_id: row.try_get("selected_option_id").ok(),
                created_at: row.get("created_at"),
                responded_at: row.try_get("responded_at").ok(),
            });
        }
        Ok(out)
    }
}
