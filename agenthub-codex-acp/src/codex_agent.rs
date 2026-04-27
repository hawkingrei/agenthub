use agent_client_protocol_legacy::Error;
use agent_client_protocol_legacy::{
    Agent, AgentCapabilities, AuthEnvVar, AuthMethod, AuthMethodAgent, AuthMethodEnvVar,
    AuthMethodId, AuthenticateRequest, AuthenticateResponse, CancelNotification,
    ClientCapabilities, CloseSessionRequest, CloseSessionResponse, Implementation,
    InitializeRequest, InitializeResponse, ListSessionsRequest, ListSessionsResponse,
    LoadSessionRequest, LoadSessionResponse, McpCapabilities, McpServer, McpServerHttp,
    McpServerStdio, NewSessionRequest, NewSessionResponse, PromptCapabilities, PromptRequest,
    PromptResponse, ProtocolVersion, SessionCapabilities, SessionCloseCapabilities, SessionId,
    SessionInfo, SessionListCapabilities, SetSessionConfigOptionRequest,
    SetSessionConfigOptionResponse, SetSessionModeRequest, SetSessionModeResponse,
    SetSessionModelRequest, SetSessionModelResponse,
};
use codex_config::types::{McpServerConfig, McpServerTransportConfig};
use codex_core::{
    RolloutRecorder, SortDirection, ThreadManager, ThreadSortKey, config::Config,
    find_thread_path_by_id_str, parse_cursor,
};
use codex_login::auth::{read_codex_api_key_from_env, read_openai_api_key_from_env};
use codex_login::{
    AuthManager, CLIENT_ID, CODEX_API_KEY_ENV_VAR, CodexAuth, OPENAI_API_KEY_ENV_VAR,
};
use codex_models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_protocol::{
    ThreadId,
    models::{FunctionCallOutputPayload, ResponseItem},
    protocol::{InitialHistory, ResumedHistory, RolloutItem, SessionSource},
};
use std::{
    cell::RefCell,
    collections::HashMap,
    collections::HashSet,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex},
};
use tracing::{debug, info, warn};
use unicode_segmentation::UnicodeSegmentation;

use crate::app_server_thread;
use crate::build_environment_manager;
use crate::thread::{Thread, adapt_models_manager};

/// The Codex implementation of the ACP Agent trait.
///
/// This bridges the ACP protocol with the existing codex-rs infrastructure,
/// allowing codex to be used as an ACP agent.
pub struct CodexAgent {
    /// Handle to the current authentication
    auth_manager: Arc<AuthManager>,
    /// Capabilities of the connected client
    client_capabilities: Arc<Mutex<ClientCapabilities>>,
    /// The underlying codex configuration
    config: Config,
    /// Thread manager for handling sessions
    thread_manager: ThreadManager,
    /// Active sessions mapped by `SessionId`
    sessions: Rc<RefCell<HashMap<SessionId, Rc<Thread>>>>,
    /// Session working directories for filesystem sandboxing
    session_roots: Arc<Mutex<HashMap<SessionId, PathBuf>>>,
}

const SESSION_LIST_PAGE_SIZE: usize = 25;
const SESSION_TITLE_MAX_GRAPHEMES: usize = 120;

impl CodexAgent {
    /// Create a new `CodexAgent` with the given configuration
    pub fn new(config: Config) -> Result<Self, Error> {
        let auth_manager = AuthManager::shared(
            config.codex_home.to_path_buf(),
            false,
            config.cli_auth_credentials_store_mode,
            Some(config.chatgpt_base_url.clone()),
        );

        let client_capabilities: Arc<Mutex<ClientCapabilities>> = Arc::default();

        let session_roots: Arc<Mutex<HashMap<SessionId, PathBuf>>> = Arc::default();
        let thread_manager = ThreadManager::new(
            &config,
            auth_manager.clone(),
            SessionSource::Unknown,
            CollaborationModesConfig {
                default_mode_request_user_input: true,
            },
            build_environment_manager(&config)?,
            None,
        );
        Ok(Self {
            auth_manager,
            client_capabilities,
            config,
            thread_manager,
            sessions: Rc::default(),
            session_roots,
        })
    }

    fn get_thread(&self, session_id: &SessionId) -> Result<Rc<Thread>, Error> {
        Ok(self
            .sessions
            .borrow()
            .get(session_id)
            .ok_or_else(|| Error::resource_not_found(None))?
            .clone())
    }

