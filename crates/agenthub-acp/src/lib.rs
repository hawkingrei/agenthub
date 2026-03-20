mod actor_runtime_skill;
mod team_role_skills;

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use agent_client_protocol::{
    Agent, CancelNotification, Client, ClientCapabilities, ClientSideConnection, ContentBlock,
    ContentChunk, EnvVariable, Implementation, InitializeRequest, LoadSessionRequest, McpServer,
    McpServerStdio, NewSessionRequest, PermissionOption, PermissionOptionKind, PromptRequest,
    ProtocolVersion, RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    SelectedPermissionOutcome, SessionNotification, SessionUpdate, SetSessionConfigOptionRequest,
    SetSessionModeRequest, SetSessionModelRequest, TextContent, ToolCall, ToolCallUpdate,
    ToolCallUpdateFields,
};
use chrono::Utc;
use serde_json::{Map, Number, Value};
use sqlx::{Row, SqlitePool};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::runtime::Handle;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use uuid::Uuid;

use actor_runtime_skill::build_actor_runtime_skill;
use agenthub_acp_core::{
    AcpSkill, build_skill, build_skill_blocks, build_skills_meta, expand_tilde, extract_skill_name,
    filter_mcp_servers, parse_mcp_config, parse_skills_config,
};
use agenthub_config::path_utils::{is_path_allowed, normalize_path};
use team_role_skills::{
    build_team_role_skills, is_reserved_team_role_skill, should_attach_team_role_skills,
};

const MCP_CONFIG_FILE: &str = ".agenthub/mcp.json";
const SKILLS_CONFIG_FILE: &str = ".agenthub/skills.json";
const ACP_COMMAND_CHANNEL_CAPACITY: usize = 64;
const ACP_COMMAND_SEND_TIMEOUT: Duration = Duration::from_secs(5);
const ACP_SESSION_START_TIMEOUT: Duration = Duration::from_secs(30);
const ACTOR_MAILBOX_MCP_SERVER_NAME: &str = "agenthub-actor-mailbox";
const ACTOR_RUNTIME_TEAM_ID_ENV: &str = "AGENTHUB_ACTOR_TEAM_ID";
const ACTOR_RUNTIME_CURRENT_RUN_ID_ENV: &str = "AGENTHUB_ACTOR_CURRENT_RUN_ID";
const ACTOR_RUNTIME_ACTOR_ID_ENV: &str = "AGENTHUB_ACTOR_ID";
const ACTOR_RUNTIME_CHANNEL_ENV: &str = "AGENTHUB_ACTOR_CHANNEL";

#[derive(Debug, Clone)]
pub struct AcpActorContinuityEnvelope {
    pub mode: String,
    pub source_run_id: String,
    pub source_session_id: Option<String>,
    pub summary_text: String,
    pub history_window: Value,
}

#[derive(Debug, Clone)]
pub struct AcpActorSkillContext {
    pub team_id: Option<String>,
    pub current_run_id: Option<String>,
    pub actor_id: String,
    pub default_channel: String,
    pub actor_cli_path: String,
    pub member_role: Option<String>,
    pub member_skills: Vec<String>,
    pub contract_version: Option<String>,
    pub continuity: Option<AcpActorContinuityEnvelope>,
}

#[derive(Debug, Clone)]
pub struct AcpPermissionRoutingMetadata {
    pub team_id: Option<String>,
    pub requester_actor_id: Option<String>,
    pub requester_role: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AcpPermissionReviewRequest {
    pub request_id: String,
    pub agent_id: String,
    pub agent_session_id: String,
    pub acp_session_id: String,
    pub tool_call_id: Option<String>,
    pub options: Vec<AcpPermissionOption>,
    pub tool_call: Option<Value>,
    pub current_run_id: Option<String>,
    pub routing: AcpPermissionRoutingMetadata,
}

#[async_trait::async_trait]
pub trait AcpPermissionReviewDispatcher: Send + Sync {
    async fn dispatch_review(&self, request: AcpPermissionReviewRequest) -> anyhow::Result<()>;
}

pub struct SpawnAcpSessionRequest {
    pub event_sink: Arc<dyn AcpEventSink>,
    pub permissions: Arc<AcpPermissionService>,
    pub permission_review_dispatcher: Option<Arc<dyn AcpPermissionReviewDispatcher>>,
    pub agent_id: String,
    pub agent_session_id: String,
    pub resume_session_id: Option<String>,
    pub workdir: String,
    pub client_info: Implementation,
    pub stdout: ChildStdout,
    pub stdin: ChildStdin,
    pub safe_paths: Vec<String>,
    pub actor_context: Option<AcpActorSkillContext>,
    pub prompt_delivery_policy: AcpPromptDeliveryPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpStream {
    System,
    Acp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpPromptDeliveryPolicy {
    StrictFifo,
    AllowConcurrentPrompts,
}

#[async_trait::async_trait]
pub trait AcpEventSink: Send + Sync {
    async fn emit_raw(&self, stream: AcpStream, message: String);
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
            self.current_message_id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
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

fn mcp_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(MCP_CONFIG_FILE)
}

fn skills_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(SKILLS_CONFIG_FILE)
}

pub async fn load_safe_paths(db: &SqlitePool) -> anyhow::Result<Vec<String>> {
    let rows = sqlx::query("SELECT path FROM safe_paths ORDER BY id ASC")
        .fetch_all(db)
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| row.try_get::<String, _>("path").ok())
        .collect())
}

fn is_skill_path_allowed(path: &Path, safe_paths: &[String]) -> bool {
    if !path.is_absolute() {
        return false;
    }
    if safe_paths.is_empty() {
        return false;
    }
    let target = normalize_path(&path.to_string_lossy());
    for safe_path in safe_paths {
        let allowed = normalize_path(&expand_tilde(safe_path));
        if is_path_allowed(&target, &allowed) {
            return true;
        }
    }
    false
}

