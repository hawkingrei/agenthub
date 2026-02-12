use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_client_protocol::{
    Agent, CancelNotification, Client, ClientCapabilities, ClientSideConnection, ContentBlock,
    ContentChunk, EnvVariable, HttpHeader, Implementation, InitializeRequest, LoadSessionRequest,
    McpCapabilities, McpServer, McpServerHttp, McpServerStdio, NewSessionRequest,
    PermissionOption, PermissionOptionKind, PromptRequest, ProtocolVersion, TextContent,
    RequestPermissionOutcome, RequestPermissionRequest,
    RequestPermissionResponse, SelectedPermissionOutcome, SessionNotification, SessionUpdate,
    SetSessionConfigOptionRequest, SetSessionModeRequest, SetSessionModelRequest, ToolCall,
    ToolCallUpdate, ToolCallUpdateFields,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Map, Number, Value};
use sqlx::{Row, SqlitePool};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::agent::{AgentOutput, OutputStream};
use uuid::Uuid;

const MCP_CONFIG_FILE: &str = ".agenthub/mcp.json";
const SKILLS_CONFIG_FILE: &str = ".agenthub/skills.json";

#[derive(Debug, Clone)]
struct AcpSkill {
    name: String,
    path: String,
    instructions: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpConfigFile {
    #[serde(default, rename = "mcpServers")]
    mcp_servers: HashMap<String, McpServerConfigJson>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpServerConfigJson {
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    url: Option<String>,
    headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct SkillsConfigFile {
    #[serde(default)]
    skills: Vec<SkillEntryJson>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SkillEntryJson {
    Path(String),
    Detailed { path: String, name: Option<String> },
}

#[derive(Clone)]
pub struct AcpEventSink {
    db: SqlitePool,
    output_tx: broadcast::Sender<AgentOutput>,
    agent_id: String,
    session_id: String,
    chunk_state: Arc<Mutex<AcpChunkState>>,
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
            chunk_state: Arc::new(Mutex::new(AcpChunkState::default())),
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
        let ts = Utc::now().timestamp();
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
        .bind(stream_to_str(&stream))
        .bind(message.clone())
        .execute(&self.db)
        .await;
        let Ok(result) = result else {
            tracing::error!("acp emit_raw: failed to persist event");
            return;
        };
        let output = AgentOutput {
            event_id: result.last_insert_rowid(),
            agent_id: self.agent_id.clone(),
            session_id: self.session_id.clone(),
            seq: seq.clone(),
            ts,
            stream: stream.clone(),
            message: message.clone(),
        };
        let _ = self.output_tx.send(output);
    }

    async fn emit_update(&self, update: SessionUpdate) {
        let value = {
            let mut chunk_state = self.chunk_state.lock().await;
            update_to_event(update, &mut chunk_state)
        };
        if let Some(value) = value {
            self.emit_json(value).await;
        }
    }
}

// Tracks synthesized message_id/chunk_index values when ACP updates omit them.
#[derive(Default)]
struct AcpChunkState {
    current_kind: Option<String>,
    current_message_id: Option<String>,
    current_chunk_index: u64,
}

impl AcpChunkState {
    // Reset state when switching away from message chunks.
    fn reset(&mut self) {
        self.current_kind = None;
        self.current_message_id = None;
        self.current_chunk_index = 0;
    }

    // Allocate the next message_id/chunk_index for a message kind.
    fn next_for_kind(&mut self, kind: &str) -> (String, u64) {
        if self.current_kind.as_deref() != Some(kind) || self.current_message_id.is_none() {
            self.current_kind = Some(kind.to_string());
            self.current_message_id = Some(Uuid::new_v4().to_string());
            self.current_chunk_index = 0;
        } else {
            self.current_chunk_index = self.current_chunk_index.saturating_add(1);
        }
        (
            self.current_message_id.clone().unwrap_or_else(|| Uuid::new_v4().to_string()),
            self.current_chunk_index,
        )
    }

    // Record explicit metadata when the agent provides message_id or chunk_index.
    fn observe(&mut self, kind: &str, message_id: &str, chunk_index: Option<u64>) {
        self.current_kind = Some(kind.to_string());
        self.current_message_id = Some(message_id.to_string());
        if let Some(idx) = chunk_index {
            self.current_chunk_index = idx;
        }
    }

    // Increment chunk_index for a known message_id within the same kind.
    fn next_index_for_message(&mut self, kind: &str, message_id: &str) -> u64 {
        if self.current_kind.as_deref() != Some(kind)
            || self.current_message_id.as_deref() != Some(message_id)
        {
            self.current_kind = Some(kind.to_string());
            self.current_message_id = Some(message_id.to_string());
            self.current_chunk_index = 0;
            return 0;
        }
        self.current_chunk_index = self.current_chunk_index.saturating_add(1);
        self.current_chunk_index
    }
}

#[derive(Debug, Clone)]
struct SkillEntry {
    path: String,
    name: Option<String>,
}

fn mcp_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(MCP_CONFIG_FILE)
}

fn skills_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(SKILLS_CONFIG_FILE)
}

fn expand_tilde(path: &str) -> String {
    if path == "~" {
        std::env::var("HOME").unwrap_or_else(|_| path.to_string())
    } else if let Some(stripped) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            Path::new(&home)
                .join(stripped)
                .to_string_lossy()
                .to_string()
        } else {
            path.to_string()
        }
    } else {
        path.to_string()
    }
}