    async fn check_auth(&self) -> Result<(), Error> {
        if self.config.model_provider_id == "openai" && self.auth_manager.auth().await.is_none() {
            return Err(Error::auth_required());
        }
        Ok(())
    }

    /// Build a session config from base config, working directory, and MCP servers.
    /// This is shared between `new_session` and `load_session`.
    fn build_session_config(
        &self,
        cwd: &Path,
        mcp_servers: Vec<McpServer>,
    ) -> Result<Config, Error> {
        let mut config = self.config.clone();
        config.include_apply_patch_tool = true;
        config.cwd = cwd
            .to_path_buf()
            .try_into()
            .map_err(|e: std::io::Error| Error::internal_error().data(e.to_string()))?;

        // Propagate any client-provided MCP servers that codex-rs supports.
        let mut new_mcp_servers = config.mcp_servers.get().clone();
        for mcp_server in mcp_servers {
            match mcp_server {
                // Not supported in codex
                McpServer::Sse(..) => {}
                McpServer::Http(McpServerHttp {
                    name, url, headers, ..
                }) => {
                    // Codex does not allow whitespace in MCP server names; replace with underscores.
                    let name = name.replace(|c: char| c.is_whitespace(), "_");
                    new_mcp_servers.insert(
                        name,
                        McpServerConfig {
                            transport: McpServerTransportConfig::StreamableHttp {
                                url,
                                bearer_token_env_var: None,
                                http_headers: if headers.is_empty() {
                                    None
                                } else {
                                    Some(headers.into_iter().map(|h| (h.name, h.value)).collect())
                                },
                                env_http_headers: None,
                            },
                            experimental_environment: None,
                            required: false,
                            enabled: true,
                            supports_parallel_tool_calls: false,
                            startup_timeout_sec: None,
                            tool_timeout_sec: None,
                            default_tools_approval_mode: None,
                            disabled_tools: None,
                            enabled_tools: None,
                            disabled_reason: None,
                            scopes: None,
                            oauth_resource: None,
                            tools: HashMap::new(),
                        },
                    );
                }
                McpServer::Stdio(McpServerStdio {
                    name,
                    command,
                    args,
                    env,
                    ..
                }) => {
                    // Codex does not allow whitespace in MCP server names; replace with underscores.
                    let name = name.replace(|c: char| c.is_whitespace(), "_");
                    new_mcp_servers.insert(
                        name,
                        McpServerConfig {
                            transport: McpServerTransportConfig::Stdio {
                                command: command.display().to_string(),
                                args,
                                env: if env.is_empty() {
                                    None
                                } else {
                                    Some(env.into_iter().map(|env| (env.name, env.value)).collect())
                                },
                                env_vars: vec![],
                                cwd: Some(cwd.to_path_buf()),
                            },
                            experimental_environment: None,
                            required: false,
                            enabled: true,
                            supports_parallel_tool_calls: false,
                            startup_timeout_sec: None,
                            tool_timeout_sec: None,
                            default_tools_approval_mode: None,
                            disabled_tools: None,
                            enabled_tools: None,
                            disabled_reason: None,
                            scopes: None,
                            oauth_resource: None,
                            tools: HashMap::new(),
                        },
                    );
                }
                _ => {}
            }
        }

        config
            .mcp_servers
            .set(new_mcp_servers)
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(config)
    }
}

