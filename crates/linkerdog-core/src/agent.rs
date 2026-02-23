use crate::{LinkerdogRuntimeConfig, runtime::ACP_CLIENT};
use agent_client_protocol::{
    Agent, AgentCapabilities, AuthenticateRequest, AuthenticateResponse, CancelNotification,
    Client, ContentBlock, ContentChunk, CurrentModeUpdate, Error, Implementation,
    InitializeRequest, InitializeResponse, ListSessionsRequest, ListSessionsResponse,
    LoadSessionRequest, LoadSessionResponse, McpCapabilities, ModelId, ModelInfo,
    NewSessionRequest, NewSessionResponse, PermissionOption, PermissionOptionId,
    PermissionOptionKind, PromptCapabilities, PromptRequest, PromptResponse, ProtocolVersion,
    RequestPermissionOutcome, RequestPermissionRequest, SelectedPermissionOutcome,
    SessionCapabilities, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOption, SessionId, SessionInfo, SessionListCapabilities, SessionMode,
    SessionModeId, SessionModeState, SessionModelState, SessionNotification, SessionUpdate,
    SetSessionConfigOptionRequest, SetSessionConfigOptionResponse, SetSessionModeRequest,
    SetSessionModeResponse, SetSessionModelRequest, SetSessionModelResponse, StopReason,
    TextContent, ToolCall, ToolCallId, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields,
    ToolKind,
};
use anyhow::Context;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::{debug, warn};
use uuid::Uuid;

const MODE_ASK: &str = "ask";
const MODE_CODE: &str = "code";
const MODE_REVIEW: &str = "review";

const CONFIG_PROVIDER: &str = "provider";
const CONFIG_MODEL: &str = "model";
const CONFIG_MODE: &str = "mode";

const PERMISSION_ALLOW_ONCE: &str = "allow_once";
const PERMISSION_REJECT_ONCE: &str = "reject_once";

#[derive(Clone, Copy)]
struct ProviderSpec {
    id: &'static str,
    name: &'static str,
    models: &'static [(&'static str, &'static str)],
}

