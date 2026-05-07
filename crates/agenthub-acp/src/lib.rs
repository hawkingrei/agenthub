mod actor_runtime_skill;
mod team_role_skills;
#[cfg(test)]
mod test_utils;

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use agent_client_protocol::schema::{
    CancelNotification, ClientCapabilities, ContentBlock, ContentChunk, Implementation,
    InitializeRequest, LoadSessionRequest, McpServer, NewSessionRequest, PermissionOption,
    PermissionOptionKind, PromptRequest, ProtocolVersion, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SessionNotification, SessionUpdate,
    SetSessionConfigOptionRequest, SetSessionModeRequest, SetSessionModelRequest, TextContent,
    ToolCall, ToolCallUpdate, ToolCallUpdateFields,
};
use agent_client_protocol::{Agent, ConnectionTo, Error as AcpError, ErrorCode as AcpErrorCode};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use sqlx::{Row, SqlitePool};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::runtime::Handle;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use uuid::Uuid;

use actor_runtime_skill::{build_actor_runtime_context_block, build_actor_runtime_skill};
use agenthub_acp_core::{
    AcpSkill, build_skill, build_skill_blocks, build_skills_meta, expand_tilde, extract_skill_name,
    filter_mcp_servers, parse_mcp_config, parse_skills_config,
};
use agenthub_config::path_utils::{is_path_allowed, normalize_path};
use agenthub_managed_skills::install_managed_skills;
use team_role_skills::{
    build_team_role_skills, is_reserved_team_role_skill, should_attach_team_role_skills,
};

const MCP_CONFIG_FILE: &str = ".agenthub/mcp.json";
const SKILLS_CONFIG_FILE: &str = ".agenthub/skills.json";
const ACP_COMMAND_CHANNEL_CAPACITY: usize = 64;
const ACP_COMMAND_SEND_TIMEOUT: Duration = Duration::from_secs(5);
// Team workers may need a long resume/bootstrap window after service restarts,
// so ACP startup should tolerate late session readiness instead of failing fast.
const ACP_SESSION_START_TIMEOUT: Duration = Duration::from_secs(300);
const ACP_PERMISSION_REVIEW_TIMEOUT: Duration = Duration::from_secs(120);

type AcpClientConnection = ConnectionTo<Agent>;

pub const fn acp_permission_review_timeout() -> Duration {
    ACP_PERMISSION_REVIEW_TIMEOUT
}

pub const fn acp_session_start_timeout() -> Duration {
    ACP_SESSION_START_TIMEOUT
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpActorContinuityEnvelope {
    pub mode: String,
    pub source_run_id: String,
    pub source_session_id: Option<String>,
    pub summary_text: String,
    pub history_window: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpActorSkillContext {
    pub team_id: Option<String>,
    pub current_run_id: Option<String>,
    pub actor_id: String,
    pub default_channel: String,
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
    pub provider_id: String,
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
    pub runtime_location: AcpRuntimeLocation,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AcpRuntimeLocation {
    #[default]
    LocalProcess,
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

fn load_mcp_servers_from_path(path: &Path) -> Vec<McpServer> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == ErrorKind::NotFound => return Vec::new(),
        Err(err) => {
            tracing::warn!(
                "mcp config read failed: path={} error={}",
                path.display(),
                err
            );
            return Vec::new();
        }
    };
    match parse_mcp_config(&contents) {
        Ok(parsed_servers) => parsed_servers,
        Err(err) => {
            tracing::warn!(
                "mcp config parse failed: path={} error={}",
                path.display(),
                err
            );
            Vec::new()
        }
    }
}

fn load_mcp_servers() -> Vec<McpServer> {
    let path = mcp_config_path();
    load_mcp_servers_from_path(&path)
}

fn mcp_server_name(server: &McpServer) -> &str {
    match server {
        McpServer::Http(cfg) => cfg.name.as_str(),
        McpServer::Sse(cfg) => cfg.name.as_str(),
        McpServer::Stdio(cfg) => cfg.name.as_str(),
        _ => "unknown",
    }
}

fn default_skill_name_for_path(path: &Path) -> String {
    let stem = path.file_stem().and_then(|value| value.to_str());
    if matches!(stem, Some("SKILL"))
        && let Some(parent_name) = path
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
    {
        return parent_name.to_string();
    }
    stem.unwrap_or("skill").to_string()
}

fn load_skill_from_path(
    path_buf: &Path,
    explicit_name: Option<String>,
    safe_paths: &[String],
) -> Option<AcpSkill> {
    if !is_skill_path_allowed(path_buf, safe_paths) {
        tracing::warn!(
            "skill skipped: path={} reason=not allowed",
            path_buf.display()
        );
        return None;
    }
    let contents = match fs::read_to_string(path_buf) {
        Ok(contents) => contents,
        Err(err) => {
            tracing::warn!(
                "skills file read failed: path={} error={}",
                path_buf.display(),
                err
            );
            return None;
        }
    };
    let name = explicit_name
        .or_else(|| extract_skill_name(&contents))
        .unwrap_or_else(|| default_skill_name_for_path(path_buf));
    let path_display = path_buf.to_string_lossy().to_string();
    Some(build_skill(name, path_display, &contents))
}

fn load_skills_from_config(path: &Path, safe_paths: &[String]) -> Vec<AcpSkill> {
    let contents = match fs::read_to_string(path) {
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
        if let Some(skill) = load_skill_from_path(&path_buf, entry.name, safe_paths) {
            skills.push(skill);
        }
    }
    skills
}

fn collect_workdir_skill_paths(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            tracing::warn!(
                "skill discovery skipped: path={} error={}",
                dir.display(),
                err
            );
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                tracing::warn!(
                    "skill discovery entry read failed: path={} error={}",
                    dir.display(),
                    err
                );
                continue;
            }
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(err) => {
                tracing::warn!(
                    "skill discovery entry type failed: path={} error={}",
                    entry.path().display(),
                    err
                );
                continue;
            }
        };
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            collect_workdir_skill_paths(&path, out);
            continue;
        }
        if file_type.is_file()
            && path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value == "SKILL.md")
        {
            out.push(path);
        }
    }
}