fn aborted_call_output() -> FunctionCallOutputPayload {
    FunctionCallOutputPayload::from_text("aborted".to_string())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct HistoryRepairStats {
    inserted_function_call_outputs: usize,
    inserted_custom_tool_call_outputs: usize,
    dropped_orphan_function_call_outputs: usize,
    dropped_orphan_custom_tool_call_outputs: usize,
}

impl HistoryRepairStats {
    fn total(self) -> usize {
        self.inserted_function_call_outputs
            + self.inserted_custom_tool_call_outputs
            + self.dropped_orphan_function_call_outputs
            + self.dropped_orphan_custom_tool_call_outputs
    }
}

impl std::ops::AddAssign for HistoryRepairStats {
    fn add_assign(&mut self, rhs: Self) {
        self.inserted_function_call_outputs += rhs.inserted_function_call_outputs;
        self.inserted_custom_tool_call_outputs += rhs.inserted_custom_tool_call_outputs;
        self.dropped_orphan_function_call_outputs += rhs.dropped_orphan_function_call_outputs;
        self.dropped_orphan_custom_tool_call_outputs += rhs.dropped_orphan_custom_tool_call_outputs;
    }
}

fn repair_response_item_history(items: &mut Vec<ResponseItem>) -> HistoryRepairStats {
    let function_call_ids = items
        .iter()
        .filter_map(|item| match item {
            ResponseItem::FunctionCall { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let local_shell_call_ids = items
        .iter()
        .filter_map(|item| match item {
            ResponseItem::LocalShellCall {
                call_id: Some(call_id),
                ..
            } => Some(call_id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let custom_tool_call_ids = items
        .iter()
        .filter_map(|item| match item {
            ResponseItem::CustomToolCall { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();

    let mut repaired = HistoryRepairStats::default();
    items.retain(|item| match item {
        ResponseItem::FunctionCallOutput { call_id, .. } => {
            let keep =
                function_call_ids.contains(call_id) || local_shell_call_ids.contains(call_id);
            if !keep {
                repaired.dropped_orphan_function_call_outputs += 1;
            }
            keep
        }
        ResponseItem::CustomToolCallOutput { call_id, .. } => {
            let keep = custom_tool_call_ids.contains(call_id);
            if !keep {
                repaired.dropped_orphan_custom_tool_call_outputs += 1;
            }
            keep
        }
        _ => true,
    });
    let mut function_output_call_ids = items
        .iter()
        .filter_map(|item| match item {
            ResponseItem::FunctionCallOutput { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let mut custom_output_call_ids = items
        .iter()
        .filter_map(|item| match item {
            ResponseItem::CustomToolCallOutput { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();

    let mut synthetic_outputs = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        match item {
            ResponseItem::FunctionCall { call_id, .. }
                if function_output_call_ids.insert(call_id.clone()) =>
            {
                repaired.inserted_function_call_outputs += 1;
                synthetic_outputs.push((
                    idx,
                    ResponseItem::FunctionCallOutput {
                        call_id: call_id.clone(),
                        output: aborted_call_output(),
                    },
                ));
            }
            ResponseItem::CustomToolCall { call_id, .. }
                if custom_output_call_ids.insert(call_id.clone()) =>
            {
                repaired.inserted_custom_tool_call_outputs += 1;
                synthetic_outputs.push((
                    idx,
                    ResponseItem::CustomToolCallOutput {
                        call_id: call_id.clone(),
                        name: None,
                        output: aborted_call_output(),
                    },
                ));
            }
            ResponseItem::LocalShellCall {
                call_id: Some(call_id),
                ..
            } if function_output_call_ids.insert(call_id.clone()) => {
                repaired.inserted_function_call_outputs += 1;
                synthetic_outputs.push((
                    idx,
                    ResponseItem::FunctionCallOutput {
                        call_id: call_id.clone(),
                        output: aborted_call_output(),
                    },
                ));
            }
            _ => {}
        }
    }

    for (idx, output) in synthetic_outputs.into_iter().rev() {
        items.insert(idx + 1, output);
    }
    repaired
}

fn repair_rollout_items(items: &mut Vec<RolloutItem>) -> HistoryRepairStats {
    let function_call_ids = items
        .iter()
        .filter_map(|item| match item {
            RolloutItem::ResponseItem(ResponseItem::FunctionCall { call_id, .. }) => {
                Some(call_id.clone())
            }
            _ => None,
        })
        .collect::<HashSet<_>>();
    let local_shell_call_ids = items
        .iter()
        .filter_map(|item| match item {
            RolloutItem::ResponseItem(ResponseItem::LocalShellCall {
                call_id: Some(call_id),
                ..
            }) => Some(call_id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let custom_tool_call_ids = items
        .iter()
        .filter_map(|item| match item {
            RolloutItem::ResponseItem(ResponseItem::CustomToolCall { call_id, .. }) => {
                Some(call_id.clone())
            }
            _ => None,
        })
        .collect::<HashSet<_>>();

    let mut repaired = HistoryRepairStats::default();
    let mut retained = Vec::with_capacity(items.len());
    for item in std::mem::take(items) {
        match item {
            RolloutItem::ResponseItem(ResponseItem::FunctionCallOutput { call_id, output }) => {
                if function_call_ids.contains(&call_id) || local_shell_call_ids.contains(&call_id) {
                    retained.push(RolloutItem::ResponseItem(
                        ResponseItem::FunctionCallOutput { call_id, output },
                    ));
                } else {
                    repaired.dropped_orphan_function_call_outputs += 1;
                }
            }
            RolloutItem::ResponseItem(ResponseItem::CustomToolCallOutput {
                call_id,
                output,
                ..
            }) => {
                if custom_tool_call_ids.contains(&call_id) {
                    retained.push(RolloutItem::ResponseItem(
                        ResponseItem::CustomToolCallOutput {
                            call_id,
                            name: None,
                            output,
                        },
                    ));
                } else {
                    repaired.dropped_orphan_custom_tool_call_outputs += 1;
                }
            }
            RolloutItem::Compacted(mut compacted) => {
                if let Some(replacement_history) = compacted.replacement_history.as_mut() {
                    repaired += repair_response_item_history(replacement_history);
                }
                retained.push(RolloutItem::Compacted(compacted));
            }
            other => retained.push(other),
        }
    }
    *items = retained;
    let mut function_output_call_ids = items
        .iter()
        .filter_map(|item| match item {
            RolloutItem::ResponseItem(ResponseItem::FunctionCallOutput { call_id, .. }) => {
                Some(call_id.clone())
            }
            _ => None,
        })
        .collect::<HashSet<_>>();
    let mut custom_output_call_ids = items
        .iter()
        .filter_map(|item| match item {
            RolloutItem::ResponseItem(ResponseItem::CustomToolCallOutput { call_id, .. }) => {
                Some(call_id.clone())
            }
            _ => None,
        })
        .collect::<HashSet<_>>();

    let mut synthetic_outputs = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        match item {
            RolloutItem::ResponseItem(ResponseItem::FunctionCall { call_id, .. })
                if function_output_call_ids.insert(call_id.clone()) =>
            {
                repaired.inserted_function_call_outputs += 1;
                synthetic_outputs.push((
                    idx,
                    RolloutItem::ResponseItem(ResponseItem::FunctionCallOutput {
                        call_id: call_id.clone(),
                        output: aborted_call_output(),
                    }),
                ));
            }
            RolloutItem::ResponseItem(ResponseItem::CustomToolCall { call_id, .. })
                if custom_output_call_ids.insert(call_id.clone()) =>
            {
                repaired.inserted_custom_tool_call_outputs += 1;
                synthetic_outputs.push((
                    idx,
                    RolloutItem::ResponseItem(ResponseItem::CustomToolCallOutput {
                        call_id: call_id.clone(),
                        name: None,
                        output: aborted_call_output(),
                    }),
                ));
            }
            RolloutItem::ResponseItem(ResponseItem::LocalShellCall {
                call_id: Some(call_id),
                ..
            }) if function_output_call_ids.insert(call_id.clone()) => {
                repaired.inserted_function_call_outputs += 1;
                synthetic_outputs.push((
                    idx,
                    RolloutItem::ResponseItem(ResponseItem::FunctionCallOutput {
                        call_id: call_id.clone(),
                        output: aborted_call_output(),
                    }),
                ));
            }
            _ => {}
        }
    }

    for (idx, output) in synthetic_outputs.into_iter().rev() {
        items.insert(idx + 1, output);
    }
    repaired
}

fn repair_initial_history(history: InitialHistory) -> (InitialHistory, HistoryRepairStats) {
    match history {
        InitialHistory::New => (InitialHistory::New, HistoryRepairStats::default()),
        InitialHistory::Cleared => (InitialHistory::Cleared, HistoryRepairStats::default()),
        InitialHistory::Forked(mut items) => {
            let repaired = repair_rollout_items(&mut items);
            (InitialHistory::Forked(items), repaired)
        }
        InitialHistory::Resumed(ResumedHistory {
            conversation_id,
            mut history,
            rollout_path,
        }) => {
            let repaired = repair_rollout_items(&mut history);
            (
                InitialHistory::Resumed(ResumedHistory {
                    conversation_id,
                    history,
                    rollout_path,
                }),
                repaired,
            )
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Agent for CodexAgent {
    async fn initialize(&self, request: InitializeRequest) -> Result<InitializeResponse, Error> {
        let InitializeRequest {
            protocol_version,
            client_capabilities,
            client_info: _, // TODO: save and pass into Codex somehow
            ..
        } = request;
        debug!("Received initialize request with protocol version {protocol_version:?}",);
        let protocol_version = ProtocolVersion::V1;

        *self.client_capabilities.lock().unwrap() = client_capabilities;

        let mut agent_capabilities = AgentCapabilities::new()
            .prompt_capabilities(PromptCapabilities::new().embedded_context(true).image(true))
            .mcp_capabilities(McpCapabilities::new().http(true))
            .load_session(true);

        agent_capabilities.session_capabilities = SessionCapabilities::new()
            .close(SessionCloseCapabilities::new())
            .list(SessionListCapabilities::new());

        let mut auth_methods = vec![
            CodexAuthMethod::ChatGpt.into(),
            CodexAuthMethod::CodexApiKey.into(),
            CodexAuthMethod::OpenAiApiKey.into(),
        ];
        // Until codex device code auth works, we can't use this in remote ssh projects
        if std::env::var("NO_BROWSER").is_ok() {
            auth_methods.remove(0);
        }

        Ok(InitializeResponse::new(protocol_version)
            .agent_capabilities(agent_capabilities)
            .agent_info(
                Implementation::new("agenthub-codex-acp", env!("CARGO_PKG_VERSION"))
                    .title("AgentHub Codex ACP"),
            )
            .auth_methods(auth_methods))
    }

    async fn authenticate(
        &self,
        request: AuthenticateRequest,
    ) -> Result<AuthenticateResponse, Error> {
        let auth_method = CodexAuthMethod::try_from(request.method_id)?;

        // Check before starting login flow if already authenticated with the same method
        if let Some(auth) = self.auth_manager.auth().await {
            match (auth, auth_method) {
                (
                    CodexAuth::ApiKey(..),
                    CodexAuthMethod::CodexApiKey | CodexAuthMethod::OpenAiApiKey,
                )
                | (CodexAuth::Chatgpt(..), CodexAuthMethod::ChatGpt) => {
                    return Ok(AuthenticateResponse::new());
                }
                _ => {}
            }
        }

        match auth_method {
            CodexAuthMethod::ChatGpt => {
                // Perform browser/device login via codex-rs, then report success/failure to the client.
                let opts = codex_login::ServerOptions::new(
                    self.config.codex_home.to_path_buf(),
                    CLIENT_ID.to_string(),
                    None,
                    self.config.cli_auth_credentials_store_mode,
                );

                let server =
                    codex_login::run_login_server(opts).map_err(Error::into_internal_error)?;

                server
                    .block_until_done()
                    .await
                    .map_err(Error::into_internal_error)?;

                self.auth_manager.reload();
            }
            CodexAuthMethod::CodexApiKey => {
                let api_key = read_codex_api_key_from_env().ok_or_else(|| {
                    Error::internal_error().data(format!("{CODEX_API_KEY_ENV_VAR} is not set"))
                })?;
                codex_login::login_with_api_key(
                    &self.config.codex_home,
                    &api_key,
                    self.config.cli_auth_credentials_store_mode,
                )
                .map_err(Error::into_internal_error)?;
            }
            CodexAuthMethod::OpenAiApiKey => {
                let api_key = read_openai_api_key_from_env().ok_or_else(|| {
                    Error::internal_error().data(format!("{OPENAI_API_KEY_ENV_VAR} is not set"))
                })?;
                codex_login::login_with_api_key(
                    &self.config.codex_home,
                    &api_key,
                    self.config.cli_auth_credentials_store_mode,
                )
                .map_err(Error::into_internal_error)?;
            }
        }

        self.auth_manager.reload();

        Ok(AuthenticateResponse::new())
    }

    async fn new_session(&self, request: NewSessionRequest) -> Result<NewSessionResponse, Error> {
        // Check before sending if authentication was successful or not
        self.check_auth().await?;

        let NewSessionRequest {
            cwd, mcp_servers, ..
        } = request;
        info!("Creating new session with cwd: {}", cwd.display());

        let config = self.build_session_config(&cwd, mcp_servers)?;
        let num_mcp_servers = config.mcp_servers.len();

        let (session_id, thread_impl) = app_server_thread::start_new_thread(config.clone()).await?;
        // Record the session root for filesystem sandboxing.
        self.session_roots
            .lock()
            .unwrap()
            .insert(session_id.clone(), config.cwd.to_path_buf());
        let thread = Rc::new(Thread::new(
            session_id.clone(),
            thread_impl,
            self.auth_manager.clone(),
            adapt_models_manager(self.thread_manager.get_models_manager()),
            self.client_capabilities.clone(),
            config.clone(),
        ));
        let load = thread.load().await?;

        self.sessions
            .borrow_mut()
            .insert(session_id.clone(), thread);

        debug!("Created new session with {} MCP servers", num_mcp_servers);

        Ok(NewSessionResponse::new(session_id)
            .modes(load.modes)
            .models(load.models)
            .config_options(load.config_options))
    }

    async fn load_session(
        &self,
        request: LoadSessionRequest,
    ) -> Result<LoadSessionResponse, Error> {
        info!("Loading session: {}", request.session_id);
        // Check before sending if authentication was successful or not
        self.check_auth().await?;

        let LoadSessionRequest {
            session_id,
            cwd,
            mcp_servers,
            ..
        } = request;

        let rollout_path =
            find_thread_path_by_id_str(&self.config.codex_home, session_id.0.as_ref())
                .await
                .map_err(|e| Error::internal_error().data(e.to_string()))?
                .ok_or_else(|| Error::resource_not_found(None))?;

        let history = RolloutRecorder::get_rollout_history(&rollout_path)
            .await
            .map_err(|e| Error::internal_error().data(e.to_string()))?;
        let (history, repaired_stats) = repair_initial_history(history);
        if repaired_stats.total() > 0 {
            warn!(
                session_id = %session_id,
                repaired_items = repaired_stats.total(),
                inserted_function_call_outputs = repaired_stats.inserted_function_call_outputs,
                inserted_custom_tool_call_outputs = repaired_stats.inserted_custom_tool_call_outputs,
                dropped_orphan_function_call_outputs = repaired_stats.dropped_orphan_function_call_outputs,
                dropped_orphan_custom_tool_call_outputs = repaired_stats.dropped_orphan_custom_tool_call_outputs,
                "repaired dirty Codex rollout history before session resume"
            );
        }

        let rollout_items = history.get_rollout_items();

        let config = self.build_session_config(&cwd, mcp_servers)?;

        let thread_impl = app_server_thread::resume_thread(config.clone(), &session_id).await?;

        let thread = Rc::new(Thread::new(
            session_id.clone(),
            thread_impl,
            self.auth_manager.clone(),
            adapt_models_manager(self.thread_manager.get_models_manager()),
            self.client_capabilities.clone(),
            config.clone(),
        ));

        thread.replay_history(rollout_items).await?;

        let load = thread.load().await?;

        self.session_roots
            .lock()
            .unwrap()
            .insert(session_id.clone(), config.cwd.to_path_buf());
        self.sessions.borrow_mut().insert(session_id, thread);

        Ok(LoadSessionResponse::new()
            .modes(load.modes)
            .models(load.models)
            .config_options(load.config_options))
    }

    async fn list_sessions(
        &self,
        request: ListSessionsRequest,
    ) -> Result<ListSessionsResponse, Error> {
        self.check_auth().await?;

        let ListSessionsRequest { cwd, cursor, .. } = request;
        let cursor_obj = cursor.as_deref().and_then(parse_cursor);

        let page = RolloutRecorder::list_threads(
            &self.config,
            SESSION_LIST_PAGE_SIZE,
            cursor_obj.as_ref(),
            ThreadSortKey::UpdatedAt,
            SortDirection::Desc,
            &[
                SessionSource::Cli,
                SessionSource::VSCode,
                SessionSource::Unknown,
            ],
            None,
            None,
            self.config.model_provider_id.as_str(),
            None,
        )
        .await
        .map_err(|err| Error::internal_error().data(format!("failed to list sessions: {err}")))?;

        let sessions = page
            .items
            .into_iter()
            .filter_map(|item| {
                let thread_id = item.thread_id?;
                let item_cwd = item.cwd?;

                if let Some(filter_cwd) = cwd.as_ref()
                    && item_cwd != *filter_cwd
                {
                    return None;
                }

                let title = item
                    .first_user_message
                    .as_deref()
                    .and_then(format_session_title);
                let updated_at = item.updated_at.or(item.created_at);

                Some(
                    SessionInfo::new(SessionId::new(thread_id.to_string()), item_cwd)
                        .title(title)
                        .updated_at(updated_at),
                )
            })
            .collect::<Vec<_>>();

        let next_cursor = page
            .next_cursor
            .as_ref()
            .and_then(|next_cursor| serde_json::to_value(next_cursor).ok())
            .and_then(|value| value.as_str().map(str::to_owned));

        Ok(ListSessionsResponse::new(sessions).next_cursor(next_cursor))
    }

    async fn close_session(
        &self,
        request: CloseSessionRequest,
    ) -> Result<CloseSessionResponse, Error> {
        self.get_thread(&request.session_id)?.shutdown().await?;
        self.thread_manager
            .remove_thread(
                &ThreadId::from_string(&request.session_id.0)
                    .map_err(Error::into_internal_error)?,
            )
            .await;
        self.sessions.borrow_mut().remove(&request.session_id);
        self.session_roots
            .lock()
            .unwrap()
            .remove(&request.session_id);
        Ok(CloseSessionResponse::new())
    }

    async fn prompt(&self, request: PromptRequest) -> Result<PromptResponse, Error> {
        info!("Processing prompt for session: {}", request.session_id);
        // Check before sending if authentication was successful or not
        self.check_auth().await?;

        // Get the session state
        let thread = self.get_thread(&request.session_id)?;
        let stop_reason = thread.prompt(request).await?;

        Ok(PromptResponse::new(stop_reason))
    }

    async fn cancel(&self, args: CancelNotification) -> Result<(), Error> {
        info!("Cancelling operations for session: {}", args.session_id);
        self.get_thread(&args.session_id)?.cancel().await?;
        Ok(())
    }

    async fn set_session_mode(
        &self,
        args: SetSessionModeRequest,
    ) -> Result<SetSessionModeResponse, Error> {
        info!("Setting session mode for session: {}", args.session_id);
        self.get_thread(&args.session_id)?
            .set_mode(args.mode_id)
            .await?;
        Ok(SetSessionModeResponse::default())
    }

    async fn set_session_model(
        &self,
        args: SetSessionModelRequest,
    ) -> Result<SetSessionModelResponse, Error> {
        info!("Setting session model for session: {}", args.session_id);

        self.get_thread(&args.session_id)?
            .set_model(args.model_id)
            .await?;

        Ok(SetSessionModelResponse::default())
    }

    async fn set_session_config_option(
        &self,
        args: SetSessionConfigOptionRequest,
    ) -> Result<SetSessionConfigOptionResponse, Error> {
        info!(
            "Setting session config option for session: {} (config_id: {}, value: {:?})",
            args.session_id, args.config_id.0, args.value
        );

        let thread = self.get_thread(&args.session_id)?;

        thread.set_config_option(args.config_id, args.value).await?;

        let config_options = thread.config_options().await?;

        Ok(SetSessionConfigOptionResponse::new(config_options))
    }
}

#[cfg(test)]
mod tests {
    use super::{HistoryRepairStats, repair_initial_history, repair_response_item_history};
    use codex_protocol::{
        ThreadId,
        models::{
            FunctionCallOutputPayload, LocalShellAction, LocalShellExecAction, LocalShellStatus,
            ResponseItem,
        },
        protocol::{CompactedItem, InitialHistory, ResumedHistory, RolloutItem},
    };
    use std::path::PathBuf;

    #[test]
    fn repair_initial_history_inserts_missing_custom_tool_outputs() {
        let thread_id = ThreadId::new();
        let history = InitialHistory::Resumed(ResumedHistory {
            conversation_id: thread_id,
            history: vec![RolloutItem::ResponseItem(ResponseItem::CustomToolCall {
                id: None,
                status: Some("completed".to_string()),
                call_id: "call-1".to_string(),
                name: "actor_send".to_string(),
                input: "{}".to_string(),
            })],
            rollout_path: PathBuf::from("/tmp/rollout.jsonl"),
        });

        let (repaired, repaired_stats) = repair_initial_history(history);
        assert_eq!(
            repaired_stats,
            HistoryRepairStats {
                inserted_custom_tool_call_outputs: 1,
                ..HistoryRepairStats::default()
            }
        );
        let items = repaired.get_rollout_items();
        assert_eq!(items.len(), 2);
        assert!(matches!(
            &items[1],
            RolloutItem::ResponseItem(ResponseItem::CustomToolCallOutput { call_id, output, .. })
                if call_id == "call-1" && output == &FunctionCallOutputPayload::from_text("aborted".to_string())
        ));
    }

    #[test]
    fn repair_response_item_history_drops_orphan_outputs() {
        let mut history = vec![
            ResponseItem::CustomToolCallOutput {
                call_id: "missing".to_string(),
                name: None,
                output: FunctionCallOutputPayload::from_text("ok".to_string()),
            },
            ResponseItem::CustomToolCall {
                id: None,
                status: Some("completed".to_string()),
                call_id: "call-2".to_string(),
                name: "actor_ack".to_string(),
                input: "{}".to_string(),
            },
        ];

        let repaired = repair_response_item_history(&mut history);
        assert_eq!(
            repaired,
            HistoryRepairStats {
                inserted_custom_tool_call_outputs: 1,
                dropped_orphan_custom_tool_call_outputs: 1,
                ..HistoryRepairStats::default()
            }
        );
        assert_eq!(history.len(), 2);
        assert!(matches!(
            &history[1],
            ResponseItem::CustomToolCallOutput { call_id, output, .. }
                if call_id == "call-2" && output == &FunctionCallOutputPayload::from_text("aborted".to_string())
        ));
    }

    #[test]
    fn repair_initial_history_updates_compacted_replacement_history() {
        let history = InitialHistory::Forked(vec![RolloutItem::Compacted(CompactedItem {
            message: "compacted".to_string(),
            replacement_history: Some(vec![ResponseItem::LocalShellCall {
                id: None,
                call_id: Some("shell-1".to_string()),
                status: LocalShellStatus::Completed,
                action: LocalShellAction::Exec(LocalShellExecAction {
                    command: vec!["echo".to_string(), "hi".to_string()],
                    timeout_ms: None,
                    working_directory: Some(".".to_string()),
                    env: None,
                    user: None,
                }),
            }]),
        })]);

        let (repaired, repaired_stats) = repair_initial_history(history);
        assert_eq!(
            repaired_stats,
            HistoryRepairStats {
                inserted_function_call_outputs: 1,
                ..HistoryRepairStats::default()
            }
        );
        let items = repaired.get_rollout_items();
        let RolloutItem::Compacted(compacted) = &items[0] else {
            panic!("expected compacted item");
        };
        let replacement_history = compacted
            .replacement_history
            .as_ref()
            .expect("replacement history");
        assert!(matches!(
            &replacement_history[1],
            ResponseItem::FunctionCallOutput { call_id, output }
                if call_id == "shell-1" && output == &FunctionCallOutputPayload::from_text("aborted".to_string())
        ));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodexAuthMethod {
    ChatGpt,
    CodexApiKey,
    OpenAiApiKey,
}

impl From<CodexAuthMethod> for AuthMethodId {
    fn from(method: CodexAuthMethod) -> Self {
        Self::new(match method {
            CodexAuthMethod::ChatGpt => "chatgpt",
            CodexAuthMethod::CodexApiKey => "codex-api-key",
            CodexAuthMethod::OpenAiApiKey => "openai-api-key",
        })
    }
}

impl From<CodexAuthMethod> for AuthMethod {
    fn from(method: CodexAuthMethod) -> Self {
        match method {
            CodexAuthMethod::ChatGpt => Self::Agent(
                AuthMethodAgent::new(method, "Login with ChatGPT").description(
                    "Use your ChatGPT login with Codex CLI (requires a paid ChatGPT subscription)",
                ),
            ),
            CodexAuthMethod::CodexApiKey => Self::EnvVar(
                AuthMethodEnvVar::new(
                    method,
                    format!("Use {CODEX_API_KEY_ENV_VAR}"),
                    vec![AuthEnvVar::new(CODEX_API_KEY_ENV_VAR)],
                )
                .description(format!(
                    "Requires setting the `{CODEX_API_KEY_ENV_VAR}` environment variable."
                )),
            ),
            CodexAuthMethod::OpenAiApiKey => Self::EnvVar(
                AuthMethodEnvVar::new(
                    method,
                    format!("Use {OPENAI_API_KEY_ENV_VAR}"),
                    vec![AuthEnvVar::new(OPENAI_API_KEY_ENV_VAR)],
                )
                .description(format!(
                    "Requires setting the `{OPENAI_API_KEY_ENV_VAR}` environment variable."
                )),
            ),
        }
    }
}

impl TryFrom<AuthMethodId> for CodexAuthMethod {
    type Error = Error;

    fn try_from(value: AuthMethodId) -> Result<Self, Self::Error> {
        match value.0.as_ref() {
            "chatgpt" => Ok(CodexAuthMethod::ChatGpt),
            "codex-api-key" => Ok(CodexAuthMethod::CodexApiKey),
            "openai-api-key" => Ok(CodexAuthMethod::OpenAiApiKey),
            _ => Err(Error::invalid_params().data("unsupported authentication method")),
        }
    }
}

fn truncate_graphemes(text: &str, max_graphemes: usize) -> String {
    let mut graphemes = text.grapheme_indices(true);

    if let Some((byte_index, _)) = graphemes.nth(max_graphemes) {
        if max_graphemes >= 3 {
            let mut truncate_graphemes = text.grapheme_indices(true);
            if let Some((truncate_byte_index, _)) = truncate_graphemes.nth(max_graphemes - 3) {
                let truncated = &text[..truncate_byte_index];
                format!("{truncated}...")
            } else {
                text.to_string()
            }
        } else {
            let truncated = &text[..byte_index];
            truncated.to_string()
        }
    } else {
        text.to_string()
    }
}

fn format_session_title(message: &str) -> Option<String> {
    let normalized = message.replace(['\r', '\n'], " ");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(truncate_graphemes(trimmed, SESSION_TITLE_MAX_GRAPHEMES))
    }
}
