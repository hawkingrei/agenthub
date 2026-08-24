use agent_client_protocol::Error;
use agent_client_protocol::schema::v1::{
    AgentCapabilities, AuthEnvVar, AuthMethod, AuthMethodAgent, AuthMethodEnvVar, AuthMethodId,
    AuthenticateRequest, AuthenticateResponse, CancelNotification, ClientCapabilities,
    CloseSessionRequest, CloseSessionResponse, Implementation, InitializeRequest,
    InitializeResponse, ListSessionsRequest, ListSessionsResponse, LoadSessionRequest,
    LoadSessionResponse, McpCapabilities, McpServer, McpServerHttp, McpServerStdio,
    NewSessionRequest, NewSessionResponse, PromptCapabilities, PromptRequest, PromptResponse,
    SessionCapabilities, SessionCloseCapabilities, SessionId, SessionInfo, SessionListCapabilities,
    SetSessionConfigOptionRequest, SetSessionConfigOptionResponse, SetSessionModeRequest,
    SetSessionModeResponse,
};
// `ProtocolVersion` is version-agnostic in 0.15 (schema root, not `schema::v1`).
use agent_client_protocol::schema::ProtocolVersion;
use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_config::types::{AuthKeyringBackendKind, McpServerConfig, McpServerTransportConfig};
use codex_core::{
    CodexAppsToolsCache, RolloutRecorder, SortDirection, ThreadManager, ThreadSortKey,
    build_models_manager, config::Config, find_thread_path_by_id_str, parse_cursor,
    thread_store_from_config,
};
use codex_extension_api::{
    LoadUserInstructionsFuture, LoadedUserInstructions, UserInstructionsProvider,
    empty_extension_registry,
};
use codex_history::{
    InitialHistory, ResponseItemEnvelope, ResumedHistory, RolloutItem, RolloutLine,
};
use codex_login::auth::{read_codex_api_key_from_env, read_openai_api_key_from_env};
use codex_login::{
    AuthManager, CLIENT_ID, CODEX_API_KEY_ENV_VAR, CodexAuth, OPENAI_API_KEY_ENV_VAR,
};
use codex_protocol::{
    ThreadId,
    models::{FunctionCallOutputPayload, ResponseItem},
    protocol::SessionSource,
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
const REPAIRED_ROLLOUT_TIMESTAMP: &str = "1970-01-01T00:00:00.000Z";

struct EmptyUserInstructionsProvider;

impl UserInstructionsProvider for EmptyUserInstructionsProvider {
    fn load_user_instructions(&self) -> LoadUserInstructionsFuture<'_> {
        Box::pin(std::future::ready(LoadedUserInstructions::default()))
    }
}