fn load_workdir_skills(workdir: &Path, safe_paths: &[String]) -> Vec<AcpSkill> {
    let skills_dir = workdir.join(".agents").join("skills");
    if !skills_dir.exists() {
        return Vec::new();
    }
    if safe_paths.is_empty() {
        tracing::warn!("workdir skills skipped: no safe paths configured");
        return Vec::new();
    }
    match fs::symlink_metadata(&skills_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            tracing::warn!(
                "workdir skills skipped: skills_dir is a symlink: path={}",
                skills_dir.display()
            );
            return Vec::new();
        }
        Ok(_) => {}
        Err(err) => {
            tracing::warn!(
                "workdir skills skipped: failed to stat skills_dir: path={} error={}",
                skills_dir.display(),
                err
            );
            return Vec::new();
        }
    }

    let mut skill_paths = Vec::new();
    collect_workdir_skill_paths(&skills_dir, &mut skill_paths);
    skill_paths.sort();

    skill_paths
        .into_iter()
        .filter_map(|path| load_skill_from_path(&path, None, safe_paths))
        .collect()
}

fn load_skills(workdir: &Path, safe_paths: &[String]) -> Vec<AcpSkill> {
    let mut skills = load_workdir_skills(workdir, safe_paths);
    skills.extend(load_skills_from_config(&skills_config_path(), safe_paths));
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

fn remove_skills_conflicting_with_reserved(existing: &mut Vec<AcpSkill>, reserved: &[AcpSkill]) {
    if reserved.is_empty() {
        return;
    }

    let reserved_names = reserved
        .iter()
        .map(|skill| skill.name.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let reserved_paths = reserved
        .iter()
        .map(|skill| skill.path.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    existing.retain(|skill| {
        !reserved_names.contains(&skill.name.to_ascii_lowercase())
            && !reserved_paths.contains(&skill.path.to_ascii_lowercase())
    });
}

fn build_prompt_prefix_blocks(
    skills: &[AcpSkill],
    actor_context: Option<&AcpActorSkillContext>,
) -> Vec<ContentBlock> {
    let mut blocks = build_skill_blocks(skills);
    if let Some(context) = actor_context {
        blocks.push(build_actor_runtime_context_block(context));
    }
    blocks
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

impl AcpClient {
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

        let outcome = match tokio::time::timeout(ACP_PERMISSION_REVIEW_TIMEOUT, response_rx).await {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(_)) => {
                let fallback = permission_review_failure_outcome();
                let _ = self.permissions.mark_timeout(&request_id, None).await;
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
            Err(_) => {
                let fallback = permission_review_failure_outcome();
                let _ = self.permissions.mark_timeout(&request_id, None).await;
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

fn permission_review_failure_outcome() -> RequestPermissionOutcome {
    RequestPermissionOutcome::Cancelled
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

async fn send_acp_request<Req>(
    conn: &AcpClientConnection,
    request: Req,
) -> Result<Req::Response, AcpError>
where
    Req: agent_client_protocol::JsonRpcRequest,
{
    conn.send_request(request).block_task().await
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
    conn: Rc<AcpClientConnection>,
    event_sink: Arc<dyn AcpEventSink>,
    session_id: &str,
    prompt_prefix_blocks: &[ContentBlock],
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
            let mut blocks = Vec::with_capacity(prompt_prefix_blocks.len() + 1);
            blocks.extend(prompt_prefix_blocks.iter().cloned());
            blocks.push(ContentBlock::Text(TextContent::new(prompt)));
            tokio::task::spawn_local(async move {
                let request = PromptRequest::new(session_id, blocks);
                if let Err(err) = send_acp_request(&conn, request).await {
                    event_sink
                        .emit_raw(AcpStream::System, format!("acp prompt error: {err}"))
                        .await;
                }
                let _ = prompt_done_tx.send(());
            });
        }
        AcpCommand::SetMode(mode_id) => {
            let request = SetSessionModeRequest::new(session_id.to_string(), mode_id);
            if let Err(err) = send_acp_request(&conn, request).await {
                event_sink
                    .emit_raw(AcpStream::System, format!("acp set_mode error: {err}"))
                    .await;
            }
        }
        AcpCommand::SetModel(model_id) => {
            let request = SetSessionModelRequest::new(session_id.to_string(), model_id);
            if let Err(err) = send_acp_request(&conn, request).await {
                event_sink
                    .emit_raw(AcpStream::System, format!("acp set_model error: {err}"))
                    .await;
            }
        }
        AcpCommand::SetConfig { config_id, value } => {
            let request = SetSessionConfigOptionRequest::new(
                session_id.to_string(),
                config_id,
                value.as_str(),
            );
            if let Err(err) = send_acp_request(&conn, request).await {
                event_sink
                    .emit_raw(AcpStream::System, format!("acp set_config error: {err}"))
                    .await;
            }
        }
        AcpCommand::Cancel => {
            let request = CancelNotification::new(session_id.to_string());
            if let Err(err) = conn.send_notification(request) {
                event_sink
                    .emit_raw(AcpStream::System, format!("acp cancel error: {err}"))
                    .await;
            }
        }
    }
}

async fn handle_auth_required_failure(
    err: &AcpError,
    provider_id: &str,
    action: &str,
    event_sink: &dyn AcpEventSink,
) -> Option<String> {
    if !is_auth_required_error(err) {
        return None;
    }

    let message = format_auth_required_message(provider_id);
    event_sink
        .emit_raw(
            AcpStream::System,
            format!("acp {action} auth required: {message}"),
        )
        .await;
    Some(message)
}

pub async fn spawn_acp_session(request: SpawnAcpSessionRequest) -> anyhow::Result<AcpHandle> {
    let SpawnAcpSessionRequest {
        provider_id,
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
        runtime_location,
    } = request;
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<AcpCommand>(ACP_COMMAND_CHANNEL_CAPACITY);
    let (ready_tx, ready_rx) = oneshot::channel::<Result<String, String>>();

    std::thread::spawn(move || match runtime_location {
        AcpRuntimeLocation::LocalProcess => {
            if actor_context.is_some()
                && let Err(err) = install_managed_skills(None)
            {
                let _ = ready_tx.send(Err(format!("acp managed skill install failed: {err}")));
                return;
            }
            let mcp_servers = load_mcp_servers();
            let mut skills = load_skills(Path::new(&workdir), &safe_paths);
            skills.retain(|skill| !is_reserved_team_role_skill(skill.name.as_str()));
            let mut attached_team_role_skills = false;
            if let Some(ctx) = actor_context.as_ref() {
                if should_attach_team_role_skills(Some(ctx)) {
                    let team_role_skills = match build_team_role_skills(ctx) {
                        Ok(team_role_skills) => team_role_skills,
                        Err(err) => {
                            let _ = ready_tx
                                .send(Err(format!("acp managed team skill load failed: {err}")));
                            return;
                        }
                    };
                    remove_skills_conflicting_with_reserved(&mut skills, &team_role_skills);
                    skills.extend(team_role_skills);
                    attached_team_role_skills = true;
                }
                let actor_runtime_skill = match build_actor_runtime_skill() {
                    Ok(actor_runtime_skill) => actor_runtime_skill,
                    Err(err) => {
                        let _ = ready_tx
                            .send(Err(format!("acp actor runtime skill load failed: {err}")));
                        return;
                    }
                };
                remove_skills_conflicting_with_reserved(
                    &mut skills,
                    std::slice::from_ref(&actor_runtime_skill),
                );
                skills.push(actor_runtime_skill);
            }
            let skills = dedupe_skills(skills);
            if let Some(ctx) = actor_context.as_ref() {
                let skill_names = skills
                    .iter()
                    .map(|skill| skill.name.as_str())
                    .collect::<Vec<_>>();
                let mcp_server_names = mcp_servers.iter().map(mcp_server_name).collect::<Vec<_>>();
                tracing::info!(
                    team_id = %ctx.team_id.as_deref().unwrap_or("none"),
                    current_run_id = %ctx.current_run_id.as_deref().unwrap_or("none"),
                    actor_id = %ctx.actor_id,
                    member_role = %ctx.member_role.as_deref().unwrap_or("none"),
                    attached_team_role_skills,
                    skill_names = ?skill_names,
                    mcp_server_names = ?mcp_server_names,
                    "acp actor session bootstrap prepared runtime capabilities"
                );
            }
            let prompt_prefix_blocks = build_prompt_prefix_blocks(&skills, actor_context.as_ref());
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
                let transport =
                    agent_client_protocol::ByteStreams::new(stdin.compat_write(), stdout.compat());
                let client_for_permission = client.clone();
                let client_for_notifications = client.clone();
                let event_sink_for_connection = event_sink.clone();
                let connect_result = agent_client_protocol::Client
                .builder()
                .on_receive_request(
                    async move |request: RequestPermissionRequest, responder, _connection| {
                        responder.respond_with_result(
                            client_for_permission.request_permission(request).await,
                        )
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_notification(
                    async move |notification: SessionNotification, _connection| {
                        client_for_notifications
                            .session_notification(notification)
                            .await
                    },
                    agent_client_protocol::on_receive_notification!(),
                )
                .connect_with(transport, |conn: AcpClientConnection| async move {
            let event_sink = event_sink_for_connection;

            let init = InitializeRequest::new(ProtocolVersion::V1)
                .client_capabilities(ClientCapabilities::default())
                .client_info(client_info);

            let init_response = match send_acp_request(&conn, init).await {
                Ok(response) => response,
                Err(err) => {
                    let _ = ready_tx.send(Err(format!("acp initialize failed: {err}")));
                    return Ok(());
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
                match send_acp_request(&conn, request).await {
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
                        if let Some(message) = handle_auth_required_failure(
                            &err,
                            &provider_id,
                            "load_session",
                            event_sink.as_ref(),
                        )
                        .await
                        {
                            let _ = ready_tx.send(Err(message));
                            return Ok(());
                        }
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
                let session = match send_acp_request(&conn, request).await {
                    Ok(session) => session,
                    Err(err) => {
                        if let Some(message) = handle_auth_required_failure(
                            &err,
                            &provider_id,
                            "new_session",
                            event_sink.as_ref(),
                        )
                        .await
                        {
                            let _ = ready_tx.send(Err(message));
                            return Ok(());
                        }
                        let _ = ready_tx.send(Err(format!("acp new_session failed: {err}")));
                        return Ok(());
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
                        &prompt_prefix_blocks,
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
                                    &prompt_prefix_blocks,
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
            Ok(())
                })
                .await;

                if let Err(err) = connect_result {
                    event_sink
                        .emit_raw(AcpStream::System, format!("acp io error: {err}"))
                        .await;
                }
            }));
        }
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

fn is_auth_required_error(err: &AcpError) -> bool {
    err.code == AcpErrorCode::AuthRequired
}

fn format_auth_required_message(provider_id: &str) -> String {
    match provider_id {
        "gemini" => "Gemini ACP requires prior authentication on the host. AgentHub does not trigger interactive Google login for remote sessions; authenticate Gemini CLI on the host first.".to_string(),
        _ => format!(
            "ACP authentication is required for provider '{provider_id}' before AgentHub can create or resume a session."
        ),
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

    pub async fn has_pending_review_for_actor(
        &self,
        team_id: &str,
        actor_id: &str,
    ) -> anyhow::Result<bool> {
        let db = self.db.clone();
        let team_id = team_id.to_string();
        let actor_id = actor_id.to_string();
        let row = self
            .runtime_handle
            .spawn(async move {
                sqlx::query_scalar::<_, i64>(
                    r#"
                    SELECT COUNT(*)
                    FROM acp_permission_requests
                    WHERE team_id = ?1
                      AND review_target_actor_id = ?2
                      AND status = 'pending'
                    "#,
                )
                .bind(team_id)
                .bind(actor_id)
                .fetch_one(&db)
                .await
            })
            .await
            .map_err(|err| {
                anyhow::anyhow!("acp permission reviewer lookup join failed: {err}")
            })??;
        Ok(row > 0)
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
        ACP_PERMISSION_REVIEW_TIMEOUT, AcpActorContinuityEnvelope, AcpActorSkillContext,
        AcpCommand, AcpHandle, AcpPermissionRespondResult, AcpPermissionService,
        AcpPromptDeliveryPolicy, AcpRuntimeLocation, AcpSendError, acp_permission_review_timeout,
        acp_session_start_timeout, build_prompt_prefix_blocks, dedupe_skills,
        format_auth_required_message, handle_auth_required_failure, is_auth_required_error,
        load_mcp_servers_from_path, load_skills_from_config, load_workdir_skills,
        permission_review_failure_outcome, remove_skills_conflicting_with_reserved,
        should_queue_while_prompts_active,
    };
    use agent_client_protocol::schema::{
        ContentBlock, McpServer, RequestPermissionOutcome, SelectedPermissionOutcome,
    };
    use agent_client_protocol::{Error as AcpError, ErrorCode as AcpErrorCode};
    use agenthub_acp_core::build_skill;
    use agenthub_managed_skills::{
        ManagedSkillKind, install_managed_skills, managed_skill_doc_path,
    };
    use sqlx::Row;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs as unix_fs;
    use std::path::{Path, PathBuf};
    use std::time::Duration;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    use crate::test_utils::TempManagedSkillsHome;

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

    struct TempSkillWorkspace {
        root: PathBuf,
        workdir: PathBuf,
        config_path: PathBuf,
    }

    impl TempSkillWorkspace {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("agenthub-acp-skills-{}", Uuid::new_v4()));
            let workdir = root.join("repo");
            let config_path = root.join(".agenthub").join("skills.json");
            fs::create_dir_all(&workdir).expect("create workdir");
            Self {
                root,
                workdir,
                config_path,
            }
        }

        fn workdir(&self) -> &Path {
            &self.workdir
        }

        fn config_path(&self) -> &Path {
            &self.config_path
        }

        fn safe_paths(&self) -> Vec<String> {
            vec![self.root.to_string_lossy().to_string()]
        }

        fn create_workdir_skill(&self, relative_dir: &str, contents: &str) -> PathBuf {
            let path = self
                .workdir
                .join(".agents")
                .join("skills")
                .join(relative_dir)
                .join("SKILL.md");
            fs::create_dir_all(path.parent().expect("workdir skill parent"))
                .expect("create workdir skill parent");
            fs::write(&path, contents).expect("write workdir skill");
            path
        }

        fn create_global_skill(&self, relative_dir: &str, contents: &str) -> PathBuf {
            let path = self
                .root
                .join("global-skills")
                .join(relative_dir)
                .join("SKILL.md");
            fs::create_dir_all(path.parent().expect("global skill parent"))
                .expect("create global skill parent");
            fs::write(&path, contents).expect("write global skill");
            path
        }

        fn write_skills_config(&self, skill_paths: &[PathBuf]) {
            fs::create_dir_all(self.config_path.parent().expect("skills config parent"))
                .expect("create skills config parent");
            let skills = skill_paths
                .iter()
                .map(|path| serde_json::Value::String(path.to_string_lossy().to_string()))
                .collect::<Vec<_>>();
            fs::write(
                &self.config_path,
                serde_json::json!({ "skills": skills }).to_string(),
            )
            .expect("write skills config");
        }
    }

    impl Drop for TempSkillWorkspace {
        fn drop(&mut self) {
            if self.root.exists() {
                let _ = fs::remove_dir_all(&self.root);
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
    fn acp_runtime_location_defaults_to_local_process() {
        assert_eq!(
            AcpRuntimeLocation::default(),
            AcpRuntimeLocation::LocalProcess
        );
    }

    #[test]
    fn auth_required_error_detection_matches_protocol_error_code() {
        let err = AcpError::auth_required();
        assert!(is_auth_required_error(&err));

        let other = AcpError::new(AcpErrorCode::InternalError.into(), "boom");
        assert!(!is_auth_required_error(&other));
    }

    #[test]
    fn gemini_auth_required_message_mentions_host_side_authentication() {
        let message = format_auth_required_message("gemini");
        assert!(message.contains("prior authentication on the host"));
        assert!(message.contains("does not trigger interactive Google login"));
    }

    struct NoopEventSink;

    #[async_trait::async_trait]
    impl super::AcpEventSink for NoopEventSink {
        async fn emit_raw(&self, _stream: super::AcpStream, _message: String) {}
    }

    #[tokio::test]
    async fn auth_required_failure_helper_returns_provider_message() {
        let sink: std::sync::Arc<dyn super::AcpEventSink> = std::sync::Arc::new(NoopEventSink);
        let handled = handle_auth_required_failure(
            &AcpError::auth_required(),
            "gemini",
            "new_session",
            sink.as_ref(),
        )
        .await;

        let err = handled.expect("auth required should produce a startup error");
        assert!(err.contains("Gemini ACP requires prior authentication on the host"));
    }

    #[test]
    fn load_mcp_servers_returns_empty_when_config_missing() {
        let config = TempMcpConfig::new();
        let servers = load_mcp_servers_from_path(config.path());
        assert!(servers.is_empty());
    }

    #[test]
    fn load_mcp_servers_preserves_existing_config_servers() {
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

        let servers = load_mcp_servers_from_path(config.path());
        assert_eq!(servers.len(), 1);
        let names = servers.iter().map(server_name).collect::<Vec<_>>();
        assert!(names.contains(&"local-stdio"));
    }

    #[test]
    fn load_workdir_skills_discovers_nested_skill_markdown_files() {
        let workspace = TempSkillWorkspace::new();
        workspace.create_workdir_skill(
            "alpha/research",
            r#"---
name: "research-skill"
---
Use the repo-local research workflow.
"#,
        );
        workspace.create_workdir_skill(
            "beta/implementation",
            r#"---
name: "implementation-skill"
---
Use the repo-local implementation workflow.
"#,
        );

        let skills = load_workdir_skills(workspace.workdir(), &workspace.safe_paths());
        let names = skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["research-skill", "implementation-skill"]);
    }

    #[test]
    fn load_workdir_skills_uses_parent_directory_name_as_fallback() {
        let workspace = TempSkillWorkspace::new();
        let skill_path = workspace.create_workdir_skill(
            "incident-response",
            "Respond to incidents with the repo-local workflow.\n",
        );

        let skills = load_workdir_skills(workspace.workdir(), &workspace.safe_paths());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "incident-response");
        assert_eq!(skills[0].path, skill_path.to_string_lossy());
    }

    #[test]
    fn load_workdir_skills_respects_safe_paths() {
        let workspace = TempSkillWorkspace::new();
        workspace.create_workdir_skill(
            "ops",
            r#"---
name: "ops-skill"
---
Use the repo-local ops workflow.
"#,
        );

        let skills = load_workdir_skills(workspace.workdir(), &[]);
        assert!(skills.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn load_workdir_skills_skips_symlinked_directories() {
        let workspace = TempSkillWorkspace::new();
        workspace.create_workdir_skill(
            "primary",
            r#"---
name: "primary-skill"
---
Use the repo-local primary workflow.
"#,
        );
        let loop_dir = workspace
            .workdir()
            .join(".agents")
            .join("skills")
            .join("loop");
        unix_fs::symlink(
            workspace.workdir().join(".agents").join("skills"),
            &loop_dir,
        )
        .expect("create loop symlink");

        let skills = load_workdir_skills(workspace.workdir(), &workspace.safe_paths());
        let names = skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["primary-skill"]);
    }

    #[cfg(unix)]
    #[test]
    fn load_workdir_skills_skips_symlinked_skills_root() {
        let workspace = TempSkillWorkspace::new();
        let real_dir = workspace.root.join("detached-skills-root");
        fs::create_dir_all(real_dir.join("shadow")).expect("create detached root");
        fs::write(
            real_dir.join("shadow").join("SKILL.md"),
            r#"---
name: "shadow-skill"
---
Use the detached skill root.
"#,
        )
        .expect("write detached root skill");

        let agents_dir = workspace.workdir().join(".agents");
        fs::create_dir_all(&agents_dir).expect("create .agents directory");
        unix_fs::symlink(&real_dir, agents_dir.join("skills")).expect("symlink skills root");

        let skills = load_workdir_skills(workspace.workdir(), &workspace.safe_paths());
        assert!(skills.is_empty());
    }

    #[test]
    fn repo_local_skills_take_precedence_over_global_config_skills() {
        let workspace = TempSkillWorkspace::new();
        let local_skill = workspace.create_workdir_skill(
            "review",
            r#"---
name: "shared-review"
---
Prefer the repo-local review contract.
"#,
        );
        let global_skill = workspace.create_global_skill(
            "review",
            r#"---
name: "shared-review"
---
Fallback to the user-level review contract.
"#,
        );
        workspace.write_skills_config(std::slice::from_ref(&global_skill));

        let mut skills = load_workdir_skills(workspace.workdir(), &workspace.safe_paths());
        skills.extend(load_skills_from_config(
            workspace.config_path(),
            &workspace.safe_paths(),
        ));
        let deduped = dedupe_skills(skills);

        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].name, "shared-review");
        assert_eq!(deduped[0].path, local_skill.to_string_lossy());
    }

    #[test]
    fn reserved_skills_take_precedence_over_preloaded_name_and_path_aliases() {
        let reserved = build_skill(
            "agenthub-actor-runtime".to_string(),
            "/tmp/runtime/actor/SKILL.md".to_string(),
            "---\nname: agenthub-actor-runtime\n---\nCanonical runtime skill.\n",
        );
        let mut skills = vec![
            build_skill(
                "agenthub-actor-runtime".to_string(),
                "/tmp/custom/runtime-alias/SKILL.md".to_string(),
                "---\nname: agenthub-actor-runtime\n---\nAlias by name.\n",
            ),
            build_skill(
                "custom-runtime-alias".to_string(),
                reserved.path.clone(),
                "---\nname: custom-runtime-alias\n---\nAlias by path.\n",
            ),
            build_skill(
                "shared-review".to_string(),
                "/tmp/review/SKILL.md".to_string(),
                "---\nname: shared-review\n---\nUnrelated skill.\n",
            ),
        ];

        remove_skills_conflicting_with_reserved(&mut skills, std::slice::from_ref(&reserved));
        skills.push(reserved.clone());
        let deduped = dedupe_skills(skills);

        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].name, "shared-review");
        assert_eq!(deduped[1].name, reserved.name);
        assert_eq!(deduped[1].path, reserved.path);
    }

    fn sample_actor_context() -> AcpActorSkillContext {
        AcpActorSkillContext {
            team_id: Some("team-7".to_string()),
            current_run_id: Some("run-42".to_string()),
            actor_id: "planner".to_string(),
            default_channel: "coordination".to_string(),
            member_role: Some("coordinator".to_string()),
            member_skills: Vec::new(),
            contract_version: Some("v1".to_string()),
            continuity: Some(AcpActorContinuityEnvelope {
                mode: "resume".to_string(),
                source_run_id: "run-41".to_string(),
                source_session_id: Some("session-41".to_string()),
                summary_text: "Continue implementation with the same branch.".to_string(),
                history_window: serde_json::json!({"messages": 3}),
            }),
        }
    }

    #[test]
    fn prompt_prefix_blocks_append_runtime_context_after_skills() {
        let skill = build_skill(
            "demo-skill".to_string(),
            "/tmp/demo-skill/SKILL.md".to_string(),
            "---\nname: demo-skill\n---\nUse the demo skill.\n",
        );
        let blocks = build_prompt_prefix_blocks(&[skill], Some(&sample_actor_context()));

        assert_eq!(blocks.len(), 2);
        let ContentBlock::Text(skill_block) = &blocks[0] else {
            panic!("expected first prompt prefix block to be text");
        };
        assert!(skill_block.text.starts_with("<skill>\n"));

        let ContentBlock::Text(runtime_block) = &blocks[1] else {
            panic!("expected runtime context block to be text");
        };
        assert!(runtime_block.text.contains("AgentHub runtime context:"));
        assert!(runtime_block.text.contains("actor_id: planner"));
        assert!(
            runtime_block
                .text
                .contains("current_execution_run_id: run-42")
        );
        assert!(
            runtime_block
                .text
                .contains("continuity_source_execution_run_id: run-41")
        );
        assert!(
            runtime_block
                .text
                .contains("continuity_state_path: .cache/context/state.md")
        );
        assert!(
            runtime_block
                .text
                .contains("continuity_note_path: .cache/context/run/run-41/continuity.md")
        );
        assert!(!runtime_block.text.contains("continuity_summary:"));
    }

    #[test]
    fn prompt_prefix_blocks_skip_runtime_context_without_actor_context() {
        let skill = build_skill(
            "demo-skill".to_string(),
            "/tmp/demo-skill/SKILL.md".to_string(),
            "---\nname: demo-skill\n---\nUse the demo skill.\n",
        );
        let blocks = build_prompt_prefix_blocks(&[skill], None);

        assert_eq!(blocks.len(), 1);
        let ContentBlock::Text(skill_block) = &blocks[0] else {
            panic!("expected text skill block");
        };
        assert!(skill_block.text.starts_with("<skill>\n"));
    }

    #[test]
    fn prompt_prefix_blocks_keep_managed_skill_file_static_and_runtime_context_dynamic() {
        let home = TempManagedSkillsHome::new("agenthub-acp-runtime-skill-prefix-home");
        install_managed_skills(Some(home.path())).expect("install managed skills");
        let skill = super::actor_runtime_skill::build_required_managed_skill(
            ManagedSkillKind::ActorRuntime,
            Some(home.path()),
        )
        .expect("build managed actor runtime skill");
        let expected_path =
            managed_skill_doc_path(ManagedSkillKind::ActorRuntime, Some(home.path()))
                .expect("resolve actor runtime skill path");

        let blocks = build_prompt_prefix_blocks(&[skill], Some(&sample_actor_context()));

        assert_eq!(blocks.len(), 2);

        let ContentBlock::Text(skill_block) = &blocks[0] else {
            panic!("expected first prompt prefix block to be text");
        };
        assert!(skill_block.text.starts_with("<skill>\n"));
        assert!(skill_block.text.contains("agenthub-actor-runtime"));
        assert!(
            skill_block
                .text
                .contains(&format!("<path>{}</path>", expected_path.display()))
        );
        assert!(
            !skill_block
                .text
                .contains("current_execution_run_id: run-42")
        );
        assert!(!skill_block.text.contains("actor_id: planner"));

        let ContentBlock::Text(runtime_block) = &blocks[1] else {
            panic!("expected runtime context block to be text");
        };
        assert!(runtime_block.text.contains("AgentHub runtime context:"));
        assert!(
            runtime_block
                .text
                .contains("current_execution_run_id: run-42")
        );
        assert!(runtime_block.text.contains("actor_id: planner"));
        assert!(!runtime_block.text.contains("AgentHub Actor Runtime Skill"));
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
                Some("coordinator".to_string()),
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
        assert_eq!(row.get::<String, _>("reviewed_by_actor_id"), "coordinator");
    }

    #[tokio::test]
    async fn permission_timeout_rejects_without_selecting_allow_option() {
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
        let request_id = "perm-service-timeout".to_string();
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
        .bind(serde_json::json!({"tool":{"name":"mcp__fs__write"}}).to_string())
        .bind(chrono::Utc::now().timestamp())
        .execute(&db)
        .await
        .expect("insert permission request");

        assert!(matches!(
            permission_review_failure_outcome(),
            RequestPermissionOutcome::Cancelled
        ));

        service
            .mark_timeout(&request_id, Some(&permission_review_failure_outcome()))
            .await
            .expect("mark timeout");

        let row = sqlx::query(
            "SELECT status, selected_option_id FROM acp_permission_requests WHERE id = ?1",
        )
        .bind(&request_id)
        .fetch_one(&db)
        .await
        .expect("reload permission request");
        assert_eq!(row.get::<String, _>("status"), "timeout");
        assert_eq!(row.get::<Option<String>, _>("selected_option_id"), None);
    }

    #[test]
    fn permission_review_failure_outcome_is_cancelled() {
        assert!(matches!(
            permission_review_failure_outcome(),
            RequestPermissionOutcome::Cancelled
        ));
    }

    #[test]
    fn permission_review_timeout_defaults_to_two_minutes() {
        assert_eq!(ACP_PERMISSION_REVIEW_TIMEOUT, Duration::from_secs(120));
        assert_eq!(
            acp_permission_review_timeout(),
            ACP_PERMISSION_REVIEW_TIMEOUT
        );
    }

    #[test]
    fn session_start_timeout_defaults_to_five_minutes() {
        assert_eq!(acp_session_start_timeout(), Duration::from_secs(300));
    }
}