fn build_actor_mailbox_mcp_server(context: &AcpActorSkillContext) -> McpServer {
    let mut env = vec![
        EnvVariable::new(
            ACTOR_RUNTIME_ACTOR_ID_ENV.to_string(),
            context.actor_id.clone(),
        ),
        EnvVariable::new(
            ACTOR_RUNTIME_CHANNEL_ENV.to_string(),
            context.default_channel.clone(),
        ),
    ];
    if let Some(team_id) = context
        .team_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        env.push(EnvVariable::new(
            ACTOR_RUNTIME_TEAM_ID_ENV.to_string(),
            team_id.to_string(),
        ));
    }
    if let Some(run_id) = context
        .current_run_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        env.push(EnvVariable::new(
            ACTOR_RUNTIME_CURRENT_RUN_ID_ENV.to_string(),
            run_id.to_string(),
        ));
    }
    let mut args = vec![
        "actor-mcp".to_string(),
        "--actor-id".to_string(),
        context.actor_id.clone(),
        "--channel".to_string(),
        context.default_channel.clone(),
    ];
    if let Some(team_id) = context
        .team_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.push("--team-id".to_string());
        args.push(team_id.to_string());
    }
    if let Some(run_id) = context
        .current_run_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.push("--run-id".to_string());
        args.push(run_id.to_string());
    }
    let server = McpServerStdio::new(
        ACTOR_MAILBOX_MCP_SERVER_NAME.to_string(),
        PathBuf::from(context.actor_cli_path.clone()),
    )
    .args(args)
    .env(env);
    McpServer::Stdio(server)
}

fn load_mcp_servers_from_path(
    path: &Path,
    actor_context: Option<&AcpActorSkillContext>,
) -> Vec<McpServer> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            if let Some(context) = actor_context {
                return vec![build_actor_mailbox_mcp_server(context)];
            }
            return Vec::new();
        }
        Err(err) => {
            tracing::warn!(
                "mcp config read failed: path={} error={}",
                path.display(),
                err
            );
            if let Some(context) = actor_context {
                return vec![build_actor_mailbox_mcp_server(context)];
            }
            return Vec::new();
        }
    };
    match parse_mcp_config(&contents) {
        Ok(mut parsed_servers) => {
            if let Some(context) = actor_context {
                parsed_servers.push(build_actor_mailbox_mcp_server(context));
            }
            parsed_servers
        }
        Err(err) => {
            tracing::warn!(
                "mcp config parse failed: path={} error={}",
                path.display(),
                err
            );
            if let Some(context) = actor_context {
                vec![build_actor_mailbox_mcp_server(context)]
            } else {
                Vec::new()
            }
        }
    }
}

fn load_mcp_servers(actor_context: Option<&AcpActorSkillContext>) -> Vec<McpServer> {
    let path = mcp_config_path();
    load_mcp_servers_from_path(&path, actor_context)
}

fn mcp_server_name(server: &McpServer) -> &str {
    match server {
        McpServer::Http(cfg) => cfg.name.as_str(),
        McpServer::Sse(cfg) => cfg.name.as_str(),
        McpServer::Stdio(cfg) => cfg.name.as_str(),
        _ => "unknown",
    }
}

