use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Component, Path, PathBuf},
    sync::{Arc, LazyLock, Mutex},
};

use agent_client_protocol_legacy::{Client, Error};
use agent_client_protocol_legacy::{
    AvailableCommand, AvailableCommandInput, AvailableCommandsUpdate, ClientCapabilities,
    ConfigOptionUpdate, Content, ContentBlock, ContentChunk, Diff, EmbeddedResource,
    EmbeddedResourceResource, LoadSessionResponse, Meta, ModelId, ModelInfo, PermissionOption,
    PermissionOptionKind, Plan, PlanEntry, PlanEntryPriority, PlanEntryStatus, PromptRequest,
    RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse, ResourceLink,
    SelectedPermissionOutcome, SessionConfigId, SessionConfigOption,
    SessionConfigOptionCategory, SessionConfigOptionValue, SessionConfigSelectOption,
    SessionConfigValueId, SessionId, SessionInfoUpdate, SessionMode, SessionModeId,
    SessionModeState, SessionModelState, SessionNotification, SessionUpdate, StopReason, Terminal,
    TextResourceContents, ToolCall, ToolCallContent, ToolCallId, ToolCallLocation, ToolCallStatus,
    ToolCallUpdate, ToolCallUpdateFields, ToolKind, UnstructuredCommandInput, UsageUpdate,
};
use agenthub_managed_skills::managed_skills_root;
use codex_apply_patch::parse_patch;
use codex_core::{
    CodexThread,
    config::{Config, set_project_trust_level},
    review_format::format_review_findings_block,
    review_prompts::user_facing_hint,
};
use codex_login::AuthManager;
use codex_models_manager::manager::{ModelsManager, RefreshStrategy};
use codex_protocol::{
    approvals::{ElicitationRequest, ElicitationRequestEvent},
    config_types::TrustLevel,
    dynamic_tools::{DynamicToolCallOutputContentItem, DynamicToolCallRequest},
    error::CodexErr,
    mcp::CallToolResult,
    models::{PermissionProfile, ResponseItem, WebSearchAction},
    openai_models::{ModelPreset, ReasoningEffort},
    parse_command::ParsedCommand,
    plan_tool::{PlanItemArg, StepStatus, UpdatePlanArgs},
    protocol::{
        AgentMessageContentDeltaEvent, AgentMessageEvent, AgentReasoningDeltaEvent,
        AgentReasoningEvent, AgentReasoningRawContentDeltaEvent, AgentReasoningRawContentEvent,
        AgentReasoningSectionBreakEvent, ApplyPatchApprovalRequestEvent, DeprecationNoticeEvent,
        DynamicToolCallResponseEvent, ElicitationAction, ErrorEvent, Event, EventMsg,
        ExecApprovalRequestEvent, ExecCommandBeginEvent, ExecCommandEndEvent,
        ExecCommandOutputDeltaEvent, ExecCommandStatus, ExitedReviewModeEvent, FileChange,
        ItemCompletedEvent, ItemStartedEvent, McpInvocation, McpStartupCompleteEvent,
        McpStartupUpdateEvent, McpToolCallBeginEvent, McpToolCallEndEvent, ModelRerouteEvent,
        NetworkApprovalContext, NetworkPolicyRuleAction, Op, PatchApplyBeginEvent,
        PatchApplyEndEvent, PatchApplyStatus, ReasoningContentDeltaEvent,
        ReasoningRawContentDeltaEvent, ReviewDecision, ReviewOutputEvent, ReviewRequest,
        ReviewTarget, RolloutItem, SandboxPolicy, StreamErrorEvent, TerminalInteractionEvent,
        TokenCountEvent, TurnAbortedEvent, TurnCompleteEvent, TurnStartedEvent, UserMessageEvent,
        ViewImageToolCallEvent, WarningEvent, WebSearchBeginEvent, WebSearchEndEvent,
    },
    request_permissions::{
        PermissionGrantScope, RequestPermissionsEvent, RequestPermissionsResponse,
    },
    request_user_input::{
        RequestUserInputAnswer, RequestUserInputEvent, RequestUserInputQuestion,
        RequestUserInputResponse,
    },
    user_input::UserInput,
};
use codex_shell_command::parse_command::parse_command;
use codex_utils_approval_presets::{ApprovalPreset, builtin_approval_presets};
use heck::ToTitleCase;
use itertools::Itertools;
use serde_json::json;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{ACP_CLIENT, prompt_args::parse_slash_name};

static APPROVAL_PRESETS: LazyLock<Vec<ApprovalPreset>> = LazyLock::new(builtin_approval_presets);
const INIT_COMMAND_PROMPT: &str = include_str!("./prompt_for_init_command.md");
/// Trait for abstracting over the `CodexThread` to make testing easier.
#[async_trait::async_trait]
pub trait CodexThreadImpl {
    async fn submit(&self, submission_id: String, op: Op) -> Result<String, CodexErr>;
    async fn next_event(&self) -> Result<Event, CodexErr>;
}

#[async_trait::async_trait]
impl CodexThreadImpl for CodexThread {
    async fn submit(&self, _submission_id: String, op: Op) -> Result<String, CodexErr> {
        self.submit(op).await
    }

    async fn next_event(&self) -> Result<Event, CodexErr> {
        self.next_event().await
    }
}

#[async_trait::async_trait]
pub trait ModelsManagerImpl {
    async fn get_model(&self, model_id: &Option<String>) -> String;
    async fn list_models(&self) -> Vec<ModelPreset>;
}

#[async_trait::async_trait]
impl ModelsManagerImpl for ModelsManager {
    async fn get_model(&self, model_id: &Option<String>) -> String {
        self.get_default_model(model_id, RefreshStrategy::OnlineIfUncached)
            .await
    }

    async fn list_models(&self) -> Vec<ModelPreset> {
        self.list_models(RefreshStrategy::OnlineIfUncached).await
    }
}

pub trait Auth {
    fn logout(&self) -> Result<bool, Error>;
}

impl Auth for Arc<AuthManager> {
    fn logout(&self) -> Result<bool, Error> {
        self.as_ref()
            .logout()
            .map_err(|e| Error::internal_error().data(e.to_string()))
    }
}

enum ThreadMessage {
    Load {
        response_tx: oneshot::Sender<Result<LoadSessionResponse, Error>>,
    },
    GetConfigOptions {
        response_tx: oneshot::Sender<Result<Vec<SessionConfigOption>, Error>>,
    },
    Prompt {
        request: PromptRequest,
        response_tx: oneshot::Sender<Result<oneshot::Receiver<Result<StopReason, Error>>, Error>>,
    },
    SetMode {
        mode: SessionModeId,
        response_tx: oneshot::Sender<Result<(), Error>>,
    },
    SetModel {
        model: ModelId,
        response_tx: oneshot::Sender<Result<(), Error>>,
    },
    SetConfigOption {
        config_id: SessionConfigId,
        value: SessionConfigOptionValue,
        response_tx: oneshot::Sender<Result<(), Error>>,
    },
    Cancel {
        response_tx: oneshot::Sender<Result<(), Error>>,
    },
    Shutdown {
        response_tx: oneshot::Sender<Result<(), Error>>,
    },
    ReplayHistory {
        history: Vec<RolloutItem>,
        response_tx: oneshot::Sender<Result<(), Error>>,
    },
    PermissionRequestResolved {
        submission_id: String,
        request_key: String,
        response: Result<RequestPermissionResponse, Error>,
    },
}

pub struct Thread {
    /// Direct handle to the underlying Codex thread for out-of-band shutdown.
    thread: Arc<dyn CodexThreadImpl>,
    /// A sender for interacting with the thread.
    message_tx: mpsc::UnboundedSender<ThreadMessage>,
    /// Keep the actor task alive for the lifetime of the thread wrapper.
    _handle: tokio::task::JoinHandle<()>,
}

impl Thread {
    pub fn new(
        session_id: SessionId,
        thread: Arc<dyn CodexThreadImpl>,
        auth: Arc<AuthManager>,
        models_manager: Arc<dyn ModelsManagerImpl>,
        client_capabilities: Arc<Mutex<ClientCapabilities>>,
        config: Config,
    ) -> Self {
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        let (resolution_tx, resolution_rx) = mpsc::unbounded_channel();

        let actor = ThreadActor::new(
            auth,
            SessionClient::new(session_id, client_capabilities),
            thread.clone(),
            models_manager,
            config,
            message_rx,
            resolution_tx,
            resolution_rx,
        );
        let handle = tokio::task::spawn_local(actor.spawn());

        Self {
            thread,
            message_tx,
            _handle: handle,
        }
    }

    pub async fn load(&self) -> Result<LoadSessionResponse, Error> {
        let (response_tx, response_rx) = oneshot::channel();

        let message = ThreadMessage::Load { response_tx };
        drop(self.message_tx.send(message));

        response_rx
            .await
            .map_err(|e| Error::internal_error().data(e.to_string()))?
    }

    pub async fn config_options(&self) -> Result<Vec<SessionConfigOption>, Error> {
        let (response_tx, response_rx) = oneshot::channel();

        let message = ThreadMessage::GetConfigOptions { response_tx };
        drop(self.message_tx.send(message));

        response_rx
            .await
            .map_err(|e| Error::internal_error().data(e.to_string()))?
    }

    pub async fn prompt(&self, request: PromptRequest) -> Result<StopReason, Error> {
        let (response_tx, response_rx) = oneshot::channel();

        let message = ThreadMessage::Prompt {
            request,
            response_tx,
        };
        drop(self.message_tx.send(message));

        response_rx
            .await
            .map_err(|e| Error::internal_error().data(e.to_string()))??
            .await
            .map_err(|e| Error::internal_error().data(e.to_string()))?
    }

    pub async fn set_mode(&self, mode: SessionModeId) -> Result<(), Error> {
        let (response_tx, response_rx) = oneshot::channel();

        let message = ThreadMessage::SetMode { mode, response_tx };
        drop(self.message_tx.send(message));

        response_rx
            .await
            .map_err(|e| Error::internal_error().data(e.to_string()))?
    }

    pub async fn set_model(&self, model: ModelId) -> Result<(), Error> {
        let (response_tx, response_rx) = oneshot::channel();

        let message = ThreadMessage::SetModel { model, response_tx };
        drop(self.message_tx.send(message));

        response_rx
            .await
            .map_err(|e| Error::internal_error().data(e.to_string()))?
    }

    pub async fn set_config_option(
        &self,
        config_id: SessionConfigId,
        value: SessionConfigOptionValue,
    ) -> Result<(), Error> {
        let (response_tx, response_rx) = oneshot::channel();

        let message = ThreadMessage::SetConfigOption {
            config_id,
            value,
            response_tx,
        };
        drop(self.message_tx.send(message));

        response_rx
            .await
            .map_err(|e| Error::internal_error().data(e.to_string()))?
    }

    pub async fn cancel(&self) -> Result<(), Error> {
        let (response_tx, response_rx) = oneshot::channel();

        let message = ThreadMessage::Cancel { response_tx };
        drop(self.message_tx.send(message));

        response_rx
            .await
            .map_err(|e| Error::internal_error().data(e.to_string()))?
    }

    pub async fn replay_history(&self, history: Vec<RolloutItem>) -> Result<(), Error> {
        let (response_tx, response_rx) = oneshot::channel();

        let message = ThreadMessage::ReplayHistory {
            history,
            response_tx,
        };
        drop(self.message_tx.send(message));

        response_rx
            .await
            .map_err(|e| Error::internal_error().data(e.to_string()))?
    }

    pub async fn shutdown(&self) -> Result<(), Error> {
        let (response_tx, response_rx) = oneshot::channel();
        let message = ThreadMessage::Shutdown { response_tx };

        if self.message_tx.send(message).is_err() {
            self.thread
                .submit("shutdown".to_string(), Op::Shutdown)
                .await
                .map_err(|e| Error::from(anyhow::anyhow!(e)))?;
        } else {
            response_rx
                .await
                .map_err(|e| Error::internal_error().data(e.to_string()))??;
        }
        // Let the actor drain the resulting turn-aborted/shutdown events so any in-flight
        // prompt callers observe a clean cancellation instead of a dropped response channel.
        Ok(())
    }
}

enum PendingPermissionRequest {
    Exec {
        approval_id: String,
        turn_id: String,
        option_map: HashMap<String, ReviewDecision>,
    },
    Patch {
        call_id: String,
        option_map: HashMap<String, ReviewDecision>,
    },
    RequestPermissions {
        call_id: String,
        permissions: PermissionProfile,
    },
}

struct PendingPermissionInteraction {
    request: PendingPermissionRequest,
    task: tokio::task::JoinHandle<()>,
}

#[derive(Clone)]
struct PendingUserInputRequest {
    response_id: String,
    call_id: String,
    questions: Vec<RequestUserInputQuestion>,
    tool_call_id: ToolCallId,
}

struct PreparedUserInputAnswer {
    response_id: String,
    call_id: String,
    questions: Vec<RequestUserInputQuestion>,
    response: RequestUserInputResponse,
    tool_call_id: ToolCallId,
}

fn exec_request_key(call_id: &str) -> String {
    format!("exec:{call_id}")
}

fn patch_request_key(call_id: &str) -> String {
    format!("patch:{call_id}")
}

fn permissions_request_key(call_id: &str) -> String {
    format!("permissions:{call_id}")
}

enum SubmissionState {
    /// User prompts, including slash commands like /init, /review, /compact, /undo.
    Prompt(Box<PromptState>),
}

impl SubmissionState {
    fn is_active(&self) -> bool {
        match self {
            Self::Prompt(state) => state.is_active(),
        }
    }

    async fn handle_event(&mut self, client: &SessionClient, event: EventMsg) {
        match self {
            Self::Prompt(state) => state.handle_event(client, event).await,
        }
    }

    async fn handle_permission_request_resolved(
        &mut self,
        client: &SessionClient,
        request_key: String,
        response: Result<RequestPermissionResponse, Error>,
    ) -> Result<(), Error> {
        match self {
            Self::Prompt(state) => {
                state
                    .handle_permission_request_resolved(client, request_key, response)
                    .await
            }
        }
    }

    fn abort_pending_interactions(&mut self) {
        let Self::Prompt(state) = self;
        state.abort_pending_interactions();
    }

    fn has_pending_user_input(&self) -> bool {
        match self {
            Self::Prompt(state) => state.has_pending_user_input(),
        }
    }

    fn prepare_user_input_answer(
        &self,
        prompt: &[ContentBlock],
    ) -> Result<Option<PreparedUserInputAnswer>, Error> {
        match self {
            Self::Prompt(state) => state.prepare_user_input_answer(prompt),
        }
    }

    async fn finalize_user_input_answer(
        &mut self,
        client: &SessionClient,
        prepared: PreparedUserInputAnswer,
        response_tx: oneshot::Sender<Result<StopReason, Error>>,
    ) {
        match self {
            Self::Prompt(state) => {
                state
                    .finalize_user_input_answer(client, prepared, response_tx)
                    .await
            }
        }
    }

    fn fail(&mut self, err: Error) {
        let Self::Prompt(state) = self;
        state.finish_err(err);
    }
}

struct ActiveCommand {
    tool_call_id: ToolCallId,
    title: String,
    terminal_output: bool,
    output: String,
    file_extension: Option<String>,
    background_terminal_waiting: bool,
}

struct PromptState {
    submission_id: String,
    active_commands: HashMap<String, ActiveCommand>,
    active_web_search: Option<String>,
    thread: Arc<dyn CodexThreadImpl>,
    runtime_actor_cli_path: Option<PathBuf>,
    resolution_tx: mpsc::UnboundedSender<ThreadMessage>,
    pending_permission_interactions: HashMap<String, PendingPermissionInteraction>,
    pending_user_input_request: Option<PendingUserInputRequest>,
    event_count: usize,
    stream_open: bool,
    response_txs: Vec<oneshot::Sender<Result<StopReason, Error>>>,
    seen_message_deltas: bool,
    seen_reasoning_deltas: bool,
    seen_reasoning_final: bool,
}

impl PromptState {
    fn new(
        submission_id: String,
        thread: Arc<dyn CodexThreadImpl>,
        resolution_tx: mpsc::UnboundedSender<ThreadMessage>,
        response_tx: oneshot::Sender<Result<StopReason, Error>>,
    ) -> Self {
        Self::new_with_runtime_actor_cli_path(
            submission_id,
            thread,
            resolution_tx,
            response_tx,
            resolve_runtime_actor_cli_path(),
        )
    }

    fn new_with_runtime_actor_cli_path(
        submission_id: String,
        thread: Arc<dyn CodexThreadImpl>,
        resolution_tx: mpsc::UnboundedSender<ThreadMessage>,
        response_tx: oneshot::Sender<Result<StopReason, Error>>,
        runtime_actor_cli_path: Option<PathBuf>,
    ) -> Self {
        Self {
            submission_id,
            active_commands: HashMap::new(),
            active_web_search: None,
            thread,
            runtime_actor_cli_path,
            resolution_tx,
            pending_permission_interactions: HashMap::new(),
            pending_user_input_request: None,
            event_count: 0,
            stream_open: true,
            response_txs: vec![response_tx],
            seen_message_deltas: false,
            seen_reasoning_deltas: false,
            seen_reasoning_final: false,
        }
    }

    fn new_detached(
        submission_id: String,
        thread: Arc<dyn CodexThreadImpl>,
        resolution_tx: mpsc::UnboundedSender<ThreadMessage>,
    ) -> Self {
        Self {
            submission_id,
            active_commands: HashMap::new(),
            active_web_search: None,
            thread,
            runtime_actor_cli_path: resolve_runtime_actor_cli_path(),
            resolution_tx,
            pending_permission_interactions: HashMap::new(),
            pending_user_input_request: None,
            event_count: 0,
            stream_open: true,
            response_txs: Vec::new(),
            seen_message_deltas: false,
            seen_reasoning_deltas: false,
            seen_reasoning_final: false,
        }
    }

    fn is_active(&self) -> bool {
        self.stream_open
            || self
                .response_txs
                .iter()
                .any(|response_tx| !response_tx.is_closed())
    }

    fn add_response_waiter(&mut self, response_tx: oneshot::Sender<Result<StopReason, Error>>) {
        self.response_txs.push(response_tx);
    }

    fn has_pending_user_input(&self) -> bool {
        self.pending_user_input_request.is_some()
    }

    fn finish_ok(&mut self, stop_reason: StopReason) {
        self.stream_open = false;
        for response_tx in self.response_txs.drain(..) {
            drop(response_tx.send(Ok(stop_reason)));
        }
    }

    fn finish_err(&mut self, err: Error) {
        self.stream_open = false;
        for response_tx in self.response_txs.drain(..) {
            drop(response_tx.send(Err(err.clone())));
        }
    }

    fn should_emit_final_reasoning(&mut self) -> bool {
        if self.seen_reasoning_deltas {
            self.seen_reasoning_deltas = false;
            self.seen_reasoning_final = true;
            return false;
        }
        if self.seen_reasoning_final {
            return false;
        }
        self.seen_reasoning_final = true;
        true
    }

    fn abort_pending_interactions(&mut self) {
        for (_, interaction) in self.pending_permission_interactions.drain() {
            interaction.task.abort();
        }
        self.pending_user_input_request = None;
    }

    fn spawn_permission_request(
        &mut self,
        client: &SessionClient,
        request_key: String,
        pending_request: PendingPermissionRequest,
        tool_call: ToolCallUpdate,
        options: Vec<PermissionOption>,
    ) {
        let client = client.clone();
        let resolution_tx = self.resolution_tx.clone();
        let submission_id = self.submission_id.clone();
        let resolved_request_key = request_key.clone();
        let handle = tokio::task::spawn_local(async move {
            let response = client.request_permission(tool_call, options).await;
            drop(
                resolution_tx.send(ThreadMessage::PermissionRequestResolved {
                    submission_id,
                    request_key: resolved_request_key,
                    response,
                }),
            );
        });

        if let Some(interaction) = self.pending_permission_interactions.insert(
            request_key,
            PendingPermissionInteraction {
                request: pending_request,
                task: handle,
            },
        ) {
            interaction.task.abort();
        }
    }

    async fn handle_permission_request_resolved(
        &mut self,
        _client: &SessionClient,
        request_key: String,
        response: Result<RequestPermissionResponse, Error>,
    ) -> Result<(), Error> {
        let Some(interaction) = self.pending_permission_interactions.remove(&request_key) else {
            warn!("Ignoring permission response for unknown request key: {request_key}");
            return Ok(());
        };
        let pending_request = interaction.request;
        let response = response?;

        match pending_request {
            PendingPermissionRequest::Exec {
                approval_id,
                turn_id,
                option_map,
            } => {
                let decision = match response.outcome {
                    RequestPermissionOutcome::Selected(SelectedPermissionOutcome {
                        option_id,
                        ..
                    }) => option_map
                        .get(option_id.0.as_ref())
                        .cloned()
                        .unwrap_or(ReviewDecision::Abort),
                    RequestPermissionOutcome::Cancelled | _ => ReviewDecision::Abort,
                };

                self.thread
                    .submit(
                        self.submission_id.clone(),
                        Op::ExecApproval {
                            id: approval_id,
                            turn_id: Some(turn_id),
                            decision,
                        },
                    )
                    .await
                    .map_err(|e| Error::from(anyhow::anyhow!(e)))?;
            }
            PendingPermissionRequest::Patch {
                call_id,
                option_map,
            } => {
                let decision = match response.outcome {
                    RequestPermissionOutcome::Selected(SelectedPermissionOutcome {
                        option_id,
                        ..
                    }) => option_map
                        .get(option_id.0.as_ref())
                        .cloned()
                        .unwrap_or(ReviewDecision::Abort),
                    RequestPermissionOutcome::Cancelled | _ => ReviewDecision::Abort,
                };

                self.thread
                    .submit(
                        self.submission_id.clone(),
                        Op::PatchApproval {
                            id: call_id,
                            decision,
                        },
                    )
                    .await
                    .map_err(|e| Error::from(anyhow::anyhow!(e)))?;
            }
            PendingPermissionRequest::RequestPermissions {
                call_id,
                permissions,
            } => {
                let response = match response.outcome {
                    RequestPermissionOutcome::Selected(SelectedPermissionOutcome {
                        option_id,
                        ..
                    }) => match option_id.0.as_ref() {
                        "approved-for-session" => RequestPermissionsResponse {
                            permissions: permissions.into(),
                            scope: PermissionGrantScope::Session,
                        },
                        "approved" => RequestPermissionsResponse {
                            permissions: permissions.into(),
                            scope: PermissionGrantScope::Turn,
                        },
                        _ => RequestPermissionsResponse {
                            permissions: PermissionProfile::default().into(),
                            scope: PermissionGrantScope::Turn,
                        },
                    },
                    RequestPermissionOutcome::Cancelled | _ => RequestPermissionsResponse {
                        permissions: PermissionProfile::default().into(),
                        scope: PermissionGrantScope::Turn,
                    },
                };

                self.thread
                    .submit(
                        self.submission_id.clone(),
                        Op::RequestPermissionsResponse {
                            id: call_id,
                            response,
                        },
                    )
                    .await
                    .map_err(|e| Error::from(anyhow::anyhow!(e)))?;
            }
        }

        Ok(())
    }