fn load_mcp_servers() -> Vec<McpServer> {
    let path = mcp_config_path();
    let Ok(contents) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    match parse_mcp_config(&contents) {
        Ok(servers) => servers,
        Err(err) => {
            tracing::warn!("mcp config parse failed: path={} error={}", path.display(), err);
            Vec::new()
        }
    }
}

fn parse_mcp_config(contents: &str) -> Result<Vec<McpServer>, serde_json::Error> {
    let config: McpConfigFile = serde_json::from_str(contents)?;
    let mut servers = Vec::new();
    for (name, entry) in config.mcp_servers {
        if let Some(server) = build_mcp_server(&name, &entry) {
            servers.push(server);
        }
    }
    Ok(servers)
}

fn build_mcp_server(name: &str, entry: &McpServerConfigJson) -> Option<McpServer> {
    if let Some(command) = entry.command.as_deref() {
        let command = expand_tilde(command);
        let args = entry.args.clone().unwrap_or_default();
        let env = entry
            .env
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| EnvVariable::new(key, value))
            .collect::<Vec<_>>();
        let server = McpServerStdio::new(name.to_string(), PathBuf::from(command))
            .args(args)
            .env(env);
        return Some(McpServer::Stdio(server));
    }

    if let Some(url) = entry.url.as_deref() {
        let headers = entry
            .headers
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| HttpHeader::new(key, value))
            .collect::<Vec<_>>();
        let server = McpServerHttp::new(name.to_string(), url.to_string()).headers(headers);
        return Some(McpServer::Http(server));
    }

    tracing::warn!("mcp server skipped: name={} reason=missing command/url", name);
    None
}

fn filter_mcp_servers(mcp_servers: Vec<McpServer>, caps: &McpCapabilities) -> Vec<McpServer> {
    mcp_servers
        .into_iter()
        .filter(|server| match server {
            McpServer::Http(_) => caps.http,
            McpServer::Sse(_) => caps.sse,
            McpServer::Stdio(_) => true,
            _ => false,
        })
        .collect()
}

fn load_skills() -> Vec<AcpSkill> {
    let path = skills_config_path();
    let Ok(contents) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let entries = match parse_skills_config(&contents) {
        Ok(entries) => entries,
        Err(err) => {
            tracing::warn!(
                "skills config parse failed: path={} error={}",
                path.display(),
                err
            );
            return Vec::new();
        }
    };

    let mut skills = Vec::new();
    for entry in entries {
        let raw_path = expand_tilde(&entry.path);
        let path_buf = PathBuf::from(&raw_path);
        let Ok(contents) = fs::read_to_string(&path_buf) else {
            tracing::warn!("skills config read failed: path={}", path_buf.display());
            continue;
        };
        let name = entry
            .name
            .or_else(|| extract_skill_name(&contents))
            .unwrap_or_else(|| {
                path_buf
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("skill")
                    .to_string()
            });
        let instructions = format!(
            "<skill>\n<name>{}</name>\n<path>{}</path>\n{}\n</skill>",
            name,
            path_buf.display(),
            contents
        );
        skills.push(AcpSkill {
            name,
            path: path_buf.to_string_lossy().to_string(),
            instructions,
        });
    }
    skills
}