fn load_skills(safe_paths: &[String]) -> Vec<AcpSkill> {
    let path = skills_config_path();
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == ErrorKind::NotFound => return Vec::new(),
        Err(err) => {
            tracing::warn!(
                "skills config read failed: path={} error={}",
                path.display(),
                err
            );
            return Vec::new();
        }
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

    if safe_paths.is_empty() {
        tracing::warn!("skills config skipped: no safe paths configured");
        return Vec::new();
    }

    let mut skills = Vec::new();
    for entry in entries {
        let raw_path = expand_tilde(&entry.path);
        let path_buf = PathBuf::from(&raw_path);
        if !is_skill_path_allowed(&path_buf, safe_paths) {
            tracing::warn!(
                "skills config skipped: path={} reason=not allowed",
                path_buf.display()
            );
            continue;
        }
        let contents = match fs::read_to_string(&path_buf) {
            Ok(contents) => contents,
            Err(err) => {
                tracing::warn!(
                    "skills file read failed: path={} error={}",
                    path_buf.display(),
                    err
                );
                continue;
            }
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
        let path_display = path_buf.to_string_lossy().to_string();
        skills.push(build_skill(name, path_display, &contents));
    }
    skills
}

fn dedupe_skills(skills: Vec<AcpSkill>) -> Vec<AcpSkill> {
    let mut seen_name = std::collections::HashSet::new();
    let mut seen_path = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(skills.len());
    for skill in skills {
        let name_key = skill.name.to_ascii_lowercase();
        let path_key = skill.path.to_ascii_lowercase();
        if seen_name.contains(&name_key) || seen_path.contains(&path_key) {
            continue;
        }
        seen_name.insert(name_key);
        seen_path.insert(path_key);
        out.push(skill);
    }
    out
}

#[derive(Clone)]
pub struct AcpClient {
    sink: Arc<dyn AcpEventSink>,
    permissions: Arc<AcpPermissionService>,
    permission_review_dispatcher: Option<Arc<dyn AcpPermissionReviewDispatcher>>,
    agent_id: String,
    session_id: String,
    actor_context: Option<AcpActorSkillContext>,
    chunk_state: Arc<Mutex<AcpChunkState>>,
}

impl AcpClient {
    pub fn new(
        sink: Arc<dyn AcpEventSink>,
        permissions: Arc<AcpPermissionService>,
        permission_review_dispatcher: Option<Arc<dyn AcpPermissionReviewDispatcher>>,
        agent_id: String,
        session_id: String,
        actor_context: Option<AcpActorSkillContext>,
    ) -> Self {
        Self {
            sink,
            permissions,
            permission_review_dispatcher,
            agent_id,
            session_id,
            actor_context,
            chunk_state: Arc::new(Mutex::new(AcpChunkState::default())),
        }
    }

    async fn emit_json(&self, value: Value) {
        let message = value.to_string();
        self.sink.emit_raw(AcpStream::Acp, message).await;
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
                &self.agent_id,
                &self.session_id,
                &args,
                self.actor_context
                    .as_ref()
                    .map(|context| AcpPermissionRoutingMetadata {
                        team_id: context.team_id.clone(),
                        requester_actor_id: Some(context.actor_id.clone()),
                        requester_role: context.member_role.clone(),
                    }),
            )
            .await
            .map_err(|err| agent_client_protocol::Error::internal_error().data(err.to_string()))?;
        if let Some(dispatcher) = self.permission_review_dispatcher.as_ref() {
            let tool_call = serde_json::to_value(&args.tool_call).ok();
            let dispatch_request = AcpPermissionReviewRequest {
                request_id: request_id.clone(),
                agent_id: self.agent_id.clone(),
                agent_session_id: self.session_id.clone(),
                acp_session_id: args.session_id.to_string(),
                tool_call_id: Some(args.tool_call.tool_call_id.to_string()),
                options: options.clone(),
                tool_call,
                current_run_id: self
                    .actor_context
                    .as_ref()
                    .and_then(|context| context.current_run_id.clone()),
                routing: self
                    .actor_context
                    .as_ref()
                    .map(|context| AcpPermissionRoutingMetadata {
                        team_id: context.team_id.clone(),
                        requester_actor_id: Some(context.actor_id.clone()),
                        requester_role: context.member_role.clone(),
                    })
                    .unwrap_or(AcpPermissionRoutingMetadata {
                        team_id: None,
                        requester_actor_id: None,
                        requester_role: None,
                    }),
            };
            if let Err(err) = dispatcher.dispatch_review(dispatch_request).await {
                self.emit_json(serde_json::json!({
                    "type": "permission_review_dispatch_error",
                    "permission_id": request_id,
                    "error": err.to_string(),
                }))
                .await;
            }
        }
        self.emit_json(serde_json::json!({
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
                    self.emit_json(serde_json::json!({
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

        self.emit_json(serde_json::json!({
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
        self.emit_update(args.update).await;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcpSendError {
    ChannelClosed,
    Timeout(Duration),
}

impl std::fmt::Display for AcpSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcpSendError::ChannelClosed => write!(f, "acp command channel closed"),
            AcpSendError::Timeout(duration) => write!(
                f,
                "acp command queue is backpressured; timed out after {}ms",
                duration.as_millis()
            ),
        }
    }
}

impl std::error::Error for AcpSendError {}

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
        self.send_with_timeout(cmd, ACP_COMMAND_SEND_TIMEOUT).await
    }

    async fn send_with_timeout(&self, cmd: AcpCommand, timeout: Duration) -> anyhow::Result<()> {
        match tokio::time::timeout(timeout, self.tx.send(cmd)).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(anyhow::Error::new(AcpSendError::ChannelClosed)),
            Err(_) => {
                tracing::warn!(
                    session_id = %self.session_id,
                    timeout_ms = timeout.as_millis(),
                    "acp command send timed out due to backpressure"
                );
                Err(anyhow::Error::new(AcpSendError::Timeout(timeout)))
            }
        }
    }
}

fn is_session_mutation_command(cmd: &AcpCommand) -> bool {
    matches!(
        cmd,
        AcpCommand::SetMode(_) | AcpCommand::SetModel(_) | AcpCommand::SetConfig { .. }
    )
}

fn should_queue_while_prompts_active(
    active_prompt_count: usize,
    prompt_delivery_policy: AcpPromptDeliveryPolicy,
    has_pending_session_mutation: bool,
    cmd: &AcpCommand,
) -> bool {
    if active_prompt_count == 0 {
        return false;
    }

    match cmd {
        AcpCommand::Cancel => false,
        AcpCommand::Prompt(_) => {
            has_pending_session_mutation
                || !matches!(
                    prompt_delivery_policy,
                    AcpPromptDeliveryPolicy::AllowConcurrentPrompts
                )
        }
        AcpCommand::SetMode(_) | AcpCommand::SetModel(_) | AcpCommand::SetConfig { .. } => true,
    }
}

async fn dispatch_acp_command(
    cmd: AcpCommand,
    conn: Rc<ClientSideConnection>,
    event_sink: Arc<dyn AcpEventSink>,
    session_id: &str,
    skill_blocks: &[ContentBlock],
    prompt_done_tx: &mpsc::UnboundedSender<()>,
    active_prompt_count: &mut usize,
) {
    match cmd {
        AcpCommand::Prompt(prompt) => {
            *active_prompt_count = active_prompt_count.saturating_add(1);

            let conn = conn.clone();
            let event_sink = event_sink.clone();
            let session_id = session_id.to_string();
            let prompt_done_tx = prompt_done_tx.clone();
            let mut blocks = Vec::with_capacity(skill_blocks.len() + 1);
            blocks.extend(skill_blocks.iter().cloned());
            blocks.push(ContentBlock::Text(TextContent::new(prompt)));
            tokio::task::spawn_local(async move {
                let request = PromptRequest::new(session_id, blocks);
                if let Err(err) = conn.prompt(request).await {
                    event_sink
                        .emit_raw(AcpStream::System, format!("acp prompt error: {err}"))
                        .await;
                }
                let _ = prompt_done_tx.send(());
            });
        }
        AcpCommand::SetMode(mode_id) => {
            let request = SetSessionModeRequest::new(session_id.to_string(), mode_id);
            if let Err(err) = conn.set_session_mode(request).await {
                event_sink
                    .emit_raw(AcpStream::System, format!("acp set_mode error: {err}"))
                    .await;
            }
        }
        AcpCommand::SetModel(model_id) => {
            let request = SetSessionModelRequest::new(session_id.to_string(), model_id);
            if let Err(err) = conn.set_session_model(request).await {
                event_sink
                    .emit_raw(AcpStream::System, format!("acp set_model error: {err}"))
                    .await;
            }
        }
        AcpCommand::SetConfig { config_id, value } => {
            let request =
                SetSessionConfigOptionRequest::new(session_id.to_string(), config_id, value);
            if let Err(err) = conn.set_session_config_option(request).await {
                event_sink
                    .emit_raw(AcpStream::System, format!("acp set_config error: {err}"))
                    .await;
            }
        }
        AcpCommand::Cancel => {
            let request = CancelNotification::new(session_id.to_string());
            if let Err(err) = conn.cancel(request).await {
                event_sink
                    .emit_raw(AcpStream::System, format!("acp cancel error: {err}"))
                    .await;
            }
        }
    }
}

pub async fn spawn_acp_session(request: SpawnAcpSessionRequest) -> anyhow::Result<AcpHandle> {
    let SpawnAcpSessionRequest {
        event_sink,
        permissions,
        permission_review_dispatcher,
        agent_id,
        agent_session_id,
        resume_session_id,
        workdir,
        client_info,
        stdout,
        stdin,
        safe_paths,
        actor_context,
        prompt_delivery_policy,
    } = request;
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<AcpCommand>(ACP_COMMAND_CHANNEL_CAPACITY);
    let (ready_tx, ready_rx) = oneshot::channel::<Result<String, String>>();

    std::thread::spawn(move || {
        let mcp_servers = load_mcp_servers(actor_context.as_ref());
        let mut skills = load_skills(&safe_paths);
        skills.retain(|skill| !is_reserved_team_role_skill(skill.name.as_str()));
        let mut attached_team_role_skills = false;
        if let Some(ctx) = actor_context.as_ref() {
            if should_attach_team_role_skills(Some(ctx)) {
                skills.extend(build_team_role_skills(ctx));
                attached_team_role_skills = true;
            }
            skills.push(build_actor_runtime_skill(ctx));
        }
        let skills = dedupe_skills(skills);
        if let Some(ctx) = actor_context.as_ref() {
            let skill_names = skills
                .iter()
                .map(|skill| skill.name.as_str())
                .collect::<Vec<_>>();
            let mcp_server_names = mcp_servers.iter().map(mcp_server_name).collect::<Vec<_>>();
            let has_actor_mailbox_mcp = mcp_server_names.contains(&ACTOR_MAILBOX_MCP_SERVER_NAME);
            tracing::info!(
                team_id = %ctx.team_id.as_deref().unwrap_or("none"),
                current_run_id = %ctx.current_run_id.as_deref().unwrap_or("none"),
                actor_id = %ctx.actor_id,
                member_role = %ctx.member_role.as_deref().unwrap_or("none"),
                attached_team_role_skills,
                has_actor_mailbox_mcp,
                skill_names = ?skill_names,
                mcp_server_names = ?mcp_server_names,
                "acp actor session bootstrap prepared runtime capabilities"
            );
        }
        let skill_blocks = build_skill_blocks(&skills);
        let skills_meta = build_skills_meta(&skills);
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
            let client = AcpClient::new(
                event_sink.clone(),
                permissions,
                permission_review_dispatcher,
                agent_id.clone(),
                agent_session_id.clone(),
                actor_context.clone(),
            );
            let outgoing = stdin.compat_write();
            let incoming = stdout.compat();
            let (conn, io_task) = ClientSideConnection::new(client, outgoing, incoming, |fut| {
                tokio::task::spawn_local(fut);
            });

            let io_sink = event_sink.clone();
            tokio::task::spawn_local(async move {
                if let Err(err) = io_task.await {
                    io_sink
                        .emit_raw(AcpStream::System, format!("acp io error: {err}"))
                        .await;
                }
            });

            let init = InitializeRequest::new(ProtocolVersion::V1)
                .client_capabilities(ClientCapabilities::default())
                .client_info(client_info);

            let init_response = match conn.initialize(init).await {
                Ok(response) => response,
                Err(err) => {
                    let _ = ready_tx.send(Err(format!("acp initialize failed: {err}")));
                    return;
                }
            };

            let mcp_servers = filter_mcp_servers(
                mcp_servers,
                &init_response.agent_capabilities.mcp_capabilities,
            );

            let cwd = PathBuf::from(&workdir);
            let mut session_id = None;

            if let Some(resume_id) = resume_session_id.clone() {
                let mut request = LoadSessionRequest::new(resume_id.clone(), cwd.clone())
                    .mcp_servers(mcp_servers.clone());
                if let Some(meta) = skills_meta.clone() {
                    request = request.meta(meta);
                }
                match conn.load_session(request).await {
                    Ok(_) => {
                        event_sink
                            .emit_raw(
                                AcpStream::System,
                                format!("acp session resumed: {resume_id}"),
                            )
                            .await;
                        session_id = Some(resume_id);
                    }
                    Err(err) => {
                        event_sink
                            .emit_raw(AcpStream::System, format!("acp load_session failed: {err}"))
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

            let conn = Rc::new(conn);
            let (prompt_done_tx, mut prompt_done_rx) = mpsc::unbounded_channel::<()>();
            let mut active_prompt_count = 0usize;
            let mut cmd_rx_closed = false;
            let mut pending_commands = VecDeque::<AcpCommand>::new();

            while !cmd_rx_closed || active_prompt_count > 0 || !pending_commands.is_empty() {
                if active_prompt_count == 0
                    && let Some(cmd) = pending_commands.pop_front()
                {
                    dispatch_acp_command(
                        cmd,
                        conn.clone(),
                        event_sink.clone(),
                        &session_id,
                        &skill_blocks,
                        &prompt_done_tx,
                        &mut active_prompt_count,
                    )
                    .await;
                    continue;
                }

                tokio::select! {
                    maybe_done = prompt_done_rx.recv(), if active_prompt_count > 0 => {
                        if maybe_done.is_none() {
                            event_sink
                                .emit_raw(
                                    AcpStream::System,
                                    "acp prompt completion channel closed unexpectedly".to_string(),
                                )
                                .await;
                            break;
                        }
                        active_prompt_count = active_prompt_count.saturating_sub(1);
                    }
                    maybe_cmd = cmd_rx.recv(), if !cmd_rx_closed => {
                        match maybe_cmd {
                            Some(cmd) => {
                                let has_pending_session_mutation =
                                    pending_commands.iter().any(is_session_mutation_command);
                                if should_queue_while_prompts_active(
                                    active_prompt_count,
                                    prompt_delivery_policy,
                                    has_pending_session_mutation,
                                    &cmd,
                                ) {
                                    pending_commands.push_back(cmd);
                                    continue;
                                }
                                dispatch_acp_command(
                                    cmd,
                                    conn.clone(),
                                    event_sink.clone(),
                                    &session_id,
                                    &skill_blocks,
                                    &prompt_done_tx,
                                    &mut active_prompt_count,
                                )
                                .await;
                            }
                            None => {
                                cmd_rx_closed = true;
                            }
                        }
                    }
                }
            }
        }));
    });

    match tokio::time::timeout(ACP_SESSION_START_TIMEOUT, ready_rx).await {
        Ok(Ok(Ok(session_id))) => Ok(AcpHandle {
            session_id,
            tx: cmd_tx,
        }),
        Ok(Ok(Err(err))) => Err(anyhow::anyhow!(err)),
        Ok(Err(_)) => Err(anyhow::anyhow!("acp session init cancelled")),
        Err(_) => Err(anyhow::anyhow!(
            "acp session init timed out after {}s",
            ACP_SESSION_START_TIMEOUT.as_secs()
        )),
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
        } else if let Some(Value::String(raw_chunk_index)) = meta.get("chunk_index")
            && let Ok(value) = raw_chunk_index.parse::<u64>()
        {
            chunk_index = Some(value);
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
        "kind": serde_json::to_value(tool_call.kind).unwrap_or(Value::Null),
        "status": serde_json::to_value(tool_call.status).unwrap_or(Value::Null),
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

#[derive(Clone)]
pub struct AcpPermissionService {
    db: SqlitePool,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<RequestPermissionOutcome>>>>,
    runtime_handle: Handle,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AcpPermissionRecord {
    pub id: String,
    pub agent_id: String,
    pub session_id: String,
    pub acp_session_id: Option<String>,
    pub team_id: Option<String>,
    pub requester_actor_id: Option<String>,
    pub requester_role: Option<String>,
    pub review_target_actor_id: Option<String>,
    pub review_dispatch_status: Option<String>,
    pub review_delivery_run_id: Option<String>,
    pub review_message_id: Option<i64>,
    pub reviewed_by_actor_id: Option<String>,
    pub human_review_notified_at: Option<i64>,
    pub tool_call_id: Option<String>,
    pub options: Vec<AcpPermissionOption>,
    pub tool_call: Option<Value>,
    pub status: String,
    pub selected_option_id: Option<String>,
    pub created_at: i64,
    pub responded_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpPermissionRespondResult {
    Applied,
    AlreadyResolved,
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
            runtime_handle: Handle::current(),
        }
    }

    pub async fn create_request(
        &self,
        agent_id: &str,
        agent_session_id: &str,
        args: &RequestPermissionRequest,
        routing: Option<AcpPermissionRoutingMetadata>,
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
        let db = self.db.clone();
        let id_for_db = id.clone();
        let agent_id = agent_id.to_string();
        let agent_session_id = agent_session_id.to_string();
        let acp_session_id = args.session_id.to_string();
        let tool_call_id = args.tool_call.tool_call_id.to_string();
        let routing_team_id = routing.as_ref().and_then(|value| value.team_id.clone());
        let routing_requester_actor_id = routing
            .as_ref()
            .and_then(|value| value.requester_actor_id.clone());
        let routing_requester_role = routing
            .as_ref()
            .and_then(|value| value.requester_role.clone());
        self.runtime_handle
            .spawn(async move {
                sqlx::query(
                    r#"
                    INSERT INTO acp_permission_requests (
                        id, agent_id, session_id, acp_session_id, team_id, requester_actor_id, requester_role,
                        tool_call_id, options_json, tool_call_json, status, created_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11)
                    "#,
                )
                .bind(id_for_db)
                .bind(agent_id)
                .bind(agent_session_id)
                .bind(acp_session_id)
                .bind(routing_team_id)
                .bind(routing_requester_actor_id)
                .bind(routing_requester_role)
                .bind(tool_call_id)
                .bind(options_json)
                .bind(tool_call_json)
                .bind(now)
                .execute(&db)
                .await
            })
            .await
            .map_err(|err| anyhow::anyhow!("acp permission create join failed: {err}"))??;

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
        reviewed_by_actor_id: Option<String>,
    ) -> anyhow::Result<AcpPermissionRespondResult> {
        let now = Utc::now().timestamp();
        let db = self.db.clone();
        let request_id_for_db = request_id.to_string();
        let rows_affected = self
            .runtime_handle
            .spawn(async move {
                sqlx::query(
                    r#"
                    UPDATE acp_permission_requests
                    SET status = 'responded', selected_option_id = ?1, reviewed_by_actor_id = ?2, responded_at = ?3
                    WHERE id = ?4 AND status = 'pending'
                    "#,
                )
                .bind(selected_option_id)
                .bind(reviewed_by_actor_id)
                .bind(now)
                .bind(request_id_for_db)
                .execute(&db)
                .await
            })
            .await
            .map_err(|err| anyhow::anyhow!("acp permission respond join failed: {err}"))??
            .rows_affected();

        if rows_affected == 0 {
            return Ok(AcpPermissionRespondResult::AlreadyResolved);
        }
        let mut pending = self.pending.lock().await;
        if let Some(sender) = pending.remove(request_id) {
            let _ = sender.send(outcome);
        }
        Ok(AcpPermissionRespondResult::Applied)
    }

    pub async fn belongs_to_agent(&self, request_id: &str, agent_id: &str) -> anyhow::Result<bool> {
        let db = self.db.clone();
        let request_id = request_id.to_string();
        let agent_id = agent_id.to_string();
        let row = self
            .runtime_handle
            .spawn(async move {
                sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM acp_permission_requests WHERE id = ?1 AND agent_id = ?2",
                )
                .bind(request_id)
                .bind(agent_id)
                .fetch_one(&db)
                .await
            })
            .await
            .map_err(|err| anyhow::anyhow!("acp permission ownership join failed: {err}"))??;
        Ok(row > 0)
    }

    pub async fn get(&self, request_id: &str) -> anyhow::Result<Option<AcpPermissionRecord>> {
        let db = self.db.clone();
        let request_id = request_id.to_string();
        let row = self
            .runtime_handle
            .spawn(async move {
                sqlx::query(
                    r#"
                    SELECT id, agent_id, session_id, acp_session_id, team_id, requester_actor_id, requester_role,
                           review_target_actor_id, review_dispatch_status, review_delivery_run_id, review_message_id,
                           reviewed_by_actor_id, human_review_notified_at, tool_call_id, options_json, tool_call_json,
                           status, selected_option_id, created_at, responded_at
                    FROM acp_permission_requests
                    WHERE id = ?1
                    "#,
                )
                .bind(request_id)
                .fetch_optional(&db)
                .await
            })
            .await
            .map_err(|err| anyhow::anyhow!("acp permission get join failed: {err}"))??;
        row.map(parse_permission_record_row).transpose()
    }

    pub async fn record_review_dispatch(
        &self,
        request_id: &str,
        review_target_actor_id: Option<&str>,
        review_dispatch_status: &str,
        review_delivery_run_id: Option<&str>,
        review_message_id: Option<i64>,
    ) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        let db = self.db.clone();
        let request_id = request_id.to_string();
        let review_target_actor_id = review_target_actor_id.map(str::to_string);
        let review_dispatch_status = review_dispatch_status.to_string();
        let review_delivery_run_id = review_delivery_run_id.map(str::to_string);
        self.runtime_handle
            .spawn(async move {
                sqlx::query(
                    r#"
                    UPDATE acp_permission_requests
                    SET review_target_actor_id = ?1,
                        review_dispatch_status = ?2,
                        review_delivery_run_id = ?3,
                        review_message_id = ?4,
                        review_dispatched_at = ?5
                    WHERE id = ?6
                    "#,
                )
                .bind(review_target_actor_id)
                .bind(review_dispatch_status)
                .bind(review_delivery_run_id)
                .bind(review_message_id)
                .bind(now)
                .bind(request_id)
                .execute(&db)
                .await
            })
            .await
            .map_err(|err| anyhow::anyhow!("acp permission dispatch join failed: {err}"))??;
        Ok(())
    }

    pub async fn mark_human_review_notified(&self, request_id: &str) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let db = self.db.clone();
        let request_id = request_id.to_string();
        let rows_affected = self
            .runtime_handle
            .spawn(async move {
                sqlx::query(
                    r#"
                    UPDATE acp_permission_requests
                    SET human_review_notified_at = ?1
                    WHERE id = ?2
                      AND status = 'pending'
                      AND human_review_notified_at IS NULL
                    "#,
                )
                .bind(now)
                .bind(request_id)
                .execute(&db)
                .await
            })
            .await
            .map_err(|err| anyhow::anyhow!("acp permission human notify join failed: {err}"))??
            .rows_affected();
        Ok(rows_affected > 0)
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
        let db = self.db.clone();
        let request_id_for_db = request_id.to_string();
        let rows_affected = self
            .runtime_handle
            .spawn(async move {
                sqlx::query(
                    r#"
                    UPDATE acp_permission_requests
                    SET status = 'timeout', selected_option_id = ?1, responded_at = ?2
                    WHERE id = ?3 AND status = 'pending'
                    "#,
                )
                .bind(selected_option_id)
                .bind(now)
                .bind(request_id_for_db)
                .execute(&db)
                .await
            })
            .await
            .map_err(|err| anyhow::anyhow!("acp permission timeout join failed: {err}"))??
            .rows_affected();
        if rows_affected > 0 {
            let mut pending = self.pending.lock().await;
            pending.remove(request_id);
        }
        Ok(())
    }

    pub async fn list(
        &self,
        agent_id: &str,
        status: Option<&str>,
    ) -> anyhow::Result<Vec<AcpPermissionRecord>> {
        let db = self.db.clone();
        let agent_id = agent_id.to_string();
        let status = status.map(str::to_string);
        let rows = self
            .runtime_handle
            .spawn(async move {
                if let Some(status) = status {
                    sqlx::query(
                        r#"
                        SELECT id, agent_id, session_id, acp_session_id, team_id, requester_actor_id, requester_role,
                               review_target_actor_id, review_dispatch_status, review_delivery_run_id, review_message_id,
                               reviewed_by_actor_id, human_review_notified_at, tool_call_id, options_json, tool_call_json,
                               status, selected_option_id, created_at, responded_at
                        FROM acp_permission_requests
                        WHERE agent_id = ?1 AND status = ?2
                        ORDER BY created_at DESC
                        "#,
                    )
                    .bind(agent_id)
                    .bind(status)
                    .fetch_all(&db)
                    .await
                } else {
                    sqlx::query(
                        r#"
                        SELECT id, agent_id, session_id, acp_session_id, team_id, requester_actor_id, requester_role,
                               review_target_actor_id, review_dispatch_status, review_delivery_run_id, review_message_id,
                               reviewed_by_actor_id, human_review_notified_at, tool_call_id, options_json, tool_call_json,
                               status, selected_option_id, created_at, responded_at
                        FROM acp_permission_requests
                        WHERE agent_id = ?1
                        ORDER BY created_at DESC
                        "#,
                    )
                    .bind(agent_id)
                    .fetch_all(&db)
                    .await
                }
            })
            .await
            .map_err(|err| anyhow::anyhow!("acp permission list join failed: {err}"))??;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(parse_permission_record_row(row)?);
        }
        Ok(out)
    }
}

fn parse_permission_record_row(
    row: sqlx::sqlite::SqliteRow,
) -> anyhow::Result<AcpPermissionRecord> {
    let options_json: String = row.get("options_json");
    let tool_call_json = row
        .try_get::<Option<String>, _>("tool_call_json")
        .unwrap_or(None);
    let options =
        serde_json::from_str::<Vec<AcpPermissionOption>>(&options_json).unwrap_or_default();
    let tool_call = tool_call_json.and_then(|raw| serde_json::from_str(&raw).ok());
    Ok(AcpPermissionRecord {
        id: row.get("id"),
        agent_id: row.get("agent_id"),
        session_id: row.get("session_id"),
        acp_session_id: row
            .try_get::<Option<String>, _>("acp_session_id")
            .unwrap_or(None),
        team_id: row.try_get::<Option<String>, _>("team_id").unwrap_or(None),
        requester_actor_id: row
            .try_get::<Option<String>, _>("requester_actor_id")
            .unwrap_or(None),
        requester_role: row
            .try_get::<Option<String>, _>("requester_role")
            .unwrap_or(None),
        review_target_actor_id: row
            .try_get::<Option<String>, _>("review_target_actor_id")
            .unwrap_or(None),
        review_dispatch_status: row
            .try_get::<Option<String>, _>("review_dispatch_status")
            .unwrap_or(None),
        review_delivery_run_id: row
            .try_get::<Option<String>, _>("review_delivery_run_id")
            .unwrap_or(None),
        review_message_id: row
            .try_get::<Option<i64>, _>("review_message_id")
            .unwrap_or(None),
        reviewed_by_actor_id: row
            .try_get::<Option<String>, _>("reviewed_by_actor_id")
            .unwrap_or(None),
        human_review_notified_at: row
            .try_get::<Option<i64>, _>("human_review_notified_at")
            .unwrap_or(None),
        tool_call_id: row
            .try_get::<Option<String>, _>("tool_call_id")
            .unwrap_or(None),
        options,
        tool_call,
        status: row.get("status"),
        selected_option_id: row
            .try_get::<Option<String>, _>("selected_option_id")
            .unwrap_or(None),
        created_at: row.get("created_at"),
        responded_at: row
            .try_get::<Option<i64>, _>("responded_at")
            .unwrap_or(None),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ACTOR_MAILBOX_MCP_SERVER_NAME, AcpActorSkillContext, AcpCommand, AcpHandle,
        AcpPermissionRespondResult, AcpPermissionService, AcpPromptDeliveryPolicy, AcpSendError,
        build_actor_mailbox_mcp_server, load_mcp_servers_from_path,
        should_queue_while_prompts_active,
    };
    use agent_client_protocol::McpServer;
    use agent_client_protocol::{RequestPermissionOutcome, SelectedPermissionOutcome};
    use sqlx::Row;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::Duration;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    fn test_actor_context() -> AcpActorSkillContext {
        AcpActorSkillContext {
            team_id: Some("team-1".to_string()),
            current_run_id: Some("run-1".to_string()),
            actor_id: "leader".to_string(),
            default_channel: "coordination".to_string(),
            actor_cli_path: "/tmp/agenthub".to_string(),
            member_role: Some("leader".to_string()),
            member_skills: Vec::new(),
            contract_version: None,
            continuity: None,
        }
    }

    fn server_name(server: &McpServer) -> &str {
        match server {
            McpServer::Http(cfg) => cfg.name.as_str(),
            McpServer::Sse(cfg) => cfg.name.as_str(),
            McpServer::Stdio(cfg) => cfg.name.as_str(),
            _ => "unknown",
        }
    }

    struct TempMcpConfig {
        path: PathBuf,
    }

    impl TempMcpConfig {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("agenthub-acp-test-{}", Uuid::new_v4()))
                .join(".agenthub")
                .join("mcp.json");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempMcpConfig {
        fn drop(&mut self) {
            if let Some(root) = self
                .path
                .parent()
                .and_then(Path::parent)
                .filter(|dir| dir.exists())
            {
                let _ = fs::remove_dir_all(root);
            }
        }
    }

    #[tokio::test]
    async fn acp_handle_send_times_out_when_channel_is_backpressured() {
        let (tx, _rx) = mpsc::channel::<AcpCommand>(1);
        tx.try_send(AcpCommand::Cancel).expect("fill queue");
        let handle = AcpHandle {
            session_id: "session-backpressure".to_string(),
            tx,
        };

        let err = handle
            .send_with_timeout(AcpCommand::Cancel, Duration::from_millis(25))
            .await
            .expect_err("send should timeout when queue is full");
        let typed = err
            .downcast_ref::<AcpSendError>()
            .expect("error should be AcpSendError");
        assert_eq!(*typed, AcpSendError::Timeout(Duration::from_millis(25)));
        assert!(
            err.to_string().contains("backpressured"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn acp_handle_send_returns_closed_when_receiver_dropped() {
        let (tx, rx) = mpsc::channel::<AcpCommand>(1);
        drop(rx);
        let handle = AcpHandle {
            session_id: "session-closed".to_string(),
            tx,
        };

        let err = handle
            .send_with_timeout(AcpCommand::Cancel, Duration::from_millis(25))
            .await
            .expect_err("send should fail when channel receiver is dropped");
        let typed = err
            .downcast_ref::<AcpSendError>()
            .expect("error should be AcpSendError");
        assert_eq!(*typed, AcpSendError::ChannelClosed);
        assert!(
            err.to_string().contains("channel closed"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn prompt_delivery_policy_is_provider_aware() {
        assert!(should_queue_while_prompts_active(
            1,
            AcpPromptDeliveryPolicy::StrictFifo,
            false,
            &AcpCommand::Prompt("hello".to_string())
        ));
        assert!(!should_queue_while_prompts_active(
            1,
            AcpPromptDeliveryPolicy::AllowConcurrentPrompts,
            false,
            &AcpCommand::Prompt("hello".to_string())
        ));
        assert!(should_queue_while_prompts_active(
            1,
            AcpPromptDeliveryPolicy::AllowConcurrentPrompts,
            true,
            &AcpCommand::Prompt("hello".to_string())
        ));
        assert!(should_queue_while_prompts_active(
            1,
            AcpPromptDeliveryPolicy::AllowConcurrentPrompts,
            false,
            &AcpCommand::SetMode("auto".to_string())
        ));
        assert!(should_queue_while_prompts_active(
            2,
            AcpPromptDeliveryPolicy::AllowConcurrentPrompts,
            false,
            &AcpCommand::SetModel("gpt-5".to_string())
        ));
        assert!(should_queue_while_prompts_active(
            1,
            AcpPromptDeliveryPolicy::AllowConcurrentPrompts,
            false,
            &AcpCommand::SetConfig {
                config_id: "mode".to_string(),
                value: "auto".to_string(),
            }
        ));
        assert!(!should_queue_while_prompts_active(
            1,
            AcpPromptDeliveryPolicy::AllowConcurrentPrompts,
            true,
            &AcpCommand::Cancel
        ));
        assert!(!should_queue_while_prompts_active(
            0,
            AcpPromptDeliveryPolicy::StrictFifo,
            false,
            &AcpCommand::Prompt("hello".to_string())
        ));
    }

    #[test]
    fn build_actor_mailbox_mcp_server_uses_actor_runtime_binary_and_context() {
        let context = test_actor_context();
        let server = build_actor_mailbox_mcp_server(&context);
        match server {
            McpServer::Stdio(server) => {
                assert_eq!(server.name, ACTOR_MAILBOX_MCP_SERVER_NAME);
                assert_eq!(server.command.to_string_lossy(), "/tmp/agenthub");
                assert_eq!(
                    server.args,
                    vec![
                        "actor-mcp".to_string(),
                        "--actor-id".to_string(),
                        "leader".to_string(),
                        "--channel".to_string(),
                        "coordination".to_string(),
                        "--team-id".to_string(),
                        "team-1".to_string(),
                        "--run-id".to_string(),
                        "run-1".to_string(),
                    ]
                );
            }
            _ => panic!("expected stdio mcp server"),
        }
    }

    #[test]
    fn load_mcp_servers_injects_actor_mailbox_when_config_missing() {
        let config = TempMcpConfig::new();
        let context = test_actor_context();
        let servers = load_mcp_servers_from_path(config.path(), Some(&context));
        assert_eq!(servers.len(), 1);
        assert_eq!(server_name(&servers[0]), ACTOR_MAILBOX_MCP_SERVER_NAME);
    }

    #[test]
    fn load_mcp_servers_appends_actor_mailbox_to_existing_config_servers() {
        let config = TempMcpConfig::new();
        fs::create_dir_all(config.path().parent().expect("mcp config parent"))
            .expect("create mcp config parent");
        fs::write(
            config.path(),
            r#"
            {
              "mcpServers": {
                "local-stdio": {
                  "command": "node",
                  "args": ["server.js"]
                }
              }
            }
            "#,
        )
        .expect("write mcp config");

        let context = test_actor_context();
        let servers = load_mcp_servers_from_path(config.path(), Some(&context));
        assert_eq!(servers.len(), 2);
        let names = servers.iter().map(server_name).collect::<Vec<_>>();
        assert!(names.contains(&"local-stdio"));
        assert!(names.contains(&ACTOR_MAILBOX_MCP_SERVER_NAME));
    }

    #[tokio::test]
    async fn permission_service_respond_reports_already_resolved_after_first_winner() {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect sqlite");
        sqlx::query(
            r#"
            CREATE TABLE acp_permission_requests (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                acp_session_id TEXT,
                team_id TEXT,
                requester_actor_id TEXT,
                requester_role TEXT,
                review_target_actor_id TEXT,
                review_dispatch_status TEXT,
                review_delivery_run_id TEXT,
                review_message_id INTEGER,
                review_dispatched_at INTEGER,
                reviewed_by_actor_id TEXT,
                human_review_notified_at INTEGER,
                tool_call_id TEXT,
                options_json TEXT NOT NULL,
                tool_call_json TEXT,
                status TEXT NOT NULL,
                selected_option_id TEXT,
                created_at INTEGER NOT NULL,
                responded_at INTEGER
            )
            "#,
        )
        .execute(&db)
        .await
        .expect("create permission table");
        let service = AcpPermissionService::new(db.clone());
        let request_id = "perm-service-race".to_string();
        sqlx::query(
            r#"
            INSERT INTO acp_permission_requests (
                id, agent_id, session_id, acp_session_id, tool_call_id, options_json, tool_call_json, status, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8)
            "#,
        )
        .bind(&request_id)
        .bind("agent-1")
        .bind("session-1")
        .bind("acp-session-1")
        .bind("tool-call-1")
        .bind(
            serde_json::json!([
                {
                    "option_id": "allow",
                    "name": "Allow once",
                    "kind": "allow_once"
                }
            ])
            .to_string(),
        )
        .bind(serde_json::json!({"tool":{"name":"mcp__fs__read"}}).to_string())
        .bind(chrono::Utc::now().timestamp())
        .execute(&db)
        .await
        .expect("insert permission request");

        let first = service
            .respond(
                &request_id,
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new("allow")),
                Some("allow".to_string()),
                Some("leader".to_string()),
            )
            .await
            .expect("first respond");
        assert_eq!(first, AcpPermissionRespondResult::Applied);

        let second = service
            .respond(
                &request_id,
                RequestPermissionOutcome::Cancelled,
                None,
                Some("worker".to_string()),
            )
            .await
            .expect("second respond");
        assert_eq!(second, AcpPermissionRespondResult::AlreadyResolved);

        let row = sqlx::query(
            "SELECT status, selected_option_id, reviewed_by_actor_id FROM acp_permission_requests WHERE id = ?1",
        )
        .bind(&request_id)
        .fetch_one(&db)
        .await
        .expect("reload permission request");
        assert_eq!(row.get::<String, _>("status"), "responded");
        assert_eq!(row.get::<String, _>("selected_option_id"), "allow");
        assert_eq!(row.get::<String, _>("reviewed_by_actor_id"), "leader");
    }
}