    #[expect(clippy::too_many_lines)]
    async fn handle_event(&mut self, client: &SessionClient, event: EventMsg) {
        self.event_count += 1;

        // Complete any previous web search before starting a new one
        match &event {
            EventMsg::Error(..)
            | EventMsg::StreamError(..)
            | EventMsg::WebSearchBegin(..)
            | EventMsg::UserMessage(..)
            | EventMsg::ExecApprovalRequest(..)
            | EventMsg::ExecCommandBegin(..)
            | EventMsg::ExecCommandOutputDelta(..)
            | EventMsg::ExecCommandEnd(..)
            | EventMsg::McpToolCallBegin(..)
            | EventMsg::McpToolCallEnd(..)
            | EventMsg::ApplyPatchApprovalRequest(..)
            | EventMsg::PatchApplyBegin(..)
            | EventMsg::PatchApplyEnd(..)
            | EventMsg::TurnStarted(..)
            | EventMsg::TurnComplete(..)
            | EventMsg::TurnDiff(..)
            | EventMsg::TurnAborted(..)
            | EventMsg::EnteredReviewMode(..)
            | EventMsg::ExitedReviewMode(..)
            | EventMsg::ShutdownComplete => {
                self.complete_web_search(client).await;
            }
            _ => {}
        }

        match event {
            EventMsg::TurnStarted(TurnStartedEvent {
                started_at: _,
                model_context_window,
                collaboration_mode_kind,
                turn_id,
            }) => {
                info!("Task started with context window of {turn_id} {model_context_window:?} {collaboration_mode_kind:?}");
            }
            EventMsg::TokenCount(TokenCountEvent { info, .. }) => {
                if let Some(info) = info
                    && let Some(size) = info.model_context_window {
                        let used = info.last_token_usage.tokens_in_context_window().max(0) as u64;
                        client
                            .send_notification(SessionUpdate::UsageUpdate(UsageUpdate::new(
                                used,
                                size as u64,
                            )))
                            .await;
                    }
            }
            EventMsg::ItemStarted(ItemStartedEvent { thread_id, turn_id, item }) => {
                info!("Item started with thread_id: {thread_id}, turn_id: {turn_id}, item: {item:?}");
            }
            EventMsg::UserMessage(UserMessageEvent {
                message,
                images: _,
                text_elements: _,
                local_images: _,
            }) => {
                info!("User message: {message:?}");
            }
            EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent {
                thread_id,
                turn_id,
                item_id,
                delta,
            }) => {
                info!("Agent message content delta received: thread_id: {thread_id}, turn_id: {turn_id}, item_id: {item_id}, delta: {delta:?}");
                self.seen_message_deltas = true;
                client.send_agent_text(delta).await;
            }
            EventMsg::ReasoningContentDelta(ReasoningContentDeltaEvent {
                thread_id,
                turn_id,
                item_id,
                delta,
                summary_index: index,
            })
            | EventMsg::ReasoningRawContentDelta(ReasoningRawContentDeltaEvent {
                thread_id,
                turn_id,
                item_id,
                delta,
                content_index: index,
            }) => {
                info!(
                    "Agent reasoning content delta received: thread_id: {thread_id}, turn_id: {turn_id}, item_id: {item_id}, index: {index}, delta: {delta:?}"
                );
                self.seen_reasoning_deltas = true;
                client.send_agent_thought(delta).await;
            }
            EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta })
            | EventMsg::AgentReasoningRawContentDelta(AgentReasoningRawContentDeltaEvent {
                delta,
            }) => {
                info!("Agent legacy reasoning delta received: {delta:?}");
                self.seen_reasoning_deltas = true;
                client.send_agent_thought(delta).await;
            }
            EventMsg::AgentReasoningSectionBreak(AgentReasoningSectionBreakEvent {
                item_id,
                summary_index,
            }) => {
                info!("Agent reasoning section break received:  item_id: {item_id}, index: {summary_index}");
                // Make sure the section heading actually get spacing
                self.seen_reasoning_deltas = true;
                client.send_agent_thought("\n\n").await;
            }
            EventMsg::AgentMessage(AgentMessageEvent { message , phase: _, .. }) => {
                info!("Agent message (non-delta) received: {message:?}");
                // We didn't receive this message via streaming
                if !std::mem::take(&mut self.seen_message_deltas) {
                    client.send_agent_text(message).await;
                }
            }
            EventMsg::AgentReasoning(AgentReasoningEvent { text }) => {
                info!("Agent reasoning (non-delta) received: {text:?}");
                if self.should_emit_final_reasoning() {
                    client.send_agent_thought(text).await;
                }
            }
            EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent { text }) => {
                info!("Agent raw reasoning (non-delta) received: {text:?}");
                if self.should_emit_final_reasoning() {
                    client.send_agent_thought(text).await;
                }
            }
            EventMsg::ThreadNameUpdated(event) => {
                info!("Thread name updated: {:?}", event.thread_name);
                if let Some(title) = event.thread_name {
                    client
                        .send_notification(SessionUpdate::SessionInfoUpdate(
                            SessionInfoUpdate::new().title(title),
                        ))
                        .await;
                }
            }
            EventMsg::PlanUpdate(UpdatePlanArgs { explanation, plan }) => {
                // Send this to the client via session/update notification
                info!("Agent plan updated. Explanation: {:?}", explanation);
                client.update_plan(plan).await;
            }
            EventMsg::WebSearchBegin(WebSearchBeginEvent { call_id }) => {
                info!("Web search started: call_id={}", call_id);
                // Create a ToolCall notification for the search beginning
                self.start_web_search(client, call_id).await;
            }
            EventMsg::WebSearchEnd(WebSearchEndEvent {
                call_id,
                query,
                action,
            }) => {
                info!("Web search query received: call_id={call_id}, query={query}");
                // Send update that the search is in progress with the query
                // (WebSearchEnd just means we have the query, not that results are ready)
                self.update_web_search_query(client, call_id, query, action)
                    .await;
                // The actual search results will come through AgentMessage events
                // We mark as completed when a new tool call begins
            }
            EventMsg::ExecApprovalRequest(event) => {
                info!(
                    "Command execution started: call_id={}, command={:?}",
                    event.call_id, event.command
                );
                if let Err(err) = self.exec_approval(client, event).await {
                    self.finish_err(err);
                }
            }
            EventMsg::ExecCommandBegin(event) => {
                info!(
                    "Command execution started: call_id={}, command={:?}",
                    event.call_id, event.command
                );
                self.exec_command_begin(client, event).await;
            }
            EventMsg::ExecCommandOutputDelta(delta_event) => {
                self.exec_command_output_delta(client, delta_event).await;
            }
            EventMsg::ExecCommandEnd(end_event) => {
                info!(
                    "Command execution ended: call_id={}, exit_code={}",
                    end_event.call_id, end_event.exit_code
                );
                self.exec_command_end(client, end_event).await;
            }
            EventMsg::TerminalInteraction(event) => {
                info!(
                    "Terminal interaction: call_id={}, process_id={}, stdin={}",
                    event.call_id, event.process_id, event.stdin
                );
                self.terminal_interaction(client, event).await;
            }
            EventMsg::DynamicToolCallRequest(DynamicToolCallRequest { call_id, turn_id, tool, arguments }) => {
                info!("Dynamic tool call request: call_id={call_id}, turn_id={turn_id}, tool={tool}");
                self.start_dynamic_tool_call(client, call_id, tool, arguments).await;
            }
            EventMsg::DynamicToolCallResponse(event) => {
                info!(
                    "Dynamic tool call response: call_id={}, turn_id={}, tool={}",
                    event.call_id, event.turn_id, event.tool
                );
                self.end_dynamic_tool_call(client, event).await;
            }
            EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                call_id,
                invocation,
            }) => {
                info!(
                    "MCP tool call begin: call_id={call_id}, invocation={} {}",
                    invocation.server, invocation.tool
                );
                self.start_mcp_tool_call(client, call_id, invocation).await;
            }
            EventMsg::McpToolCallEnd(McpToolCallEndEvent {
                call_id,
                invocation,
                duration,
                result,
            }) => {
                info!(
                    "MCP tool call ended: call_id={call_id}, invocation={} {}, duration={duration:?}",
                    invocation.server, invocation.tool
                );
                self.end_mcp_tool_call(client, call_id, result).await;
            }
            EventMsg::ApplyPatchApprovalRequest(event) => {
                info!(
                    "Apply patch approval request: call_id={}, reason={:?}",
                    event.call_id, event.reason
                );
                if let Err(err) = self.patch_approval(client, event).await {
                    self.finish_err(err);
                }
            }
            EventMsg::PatchApplyBegin(event) => {
                info!(
                    "Patch apply begin: call_id={}, auto_approved={}",
                    event.call_id, event.auto_approved
                );
                self.start_patch_apply(client, event).await;
            }
            EventMsg::PatchApplyEnd(event) => {
                info!(
                    "Patch apply end: call_id={}, success={}",
                    event.call_id, event.success
                );
                self.end_patch_apply(client, event).await;
            }
            EventMsg::ItemCompleted(ItemCompletedEvent {
                thread_id,
                turn_id,
                item,
            }) => {
                info!("Item completed: thread_id={}, turn_id={}, item={:?}", thread_id, turn_id, item);
            }
            EventMsg::TurnComplete(TurnCompleteEvent {
                last_agent_message,
                turn_id,
                completed_at: _,
                duration_ms: _,
            }) => {
                info!(
                    "Task {turn_id} completed successfully after {} events. Last agent message: {last_agent_message:?}",
                    self.event_count
                );
                self.abort_pending_interactions();
                self.finish_ok(StopReason::EndTurn);
            }
            EventMsg::UndoStarted(event) => {
                client
                    .send_agent_text(
                        event
                            .message
                            .unwrap_or_else(|| "Undo in progress...".to_string()),
                    )
                    .await;
            }
            EventMsg::UndoCompleted(event) => {
                let fallback = if event.success {
                    "Undo completed.".to_string()
                } else {
                    "Undo failed.".to_string()
                };
                client.send_agent_text(event.message.unwrap_or(fallback)).await;
            }
            EventMsg::StreamError(StreamErrorEvent {
                message,
                codex_error_info,
                additional_details,
            }) => {
                warn!(
                    "Handled error during turn: {message} {codex_error_info:?} {additional_details:?}"
                );
            }
            EventMsg::Error(ErrorEvent {
                message,
                codex_error_info,
            }) => {
                error!("Unhandled error during turn: {message} {codex_error_info:?}");
                self.abort_pending_interactions();
                self.finish_err(Error::internal_error().data(
                    json!({ "message": message, "codex_error_info": codex_error_info }),
                ));
            }
            EventMsg::TurnAborted(TurnAbortedEvent {
                reason,
                turn_id,
                completed_at: _,
                duration_ms: _,
            }) => {
                info!("Turn {turn_id:?} aborted: {reason:?}");
                self.abort_pending_interactions();
                self.finish_ok(StopReason::Cancelled);
            }
            EventMsg::ShutdownComplete => {
                info!("Agent shutting down");
                self.abort_pending_interactions();
                self.finish_ok(StopReason::Cancelled);
            }
            EventMsg::ViewImageToolCall(ViewImageToolCallEvent { call_id, path }) => {
                info!("ViewImageToolCallEvent received");
                let display_path = path.display().to_string();
                client
                    .send_notification(
                        SessionUpdate::ToolCall(
                            ToolCall::new(call_id, format!("View Image {display_path}"))
                                .kind(ToolKind::Read).status(ToolCallStatus::Completed)
                                .content(vec![ToolCallContent::Content(Content::new(ContentBlock::ResourceLink(ResourceLink::new(display_path.clone(), display_path.clone())
                            )
                        )
                    )]).locations(vec![ToolCallLocation::new(path)])))
                    .await;
            }
            EventMsg::EnteredReviewMode(review_request) => {
                info!("Review begin: request={review_request:?}");
            }
            EventMsg::ExitedReviewMode(event) => {
                info!("Review end: output={event:?}");
                if let Err(err) = self.review_mode_exit(client, event).await {
                    self.finish_err(err);
                }
            }
            EventMsg::Warning(WarningEvent { message }) => {
                warn!("Warning: {message}");
                // Forward warnings to the client as agent messages so users see
                // informational notices (e.g., the post-compact advisory message).
                client.send_agent_text(message).await;
            }
            EventMsg::McpStartupUpdate(McpStartupUpdateEvent { server, status }) => {
                info!("MCP startup update: server={server}, status={status:?}");
            }
            EventMsg::McpStartupComplete(McpStartupCompleteEvent {
                ready,
                failed,
                cancelled,
            }) => {
                info!(
                    "MCP startup complete: ready={ready:?}, failed={failed:?}, cancelled={cancelled:?}"
                );
            }
            EventMsg::ElicitationRequest(event) => {
                info!("Elicitation request: server={}, id={:?}", event.server_name, event.id);
                if let Err(err) = self.mcp_elicitation(client, event).await {
                    self.finish_err(err);
                }
            }
            EventMsg::ModelReroute(ModelRerouteEvent { from_model, to_model, reason }) => {
                info!("Model reroute: from={from_model}, to={to_model}, reason={reason:?}");
                client.send_agent_text(render_model_reroute_message(
                    &from_model,
                    &to_model,
                    &reason,
                ))
                .await;
            }
            EventMsg::DeprecationNotice(DeprecationNoticeEvent { summary, details }) => {
                info!("Deprecation notice: summary={summary}, details={details:?}");
                client
                    .send_agent_text(render_deprecation_notice_message(&summary, details.as_deref()))
                    .await;
            }

            EventMsg::ContextCompacted(..) => {
                info!("Context compacted");
                client.send_agent_text("Context compacted\n".to_string()).await;
            }
            EventMsg::RequestPermissions(event) => {
                info!("Request permissions: {} {}", event.call_id, event.turn_id);
                if let Err(err) = self.request_permissions(client, event).await {
                    self.finish_err(err);
                }
            }
            EventMsg::RequestUserInput(event) => {
                info!("Request user input: {} {}", event.call_id, event.turn_id);
                if let Err(err) = self.request_user_input(client, event).await {
                    self.finish_err(err);
                }
            }

            // Ignore these events
            EventMsg::ImageGenerationBegin(..)
            | EventMsg::ImageGenerationEnd(..)
            | EventMsg::ThreadRolledBack(..)
            | EventMsg::HookStarted(..)
            | EventMsg::HookCompleted(..)
            // we already have a way to diff the turn, so ignore
            | EventMsg::TurnDiff(..)
            // Revisit when we can emit status updates
            | EventMsg::BackgroundEvent(..)
            | EventMsg::SkillsUpdateAvailable
            // Old events
            | EventMsg::AgentMessageDelta(..)
            | EventMsg::RawResponseItem(..)
            | EventMsg::SessionConfigured(..)
            // TODO: Subagent UI?
            | EventMsg::CollabAgentSpawnBegin(..)
            | EventMsg::CollabAgentSpawnEnd(..)
            | EventMsg::CollabAgentInteractionBegin(..)
            | EventMsg::CollabAgentInteractionEnd(..)
            | EventMsg::RealtimeConversationStarted(..)
            | EventMsg::RealtimeConversationRealtime(..)
            | EventMsg::RealtimeConversationSdp(..)
            | EventMsg::RealtimeConversationClosed(..)
            | EventMsg::RealtimeConversationListVoicesResponse(..)
            | EventMsg::CollabWaitingBegin(..)
            | EventMsg::CollabWaitingEnd(..)
            | EventMsg::CollabResumeBegin(..)
            | EventMsg::CollabResumeEnd(..)
            | EventMsg::CollabCloseBegin(..)
            | EventMsg::CollabCloseEnd(..)
            | EventMsg::PlanDelta(..) => {}
            EventMsg::GuardianAssessment(..) => {}
            e @ (EventMsg::McpListToolsResponse(..)
            | EventMsg::ListSkillsResponse(..)
            // Used for returning a single history entry
            | EventMsg::GetHistoryEntryResponse(..)) => {
                warn!("Unexpected event: {:?}", e);
            }
        }
    }

    async fn mcp_elicitation(
        &mut self,
        _client: &SessionClient,
        event: ElicitationRequestEvent,
    ) -> Result<(), Error> {
        let ElicitationRequestEvent {
            server_name,
            id,
            request,
            turn_id: _,
        } = event;
        let request_kind = match &request {
            ElicitationRequest::Form { .. } => "form",
            ElicitationRequest::Url { .. } => "url",
        };

        info!(
            "Auto-declining unsupported MCP elicitation: server={}, id={:?}, kind={request_kind}",
            server_name, id
        );

        self.thread
            .submit(
                self.submission_id.clone(),
                Op::ResolveElicitation {
                    server_name,
                    request_id: id,
                    decision: ElicitationAction::Decline,
                    content: None,
                    meta: None,
                },
            )
            .await
            .map_err(|e| Error::from(anyhow::anyhow!(e)))?;

        Ok(())
    }

    async fn review_mode_exit(
        &self,
        client: &SessionClient,
        event: ExitedReviewModeEvent,
    ) -> Result<(), Error> {
        let ExitedReviewModeEvent { review_output } = event;
        let Some(ReviewOutputEvent {
            findings,
            overall_correctness: _,
            overall_explanation,
            overall_confidence_score: _,
        }) = review_output
        else {
            return Ok(());
        };

        let text = if findings.is_empty() {
            let explanation = overall_explanation.trim();
            if explanation.is_empty() {
                "Reviewer failed to output a response"
            } else {
                explanation
            }
            .to_string()
        } else {
            format_review_findings_block(&findings, None)
        };

        client.send_agent_text(&text).await;
        Ok(())
    }

    async fn patch_approval(
        &mut self,
        client: &SessionClient,
        event: ApplyPatchApprovalRequestEvent,
    ) -> Result<(), Error> {
        let raw_input = serde_json::json!(&event);
        let ApplyPatchApprovalRequestEvent {
            call_id,
            changes,
            reason,
            // grant_root doesn't seem to be set anywhere on the codex side
            grant_root: _,
            turn_id: _,
        } = event;
        let (title, locations, content) = extract_tool_call_content_from_changes(changes);
        let request_key = patch_request_key(&call_id);
        let options = vec![
            PermissionOption::new("approved", "Yes", PermissionOptionKind::AllowOnce),
            PermissionOption::new(
                "abort",
                "No, provide feedback",
                PermissionOptionKind::RejectOnce,
            ),
        ];
        self.spawn_permission_request(
            client,
            request_key,
            PendingPermissionRequest::Patch {
                call_id: call_id.clone(),
                option_map: HashMap::from([
                    ("approved".to_string(), ReviewDecision::Approved),
                    ("abort".to_string(), ReviewDecision::Abort),
                ]),
            },
            ToolCallUpdate::new(
                call_id,
                ToolCallUpdateFields::new()
                    .kind(ToolKind::Edit)
                    .status(ToolCallStatus::Pending)
                    .title(title)
                    .locations(locations)
                    .content(content.chain(reason.map(|r| r.into())).collect::<Vec<_>>())
                    .raw_input(raw_input),
            ),
            options,
        );
        Ok(())
    }

    async fn start_patch_apply(&self, client: &SessionClient, event: PatchApplyBeginEvent) {
        let raw_input = serde_json::json!(&event);
        let PatchApplyBeginEvent {
            call_id,
            auto_approved: _,
            changes,
            turn_id: _,
        } = event;

        let (title, locations, content) = extract_tool_call_content_from_changes(changes);

        client
            .send_tool_call(
                ToolCall::new(call_id, title)
                    .kind(ToolKind::Edit)
                    .status(ToolCallStatus::InProgress)
                    .locations(locations)
                    .content(content.collect())
                    .raw_input(raw_input),
            )
            .await;
    }

    async fn end_patch_apply(&self, client: &SessionClient, event: PatchApplyEndEvent) {
        let raw_output = serde_json::json!(&event);
        let PatchApplyEndEvent {
            call_id,
            stdout: _,
            stderr: _,
            success,
            changes,
            turn_id: _,
            status,
        } = event;

        let (title, locations, content) = if !changes.is_empty() {
            let (title, locations, content) = extract_tool_call_content_from_changes(changes);
            (Some(title), Some(locations), Some(content.collect()))
        } else {
            (None, None, None)
        };

        let status = match status {
            PatchApplyStatus::Completed => ToolCallStatus::Completed,
            _ if success => ToolCallStatus::Completed,
            PatchApplyStatus::Failed | PatchApplyStatus::Declined => ToolCallStatus::Failed,
        };

        client
            .send_tool_call_update(ToolCallUpdate::new(
                call_id,
                ToolCallUpdateFields::new()
                    .status(status)
                    .raw_output(raw_output)
                    .title(title)
                    .locations(locations)
                    .content(content),
            ))
            .await;
    }

    async fn start_dynamic_tool_call(
        &self,
        client: &SessionClient,
        call_id: String,
        tool: String,
        arguments: serde_json::Value,
    ) {
        client
            .send_tool_call(
                ToolCall::new(call_id, format!("Tool: {tool}"))
                    .status(ToolCallStatus::InProgress)
                    .raw_input(serde_json::json!(&arguments)),
            )
            .await;
    }

    async fn start_mcp_tool_call(
        &self,
        client: &SessionClient,
        call_id: String,
        invocation: McpInvocation,
    ) {
        let title = format!("Tool: {}/{}", invocation.server, invocation.tool);
        client
            .send_tool_call(
                ToolCall::new(call_id, title)
                    .status(ToolCallStatus::InProgress)
                    .raw_input(serde_json::json!(&invocation)),
            )
            .await;
    }

    async fn end_dynamic_tool_call(
        &self,
        client: &SessionClient,
        event: DynamicToolCallResponseEvent,
    ) {
        let raw_output = serde_json::json!(event);
        let DynamicToolCallResponseEvent {
            call_id,
            turn_id: _,
            tool: _,
            arguments: _,
            content_items,
            success,
            error,
            duration: _,
        } = event;

        client
            .send_tool_call_update(ToolCallUpdate::new(
                call_id,
                ToolCallUpdateFields::new()
                    .status(if success {
                        ToolCallStatus::Completed
                    } else {
                        ToolCallStatus::Failed
                    })
                    .raw_output(raw_output)
                    .content(
                        content_items
                            .into_iter()
                            .map(|item| match item {
                                DynamicToolCallOutputContentItem::InputText { text } => {
                                    ToolCallContent::Content(Content::new(text))
                                }
                                DynamicToolCallOutputContentItem::InputImage { image_url } => {
                                    ToolCallContent::Content(Content::new(
                                        ContentBlock::ResourceLink(ResourceLink::new(
                                            image_url.clone(),
                                            image_url,
                                        )),
                                    ))
                                }
                            })
                            .chain(error.map(|e| ToolCallContent::Content(Content::new(e))))
                            .collect::<Vec<_>>(),
                    ),
            ))
            .await;
    }

    async fn end_mcp_tool_call(
        &self,
        client: &SessionClient,
        call_id: String,
        result: Result<CallToolResult, String>,
    ) {
        let is_error = match result.as_ref() {
            Ok(result) => result.is_error.unwrap_or_default(),
            Err(_) => true,
        };
        let raw_output = match result.as_ref() {
            Ok(result) => serde_json::json!(result),
            Err(err) => serde_json::json!(err),
        };

        client
            .send_tool_call_update(ToolCallUpdate::new(
                call_id,
                ToolCallUpdateFields::new()
                    .status(if is_error {
                        ToolCallStatus::Failed
                    } else {
                        ToolCallStatus::Completed
                    })
                    .raw_output(raw_output)
                    .content(result.ok().filter(|result| !result.content.is_empty()).map(
                        |result| {
                            result
                                .content
                                .into_iter()
                                .filter_map(|content| {
                                    serde_json::from_value::<ContentBlock>(content).ok()
                                })
                                .map(|content| ToolCallContent::Content(Content::new(content)))
                                .collect()
                        },
                    )),
            ))
            .await;
    }

    async fn exec_approval(
        &mut self,
        client: &SessionClient,
        event: ExecApprovalRequestEvent,
    ) -> Result<(), Error> {
        let available_decisions = event.effective_available_decisions();
        let raw_input = serde_json::json!(&event);
        let ExecApprovalRequestEvent {
            call_id,
            command,
            turn_id,
            cwd,
            reason,
            parsed_cmd,
            proposed_execpolicy_amendment,
            approval_id,
            network_approval_context,
            additional_permissions,
            available_decisions: _,
            proposed_network_policy_amendments,
        } = event;

        // Create a new tool call for the command execution
        let tool_call_id = ToolCallId::new(call_id.clone());
        let ParseCommandToolCall {
            title,
            terminal_output,
            file_extension,
            locations,
            kind,
        } = parse_command_tool_call(parsed_cmd, &cwd);
        self.active_commands.insert(
            call_id.clone(),
            ActiveCommand {
                title: title.clone(),
                terminal_output,
                tool_call_id: tool_call_id.clone(),
                output: String::new(),
                file_extension,
                background_terminal_waiting: false,
            },
        );

        let resolved_approval_id = approval_id.unwrap_or(call_id.clone());
        if let Some(decision) = auto_approve_runtime_actor_cli_decision(
            &command,
            &available_decisions,
            self.runtime_actor_cli_path.as_deref(),
        ) {
            info!(
                "Auto-approving runtime actor CLI command: call_id={}, command={:?}",
                call_id, command
            );
            self.thread
                .submit(
                    self.submission_id.clone(),
                    Op::ExecApproval {
                        id: resolved_approval_id,
                        turn_id: Some(turn_id),
                        decision,
                    },
                )
                .await
                .map_err(|e| Error::from(anyhow::anyhow!(e)))?;
            return Ok(());
        }

        let mut content = vec![];

        if let Some(reason) = reason {
            content.push(reason);
        }
        if let Some(amendment) = proposed_execpolicy_amendment.as_ref() {
            content.push(format!(
                "Proposed Amendment: {}",
                amendment.command().join("\n")
            ));
        }
        if let Some(policy) = network_approval_context.as_ref() {
            let NetworkApprovalContext { host, protocol } = policy;
            content.push(format!("Network Approval Context: {:?} {}", protocol, host));
        }
        if let Some(permissions) = additional_permissions.as_ref() {
            content.push(format!(
                "Additional Permissions: {}",
                serde_json::to_string_pretty(&permissions)?
            ));
        }
        content.push(format!(
            "Available Decisions: {}",
            available_decisions.iter().map(|d| d.to_string()).join("\n")
        ));
        if let Some(amendments) = proposed_network_policy_amendments.as_ref() {
            content.push(format!(
                "Proposed Network Policy Amendments: {}",
                amendments
                    .iter()
                    .map(|amendment| format!("{:?} {:?}", amendment.action, amendment.host))
                    .join("\n")
            ));
        }

        let content = if content.is_empty() {
            None
        } else {
            Some(vec![content.join("\n").into()])
        };
        let permission_options = build_exec_permission_options(
            &available_decisions,
            network_approval_context.as_ref(),
            additional_permissions.as_ref(),
        );

        self.spawn_permission_request(
            client,
            exec_request_key(&call_id),
            PendingPermissionRequest::Exec {
                approval_id: resolved_approval_id,
                turn_id,
                option_map: permission_options
                    .iter()
                    .map(|option| (option.option_id.to_string(), option.decision.clone()))
                    .collect(),
            },
            ToolCallUpdate::new(
                tool_call_id,
                ToolCallUpdateFields::new()
                    .kind(kind)
                    .status(ToolCallStatus::Pending)
                    .title(title)
                    .raw_input(raw_input)
                    .content(content)
                    .locations(if locations.is_empty() {
                        None
                    } else {
                        Some(locations)
                    }),
            ),
            permission_options
                .into_iter()
                .map(|option| option.permission_option)
                .collect(),
        );

        Ok(())
    }

    async fn exec_command_begin(&mut self, client: &SessionClient, event: ExecCommandBeginEvent) {
        let raw_input = serde_json::json!(&event);
        let ExecCommandBeginEvent {
            turn_id: _,
            source: _,
            interaction_input: _,
            call_id,
            command: _,
            cwd,
            parsed_cmd,
            process_id: _,
        } = event;
        // Create a new tool call for the command execution
        let tool_call_id = ToolCallId::new(call_id.clone());
        let ParseCommandToolCall {
            title,
            file_extension,
            locations,
            terminal_output,
            kind,
        } = parse_command_tool_call(parsed_cmd, &cwd);

        let active_command = ActiveCommand {
            tool_call_id: tool_call_id.clone(),
            title: title.clone(),
            output: String::new(),
            file_extension,
            terminal_output,
            background_terminal_waiting: false,
        };
        let (content, meta) = if client.supports_terminal_output(&active_command) {
            let content = vec![ToolCallContent::Terminal(Terminal::new(call_id.clone()))];
            let meta = Some(Meta::from_iter([(
                "terminal_info".to_owned(),
                serde_json::json!({
                    "terminal_id": call_id,
                    "cwd": cwd
                }),
            )]));
            (content, meta)
        } else {
            (vec![], None)
        };

        self.active_commands.insert(call_id.clone(), active_command);

        client
            .send_tool_call(
                ToolCall::new(tool_call_id, title)
                    .kind(kind)
                    .status(ToolCallStatus::InProgress)
                    .locations(locations)
                    .raw_input(raw_input)
                    .content(content)
                    .meta(meta),
            )
            .await;
    }

    async fn exec_command_output_delta(
        &mut self,
        client: &SessionClient,
        event: ExecCommandOutputDeltaEvent,
    ) {
        let ExecCommandOutputDeltaEvent {
            call_id,
            chunk,
            stream: _,
        } = event;
        // Stream output bytes to the display-only terminal via ToolCallUpdate meta.
        if let Some(active_command) = self.active_commands.get_mut(&call_id) {
            let data_str = String::from_utf8_lossy(&chunk).to_string();
            if active_command.background_terminal_waiting {
                active_command.background_terminal_waiting = false;
                client
                    .send_tool_call_update(
                        ToolCallUpdate::new(
                            active_command.tool_call_id.clone(),
                            ToolCallUpdateFields::new(),
                        )
                        .meta(background_terminal_activity_meta(
                            "waited",
                            &active_command.title,
                        )),
                    )
                    .await;
            }

            let update = if client.supports_terminal_output(active_command) {
                ToolCallUpdate::new(
                    active_command.tool_call_id.clone(),
                    ToolCallUpdateFields::new(),
                )
                .meta(Meta::from_iter([(
                    "terminal_output".to_owned(),
                    serde_json::json!({
                        "terminal_id": call_id,
                        "data": data_str
                    }),
                )]))
            } else {
                active_command.output.push_str(&data_str);
                let content = match active_command.file_extension.as_deref() {
                    Some("md") => active_command.output.clone(),
                    Some(ext) => format!(
                        "```{ext}\n{}\n```\n",
                        active_command.output.trim_end_matches('\n')
                    ),
                    None => format!(
                        "```sh\n{}\n```\n",
                        active_command.output.trim_end_matches('\n')
                    ),
                };
                ToolCallUpdate::new(
                    active_command.tool_call_id.clone(),
                    ToolCallUpdateFields::new().content(vec![content.into()]),
                )
            };

            client.send_tool_call_update(update).await;
        }
    }

    async fn exec_command_end(&mut self, client: &SessionClient, event: ExecCommandEndEvent) {
        let raw_output = serde_json::json!(&event);
        let ExecCommandEndEvent {
            turn_id: _,
            command: _,
            cwd: _,
            parsed_cmd: _,
            source: _,
            interaction_input: _,
            call_id,
            exit_code,
            stdout: _,
            stderr: _,
            aggregated_output: _,
            duration: _,
            formatted_output: _,
            process_id: _,
            status,
        } = event;
        if let Some(active_command) = self.active_commands.remove(&call_id) {
            let is_success = exit_code == 0;

            let status = match status {
                ExecCommandStatus::Completed => ToolCallStatus::Completed,
                _ if is_success => ToolCallStatus::Completed,
                ExecCommandStatus::Failed | ExecCommandStatus::Declined => ToolCallStatus::Failed,
            };

            let meta = match (
                client.supports_terminal_output(&active_command),
                active_command.background_terminal_waiting,
            ) {
                (true, true) => Some(Meta::from_iter([
                    (
                        "terminal_exit".into(),
                        serde_json::json!({
                            "terminal_id": call_id,
                            "exit_code": exit_code,
                            "signal": null
                        }),
                    ),
                    (
                        "terminal_activity".to_owned(),
                        serde_json::json!({
                            "kind": "waited",
                            "command": &active_command.title,
                        }),
                    ),
                ])),
                (true, false) => Some(Meta::from_iter([(
                    "terminal_exit".into(),
                    serde_json::json!({
                        "terminal_id": call_id,
                        "exit_code": exit_code,
                        "signal": null
                    }),
                )])),
                (false, true) => Some(background_terminal_activity_meta(
                    "waited",
                    &active_command.title,
                )),
                (false, false) => None,
            };

            client
                .send_tool_call_update(
                    ToolCallUpdate::new(
                        active_command.tool_call_id.clone(),
                        ToolCallUpdateFields::new()
                            .status(status)
                            .raw_output(raw_output),
                    )
                    .meta(meta),
                )
                .await;
        }
    }

    async fn terminal_interaction(
        &mut self,
        client: &SessionClient,
        event: TerminalInteractionEvent,
    ) {
        let TerminalInteractionEvent {
            call_id,
            process_id: _,
            stdin,
        } = event;

        if let Some(active_command) = self.active_commands.get_mut(&call_id) {
            if stdin.is_empty() {
                if active_command.background_terminal_waiting {
                    return;
                }
                active_command.background_terminal_waiting = true;
                client
                    .send_tool_call_update(
                        ToolCallUpdate::new(
                            active_command.tool_call_id.clone(),
                            ToolCallUpdateFields::new(),
                        )
                        .meta(background_terminal_activity_meta(
                            "waiting",
                            &active_command.title,
                        )),
                    )
                    .await;
                return;
            }

            if active_command.background_terminal_waiting {
                active_command.background_terminal_waiting = false;
                client
                    .send_tool_call_update(
                        ToolCallUpdate::new(
                            active_command.tool_call_id.clone(),
                            ToolCallUpdateFields::new(),
                        )
                        .meta(background_terminal_activity_meta(
                            "waited",
                            &active_command.title,
                        )),
                    )
                    .await;
            }

            let stdin = format!("\n{stdin}\n");
            let update = if client.supports_terminal_output(active_command) {
                ToolCallUpdate::new(
                    active_command.tool_call_id.clone(),
                    ToolCallUpdateFields::new(),
                )
                .meta(Meta::from_iter([
                    (
                        "terminal_activity".to_owned(),
                        serde_json::json!({
                            "kind": "interacted",
                            "command": &active_command.title,
                        }),
                    ),
                    (
                        "terminal_output".to_owned(),
                        serde_json::json!({
                            "terminal_id": call_id,
                            "data": stdin
                        }),
                    ),
                ]))
            } else {
                active_command.output.push_str(&stdin);
                let content = match active_command.file_extension.as_deref() {
                    Some("md") => active_command.output.clone(),
                    Some(ext) => format!(
                        "```{ext}\n{}\n```\n",
                        active_command.output.trim_end_matches('\n')
                    ),
                    None => format!(
                        "```sh\n{}\n```\n",
                        active_command.output.trim_end_matches('\n')
                    ),
                };
                ToolCallUpdate::new(
                    active_command.tool_call_id.clone(),
                    ToolCallUpdateFields::new().content(vec![content.into()]),
                )
                .meta(background_terminal_activity_meta(
                    "interacted",
                    &active_command.title,
                ))
            };

            client.send_tool_call_update(update).await;
        }
    }

    async fn start_web_search(&mut self, client: &SessionClient, call_id: String) {
        self.active_web_search = Some(call_id.clone());
        client
            .send_tool_call(ToolCall::new(call_id, "Searching the Web").kind(ToolKind::Fetch))
            .await;
    }

    async fn update_web_search_query(
        &self,
        client: &SessionClient,
        call_id: String,
        query: String,
        action: WebSearchAction,
    ) {
        let title = match &action {
            WebSearchAction::Search { query, queries } => queries
                .as_ref()
                .map(|q| format!("Searching for: {}", q.join(", ")))
                .or_else(|| query.as_ref().map(|q| format!("Searching for: {q}")))
                .unwrap_or_else(|| "Web search".to_string()),
            WebSearchAction::OpenPage { url } => url
                .as_ref()
                .map(|u| format!("Opening: {u}"))
                .unwrap_or_else(|| "Open page".to_string()),
            WebSearchAction::FindInPage { pattern, url } => match (pattern, url) {
                (Some(p), Some(u)) => format!("Finding: {p} in {u}"),
                (Some(p), None) => format!("Finding: {p}"),
                (None, Some(u)) => format!("Find in page: {u}"),
                (None, None) => "Find in page".to_string(),
            },
            WebSearchAction::Other => "Web search".to_string(),
        };

        client
            .send_tool_call_update(ToolCallUpdate::new(
                call_id,
                ToolCallUpdateFields::new()
                    .status(ToolCallStatus::InProgress)
                    .title(title)
                    .raw_input(serde_json::json!({
                        "query": query,
                        "action": action
                    })),
            ))
            .await;
    }

    async fn complete_web_search(&mut self, client: &SessionClient) {
        if let Some(call_id) = self.active_web_search.take() {
            client
                .send_tool_call_update(ToolCallUpdate::new(
                    call_id,
                    ToolCallUpdateFields::new().status(ToolCallStatus::Completed),
                ))
                .await;
        }
    }

    async fn request_permissions(
        &mut self,
        client: &SessionClient,
        event: RequestPermissionsEvent,
    ) -> Result<(), Error> {
        let raw_input = serde_json::json!(&event);
        let RequestPermissionsEvent {
            call_id,
            turn_id: _,
            reason,
            permissions,
        } = event;

        // Create a new tool call for the command execution
        let tool_call_id = ToolCallId::new(call_id.clone());

        let mut content = vec![];

        if let Some(reason) = reason.as_ref() {
            content.push(reason.clone());
        }
        if let Some(file_system) = permissions.file_system.as_ref() {
            if let Some(read) = file_system.read.as_ref() {
                content.push(format!(
                    "File System Read Access: {}",
                    read.iter().map(|p| p.display()).join(", ")
                ));
            }
            if let Some(write) = file_system.write.as_ref() {
                content.push(format!(
                    "File System Write Access: {}",
                    write.iter().map(|p| p.display()).join(", ")
                ));
            }
        }
        if let Some(network) = permissions.network.as_ref()
            && let Some(enabled) = network.enabled
        {
            content.push(format!("Network Access: {enabled}"));
        }
        let content = if content.is_empty() {
            None
        } else {
            Some(vec![content.join("\n").into()])
        };

        self.spawn_permission_request(
            client,
            permissions_request_key(&call_id),
            PendingPermissionRequest::RequestPermissions {
                call_id,
                permissions: permissions.into(),
            },
            ToolCallUpdate::new(
                tool_call_id,
                ToolCallUpdateFields::new()
                    .status(ToolCallStatus::Pending)
                    .title(reason.unwrap_or_else(|| "Permissions Request".to_string()))
                    .raw_input(raw_input)
                    .content(content),
            ),
            vec![
                PermissionOption::new(
                    "approved-for-session",
                    "Yes, for session",
                    PermissionOptionKind::AllowAlways,
                ),
                PermissionOption::new("approved", "Yes", PermissionOptionKind::AllowOnce),
                PermissionOption::new("abort", "No", PermissionOptionKind::RejectOnce),
            ],
        );

        Ok(())
    }

    async fn request_user_input(
        &mut self,
        client: &SessionClient,
        event: RequestUserInputEvent,
    ) -> Result<(), Error> {
        let RequestUserInputEvent {
            call_id,
            turn_id,
            questions,
        } = event;
        let tool_call_id = ToolCallId::new(format!("request-user-input:{call_id}"));
        let raw_input = serde_json::to_value(&questions)
            .map_err(|err| Error::internal_error().data(err.to_string()))?;

        client
            .send_tool_call(
                ToolCall::new(tool_call_id.clone(), request_user_input_title(&questions))
                    .kind(ToolKind::Other)
                    .status(ToolCallStatus::Pending)
                    .content(vec![render_request_user_input_prompt(&questions).into()])
                    .raw_input(raw_input),
            )
            .await;

        if self.pending_user_input_request.is_some() {
            warn!(
                "Overwriting existing pending request_user_input for submission {}",
                self.submission_id
            );
        }
        self.pending_user_input_request = Some(PendingUserInputRequest {
            response_id: turn_id,
            call_id,
            questions,
            tool_call_id,
        });

        Ok(())
    }

    fn prepare_user_input_answer(
        &self,
        prompt: &[ContentBlock],
    ) -> Result<Option<PreparedUserInputAnswer>, Error> {
        let Some(pending) = self.pending_user_input_request.as_ref() else {
            return Ok(None);
        };
        let response = build_request_user_input_response(prompt, &pending.questions)?;
        Ok(Some(PreparedUserInputAnswer {
            response_id: pending.response_id.clone(),
            call_id: pending.call_id.clone(),
            questions: pending.questions.clone(),
            response,
            tool_call_id: pending.tool_call_id.clone(),
        }))
    }

    async fn finalize_user_input_answer(
        &mut self,
        client: &SessionClient,
        prepared: PreparedUserInputAnswer,
        response_tx: oneshot::Sender<Result<StopReason, Error>>,
    ) {
        self.pending_user_input_request = None;
        self.add_response_waiter(response_tx);

        let raw_output =
            (!prepared.questions.iter().any(|question| question.is_secret)).then(|| {
                serde_json::to_value(&prepared.response)
                    .unwrap_or_else(|_| serde_json::json!({ "callId": prepared.call_id }))
            });

        client
            .send_tool_call_update(ToolCallUpdate::new(
                prepared.tool_call_id,
                ToolCallUpdateFields::new()
                    .status(ToolCallStatus::Completed)
                    .raw_output(raw_output),
            ))
            .await;
    }
}