fn parse_skills_config(contents: &str) -> Result<Vec<SkillEntry>, serde_json::Error> {
    let config: SkillsConfigFile = serde_json::from_str(contents)?;
    Ok(config
        .skills
        .into_iter()
        .map(|entry| match entry {
            SkillEntryJson::Path(path) => SkillEntry { path, name: None },
            SkillEntryJson::Detailed { path, name } => SkillEntry { path, name },
        })
        .collect())
}

fn extract_skill_name(contents: &str) -> Option<String> {
    let mut lines = contents.lines();
    let first = lines.next()?.trim();
    if first != "---" {
        return None;
    }
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" || trimmed == "..." {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("name:") {
            let mut value = rest.trim().to_string();
            if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                value = value[1..value.len() - 1].to_string();
            }
            if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
                value = value[1..value.len() - 1].to_string();
            }
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn build_skill_blocks(skills: &[AcpSkill]) -> Vec<ContentBlock> {
    skills
        .iter()
        .map(|skill| ContentBlock::Text(TextContent::new(skill.instructions.clone())))
        .collect()
}

fn build_skills_meta(skills: &[AcpSkill]) -> Option<Map<String, Value>> {
    if skills.is_empty() {
        return None;
    }
    let skill_items = skills
        .iter()
        .map(|skill| {
            serde_json::json!({
                "name": skill.name,
                "path": skill.path,
            })
        })
        .collect::<Vec<_>>();
    let mut agenthub = Map::new();
    agenthub.insert("skills".to_string(), Value::Array(skill_items));
    let mut meta = Map::new();
    meta.insert("agenthub".to_string(), Value::Object(agenthub));
    Some(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server_name(server: &McpServer) -> &str {
        match server {
            McpServer::Http(cfg) => cfg.name.as_str(),
            McpServer::Sse(cfg) => cfg.name.as_str(),
            McpServer::Stdio(cfg) => cfg.name.as_str(),
            _ => "unknown",
        }
    }

    #[test]
    fn parse_mcp_config_supports_stdio_and_http() {
        let json = r#"
        {
          "mcpServers": {
            "stdio": {
              "command": "node",
              "args": ["server.js"],
              "env": { "TOKEN": "abc" }
            },
            "http": {
              "url": "http://localhost:7777",
              "headers": { "Authorization": "Bearer xyz" }
            }
          }
        }
        "#;
        let servers = parse_mcp_config(json).expect("parse mcp config");
        assert_eq!(servers.len(), 2);

        let mut by_name = HashMap::new();
        for server in servers {
            by_name.insert(server_name(&server).to_string(), server);
        }

        match by_name.get("stdio") {
            Some(McpServer::Stdio(cfg)) => {
                assert_eq!(cfg.command, PathBuf::from("node"));
                assert_eq!(cfg.args, vec!["server.js".to_string()]);
                let env_map: HashMap<_, _> =
                    cfg.env.iter().map(|e| (e.name.clone(), e.value.clone())).collect();
                assert_eq!(env_map.get("TOKEN"), Some(&"abc".to_string()));
            }
            other => panic!("unexpected stdio config: {other:?}"),
        }

        match by_name.get("http") {
            Some(McpServer::Http(cfg)) => {
                assert_eq!(cfg.url, "http://localhost:7777");
                let header_map: HashMap<_, _> =
                    cfg.headers.iter().map(|h| (h.name.clone(), h.value.clone())).collect();
                assert_eq!(header_map.get("Authorization"), Some(&"Bearer xyz".to_string()));
            }
            other => panic!("unexpected http config: {other:?}"),
        }
    }

    #[test]
    fn parse_skills_config_accepts_strings_and_objects() {
        let json = r#"
        {
          "skills": [
            "/tmp/skills/demo/SKILL.md",
            { "path": "/opt/skills/alpha/SKILL.md", "name": "alpha-skill" }
          ]
        }
        "#;
        let entries = parse_skills_config(json).expect("parse skills config");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "/tmp/skills/demo/SKILL.md");
        assert!(entries[0].name.is_none());
        assert_eq!(entries[1].path, "/opt/skills/alpha/SKILL.md");
        assert_eq!(entries[1].name.as_deref(), Some("alpha-skill"));
    }

    #[test]
    fn extract_skill_name_reads_front_matter() {
        let contents = r#"---
name: demo-skill
description: sample
---
# Body
"#;
        assert_eq!(
            extract_skill_name(contents),
            Some("demo-skill".to_string())
        );
        assert_eq!(extract_skill_name("no front matter"), None);
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
        let options = args
            .options
            .iter()
            .map(AcpPermissionOption::from)
            .collect::<Vec<_>>();
        let (request_id, response_rx) = self
            .permissions
            .create_request(
                &self.sink.agent_id,
                &self.sink.session_id,
                &args,
            )
            .await
            .map_err(|err| agent_client_protocol::Error::internal_error().data(err.to_string()))?;
        self.sink
            .emit_json(serde_json::json!({
                "type": "permission_request",
                "permission_id": request_id,
                "session_id": args.session_id.to_string(),
                "tool_call_id": args.tool_call.tool_call_id.to_string(),
                "options": &options,
                "created_at": Utc::now().timestamp(),
            }))
            .await;

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
                    self.sink
                        .emit_json(serde_json::json!({
                            "type": "permission_timeout",
                            "permission_id": request_id,
                            "session_id": args.session_id.to_string(),
                            "tool_call_id": args.tool_call.tool_call_id.to_string(),
                            "outcome": &fallback,
                            "responded_at": Utc::now().timestamp(),
                        }))
                        .await;
                    fallback
                }
            };

        self.sink
            .emit_json(serde_json::json!({
                "type": "permission_response",
                "permission_id": request_id,
                "session_id": args.session_id.to_string(),
                "tool_call_id": args.tool_call.tool_call_id.to_string(),
                "outcome": &outcome,
                "responded_at": Utc::now().timestamp(),
            }))
            .await;

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
    let mcp_servers = load_mcp_servers();
    let skills = load_skills();
    let skill_blocks = build_skill_blocks(&skills);
    let skills_meta = build_skills_meta(&skills);
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

            let init_response = match conn.initialize(init).await {
                Ok(response) => response,
                Err(err) => {
                    let _ = ready_tx.send(Err(format!("acp initialize failed: {err}")));
                    return;
                }
            };

            let mcp_servers =
                filter_mcp_servers(mcp_servers, &init_response.agent_capabilities.mcp_capabilities);

            let cwd = PathBuf::from(&workdir);
            let mut session_id = None;

            if let Some(resume_id) = resume_session_id.clone() {
                let mut request =
                    LoadSessionRequest::new(resume_id.clone(), cwd.clone()).mcp_servers(
                        mcp_servers.clone(),
                    );
                if let Some(meta) = skills_meta.clone() {
                    request = request.meta(meta);
                }
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
                let mut request = NewSessionRequest::new(cwd).mcp_servers(mcp_servers.clone());
                if let Some(meta) = skills_meta.clone() {
                    request = request.meta(meta);
                }
                let session = match conn.new_session(request).await {
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
                        let mut blocks = skill_blocks.clone();
                        blocks.push(ContentBlock::Text(TextContent::new(prompt)));
                        let request = PromptRequest::new(session_id.clone(), blocks);
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

fn update_to_event(update: SessionUpdate, chunk_state: &mut AcpChunkState) -> Option<Value> {
    match &update {
        SessionUpdate::UserMessageChunk(chunk) => {
            Some(json_message("user_message", chunk, chunk_state))
        }
        SessionUpdate::AgentMessageChunk(chunk) => {
            Some(json_message("agent_message", chunk, chunk_state))
        }
        SessionUpdate::AgentThoughtChunk(chunk) => {
            Some(json_message("agent_thought", chunk, chunk_state))
        }
        SessionUpdate::ToolCall(tool_call) => {
            chunk_state.reset();
            Some(json_tool_call(tool_call))
        }
        SessionUpdate::ToolCallUpdate(update) => {
            chunk_state.reset();
            Some(json_tool_call_update(update))
        }
        SessionUpdate::Plan(plan) => {
            chunk_state.reset();
            Some(serde_json::json!({
                "type": "plan",
                "plan": plan,
            }))
        }
        SessionUpdate::AvailableCommandsUpdate(update) => {
            chunk_state.reset();
            Some(serde_json::json!({
                "type": "available_commands",
                "commands": update.available_commands,
                "meta": update.meta,
            }))
        }
        SessionUpdate::CurrentModeUpdate(update) => {
            chunk_state.reset();
            Some(serde_json::json!({
                "type": "current_mode",
                "current_mode_id": update.current_mode_id,
                "meta": update.meta,
            }))
        }
        _ => {
            chunk_state.reset();
            serde_json::to_value(&update)
                .ok()
                .map(|payload| serde_json::json!({ "type": "session_update", "payload": payload }))
        }
    }
}

fn json_message(kind: &str, chunk: &ContentChunk, chunk_state: &mut AcpChunkState) -> Value {
    let text = content_to_text(&chunk.content);
    let mut obj = Map::new();
    obj.insert("type".to_string(), Value::String(kind.to_string()));
    obj.insert("text".to_string(), Value::String(text));
    obj.insert("chunk".to_string(), Value::Bool(true));
    let mut message_id: Option<String> = None;
    let mut chunk_index: Option<u64> = None;
    if let Some(meta) = &chunk.meta {
        if let Some(Value::String(raw_message_id)) = meta.get("message_id") {
            message_id = Some(raw_message_id.clone());
        }
        if let Some(Value::Number(raw_chunk_index)) = meta.get("chunk_index") {
            chunk_index = raw_chunk_index.as_u64();
        } else if let Some(Value::String(raw_chunk_index)) = meta.get("chunk_index") {
            if let Ok(value) = raw_chunk_index.parse::<u64>() {
                chunk_index = Some(value);
            }
        }
    }
    if let Some(id) = &message_id {
        if let Some(idx) = chunk_index {
            chunk_state.observe(kind, id, Some(idx));
        } else {
            let idx = chunk_state.next_index_for_message(kind, id);
            chunk_index = Some(idx);
        }
    } else {
        let (id, idx) = chunk_state.next_for_kind(kind);
        message_id = Some(id);
        if chunk_index.is_none() {
            chunk_index = Some(idx);
        }
    }
    if let Some(message_id) = message_id {
        obj.insert("message_id".to_string(), Value::String(message_id));
    }
    if let Some(chunk_index) = chunk_index {
        obj.insert(
            "chunk_index".to_string(),
            Value::Number(Number::from(chunk_index)),
        );
    }
    Value::Object(obj)
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
    pub acp_session_id: Option<String>,
    pub tool_call_id: Option<String>,
    pub options: Vec<AcpPermissionOption>,
    pub tool_call: Option<Value>,
    pub status: String,
    pub selected_option_id: Option<String>,
    pub created_at: i64,
    pub responded_at: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AcpPermissionOption {
    #[serde(alias = "optionId")]
    pub option_id: String,
    pub name: String,
    pub kind: PermissionOptionKind,
}

impl From<&PermissionOption> for AcpPermissionOption {
    fn from(option: &PermissionOption) -> Self {
        Self {
            option_id: option.option_id.0.to_string(),
            name: option.name.clone(),
            kind: option.kind,
        }
    }
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
        agent_session_id: &str,
        args: &RequestPermissionRequest,
    ) -> anyhow::Result<(String, oneshot::Receiver<RequestPermissionOutcome>)> {
        let id = uuid::Uuid::new_v4().to_string();
        let options = args
            .options
            .iter()
            .map(AcpPermissionOption::from)
            .collect::<Vec<_>>();
        let options_json = serde_json::to_string(&options)?;
        let tool_call_json = serde_json::to_string(&args.tool_call)?;
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO acp_permission_requests (
                id, agent_id, session_id, acp_session_id, tool_call_id, options_json, tool_call_json,
                status, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8)
            "#,
        )
        .bind(&id)
        .bind(agent_id)
        .bind(agent_session_id)
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
                SELECT id, agent_id, session_id, acp_session_id, tool_call_id, options_json, tool_call_json,
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
                SELECT id, agent_id, session_id, acp_session_id, tool_call_id, options_json, tool_call_json,
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
            let options = serde_json::from_str::<Vec<AcpPermissionOption>>(&options_json)
                .unwrap_or_default();
            let tool_call = tool_call_json.and_then(|raw| serde_json::from_str(&raw).ok());
            out.push(AcpPermissionRecord {
                id: row.get("id"),
                agent_id: row.get("agent_id"),
                session_id: row.get("session_id"),
                acp_session_id: row.try_get("acp_session_id").ok(),
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