const PROVIDERS: &[ProviderSpec] = &[
    ProviderSpec {
        id: "openai",
        name: "OpenAI",
        models: &[("gpt-5", "GPT-5"), ("o3", "o3")],
    },
    ProviderSpec {
        id: "anthropic",
        name: "Anthropic",
        models: &[
            ("claude-sonnet-4", "Claude Sonnet 4"),
            ("claude-opus-4.1", "Claude Opus 4.1"),
        ],
    },
    ProviderSpec {
        id: "google",
        name: "Google",
        models: &[
            ("gemini-2.5-pro", "Gemini 2.5 Pro"),
            ("gemini-2.5-flash", "Gemini 2.5 Flash"),
        ],
    },
    ProviderSpec {
        id: "deepseek",
        name: "DeepSeek",
        models: &[
            ("deepseek-chat", "DeepSeek Chat"),
            ("deepseek-reasoner", "DeepSeek Reasoner"),
        ],
    },
];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistoryEntry {
    role: String,
    text: String,
    at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedSessionState {
    provider: String,
    model: String,
    mode: String,
}

#[derive(Debug, Clone)]
struct SessionState {
    session_id: SessionId,
    cwd: PathBuf,
    provider: String,
    model: String,
    mode: String,
    history: Vec<HistoryEntry>,
    updated_at: String,
}

pub struct LinkerdogAgent {
    runtime_config: LinkerdogRuntimeConfig,
    sessions: RefCell<HashMap<String, SessionState>>,
    cancelled_sessions: RefCell<HashSet<String>>,
}

impl LinkerdogAgent {
    pub fn new(runtime_config: LinkerdogRuntimeConfig) -> Self {
        Self {
            runtime_config,
            sessions: RefCell::new(HashMap::new()),
            cancelled_sessions: RefCell::new(HashSet::new()),
        }
    }

    fn mode_options() -> Vec<SessionMode> {
        vec![
            SessionMode::new(MODE_ASK, "Ask")
                .description("Question answering and lightweight reasoning"),
            SessionMode::new(MODE_CODE, "Code")
                .description("Implementation mode with code-first execution"),
            SessionMode::new(MODE_REVIEW, "Review").description("Code review and risk-check mode"),
        ]
    }

    fn provider_spec(provider: &str) -> Option<ProviderSpec> {
        let normalized = provider.trim().to_ascii_lowercase();
        PROVIDERS.iter().copied().find(|item| item.id == normalized)
    }

    fn first_model_for_provider(provider: &str) -> String {
        Self::provider_spec(provider)
            .and_then(|spec| spec.models.first().map(|(id, _)| (*id).to_string()))
            .unwrap_or_else(|| "gpt-5".to_string())
    }

    fn model_exists(provider: &str, model: &str) -> bool {
        let normalized = model.trim();
        Self::provider_spec(provider)
            .map(|spec| spec.models.iter().any(|(id, _)| *id == normalized))
            .unwrap_or(false)
    }

    fn session_key(session_id: &SessionId) -> String {
        session_id.to_string()
    }

    fn mode_state(current_mode: &str) -> SessionModeState {
        SessionModeState::new(
            SessionModeId::new(current_mode.to_string()),
            Self::mode_options(),
        )
    }

    fn model_state(provider: &str, current_model: &str) -> SessionModelState {
        let available_models = Self::provider_spec(provider)
            .map(|spec| {
                spec.models
                    .iter()
                    .map(|(id, name)| ModelInfo::new(ModelId::new((*id).to_string()), *name))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        SessionModelState::new(ModelId::new(current_model.to_string()), available_models)
    }

    fn config_options(provider: &str, model: &str, mode: &str) -> Vec<SessionConfigOption> {
        let provider_options = PROVIDERS
            .iter()
            .map(|item| SessionConfigSelectOption::new(item.id, item.name))
            .collect::<Vec<_>>();

        let model_options = Self::provider_spec(provider)
            .map(|spec| {
                spec.models
                    .iter()
                    .map(|(id, name)| SessionConfigSelectOption::new(*id, *name))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mode_options = Self::mode_options()
            .into_iter()
            .map(|item| SessionConfigSelectOption::new(item.id.to_string(), item.name))
            .collect::<Vec<_>>();

        vec![
            SessionConfigOption::select(
                CONFIG_PROVIDER,
                "Provider",
                provider.to_string(),
                provider_options,
            )
            .description("Model provider for this session")
            .category(SessionConfigOptionCategory::Other("_provider".to_string())),
            SessionConfigOption::select(CONFIG_MODEL, "Model", model.to_string(), model_options)
                .description("Model within selected provider")
                .category(SessionConfigOptionCategory::Model),
            SessionConfigOption::select(CONFIG_MODE, "Mode", mode.to_string(), mode_options)
                .description("Conversation mode")
                .category(SessionConfigOptionCategory::Mode),
        ]
    }

    fn sanitize_defaults(config: &LinkerdogRuntimeConfig) -> (String, String, String) {
        let provider = Self::provider_spec(&config.default_provider)
            .map(|item| item.id.to_string())
            .unwrap_or_else(|| "openai".to_string());

        let mode = match config.default_mode.as_str() {
            MODE_ASK | MODE_CODE | MODE_REVIEW => config.default_mode.clone(),
            _ => MODE_CODE.to_string(),
        };

        let model = if Self::model_exists(&provider, &config.default_model) {
            config.default_model.clone()
        } else {
            Self::first_model_for_provider(&provider)
        };

        (provider, model, mode)
    }

    fn session_dir(cwd: &Path, session_id: &SessionId) -> PathBuf {
        cwd.join(".cache")
            .join("context")
            .join("run")
            .join(session_id.to_string())
    }

    fn state_file(cwd: &Path, session_id: &SessionId) -> PathBuf {
        Self::session_dir(cwd, session_id).join("state.json")
    }

    fn history_file(cwd: &Path, session_id: &SessionId) -> PathBuf {
        Self::session_dir(cwd, session_id).join("history.jsonl")
    }

    fn persist_state(session: &SessionState) {
        let file = Self::state_file(&session.cwd, &session.session_id);
        if let Some(parent) = file.parent()
            && let Err(err) = std::fs::create_dir_all(parent)
        {
            warn!("create session state dir failed: {err}");
            return;
        }

        let payload = PersistedSessionState {
            provider: session.provider.clone(),
            model: session.model.clone(),
            mode: session.mode.clone(),
        };

        let Ok(body) = serde_json::to_vec_pretty(&payload) else {
            return;
        };

        if let Err(err) = std::fs::write(&file, body) {
            warn!("write session state failed: {} err={err}", file.display());
        }
    }

    fn append_history_entry(session: &SessionState, entry: &HistoryEntry) {
        let file = Self::history_file(&session.cwd, &session.session_id);
        if let Some(parent) = file.parent()
            && let Err(err) = std::fs::create_dir_all(parent)
        {
            warn!("create history dir failed: {err}");
            return;
        }

        let Ok(line) = serde_json::to_string(entry) else {
            return;
        };

        let result = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)
            .and_then(|mut fp| writeln!(fp, "{line}"));
        if let Err(err) = result {
            warn!("append history failed: {} err={err}", file.display());
        }
    }

    fn load_session_from_disk(
        session_id: &SessionId,
        cwd: &Path,
    ) -> anyhow::Result<Option<SessionState>> {
        let state_file = Self::state_file(cwd, session_id);
        if !state_file.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&state_file)
            .with_context(|| format!("read {}", state_file.display()))?;
        let persisted: PersistedSessionState =
            serde_json::from_str(&content).context("parse persisted session state")?;

        let history_file = Self::history_file(cwd, session_id);
        let history = if history_file.exists() {
            let text = std::fs::read_to_string(&history_file)
                .with_context(|| format!("read {}", history_file.display()))?;
            text.lines()
                .filter(|line| !line.trim().is_empty())
                .filter_map(|line| serde_json::from_str::<HistoryEntry>(line).ok())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let updated_at = history
            .last()
            .map(|item| item.at.clone())
            .unwrap_or_else(now_rfc3339);

        Ok(Some(SessionState {
            session_id: session_id.clone(),
            cwd: cwd.to_path_buf(),
            provider: persisted.provider,
            model: persisted.model,
            mode: persisted.mode,
            history,
            updated_at,
        }))
    }

    fn title_from_history(history: &[HistoryEntry]) -> Option<String> {
        history
            .iter()
            .find(|entry| entry.role == "user")
            .map(|entry| {
                let mut title = entry.text.trim().replace('\n', " ");
                if title.chars().count() > 80 {
                    title = title.chars().take(80).collect::<String>() + "...";
                }
                title
            })
            .filter(|title| !title.is_empty())
    }

    fn request_cancelled(&self, session_id: &SessionId) -> bool {
        self.cancelled_sessions
            .borrow_mut()
            .remove(&Self::session_key(session_id))
    }

    fn ensure_session(&self, session_id: &SessionId) -> Result<SessionState, Error> {
        self.sessions
            .borrow()
            .get(&Self::session_key(session_id))
            .cloned()
            .ok_or_else(|| Error::resource_not_found(None))
    }

    fn update_session<F>(&self, session_id: &SessionId, mutator: F) -> Result<SessionState, Error>
    where
        F: FnOnce(&mut SessionState) -> Result<(), Error>,
    {
        let key = Self::session_key(session_id);
        let mut sessions = self.sessions.borrow_mut();
        let session = sessions
            .get_mut(&key)
            .ok_or_else(|| Error::resource_not_found(None))?;
        mutator(session)?;
        Ok(session.clone())
    }

    fn upsert_session(&self, session: SessionState) {
        self.sessions
            .borrow_mut()
            .insert(Self::session_key(&session.session_id), session);
    }

    fn record_entry(
        &self,
        session_id: &SessionId,
        role: &str,
        text: String,
    ) -> Result<SessionState, Error> {
        self.update_session(session_id, |session| {
            let entry = HistoryEntry {
                role: role.to_string(),
                text,
                at: now_rfc3339(),
            };
            session.updated_at = entry.at.clone();
            session.history.push(entry.clone());
            Self::append_history_entry(session, &entry);
            Self::persist_state(session);
            Ok(())
        })
    }

    async fn notify(session_id: &SessionId, update: SessionUpdate) -> Result<(), Error> {
        let client = ACP_CLIENT
            .get()
            .ok_or_else(|| Error::internal_error().data("ACP client not initialized"))?;
        client
            .session_notification(SessionNotification::new(session_id.clone(), update))
            .await
    }

    async fn send_user_chunk(session_id: &SessionId, text: &str) -> Result<(), Error> {
        Self::notify(
            session_id,
            SessionUpdate::UserMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new(text),
            ))),
        )
        .await
    }

    async fn send_agent_chunk(session_id: &SessionId, text: &str) -> Result<(), Error> {
        Self::notify(
            session_id,
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new(text),
            ))),
        )
        .await
    }

    async fn send_thought_chunk(session_id: &SessionId, text: &str) -> Result<(), Error> {
        Self::notify(
            session_id,
            SessionUpdate::AgentThoughtChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new(text),
            ))),
        )
        .await
    }

    async fn send_tool_call_start(
        session_id: &SessionId,
        tool_call_id: &ToolCallId,
        title: &str,
    ) -> Result<(), Error> {
        Self::notify(
            session_id,
            SessionUpdate::ToolCall(
                ToolCall::new(tool_call_id.clone(), title)
                    .kind(ToolKind::Execute)
                    .status(ToolCallStatus::InProgress),
            ),
        )
        .await
    }

    async fn send_tool_call_finish(
        session_id: &SessionId,
        tool_call_id: &ToolCallId,
        success: bool,
        body: String,
    ) -> Result<(), Error> {
        Self::notify(
            session_id,
            SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                tool_call_id.clone(),
                ToolCallUpdateFields::new()
                    .status(if success {
                        ToolCallStatus::Completed
                    } else {
                        ToolCallStatus::Failed
                    })
                    .content(Some(vec![body.into()])),
            )),
        )
        .await
    }

    async fn request_exec_permission(
        session_id: &SessionId,
        tool_call_id: &ToolCallId,
        command: &str,
    ) -> Result<RequestPermissionOutcome, Error> {
        let client = ACP_CLIENT
            .get()
            .ok_or_else(|| Error::internal_error().data("ACP client not initialized"))?;

        let request = RequestPermissionRequest::new(
            session_id.clone(),
            ToolCallUpdate::new(
                tool_call_id.clone(),
                ToolCallUpdateFields::new()
                    .title(format!("Execute: {command}"))
                    .kind(ToolKind::Execute)
                    .status(ToolCallStatus::Pending),
            ),
            vec![
                PermissionOption::new(
                    PermissionOptionId::new(PERMISSION_ALLOW_ONCE),
                    "Allow once",
                    PermissionOptionKind::AllowOnce,
                ),
                PermissionOption::new(
                    PermissionOptionId::new(PERMISSION_REJECT_ONCE),
                    "Reject once",
                    PermissionOptionKind::RejectOnce,
                ),
            ],
        );

        let response = client.request_permission(request).await?;
        Ok(response.outcome)
    }

    async fn run_local_command(cwd: &Path, command: &str) -> anyhow::Result<(i32, String)> {
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .arg("/C")
                .arg(command)
                .current_dir(cwd)
                .output()
                .await?
        } else {
            Command::new("sh")
                .arg("-lc")
                .arg(command)
                .current_dir(cwd)
                .output()
                .await?
        };

        let code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let merged = format!(
            "exit_code={code}\nstdout:\n{}\nstderr:\n{}",
            stdout.trim_end(),
            stderr.trim_end()
        );
        Ok((code, merged))
    }

    fn prompt_text(prompt: &[ContentBlock]) -> String {
        let mut items = Vec::new();
        for block in prompt {
            match block {
                ContentBlock::Text(text) => {
                    if !text.text.trim().is_empty() {
                        items.push(text.text.trim().to_string());
                    }
                }
                ContentBlock::Resource(resource) => match &resource.resource {
                    agent_client_protocol::EmbeddedResourceResource::TextResourceContents(text) => {
                        if !text.text.trim().is_empty() {
                            items.push(text.text.trim().to_string());
                        }
                    }
                    agent_client_protocol::EmbeddedResourceResource::BlobResourceContents(_) => {}
                    _ => {}
                },
                ContentBlock::ResourceLink(link) => {
                    items.push(format!("[resource:{}]", link.uri));
                }
                ContentBlock::Image(_) | ContentBlock::Audio(_) => {}
                _ => {}
            }
        }
        items.join("\n")
    }

    fn build_context_window(session: &SessionState) -> String {
        let mut lines = session
            .history
            .iter()
            .rev()
            .take(4)
            .map(|item| format!("{}: {}", item.role, item.text.replace('\n', " ")))
            .collect::<Vec<_>>();
        lines.reverse();
        lines.join("\n")
    }

    fn build_reply(session: &SessionState, user_text: &str) -> String {
        let context = Self::build_context_window(session);
        format!(
            "[Linkerdog]\nmode={} provider={} model={}\n\nUser: {}\n\nContext Window:\n{}",
            session.mode,
            session.provider,
            session.model,
            user_text.trim(),
            if context.is_empty() {
                "(empty)".to_string()
            } else {
                context
            }
        )
    }

    fn set_provider_model_mode(
        session: &mut SessionState,
        provider: Option<String>,
        model: Option<String>,
        mode: Option<String>,
    ) -> Result<(), Error> {
        if let Some(provider) = provider {
            let provider_normalized = provider.trim().to_ascii_lowercase();
            if Self::provider_spec(&provider_normalized).is_none() {
                return Err(Error::invalid_params()
                    .data(format!("unknown provider: {provider_normalized}")));
            }
            session.provider = provider_normalized;
            if !Self::model_exists(&session.provider, &session.model) {
                session.model = Self::first_model_for_provider(&session.provider);
            }
        }

        if let Some(model) = model {
            if !Self::model_exists(&session.provider, &model) {
                return Err(Error::invalid_params().data(format!(
                    "model {model} is not available for provider {}",
                    session.provider
                )));
            }
            session.model = model;
        }

        if let Some(mode) = mode {
            let mode_normalized = mode.trim().to_ascii_lowercase();
            if !matches!(mode_normalized.as_str(), MODE_ASK | MODE_CODE | MODE_REVIEW) {
                return Err(
                    Error::invalid_params().data(format!("unknown mode: {mode_normalized}"))
                );
            }
            session.mode = mode_normalized;
        }

        session.updated_at = now_rfc3339();
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Agent for LinkerdogAgent {
    async fn initialize(&self, request: InitializeRequest) -> Result<InitializeResponse, Error> {
        debug!("initialize protocol={:?}", request.protocol_version);

        let mut caps = AgentCapabilities::new()
            .prompt_capabilities(PromptCapabilities::new().embedded_context(true).image(true))
            .mcp_capabilities(McpCapabilities::new().http(true))
            .load_session(true);
        caps.session_capabilities = SessionCapabilities::new().list(SessionListCapabilities::new());

        Ok(InitializeResponse::new(ProtocolVersion::V1)
            .agent_capabilities(caps)
            .agent_info(
                Implementation::new("linkerdog", env!("CARGO_PKG_VERSION")).title("Linkerdog ACP"),
            ))
    }

    async fn authenticate(
        &self,
        _request: AuthenticateRequest,
    ) -> Result<AuthenticateResponse, Error> {
        Ok(AuthenticateResponse::new())
    }

    async fn new_session(&self, request: NewSessionRequest) -> Result<NewSessionResponse, Error> {
        let session_id = SessionId::new(format!("linkerdog-{}", Uuid::new_v4()));
        let (provider, model, mode) = Self::sanitize_defaults(&self.runtime_config);

        let session = SessionState {
            session_id: session_id.clone(),
            cwd: request.cwd,
            provider: provider.clone(),
            model: model.clone(),
            mode: mode.clone(),
            history: Vec::new(),
            updated_at: now_rfc3339(),
        };
        Self::persist_state(&session);
        self.upsert_session(session);

        Ok(NewSessionResponse::new(session_id)
            .modes(Self::mode_state(&mode))
            .models(Self::model_state(&provider, &model))
            .config_options(Self::config_options(&provider, &model, &mode)))
    }

    async fn prompt(&self, request: PromptRequest) -> Result<PromptResponse, Error> {
        let session_id = request.session_id.clone();
        if self.request_cancelled(&session_id) {
            return Ok(PromptResponse::new(StopReason::Cancelled));
        }

        let user_text = Self::prompt_text(&request.prompt);
        let user_text = if user_text.trim().is_empty() {
            "(empty prompt)".to_string()
        } else {
            user_text
        };

        Self::send_user_chunk(&session_id, &user_text).await?;
        self.record_entry(&session_id, "user", user_text.clone())?;

        let session_before = self.ensure_session(&session_id)?;
        let response_text = if let Some(command) = user_text.strip_prefix("/tool exec ") {
            let command = command.trim();
            if command.is_empty() {
                "Tool command is empty. Use: /tool exec <command>".to_string()
            } else {
                let tool_call_id = ToolCallId::new(format!("exec-{}", Uuid::new_v4()));
                Self::send_tool_call_start(
                    &session_id,
                    &tool_call_id,
                    &format!("Execute: {command}"),
                )
                .await?;

                let permission = Self::request_exec_permission(&session_id, &tool_call_id, command)
                    .await
                    .unwrap_or(RequestPermissionOutcome::Cancelled);

                let allowed = matches!(
                    permission,
                    RequestPermissionOutcome::Selected(SelectedPermissionOutcome { option_id, .. })
                        if option_id == PermissionOptionId::new(PERMISSION_ALLOW_ONCE)
                );

                if !allowed {
                    Self::send_tool_call_finish(
                        &session_id,
                        &tool_call_id,
                        false,
                        "permission denied".to_string(),
                    )
                    .await?;
                    "Tool execution rejected by permission policy.".to_string()
                } else {
                    Self::send_thought_chunk(&session_id, "Running command...").await?;
                    match Self::run_local_command(&session_before.cwd, command).await {
                        Ok((exit_code, output)) => {
                            let success = exit_code == 0;
                            Self::send_tool_call_finish(
                                &session_id,
                                &tool_call_id,
                                success,
                                output.clone(),
                            )
                            .await?;
                            format!("Command finished with exit_code={exit_code}.\n\n{}", output)
                        }
                        Err(err) => {
                            let message = format!("command execution failed: {err}");
                            Self::send_tool_call_finish(
                                &session_id,
                                &tool_call_id,
                                false,
                                message.clone(),
                            )
                            .await?;
                            message
                        }
                    }
                }
            }
        } else {
            Self::send_thought_chunk(&session_id, "Planning response with local context...")
                .await?;
            let latest = self.ensure_session(&session_id)?;
            Self::build_reply(&latest, &user_text)
        };

        Self::send_agent_chunk(&session_id, &response_text).await?;
        self.record_entry(&session_id, "assistant", response_text)?;

        Ok(PromptResponse::new(StopReason::EndTurn))
    }

    async fn cancel(&self, request: CancelNotification) -> Result<(), Error> {
        self.cancelled_sessions
            .borrow_mut()
            .insert(Self::session_key(&request.session_id));
        Ok(())
    }

    async fn load_session(
        &self,
        request: LoadSessionRequest,
    ) -> Result<LoadSessionResponse, Error> {
        let session_key = Self::session_key(&request.session_id);

        let session = if let Some(existing) = self.sessions.borrow().get(&session_key).cloned() {
            existing
        } else {
            let loaded = Self::load_session_from_disk(&request.session_id, &request.cwd)
                .map_err(|err| Error::resource_not_found(None).data(err.to_string()))?;
            let Some(loaded) = loaded else {
                return Err(Error::resource_not_found(None));
            };
            self.upsert_session(loaded.clone());
            loaded
        };

        for entry in &session.history {
            match entry.role.as_str() {
                "user" => Self::send_user_chunk(&session.session_id, &entry.text).await?,
                "assistant" => Self::send_agent_chunk(&session.session_id, &entry.text).await?,
                _ => {}
            }
        }

        Ok(LoadSessionResponse::new()
            .modes(Self::mode_state(&session.mode))
            .models(Self::model_state(&session.provider, &session.model))
            .config_options(Self::config_options(
                &session.provider,
                &session.model,
                &session.mode,
            )))
    }

    async fn set_session_mode(
        &self,
        request: SetSessionModeRequest,
    ) -> Result<SetSessionModeResponse, Error> {
        let mode = request.mode_id.to_string();
        let session = self.update_session(&request.session_id, |session| {
            Self::set_provider_model_mode(session, None, None, Some(mode.clone()))?;
            Self::persist_state(session);
            Ok(())
        })?;

        Self::notify(
            &request.session_id,
            SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(session.mode.clone())),
        )
        .await?;

        Self::notify(
            &request.session_id,
            SessionUpdate::ConfigOptionUpdate(agent_client_protocol::ConfigOptionUpdate::new(
                Self::config_options(&session.provider, &session.model, &session.mode),
            )),
        )
        .await?;

        Ok(SetSessionModeResponse::new())
    }

    async fn set_session_model(
        &self,
        request: SetSessionModelRequest,
    ) -> Result<SetSessionModelResponse, Error> {
        let model = request.model_id.to_string();
        let session = self.update_session(&request.session_id, |session| {
            Self::set_provider_model_mode(session, None, Some(model.clone()), None)?;
            Self::persist_state(session);
            Ok(())
        })?;

        Self::notify(
            &request.session_id,
            SessionUpdate::ConfigOptionUpdate(agent_client_protocol::ConfigOptionUpdate::new(
                Self::config_options(&session.provider, &session.model, &session.mode),
            )),
        )
        .await?;

        Ok(SetSessionModelResponse::new())
    }

    async fn set_session_config_option(
        &self,
        request: SetSessionConfigOptionRequest,
    ) -> Result<SetSessionConfigOptionResponse, Error> {
        let config_id = request.config_id.to_string();
        let value = request.value.to_string();

        let session = self.update_session(&request.session_id, |session| {
            let result = match config_id.as_str() {
                CONFIG_PROVIDER => {
                    Self::set_provider_model_mode(session, Some(value.clone()), None, None)
                }
                CONFIG_MODEL => {
                    Self::set_provider_model_mode(session, None, Some(value.clone()), None)
                }
                CONFIG_MODE => {
                    Self::set_provider_model_mode(session, None, None, Some(value.clone()))
                }
                _ => Err(Error::invalid_params()
                    .data(format!("unknown config option id: {}", config_id))),
            };

            result?;
            Self::persist_state(session);
            Ok(())
        })?;

        if config_id == CONFIG_MODE {
            Self::notify(
                &request.session_id,
                SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(session.mode.clone())),
            )
            .await?;
        }

        let config_options = Self::config_options(&session.provider, &session.model, &session.mode);
        Self::notify(
            &request.session_id,
            SessionUpdate::ConfigOptionUpdate(agent_client_protocol::ConfigOptionUpdate::new(
                config_options.clone(),
            )),
        )
        .await?;

        Ok(SetSessionConfigOptionResponse::new(config_options))
    }

    async fn list_sessions(
        &self,
        request: ListSessionsRequest,
    ) -> Result<ListSessionsResponse, Error> {
        let cwd_filter = request.cwd;

        let mut sessions = self
            .sessions
            .borrow()
            .values()
            .filter(|session| {
                if let Some(cwd) = &cwd_filter {
                    session.cwd == *cwd
                } else {
                    true
                }
            })
            .map(|session| {
                SessionInfo::new(session.session_id.clone(), session.cwd.clone())
                    .title(Self::title_from_history(&session.history))
                    .updated_at(session.updated_at.clone())
            })
            .collect::<Vec<_>>();

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(ListSessionsResponse::new(sessions))
    }
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::{
        EmbeddedResource, EmbeddedResourceResource, ResourceLink, TextResourceContents,
    };
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("linkerdog-core-{name}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn sample_session(cwd: PathBuf, session_id: &str) -> SessionState {
        SessionState {
            session_id: SessionId::new(session_id.to_string()),
            cwd,
            provider: "openai".to_string(),
            model: "gpt-5".to_string(),
            mode: MODE_CODE.to_string(),
            history: Vec::new(),
            updated_at: now_rfc3339(),
        }
    }

    #[test]
    fn provider_and_default_helpers_work() {
        assert!(LinkerdogAgent::provider_spec("openai").is_some());
        assert!(LinkerdogAgent::provider_spec(" OpenAI ").is_some());
        assert!(LinkerdogAgent::provider_spec("unknown").is_none());

        assert_eq!(LinkerdogAgent::first_model_for_provider("openai"), "gpt-5");
        assert_eq!(LinkerdogAgent::first_model_for_provider("missing"), "gpt-5");
        assert!(LinkerdogAgent::model_exists("google", "gemini-2.5-pro"));
        assert!(!LinkerdogAgent::model_exists("google", "not-exist"));

        let defaults = LinkerdogRuntimeConfig {
            default_provider: "not-provider".to_string(),
            default_model: "not-model".to_string(),
            default_mode: "not-mode".to_string(),
        };
        let (provider, model, mode) = LinkerdogAgent::sanitize_defaults(&defaults);
        assert_eq!(provider, "openai");
        assert_eq!(model, "gpt-5");
        assert_eq!(mode, MODE_CODE);
    }

    #[test]
    fn session_path_and_persistence_helpers_roundtrip() {
        let cwd = temp_dir("persist");
        let mut session = sample_session(cwd.clone(), "session-persist");
        let session_id = session.session_id.clone();
        let agent = LinkerdogAgent::new(LinkerdogRuntimeConfig::default());
        agent.upsert_session(session.clone());

        let updated = agent
            .record_entry(&session_id, "user", "hello world".to_string())
            .expect("record entry");
        session = updated;

        let state_file = LinkerdogAgent::state_file(&cwd, &session_id);
        let history_file = LinkerdogAgent::history_file(&cwd, &session_id);
        assert!(state_file.exists());
        assert!(history_file.exists());

        let loaded = LinkerdogAgent::load_session_from_disk(&session_id, &cwd)
            .expect("load from disk")
            .expect("session exists");
        assert_eq!(loaded.provider, session.provider);
        assert_eq!(loaded.model, session.model);
        assert_eq!(loaded.mode, session.mode);
        assert_eq!(loaded.history.len(), 1);
        assert_eq!(loaded.history[0].role, "user");
        assert_eq!(loaded.history[0].text, "hello world");
        assert_eq!(
            LinkerdogAgent::title_from_history(&loaded.history),
            Some("hello world".to_string())
        );
    }

    #[test]
    fn title_context_and_reply_helpers_work() {
        let cwd = temp_dir("reply");
        let mut session = sample_session(cwd, "session-reply");
        session.history = vec![
            HistoryEntry {
                role: "system".to_string(),
                text: "boot".to_string(),
                at: now_rfc3339(),
            },
            HistoryEntry {
                role: "user".to_string(),
                text: "line-1\nline-2".to_string(),
                at: now_rfc3339(),
            },
            HistoryEntry {
                role: "assistant".to_string(),
                text: "ok".to_string(),
                at: now_rfc3339(),
            },
        ];

        let context = LinkerdogAgent::build_context_window(&session);
        assert!(context.contains("user: line-1 line-2"));

        let reply = LinkerdogAgent::build_reply(&session, "  ping  ");
        assert!(reply.contains("mode=code provider=openai model=gpt-5"));
        assert!(reply.contains("User: ping"));
        assert!(reply.contains("Context Window:"));

        let long_text = "x".repeat(100);
        let long_history = vec![HistoryEntry {
            role: "user".to_string(),
            text: long_text,
            at: now_rfc3339(),
        }];
        let title = LinkerdogAgent::title_from_history(&long_history).expect("title");
        assert!(title.ends_with("..."));
        assert_eq!(title.chars().count(), 83);
    }

    #[test]
    fn prompt_text_collects_supported_blocks() {
        let blocks = vec![
            ContentBlock::Text(TextContent::new("  hello  ")),
            ContentBlock::Resource(EmbeddedResource::new(
                EmbeddedResourceResource::TextResourceContents(TextResourceContents::new(
                    "  from resource  ",
                    "file://resource.txt",
                )),
            )),
            ContentBlock::ResourceLink(ResourceLink::new("spec", "file://spec.md")),
        ];
        let text = LinkerdogAgent::prompt_text(&blocks);
        assert_eq!(text, "hello\nfrom resource\n[resource:file://spec.md]");
    }

    #[test]
    fn set_provider_model_mode_validates_inputs() {
        let cwd = temp_dir("provider-mode");
        let mut session = sample_session(cwd, "session-provider-mode");

        LinkerdogAgent::set_provider_model_mode(
            &mut session,
            Some("anthropic".to_string()),
            None,
            None,
        )
        .expect("set provider");
        assert_eq!(session.provider, "anthropic");
        assert_eq!(session.model, "claude-sonnet-4");

        let invalid_provider = LinkerdogAgent::set_provider_model_mode(
            &mut session,
            Some("bad-provider".to_string()),
            None,
            None,
        )
        .expect_err("invalid provider");
        assert!(invalid_provider.to_string().contains("unknown provider"));

        let invalid_model = LinkerdogAgent::set_provider_model_mode(
            &mut session,
            None,
            Some("bad-model".to_string()),
            None,
        )
        .expect_err("invalid model");
        assert!(
            invalid_model
                .to_string()
                .contains("is not available for provider")
        );

        let invalid_mode =
            LinkerdogAgent::set_provider_model_mode(&mut session, None, None, Some("bad".into()))
                .expect_err("invalid mode");
        assert!(invalid_mode.to_string().contains("unknown mode"));

        LinkerdogAgent::set_provider_model_mode(
            &mut session,
            None,
            Some("claude-opus-4.1".into()),
            Some("review".into()),
        )
        .expect("set model and mode");
        assert_eq!(session.model, "claude-opus-4.1");
        assert_eq!(session.mode, MODE_REVIEW);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_local_command_reports_stdout_and_exit_code() {
        let cwd = temp_dir("run-local-command");
        let (exit_code, output) =
            LinkerdogAgent::run_local_command(&cwd, "printf 'hello-linkerdog'")
                .await
                .expect("run command");
        assert_eq!(exit_code, 0);
        assert!(output.contains("exit_code=0"));
        assert!(output.contains("hello-linkerdog"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lifecycle_paths_work_without_initialized_acp_client() {
        let cwd = temp_dir("lifecycle");
        let agent = LinkerdogAgent::new(LinkerdogRuntimeConfig {
            default_provider: "invalid-provider".to_string(),
            default_model: "invalid-model".to_string(),
            default_mode: "invalid-mode".to_string(),
        });

        let new_session = Agent::new_session(&agent, NewSessionRequest::new(cwd.clone()))
            .await
            .expect("new session");
        assert!(new_session.session_id.to_string().starts_with("linkerdog-"));
        assert!(new_session.modes.is_some());
        assert!(new_session.models.is_some());
        assert!(new_session.config_options.is_some());

        let list_all = Agent::list_sessions(&agent, ListSessionsRequest::new())
            .await
            .expect("list sessions");
        assert_eq!(list_all.sessions.len(), 1);

        let list_other_cwd = Agent::list_sessions(
            &agent,
            ListSessionsRequest::new().cwd(temp_dir("other-cwd")),
        )
        .await
        .expect("list with cwd filter");
        assert!(list_other_cwd.sessions.is_empty());

        let loaded = Agent::load_session(
            &agent,
            LoadSessionRequest::new(new_session.session_id.clone(), cwd.clone()),
        )
        .await
        .expect("load session");
        assert!(loaded.config_options.is_some());

        Agent::cancel(
            &agent,
            CancelNotification::new(new_session.session_id.clone()),
        )
        .await
        .expect("cancel");
        let cancelled_prompt = Agent::prompt(
            &agent,
            PromptRequest::new(
                new_session.session_id.clone(),
                vec![ContentBlock::Text(TextContent::new("hello"))],
            ),
        )
        .await
        .expect("cancelled prompt");
        assert_eq!(cancelled_prompt.stop_reason, StopReason::Cancelled);

        let missing = Agent::load_session(&agent, LoadSessionRequest::new("missing-session", cwd))
            .await
            .expect_err("missing session");
        assert!(missing.to_string().contains("not found"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn session_control_methods_validate_and_require_client_for_notifications() {
        let cwd = temp_dir("session-control");
        let agent = LinkerdogAgent::new(LinkerdogRuntimeConfig::default());
        let new_session = Agent::new_session(&agent, NewSessionRequest::new(cwd))
            .await
            .expect("new session");
        let session_id = new_session.session_id.clone();

        let mode_err = Agent::set_session_mode(
            &agent,
            SetSessionModeRequest::new(session_id.clone(), MODE_ASK),
        )
        .await
        .expect_err("mode update requires ACP client");
        assert!(mode_err.to_string().contains("ACP client not initialized"));

        let model_err = Agent::set_session_model(
            &agent,
            SetSessionModelRequest::new(session_id.clone(), "o3"),
        )
        .await
        .expect_err("model update requires ACP client");
        assert!(model_err.to_string().contains("ACP client not initialized"));

        let mode_config_err = Agent::set_session_config_option(
            &agent,
            SetSessionConfigOptionRequest::new(session_id.clone(), CONFIG_MODE, MODE_REVIEW),
        )
        .await
        .expect_err("mode config requires ACP client");
        assert!(
            mode_config_err
                .to_string()
                .contains("ACP client not initialized")
        );

        let unknown_config = Agent::set_session_config_option(
            &agent,
            SetSessionConfigOptionRequest::new(session_id, "unknown", "value"),
        )
        .await
        .expect_err("unknown config");
        assert!(
            unknown_config
                .to_string()
                .contains("unknown config option id")
        );
    }
}