#[derive(Clone)]
struct ExecPermissionOption {
    option_id: &'static str,
    permission_option: PermissionOption,
    decision: ReviewDecision,
}

fn build_exec_permission_options(
    available_decisions: &[ReviewDecision],
    network_approval_context: Option<&NetworkApprovalContext>,
    additional_permissions: Option<&PermissionProfile>,
) -> Vec<ExecPermissionOption> {
    available_decisions
        .iter()
        .filter_map(|decision| match decision {
            ReviewDecision::Approved => Some(ExecPermissionOption {
                option_id: "approved",
                permission_option: PermissionOption::new(
                    "approved",
                    if network_approval_context.is_some() {
                        "Yes, just this once"
                    } else {
                        "Yes, proceed"
                    },
                    PermissionOptionKind::AllowOnce,
                ),
                decision: ReviewDecision::Approved,
            }),
            ReviewDecision::ApprovedExecpolicyAmendment {
                proposed_execpolicy_amendment,
            } => Some({
                let command_prefix = proposed_execpolicy_amendment.command().join(" ");
                let label = if command_prefix.contains('\n')
                    || command_prefix.contains('\r')
                    || command_prefix.is_empty()
                {
                    "Yes, and remember this command pattern".to_string()
                } else {
                    format!(
                        "Yes, and don't ask again for commands that start with `{command_prefix}`"
                    )
                };
                ExecPermissionOption {
                    option_id: "approved-execpolicy-amendment",
                    permission_option: PermissionOption::new(
                        "approved-execpolicy-amendment",
                        label,
                        PermissionOptionKind::AllowAlways,
                    ),
                    decision: ReviewDecision::ApprovedExecpolicyAmendment {
                        proposed_execpolicy_amendment: proposed_execpolicy_amendment.clone(),
                    },
                }
            }),
            ReviewDecision::ApprovedForSession => Some(ExecPermissionOption {
                option_id: "approved-for-session",
                permission_option: PermissionOption::new(
                    "approved-for-session",
                    if network_approval_context.is_some() {
                        "Yes, and allow this host for this session"
                    } else if additional_permissions.is_some() {
                        "Yes, and allow these permissions for this session"
                    } else {
                        "Yes, and don't ask again for this command in this session"
                    },
                    PermissionOptionKind::AllowAlways,
                ),
                decision: ReviewDecision::ApprovedForSession,
            }),
            ReviewDecision::NetworkPolicyAmendment {
                network_policy_amendment,
            } => Some({
                let (option_id, label, kind) = match network_policy_amendment.action {
                    NetworkPolicyRuleAction::Allow => (
                        "network-policy-amendment-allow",
                        "Yes, and allow this host in the future",
                        PermissionOptionKind::AllowAlways,
                    ),
                    NetworkPolicyRuleAction::Deny => (
                        "network-policy-amendment-deny",
                        "No, and block this host in the future",
                        PermissionOptionKind::RejectAlways,
                    ),
                };
                ExecPermissionOption {
                    option_id,
                    permission_option: PermissionOption::new(option_id, label, kind),
                    decision: ReviewDecision::NetworkPolicyAmendment {
                        network_policy_amendment: network_policy_amendment.clone(),
                    },
                }
            }),
            ReviewDecision::Denied => Some(ExecPermissionOption {
                option_id: "denied",
                permission_option: PermissionOption::new(
                    "denied",
                    "No, continue without running it",
                    PermissionOptionKind::RejectOnce,
                ),
                decision: ReviewDecision::Denied,
            }),
            ReviewDecision::TimedOut => None,
            ReviewDecision::Abort => Some(ExecPermissionOption {
                option_id: "abort",
                permission_option: PermissionOption::new(
                    "abort",
                    "No, and tell Codex what to do differently",
                    PermissionOptionKind::RejectOnce,
                ),
                decision: ReviewDecision::Abort,
            }),
        })
        .collect()
}

fn resolve_runtime_actor_cli_path() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| std::fs::canonicalize(&path).ok())
}

fn canonicalize_runtime_actor_cli_path(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    std::fs::canonicalize(trimmed).ok()
}

fn auto_approve_runtime_actor_cli_decision(
    command: &[String],
    available_decisions: &[ReviewDecision],
    runtime_actor_cli_path: Option<&Path>,
) -> Option<ReviewDecision> {
    let runtime_actor_cli_path = runtime_actor_cli_path?;
    if !matches_runtime_actor_cli_command(command, runtime_actor_cli_path) {
        return None;
    }
    available_decisions
        .iter()
        .find(|decision| matches!(decision, ReviewDecision::Approved))
        .cloned()
        .or_else(|| {
            available_decisions
                .iter()
                .find(|decision| {
                    matches!(decision, ReviewDecision::ApprovedExecpolicyAmendment { .. })
                })
                .cloned()
        })
}

fn matches_runtime_actor_cli_command(command: &[String], runtime_actor_cli_path: &Path) -> bool {
    command_has_runtime_actor_cli_prefix(command, runtime_actor_cli_path)
        || extract_shell_wrapped_command(command)
            .filter(|shell_command| shell_command_is_single_actor_cli_invocation(shell_command))
            .and_then(shlex::split)
            .is_some_and(|segments| {
                command_has_runtime_actor_cli_prefix(segments.as_slice(), runtime_actor_cli_path)
            })
}

fn command_has_runtime_actor_cli_prefix<T: AsRef<str>>(
    command: &[T],
    runtime_actor_cli_path: &Path,
) -> bool {
    let path_env = std::env::var_os("PATH");
    let path_ext_env = std::env::var_os("PATHEXT");
    command_has_runtime_actor_cli_prefix_with_env(
        command,
        runtime_actor_cli_path,
        path_env.as_deref(),
        path_ext_env.as_deref(),
    )
}

fn command_has_runtime_actor_cli_prefix_with_env<T: AsRef<str>>(
    command: &[T],
    runtime_actor_cli_path: &Path,
    path_env: Option<&OsStr>,
    path_ext_env: Option<&OsStr>,
) -> bool {
    if command.len() < 2 {
        return false;
    }
    if command[1].as_ref() != "actor" {
        return false;
    }
    resolve_runtime_actor_cli_command_path(command[0].as_ref(), path_env, path_ext_env)
        .as_deref()
        .is_some_and(|candidate| candidate == runtime_actor_cli_path)
}

fn resolve_runtime_actor_cli_command_path(
    command: &str,
    path_env: Option<&OsStr>,
    path_ext_env: Option<&OsStr>,
) -> Option<PathBuf> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }
    if is_path_like_command(trimmed) {
        return canonicalize_runtime_actor_cli_path(trimmed);
    }
    resolve_command_on_path(trimmed, path_env, path_ext_env)
}

fn is_path_like_command(command: &str) -> bool {
    let mut components = Path::new(command).components();
    match components.next() {
        Some(
            Component::CurDir | Component::ParentDir | Component::RootDir | Component::Prefix(_),
        ) => true,
        Some(_) => components.next().is_some(),
        None => false,
    }
}

fn resolve_command_on_path(
    command: &str,
    path_env: Option<&OsStr>,
    path_ext_env: Option<&OsStr>,
) -> Option<PathBuf> {
    let path_env = path_env?;
    for dir in std::env::split_paths(path_env) {
        for candidate in runtime_actor_cli_path_candidates(dir.as_path(), command, path_ext_env) {
            if let Some(resolved) =
                canonicalize_runtime_actor_cli_path(candidate.to_string_lossy().as_ref())
            {
                return Some(resolved);
            }
        }
    }
    None
}

fn runtime_actor_cli_path_candidates(
    dir: &Path,
    command: &str,
    path_ext_env: Option<&OsStr>,
) -> Vec<PathBuf> {
    let base = dir.join(command);
    cfg_select! {
        windows => {
            let mut candidates = vec![base.clone()];
            if base.extension().is_none() {
                let path_ext_env = path_ext_env
                    .and_then(OsStr::to_str)
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(".COM;.EXE;.BAT;.CMD");
                for ext in path_ext_env.split(';') {
                    let normalized = ext.trim().trim_start_matches('.');
                    if normalized.is_empty() {
                        continue;
                    }
                    candidates.push(base.with_extension(normalized));
                }
            }
            candidates
        },
        _ => {
            let _ = path_ext_env;
            vec![base]
        },
    }
}

fn shell_command_is_single_actor_cli_invocation(shell_command: &str) -> bool {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum QuoteState {
        Unquoted,
        SingleQuoted,
        DoubleQuoted,
    }

    let mut state = QuoteState::Unquoted;
    let mut escaped = false;
    for ch in shell_command.chars() {
        match state {
            QuoteState::Unquoted => {
                if escaped {
                    escaped = false;
                    continue;
                }
                match ch {
                    '\\' => escaped = true,
                    '\'' => state = QuoteState::SingleQuoted,
                    '"' => state = QuoteState::DoubleQuoted,
                    ';' | '|' | '&' | '<' | '>' | '`' | '$' | '(' | ')' | '\n' | '\r' => {
                        return false;
                    }
                    _ => {}
                }
            }
            QuoteState::SingleQuoted => {
                if ch == '\'' {
                    state = QuoteState::Unquoted;
                }
            }
            QuoteState::DoubleQuoted => {
                if escaped {
                    escaped = false;
                    continue;
                }
                match ch {
                    '\\' => escaped = true,
                    '"' => state = QuoteState::Unquoted,
                    '`' | '$' => return false,
                    _ => {}
                }
            }
        }
    }
    !escaped && state == QuoteState::Unquoted
}

fn extract_shell_wrapped_command(command: &[String]) -> Option<&str> {
    if command.len() != 3 {
        return None;
    }
    match command[1].as_str() {
        "-c" | "-lc" => {}
        _ => return None,
    }
    let shell = Path::new(command[0].as_str())
        .file_name()
        .and_then(OsStr::to_str)?;
    match shell {
        "sh" | "bash" | "zsh" => Some(command[2].as_str()),
        _ => None,
    }
}

struct ParseCommandToolCall {
    title: String,
    file_extension: Option<String>,
    terminal_output: bool,
    locations: Vec<ToolCallLocation>,
    kind: ToolKind,
}

fn parse_command_tool_call(parsed_cmd: Vec<ParsedCommand>, cwd: &Path) -> ParseCommandToolCall {
    let mut titles = Vec::new();
    let mut locations = Vec::new();
    let mut file_extension = None;
    let mut terminal_output = false;
    let mut kind = ToolKind::Execute;

    for cmd in parsed_cmd {
        let mut cmd_path = None;
        match cmd {
            ParsedCommand::Read { cmd: _, name, path } => {
                titles.push(format!("Read {name}"));
                file_extension = path
                    .extension()
                    .map(|ext| ext.to_string_lossy().to_string());
                cmd_path = Some(path);
                kind = ToolKind::Read;
            }
            ParsedCommand::ListFiles { cmd: _, path } => {
                let dir = if let Some(path) = path.as_ref() {
                    &cwd.join(path)
                } else {
                    cwd
                };
                titles.push(format!("List {}", dir.display()));
                cmd_path = path.map(PathBuf::from);
                kind = ToolKind::Search;
            }
            ParsedCommand::Search { cmd, query, path } => {
                titles.push(match (query, path.as_ref()) {
                    (Some(query), Some(path)) => format!("Search {query} in {path}"),
                    (Some(query), None) => format!("Search {query}"),
                    _ => format!("Search {cmd}"),
                });
                kind = ToolKind::Search;
            }
            ParsedCommand::Unknown { cmd } => {
                titles.push(format!("Run {cmd}"));
                terminal_output = true;
            }
        }

        if let Some(path) = cmd_path {
            locations.push(ToolCallLocation::new(if path.is_relative() {
                cwd.join(&path)
            } else {
                path
            }));
        }
    }

    ParseCommandToolCall {
        title: titles.join(", "),
        file_extension,
        terminal_output,
        locations,
        kind,
    }
}

fn background_terminal_activity_meta(kind: &str, command: &str) -> Meta {
    Meta::from_iter([(
        "terminal_activity".to_owned(),
        serde_json::json!({
            "kind": kind,
            "command": command,
        }),
    )])
}

#[derive(Clone)]
struct SessionClient {
    session_id: SessionId,
    client: Arc<dyn Client>,
    client_capabilities: Arc<Mutex<ClientCapabilities>>,
}

impl SessionClient {
    fn new(session_id: SessionId, client_capabilities: Arc<Mutex<ClientCapabilities>>) -> Self {
        Self {
            session_id,
            client: ACP_CLIENT.get().expect("Client should be set").clone(),
            client_capabilities,
        }
    }

    #[cfg(test)]
    fn with_client(
        session_id: SessionId,
        client: Arc<dyn Client>,
        client_capabilities: Arc<Mutex<ClientCapabilities>>,
    ) -> Self {
        Self {
            session_id,
            client,
            client_capabilities,
        }
    }

    fn supports_terminal_output(&self, active_command: &ActiveCommand) -> bool {
        active_command.terminal_output
            && self
                .client_capabilities
                .lock()
                .unwrap()
                .meta
                .as_ref()
                .is_some_and(|v| {
                    v.get("terminal_output")
                        .is_some_and(|v| v.as_bool().unwrap_or_default())
                })
    }

    async fn send_notification(&self, update: SessionUpdate) {
        if let Err(e) = self
            .client
            .session_notification(SessionNotification::new(self.session_id.clone(), update))
            .await
        {
            error!("Failed to send session notification: {:?}", e);
        }
    }

    async fn send_user_message(&self, text: impl Into<String>) {
        self.send_notification(SessionUpdate::UserMessageChunk(ContentChunk::new(
            text.into().into(),
        )))
        .await;
    }

    async fn send_agent_text(&self, text: impl Into<String>) {
        self.send_notification(SessionUpdate::AgentMessageChunk(ContentChunk::new(
            text.into().into(),
        )))
        .await;
    }

    async fn send_agent_thought(&self, text: impl Into<String>) {
        self.send_notification(SessionUpdate::AgentThoughtChunk(ContentChunk::new(
            text.into().into(),
        )))
        .await;
    }

    async fn send_tool_call(&self, tool_call: ToolCall) {
        self.send_notification(SessionUpdate::ToolCall(tool_call))
            .await;
    }

    async fn send_tool_call_update(&self, update: ToolCallUpdate) {
        self.send_notification(SessionUpdate::ToolCallUpdate(update))
            .await;
    }

    /// Send a completed tool call (used for replay and simple cases)
    async fn send_completed_tool_call(
        &self,
        call_id: impl Into<ToolCallId>,
        title: impl Into<String>,
        kind: ToolKind,
        raw_input: Option<serde_json::Value>,
    ) {
        let mut tool_call = ToolCall::new(call_id, title)
            .kind(kind)
            .status(ToolCallStatus::Completed);
        if let Some(input) = raw_input {
            tool_call = tool_call.raw_input(input);
        }
        self.send_tool_call(tool_call).await;
    }