impl CodexAgent {
    /// Create a new `CodexAgent` with the given configuration
    pub async fn new(config: Config) -> Result<Self, Error> {
        let auth_manager = AuthManager::shared(
            config.codex_home.to_path_buf(),
            false,
            config.cli_auth_credentials_store_mode,
            config.forced_chatgpt_workspace_id.clone(),
            Some(config.chatgpt_base_url.clone()),
            AuthKeyringBackendKind::default(),
            config.auth_route_config(),
        )
        .await;

        let client_capabilities: Arc<Mutex<ClientCapabilities>> = Arc::default();

        let session_roots: Arc<Mutex<HashMap<SessionId, PathBuf>>> = Arc::default();
        let thread_manager = ThreadManager::new(
            &config,
            auth_manager.clone(),
            build_models_manager(&config, auth_manager.clone()),
            CodexAppsToolsCache::default(),
            SessionSource::Unknown,
            build_environment_manager(&config).await?,
            empty_extension_registry(),
            Arc::new(EmptyUserInstructionsProvider),
            None,
            thread_store_from_config(&config, None),
            None,
            "agenthub-codex-acp".to_string(),
            None,
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
        config.cwd = cwd
            .to_path_buf()
            .try_into()
            .map_err(|e: std::io::Error| Error::internal_error().data(e.to_string()))?;

        // Propagate any client-provided MCP servers that codex-rs supports.
        let mut new_mcp_servers = config.mcp_servers.get().clone();
        for mcp_server in mcp_servers {
            if let Some((name, mcp_server_config)) = codex_mcp_server_config(cwd, mcp_server) {
                new_mcp_servers.insert(name, mcp_server_config);
            }
        }

        config
            .mcp_servers
            .set(new_mcp_servers)
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(config)
    }
}

fn sanitize_codex_mcp_server_name(name: &str) -> String {
    name.replace(|c: char| c.is_whitespace(), "_")
}

fn agenthub_managed_mcp_supports_parallel_tool_calls() -> bool {
    // Keep this opt-out until AgentHub can prove each managed MCP server is safe
    // for concurrent calls. Codex supports the flag, but ACP does not expose a
    // per-server concurrency contract for these passthrough definitions.
    false
}

fn codex_mcp_server_config(cwd: &Path, mcp_server: McpServer) -> Option<(String, McpServerConfig)> {
    let (name, transport) = match mcp_server {
        McpServer::Http(McpServerHttp {
            name, url, headers, ..
        }) => (
            name,
            McpServerTransportConfig::StreamableHttp {
                url,
                bearer_token_env_var: None,
                http_headers: if headers.is_empty() {
                    None
                } else {
                    Some(headers.into_iter().map(|h| (h.name, h.value)).collect())
                },
                env_http_headers: None,
                http_headers_helper: None,
            },
        ),
        McpServer::Stdio(McpServerStdio {
            name,
            command,
            args,
            env,
            ..
        }) => (
            name,
            McpServerTransportConfig::Stdio {
                command: command.display().to_string(),
                args,
                env: if env.is_empty() {
                    None
                } else {
                    Some(env.into_iter().map(|env| (env.name, env.value)).collect())
                },
                env_vars: vec![],
                cwd: codex_utils_absolute_path::AbsolutePathBuf::try_from(cwd)
                    .ok()
                    .map(Into::into),
            },
        ),
        // Codex does not support ACP SSE MCP servers.
        _ => return None,
    };

    Some((
        sanitize_codex_mcp_server_name(&name),
        McpServerConfig {
            transport,
            auth: Default::default(),
            environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
            required: false,
            enabled: true,
            supports_parallel_tool_calls: agenthub_managed_mcp_supports_parallel_tool_calls(),
            omit_tools_from: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            default_tools_approval_mode: None,
            disabled_tools: None,
            enabled_tools: None,
            disabled_reason: None,
            scopes: None,
            oauth: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    ))
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

fn repair_response_item_history(items: &mut Vec<ResponseItemEnvelope>) -> HistoryRepairStats {
    let function_call_ids = items
        .iter()
        .filter_map(|item| match &item.item {
            ResponseItem::FunctionCall { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let local_shell_call_ids = items
        .iter()
        .filter_map(|item| match &item.item {
            ResponseItem::LocalShellCall {
                call_id: Some(call_id),
                ..
            } => Some(call_id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let custom_tool_call_ids = items
        .iter()
        .filter_map(|item| match &item.item {
            ResponseItem::CustomToolCall { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();

    let mut repaired = HistoryRepairStats::default();
    items.retain(|item| match &item.item {
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
        .filter_map(|item| match &item.item {
            ResponseItem::FunctionCallOutput { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let mut custom_output_call_ids = items
        .iter()
        .filter_map(|item| match &item.item {
            ResponseItem::CustomToolCallOutput { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();

    let mut synthetic_outputs = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        match &item.item {
            ResponseItem::FunctionCall { call_id, .. }
                if function_output_call_ids.insert(call_id.clone()) =>
            {
                repaired.inserted_function_call_outputs += 1;
                synthetic_outputs.push((
                    idx,
                    ResponseItemEnvelope::from(ResponseItem::FunctionCallOutput {
                        id: None,
                        call_id: call_id.clone(),
                        output: aborted_call_output(),
                        internal_chat_message_metadata_passthrough: None,
                    }),
                ));
            }
            ResponseItem::CustomToolCall { call_id, .. }
                if custom_output_call_ids.insert(call_id.clone()) =>
            {
                repaired.inserted_custom_tool_call_outputs += 1;
                synthetic_outputs.push((
                    idx,
                    ResponseItemEnvelope::from(ResponseItem::CustomToolCallOutput {
                        id: None,
                        call_id: call_id.clone(),
                        name: None,
                        output: aborted_call_output(),
                        internal_chat_message_metadata_passthrough: None,
                    }),
                ));
            }
            ResponseItem::LocalShellCall {
                call_id: Some(call_id),
                ..
            } if function_output_call_ids.insert(call_id.clone()) => {
                repaired.inserted_function_call_outputs += 1;
                synthetic_outputs.push((
                    idx,
                    ResponseItemEnvelope::from(ResponseItem::FunctionCallOutput {
                        id: None,
                        call_id: call_id.clone(),
                        output: aborted_call_output(),
                        internal_chat_message_metadata_passthrough: None,
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

fn repair_rollout_items(items: &mut Vec<RolloutItem>) -> HistoryRepairStats {
    let function_call_ids = items
        .iter()
        .filter_map(|item| match item {
            RolloutItem::ResponseItem(item) => match &item.item {
                ResponseItem::FunctionCall { call_id, .. } => Some(call_id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect::<HashSet<_>>();
    let local_shell_call_ids = items
        .iter()
        .filter_map(|item| match item {
            RolloutItem::ResponseItem(item) => match &item.item {
                ResponseItem::LocalShellCall {
                    call_id: Some(call_id),
                    ..
                } => Some(call_id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect::<HashSet<_>>();
    let custom_tool_call_ids = items
        .iter()
        .filter_map(|item| match item {
            RolloutItem::ResponseItem(item) => match &item.item {
                ResponseItem::CustomToolCall { call_id, .. } => Some(call_id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect::<HashSet<_>>();

    let mut repaired = HistoryRepairStats::default();
    let mut retained = Vec::with_capacity(items.len());
    for item in std::mem::take(items) {
        match item {
            RolloutItem::ResponseItem(response_item) => {
                let keep = match &response_item.item {
                    ResponseItem::FunctionCallOutput { call_id, .. } => {
                        let keep = function_call_ids.contains(call_id)
                            || local_shell_call_ids.contains(call_id);
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
                };
                if keep {
                    retained.push(RolloutItem::ResponseItem(response_item));
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
            RolloutItem::ResponseItem(item) => match &item.item {
                ResponseItem::FunctionCallOutput { call_id, .. } => Some(call_id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect::<HashSet<_>>();
    let mut custom_output_call_ids = items
        .iter()
        .filter_map(|item| match item {
            RolloutItem::ResponseItem(item) => match &item.item {
                ResponseItem::CustomToolCallOutput { call_id, .. } => Some(call_id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect::<HashSet<_>>();

    let mut synthetic_outputs = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        match item {
            RolloutItem::ResponseItem(item) => match &item.item {
                ResponseItem::FunctionCall { call_id, .. }
                    if function_output_call_ids.insert(call_id.clone()) =>
                {
                    repaired.inserted_function_call_outputs += 1;
                    synthetic_outputs.push((
                        idx,
                        RolloutItem::ResponseItem(ResponseItemEnvelope::from(
                            ResponseItem::FunctionCallOutput {
                                id: None,
                                call_id: call_id.clone(),
                                output: aborted_call_output(),
                                internal_chat_message_metadata_passthrough: None,
                            },
                        )),
                    ));
                }
                ResponseItem::CustomToolCall { call_id, .. }
                    if custom_output_call_ids.insert(call_id.clone()) =>
                {
                    repaired.inserted_custom_tool_call_outputs += 1;
                    synthetic_outputs.push((
                        idx,
                        RolloutItem::ResponseItem(ResponseItemEnvelope::from(
                            ResponseItem::CustomToolCallOutput {
                                id: None,
                                call_id: call_id.clone(),
                                name: None,
                                output: aborted_call_output(),
                                internal_chat_message_metadata_passthrough: None,
                            },
                        )),
                    ));
                }
                ResponseItem::LocalShellCall {
                    call_id: Some(call_id),
                    ..
                } if function_output_call_ids.insert(call_id.clone()) => {
                    repaired.inserted_function_call_outputs += 1;
                    synthetic_outputs.push((
                        idx,
                        RolloutItem::ResponseItem(ResponseItemEnvelope::from(
                            ResponseItem::FunctionCallOutput {
                                id: None,
                                call_id: call_id.clone(),
                                output: aborted_call_output(),
                                internal_chat_message_metadata_passthrough: None,
                            },
                        )),
                    ));
                }
                _ => {}
            },
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
            let repaired = repair_rollout_items(Arc::make_mut(&mut history));
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

async fn persist_repaired_initial_history(
    rollout_path: &Path,
    history: &InitialHistory,
) -> std::io::Result<()> {
    let temp_path = rollout_path.with_extension("jsonl.agenthub-repair-tmp");
    let timestamp = repaired_rollout_timestamp(history);
    let mut contents = String::new();
    for item in history.get_rollout_items() {
        let line = RolloutLine {
            timestamp: timestamp.clone(),
            ordinal: None,
            item: item.clone(),
        };
        let serialized = serde_json::to_string(&line).map_err(|err| {
            std::io::Error::other(format!("failed to serialize repaired rollout line: {err}"))
        })?;
        contents.push_str(&serialized);
        contents.push('\n');
    }

    tokio::fs::write(&temp_path, contents).await?;
    tokio::fs::rename(&temp_path, rollout_path).await?;
    Ok(())
}

fn repaired_rollout_timestamp(history: &InitialHistory) -> String {
    history
        .get_rollout_items()
        .iter()
        .find_map(|item| match item {
            RolloutItem::SessionMeta(meta_line) if !meta_line.meta.timestamp.is_empty() => {
                Some(meta_line.meta.timestamp.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| REPAIRED_ROLLOUT_TIMESTAMP.to_string())
}

impl CodexAgent {
    pub(crate) async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> Result<InitializeResponse, Error> {
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

    pub(crate) async fn authenticate(
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
                    AuthKeyringBackendKind::default(),
                    self.config.auth_route_config(),
                );

                let server =
                    codex_login::run_login_server(opts).map_err(Error::into_internal_error)?;

                server
                    .block_until_done()
                    .await
                    .map_err(Error::into_internal_error)?;
            }
            CodexAuthMethod::CodexApiKey => {
                let api_key = read_codex_api_key_from_env().ok_or_else(|| {
                    Error::internal_error().data(format!("{CODEX_API_KEY_ENV_VAR} is not set"))
                })?;
                codex_login::login_with_api_key(
                    &self.config.codex_home,
                    &api_key,
                    self.config.cli_auth_credentials_store_mode,
                    AuthKeyringBackendKind::default(),
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
                    AuthKeyringBackendKind::default(),
                )
                .map_err(Error::into_internal_error)?;
            }
        }

        self.auth_manager.reload().await;

        Ok(AuthenticateResponse::new())
    }

    pub(crate) async fn new_session(
        &self,
        request: NewSessionRequest,
    ) -> Result<NewSessionResponse, Error> {
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
            adapt_models_manager(self.thread_manager.get_models_manager(), config.clone()),
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
            .config_options(load.config_options))
    }

    pub(crate) async fn load_session(
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
            find_thread_path_by_id_str(&self.config.codex_home, session_id.0.as_ref(), None)
                .await
                .map_err(|e| Error::internal_error().data(e.to_string()))?
                .ok_or_else(|| Error::resource_not_found(None))?;

        let history = RolloutRecorder::get_rollout_history(&rollout_path)
            .await
            .map_err(|e| Error::internal_error().data(e.to_string()))?;
        let (history, repaired_stats) = repair_initial_history(history);
        if repaired_stats.total() > 0 {
            persist_repaired_initial_history(&rollout_path, &history)
                .await
                .map_err(|e| Error::internal_error().data(e.to_string()))?;
            warn!(
                session_id = %session_id,
                repaired_items = repaired_stats.total(),
                inserted_function_call_outputs = repaired_stats.inserted_function_call_outputs,
                inserted_custom_tool_call_outputs = repaired_stats.inserted_custom_tool_call_outputs,
                dropped_orphan_function_call_outputs = repaired_stats.dropped_orphan_function_call_outputs,
                dropped_orphan_custom_tool_call_outputs = repaired_stats.dropped_orphan_custom_tool_call_outputs,
                "repaired and persisted dirty Codex rollout history before session resume"
            );
        }

        let rollout_items = history.get_rollout_items();

        let config = self.build_session_config(&cwd, mcp_servers)?;

        let thread_impl = app_server_thread::resume_thread(config.clone(), &session_id).await?;

        let thread = Rc::new(Thread::new(
            session_id.clone(),
            thread_impl,
            self.auth_manager.clone(),
            adapt_models_manager(self.thread_manager.get_models_manager(), config.clone()),
            self.client_capabilities.clone(),
            config.clone(),
        ));

        thread.replay_history(rollout_items.to_vec()).await?;

        let load = thread.load().await?;

        self.session_roots
            .lock()
            .unwrap()
            .insert(session_id.clone(), config.cwd.to_path_buf());
        self.sessions.borrow_mut().insert(session_id, thread);

        Ok(LoadSessionResponse::new()
            .modes(load.modes)
            .config_options(load.config_options))
    }

    pub(crate) async fn list_sessions(
        &self,
        request: ListSessionsRequest,
    ) -> Result<ListSessionsResponse, Error> {
        self.check_auth().await?;

        let ListSessionsRequest { cwd, cursor, .. } = request;
        let cursor_obj = cursor.as_deref().and_then(parse_cursor);

        let page = RolloutRecorder::list_threads(
            None,
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

    pub(crate) async fn close_session(
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

    pub(crate) async fn prompt(&self, request: PromptRequest) -> Result<PromptResponse, Error> {
        info!("Processing prompt for session: {}", request.session_id);
        // Check before sending if authentication was successful or not
        self.check_auth().await?;

        // Get the session state
        let thread = self.get_thread(&request.session_id)?;
        let stop_reason = thread.prompt(request).await?;

        Ok(PromptResponse::new(stop_reason))
    }

    pub(crate) async fn cancel(&self, args: CancelNotification) -> Result<(), Error> {
        info!("Cancelling operations for session: {}", args.session_id);
        self.get_thread(&args.session_id)?.cancel().await?;
        Ok(())
    }

    pub(crate) async fn set_session_mode(
        &self,
        args: SetSessionModeRequest,
    ) -> Result<SetSessionModeResponse, Error> {
        info!("Setting session mode for session: {}", args.session_id);
        self.get_thread(&args.session_id)?
            .set_mode(args.mode_id)
            .await?;
        Ok(SetSessionModeResponse::default())
    }

    pub(crate) async fn set_session_config_option(
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
    use super::{
        HistoryRepairStats, codex_mcp_server_config, persist_repaired_initial_history,
        repair_initial_history, repair_response_item_history,
    };
    use agent_client_protocol::schema::v1::{
        EnvVariable, HttpHeader, McpServer, McpServerHttp, McpServerSse, McpServerStdio,
    };
    use codex_config::types::McpServerTransportConfig;
    use codex_core::RolloutRecorder;
    use codex_core::config::ConfigBuilder;
    use codex_history::{
        CompactedItem, InitialHistory, ResponseItemEnvelope, ResumedHistory, RolloutItem,
    };
    use codex_protocol::{
        ThreadId,
        models::{
            FunctionCallOutputPayload, LocalShellAction, LocalShellExecAction, LocalShellStatus,
            ResponseItem,
        },
        protocol::{SessionMeta, SessionMetaLine, SessionSource},
    };
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
        sync::Arc,
    };

    #[tokio::test]
    async fn codex_agent_new_initializes_thread_manager() {
        let codex_home =
            std::env::temp_dir().join(format!("agenthub-codex-home-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&codex_home).expect("create codex home");
        let config = ConfigBuilder::default()
            .codex_home(codex_home.clone())
            .fallback_cwd(Some(codex_home))
            .build()
            .await
            .expect("build config");

        let agent = super::CodexAgent::new(config)
            .await
            .expect("create codex agent");

        assert!(agent.sessions.borrow().is_empty());
    }

    #[test]
    fn codex_mcp_http_servers_keep_parallel_tool_calls_disabled() {
        let cwd = PathBuf::from("/tmp/agenthub-codex-acp-test");
        let (name, config) = codex_mcp_server_config(
            &cwd,
            McpServer::Http(
                McpServerHttp::new("AgentHub Tools", "https://mcp.example.test")
                    .headers(vec![HttpHeader::new("Authorization", "Bearer test")]),
            ),
        )
        .expect("http mcp server should be supported");

        assert_eq!(name, "AgentHub_Tools");
        assert!(!config.supports_parallel_tool_calls);
        assert_eq!(config.oauth, None);
        let McpServerTransportConfig::StreamableHttp {
            url, http_headers, ..
        } = config.transport
        else {
            panic!("expected streamable http transport");
        };
        assert_eq!(url, "https://mcp.example.test");
        assert_eq!(
            http_headers.expect("headers"),
            HashMap::from([("Authorization".to_string(), "Bearer test".to_string())])
        );
    }

    #[test]
    fn codex_mcp_stdio_servers_keep_parallel_tool_calls_disabled() {
        let cwd = PathBuf::from("/tmp/agenthub-codex-acp-test");
        let (name, config) = codex_mcp_server_config(
            &cwd,
            McpServer::Stdio(
                McpServerStdio::new("Mailbox Bridge", "agenthub")
                    .args(vec!["actor".to_string(), "receive".to_string()])
                    .env(vec![EnvVariable::new("AGENTHUB_TEST", "1")]),
            ),
        )
        .expect("stdio mcp server should be supported");

        assert_eq!(name, "Mailbox_Bridge");
        assert!(!config.supports_parallel_tool_calls);
        assert_eq!(config.oauth, None);
        let McpServerTransportConfig::Stdio {
            command,
            args,
            env,
            cwd: config_cwd,
            ..
        } = config.transport
        else {
            panic!("expected stdio transport");
        };
        assert_eq!(command, "agenthub");
        assert_eq!(args, vec!["actor".to_string(), "receive".to_string()]);
        assert_eq!(
            env.expect("env"),
            HashMap::from([("AGENTHUB_TEST".to_string(), "1".to_string())])
        );
        assert_eq!(
            config_cwd.as_ref().map(|path| path.as_str()),
            Some(cwd.to_str().expect("test cwd is valid UTF-8"))
        );
    }

    #[test]
    fn codex_mcp_sse_servers_remain_unsupported() {
        let cwd = PathBuf::from("/tmp/agenthub-codex-acp-test");
        let config = codex_mcp_server_config(
            &cwd,
            McpServer::Sse(McpServerSse::new("legacy", "https://mcp.example.test/sse")),
        );

        assert!(config.is_none());
    }

    #[test]
    fn codex_mcp_stdio_servers_resolve_relative_cwd() {
        let (_, config) = codex_mcp_server_config(
            Path::new("relative-workspace"),
            McpServer::Stdio(McpServerStdio::new("Mailbox Bridge", "agenthub")),
        )
        .expect("stdio mcp server should be supported");

        let McpServerTransportConfig::Stdio { cwd, .. } = config.transport else {
            panic!("expected stdio transport");
        };
        assert_eq!(
            cwd.as_ref().map(|path| path.as_str()),
            Some(
                std::env::current_dir()
                    .expect("read current directory")
                    .join("relative-workspace")
                    .to_str()
                    .expect("test cwd is valid UTF-8")
            )
        );
    }

    #[test]
    fn repair_initial_history_inserts_missing_custom_tool_outputs() {
        let thread_id = ThreadId::new();
        let history = InitialHistory::Resumed(ResumedHistory {
            conversation_id: thread_id,
            history: Arc::new(vec![RolloutItem::ResponseItem(ResponseItemEnvelope::from(
                ResponseItem::CustomToolCall {
                    id: None,
                    status: Some("completed".to_string()),
                    call_id: "call-1".to_string(),
                    name: "actor_send".to_string(),
                    namespace: None,
                    input: "{}".to_string(),
                    internal_chat_message_metadata_passthrough: None,
                },
            ))]),
            rollout_path: Some(PathBuf::from("/tmp/rollout.jsonl")),
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
            RolloutItem::ResponseItem(ResponseItemEnvelope {
                item: ResponseItem::CustomToolCallOutput { call_id, output, .. },
                ..
            }) if call_id == "call-1" && output.text_content() == Some("aborted")
        ));
    }

    #[tokio::test]
    async fn persist_repaired_initial_history_rewrites_rollout_before_resume() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let rollout_path = temp_dir.path().join("rollout.jsonl");
        let thread_id = ThreadId::new();
        let history = InitialHistory::Resumed(ResumedHistory {
            conversation_id: thread_id,
            history: Arc::new(vec![
                RolloutItem::SessionMeta(SessionMetaLine {
                    meta: SessionMeta {
                        id: thread_id,
                        timestamp: "2026-05-07T00:00:00.000Z".to_string(),
                        cwd: temp_dir.path().to_path_buf(),
                        originator: "agenthub-test".to_string(),
                        cli_version: "test".to_string(),
                        source: SessionSource::Custom("agenthub-test".to_string()),
                        ..SessionMeta::default()
                    },
                    git: None,
                }),
                RolloutItem::ResponseItem(ResponseItemEnvelope::from(
                    ResponseItem::CustomToolCall {
                        id: None,
                        status: Some("completed".to_string()),
                        call_id: "call-persist".to_string(),
                        name: "actor_send".to_string(),
                        namespace: None,
                        input: "{}".to_string(),
                        internal_chat_message_metadata_passthrough: None,
                    },
                )),
            ]),
            rollout_path: Some(rollout_path.clone()),
        });

        let (repaired, repaired_stats) = repair_initial_history(history);
        assert_eq!(
            repaired_stats,
            HistoryRepairStats {
                inserted_custom_tool_call_outputs: 1,
                ..HistoryRepairStats::default()
            }
        );

        persist_repaired_initial_history(&rollout_path, &repaired)
            .await
            .expect("persist repaired rollout");
        let text = tokio::fs::read_to_string(&rollout_path)
            .await
            .expect("read repaired rollout");
        assert!(
            text.lines()
                .all(|line| line.contains("2026-05-07T00:00:00.000Z"))
        );

        let loaded = RolloutRecorder::get_rollout_history(&rollout_path)
            .await
            .expect("load repaired rollout");
        let items = loaded.get_rollout_items();

        assert_eq!(items.len(), 3);
        assert!(matches!(&items[0], RolloutItem::SessionMeta(_)));
        assert!(matches!(
            &items[2],
            RolloutItem::ResponseItem(ResponseItemEnvelope {
                item: ResponseItem::CustomToolCallOutput { call_id, output, .. },
                ..
            }) if call_id == "call-persist" && output.text_content() == Some("aborted")
        ));
    }

    #[test]
    fn repair_response_item_history_drops_orphan_outputs() {
        let mut history = vec![
            ResponseItemEnvelope::from(ResponseItem::CustomToolCallOutput {
                id: None,
                call_id: "missing".to_string(),
                name: None,
                output: FunctionCallOutputPayload::from_text("ok".to_string()),
                internal_chat_message_metadata_passthrough: None,
            }),
            ResponseItemEnvelope::from(ResponseItem::CustomToolCall {
                id: None,
                status: Some("completed".to_string()),
                call_id: "call-2".to_string(),
                name: "actor_ack".to_string(),
                namespace: None,
                input: "{}".to_string(),
                internal_chat_message_metadata_passthrough: None,
            }),
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
            &history[1].item,
            ResponseItem::CustomToolCallOutput { call_id, output, .. }
                if call_id == "call-2" && output.text_content() == Some("aborted")
        ));
    }

    #[test]
    fn repair_initial_history_updates_compacted_replacement_history() {
        let history = InitialHistory::Forked(vec![RolloutItem::Compacted(CompactedItem {
            message: "compacted".to_string(),
            replacement_history: Some(vec![ResponseItemEnvelope::from(
                ResponseItem::LocalShellCall {
                    id: None,
                    call_id: Some("shell-1".to_string()),
                    status: LocalShellStatus::Completed,
                    internal_chat_message_metadata_passthrough: None,
                    action: LocalShellAction::Exec(LocalShellExecAction {
                        command: vec!["echo".to_string(), "hi".to_string()],
                        timeout_ms: None,
                        working_directory: Some(".".to_string()),
                        env: None,
                        user: None,
                    }),
                },
            )]),
            mcp_resource_origins: None,
            window_number: None,
            first_window_id: None,
            previous_window_id: None,
            window_id: None,
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
            &replacement_history[1].item,
            ResponseItem::FunctionCallOutput {
                call_id, output, ..
            }
                if call_id == "shell-1" && output.text_content() == Some("aborted")
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