    /// Send a tool call completion update (used for replay)
    async fn send_tool_call_completed(
        &self,
        call_id: impl Into<ToolCallId>,
        raw_output: Option<serde_json::Value>,
    ) {
        let mut fields = ToolCallUpdateFields::new().status(ToolCallStatus::Completed);
        if let Some(output) = raw_output {
            fields = fields.raw_output(output);
        }
        self.send_tool_call_update(ToolCallUpdate::new(call_id, fields))
            .await;
    }

    async fn update_plan(&self, plan: Vec<PlanItemArg>) {
        self.send_notification(SessionUpdate::Plan(Plan::new(
            plan.into_iter()
                .map(|entry| {
                    PlanEntry::new(
                        entry.step,
                        PlanEntryPriority::Medium,
                        match entry.status {
                            StepStatus::Pending => PlanEntryStatus::Pending,
                            StepStatus::InProgress => PlanEntryStatus::InProgress,
                            StepStatus::Completed => PlanEntryStatus::Completed,
                        },
                    )
                })
                .collect(),
        )))
        .await;
    }

    async fn request_permission(
        &self,
        tool_call: ToolCallUpdate,
        options: Vec<PermissionOption>,
    ) -> Result<RequestPermissionResponse, Error> {
        self.client
            .request_permission(RequestPermissionRequest::new(
                self.session_id.clone(),
                tool_call,
                options,
            ))
            .await
    }
}

struct ThreadActor<A> {
    /// Allows for logging out from slash commands
    auth: A,
    /// Used for sending messages back to the client.
    client: SessionClient,
    /// The thread associated with this task.
    thread: Arc<dyn CodexThreadImpl>,
    /// The configuration for the thread.
    config: Config,
    /// The models available for this thread.
    models_manager: Arc<dyn ModelsManagerImpl>,
    /// Internal message sender used to route spawned interaction results back to the actor.
    resolution_tx: mpsc::UnboundedSender<ThreadMessage>,
    /// A sender for each interested `Op` submission that needs events routed.
    submissions: HashMap<String, SubmissionState>,
    /// A receiver for incoming thread messages.
    message_rx: mpsc::UnboundedReceiver<ThreadMessage>,
    /// A receiver for spawned interaction results.
    resolution_rx: mpsc::UnboundedReceiver<ThreadMessage>,
    /// Last config options state we emitted to the client, used for deduping updates.
    last_sent_config_options: Option<Vec<SessionConfigOption>>,
}

impl<A: Auth> ThreadActor<A> {
    #[expect(clippy::too_many_arguments)]
    fn new(
        auth: A,
        client: SessionClient,
        thread: Arc<dyn CodexThreadImpl>,
        models_manager: Arc<dyn ModelsManagerImpl>,
        config: Config,
        message_rx: mpsc::UnboundedReceiver<ThreadMessage>,
        resolution_tx: mpsc::UnboundedSender<ThreadMessage>,
        resolution_rx: mpsc::UnboundedReceiver<ThreadMessage>,
    ) -> Self {
        Self {
            auth,
            client,
            thread,
            config,
            models_manager,
            resolution_tx,
            submissions: HashMap::new(),
            message_rx,
            resolution_rx,
            last_sent_config_options: None,
        }
    }

    async fn spawn(mut self) {
        let mut message_rx_open = true;
        loop {
            tokio::select! {
                biased;
                message = self.message_rx.recv(), if message_rx_open => match message {
                    Some(message) => self.handle_message(message).await,
                    None => message_rx_open = false,
                },
                message = self.resolution_rx.recv() => if let Some(message) = message {
                    self.handle_message(message).await
                },
                event = self.thread.next_event() => match event {
                    Ok(event) => self.handle_event(event).await,
                    Err(e) => {
                        error!("Error getting next event: {:?}", e);
                        break;
                    }
                }
            }
            // Litter collection of senders with no receivers
            self.submissions
                .retain(|_, submission| submission.is_active());

            if !message_rx_open && self.submissions.is_empty() {
                break;
            }
        }
    }

    async fn handle_message(&mut self, message: ThreadMessage) {
        match message {
            ThreadMessage::Load { response_tx } => {
                let result = self.handle_load().await;
                drop(response_tx.send(result));
                self.client
                    .send_notification(SessionUpdate::AvailableCommandsUpdate(
                        AvailableCommandsUpdate::new(Self::builtin_commands()),
                    ))
                    .await;
            }
            ThreadMessage::GetConfigOptions { response_tx } => {
                let result = self.config_options().await;
                drop(response_tx.send(result));
            }
            ThreadMessage::Prompt {
                request,
                response_tx,
            } => {
                let result = self.handle_prompt(request).await;
                drop(response_tx.send(result));
            }
            ThreadMessage::SetMode { mode, response_tx } => {
                let result = self.handle_set_mode(mode).await;
                drop(response_tx.send(result));
                self.maybe_emit_config_options_update().await;
            }
            ThreadMessage::SetModel { model, response_tx } => {
                let result = self.handle_set_model(model).await;
                drop(response_tx.send(result));
                self.maybe_emit_config_options_update().await;
            }
            ThreadMessage::SetConfigOption {
                config_id,
                value,
                response_tx,
            } => {
                let result = self.handle_set_config_option(config_id, value).await;
                drop(response_tx.send(result));
            }
            ThreadMessage::Cancel { response_tx } => {
                let result = self.handle_cancel().await;
                drop(response_tx.send(result));
            }
            ThreadMessage::Shutdown { response_tx } => {
                let result = self.handle_shutdown().await;
                drop(response_tx.send(result));
            }
            ThreadMessage::ReplayHistory {
                history,
                response_tx,
            } => {
                let result = self.handle_replay_history(history).await;
                drop(response_tx.send(result));
            }
            ThreadMessage::PermissionRequestResolved {
                submission_id,
                request_key,
                response,
            } => {
                let Some(submission) = self.submissions.get_mut(&submission_id) else {
                    warn!(
                        "Ignoring permission response for unknown submission ID: {submission_id}"
                    );
                    return;
                };

                if let Err(err) = submission
                    .handle_permission_request_resolved(&self.client, request_key, response)
                    .await
                {
                    submission.abort_pending_interactions();
                    submission.fail(err);
                }
            }
        }
    }

    fn builtin_commands() -> Vec<AvailableCommand> {
        vec![
            AvailableCommand::new("review", "Review my current changes and find issues").input(
                AvailableCommandInput::Unstructured(UnstructuredCommandInput::new(
                    "optional custom review instructions",
                )),
            ),
            AvailableCommand::new(
                "review-branch",
                "Review the code changes against a specific branch",
            )
            .input(AvailableCommandInput::Unstructured(
                UnstructuredCommandInput::new("branch name"),
            )),
            AvailableCommand::new(
                "review-commit",
                "Review the code changes introduced by a commit",
            )
            .input(AvailableCommandInput::Unstructured(
                UnstructuredCommandInput::new("commit sha"),
            )),
            AvailableCommand::new(
                "init",
                "create an AGENTS.md file with instructions for Codex",
            ),
            AvailableCommand::new(
                "compact",
                "summarize conversation to prevent hitting the context limit",
            ),
            AvailableCommand::new("undo", "undo Codex’s most recent turn"),
            AvailableCommand::new("logout", "logout of Codex"),
        ]
    }

    fn modes(&self) -> Option<SessionModeState> {
        let current_mode_id = APPROVAL_PRESETS
            .iter()
            .find(|preset| {
                &preset.approval == self.config.permissions.approval_policy.get()
                    && &preset.sandbox == self.config.permissions.sandbox_policy.get()
            })
            .or_else(|| {
                // When the project is untrusted, the above code won't match
                // since AskForApproval::UnlessTrusted is not part of the
                // default presets. However, in this case we still want to show
                // the mode selector, which allows the user to choose a
                // different mode (which will set the project to be trusted)
                // See https://github.com/zed-industries/zed/issues/48132
                if self.config.active_project.is_untrusted() {
                    APPROVAL_PRESETS
                        .iter()
                        .find(|preset| preset.id == "read-only")
                } else {
                    None
                }
            })
            .map(|preset| SessionModeId::new(preset.id))?;

        Some(SessionModeState::new(
            current_mode_id,
            APPROVAL_PRESETS
                .iter()
                .map(|preset| {
                    SessionMode::new(preset.id, preset.label).description(preset.description)
                })
                .collect(),
        ))
    }

    async fn find_current_model(&self) -> Option<ModelId> {
        let model_presets = self.models_manager.list_models().await;
        let config_model = self.get_current_model().await;
        let preset = model_presets
            .iter()
            .find(|preset| preset.model == config_model)?;

        let effort = self
            .config
            .model_reasoning_effort
            .and_then(|effort| {
                preset
                    .supported_reasoning_efforts
                    .iter()
                    .find_map(|e| (e.effort == effort).then_some(effort))
            })
            .unwrap_or(preset.default_reasoning_effort);

        Some(Self::model_id(&preset.id, effort))
    }

    fn model_id(id: &str, effort: ReasoningEffort) -> ModelId {
        ModelId::new(format!("{id}/{effort}"))
    }

    fn parse_model_id(id: &ModelId) -> Option<(String, ReasoningEffort)> {
        let (model, reasoning) = id.0.split_once('/')?;
        let reasoning = serde_json::from_value(reasoning.into()).ok()?;
        Some((model.to_owned(), reasoning))
    }

    async fn config_options(&self) -> Result<Vec<SessionConfigOption>, Error> {
        let mut options = Vec::new();

        if let Some(modes) = self.modes() {
            let select_options = modes
                .available_modes
                .into_iter()
                .map(|m| SessionConfigSelectOption::new(m.id.0, m.name).description(m.description))
                .collect::<Vec<_>>();

            options.push(
                SessionConfigOption::select(
                    "mode",
                    "Approval Preset",
                    modes.current_mode_id.0,
                    select_options,
                )
                .category(SessionConfigOptionCategory::Mode)
                .description("Choose an approval and sandboxing preset for your session"),
            );
        }

        let presets = self.models_manager.list_models().await;

        let current_model = self.get_current_model().await;
        let current_preset = presets.iter().find(|p| p.model == current_model).cloned();

        let mut model_select_options = Vec::new();

        if current_preset.is_none() {
            // If no preset found, return the current model string as-is
            model_select_options.push(SessionConfigSelectOption::new(
                current_model.clone(),
                current_model.clone(),
            ));
        };

        model_select_options.extend(
            presets
                .into_iter()
                .filter(|model| model.show_in_picker || model.model == current_model)
                .map(|preset| {
                    SessionConfigSelectOption::new(preset.id, preset.display_name)
                        .description(preset.description)
                }),
        );

        options.push(
            SessionConfigOption::select("model", "Model", current_model, model_select_options)
                .category(SessionConfigOptionCategory::Model)
                .description("Choose which model Codex should use"),
        );

        // Reasoning effort selector (only if the current preset exists and has >1 supported effort)
        if let Some(preset) = current_preset
            && preset.supported_reasoning_efforts.len() > 1
        {
            let supported = &preset.supported_reasoning_efforts;

            let current_effort = self
                .config
                .model_reasoning_effort
                .and_then(|effort| {
                    supported
                        .iter()
                        .find_map(|e| (e.effort == effort).then_some(effort))
                })
                .unwrap_or(preset.default_reasoning_effort);

            let effort_select_options = supported
                .iter()
                .map(|e| {
                    SessionConfigSelectOption::new(
                        e.effort.to_string(),
                        e.effort.to_string().to_title_case(),
                    )
                    .description(e.description.clone())
                })
                .collect::<Vec<_>>();

            options.push(
                SessionConfigOption::select(
                    "reasoning_effort",
                    "Reasoning Effort",
                    current_effort.to_string(),
                    effort_select_options,
                )
                .category(SessionConfigOptionCategory::ThoughtLevel)
                .description("Choose how much reasoning effort the model should use"),
            );
        }

        Ok(options)
    }

    async fn maybe_emit_config_options_update(&mut self) {
        let config_options = self.config_options().await.unwrap_or_default();

        if self
            .last_sent_config_options
            .as_ref()
            .is_some_and(|prev| prev == &config_options)
        {
            return;
        }

        self.last_sent_config_options = Some(config_options.clone());

        self.client
            .send_notification(SessionUpdate::ConfigOptionUpdate(ConfigOptionUpdate::new(
                config_options,
            )))
            .await;
    }

    async fn handle_set_config_option(
        &mut self,
        config_id: SessionConfigId,
        value: SessionConfigOptionValue,
    ) -> Result<(), Error> {
        let SessionConfigOptionValue::ValueId { value } = value else {
            return Err(Error::invalid_params().data("Unsupported config option value"));
        };
        match config_id.0.as_ref() {
            "mode" => self.handle_set_mode(SessionModeId::new(value.0)).await,
            "model" => self.handle_set_config_model(value).await,
            "reasoning_effort" => self.handle_set_config_reasoning_effort(value).await,
            _ => Err(Error::invalid_params().data("Unsupported config option")),
        }
    }

    async fn handle_set_config_model(&mut self, value: SessionConfigValueId) -> Result<(), Error> {
        let model_id = value.0;

        let presets = self.models_manager.list_models().await;
        let preset = presets.iter().find(|p| p.id.as_str() == &*model_id);

        let model_to_use = preset
            .map(|p| p.model.clone())
            .unwrap_or_else(|| model_id.to_string());

        if model_to_use.is_empty() {
            return Err(Error::invalid_params().data("No model selected"));
        }

        let effort_to_use = if let Some(preset) = preset {
            if let Some(effort) = self.config.model_reasoning_effort
                && preset
                    .supported_reasoning_efforts
                    .iter()
                    .any(|e| e.effort == effort)
            {
                Some(effort)
            } else {
                Some(preset.default_reasoning_effort)
            }
        } else {
            // If the user selected a raw model string (not a known preset), don't invent a default.
            // Keep whatever was previously configured (or leave unset) so Codex can decide.
            self.config.model_reasoning_effort
        };

        self.thread
            .submit(
                "config".to_string(),
                Op::OverrideTurnContext {
                    cwd: None,
                    approval_policy: None,
                    approvals_reviewer: None,
                    sandbox_policy: None,
                    model: Some(model_to_use.clone()),
                    effort: Some(effort_to_use),
                    summary: None,
                    collaboration_mode: None,
                    personality: None,
                    windows_sandbox_level: None,
                    service_tier: None,
                },
            )
            .await
            .map_err(|e| Error::from(anyhow::anyhow!(e)))?;

        self.config.model = Some(model_to_use);
        self.config.model_reasoning_effort = effort_to_use;

        Ok(())
    }

    async fn handle_set_config_reasoning_effort(
        &mut self,
        value: SessionConfigValueId,
    ) -> Result<(), Error> {
        let effort: ReasoningEffort =
            serde_json::from_value(value.0.as_ref().into()).map_err(|_| Error::invalid_params())?;

        let current_model = self.get_current_model().await;
        let presets = self.models_manager.list_models().await;
        let Some(preset) = presets.iter().find(|p| p.model == current_model) else {
            return Err(Error::invalid_params()
                .data("Reasoning effort can only be set for known model presets"));
        };

        if !preset
            .supported_reasoning_efforts
            .iter()
            .any(|e| e.effort == effort)
        {
            return Err(
                Error::invalid_params().data("Unsupported reasoning effort for selected model")
            );
        }

        self.thread
            .submit(
                "config".to_string(),
                Op::OverrideTurnContext {
                    cwd: None,
                    approval_policy: None,
                    approvals_reviewer: None,
                    sandbox_policy: None,
                    model: None,
                    effort: Some(Some(effort)),
                    summary: None,
                    collaboration_mode: None,
                    personality: None,
                    windows_sandbox_level: None,
                    service_tier: None,
                },
            )
            .await
            .map_err(|e| Error::from(anyhow::anyhow!(e)))?;

        self.config.model_reasoning_effort = Some(effort);

        Ok(())
    }

    async fn models(&self) -> Result<SessionModelState, Error> {
        let mut available_models = Vec::new();
        let config_model = self.get_current_model().await;

        let current_model_id = if let Some(model_id) = self.find_current_model().await {
            model_id
        } else {
            // If no preset found, return the current model string as-is
            let model_id = ModelId::new(self.get_current_model().await);
            available_models.push(ModelInfo::new(model_id.clone(), model_id.to_string()));
            model_id
        };

        available_models.extend(
            self.models_manager
                .list_models()
                .await
                .iter()
                .filter(|model| model.show_in_picker || model.model == config_model)
                .flat_map(|preset| {
                    preset.supported_reasoning_efforts.iter().map(|effort| {
                        ModelInfo::new(
                            Self::model_id(&preset.id, effort.effort),
                            format!("{} ({})", preset.display_name, effort.effort),
                        )
                        .description(format!("{} {}", preset.description, effort.description))
                    })
                }),
        );

        Ok(SessionModelState::new(current_model_id, available_models))
    }

    async fn handle_load(&mut self) -> Result<LoadSessionResponse, Error> {
        Ok(LoadSessionResponse::new()
            .models(self.models().await?)
            .modes(self.modes())
            .config_options(self.config_options().await?))
    }

    async fn handle_prompt(
        &mut self,
        request: PromptRequest,
    ) -> Result<oneshot::Receiver<Result<StopReason, Error>>, Error> {
        let (response_tx, response_rx) = oneshot::channel();

        if let Some((submission_id, prepared)) =
            self.find_pending_user_input_answer(request.prompt.as_slice())?
        {
            self.thread
                .submit(
                    submission_id.clone(),
                    Op::UserInputAnswer {
                        id: prepared.response_id.clone(),
                        response: prepared.response.clone(),
                    },
                )
                .await
                .map_err(|e| {
                    let err = Error::internal_error().data(e.to_string());
                    if let Some(submission) = self.submissions.get_mut(&submission_id) {
                        submission.fail(err.clone());
                    }
                    err
                })?;

            let submission = self
                .submissions
                .get_mut(&submission_id)
                .ok_or_else(|| Error::internal_error().data("missing pending question state"))?;
            submission
                .finalize_user_input_answer(&self.client, prepared, response_tx)
                .await;
            return Ok(response_rx);
        }

        let items = build_prompt_items(request.prompt);
        let op;
        if let Some((name, rest)) = extract_slash_command(&items) {
            match name {
                "compact" => op = Op::Compact,
                "undo" => op = Op::Undo,
                "init" => {
                    op = Op::UserInput {
                        items: vec![UserInput::Text {
                            text: INIT_COMMAND_PROMPT.into(),
                            text_elements: vec![],
                        }],
                        final_output_json_schema: None,
                        responsesapi_client_metadata: None,
                    }
                }
                "review" => {
                    let instructions = rest.trim();
                    let target = if instructions.is_empty() {
                        ReviewTarget::UncommittedChanges
                    } else {
                        ReviewTarget::Custom {
                            instructions: instructions.to_owned(),
                        }
                    };

                    op = Op::Review {
                        review_request: ReviewRequest {
                            user_facing_hint: Some(user_facing_hint(&target)),
                            target,
                        },
                    }
                }
                "review-branch" if !rest.is_empty() => {
                    let target = ReviewTarget::BaseBranch {
                        branch: rest.trim().to_owned(),
                    };
                    op = Op::Review {
                        review_request: ReviewRequest {
                            user_facing_hint: Some(user_facing_hint(&target)),
                            target,
                        },
                    }
                }
                "review-commit" if !rest.is_empty() => {
                    let target = ReviewTarget::Commit {
                        sha: rest.trim().to_owned(),
                        title: None,
                    };
                    op = Op::Review {
                        review_request: ReviewRequest {
                            user_facing_hint: Some(user_facing_hint(&target)),
                            target,
                        },
                    }
                }
                "logout" => {
                    self.auth.logout()?;
                    return Err(Error::auth_required());
                }
                _ => {
                    op = Op::UserInput {
                        items,
                        final_output_json_schema: None,
                        responsesapi_client_metadata: None,
                    }
                }
            }
        } else {
            op = Op::UserInput {
                items,
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
            }
        }

        let submission_id = self
            .thread
            .submit(Uuid::new_v4().to_string(), op.clone())
            .await
            .map_err(|e| Error::internal_error().data(e.to_string()))?;

        info!("Submitted prompt with submission_id: {submission_id}");
        info!("Starting to wait for conversation events for submission_id: {submission_id}");

        if let Some(SubmissionState::Prompt(state)) = self.submissions.get_mut(&submission_id) {
            state.add_response_waiter(response_tx);
        } else {
            let state = SubmissionState::Prompt(Box::new(PromptState::new(
                submission_id.clone(),
                self.thread.clone(),
                self.resolution_tx.clone(),
                response_tx,
            )));
            self.submissions.insert(submission_id, state);
        }

        Ok(response_rx)
    }

    fn find_pending_user_input_answer(
        &self,
        prompt: &[ContentBlock],
    ) -> Result<Option<(String, PreparedUserInputAnswer)>, Error> {
        for (submission_id, submission) in &self.submissions {
            if !submission.has_pending_user_input() {
                continue;
            }
            if let Some(prepared) = submission.prepare_user_input_answer(prompt)? {
                return Ok(Some((submission_id.clone(), prepared)));
            }
        }
        Ok(None)
    }

    async fn handle_set_mode(&mut self, mode: SessionModeId) -> Result<(), Error> {
        let preset = APPROVAL_PRESETS
            .iter()
            .find(|preset| mode.0.as_ref() == preset.id)
            .ok_or_else(Error::invalid_params)?;

        self.thread
            .submit(
                "config".to_string(),
                Op::OverrideTurnContext {
                    cwd: None,
                    approval_policy: Some(preset.approval),
                    approvals_reviewer: None,
                    sandbox_policy: Some(preset.sandbox.clone()),
                    model: None,
                    effort: None,
                    summary: None,
                    collaboration_mode: None,
                    personality: None,
                    windows_sandbox_level: None,
                    service_tier: None,
                },
            )
            .await
            .map_err(|e| Error::from(anyhow::anyhow!(e)))?;

        self.config
            .permissions
            .approval_policy
            .set(preset.approval)
            .map_err(|e| Error::from(anyhow::anyhow!(e)))?;
        self.config
            .permissions
            .sandbox_policy
            .set(preset.sandbox.clone())
            .map_err(|e| Error::from(anyhow::anyhow!(e)))?;

        match preset.sandbox {
            // Treat this user action as a trusted dir
            SandboxPolicy::DangerFullAccess
            | SandboxPolicy::WorkspaceWrite { .. }
            | SandboxPolicy::ExternalSandbox { .. } => {
                set_project_trust_level(
                    &self.config.codex_home,
                    &self.config.cwd,
                    TrustLevel::Trusted,
                )?;
            }
            SandboxPolicy::ReadOnly { .. } => {}
        }

        Ok(())
    }

    async fn get_current_model(&self) -> String {
        self.models_manager.get_model(&self.config.model).await
    }

    async fn handle_set_model(&mut self, model: ModelId) -> Result<(), Error> {
        // Try parsing as preset format, otherwise use as-is, fallback to config
        let (model_to_use, effort_to_use) = if let Some((m, e)) = Self::parse_model_id(&model) {
            (m, Some(e))
        } else {
            let model_str = model.0.to_string();
            let fallback = if !model_str.is_empty() {
                model_str
            } else {
                self.get_current_model().await
            };
            (fallback, self.config.model_reasoning_effort)
        };

        if model_to_use.is_empty() {
            return Err(Error::invalid_params().data("No model parsed or configured"));
        }

        self.thread
            .submit(
                "config".to_string(),
                Op::OverrideTurnContext {
                    cwd: None,
                    approval_policy: None,
                    approvals_reviewer: None,
                    sandbox_policy: None,
                    model: Some(model_to_use.clone()),
                    effort: Some(effort_to_use),
                    summary: None,
                    collaboration_mode: None,
                    personality: None,
                    windows_sandbox_level: None,
                    service_tier: None,
                },
            )
            .await
            .map_err(|e| Error::from(anyhow::anyhow!(e)))?;

        self.config.model = Some(model_to_use);
        self.config.model_reasoning_effort = effort_to_use;

        Ok(())
    }

    async fn handle_cancel(&mut self) -> Result<(), Error> {
        self.abort_pending_interactions();
        self.thread
            .submit("interrupt".to_string(), Op::Interrupt)
            .await
            .map_err(|e| Error::from(anyhow::anyhow!(e)))?;
        Ok(())
    }

    async fn handle_shutdown(&mut self) -> Result<(), Error> {
        self.abort_pending_interactions();
        self.thread
            .submit("shutdown".to_string(), Op::Shutdown)
            .await
            .map_err(|e| Error::from(anyhow::anyhow!(e)))?;
        Ok(())
    }

    fn abort_pending_interactions(&mut self) {
        for submission in self.submissions.values_mut() {
            submission.abort_pending_interactions();
        }
    }

    /// Replay conversation history to the client via session/update notifications.
    /// This is called when loading a session to stream all prior messages.
    ///
    /// We process both `EventMsg` and `ResponseItem`:
    /// - `EventMsg` for user/agent messages and reasoning (like the TUI does)
    /// - `ResponseItem` for tool calls only (not persisted as EventMsg)
    async fn handle_replay_history(&mut self, history: Vec<RolloutItem>) -> Result<(), Error> {
        for item in history {
            match item {
                RolloutItem::EventMsg(event_msg) => {
                    self.replay_event_msg(&event_msg).await;
                }
                RolloutItem::ResponseItem(response_item) => {
                    self.replay_response_item(&response_item).await;
                }
                // Skip SessionMeta, TurnContext, Compacted
                _ => {}
            }
        }
        Ok(())
    }

    /// Convert and send an EventMsg as ACP notification(s) during replay.
    /// Handles messages and reasoning - mirrors the live event handling in PromptState.
    async fn replay_event_msg(&self, msg: &EventMsg) {
        match msg {
            EventMsg::UserMessage(UserMessageEvent { message, .. }) => {
                self.client.send_user_message(message.clone()).await;
            }
            EventMsg::AgentMessage(AgentMessageEvent {
                message, phase: _, ..
            }) => {
                self.client.send_agent_text(message.clone()).await;
            }
            EventMsg::AgentReasoning(AgentReasoningEvent { text }) => {
                self.client.send_agent_thought(text.clone()).await;
            }
            EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent { text }) => {
                self.client.send_agent_thought(text.clone()).await;
            }
            // Skip other event types during replay - they either:
            // - Are transient (deltas, turn lifecycle)
            // - Don't have direct ACP equivalents
            // - Are handled via ResponseItem instead
            _ => {}
        }
    }

    /// Parse apply_patch call input to extract patch content for display.
    /// Returns (title, locations, content) if successful.
    /// For CustomToolCall, the input is the patch string directly.
    fn parse_apply_patch_call(
        &self,
        input: &str,
    ) -> Option<(String, Vec<ToolCallLocation>, Vec<ToolCallContent>)> {
        // Try to parse the patch using codex-apply-patch parser
        let parsed = parse_patch(input).ok()?;

        let mut locations = Vec::new();
        let mut file_names = Vec::new();
        let mut content = Vec::new();

        for hunk in &parsed.hunks {
            match hunk {
                codex_apply_patch::Hunk::AddFile { path, contents } => {
                    let full_path = self.config.cwd.join(path);
                    file_names.push(path.display().to_string());
                    locations.push(ToolCallLocation::new(full_path.clone()));
                    // New file: no old_text, new_text is the contents
                    content.push(ToolCallContent::Diff(Diff::new(
                        full_path,
                        contents.clone(),
                    )));
                }
                codex_apply_patch::Hunk::DeleteFile { path } => {
                    let full_path = self.config.cwd.join(path);
                    file_names.push(path.display().to_string());
                    locations.push(ToolCallLocation::new(full_path.clone()));
                    // Delete file: old_text would be original content, new_text is empty
                    content.push(ToolCallContent::Diff(
                        Diff::new(full_path, "").old_text("[file deleted]"),
                    ));
                }
                codex_apply_patch::Hunk::UpdateFile {
                    path,
                    move_path,
                    chunks,
                } => {
                    let full_path = self.config.cwd.join(path);
                    let dest_path = move_path
                        .as_ref()
                        .map(|p| self.config.cwd.join(p))
                        .unwrap_or_else(|| full_path.clone());
                    file_names.push(path.display().to_string());
                    locations.push(ToolCallLocation::new(dest_path.clone()));

                    // Build old and new text from chunks
                    let old_lines: Vec<String> = chunks
                        .iter()
                        .flat_map(|c| c.old_lines.iter().cloned())
                        .collect();
                    let new_lines: Vec<String> = chunks
                        .iter()
                        .flat_map(|c| c.new_lines.iter().cloned())
                        .collect();

                    content.push(ToolCallContent::Diff(
                        Diff::new(dest_path, new_lines.join("\n")).old_text(old_lines.join("\n")),
                    ));
                }
            }
        }

        let title = if file_names.is_empty() {
            "Apply patch".to_string()
        } else {
            format!("Edit {}", file_names.join(", "))
        };

        Some((title, locations, content))
    }

    /// Parse shell function call arguments to extract command info for rich display.
    /// Returns (title, kind, locations) if successful.
    ///
    /// Handles both:
    /// - `shell` / `container.exec`: `command` is `Vec<String>`
    /// - `shell_command`: `command` is a `String` (shell script)
    fn parse_shell_function_call(
        &self,
        name: &str,
        arguments: &str,
    ) -> Option<(String, ToolKind, Vec<ToolCallLocation>)> {
        // Extract command and workdir based on tool type
        let (command_vec, workdir): (Vec<String>, Option<String>) = if name == "shell_command" {
            // shell_command: command is a string (shell script)
            #[derive(serde::Deserialize)]
            struct ShellCommandArgs {
                command: String,
                #[serde(default)]
                workdir: Option<String>,
            }
            let args: ShellCommandArgs = serde_json::from_str(arguments).ok()?;
            // Wrap in bash -lc for parsing
            (
                vec!["bash".to_string(), "-lc".to_string(), args.command],
                args.workdir,
            )
        } else {
            // shell / container.exec: command is Vec<String>
            #[derive(serde::Deserialize)]
            struct ShellArgs {
                command: Vec<String>,
                #[serde(default)]
                workdir: Option<String>,
            }
            let args: ShellArgs = serde_json::from_str(arguments).ok()?;
            (args.command, args.workdir)
        };

        let cwd = workdir
            .map(PathBuf::from)
            .unwrap_or_else(|| self.config.cwd.to_path_buf());

        let parsed_cmd = parse_command(&command_vec);
        let ParseCommandToolCall {
            title,
            file_extension: _,
            terminal_output: _,
            locations,
            kind,
        } = parse_command_tool_call(parsed_cmd, &cwd);

        Some((title, kind, locations))
    }

    /// Convert and send a single ResponseItem as ACP notification(s) during replay.
    /// Only handles tool calls - messages/reasoning are handled via EventMsg.
    async fn replay_response_item(&self, item: &ResponseItem) {
        match item {
            // Skip Message and Reasoning - these are handled via EventMsg
            ResponseItem::Message { .. } | ResponseItem::Reasoning { .. } => {}
            ResponseItem::FunctionCall {
                name,
                arguments,
                call_id,
                ..
            } => {
                // Check if this is a shell command - parse it like we do for LocalShellCall
                if matches!(name.as_str(), "shell" | "container.exec" | "shell_command")
                    && let Some((title, kind, locations)) =
                        self.parse_shell_function_call(name, arguments)
                {
                    self.client
                        .send_tool_call(
                            ToolCall::new(call_id.clone(), title)
                                .kind(kind)
                                .status(ToolCallStatus::Completed)
                                .locations(locations)
                                .raw_input(
                                    serde_json::from_str::<serde_json::Value>(arguments).ok(),
                                ),
                        )
                        .await;
                    return;
                }

                // Fall through to generic function call handling
                self.client
                    .send_completed_tool_call(
                        call_id.clone(),
                        name.clone(),
                        ToolKind::Other,
                        serde_json::from_str(arguments).ok(),
                    )
                    .await;
            }
            ResponseItem::FunctionCallOutput { call_id, output } => {
                self.client
                    .send_tool_call_completed(call_id.clone(), serde_json::to_value(output).ok())
                    .await;
            }
            ResponseItem::LocalShellCall {
                call_id: Some(call_id),
                action,
                status,
                ..
            } => {
                let codex_protocol::models::LocalShellAction::Exec(exec) = action;
                let cwd = exec
                    .working_directory
                    .as_ref()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| self.config.cwd.to_path_buf());

                // Parse the command to get rich info like the live event handler does
                let parsed_cmd = parse_command(&exec.command);
                let ParseCommandToolCall {
                    title,
                    file_extension: _,
                    terminal_output: _,
                    locations,
                    kind,
                } = parse_command_tool_call(parsed_cmd, &cwd);

                let tool_status = match status {
                    codex_protocol::models::LocalShellStatus::Completed => {
                        ToolCallStatus::Completed
                    }
                    codex_protocol::models::LocalShellStatus::InProgress
                    | codex_protocol::models::LocalShellStatus::Incomplete => {
                        ToolCallStatus::Failed
                    }
                };
                self.client
                    .send_tool_call(
                        ToolCall::new(call_id.clone(), title)
                            .kind(kind)
                            .status(tool_status)
                            .locations(locations),
                    )
                    .await;
            }
            ResponseItem::CustomToolCall {
                name,
                input,
                call_id,
                ..
            } => {
                // Check if this is an apply_patch call - show the patch content
                if name == "apply_patch"
                    && let Some((title, locations, content)) = self.parse_apply_patch_call(input)
                {
                    self.client
                        .send_tool_call(
                            ToolCall::new(call_id.clone(), title)
                                .kind(ToolKind::Edit)
                                .status(ToolCallStatus::Completed)
                                .locations(locations)
                                .content(content)
                                .raw_input(serde_json::from_str::<serde_json::Value>(input).ok()),
                        )
                        .await;
                    return;
                }

                // Fall through to generic custom tool call handling
                self.client
                    .send_completed_tool_call(
                        call_id.clone(),
                        name.clone(),
                        ToolKind::Other,
                        serde_json::from_str(input).ok(),
                    )
                    .await;
            }
            ResponseItem::CustomToolCallOutput {
                call_id, output, ..
            } => {
                self.client
                    .send_tool_call_completed(call_id.clone(), Some(serde_json::json!(output)))
                    .await;
            }
            ResponseItem::WebSearchCall { id, action, .. } => {
                let (title, call_id) = if let Some(action) = action {
                    web_search_action_to_title_and_id(id, action)
                } else {
                    ("Web Search".into(), generate_fallback_id("web_search"))
                };
                self.client
                    .send_tool_call(
                        ToolCall::new(call_id, title)
                            .kind(ToolKind::Search)
                            .status(ToolCallStatus::Completed),
                    )
                    .await;
            }
            // Skip GhostSnapshot, Compaction, Other, LocalShellCall without call_id
            _ => {}
        }
    }

    async fn handle_event(&mut self, Event { id, msg }: Event) {
        if !self.submissions.contains_key(&id) && should_attach_detached_submission(&msg) {
            info!("Attaching detached submission state for resumed live event: {id}");
            self.submissions.insert(
                id.clone(),
                SubmissionState::Prompt(Box::new(PromptState::new_detached(
                    id.clone(),
                    self.thread.clone(),
                    self.resolution_tx.clone(),
                ))),
            );
        }

        if let Some(submission) = self.submissions.get_mut(&id) {
            submission.handle_event(&self.client, msg).await;
        } else {
            let debug_msg = format!("{msg:?}");
            if !forward_global_visible_event(&self.client, msg).await {
                warn!("Received event for unknown submission ID: {id} {debug_msg}");
            }
        }
    }
}

fn should_attach_detached_submission(msg: &EventMsg) -> bool {
    matches!(
        msg,
        EventMsg::TokenCount(..)
            | EventMsg::ItemStarted(..)
            | EventMsg::UserMessage(..)
            | EventMsg::AgentMessageContentDelta(..)
            | EventMsg::ReasoningContentDelta(..)
            | EventMsg::ReasoningRawContentDelta(..)
            | EventMsg::AgentReasoningDelta(..)
            | EventMsg::AgentReasoningRawContentDelta(..)
            | EventMsg::AgentReasoningSectionBreak(..)
            | EventMsg::AgentMessage(..)
            | EventMsg::AgentReasoning(..)
            | EventMsg::AgentReasoningRawContent(..)
            | EventMsg::PlanUpdate(..)
            | EventMsg::ExecApprovalRequest(..)
            | EventMsg::DynamicToolCallRequest(..)
            | EventMsg::DynamicToolCallResponse(..)
            | EventMsg::ExecCommandBegin(..)
            | EventMsg::ExecCommandOutputDelta(..)
            | EventMsg::TerminalInteraction(..)
            | EventMsg::ExecCommandEnd(..)
            | EventMsg::McpToolCallBegin(..)
            | EventMsg::McpToolCallEnd(..)
            | EventMsg::ApplyPatchApprovalRequest(..)
            | EventMsg::PatchApplyBegin(..)
            | EventMsg::PatchApplyEnd(..)
            | EventMsg::TurnDiff(..)
            | EventMsg::WebSearchBegin(..)
            | EventMsg::WebSearchEnd(..)
            | EventMsg::ViewImageToolCall(..)
            | EventMsg::TurnStarted(..)
            | EventMsg::TurnComplete(..)
            | EventMsg::UndoStarted(..)
            | EventMsg::UndoCompleted(..)
            | EventMsg::StreamError(..)
            | EventMsg::Error(..)
            | EventMsg::TurnAborted(..)
            | EventMsg::ShutdownComplete
            | EventMsg::EnteredReviewMode(..)
            | EventMsg::ExitedReviewMode(..)
            | EventMsg::ElicitationRequest(..)
            | EventMsg::RequestPermissions(..)
            | EventMsg::RequestUserInput(..)
            | EventMsg::ModelReroute(..)
            | EventMsg::ContextCompacted(..)
    )
}

async fn forward_global_visible_event(client: &SessionClient, msg: EventMsg) -> bool {
    match msg {
        EventMsg::Warning(WarningEvent { message }) => {
            client.send_agent_text(message).await;
            true
        }
        EventMsg::DeprecationNotice(DeprecationNoticeEvent { summary, details }) => {
            client
                .send_agent_text(render_deprecation_notice_message(
                    &summary,
                    details.as_deref(),
                ))
                .await;
            true
        }
        _ => false,
    }
}

fn render_model_reroute_message(
    from_model: &str,
    to_model: &str,
    reason: &codex_protocol::protocol::ModelRerouteReason,
) -> String {
    format!("Model rerouted: {from_model} -> {to_model} ({reason:?})")
}

fn render_deprecation_notice_message(summary: &str, details: Option<&str>) -> String {
    match details.map(str::trim).filter(|details| !details.is_empty()) {
        Some(details) => format!("Deprecation notice: {summary}\n{details}"),
        None => format!("Deprecation notice: {summary}"),
    }
}

fn request_user_input_title(questions: &[RequestUserInputQuestion]) -> String {
    if questions.len() == 1 {
        let header = questions[0].header.trim();
        if !header.is_empty() {
            return header.to_string();
        }
    }
    "Question".to_string()
}

fn render_request_user_input_prompt(questions: &[RequestUserInputQuestion]) -> String {
    let mut lines = Vec::new();
    lines.push("Codex needs input before continuing.".to_string());
    lines.push(String::new());

    for (index, question) in questions.iter().enumerate() {
        let prefix = if questions.len() > 1 {
            format!("{}. ", index + 1)
        } else {
            String::new()
        };
        let header = question.header.trim();
        if !header.is_empty() {
            lines.push(format!("{prefix}{header} ({})", question.id));
        } else if questions.len() > 1 {
            lines.push(format!("{prefix}{}", question.id));
        }
        lines.push(question.question.clone());
        if let Some(options) = question.options.as_ref() {
            for option in options {
                lines.push(format!("- {}: {}", option.label, option.description));
            }
        }
        if question.is_other {
            lines.push("- Other: provide your own answer".to_string());
        }
        if question.is_secret {
            lines.push("- Secret: your reply will not be echoed back in ACP".to_string());
        }
        lines.push(String::new());
    }

    if questions.len() == 1 {
        lines.push("Reply with your answer in the next message.".to_string());
    } else {
        lines.push(
            "Reply with a JSON object keyed by question id, or send one text block per question in order."
                .to_string(),
        );
    }

    lines.join("\n").trim().to_string()
}

fn build_request_user_input_response(
    prompt: &[ContentBlock],
    questions: &[RequestUserInputQuestion],
) -> Result<RequestUserInputResponse, Error> {
    if questions.is_empty() {
        return Err(Error::invalid_params().data("request_user_input contained no questions"));
    }

    let raw_answers = prompt_blocks_to_answer_strings(prompt)?;
    if raw_answers.is_empty() {
        return Err(Error::invalid_params().data("question answer cannot be empty"));
    }

    let answers = if questions.len() == 1 {
        let answers = parse_single_question_answers(&raw_answers)?;
        HashMap::from([(questions[0].id.clone(), RequestUserInputAnswer { answers })])
    } else {
        build_multi_question_answers(&raw_answers, questions)?
    };

    Ok(RequestUserInputResponse { answers })
}

fn parse_single_question_answers(raw_answers: &[String]) -> Result<Vec<String>, Error> {
    if raw_answers.len() == 1
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw_answers[0])
    {
        match value {
            serde_json::Value::Array(items) => {
                let answers = items
                    .into_iter()
                    .map(|item| {
                        item.as_str()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(ToOwned::to_owned)
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        Error::invalid_params()
                            .data("single-question JSON answers must be an array of strings")
                    })?;
                if !answers.is_empty() {
                    return Ok(answers);
                }
            }
            serde_json::Value::String(answer) => {
                let answer = answer.trim().to_string();
                if !answer.is_empty() {
                    return Ok(vec![answer]);
                }
            }
            _ => {}
        }
    }

    Ok(raw_answers.to_vec())
}

fn build_multi_question_answers(
    raw_answers: &[String],
    questions: &[RequestUserInputQuestion],
) -> Result<HashMap<String, RequestUserInputAnswer>, Error> {
    if raw_answers.len() == 1
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw_answers[0])
    {
        return build_multi_question_answers_from_json(value, questions);
    }

    if raw_answers.len() == questions.len() {
        return Ok(questions
            .iter()
            .zip(raw_answers)
            .map(|(question, answer)| {
                (
                    question.id.clone(),
                    RequestUserInputAnswer {
                        answers: vec![answer.clone()],
                    },
                )
            })
            .collect());
    }

    Err(Error::invalid_params().data(format!(
        "This question expects {} answers. Reply with JSON keyed by question id.",
        questions.len()
    )))
}

fn build_multi_question_answers_from_json(
    value: serde_json::Value,
    questions: &[RequestUserInputQuestion],
) -> Result<HashMap<String, RequestUserInputAnswer>, Error> {
    let serde_json::Value::Object(map) = value else {
        return Err(Error::invalid_params()
            .data("multi-question answers must be a JSON object keyed by question id"));
    };

    let mut answers = HashMap::new();
    for question in questions {
        let value = map
            .get(&question.id)
            .or_else(|| map.get(&question.header))
            .ok_or_else(|| {
                Error::invalid_params().data(format!("missing answer for question {}", question.id))
            })?;
        answers.insert(
            question.id.clone(),
            RequestUserInputAnswer {
                answers: json_value_to_answer_strings(value)?,
            },
        );
    }
    Ok(answers)
}

fn json_value_to_answer_strings(value: &serde_json::Value) -> Result<Vec<String>, Error> {
    match value {
        serde_json::Value::String(answer) => {
            let answer = answer.trim().to_string();
            if answer.is_empty() {
                return Err(Error::invalid_params().data("question answer cannot be empty"));
            }
            Ok(vec![answer])
        }
        serde_json::Value::Array(items) => {
            let answers = items
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned)
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    Error::invalid_params()
                        .data("question answer arrays must contain only non-empty strings")
                })?;
            if answers.is_empty() {
                return Err(Error::invalid_params().data("question answer cannot be empty"));
            }
            Ok(answers)
        }
        _ => {
            Err(Error::invalid_params()
                .data("question answers must be strings or arrays of strings"))
        }
    }
}

fn prompt_blocks_to_answer_strings(prompt: &[ContentBlock]) -> Result<Vec<String>, Error> {
    let mut answers = Vec::new();
    for block in prompt {
        match block {
            ContentBlock::Text(text_block) => {
                let text = text_block.text.trim();
                if !text.is_empty() {
                    answers.push(text.to_string());
                }
            }
            ContentBlock::ResourceLink(ResourceLink { name, uri, .. }) => {
                answers.push(format_uri_as_link(Some(name.clone()), uri.clone()));
            }
            ContentBlock::Resource(EmbeddedResource {
                resource:
                    EmbeddedResourceResource::TextResourceContents(TextResourceContents {
                        text, ..
                    }),
                ..
            }) => {
                let text = text.trim();
                if !text.is_empty() {
                    answers.push(text.to_string());
                }
            }
            ContentBlock::Image(_) | ContentBlock::Resource(_) => {
                return Err(Error::invalid_params()
                    .data("question answers currently support text and text resources only"));
            }
            _ => {
                return Err(Error::invalid_params()
                    .data("question answers currently support text and text resources only"));
            }
        }
    }
    Ok(answers)
}

fn build_prompt_items(prompt: Vec<ContentBlock>) -> Vec<UserInput> {
    let trusted_root = managed_skills_root(None)
        .ok()
        .and_then(|root| std::fs::canonicalize(root).ok());
    build_prompt_items_with_trusted_root(prompt, trusted_root.as_deref())
}

fn build_prompt_items_with_trusted_root(
    prompt: Vec<ContentBlock>,
    trusted_root: Option<&Path>,
) -> Vec<UserInput> {
    let canonical_trusted_root = trusted_root.and_then(|root| std::fs::canonicalize(root).ok());
    prompt
        .into_iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text_block) => Some(build_text_or_skill_input(
                text_block.text,
                trusted_root,
                canonical_trusted_root.as_deref(),
            )),
            ContentBlock::Image(image_block) => Some(UserInput::Image {
                image_url: format!("data:{};base64,{}", image_block.mime_type, image_block.data),
            }),
            ContentBlock::ResourceLink(ResourceLink { name, uri, .. }) => Some(UserInput::Text {
                text: format_uri_as_link(Some(name), uri),
                text_elements: vec![],
            }),
            ContentBlock::Resource(EmbeddedResource {
                resource:
                    EmbeddedResourceResource::TextResourceContents(TextResourceContents {
                        text,
                        uri,
                        ..
                    }),
                ..
            }) => Some(UserInput::Text {
                text: format!(
                    "{}\n<context ref=\"{uri}\">\n{text}\n</context>",
                    format_uri_as_link(None, uri.clone())
                ),
                text_elements: vec![],
            }),
            // Skip other content types for now
            ContentBlock::Audio(..) | ContentBlock::Resource(..) | _ => None,
        })
        .collect()
}

fn build_text_or_skill_input(
    text: String,
    trusted_root: Option<&Path>,
    canonical_trusted_root: Option<&Path>,
) -> UserInput {
    parse_agenthub_skill_input(&text, trusted_root, canonical_trusted_root).unwrap_or(
        UserInput::Text {
            text,
            text_elements: vec![],
        },
    )
}

fn parse_agenthub_skill_input(
    text: &str,
    trusted_root: Option<&Path>,
    canonical_trusted_root: Option<&Path>,
) -> Option<UserInput> {
    let body = text
        .strip_prefix("<skill>\n")
        .or_else(|| text.strip_prefix("<skill>\r\n"))?;
    let body = body
        .strip_suffix("\n</skill>")
        .or_else(|| body.strip_suffix("\r\n</skill>"))?;
    let (name_line, rest) = split_first_line(body)?;
    let name = parse_skill_meta_line(name_line, "name")?;
    let (path_line, _) = split_first_line(rest)?;
    let path =
        normalize_agenthub_skill_path(&parse_skill_meta_line(path_line, "path")?, trusted_root)?;
    if !is_trusted_agenthub_skill_path(&path, canonical_trusted_root) {
        return None;
    }
    Some(UserInput::Skill { name, path })
}

fn normalize_agenthub_skill_path(raw_path: &str, trusted_root: Option<&Path>) -> Option<PathBuf> {
    let path = PathBuf::from(raw_path);
    if path.is_absolute() {
        return Some(path);
    }

    let trusted_root = trusted_root?;
    let home_dir = infer_home_dir_from_managed_skills_root(trusted_root)?;

    if let Some(relative_path) = raw_path.strip_prefix("~/") {
        return Some(home_dir.join(relative_path));
    }

    let legacy_managed_relative = Path::new(".agents").join("skills").join("agenthub-runtime");
    if path.starts_with(&legacy_managed_relative) {
        return Some(home_dir.join(path));
    }

    None
}

fn infer_home_dir_from_managed_skills_root(trusted_root: &Path) -> Option<PathBuf> {
    let skills_dir = trusted_root.parent()?;
    let agents_dir = skills_dir.parent()?;
    let home_dir = agents_dir.parent()?;
    if skills_dir.file_name() != Some(OsStr::new("skills"))
        || agents_dir.file_name() != Some(OsStr::new(".agents"))
    {
        return None;
    }
    Some(home_dir.to_path_buf())
}

fn is_trusted_agenthub_skill_path(path: &Path, trusted_root: Option<&Path>) -> bool {
    if !path.is_absolute() || path.file_name() != Some(OsStr::new("SKILL.md")) {
        return false;
    }
    let Some(trusted_root) = trusted_root else {
        return false;
    };
    let Ok(canonical_path) = std::fs::canonicalize(path) else {
        return false;
    };
    canonical_path.starts_with(trusted_root)
}

fn split_first_line(input: &str) -> Option<(&str, &str)> {
    let newline = input.find('\n')?;
    let (line, rest) = input.split_at(newline);
    Some((line.strip_suffix('\r').unwrap_or(line), &rest[1..]))
}

fn parse_skill_meta_line(line: &str, tag: &str) -> Option<String> {
    let open_tag = format!("<{tag}>");
    let close_tag = format!("</{tag}>");
    let value = line.strip_prefix(&open_tag)?.strip_suffix(&close_tag)?;
    Some(unescape_skill_meta(value))
}

fn unescape_skill_meta(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn format_uri_as_link(name: Option<String>, uri: String) -> String {
    if let Some(name) = name
        && !name.is_empty()
    {
        format!("[@{name}]({uri})")
    } else if let Some(path) = uri.strip_prefix("file://") {
        let name = path.split('/').next_back().unwrap_or(path);
        format!("[@{name}]({uri})")
    } else if uri.starts_with("zed://") {
        let name = uri.split('/').next_back().unwrap_or(&uri);
        format!("[@{name}]({uri})")
    } else {
        uri
    }
}

fn extract_tool_call_content_from_changes(
    changes: HashMap<PathBuf, FileChange>,
) -> (
    String,
    Vec<ToolCallLocation>,
    impl Iterator<Item = ToolCallContent>,
) {
    (
        format!(
            "Edit {}",
            changes.keys().map(|p| p.display().to_string()).join(", ")
        ),
        changes.keys().map(ToolCallLocation::new).collect(),
        changes.into_iter().map(|(path, change)| {
            ToolCallContent::Diff(match change {
                codex_protocol::protocol::FileChange::Add { content } => Diff::new(path, content),
                codex_protocol::protocol::FileChange::Delete { content } => {
                    Diff::new(path, String::new()).old_text(content)
                }
                codex_protocol::protocol::FileChange::Update {
                    unified_diff,
                    move_path,
                } => Diff::new(move_path.unwrap_or(path), unified_diff),
            })
        }),
    )
}

/// Extract title and call_id from a WebSearchAction (used for replay)
fn web_search_action_to_title_and_id(
    id: &Option<String>,
    action: &codex_protocol::models::WebSearchAction,
) -> (String, String) {
    match action {
        codex_protocol::models::WebSearchAction::Search { query, queries } => {
            let title = queries
                .as_ref()
                .map(|q| q.join(", "))
                .or_else(|| query.clone())
                .unwrap_or_else(|| "Web search".to_string());
            let call_id = id
                .clone()
                .unwrap_or_else(|| generate_fallback_id("web_search"));
            (title, call_id)
        }
        codex_protocol::models::WebSearchAction::OpenPage { url } => {
            let title = url.clone().unwrap_or_else(|| "Open page".to_string());
            let call_id = id
                .clone()
                .unwrap_or_else(|| generate_fallback_id("web_open"));
            (title, call_id)
        }
        codex_protocol::models::WebSearchAction::FindInPage { pattern, .. } => {
            let title = pattern
                .clone()
                .unwrap_or_else(|| "Find in page".to_string());
            let call_id = id
                .clone()
                .unwrap_or_else(|| generate_fallback_id("web_find"));
            (title, call_id)
        }
        codex_protocol::models::WebSearchAction::Other => {
            ("Unknown".to_string(), generate_fallback_id("web_search"))
        }
    }
}

/// Generate a fallback ID using UUID (used when id is missing)
fn generate_fallback_id(prefix: &str) -> String {
    format!("{}_{}", prefix, Uuid::new_v4())
}

/// Checks if a prompt is slash command
fn extract_slash_command(content: &[UserInput]) -> Option<(&str, &str)> {
    let line = content.first().and_then(|block| match block {
        UserInput::Text { text, .. } => Some(text),
        _ => None,
    })?;

    parse_slash_name(line)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use agent_client_protocol_legacy::{RequestPermissionResponse, TextContent};
    use agenthub_managed_skills::{
        ManagedSkillKind, install_managed_skills, managed_skill_doc_path, managed_skills_root,
    };
    use codex_core::test_support::all_model_presets;
    use codex_protocol::config_types::ModeKind;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use tokio::{
        sync::{Mutex, Notify, mpsc::UnboundedSender},
        task::LocalSet,
    };

    use super::*;

    struct TempManagedSkillsHome {
        home: PathBuf,
    }

    impl TempManagedSkillsHome {
        fn new() -> Self {
            let home = std::env::temp_dir().join(format!(
                "agenthub-codex-acp-managed-skills-{}-{}",
                std::process::id(),
                Uuid::new_v4()
            ));
            std::fs::create_dir_all(&home).expect("create temp managed skills home");
            install_managed_skills(Some(home.as_path())).expect("install managed skills");
            Self { home }
        }

        fn trusted_root(&self) -> PathBuf {
            managed_skills_root(Some(self.home.as_path())).expect("resolve managed skills root")
        }

        fn skill_path(&self, kind: ManagedSkillKind) -> PathBuf {
            managed_skill_doc_path(kind, Some(self.home.as_path()))
                .expect("resolve managed skill path")
        }
    }

    impl Drop for TempManagedSkillsHome {
        fn drop(&mut self) {
            drop(std::fs::remove_dir_all(&self.home));
        }
    }

    fn test_config() -> anyhow::Result<Config> {
        Config::load_default_with_cli_overrides(vec![]).map_err(Into::into)
    }

    fn current_dir_abs() -> anyhow::Result<AbsolutePathBuf> {
        AbsolutePathBuf::from_absolute_path(std::env::current_dir()?).map_err(Into::into)
    }

    #[tokio::test]
    async fn test_prompt() -> anyhow::Result<()> {
        let (session_id, client, _, message_tx, local_set) = setup().await?;
        let (prompt_response_tx, prompt_response_rx) = tokio::sync::oneshot::channel();

        message_tx.send(ThreadMessage::Prompt {
            request: PromptRequest::new(session_id.clone(), vec!["Hi".into()]),
            response_tx: prompt_response_tx,
        })?;

        tokio::try_join!(
            async {
                let stop_reason = prompt_response_rx.await??.await??;
                assert_eq!(stop_reason, StopReason::EndTurn);
                drop(message_tx);
                anyhow::Ok(())
            },
            async {
                local_set.await;
                anyhow::Ok(())
            }
        )?;

        let notifications = client.notifications.lock().unwrap();
        assert_eq!(notifications.len(), 1);
        assert!(matches!(
            &notifications[0].update,
            SessionUpdate::AgentMessageChunk(ContentChunk {
                content: ContentBlock::Text(TextContent { text, .. }),
                ..
            }) if text == "Hi"
        ));

        Ok(())
    }

    #[tokio::test]
    async fn test_compact() -> anyhow::Result<()> {
        let (session_id, client, thread, message_tx, local_set) = setup().await?;
        let (prompt_response_tx, prompt_response_rx) = tokio::sync::oneshot::channel();

        message_tx.send(ThreadMessage::Prompt {
            request: PromptRequest::new(session_id.clone(), vec!["/compact".into()]),
            response_tx: prompt_response_tx,
        })?;

        tokio::try_join!(
            async {
                let stop_reason = prompt_response_rx.await??.await??;
                assert_eq!(stop_reason, StopReason::EndTurn);
                drop(message_tx);
                anyhow::Ok(())
            },
            async {
                local_set.await;
                anyhow::Ok(())
            }
        )?;

        let notifications = client.notifications.lock().unwrap();
        assert_eq!(notifications.len(), 1);
        assert!(matches!(
            &notifications[0].update,
            SessionUpdate::AgentMessageChunk(ContentChunk {
                content: ContentBlock::Text(TextContent { text, .. }),
                ..
            }) if text == "Compact task completed"
        ));
        let ops = thread.ops.lock().unwrap();
        assert_eq!(ops.as_slice(), &[Op::Compact]);

        Ok(())
    }

    #[tokio::test]
    async fn test_undo() -> anyhow::Result<()> {
        let (session_id, client, thread, message_tx, local_set) = setup().await?;
        let (prompt_response_tx, prompt_response_rx) = tokio::sync::oneshot::channel();

        message_tx.send(ThreadMessage::Prompt {
            request: PromptRequest::new(session_id.clone(), vec!["/undo".into()]),
            response_tx: prompt_response_tx,
        })?;

        tokio::try_join!(
            async {
                let stop_reason = prompt_response_rx.await??.await??;
                assert_eq!(stop_reason, StopReason::EndTurn);
                drop(message_tx);
                anyhow::Ok(())
            },
            async {
                local_set.await;
                anyhow::Ok(())
            }
        )?;

        let notifications = client.notifications.lock().unwrap();
        assert_eq!(
            notifications.len(),
            2,
            "notifications don't match {notifications:?}"
        );
        assert!(matches!(
            &notifications[0].update,
            SessionUpdate::AgentMessageChunk(ContentChunk {
                content: ContentBlock::Text(TextContent { text, .. }),
                ..
            }) if text == "Undo in progress..."
        ));
        assert!(matches!(
            &notifications[1].update,
            SessionUpdate::AgentMessageChunk(ContentChunk {
                content: ContentBlock::Text(TextContent { text, .. }),
                ..
            }) if text == "Undo completed."
        ));

        let ops = thread.ops.lock().unwrap();
        assert_eq!(ops.as_slice(), &[Op::Undo]);

        Ok(())
    }

    #[tokio::test]
    async fn test_init() -> anyhow::Result<()> {
        let (session_id, client, thread, message_tx, local_set) = setup().await?;
        let (prompt_response_tx, prompt_response_rx) = tokio::sync::oneshot::channel();

        message_tx.send(ThreadMessage::Prompt {
            request: PromptRequest::new(session_id.clone(), vec!["/init".into()]),
            response_tx: prompt_response_tx,
        })?;

        tokio::try_join!(
            async {
                let stop_reason = prompt_response_rx.await??.await??;
                assert_eq!(stop_reason, StopReason::EndTurn);
                drop(message_tx);
                anyhow::Ok(())
            },
            async {
                local_set.await;
                anyhow::Ok(())
            }
        )?;

        let notifications = client.notifications.lock().unwrap();
        assert_eq!(notifications.len(), 1);
        assert!(
            matches!(
                &notifications[0].update,
                SessionUpdate::AgentMessageChunk(ContentChunk {
                    content: ContentBlock::Text(TextContent { text, .. }), ..
                }) if text == INIT_COMMAND_PROMPT // we echo the prompt
            ),
            "notifications don't match {notifications:?}"
        );
        let ops = thread.ops.lock().unwrap();
        assert_eq!(
            ops.as_slice(),
            &[Op::UserInput {
                items: vec![UserInput::Text {
                    text: INIT_COMMAND_PROMPT.to_string(),
                    text_elements: vec![]
                }],
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
            }],
            "ops don't match {ops:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_review() -> anyhow::Result<()> {
        let (session_id, client, thread, message_tx, local_set) = setup().await?;
        let (prompt_response_tx, prompt_response_rx) = tokio::sync::oneshot::channel();

        message_tx.send(ThreadMessage::Prompt {
            request: PromptRequest::new(session_id.clone(), vec!["/review".into()]),
            response_tx: prompt_response_tx,
        })?;

        tokio::try_join!(
            async {
                let stop_reason = prompt_response_rx.await??.await??;
                assert_eq!(stop_reason, StopReason::EndTurn);
                drop(message_tx);
                anyhow::Ok(())
            },
            async {
                local_set.await;
                anyhow::Ok(())
            }
        )?;

        let notifications = client.notifications.lock().unwrap();
        assert_eq!(notifications.len(), 1);
        assert!(
            matches!(
                &notifications[0].update,
                SessionUpdate::AgentMessageChunk(ContentChunk {
                    content: ContentBlock::Text(TextContent { text, .. }),
                    ..
                }) if text == "current changes" // we echo the prompt
            ),
            "notifications don't match {notifications:?}"
        );

        let ops = thread.ops.lock().unwrap();
        assert_eq!(
            ops.as_slice(),
            &[Op::Review {
                review_request: ReviewRequest {
                    user_facing_hint: Some(user_facing_hint(&ReviewTarget::UncommittedChanges)),
                    target: ReviewTarget::UncommittedChanges,
                }
            }],
            "ops don't match {ops:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_custom_review() -> anyhow::Result<()> {
        let (session_id, client, thread, message_tx, local_set) = setup().await?;
        let (prompt_response_tx, prompt_response_rx) = tokio::sync::oneshot::channel();
        let instructions = "Review what we did in agents.md";

        message_tx.send(ThreadMessage::Prompt {
            request: PromptRequest::new(
                session_id.clone(),
                vec![format!("/review {instructions}").into()],
            ),
            response_tx: prompt_response_tx,
        })?;

        tokio::try_join!(
            async {
                let stop_reason = prompt_response_rx.await??.await??;
                assert_eq!(stop_reason, StopReason::EndTurn);
                drop(message_tx);
                anyhow::Ok(())
            },
            async {
                local_set.await;
                anyhow::Ok(())
            }
        )?;

        let notifications = client.notifications.lock().unwrap();
        assert_eq!(notifications.len(), 1);
        assert!(
            matches!(
                &notifications[0].update,
                SessionUpdate::AgentMessageChunk(ContentChunk {
                    content: ContentBlock::Text(TextContent { text, .. }),
                    ..
                }) if text == "Review what we did in agents.md" // we echo the prompt
            ),
            "notifications don't match {notifications:?}"
        );

        let ops = thread.ops.lock().unwrap();
        assert_eq!(
            ops.as_slice(),
            &[Op::Review {
                review_request: ReviewRequest {
                    user_facing_hint: Some(user_facing_hint(&ReviewTarget::Custom {
                        instructions: instructions.to_owned()
                    })),
                    target: ReviewTarget::Custom {
                        instructions: instructions.to_owned()
                    },
                }
            }],
            "ops don't match {ops:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_commit_review() -> anyhow::Result<()> {
        let (session_id, client, thread, message_tx, local_set) = setup().await?;
        let (prompt_response_tx, prompt_response_rx) = tokio::sync::oneshot::channel();

        message_tx.send(ThreadMessage::Prompt {
            request: PromptRequest::new(session_id.clone(), vec!["/review-commit 123456".into()]),
            response_tx: prompt_response_tx,
        })?;

        tokio::try_join!(
            async {
                let stop_reason = prompt_response_rx.await??.await??;
                assert_eq!(stop_reason, StopReason::EndTurn);
                drop(message_tx);
                anyhow::Ok(())
            },
            async {
                local_set.await;
                anyhow::Ok(())
            }
        )?;

        let notifications = client.notifications.lock().unwrap();
        assert_eq!(notifications.len(), 1);
        assert!(
            matches!(
                &notifications[0].update,
                SessionUpdate::AgentMessageChunk(ContentChunk {
                    content: ContentBlock::Text(TextContent { text, .. }),
                    ..
                }) if text == "commit 123456" // we echo the prompt
            ),
            "notifications don't match {notifications:?}"
        );

        let ops = thread.ops.lock().unwrap();
        assert_eq!(
            ops.as_slice(),
            &[Op::Review {
                review_request: ReviewRequest {
                    user_facing_hint: Some(user_facing_hint(&ReviewTarget::Commit {
                        sha: "123456".to_owned(),
                        title: None
                    })),
                    target: ReviewTarget::Commit {
                        sha: "123456".to_owned(),
                        title: None
                    },
                }
            }],
            "ops don't match {ops:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_branch_review() -> anyhow::Result<()> {
        let (session_id, client, thread, message_tx, local_set) = setup().await?;
        let (prompt_response_tx, prompt_response_rx) = tokio::sync::oneshot::channel();

        message_tx.send(ThreadMessage::Prompt {
            request: PromptRequest::new(session_id.clone(), vec!["/review-branch feature".into()]),
            response_tx: prompt_response_tx,
        })?;

        tokio::try_join!(
            async {
                let stop_reason = prompt_response_rx.await??.await??;
                assert_eq!(stop_reason, StopReason::EndTurn);
                drop(message_tx);
                anyhow::Ok(())
            },
            async {
                local_set.await;
                anyhow::Ok(())
            }
        )?;

        let notifications = client.notifications.lock().unwrap();
        assert_eq!(notifications.len(), 1);
        assert!(
            matches!(
                &notifications[0].update,
                SessionUpdate::AgentMessageChunk(ContentChunk {
                    content: ContentBlock::Text(TextContent { text, .. }),
                    ..
                }) if text == "changes against 'feature'" // we echo the prompt
            ),
            "notifications don't match {notifications:?}"
        );

        let ops = thread.ops.lock().unwrap();
        assert_eq!(
            ops.as_slice(),
            &[Op::Review {
                review_request: ReviewRequest {
                    user_facing_hint: Some(user_facing_hint(&ReviewTarget::BaseBranch {
                        branch: "feature".to_owned()
                    })),
                    target: ReviewTarget::BaseBranch {
                        branch: "feature".to_owned()
                    },
                }
            }],
            "ops don't match {ops:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_unknown_slash_command_is_forwarded_as_plain_input() -> anyhow::Result<()> {
        let (session_id, client, thread, message_tx, local_set) = setup().await?;
        let (prompt_response_tx, prompt_response_rx) = tokio::sync::oneshot::channel();

        message_tx.send(ThreadMessage::Prompt {
            request: PromptRequest::new(session_id.clone(), vec!["/custom foo".into()]),
            response_tx: prompt_response_tx,
        })?;

        tokio::try_join!(
            async {
                let stop_reason = prompt_response_rx.await??.await??;
                assert_eq!(stop_reason, StopReason::EndTurn);
                drop(message_tx);
                anyhow::Ok(())
            },
            async {
                local_set.await;
                anyhow::Ok(())
            }
        )?;

        let notifications = client.notifications.lock().unwrap();
        assert_eq!(notifications.len(), 1);
        assert!(
            matches!(
                &notifications[0].update,
                SessionUpdate::AgentMessageChunk(ContentChunk {
                    content: ContentBlock::Text(TextContent { text, .. }),
                    ..
                }) if text == "/custom foo"
            ),
            "notifications don't match {notifications:?}"
        );

        let ops = thread.ops.lock().unwrap();
        assert_eq!(
            ops.as_slice(),
            &[Op::UserInput {
                items: vec![UserInput::Text {
                    text: "/custom foo".into(),
                    text_elements: vec![]
                }],
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
            }],
            "ops don't match {ops:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_shared_submission_id_completes_all_prompt_waiters() -> anyhow::Result<()> {
        LocalSet::new()
            .run_until(async {
                let session_id = SessionId::new("test");
                let client = Arc::new(StubClient::new());
                let session_client =
                    SessionClient::with_client(session_id.clone(), client, Arc::default());
                let thread = Arc::new(SharedSubmissionThread::new("shared-submission"));
                let models_manager = Arc::new(StubModelsManager);
                let config = test_config()?;
                let (message_tx, message_rx) = tokio::sync::mpsc::unbounded_channel();
                let (resolution_tx, resolution_rx) = tokio::sync::mpsc::unbounded_channel();

                let actor = ThreadActor::new(
                    StubAuth,
                    session_client,
                    thread.clone(),
                    models_manager,
                    config,
                    message_rx,
                    resolution_tx,
                    resolution_rx,
                );
                tokio::task::spawn_local(actor.spawn());

                let (first_response_tx, first_response_rx) = tokio::sync::oneshot::channel();
                message_tx.send(ThreadMessage::Prompt {
                    request: PromptRequest::new(session_id.clone(), vec!["first".into()]),
                    response_tx: first_response_tx,
                })?;
                let first_stop_rx = first_response_rx.await??;

                let (second_response_tx, second_response_rx) = tokio::sync::oneshot::channel();
                message_tx.send(ThreadMessage::Prompt {
                    request: PromptRequest::new(session_id.clone(), vec!["second".into()]),
                    response_tx: second_response_tx,
                })?;
                let second_stop_rx = second_response_rx.await??;

                thread.emit(EventMsg::TurnComplete(TurnCompleteEvent {
                    last_agent_message: Some("done".to_string()),
                    turn_id: "shared-turn".to_string(),
                    completed_at: None,
                    duration_ms: None,
                }));

                assert_eq!(first_stop_rx.await??, StopReason::EndTurn);
                assert_eq!(second_stop_rx.await??, StopReason::EndTurn);

                let ops = thread.ops.lock().unwrap();
                assert_eq!(ops.len(), 2, "ops don't match {ops:?}");
                assert!(matches!(ops[0], Op::UserInput { .. }));
                assert!(matches!(ops[1], Op::UserInput { .. }));

                drop(message_tx);
                anyhow::Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_unknown_live_event_attaches_detached_submission() -> anyhow::Result<()> {
        LocalSet::new()
            .run_until(async {
                let session_id = SessionId::new("test");
                let client = Arc::new(StubClient::new());
                let session_client =
                    SessionClient::with_client(session_id, client.clone(), Arc::default());
                let thread = Arc::new(StubCodexThread::new());
                let models_manager = Arc::new(StubModelsManager);
                let config = test_config()?;
                let (_message_tx, message_rx) = tokio::sync::mpsc::unbounded_channel();
                let (resolution_tx, resolution_rx) = tokio::sync::mpsc::unbounded_channel();
                let mut actor = ThreadActor::new(
                    StubAuth,
                    session_client,
                    thread,
                    models_manager,
                    config,
                    message_rx,
                    resolution_tx,
                    resolution_rx,
                );

                actor
                    .handle_event(Event {
                        id: "resumed-turn".to_string(),
                        msg: EventMsg::AgentMessage(AgentMessageEvent {
                            message: "resumed output".to_string(),
                            phase: None,
                            memory_citation: None,
                        }),
                    })
                    .await;

                assert!(actor.submissions.contains_key("resumed-turn"));
                assert!(
                    actor
                        .submissions
                        .get("resumed-turn")
                        .is_some_and(SubmissionState::is_active)
                );

                actor
                    .handle_event(Event {
                        id: "resumed-turn".to_string(),
                        msg: EventMsg::TurnComplete(TurnCompleteEvent {
                            last_agent_message: Some("resumed output".to_string()),
                            turn_id: "resumed-turn".to_string(),
                            completed_at: None,
                            duration_ms: None,
                        }),
                    })
                    .await;

                assert!(
                    actor
                        .submissions
                        .get("resumed-turn")
                        .is_some_and(|submission| !submission.is_active())
                );

                let notifications = client.notifications.lock().unwrap();
                assert!(notifications.iter().any(|notification| {
                    matches!(
                        &notification.update,
                        SessionUpdate::AgentMessageChunk(ContentChunk {
                            content: ContentBlock::Text(TextContent { text, .. }),
                            ..
                        }) if text == "resumed output"
                    )
                }));

                anyhow::Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_global_visible_events_without_submission_are_forwarded() -> anyhow::Result<()> {
        let session_id = SessionId::new("test");
        let client = Arc::new(StubClient::new());
        let session_client = SessionClient::with_client(session_id, client.clone(), Arc::default());
        let thread = Arc::new(StubCodexThread::new());
        let models_manager = Arc::new(StubModelsManager);
        let config = test_config()?;
        let (_message_tx, message_rx) = tokio::sync::mpsc::unbounded_channel();
        let (resolution_tx, resolution_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut actor = ThreadActor::new(
            StubAuth,
            session_client,
            thread,
            models_manager,
            config,
            message_rx,
            resolution_tx,
            resolution_rx,
        );

        actor
            .handle_event(Event {
                id: "app-server".to_string(),
                msg: EventMsg::Warning(WarningEvent {
                    message: "Config warning: invalid profile".to_string(),
                }),
            })
            .await;
        actor
            .handle_event(Event {
                id: "app-server".to_string(),
                msg: EventMsg::DeprecationNotice(DeprecationNoticeEvent {
                    summary: "old field is deprecated".to_string(),
                    details: Some("Use new_field instead.".to_string()),
                }),
            })
            .await;

        let notifications = client.notifications.lock().unwrap();
        assert!(notifications.iter().any(|notification| {
            matches!(
                &notification.update,
                SessionUpdate::AgentMessageChunk(ContentChunk {
                    content: ContentBlock::Text(TextContent { text, .. }),
                    ..
                }) if text == "Config warning: invalid profile"
            )
        }));
        assert!(notifications.iter().any(|notification| {
            matches!(
                &notification.update,
                SessionUpdate::AgentMessageChunk(ContentChunk {
                    content: ContentBlock::Text(TextContent { text, .. }),
                    ..
                }) if text == "Deprecation notice: old field is deprecated\nUse new_field instead."
            )
        }));

        Ok(())
    }

    #[test]
    fn test_build_request_user_input_response_maps_multi_question_json_answers() {
        let response = build_request_user_input_response(
            &[ContentBlock::Text(TextContent::new(
                r#"{"question-1":"yes","question-2":["alpha","beta"]}"#,
            ))],
            &[
                RequestUserInputQuestion {
                    id: "question-1".to_string(),
                    header: "First".to_string(),
                    question: "Pick one".to_string(),
                    is_other: false,
                    is_secret: false,
                    options: None,
                },
                RequestUserInputQuestion {
                    id: "question-2".to_string(),
                    header: "Second".to_string(),
                    question: "Pick many".to_string(),
                    is_other: false,
                    is_secret: false,
                    options: None,
                },
            ],
        )
        .expect("parse multi-question response");

        assert_eq!(
            response.answers["question-1"].answers,
            vec!["yes".to_string()]
        );
        assert_eq!(
            response.answers["question-2"].answers,
            vec!["alpha".to_string(), "beta".to_string()]
        );
    }

    #[tokio::test]
    async fn test_prompt_answer_resolves_pending_request_user_input() -> anyhow::Result<()> {
        LocalSet::new()
            .run_until(async {
                let session_id = SessionId::new("test");
                let client = Arc::new(StubClient::new());
                let session_client =
                    SessionClient::with_client(session_id.clone(), client.clone(), Arc::default());
                let thread = Arc::new(SharedSubmissionThread::new("shared-submission"));
                let models_manager = Arc::new(StubModelsManager);
                let config = test_config()?;
                let (message_tx, message_rx) = tokio::sync::mpsc::unbounded_channel();
                let (resolution_tx, resolution_rx) = tokio::sync::mpsc::unbounded_channel();

                let actor = ThreadActor::new(
                    StubAuth,
                    session_client,
                    thread.clone(),
                    models_manager,
                    config,
                    message_rx,
                    resolution_tx,
                    resolution_rx,
                );
                tokio::task::spawn_local(actor.spawn());

                let (first_response_tx, first_response_rx) = tokio::sync::oneshot::channel();
                message_tx.send(ThreadMessage::Prompt {
                    request: PromptRequest::new(session_id.clone(), vec!["first".into()]),
                    response_tx: first_response_tx,
                })?;
                let first_stop_rx = first_response_rx.await??;

                thread.emit(EventMsg::RequestUserInput(RequestUserInputEvent {
                    call_id: "question-call".to_string(),
                    turn_id: "shared-submission".to_string(),
                    questions: vec![RequestUserInputQuestion {
                        id: "question-1".to_string(),
                        header: "Clarify".to_string(),
                        question: "Pick one".to_string(),
                        is_other: true,
                        is_secret: false,
                        options: None,
                    }],
                }));
                tokio::task::yield_now().await;

                let (answer_response_tx, answer_response_rx) = tokio::sync::oneshot::channel();
                message_tx.send(ThreadMessage::Prompt {
                    request: PromptRequest::new(session_id.clone(), vec!["approved".into()]),
                    response_tx: answer_response_tx,
                })?;
                let answer_stop_rx = answer_response_rx.await??;

                {
                    let ops = thread.ops.lock().unwrap();
                    assert_eq!(ops.len(), 2, "ops don't match {ops:?}");
                    assert!(matches!(ops[0], Op::UserInput { .. }));
                    match &ops[1] {
                        Op::UserInputAnswer { id, response } => {
                            assert_eq!(id, "shared-submission");
                            assert_eq!(
                                response.answers["question-1"].answers,
                                vec!["approved".to_string()]
                            );
                        }
                        other => panic!("unexpected op: {other:?}"),
                    }
                }

                {
                    let notifications = client.notifications.lock().unwrap();
                    assert!(notifications.iter().any(|notification| {
                        matches!(
                            &notification.update,
                            SessionUpdate::ToolCall(tool_call)
                                if tool_call.title == "Clarify"
                                    && tool_call.status == ToolCallStatus::Pending
                        )
                    }));
                    assert!(notifications.iter().any(|notification| {
                        matches!(
                            &notification.update,
                            SessionUpdate::ToolCallUpdate(update)
                                if update.tool_call_id.0.as_ref()
                                    == "request-user-input:question-call"
                                    && update.fields.status == Some(ToolCallStatus::Completed)
                        )
                    }));
                }

                thread.emit(EventMsg::TurnComplete(TurnCompleteEvent {
                    last_agent_message: Some("done".to_string()),
                    turn_id: "shared-turn".to_string(),
                    completed_at: None,
                    duration_ms: None,
                }));

                assert_eq!(first_stop_rx.await??, StopReason::EndTurn);
                assert_eq!(answer_stop_rx.await??, StopReason::EndTurn);

                drop(message_tx);
                anyhow::Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_delta_deduplication() -> anyhow::Result<()> {
        let (session_id, client, _, message_tx, local_set) = setup().await?;
        let (prompt_response_tx, prompt_response_rx) = tokio::sync::oneshot::channel();

        message_tx.send(ThreadMessage::Prompt {
            request: PromptRequest::new(session_id.clone(), vec!["test delta".into()]),
            response_tx: prompt_response_tx,
        })?;

        tokio::try_join!(
            async {
                let stop_reason = prompt_response_rx.await??.await??;
                assert_eq!(stop_reason, StopReason::EndTurn);
                drop(message_tx);
                anyhow::Ok(())
            },
            async {
                local_set.await;
                anyhow::Ok(())
            }
        )?;

        // We should only get ONE notification, not duplicates from both delta and non-delta
        let notifications = client.notifications.lock().unwrap();
        assert_eq!(
            notifications.len(),
            1,
            "Should only receive delta event, not duplicate non-delta. Got: {notifications:?}"
        );
        assert!(matches!(
            &notifications[0].update,
            SessionUpdate::AgentMessageChunk(ContentChunk {
                content: ContentBlock::Text(TextContent { text, .. }),
                ..
            }) if text == "test delta"
        ));

        Ok(())
    }

    async fn setup() -> anyhow::Result<(
        SessionId,
        Arc<StubClient>,
        Arc<StubCodexThread>,
        UnboundedSender<ThreadMessage>,
        LocalSet,
    )> {
        let session_id = SessionId::new("test");
        let client = Arc::new(StubClient::new());
        let session_client =
            SessionClient::with_client(session_id.clone(), client.clone(), Arc::default());
        let conversation = Arc::new(StubCodexThread::new());
        let models_manager = Arc::new(StubModelsManager);
        let config = test_config()?;
        let (message_tx, message_rx) = tokio::sync::mpsc::unbounded_channel();
        let (resolution_tx, resolution_rx) = tokio::sync::mpsc::unbounded_channel();

        let actor = ThreadActor::new(
            StubAuth,
            session_client,
            conversation.clone(),
            models_manager,
            config,
            message_rx,
            resolution_tx,
            resolution_rx,
        );

        let local_set = LocalSet::new();
        local_set.spawn_local(actor.spawn());
        Ok((session_id, client, conversation, message_tx, local_set))
    }

    struct StubAuth;

    impl Auth for StubAuth {
        fn logout(&self) -> Result<bool, Error> {
            Ok(true)
        }
    }

    struct StubModelsManager;

    #[async_trait::async_trait]
    impl ModelsManagerImpl for StubModelsManager {
        async fn get_model(&self, _model_id: &Option<String>) -> String {
            all_model_presets()[0].to_owned().id
        }

        async fn list_models(&self) -> Vec<ModelPreset> {
            all_model_presets().to_owned()
        }
    }

    struct StubCodexThread {
        current_id: AtomicUsize,
        active_prompt_id: std::sync::Mutex<Option<String>>,
        ops: std::sync::Mutex<Vec<Op>>,
        submission_ids: std::sync::Mutex<Vec<String>>,
        op_tx: mpsc::UnboundedSender<Event>,
        op_rx: Mutex<mpsc::UnboundedReceiver<Event>>,
    }

    impl StubCodexThread {
        fn new() -> Self {
            let (op_tx, op_rx) = mpsc::unbounded_channel();
            StubCodexThread {
                current_id: AtomicUsize::new(0),
                active_prompt_id: std::sync::Mutex::default(),
                ops: std::sync::Mutex::default(),
                submission_ids: std::sync::Mutex::default(),
                op_tx,
                op_rx: Mutex::new(op_rx),
            }
        }
    }

    #[async_trait::async_trait]
    impl CodexThreadImpl for StubCodexThread {
        async fn submit(&self, submission_id: String, op: Op) -> Result<String, CodexErr> {
            let id = self
                .current_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            self.submission_ids
                .lock()
                .unwrap()
                .push(submission_id.clone());
            self.ops.lock().unwrap().push(op.clone());

            match op {
                Op::UserInput { items, .. } => {
                    *self.active_prompt_id.lock().unwrap() = Some(submission_id.clone());
                    let prompt = items
                        .into_iter()
                        .map(|i| match i {
                            UserInput::Text { text, .. } => text,
                            _ => unimplemented!(),
                        })
                        .join("\n");

                    if prompt == "parallel-exec" {
                        // Emit interleaved exec events: Begin A, Begin B, End A, End B
                        let turn_id = id.to_string();
                        let cwd = current_dir_abs().unwrap();
                        let send = |msg| {
                            self.op_tx
                                .send(Event {
                                    id: submission_id.clone(),
                                    msg,
                                })
                                .unwrap();
                        };
                        send(EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                            call_id: "call-a".into(),
                            process_id: None,
                            turn_id: turn_id.clone(),
                            command: vec!["echo".into(), "a".into()],
                            cwd: cwd.clone(),
                            parsed_cmd: vec![ParsedCommand::Unknown {
                                cmd: "echo a".into(),
                            }],
                            source: Default::default(),
                            interaction_input: None,
                        }));
                        send(EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                            call_id: "call-b".into(),
                            process_id: None,
                            turn_id: turn_id.clone(),
                            command: vec!["echo".into(), "b".into()],
                            cwd: cwd.clone(),
                            parsed_cmd: vec![ParsedCommand::Unknown {
                                cmd: "echo b".into(),
                            }],
                            source: Default::default(),
                            interaction_input: None,
                        }));
                        send(EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                            call_id: "call-a".into(),
                            process_id: None,
                            turn_id: turn_id.clone(),
                            command: vec!["echo".into(), "a".into()],
                            cwd: cwd.clone(),
                            parsed_cmd: vec![],
                            source: Default::default(),
                            interaction_input: None,
                            stdout: "a\n".into(),
                            stderr: String::new(),
                            aggregated_output: "a\n".into(),
                            exit_code: 0,
                            duration: std::time::Duration::from_millis(10),
                            formatted_output: "a\n".into(),
                            status: ExecCommandStatus::Completed,
                        }));
                        send(EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                            call_id: "call-b".into(),
                            process_id: None,
                            turn_id: turn_id.clone(),
                            command: vec!["echo".into(), "b".into()],
                            cwd: cwd.clone(),
                            parsed_cmd: vec![],
                            source: Default::default(),
                            interaction_input: None,
                            stdout: "b\n".into(),
                            stderr: String::new(),
                            aggregated_output: "b\n".into(),
                            exit_code: 0,
                            duration: std::time::Duration::from_millis(10),
                            formatted_output: "b\n".into(),
                            status: ExecCommandStatus::Completed,
                        }));
                        send(EventMsg::TurnComplete(TurnCompleteEvent {
                            last_agent_message: None,
                            turn_id,
                            completed_at: None,
                            duration_ms: None,
                        }));
                    } else if prompt == "approval-block" {
                        self.op_tx
                            .send(Event {
                                id: submission_id.clone(),
                                msg: EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                                    call_id: "call-id".to_string(),
                                    approval_id: Some("approval-id".to_string()),
                                    turn_id: id.to_string(),
                                    command: vec!["echo".to_string(), "hi".to_string()],
                                    cwd: current_dir_abs().unwrap(),
                                    reason: None,
                                    network_approval_context: None,
                                    proposed_execpolicy_amendment: None,
                                    proposed_network_policy_amendments: None,
                                    additional_permissions: None,
                                    available_decisions: Some(vec![
                                        ReviewDecision::Approved,
                                        ReviewDecision::Abort,
                                    ]),
                                    parsed_cmd: vec![ParsedCommand::Unknown {
                                        cmd: "echo hi".to_string(),
                                    }],
                                }),
                            })
                            .unwrap();
                    } else {
                        self.op_tx
                            .send(Event {
                                id: submission_id.clone(),
                                msg: EventMsg::AgentMessageContentDelta(
                                    AgentMessageContentDeltaEvent {
                                        thread_id: id.to_string(),
                                        turn_id: id.to_string(),
                                        item_id: id.to_string(),
                                        delta: prompt.clone(),
                                    },
                                ),
                            })
                            .unwrap();
                        // Send non-delta event (should be deduplicated, but handled by deduplication)
                        self.op_tx
                            .send(Event {
                                id: submission_id.clone(),
                                msg: EventMsg::AgentMessage(AgentMessageEvent {
                                    message: prompt,
                                    phase: None,
                                    memory_citation: None,
                                }),
                            })
                            .unwrap();
                        self.op_tx
                            .send(Event {
                                id: submission_id.clone(),
                                msg: EventMsg::TurnComplete(TurnCompleteEvent {
                                    last_agent_message: None,
                                    turn_id: id.to_string(),
                                    completed_at: None,
                                    duration_ms: None,
                                }),
                            })
                            .unwrap();
                    }
                }
                Op::Compact => {
                    self.op_tx
                        .send(Event {
                            id: submission_id.clone(),

                            msg: EventMsg::TurnStarted(TurnStartedEvent {
                                started_at: None,
                                model_context_window: None,
                                collaboration_mode_kind: ModeKind::default(),
                                turn_id: id.to_string(),
                            }),
                        })
                        .unwrap();
                    self.op_tx
                        .send(Event {
                            id: submission_id.clone(),

                            msg: EventMsg::AgentMessage(AgentMessageEvent {
                                message: "Compact task completed".to_string(),
                                phase: None,
                                memory_citation: None,
                            }),
                        })
                        .unwrap();
                    self.op_tx
                        .send(Event {
                            id: submission_id.clone(),

                            msg: EventMsg::TurnComplete(TurnCompleteEvent {
                                last_agent_message: None,
                                turn_id: id.to_string(),
                                completed_at: None,
                                duration_ms: None,
                            }),
                        })
                        .unwrap();
                }
                Op::Undo => {
                    self.op_tx
                        .send(Event {
                            id: submission_id.clone(),

                            msg: EventMsg::UndoStarted(
                                codex_protocol::protocol::UndoStartedEvent {
                                    message: Some("Undo in progress...".to_string()),
                                },
                            ),
                        })
                        .unwrap();
                    self.op_tx
                        .send(Event {
                            id: submission_id.clone(),

                            msg: EventMsg::UndoCompleted(
                                codex_protocol::protocol::UndoCompletedEvent {
                                    success: true,
                                    message: Some("Undo completed.".to_string()),
                                },
                            ),
                        })
                        .unwrap();
                    self.op_tx
                        .send(Event {
                            id: submission_id.clone(),

                            msg: EventMsg::TurnComplete(TurnCompleteEvent {
                                last_agent_message: None,
                                turn_id: id.to_string(),
                                completed_at: None,
                                duration_ms: None,
                            }),
                        })
                        .unwrap();
                }
                Op::Review { review_request } => {
                    self.op_tx
                        .send(Event {
                            id: submission_id.clone(),

                            msg: EventMsg::EnteredReviewMode(review_request.clone()),
                        })
                        .unwrap();
                    self.op_tx
                        .send(Event {
                            id: submission_id.clone(),

                            msg: EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
                                review_output: Some(ReviewOutputEvent {
                                    findings: vec![],
                                    overall_correctness: String::new(),
                                    overall_explanation: review_request
                                        .user_facing_hint
                                        .clone()
                                        .unwrap_or_default(),
                                    overall_confidence_score: 1.,
                                }),
                            }),
                        })
                        .unwrap();
                    self.op_tx
                        .send(Event {
                            id: submission_id.clone(),

                            msg: EventMsg::TurnComplete(TurnCompleteEvent {
                                last_agent_message: None,
                                turn_id: id.to_string(),
                                completed_at: None,
                                duration_ms: None,
                            }),
                        })
                        .unwrap();
                }
                Op::ExecApproval { .. }
                | Op::ResolveElicitation { .. }
                | Op::RequestPermissionsResponse { .. }
                | Op::PatchApproval { .. }
                | Op::Interrupt
                | Op::OverrideTurnContext { .. } => {}
                Op::Shutdown => {
                    if let Some(active_prompt_id) = self.active_prompt_id.lock().unwrap().take() {
                        self.op_tx
                            .send(Event {
                                id: active_prompt_id.clone(),
                                msg: EventMsg::TurnAborted(TurnAbortedEvent {
                                    turn_id: Some(active_prompt_id),
                                    reason: codex_protocol::protocol::TurnAbortReason::Interrupted,
                                    completed_at: None,
                                    duration_ms: None,
                                }),
                            })
                            .unwrap();
                    }
                }
                _ => {
                    unimplemented!()
                }
            }
            Ok(submission_id)
        }

        async fn next_event(&self) -> Result<Event, CodexErr> {
            let Some(event) = self.op_rx.lock().await.recv().await else {
                return Err(CodexErr::InternalAgentDied);
            };
            Ok(event)
        }
    }

    struct SharedSubmissionThread {
        submission_id: String,
        ops: std::sync::Mutex<Vec<Op>>,
        op_tx: mpsc::UnboundedSender<Event>,
        op_rx: Mutex<mpsc::UnboundedReceiver<Event>>,
    }

    impl SharedSubmissionThread {
        fn new(submission_id: impl Into<String>) -> Self {
            let (op_tx, op_rx) = mpsc::unbounded_channel();
            Self {
                submission_id: submission_id.into(),
                ops: std::sync::Mutex::default(),
                op_tx,
                op_rx: Mutex::new(op_rx),
            }
        }

        fn emit(&self, msg: EventMsg) {
            self.op_tx
                .send(Event {
                    id: self.submission_id.clone(),
                    msg,
                })
                .expect("send shared submission event");
        }
    }

    #[async_trait::async_trait]
    impl CodexThreadImpl for SharedSubmissionThread {
        async fn submit(&self, _submission_id: String, op: Op) -> Result<String, CodexErr> {
            self.ops.lock().unwrap().push(op);
            Ok(self.submission_id.clone())
        }

        async fn next_event(&self) -> Result<Event, CodexErr> {
            let Some(event) = self.op_rx.lock().await.recv().await else {
                return Err(CodexErr::InternalAgentDied);
            };
            Ok(event)
        }
    }

    struct StubClient {
        notifications: std::sync::Mutex<Vec<SessionNotification>>,
        permission_requests: std::sync::Mutex<Vec<RequestPermissionRequest>>,
        permission_responses: std::sync::Mutex<VecDeque<RequestPermissionResponse>>,
        block_permission_requests: Option<Arc<Notify>>,
    }

    impl StubClient {
        fn new() -> Self {
            StubClient {
                notifications: std::sync::Mutex::default(),
                permission_requests: std::sync::Mutex::default(),
                permission_responses: std::sync::Mutex::default(),
                block_permission_requests: None,
            }
        }

        fn with_permission_responses(responses: Vec<RequestPermissionResponse>) -> Self {
            StubClient {
                notifications: std::sync::Mutex::default(),
                permission_requests: std::sync::Mutex::default(),
                permission_responses: std::sync::Mutex::new(responses.into()),
                block_permission_requests: None,
            }
        }

        fn with_blocked_permission_requests(
            responses: Vec<RequestPermissionResponse>,
            notify: Arc<Notify>,
        ) -> Self {
            StubClient {
                notifications: std::sync::Mutex::default(),
                permission_requests: std::sync::Mutex::default(),
                permission_responses: std::sync::Mutex::new(responses.into()),
                block_permission_requests: Some(notify),
            }
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Client for StubClient {
        async fn request_permission(
            &self,
            args: RequestPermissionRequest,
        ) -> Result<RequestPermissionResponse, Error> {
            self.permission_requests.lock().unwrap().push(args);
            if let Some(notify) = &self.block_permission_requests {
                notify.notified().await;
            }
            Ok(self
                .permission_responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| {
                    RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled)
                }))
        }

        async fn session_notification(&self, args: SessionNotification) -> Result<(), Error> {
            self.notifications.lock().unwrap().push(args);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_parallel_exec_commands() -> anyhow::Result<()> {
        let (session_id, client, _, message_tx, local_set) = setup().await?;
        let (prompt_response_tx, prompt_response_rx) = tokio::sync::oneshot::channel();

        message_tx.send(ThreadMessage::Prompt {
            request: PromptRequest::new(session_id.clone(), vec!["parallel-exec".into()]),
            response_tx: prompt_response_tx,
        })?;

        tokio::try_join!(
            async {
                let stop_reason = prompt_response_rx.await??.await??;
                assert_eq!(stop_reason, StopReason::EndTurn);
                drop(message_tx);
                anyhow::Ok(())
            },
            async {
                local_set.await;
                anyhow::Ok(())
            }
        )?;

        let notifications = client.notifications.lock().unwrap();

        // Collect all ToolCall (begin) notifications keyed by their tool_call_id prefix.
        let tool_calls: Vec<_> = notifications
            .iter()
            .filter_map(|n| match &n.update {
                SessionUpdate::ToolCall(tc) => Some(tc.clone()),
                _ => None,
            })
            .collect();

        // Collect all ToolCallUpdate notifications that carry a terminal status.
        let completed_updates: Vec<_> = notifications
            .iter()
            .filter_map(|n| match &n.update {
                SessionUpdate::ToolCallUpdate(update) => {
                    if update.fields.status == Some(ToolCallStatus::Completed) {
                        Some(update.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect();

        // Both commands A and B should have produced a ToolCall (begin).
        assert_eq!(
            tool_calls.len(),
            2,
            "expected 2 ToolCall begin notifications, got {tool_calls:?}"
        );

        // Both commands A and B should have produced a completed ToolCallUpdate.
        assert_eq!(
            completed_updates.len(),
            2,
            "expected 2 completed ToolCallUpdate notifications, got {completed_updates:?}"
        );

        // The completed updates should reference the same tool_call_ids as the begins.
        let begin_ids: std::collections::HashSet<_> = tool_calls
            .iter()
            .map(|tc| tc.tool_call_id.clone())
            .collect();
        let end_ids: std::collections::HashSet<_> = completed_updates
            .iter()
            .map(|u| u.tool_call_id.clone())
            .collect();
        assert_eq!(
            begin_ids, end_ids,
            "completed update tool_call_ids should match begin tool_call_ids"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_exec_approval_uses_available_decisions() -> anyhow::Result<()> {
        LocalSet::new()
            .run_until(async {
                let session_id = SessionId::new("test");
                let client = Arc::new(StubClient::with_permission_responses(vec![
                    RequestPermissionResponse::new(RequestPermissionOutcome::Selected(
                        SelectedPermissionOutcome::new("denied"),
                    )),
                ]));
                let session_client =
                    SessionClient::with_client(session_id, client.clone(), Arc::default());
                let thread = Arc::new(StubCodexThread::new());
                let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
                let (message_tx, mut message_rx) = tokio::sync::mpsc::unbounded_channel();
                let mut prompt_state = PromptState::new(
                    "submission-id".to_string(),
                    thread.clone(),
                    message_tx,
                    response_tx,
                );

                prompt_state
                    .exec_approval(
                        &session_client,
                        ExecApprovalRequestEvent {
                            call_id: "call-id".to_string(),
                            approval_id: Some("approval-id".to_string()),
                            turn_id: "turn-id".to_string(),
                            command: vec!["echo".to_string(), "hi".to_string()],
                            cwd: current_dir_abs()?,
                            reason: None,
                            network_approval_context: None,
                            proposed_execpolicy_amendment: None,
                            proposed_network_policy_amendments: None,
                            additional_permissions: None,
                            available_decisions: Some(vec![
                                ReviewDecision::Approved,
                                ReviewDecision::Denied,
                            ]),
                            parsed_cmd: vec![ParsedCommand::Unknown {
                                cmd: "echo hi".to_string(),
                            }],
                        },
                    )
                    .await?;

                let ThreadMessage::PermissionRequestResolved {
                    submission_id,
                    request_key,
                    response,
                } = message_rx.recv().await.unwrap()
                else {
                    panic!("expected permission resolution message");
                };
                assert_eq!(submission_id, "submission-id");
                prompt_state
                    .handle_permission_request_resolved(&session_client, request_key, response)
                    .await?;

                let requests = client.permission_requests.lock().unwrap();
                let request = requests.last().unwrap();
                let option_ids = request
                    .options
                    .iter()
                    .map(|option| option.option_id.0.to_string())
                    .collect::<Vec<_>>();
                assert_eq!(option_ids, vec!["approved", "denied"]);

                let ops = thread.ops.lock().unwrap();
                assert!(matches!(
                    ops.last(),
                    Some(Op::ExecApproval {
                        id,
                        turn_id,
                        decision: ReviewDecision::Denied,
                    }) if id == "approval-id" && turn_id.as_deref() == Some("turn-id")
                ));

                anyhow::Ok(())
            })
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_exec_approval_auto_approves_runtime_actor_cli_command() -> anyhow::Result<()> {
        LocalSet::new()
            .run_until(async {
                let session_id = SessionId::new("test");
                let client = Arc::new(StubClient::new());
                let session_client =
                    SessionClient::with_client(session_id, client.clone(), Arc::default());
                let thread = Arc::new(StubCodexThread::new());
                let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
                let (message_tx, mut message_rx) = tokio::sync::mpsc::unbounded_channel();
                let actor_cli_path =
                    std::env::current_exe().expect("resolve current test binary path");
                let actor_cli = actor_cli_path.to_string_lossy().to_string();
                let mut prompt_state = PromptState::new_with_runtime_actor_cli_path(
                    "submission-id".to_string(),
                    thread.clone(),
                    message_tx,
                    response_tx,
                    Some(actor_cli_path),
                );

                prompt_state
                    .exec_approval(
                        &session_client,
                        ExecApprovalRequestEvent {
                            call_id: "call-id".to_string(),
                            approval_id: Some("approval-id".to_string()),
                            turn_id: "turn-id".to_string(),
                            command: vec![
                                actor_cli.clone(),
                                "actor".to_string(),
                                "inbox".to_string(),
                            ],
                            cwd: current_dir_abs()?,
                            reason: None,
                            network_approval_context: None,
                            proposed_execpolicy_amendment: None,
                            proposed_network_policy_amendments: None,
                            additional_permissions: None,
                            available_decisions: Some(vec![
                                ReviewDecision::Approved,
                                ReviewDecision::Abort,
                            ]),
                            parsed_cmd: vec![ParsedCommand::Unknown {
                                cmd: format!("{actor_cli} actor inbox"),
                            }],
                        },
                    )
                    .await?;

                assert!(
                    message_rx.try_recv().is_err(),
                    "auto-approved runtime actor cli command should not open a permission request"
                );
                let requests = client.permission_requests.lock().unwrap();
                assert!(
                    requests.is_empty(),
                    "runtime actor cli command should bypass permission prompt"
                );

                let ops = thread.ops.lock().unwrap();
                assert!(matches!(
                    ops.last(),
                    Some(Op::ExecApproval {
                        id,
                        turn_id,
                        decision: ReviewDecision::Approved,
                    }) if id == "approval-id" && turn_id.as_deref() == Some("turn-id")
                ));

                anyhow::Ok(())
            })
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_exec_approval_auto_approves_shell_wrapped_runtime_actor_cli_command()
    -> anyhow::Result<()> {
        LocalSet::new()
            .run_until(async {
                let session_id = SessionId::new("test");
                let client = Arc::new(StubClient::new());
                let session_client =
                    SessionClient::with_client(session_id, client.clone(), Arc::default());
                let thread = Arc::new(StubCodexThread::new());
                let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
                let (message_tx, mut message_rx) = tokio::sync::mpsc::unbounded_channel();
                let actor_cli_path =
                    std::env::current_exe().expect("resolve current test binary path");
                let actor_cli = actor_cli_path.to_string_lossy().to_string();
                let shell_command = format!("{actor_cli} actor permission-review-respond");
                let mut prompt_state = PromptState::new_with_runtime_actor_cli_path(
                    "submission-id".to_string(),
                    thread.clone(),
                    message_tx,
                    response_tx,
                    Some(actor_cli_path),
                );

                prompt_state
                    .exec_approval(
                        &session_client,
                        ExecApprovalRequestEvent {
                            call_id: "call-id".to_string(),
                            approval_id: Some("approval-id".to_string()),
                            turn_id: "turn-id".to_string(),
                            command: vec![
                                "/bin/zsh".to_string(),
                                "-lc".to_string(),
                                shell_command.clone(),
                            ],
                            cwd: current_dir_abs()?,
                            reason: None,
                            network_approval_context: None,
                            proposed_execpolicy_amendment: None,
                            proposed_network_policy_amendments: None,
                            additional_permissions: None,
                            available_decisions: Some(vec![
                                ReviewDecision::Approved,
                                ReviewDecision::Abort,
                            ]),
                            parsed_cmd: vec![ParsedCommand::Unknown {
                                cmd: format!("/bin/zsh -lc '{shell_command}'"),
                            }],
                        },
                    )
                    .await?;

                assert!(
                    message_rx.try_recv().is_err(),
                    "shell-wrapped runtime actor cli command should not open a permission request"
                );
                let requests = client.permission_requests.lock().unwrap();
                assert!(
                    requests.is_empty(),
                    "shell-wrapped runtime actor cli command should bypass permission prompt"
                );

                let ops = thread.ops.lock().unwrap();
                assert!(matches!(
                    ops.last(),
                    Some(Op::ExecApproval {
                        id,
                        turn_id,
                        decision: ReviewDecision::Approved,
                    }) if id == "approval-id" && turn_id.as_deref() == Some("turn-id")
                ));

                anyhow::Ok(())
            })
            .await?;

        Ok(())
    }

    #[test]
    fn test_runtime_actor_cli_matcher_accepts_bare_agenthub_on_matching_path() -> anyhow::Result<()>
    {
        let runtime_actor_cli_path = std::env::current_exe()?;
        let temp_dir =
            std::env::temp_dir().join(format!("agenthub-codex-acp-path-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;
        let linked_agenthub = temp_dir.join("agenthub");
        create_runtime_actor_cli_test_symlink(&runtime_actor_cli_path, &linked_agenthub)?;
        let path_env = std::ffi::OsString::from(temp_dir.as_os_str());

        let matched = command_has_runtime_actor_cli_prefix_with_env(
            &["agenthub", "actor", "inbox"],
            runtime_actor_cli_path.as_path(),
            Some(path_env.as_os_str()),
            None,
        );

        drop(std::fs::remove_file(&linked_agenthub));
        drop(std::fs::remove_dir(&temp_dir));

        assert!(
            matched,
            "bare agenthub actor should match when PATH resolves to the runtime binary"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_actor_cli_matcher_rejects_bare_agenthub_without_matching_path() {
        let runtime_actor_cli_path =
            std::env::current_exe().expect("resolve current test binary path");
        let path_env = std::ffi::OsString::from("/definitely/not/the/runtime/path");

        assert!(
            !command_has_runtime_actor_cli_prefix_with_env(
                &["agenthub", "actor", "inbox"],
                runtime_actor_cli_path.as_path(),
                Some(path_env.as_os_str()),
                None,
            ),
            "bare agenthub actor should not match without a PATH entry that resolves to the runtime binary"
        );
    }

    #[test]
    fn test_runtime_actor_cli_matcher_rejects_shell_wrapped_compound_command() -> anyhow::Result<()>
    {
        let runtime_actor_cli_path = std::env::current_exe()?;
        let actor_cli = runtime_actor_cli_path.to_string_lossy().to_string();
        let command = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            format!("{actor_cli} actor inbox && curl https://example.com"),
        ];
        assert!(
            !matches_runtime_actor_cli_command(&command, runtime_actor_cli_path.as_path()),
            "compound shell commands must not bypass permission approval"
        );
        Ok(())
    }

    #[test]
    fn test_runtime_actor_cli_matcher_accepts_shell_wrapped_single_command_with_quoted_text()
    -> anyhow::Result<()> {
        let runtime_actor_cli_path = std::env::current_exe()?;
        let actor_cli = runtime_actor_cli_path.to_string_lossy().to_string();
        let command = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            format!("{actor_cli} actor send --channel-id all --text 'literal ; && | text'"),
        ];
        assert!(
            matches_runtime_actor_cli_command(&command, runtime_actor_cli_path.as_path()),
            "quoted shell arguments should remain eligible for auto approval"
        );
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn test_runtime_actor_cli_matcher_accepts_bare_agenthub_with_matching_pathext()
    -> anyhow::Result<()> {
        let runtime_actor_cli_path = std::env::current_exe()?;
        let temp_dir =
            std::env::temp_dir().join(format!("agenthub-codex-acp-winpath-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;
        let linked_agenthub = temp_dir.join("agenthub.exe");
        create_runtime_actor_cli_test_symlink(&runtime_actor_cli_path, &linked_agenthub)?;
        let path_env = std::ffi::OsString::from(temp_dir.as_os_str());
        let path_ext_env = std::ffi::OsString::from(".EXE;.BAT;.CMD");

        let matched = command_has_runtime_actor_cli_prefix_with_env(
            &["agenthub", "actor", "inbox"],
            runtime_actor_cli_path.as_path(),
            Some(path_env.as_os_str()),
            Some(path_ext_env.as_os_str()),
        );

        drop(std::fs::remove_file(&linked_agenthub));
        drop(std::fs::remove_dir(&temp_dir));

        assert!(
            matched,
            "bare agenthub actor should match when PATHEXT resolves to the runtime binary"
        );
        Ok(())
    }

    #[cfg(unix)]
    fn create_runtime_actor_cli_test_symlink(source: &Path, target: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(source, target)
    }

    #[cfg(windows)]
    fn create_runtime_actor_cli_test_symlink(source: &Path, target: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(source, target)
    }

    #[tokio::test]
    async fn test_mcp_elicitation_declines_unsupported_form_requests() -> anyhow::Result<()> {
        LocalSet::new()
            .run_until(async {
                let session_id = SessionId::new("test");
                let client = Arc::new(StubClient::with_permission_responses(vec![
                    RequestPermissionResponse::new(RequestPermissionOutcome::Selected(
                        SelectedPermissionOutcome::new("decline"),
                    )),
                ]));
                let session_client =
                    SessionClient::with_client(session_id, client.clone(), Arc::default());
                let thread = Arc::new(StubCodexThread::new());
                let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
                let (message_tx, _message_rx) = tokio::sync::mpsc::unbounded_channel();
                let mut prompt_state = PromptState::new(
                    "submission-id".to_string(),
                    thread.clone(),
                    message_tx,
                    response_tx,
                );

                prompt_state
                    .mcp_elicitation(
                        &session_client,
                        ElicitationRequestEvent {
                            turn_id: Some("turn-id".to_string()),
                            server_name: "test-server".to_string(),
                            id: codex_protocol::mcp::RequestId::String("request-id".to_string()),
                            request: ElicitationRequest::Form {
                                meta: None,
                                message: "Need some structured input".to_string(),
                                requested_schema: serde_json::json!({
                                    "type": "object",
                                    "properties": {
                                        "name": { "type": "string" }
                                    }
                                }),
                            },
                        },
                    )
                    .await?;

                let requests = client.permission_requests.lock().unwrap();
                assert!(
                    requests.is_empty(),
                    "unsupported MCP elicitations should be auto-declined"
                );

                let ops = thread.ops.lock().unwrap();
                assert!(matches!(
                    ops.last(),
                    Some(Op::ResolveElicitation {
                        server_name,
                        request_id: codex_protocol::mcp::RequestId::String(request_id),
                        decision: ElicitationAction::Decline,
                        content: None,
                        meta: None,
                    }) if server_name == "test-server" && request_id == "request-id"
                ));

                anyhow::Ok(())
            })
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_blocked_approval_does_not_block_followup_events() -> anyhow::Result<()> {
        LocalSet::new()
            .run_until(async {
                let session_id = SessionId::new("test");
                let client = Arc::new(StubClient::with_blocked_permission_requests(
                    vec![],
                    Arc::new(Notify::new()),
                ));
                let session_client =
                    SessionClient::with_client(session_id, client.clone(), Arc::default());
                let thread = Arc::new(StubCodexThread::new());
                let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
                let (message_tx, _message_rx) = tokio::sync::mpsc::unbounded_channel();
                let mut prompt_state =
                    PromptState::new("submission-id".to_string(), thread, message_tx, response_tx);

                prompt_state
                    .handle_event(
                        &session_client,
                        EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                            call_id: "call-id".to_string(),
                            approval_id: Some("approval-id".to_string()),
                            turn_id: "turn-id".to_string(),
                            command: vec!["echo".to_string(), "hi".to_string()],
                            cwd: current_dir_abs()?,
                            reason: None,
                            network_approval_context: None,
                            proposed_execpolicy_amendment: None,
                            proposed_network_policy_amendments: None,
                            additional_permissions: None,
                            available_decisions: Some(vec![
                                ReviewDecision::Approved,
                                ReviewDecision::Abort,
                            ]),
                            parsed_cmd: vec![ParsedCommand::Unknown {
                                cmd: "echo hi".to_string(),
                            }],
                        }),
                    )
                    .await;

                prompt_state
                    .handle_event(
                        &session_client,
                        EventMsg::AgentMessage(AgentMessageEvent {
                            message: "still flowing".to_string(),
                            phase: None,
                            memory_citation: None,
                        }),
                    )
                    .await;

                let notifications = client.notifications.lock().unwrap();
                assert!(notifications.iter().any(|notification| {
                    matches!(
                        &notification.update,
                        SessionUpdate::AgentMessageChunk(ContentChunk {
                            content: ContentBlock::Text(TextContent { text, .. }),
                            ..
                        }) if text == "still flowing"
                    )
                }));

                drop(notifications);
                prompt_state.abort_pending_interactions();

                anyhow::Ok(())
            })
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_legacy_reasoning_events_emit_agent_thought_chunks() -> anyhow::Result<()> {
        LocalSet::new()
            .run_until(async {
                let session_id = SessionId::new("test");
                let client = Arc::new(StubClient::new());
                let session_client =
                    SessionClient::with_client(session_id, client.clone(), Arc::default());
                let thread = Arc::new(StubCodexThread::new());
                let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
                let (message_tx, _message_rx) = tokio::sync::mpsc::unbounded_channel();
                let mut prompt_state =
                    PromptState::new("submission-id".to_string(), thread, message_tx, response_tx);

                prompt_state
                    .handle_event(
                        &session_client,
                        EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent {
                            delta: "thinking chunk".to_string(),
                        }),
                    )
                    .await;

                prompt_state
                    .handle_event(
                        &session_client,
                        EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent {
                            text: "final raw reasoning".to_string(),
                        }),
                    )
                    .await;

                let notifications = client.notifications.lock().unwrap();
                assert_eq!(
                    notifications.len(),
                    1,
                    "notifications don't match {notifications:?}"
                );
                assert!(matches!(
                    &notifications[0].update,
                    SessionUpdate::AgentThoughtChunk(ContentChunk {
                        content: ContentBlock::Text(TextContent { text, .. }),
                        ..
                    }) if text == "thinking chunk"
                ));

                anyhow::Ok(())
            })
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_legacy_raw_reasoning_event_emits_agent_thought_chunk() -> anyhow::Result<()> {
        LocalSet::new()
            .run_until(async {
                let session_id = SessionId::new("test");
                let client = Arc::new(StubClient::new());
                let session_client =
                    SessionClient::with_client(session_id, client.clone(), Arc::default());
                let thread = Arc::new(StubCodexThread::new());
                let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
                let (message_tx, _message_rx) = tokio::sync::mpsc::unbounded_channel();
                let mut prompt_state =
                    PromptState::new("submission-id".to_string(), thread, message_tx, response_tx);

                prompt_state
                    .handle_event(
                        &session_client,
                        EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent {
                            text: "raw reasoning only".to_string(),
                        }),
                    )
                    .await;

                let notifications = client.notifications.lock().unwrap();
                assert_eq!(
                    notifications.len(),
                    1,
                    "notifications don't match {notifications:?}"
                );
                assert!(matches!(
                    &notifications[0].update,
                    SessionUpdate::AgentThoughtChunk(ContentChunk {
                        content: ContentBlock::Text(TextContent { text, .. }),
                        ..
                    }) if text == "raw reasoning only"
                ));

                anyhow::Ok(())
            })
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_legacy_final_reasoning_events_are_deduplicated() -> anyhow::Result<()> {
        LocalSet::new()
            .run_until(async {
                let session_id = SessionId::new("test");
                let client = Arc::new(StubClient::new());
                let session_client =
                    SessionClient::with_client(session_id, client.clone(), Arc::default());
                let thread = Arc::new(StubCodexThread::new());
                let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
                let (message_tx, _message_rx) = tokio::sync::mpsc::unbounded_channel();
                let mut prompt_state =
                    PromptState::new("submission-id".to_string(), thread, message_tx, response_tx);

                prompt_state
                    .handle_event(
                        &session_client,
                        EventMsg::AgentReasoning(AgentReasoningEvent {
                            text: "final reasoning".to_string(),
                        }),
                    )
                    .await;

                prompt_state
                    .handle_event(
                        &session_client,
                        EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent {
                            text: "duplicate raw reasoning".to_string(),
                        }),
                    )
                    .await;

                let notifications = client.notifications.lock().unwrap();
                assert_eq!(
                    notifications.len(),
                    1,
                    "notifications don't match {notifications:?}"
                );
                assert!(matches!(
                    &notifications[0].update,
                    SessionUpdate::AgentThoughtChunk(ContentChunk {
                        content: ContentBlock::Text(TextContent { text, .. }),
                        ..
                    }) if text == "final reasoning"
                ));

                anyhow::Ok(())
            })
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_background_terminal_wait_activity_emits_terminal_activity_updates()
    -> anyhow::Result<()> {
        LocalSet::new()
            .run_until(async {
                let session_id = SessionId::new("test");
                let client = Arc::new(StubClient::new());
                let session_client =
                    SessionClient::with_client(session_id, client.clone(), Arc::default());
                let thread = Arc::new(StubCodexThread::new());
                let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
                let (message_tx, _message_rx) = tokio::sync::mpsc::unbounded_channel();
                let mut prompt_state =
                    PromptState::new("submission-id".to_string(), thread, message_tx, response_tx);

                prompt_state.active_commands.insert(
                    "call-1".to_string(),
                    ActiveCommand {
                        tool_call_id: ToolCallId::new("call-1"),
                        title: "Run cargo test -p codex-core".to_string(),
                        terminal_output: false,
                        output: String::new(),
                        file_extension: None,
                        background_terminal_waiting: false,
                    },
                );

                prompt_state
                    .handle_event(
                        &session_client,
                        EventMsg::TerminalInteraction(TerminalInteractionEvent {
                            call_id: "call-1".to_string(),
                            process_id: "proc-1".to_string(),
                            stdin: String::new(),
                        }),
                    )
                    .await;

                prompt_state
                    .handle_event(
                        &session_client,
                        EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                            call_id: "call-1".to_string(),
                            chunk: b"stdout\n".to_vec(),
                            stream: codex_protocol::protocol::ExecOutputStream::Stdout,
                        }),
                    )
                    .await;

                let notifications = client.notifications.lock().unwrap();
                let updates: Vec<_> = notifications
                    .iter()
                    .filter_map(|notification| match &notification.update {
                        SessionUpdate::ToolCallUpdate(update) => Some(update.clone()),
                        _ => None,
                    })
                    .collect();
                assert_eq!(updates.len(), 3, "updates don't match {updates:?}");
                assert_eq!(
                    updates[0]
                        .meta
                        .as_ref()
                        .and_then(|meta| meta.get("terminal_activity"))
                        .and_then(|value| value.get("kind"))
                        .and_then(serde_json::Value::as_str),
                    Some("waiting")
                );
                assert_eq!(
                    updates[1]
                        .meta
                        .as_ref()
                        .and_then(|meta| meta.get("terminal_activity"))
                        .and_then(|value| value.get("kind"))
                        .and_then(serde_json::Value::as_str),
                    Some("waited")
                );
                assert!(updates[2].fields.content.is_some());

                anyhow::Ok(())
            })
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_exec_command_end_emits_waited_meta_for_terminal_output() -> anyhow::Result<()> {
        LocalSet::new()
            .run_until(async {
                let session_id = SessionId::new("test");
                let client = Arc::new(StubClient::new());
                let client_capabilities =
                    Arc::new(std::sync::Mutex::new(ClientCapabilities::default()));
                client_capabilities.lock().unwrap().meta =
                    serde_json::json!({ "terminal_output": true })
                        .as_object()
                        .cloned();
                let session_client =
                    SessionClient::with_client(session_id, client.clone(), client_capabilities);
                let thread = Arc::new(StubCodexThread::new());
                let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
                let (message_tx, _message_rx) = tokio::sync::mpsc::unbounded_channel();
                let mut prompt_state =
                    PromptState::new("submission-id".to_string(), thread, message_tx, response_tx);

                prompt_state.active_commands.insert(
                    "call-1".to_string(),
                    ActiveCommand {
                        tool_call_id: ToolCallId::new("call-1"),
                        title: "Run cargo test -p codex-core".to_string(),
                        terminal_output: true,
                        output: String::new(),
                        file_extension: None,
                        background_terminal_waiting: true,
                    },
                );

                prompt_state
                    .handle_event(
                        &session_client,
                        EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                            call_id: "call-1".to_string(),
                            process_id: None,
                            turn_id: "turn-1".to_string(),
                            command: vec!["cargo".to_string(), "test".to_string()],
                            cwd: AbsolutePathBuf::from_absolute_path(
                                std::env::current_dir()?.join("."),
                            )?,
                            parsed_cmd: vec![],
                            source: Default::default(),
                            interaction_input: None,
                            stdout: String::new(),
                            stderr: String::new(),
                            aggregated_output: String::new(),
                            exit_code: 0,
                            duration: std::time::Duration::from_millis(10),
                            formatted_output: String::new(),
                            status: ExecCommandStatus::Completed,
                        }),
                    )
                    .await;

                let notifications = client.notifications.lock().unwrap();
                let updates: Vec<_> = notifications
                    .iter()
                    .filter_map(|notification| match &notification.update {
                        SessionUpdate::ToolCallUpdate(update) => Some(update.clone()),
                        _ => None,
                    })
                    .collect();
                assert_eq!(updates.len(), 1, "updates don't match {updates:?}");
                let meta = updates[0].meta.as_ref().expect("terminal meta missing");
                assert_eq!(
                    meta.get("terminal_activity")
                        .and_then(|value| value.get("kind"))
                        .and_then(serde_json::Value::as_str),
                    Some("waited")
                );
                assert_eq!(
                    meta.get("terminal_activity")
                        .and_then(|value| value.get("command"))
                        .and_then(serde_json::Value::as_str),
                    Some("Run cargo test -p codex-core")
                );
                assert_eq!(
                    meta.get("terminal_exit")
                        .and_then(|value| value.get("terminal_id"))
                        .and_then(serde_json::Value::as_str),
                    Some("call-1")
                );
                assert_eq!(updates[0].fields.status, Some(ToolCallStatus::Completed));

                anyhow::Ok(())
            })
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_thread_shutdown_bypasses_blocked_permission_request() -> anyhow::Result<()> {
        let session_id = SessionId::new("test");
        let client = Arc::new(StubClient::with_blocked_permission_requests(
            vec![RequestPermissionResponse::new(
                RequestPermissionOutcome::Cancelled,
            )],
            Arc::new(Notify::new()),
        ));
        let session_client =
            SessionClient::with_client(session_id.clone(), client.clone(), Arc::default());
        let conversation = Arc::new(StubCodexThread::new());
        let models_manager = Arc::new(StubModelsManager);
        let config = test_config()?;
        let (message_tx, message_rx) = tokio::sync::mpsc::unbounded_channel();
        let (resolution_tx, resolution_rx) = tokio::sync::mpsc::unbounded_channel();
        let actor = ThreadActor::new(
            StubAuth,
            session_client,
            conversation.clone(),
            models_manager,
            config,
            message_rx,
            resolution_tx,
            resolution_rx,
        );

        let local_set = LocalSet::new();
        let handle = local_set.spawn_local(actor.spawn());
        let thread = Thread {
            thread: conversation.clone(),
            message_tx,
            _handle: handle,
        };

        local_set
            .run_until(async move {
                let (prompt_response_tx, prompt_response_rx) = tokio::sync::oneshot::channel();
                thread.message_tx.send(ThreadMessage::Prompt {
                    request: PromptRequest::new(session_id, vec!["approval-block".into()]),
                    response_tx: prompt_response_tx,
                })?;
                let stop_reason_rx = prompt_response_rx.await??;

                tokio::time::timeout(Duration::from_millis(100), async {
                    loop {
                        if !client.permission_requests.lock().unwrap().is_empty() {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await?;

                tokio::time::timeout(Duration::from_millis(100), thread.shutdown()).await??;
                let stop_reason =
                    tokio::time::timeout(Duration::from_millis(100), stop_reason_rx).await??;
                assert_eq!(stop_reason?, StopReason::Cancelled);

                anyhow::Ok(())
            })
            .await?;

        let ops = conversation.ops.lock().unwrap();
        assert!(matches!(ops.last(), Some(Op::Shutdown)));
        drop(ops);

        let submission_ids = conversation.submission_ids.lock().unwrap();
        assert_eq!(submission_ids.last().map(String::as_str), Some("shutdown"));

        Ok(())
    }

    #[tokio::test]
    async fn test_set_mode_uses_config_submission_id() -> anyhow::Result<()> {
        let (_session_id, _client, thread, message_tx, local_set) = setup().await?;
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        message_tx.send(ThreadMessage::SetMode {
            mode: SessionModeId::new("read-only"),
            response_tx,
        })?;

        tokio::try_join!(
            async {
                response_rx.await??;
                drop(message_tx);
                anyhow::Ok(())
            },
            async {
                local_set.await;
                anyhow::Ok(())
            }
        )?;

        let submission_ids = thread.submission_ids.lock().unwrap();
        assert_eq!(submission_ids.as_slice(), &["config".to_string()]);
        drop(submission_ids);

        let ops = thread.ops.lock().unwrap();
        assert!(matches!(ops.as_slice(), [Op::OverrideTurnContext { .. }]));

        Ok(())
    }

    #[tokio::test]
    async fn test_set_model_uses_config_submission_id() -> anyhow::Result<()> {
        let (_session_id, _client, thread, message_tx, local_set) = setup().await?;
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        message_tx.send(ThreadMessage::SetModel {
            model: ModelId::new("test-model"),
            response_tx,
        })?;

        tokio::try_join!(
            async {
                response_rx.await??;
                drop(message_tx);
                anyhow::Ok(())
            },
            async {
                local_set.await;
                anyhow::Ok(())
            }
        )?;

        let submission_ids = thread.submission_ids.lock().unwrap();
        assert_eq!(submission_ids.as_slice(), &["config".to_string()]);
        drop(submission_ids);

        let ops = thread.ops.lock().unwrap();
        assert!(matches!(
            ops.as_slice(),
            [Op::OverrideTurnContext {
                model: Some(model),
                ..
            }] if model == "test-model"
        ));

        Ok(())
    }

    #[tokio::test]
    async fn test_set_model_config_option_uses_config_submission_id() -> anyhow::Result<()> {
        let (_session_id, _client, thread, message_tx, local_set) = setup().await?;
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        message_tx.send(ThreadMessage::SetConfigOption {
            config_id: SessionConfigId::new("model"),
            value: SessionConfigOptionValue::ValueId {
                value: SessionConfigValueId::new("test-model"),
            },
            response_tx,
        })?;

        tokio::try_join!(
            async {
                response_rx.await??;
                drop(message_tx);
                anyhow::Ok(())
            },
            async {
                local_set.await;
                anyhow::Ok(())
            }
        )?;

        let submission_ids = thread.submission_ids.lock().unwrap();
        assert_eq!(submission_ids.as_slice(), &["config".to_string()]);
        drop(submission_ids);

        let ops = thread.ops.lock().unwrap();
        assert!(matches!(
            ops.as_slice(),
            [Op::OverrideTurnContext {
                model: Some(model),
                ..
            }] if model == "test-model"
        ));

        Ok(())
    }

    #[tokio::test]
    async fn test_cancel_uses_interrupt_submission_id() -> anyhow::Result<()> {
        let (_session_id, _client, thread, message_tx, local_set) = setup().await?;
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        message_tx.send(ThreadMessage::Cancel { response_tx })?;

        tokio::try_join!(
            async {
                response_rx.await??;
                drop(message_tx);
                anyhow::Ok(())
            },
            async {
                local_set.await;
                anyhow::Ok(())
            }
        )?;

        let submission_ids = thread.submission_ids.lock().unwrap();
        assert_eq!(submission_ids.as_slice(), &["interrupt".to_string()]);
        drop(submission_ids);

        let ops = thread.ops.lock().unwrap();
        assert!(matches!(ops.as_slice(), [Op::Interrupt]));

        Ok(())
    }

    #[test]
    fn test_build_prompt_items_converts_managed_skill_block_to_native_skill_input() {
        let managed_home = TempManagedSkillsHome::new();
        let trusted_root = managed_home.trusted_root();
        let skill_path = managed_home.skill_path(ManagedSkillKind::TeamActorMailbox);
        let skill_block = format!(
            "<skill>\n<name>team-actor-mailbox</name>\n<path>{}</path>\nUse the managed skill.\n</skill>",
            skill_path.display()
        );
        let items = build_prompt_items_with_trusted_root(
            vec![ContentBlock::Text(TextContent::new(skill_block.as_str()))],
            Some(trusted_root.as_path()),
        );

        assert_eq!(
            items,
            vec![UserInput::Skill {
                name: "team-actor-mailbox".to_string(),
                path: skill_path,
            }]
        );
    }

    #[test]
    fn test_build_prompt_items_keeps_builtin_skill_block_as_text() {
        let text = "<skill>\n<name>team-leader</name>\n<path>builtin://team/leader</path>\nBuiltin body.\n</skill>";
        let items = build_prompt_items(vec![ContentBlock::Text(TextContent::new(text))]);

        assert_eq!(
            items,
            vec![UserInput::Text {
                text: text.to_string(),
                text_elements: vec![],
            }]
        );
    }

    #[test]
    fn test_build_prompt_items_preserves_mixed_skill_and_text_order() {
        let managed_home = TempManagedSkillsHome::new();
        let trusted_root = managed_home.trusted_root();
        let skill_path = managed_home.skill_path(ManagedSkillKind::TeamAgentsIndex);
        let skill_block = format!(
            "<skill>\n<name>team-agents-index</name>\n<path>{}</path>\nUse the managed skill.\n</skill>",
            skill_path.display()
        );
        let items = build_prompt_items_with_trusted_root(
            vec![
                ContentBlock::Text(TextContent::new(skill_block.as_str())),
                ContentBlock::Text(TextContent::new("Explain the result.")),
            ],
            Some(trusted_root.as_path()),
        );

        assert_eq!(
            items,
            vec![
                UserInput::Skill {
                    name: "team-agents-index".to_string(),
                    path: skill_path,
                },
                UserInput::Text {
                    text: "Explain the result.".to_string(),
                    text_elements: vec![],
                },
            ]
        );
    }

    #[test]
    fn test_build_prompt_items_converts_tilde_managed_skill_block_to_native_skill_input() {
        let managed_home = TempManagedSkillsHome::new();
        let trusted_root = managed_home.trusted_root();
        let skill_path = managed_home.skill_path(ManagedSkillKind::TeamTaskLifecycle);
        let relative_to_home = skill_path
            .strip_prefix(managed_home.home.as_path())
            .expect("skill path should live under temp home");
        let skill_block = format!(
            "<skill>\n<name>team-task-lifecycle</name>\n<path>~/{}\
</path>\nUse tilde path.\n</skill>",
            relative_to_home.display()
        );
        let items = build_prompt_items_with_trusted_root(
            vec![ContentBlock::Text(TextContent::new(skill_block.as_str()))],
            Some(trusted_root.as_path()),
        );

        assert_eq!(
            items,
            vec![UserInput::Skill {
                name: "team-task-lifecycle".to_string(),
                path: skill_path,
            }]
        );
    }

    #[test]
    fn test_build_prompt_items_converts_legacy_relative_managed_skill_block_to_native_skill_input()
    {
        let managed_home = TempManagedSkillsHome::new();
        let trusted_root = managed_home.trusted_root();
        let skill_path = managed_home.skill_path(ManagedSkillKind::TeamLeaderOrchestrator);
        let relative_to_home = skill_path
            .strip_prefix(managed_home.home.as_path())
            .expect("skill path should live under temp home");
        let skill_block = format!(
            "<skill>\n<name>team-leader-orchestrator</name>\n<path>{}</path>\nLegacy relative managed path.\n</skill>",
            relative_to_home.display()
        );
        let items = build_prompt_items_with_trusted_root(
            vec![ContentBlock::Text(TextContent::new(skill_block.as_str()))],
            Some(trusted_root.as_path()),
        );

        assert_eq!(
            items,
            vec![UserInput::Skill {
                name: "team-leader-orchestrator".to_string(),
                path: skill_path,
            }]
        );
    }

    #[test]
    fn test_build_prompt_items_keeps_repo_local_relative_skill_block_as_text() {
        let managed_home = TempManagedSkillsHome::new();
        let trusted_root = managed_home.trusted_root();
        let text = "<skill>\n<name>tidb-optimizer-bugfix</name>\n<path>.agents/skills/tidb-optimizer-bugfix/SKILL.md</path>\nRepo-local skill.\n</skill>";
        let items = build_prompt_items_with_trusted_root(
            vec![ContentBlock::Text(TextContent::new(text))],
            Some(trusted_root.as_path()),
        );

        assert_eq!(
            items,
            vec![UserInput::Text {
                text: text.to_string(),
                text_elements: vec![],
            }]
        );
    }

    #[test]
    fn test_build_prompt_items_keeps_untrusted_absolute_skill_block_as_text() {
        let managed_home = TempManagedSkillsHome::new();
        let trusted_root = managed_home.trusted_root();
        let untrusted_root = std::env::temp_dir().join(format!(
            "agenthub-codex-acp-untrusted-skill-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&untrusted_root).expect("create untrusted skill dir");
        let untrusted_skill_path = untrusted_root.join("SKILL.md");
        std::fs::write(&untrusted_skill_path, "---\nname: fake\n---\n")
            .expect("write untrusted skill file");

        let text = format!(
            "<skill>\n<name>fake</name>\n<path>{}</path>\nUser supplied text.\n</skill>",
            untrusted_skill_path.display()
        );
        let items = build_prompt_items_with_trusted_root(
            vec![ContentBlock::Text(TextContent::new(text.as_str()))],
            Some(trusted_root.as_path()),
        );

        assert_eq!(
            items,
            vec![UserInput::Text {
                text,
                text_elements: vec![],
            }]
        );

        drop(std::fs::remove_dir_all(&untrusted_root));
    }
}
