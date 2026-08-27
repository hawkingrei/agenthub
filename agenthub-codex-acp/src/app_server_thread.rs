use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use agent_client_protocol::Error;
use agent_client_protocol::schema::v1::SessionId;
use codex_app_server_client::{
    DEFAULT_IN_PROCESS_CHANNEL_CAPACITY, InProcessAppServerClient, InProcessAppServerRequestHandle,
    InProcessClientStartArgs, InProcessServerEvent, TypedRequestError,
};
use codex_app_server_protocol::{
    ClientRequest, CodexErrorInfo as AppServerCodexErrorInfo, CommandExecutionApprovalDecision,
    CommandExecutionRequestApprovalResponse, FileChangeApprovalDecision,
    FileChangeRequestApprovalResponse, FileUpdateChange, McpServerElicitationAction,
    McpServerElicitationRequestResponse, PermissionGrantScope as AppPermissionGrantScope,
    PermissionsRequestApprovalResponse, RequestId, ReviewDelivery, ReviewStartParams,
    ReviewStartResponse, ReviewTarget as AppReviewTarget, SandboxMode, ServerNotification,
    ServerRequest, ThreadCompactStartParams, ThreadCompactStartResponse, ThreadItem,
    ThreadResumeParams, ThreadResumeResponse, ThreadRollbackParams, ThreadRollbackResponse,
    ThreadStartParams, ThreadStartResponse, ThreadStatus, Turn, TurnError as AppServerTurnError,
    TurnInterruptParams, TurnInterruptResponse, TurnStartParams, TurnStartResponse, TurnStatus,
    TurnSteerParams, TurnSteerResponse,
};
use codex_arg0::Arg0DispatchPaths;
use codex_config::{CloudConfigBundleLoader, LoaderOverrides};
use codex_core::config::Config;
use codex_feedback::CodexFeedback;
use codex_protocol::approvals::{
    ApplyPatchApprovalRequestEvent, ElicitationAction, ElicitationRequest, ElicitationRequestEvent,
    ExecApprovalRequestEvent,
};
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::dynamic_tools::DynamicToolCallRequest;
use codex_protocol::error::CodexErr;
use codex_protocol::items::{
    CollabAgentTool as CoreCollabAgentTool, CollabAgentToolCallItem as CoreCollabAgentToolCallItem,
    CollabAgentToolCallStatus as CoreCollabAgentToolCallStatus,
    SubAgentActivityItem as CoreSubAgentActivityItem, TurnItem as CoreTurnItem,
};
use codex_protocol::mcp::{CallToolResult, RequestId as McpRequestId};
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::plan_tool::{PlanItemArg, StepStatus, UpdatePlanArgs};
use codex_protocol::protocol::{
    AgentMessageContentDeltaEvent, AgentMessageEvent, AgentStatus, CodexErrorInfo,
    ContextCompactedEvent, DeprecationNoticeEvent, EnteredReviewModeEvent, ErrorEvent, Event,
    EventMsg, ExecCommandBeginEvent, ExecCommandEndEvent, ExecCommandOutputDeltaEvent,
    ExecCommandSource, ExecCommandStatus, ExitedReviewModeEvent, FileChange,
    ImageGenerationBeginEvent, ImageGenerationEndEvent, ItemCompletedEvent, ItemStartedEvent,
    McpInvocation, McpToolCallBeginEvent, McpToolCallEndEvent, ModelRerouteEvent,
    NonSteerableTurnKind, Op, PatchApplyBeginEvent, PatchApplyEndEvent, PatchApplyStatus,
    ReviewDecision, ReviewOutputEvent, ReviewTarget, StreamErrorEvent,
    SubAgentActivityKind as CoreSubAgentActivityKind, TerminalInteractionEvent, TokenCountEvent,
    TokenUsage, TokenUsageInfo, TurnAbortReason, TurnAbortedEvent, TurnCompleteEvent,
    TurnStartedEvent, ViewImageToolCallEvent, WarningEvent, WebSearchBeginEvent, WebSearchEndEvent,
};
use codex_protocol::request_permissions::{
    PermissionGrantScope, RequestPermissionProfile, RequestPermissionsEvent,
    RequestPermissionsResponse,
};
use codex_protocol::request_user_input::{
    RequestUserInputEvent, RequestUserInputQuestion, RequestUserInputResponse,
};
use codex_protocol::turn_input::TurnInput as ProtocolTurnInput;
use codex_protocol::{AgentPath, ThreadId};
use codex_shell_command::parse_command::parse_command;
use tokio::sync::Mutex;
use tracing::warn;

use crate::build_environment_manager;
use crate::thread::CodexThreadImpl;

const ACP_CLIENT_NAME: &str = "agenthub-codex-acp";
const DYNAMIC_TOOL_CALLBACK_UNSUPPORTED_MESSAGE: &str =
    "dynamic tool callbacks are not supported by agenthub-codex-acp";
const ATTESTATION_GENERATION_UNSUPPORTED_MESSAGE: &str =
    "attestation generation is not supported by agenthub-codex-acp";
// The in-process client exposes event reads and server-request replies through
// the same client object. Do not hold its mutex forever while waiting for
// provider events, otherwise permission replies and interrupts can be starved.
const APP_SERVER_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub async fn start_new_thread(
    config: Config,
) -> Result<(SessionId, Arc<dyn CodexThreadImpl>), Error> {
    let client: InProcessAppServerClient = start_client(&config).await?;
    let request_handle = client.request_handle();
    let response: ThreadStartResponse = client
        .request_typed::<ThreadStartResponse>(ClientRequest::ThreadStart {
            request_id: RequestId::Integer(1),
            params: thread_start_params_from_config(&config),
        })
        .await
        .map_err(app_server_internal_error)?;

    let session_id = SessionId::new(response.thread.id.clone());
    let thread = Arc::new(AppServerCodexThread::new(
        client,
        request_handle,
        response.thread.id,
        config,
        2,
        response.thread.status,
        response.thread.turns,
    ));
    Ok((session_id, thread))
}

pub async fn resume_thread(
    config: Config,
    session_id: &SessionId,
) -> Result<Arc<dyn CodexThreadImpl>, Error> {
    let client: InProcessAppServerClient = start_client(&config).await?;
    let request_handle = client.request_handle();
    let response: ThreadResumeResponse = client
        .request_typed::<ThreadResumeResponse>(ClientRequest::ThreadResume {
            request_id: RequestId::Integer(1),
            params: thread_resume_params_from_config(&config, session_id),
        })
        .await
        .map_err(app_server_internal_error)?;

    Ok(Arc::new(AppServerCodexThread::new(
        client,
        request_handle,
        response.thread.id,
        config,
        2,
        response.thread.status,
        response.thread.turns,
    )))
}

struct AppServerCodexThread {
    client: Mutex<Option<InProcessAppServerClient>>,
    request_handle: InProcessAppServerRequestHandle,
    state: Mutex<AppServerState>,
}

struct AppServerState {
    config: Config,
    next_request_id: i64,
    thread_id: String,
    active_turn: Option<ActiveTurn>,
    queued_submissions: VecDeque<QueuedSubmission>,
    local_events: VecDeque<Event>,
    pending_exec_requests: HashMap<String, RequestId>,
    pending_patch_requests: HashMap<String, RequestId>,
    pending_patch_changes: HashMap<String, HashMap<PathBuf, FileChange>>,
    pending_permissions_requests: HashMap<String, RequestId>,
    pending_user_input_requests: HashMap<String, RequestId>,
    pending_elicitation_requests: HashMap<String, RequestId>,
    pending_turn_diffs: HashMap<String, String>,
    pending_custom_tool_calls: HashSet<String>,
    interrupt_after_turn_starts: bool,
}

#[derive(Clone)]
struct ActiveTurn {
    submission_id: String,
    turn_id: Option<String>,
    steerable: bool,
    last_agent_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MissingCustomToolOutputs {
    call_ids: Vec<String>,
}

struct PendingTurnInterrupt {
    request_id: RequestId,
    thread_id: String,
    turn_id: String,
}

impl MissingCustomToolOutputs {
    fn from_state(state: &AppServerState) -> Option<Self> {
        if state.pending_custom_tool_calls.is_empty() {
            return None;
        }

        let mut call_ids = state
            .pending_custom_tool_calls
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        call_ids.sort();
        Some(Self { call_ids })
    }

    fn into_codex_err(self) -> CodexErr {
        CodexErr::Fatal(format!(
            "Codex live history is missing CustomToolCallOutput for call id(s): {}. \
Use undo, force a new session, or clear the dirty Codex session before starting another turn or compaction.",
            self.call_ids.join(", ")
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrepareSubmissionStartError {
    MissingCustomToolOutputs(MissingCustomToolOutputs),
}

impl PrepareSubmissionStartError {
    fn into_codex_err(self) -> CodexErr {
        match self {
            Self::MissingCustomToolOutputs(missing) => missing.into_codex_err(),
        }
    }
}

struct QueuedSubmission {
    submission_id: String,
    op: Op,
}

enum SteerFollowUpAction {
    ReuseActiveSubmission(String),
    QueueFollowUp,
    StartFreshTurn,
}

enum TurnSteerFailure {
    StaleActiveTurn,
    ActiveTurnNotSteerable,
    Other,
}

#[derive(Debug)]
enum PreparedSubmissionStart {
    TurnStart {
        request_id: RequestId,
        params: Box<TurnStartParams>,
    },
    ReviewStart {
        request_id: RequestId,
        params: Box<ReviewStartParams>,
    },
    CompactStart {
        request_id: RequestId,
        params: Box<ThreadCompactStartParams>,
    },
    Rollback {
        request_id: RequestId,
        params: Box<ThreadRollbackParams>,
    },
}

struct OverrideTurnContextArgs {
    cwd: Option<PathBuf>,
    workspace_roots: Option<Vec<codex_utils_absolute_path::AbsolutePathBuf>>,
    profile_workspace_roots: Option<Vec<codex_utils_absolute_path::AbsolutePathBuf>>,
    approval_policy: Option<codex_protocol::protocol::AskForApproval>,
    approvals_reviewer: Option<codex_protocol::config_types::ApprovalsReviewer>,
    sandbox_policy: Option<codex_protocol::protocol::SandboxPolicy>,
    permission_profile: Option<codex_protocol::models::PermissionProfile>,
    active_permission_profile: Option<codex_protocol::models::ActivePermissionProfile>,
    windows_sandbox_level: Option<codex_protocol::config_types::WindowsSandboxLevel>,
    model: Option<String>,
    effort: Option<Option<ReasoningEffort>>,
    summary: Option<ReasoningSummary>,
    collaboration_mode: Option<codex_protocol::config_types::CollaborationMode>,
    personality: Option<codex_protocol::config_types::Personality>,
    service_tier: Option<Option<String>>,
}

impl AppServerCodexThread {
    fn new(
        client: InProcessAppServerClient,
        request_handle: InProcessAppServerRequestHandle,
        thread_id: String,
        config: Config,
        next_request_id: i64,
        thread_status: ThreadStatus,
        turns: Vec<codex_app_server_protocol::Turn>,
    ) -> Self {
        Self {
            client: Mutex::new(Some(client)),
            request_handle,
            state: Mutex::new(AppServerState {
                config,
                next_request_id,
                thread_id,
                active_turn: resumed_active_turn(&thread_status, &turns),
                queued_submissions: VecDeque::new(),
                local_events: VecDeque::new(),
                pending_exec_requests: HashMap::new(),
                pending_patch_requests: HashMap::new(),
                pending_patch_changes: pending_patch_changes_from_turns(&turns),
                pending_permissions_requests: HashMap::new(),
                pending_user_input_requests: HashMap::new(),
                pending_elicitation_requests: HashMap::new(),
                pending_turn_diffs: HashMap::new(),
                pending_custom_tool_calls: HashSet::new(),
                interrupt_after_turn_starts: false,
            }),
        }
    }

    async fn submit_prompt_like(&self, submission_id: String, op: Op) -> Result<String, CodexErr> {
        let active_turn = {
            let state = self.state.lock().await;
            state.active_turn.clone()
        };
        if let Some(active_turn) = active_turn {
            match self.try_steer_submission(active_turn, &op).await? {
                SteerFollowUpAction::ReuseActiveSubmission(steered_submission_id) => {
                    return Ok(steered_submission_id);
                }
                SteerFollowUpAction::QueueFollowUp => {
                    let mut state = self.state.lock().await;
                    if state.active_turn.is_some() {
                        state.queued_submissions.push_back(QueuedSubmission {
                            submission_id: submission_id.clone(),
                            op,
                        });
                    } else {
                        drop(state);
                        self.start_submission(submission_id.clone(), op).await?;
                    }
                    return Ok(submission_id);
                }
                SteerFollowUpAction::StartFreshTurn => {}
            }
        }

        self.start_submission(submission_id.clone(), op).await?;
        Ok(submission_id)
    }

    async fn try_steer_submission(
        &self,
        active_turn: ActiveTurn,
        op: &Op,
    ) -> Result<SteerFollowUpAction, CodexErr> {
        let (items, output_schema, responsesapi_client_metadata) = match op {
            Op::TurnInput { request, .. } => match &request.input {
                ProtocolTurnInput::UserInput { content, .. } => (
                    content.clone(),
                    request.start.final_output_json_schema.clone(),
                    request.responsesapi_client_metadata.clone(),
                ),
                _ => return Ok(SteerFollowUpAction::QueueFollowUp),
            },
            _ => return Ok(SteerFollowUpAction::QueueFollowUp),
        };

        if !active_turn.steerable || active_turn.turn_id.is_none() || output_schema.is_some() {
            return Ok(SteerFollowUpAction::QueueFollowUp);
        }

        let turn_id = active_turn.turn_id.clone().unwrap_or_default();
        let (request_id, thread_id) = {
            let mut state = self.state.lock().await;
            let Some(current_active_turn) = state.active_turn.as_ref() else {
                return Ok(SteerFollowUpAction::StartFreshTurn);
            };
            if !active_turn_matches(current_active_turn, &active_turn) {
                return Ok(SteerFollowUpAction::StartFreshTurn);
            }
            (next_request_id(&mut state), state.thread_id.clone())
        };
        let response = self
            .request_handle
            .request_typed::<TurnSteerResponse>(ClientRequest::TurnSteer {
                request_id,
                params: TurnSteerParams {
                    thread_id,
                    client_user_message_id: None,
                    input: items.into_iter().map(Into::into).collect(),
                    expected_turn_id: turn_id,
                    responsesapi_client_metadata,
                    additional_context: None,
                },
            })
            .await;

        match response {
            Ok(_) => {
                let state = self.state.lock().await;
                if let Some(submission_id) =
                    reused_submission_id_after_successful_turn_steer(&state, &active_turn)
                {
                    return Ok(SteerFollowUpAction::ReuseActiveSubmission(submission_id));
                }
                warn!(
                    "turn/steer succeeded but local active-turn state changed, starting a fresh turn instead"
                );
                Ok(SteerFollowUpAction::StartFreshTurn)
            }
            Err(err) => match classify_turn_steer_failure(&err) {
                TurnSteerFailure::StaleActiveTurn => {
                    let mut state = self.state.lock().await;
                    warn!(
                        "turn/steer reported stale local active-turn state, starting a fresh turn instead: {err}"
                    );
                    if state
                        .active_turn
                        .as_ref()
                        .is_some_and(|turn| active_turn_matches(turn, &active_turn))
                    {
                        clear_active_turn_state(&mut state);
                    }
                    Ok(SteerFollowUpAction::StartFreshTurn)
                }
                TurnSteerFailure::ActiveTurnNotSteerable => {
                    let mut state = self.state.lock().await;
                    warn!(
                        "turn/steer rejected because active turn is not steerable, queueing follow-up prompt: {err}"
                    );
                    if let Some(current_active_turn) = state
                        .active_turn
                        .as_mut()
                        .filter(|turn| active_turn_matches(turn, &active_turn))
                    {
                        current_active_turn.steerable = false;
                    } else {
                        return Ok(SteerFollowUpAction::StartFreshTurn);
                    }
                    Ok(SteerFollowUpAction::QueueFollowUp)
                }
                TurnSteerFailure::Other => {
                    warn!("turn/steer failed, queueing follow-up prompt instead: {err}");
                    let state = self.state.lock().await;
                    if state
                        .active_turn
                        .as_ref()
                        .is_none_or(|turn| !active_turn_matches(turn, &active_turn))
                    {
                        return Ok(SteerFollowUpAction::StartFreshTurn);
                    }
                    Ok(SteerFollowUpAction::QueueFollowUp)
                }
            },
        }
    }

    async fn start_submission(&self, submission_id: String, op: Op) -> Result<(), CodexErr> {
        let prepared = {
            let mut state = self.state.lock().await;
            if state.active_turn.is_some() {
                state
                    .queued_submissions
                    .push_back(QueuedSubmission { submission_id, op });
                return Ok(());
            }
            match prepare_submission_start(&mut state, &submission_id, &op) {
                Ok(Some(prepared)) => prepared,
                Ok(None) => {
                    return Err(CodexErr::UnsupportedOperation(format!(
                        "app-server thread cannot start submission for {}",
                        op.kind()
                    )));
                }
                Err(err) => return Err(err.into_codex_err()),
            }
        };

        match prepared {
            PreparedSubmissionStart::TurnStart { request_id, params } => {
                let response = self
                    .request_handle
                    .request_typed::<TurnStartResponse>(ClientRequest::TurnStart {
                        request_id,
                        params: *params,
                    })
                    .await
                    .map_err(typed_request_error_to_codex);
                let mut state = self.state.lock().await;
                match response {
                    Ok(response) => {
                        set_active_turn_id(
                            &mut state,
                            &submission_id,
                            Some(response.turn.id),
                            true,
                        );
                        Ok(())
                    }
                    Err(err) => {
                        clear_submission_if_active(&mut state, &submission_id);
                        Err(err)
                    }
                }
            }
            PreparedSubmissionStart::ReviewStart { request_id, params } => {
                let response = self
                    .request_handle
                    .request_typed::<ReviewStartResponse>(ClientRequest::ReviewStart {
                        request_id,
                        params: *params,
                    })
                    .await
                    .map_err(typed_request_error_to_codex);
                let mut state = self.state.lock().await;
                match response {
                    Ok(response) => {
                        set_active_turn_id(
                            &mut state,
                            &submission_id,
                            Some(response.turn.id),
                            false,
                        );
                        Ok(())
                    }
                    Err(err) => {
                        clear_submission_if_active(&mut state, &submission_id);
                        Err(err)
                    }
                }
            }
            PreparedSubmissionStart::CompactStart { request_id, params } => {
                let response = self
                    .request_handle
                    .request_typed::<ThreadCompactStartResponse>(
                        ClientRequest::ThreadCompactStart {
                            request_id,
                            params: *params,
                        },
                    )
                    .await
                    .map_err(typed_request_error_to_codex);
                let mut state = self.state.lock().await;
                match response {
                    Ok(_) => Ok(()),
                    Err(err) => {
                        clear_submission_if_active(&mut state, &submission_id);
                        Err(err)
                    }
                }
            }
            PreparedSubmissionStart::Rollback { request_id, params } => {
                let response = self
                    .request_handle
                    .request_typed::<ThreadRollbackResponse>(ClientRequest::ThreadRollback {
                        request_id,
                        params: *params,
                    })
                    .await
                    .map_err(typed_request_error_to_codex);
                let mut state = self.state.lock().await;
                match response {
                    Ok(_) => {
                        state.local_events.push_back(Event {
                            id: submission_id.clone(),
                            msg: EventMsg::AgentMessage(AgentMessageEvent {
                                message: "Undo completed.".to_string(),
                                phase: None,
                                memory_citation: None,
                                delivery: None,
                            }),
                        });
                        state.local_events.push_back(Event {
                            id: submission_id,
                            msg: EventMsg::TurnComplete(TurnCompleteEvent {
                                turn_id: String::new(),
                                last_agent_message: None,
                                error: None,
                                started_at: None,
                                completed_at: None,
                                duration_ms: None,
                                time_to_first_token_ms: None,
                            }),
                        });
                        Ok(())
                    }
                    Err(err) => Err(err),
                }
            }
        }
    }

    async fn start_next_queued_submissions(&self) {
        loop {
            let queued = {
                let mut state = self.state.lock().await;
                if state.active_turn.is_some() {
                    return;
                }
                state.queued_submissions.pop_front()
            };

            let Some(queued) = queued else {
                return;
            };

            match self
                .start_submission(queued.submission_id.clone(), queued.op)
                .await
            {
                Ok(()) => return,
                Err(err) => {
                    let mut state = self.state.lock().await;
                    state.local_events.push_back(Event {
                        id: queued.submission_id,
                        msg: EventMsg::Error(ErrorEvent {
                            message: err.to_string(),
                            codex_error_info: None,
                        }),
                    });
                }
            }
        }
    }

    async fn interrupt_active_turn(&self) -> Result<String, CodexErr> {
        let (submission_id, pending_interrupt) = {
            let mut state = self.state.lock().await;
            mark_active_turn_interrupted(&mut state)
        };

        if let Some(pending_interrupt) = pending_interrupt {
            let request_handle = self.request_handle.clone();
            tokio::task::spawn_local(async move {
                if let Err(err) = request_handle
                    .request_typed::<TurnInterruptResponse>(ClientRequest::TurnInterrupt {
                        request_id: pending_interrupt.request_id,
                        params: TurnInterruptParams {
                            thread_id: pending_interrupt.thread_id,
                            turn_id: pending_interrupt.turn_id,
                        },
                    })
                    .await
                {
                    warn!("best-effort turn interrupt request failed: {err}");
                }
            });
        }

        Ok(submission_id)
    }

    async fn shutdown_thread(&self) -> Result<String, CodexErr> {
        let active_submission_id = {
            let mut state = self.state.lock().await;
            cancel_queued_submissions(&mut state);
            let active_submission_id = state
                .active_turn
                .as_ref()
                .map(|turn| turn.submission_id.clone());
            if let Some(submission_id) = active_submission_id.clone() {
                state.local_events.push_back(Event {
                    id: submission_id,
                    msg: EventMsg::ShutdownComplete,
                });
            }
            state.active_turn = None;
            active_submission_id
        };

        if let Some(client) = self.client.lock().await.take() {
            client.shutdown().await?;
        }

        Ok(active_submission_id.unwrap_or_else(noop_submission_id))
    }

    async fn override_turn_context(
        &self,
        args: OverrideTurnContextArgs,
    ) -> Result<String, CodexErr> {
        let (updated_config, request_id, thread_id) = {
            let mut state = self.state.lock().await;
            let mut updated_config = state.config.clone();

            if let Some(cwd) = args.cwd {
                updated_config.cwd = cwd.try_into()?;
            }
            if let Some(approval_policy) = args.approval_policy {
                updated_config
                    .permissions
                    .approval_policy
                    .set(approval_policy)
                    .map_err(|err| CodexErr::Fatal(err.to_string()))?;
            }
            if let Some(approvals_reviewer) = args.approvals_reviewer {
                updated_config.approvals_reviewer = approvals_reviewer;
            }
            if let Some(sandbox_policy) = args.sandbox_policy {
                updated_config
                    .permissions
                    .set_legacy_sandbox_policy(sandbox_policy, updated_config.cwd.as_path())
                    .map_err(|err| CodexErr::Fatal(err.to_string()))?;
            }
            if let Some(permission_profile) = args.permission_profile {
                updated_config
                    .permissions
                    .set_permission_profile(permission_profile)
                    .map_err(|err| CodexErr::Fatal(err.to_string()))?;
            }
            if let Some(model) = args.model {
                updated_config.model = Some(model);
            }
            if let Some(effort) = args.effort {
                updated_config.model_reasoning_effort = effort;
            }
            if let Some(summary) = args.summary {
                updated_config.model_reasoning_summary = Some(summary);
            }
            if let Some(personality) = args.personality {
                updated_config.personality = Some(personality);
            }
            if let Some(service_tier) = args.service_tier {
                updated_config.service_tier = service_tier;
            }
            if args.workspace_roots.is_some() {
                warn!(
                    "ignoring ThreadSettings.workspace_roots because app-server resume params do not expose runtime workspace roots through the ACP adapter yet"
                );
            }
            if args.profile_workspace_roots.is_some() {
                warn!(
                    "ignoring ThreadSettings.profile_workspace_roots because app-server resume params do not expose profile workspace roots through the ACP adapter yet"
                );
            }
            if args.active_permission_profile.is_some() {
                warn!(
                    "ignoring ThreadSettings.active_permission_profile because the ACP adapter currently reapplies only the resolved permission profile snapshot"
                );
            }
            if args.windows_sandbox_level.is_some() {
                warn!(
                    "ignoring ThreadSettings.windows_sandbox_level because the ACP adapter does not currently project Windows sandbox level into app-server resume params"
                );
            }
            if args.collaboration_mode.is_some() {
                warn!(
                    "ignoring ThreadSettings.collaboration_mode because the ACP adapter does not currently project collaboration mode into app-server resume params"
                );
            }

            let request_id = next_request_id(&mut state);
            let thread_id = state.thread_id.clone();
            (updated_config, request_id, thread_id)
        };

        let _response: ThreadResumeResponse = self
            .request_handle
            .request_typed(ClientRequest::ThreadResume {
                request_id,
                params: thread_resume_params_from_config(
                    &updated_config,
                    &SessionId::new(thread_id),
                ),
            })
            .await
            .map_err(typed_request_error_to_codex)?;

        let mut state = self.state.lock().await;
        state.config = updated_config;
        Ok(noop_submission_id())
    }

    async fn resolve_exec_request(
        &self,
        id: String,
        decision: ReviewDecision,
    ) -> Result<String, CodexErr> {
        let request_id = {
            let mut state = self.state.lock().await;
            state.pending_exec_requests.remove(&id)
        }
        .ok_or_else(|| CodexErr::InvalidRequest(format!("unknown exec approval id: {id}")))?;

        self.resolve_server_request(
            request_id,
            serde_json::to_value(CommandExecutionRequestApprovalResponse {
                decision: review_decision_to_app_server(decision),
            })
            .map_err(|err| CodexErr::Fatal(err.to_string()))?,
        )
        .await?;

        Ok(noop_submission_id())
    }

    async fn resolve_patch_request(
        &self,
        id: String,
        decision: ReviewDecision,
    ) -> Result<String, CodexErr> {
        let request_id = {
            let mut state = self.state.lock().await;
            state.pending_patch_requests.remove(&id)
        }
        .ok_or_else(|| CodexErr::InvalidRequest(format!("unknown patch approval id: {id}")))?;

        self.resolve_server_request(
            request_id,
            serde_json::to_value(FileChangeRequestApprovalResponse {
                decision: patch_review_decision_to_app_server(decision),
            })
            .map_err(|err| CodexErr::Fatal(err.to_string()))?,
        )
        .await?;

        Ok(noop_submission_id())
    }

    async fn resolve_permissions_request(
        &self,
        id: String,
        response: RequestPermissionsResponse,
    ) -> Result<String, CodexErr> {
        let request_id = {
            let mut state = self.state.lock().await;
            state.pending_permissions_requests.remove(&id)
        }
        .ok_or_else(|| CodexErr::InvalidRequest(format!("unknown permissions request id: {id}")))?;

        let permissions = serde_json::to_value(PermissionsRequestApprovalResponse {
            permissions: codex_app_server_protocol::GrantedPermissionProfile {
                network: response.permissions.network.map(Into::into),
                file_system: response.permissions.file_system.map(Into::into),
            },
            scope: match response.scope {
                PermissionGrantScope::Turn => AppPermissionGrantScope::Turn,
                PermissionGrantScope::Session => AppPermissionGrantScope::Session,
            },
            strict_auto_review: Some(response.strict_auto_review),
        })
        .map_err(|err| CodexErr::Fatal(err.to_string()))?;

        self.resolve_server_request(request_id, permissions).await?;
        Ok(noop_submission_id())
    }

    async fn resolve_user_input_request(
        &self,
        id: String,
        response: RequestUserInputResponse,
    ) -> Result<String, CodexErr> {
        let request_id = {
            let mut state = self.state.lock().await;
            state.pending_user_input_requests.remove(&id)
        }
        .ok_or_else(|| CodexErr::InvalidRequest(format!("unknown user input request id: {id}")))?;

        let response = serde_json::to_value(request_user_input_response_to_app_server(response))
            .map_err(|err| CodexErr::Fatal(err.to_string()))?;

        self.resolve_server_request(request_id, response).await?;
        Ok(noop_submission_id())
    }

    async fn resolve_elicitation(
        &self,
        server_name: String,
        request_id: McpRequestId,
        decision: ElicitationAction,
        content: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) -> Result<String, CodexErr> {
        let key = elicitation_request_key(&server_name, &request_id);
        let app_request_id = {
            let mut state = self.state.lock().await;
            state.pending_elicitation_requests.remove(&key)
        }
        .ok_or_else(|| CodexErr::InvalidRequest(format!("unknown elicitation request: {key}")))?;

        self.resolve_server_request(
            app_request_id,
            serde_json::to_value(McpServerElicitationRequestResponse {
                action: match decision {
                    ElicitationAction::Accept => McpServerElicitationAction::Accept,
                    ElicitationAction::Decline => McpServerElicitationAction::Decline,
                    ElicitationAction::Cancel => McpServerElicitationAction::Cancel,
                },
                content,
                meta,
            })
            .map_err(|err| CodexErr::Fatal(err.to_string()))?,
        )
        .await?;

        Ok(noop_submission_id())
    }

    async fn resolve_server_request(
        &self,
        request_id: RequestId,
        result: serde_json::Value,
    ) -> Result<(), CodexErr> {
        let guard = self.client.lock().await;
        let Some(client) = guard.as_ref() else {
            return Err(CodexErr::InternalAgentDied);
        };
        client.resolve_server_request(request_id, result).await?;
        Ok(())
    }

    async fn reject_server_request(
        &self,
        request_id: RequestId,
        message: &str,
    ) -> Result<(), CodexErr> {
        let guard = self.client.lock().await;
        let Some(client) = guard.as_ref() else {
            return Err(CodexErr::InternalAgentDied);
        };
        client
            .reject_server_request(
                request_id,
                codex_app_server_protocol::JSONRPCErrorError {
                    code: -32000,
                    message: message.to_string(),
                    data: None,
                },
            )
            .await?;
        Ok(())
    }

    async fn pop_local_event(&self) -> Option<Event> {
        let mut state = self.state.lock().await;
        state.local_events.pop_front()
    }

    async fn translate_server_event(
        &self,
        event: InProcessServerEvent,
    ) -> Result<Option<Event>, CodexErr> {
        match event {
            InProcessServerEvent::Lagged { skipped } => {
                warn!(
                    "dropping best-effort app-server notifications because consumer lagged by {skipped} events"
                );
                Ok(None)
            }
            InProcessServerEvent::ServerRequest(request) => {
                self.translate_server_request(*request).await
            }
            InProcessServerEvent::ServerNotification(notification) => {
                self.translate_server_notification(*notification).await
            }
        }
    }

    async fn translate_server_request(
        &self,
        request: ServerRequest,
    ) -> Result<Option<Event>, CodexErr> {
        match request {
            ServerRequest::CommandExecutionRequestApproval { request_id, params } => {
                let mut state = self.state.lock().await;
                let command = params.command.unwrap_or_default();
                let command_vec = shell_command_vec(&command);
                let approval_key = params
                    .approval_id
                    .clone()
                    .unwrap_or_else(|| params.item_id.clone());
                let submission_id = submission_id_for_turn_or_fallback(&state, &params.turn_id)
                    .unwrap_or_else(noop_submission_id);
                state
                    .pending_exec_requests
                    .insert(approval_key.clone(), request_id);

                Ok(Some(Event {
                    id: submission_id,
                    msg: EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                        kind: codex_protocol::approvals::ExecApprovalKind::Command,
                        call_id: params.item_id,
                        plugin_id: None,
                        script_path: None,
                        approval_id: params.approval_id,
                        turn_id: params.turn_id,
                        environment_id: params.environment_id,
                        started_at_ms: params.started_at_ms,
                        command: command_vec.clone(),
                        cwd: params
                            .cwd
                            .and_then(|cwd| {
                                match codex_utils_absolute_path::AbsolutePathBuf::try_from(cwd) {
                                    Ok(path) => Some(path),
                                    Err(err) => {
                                        tracing::warn!(
                                            ?err,
                                            "exec-approval cwd is not an absolute path; falling back to configured cwd"
                                        );
                                        None
                                    }
                                }
                            })
                            .unwrap_or_else(|| state.config.cwd.clone()),
                        reason: params.reason,
                        network_approval_context: params
                            .network_approval_context
                            .map(app_server_network_approval_context_to_core),
                        proposed_execpolicy_amendment: params
                            .proposed_execpolicy_amendment
                            .map(codex_app_server_protocol::ExecPolicyAmendment::into_core),
                        proposed_network_policy_amendments: params
                            .proposed_network_policy_amendments
                            .map(|items| {
                                items
                                    .into_iter()
                                    .map(codex_app_server_protocol::NetworkPolicyAmendment::into_core)
                                    .collect()
                            }),
                        additional_permissions: params
                            .additional_permissions
                            .map(codex_protocol::models::AdditionalPermissionProfile::try_from)
                            .transpose()
                            .map_err(|err| CodexErr::Fatal(err.to_string()))?,
                        available_decisions: params.available_decisions.map(|items| {
                            items
                                .into_iter()
                                .map(app_server_review_decision_to_core)
                                .collect()
                        }),
                        parsed_cmd: parse_command(&command_vec),
                    }),
                }))
            }
            ServerRequest::FileChangeRequestApproval { request_id, params } => {
                let mut state = self.state.lock().await;
                let submission_id = submission_id_for_turn_or_fallback(&state, &params.turn_id)
                    .unwrap_or_else(noop_submission_id);
                let changes =
                    pending_patch_changes_for_request(&state, &params.item_id, &params.turn_id);
                state
                    .pending_patch_requests
                    .insert(params.item_id.clone(), request_id);

                Ok(Some(Event {
                    id: submission_id,
                    msg: EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                        call_id: params.item_id,
                        turn_id: params.turn_id,
                        started_at_ms: params.started_at_ms,
                        changes,
                        reason: params.reason,
                        grant_root: params.grant_root,
                    }),
                }))
            }
            ServerRequest::PermissionsRequestApproval { request_id, params } => {
                let mut state = self.state.lock().await;
                let submission_id = submission_id_for_turn_or_fallback(&state, &params.turn_id)
                    .unwrap_or_else(noop_submission_id);
                state
                    .pending_permissions_requests
                    .insert(params.item_id.clone(), request_id);

                Ok(Some(Event {
                    id: submission_id,
                    msg: EventMsg::RequestPermissions(permissions_request_event_from_params(
                        params,
                    )?),
                }))
            }
            ServerRequest::McpServerElicitationRequest { request_id, params } => {
                let mut state = self.state.lock().await;
                let mcp_request_id = server_request_id_to_mcp_request_id(&request_id);
                let submission_id = submission_id_for_turn_or_fallback(
                    &state,
                    params.turn_id.as_deref().unwrap_or_default(),
                )
                .unwrap_or_else(noop_submission_id);
                let request = match params.request {
                    codex_app_server_protocol::McpServerElicitationRequest::Form {
                        meta,
                        message,
                        requested_schema,
                    } => ElicitationRequest::Form {
                        meta,
                        message,
                        requested_schema: serde_json::to_value(requested_schema)
                            .map_err(|err| CodexErr::Fatal(err.to_string()))?,
                    },
                    codex_app_server_protocol::McpServerElicitationRequest::OpenAiForm {
                        meta,
                        message,
                        requested_schema,
                    } => ElicitationRequest::OpenAiForm {
                        meta,
                        message,
                        requested_schema,
                    },
                    codex_app_server_protocol::McpServerElicitationRequest::Url {
                        meta,
                        message,
                        url,
                        elicitation_id,
                    } => ElicitationRequest::Url {
                        meta,
                        message,
                        url,
                        elicitation_id,
                    },
                };
                state.pending_elicitation_requests.insert(
                    elicitation_request_key(&params.server_name, &mcp_request_id),
                    request_id,
                );

                Ok(Some(Event {
                    id: submission_id,
                    msg: EventMsg::ElicitationRequest(ElicitationRequestEvent {
                        turn_id: params.turn_id,
                        server_name: params.server_name,
                        id: mcp_request_id,
                        request,
                    }),
                }))
            }
            ServerRequest::ToolRequestUserInput { request_id, params } => {
                let mut state = self.state.lock().await;
                let submission_id = submission_id_for_turn_or_fallback(&state, &params.turn_id)
                    .unwrap_or_else(noop_submission_id);
                state
                    .pending_user_input_requests
                    .insert(params.turn_id.clone(), request_id);

                Ok(Some(Event {
                    id: submission_id,
                    msg: EventMsg::RequestUserInput(app_server_request_user_input_to_core(params)),
                }))
            }
            ServerRequest::DynamicToolCall { request_id, .. } => {
                self.reject_server_request(request_id, DYNAMIC_TOOL_CALLBACK_UNSUPPORTED_MESSAGE)
                    .await?;
                Ok(None)
            }
            ServerRequest::AttestationGenerate { request_id, .. } => {
                self.reject_server_request(request_id, ATTESTATION_GENERATION_UNSUPPORTED_MESSAGE)
                    .await?;
                Ok(None)
            }
            ServerRequest::ChatgptAuthTokensRefresh { .. }
            | ServerRequest::ApplyPatchApproval { .. }
            | ServerRequest::ExecCommandApproval { .. }
            // The ACP adapter does not provide an external current-time source,
            // so there is no core event to translate this request into.
            | ServerRequest::CurrentTimeRead { .. } => Ok(None),
        }
    }

    async fn translate_server_notification(
        &self,
        notification: ServerNotification,
    ) -> Result<Option<Event>, CodexErr> {
        match notification {
            ServerNotification::ThreadNameUpdated(_)
            | ServerNotification::ThreadSettingsUpdated(_)
            | ServerNotification::ThreadDeleted(_)
            | ServerNotification::TurnModerationMetadata(_)
            | ServerNotification::EnvironmentConnected(_)
            | ServerNotification::EnvironmentDisconnected(_)
            | ServerNotification::RawResponseCompleted(_)
            | ServerNotification::McpServerEventStream(_)
            | ServerNotification::ThreadRealtimeItemStarted(_)
            | ServerNotification::ThreadRealtimeItemTranscriptDelta(_)
            | ServerNotification::ThreadRealtimeItemCompleted(_) => Ok(None),
            ServerNotification::FileChangePatchUpdated(payload) => {
                let submission_id = active_submission_id_for_turn(self, &payload.turn_id).await;
                Ok(submission_id.map(|id| Event {
                    id,
                    msg: EventMsg::PatchApplyUpdated(file_change_patch_updated_to_core(payload)),
                }))
            }
            ServerNotification::ModelVerification(payload) => {
                let submission_id = active_submission_id_for_turn(self, &payload.turn_id)
                    .await
                    .unwrap_or_else(noop_submission_id);
                Ok(Some(Event {
                    id: submission_id,
                    msg: EventMsg::ModelVerification(model_verification_to_core(payload)),
                }))
            }
            ServerNotification::GuardianWarning(payload) => {
                let submission_id = {
                    let state = self.state.lock().await;
                    active_submission_id(&state).unwrap_or_else(noop_submission_id)
                };
                Ok(Some(Event {
                    id: submission_id,
                    msg: EventMsg::GuardianWarning(guardian_warning_to_core(payload)),
                }))
            }
            ServerNotification::TurnStarted(payload) => {
                let (submission_id, interrupt_request) = {
                    let mut state = self.state.lock().await;
                    let submission_id = if let Some(active_turn) = state.active_turn.as_mut() {
                        active_turn.turn_id = Some(payload.turn.id.clone());
                        active_turn.submission_id.clone()
                    } else {
                        let submission_id = payload.turn.id.clone();
                        state.active_turn = Some(ActiveTurn {
                            submission_id: submission_id.clone(),
                            turn_id: Some(payload.turn.id.clone()),
                            steerable: false,
                            last_agent_message: None,
                        });
                        submission_id
                    };

                    let interrupt_request = if state.interrupt_after_turn_starts {
                        state.interrupt_after_turn_starts = false;
                        let request_id = next_request_id(&mut state);
                        let thread_id = state.thread_id.clone();
                        Some((request_id, thread_id, payload.turn.id.clone()))
                    } else {
                        None
                    };
                    (submission_id, interrupt_request)
                };

                if let Some((request_id, thread_id, turn_id)) = interrupt_request {
                    let _: TurnInterruptResponse = self
                        .request_handle
                        .request_typed(ClientRequest::TurnInterrupt {
                            request_id,
                            params: TurnInterruptParams { thread_id, turn_id },
                        })
                        .await
                        .map_err(typed_request_error_to_codex)?;
                }

                Ok(Some(Event {
                    id: submission_id,
                    msg: EventMsg::TurnStarted(TurnStartedEvent {
                        turn_id: payload.turn.id,
                        trace_id: None,
                        started_at: payload.turn.started_at,
                        model_context_window: None,
                        collaboration_mode_kind: Default::default(),
                    }),
                }))
            }
            ServerNotification::ThreadTokenUsageUpdated(payload) => {
                let submission_id = {
                    let state = self.state.lock().await;
                    submission_id_for_turn_or_fallback(&state, &payload.turn_id)
                };
                Ok(submission_id.map(|id| Event {
                    id,
                    msg: EventMsg::TokenCount(TokenCountEvent {
                        info: Some(TokenUsageInfo {
                            total_token_usage: TokenUsage {
                                input_tokens: payload.token_usage.total.input_tokens,
                                cached_input_tokens: payload.token_usage.total.cached_input_tokens,
                                cache_write_input_tokens: payload
                                    .token_usage
                                    .total
                                    .cache_write_input_tokens,
                                output_tokens: payload.token_usage.total.output_tokens,
                                reasoning_output_tokens: payload
                                    .token_usage
                                    .total
                                    .reasoning_output_tokens,
                                total_tokens: payload.token_usage.total.total_tokens,
                                codex_rollout_budget_units: None,
                            },
                            last_token_usage: TokenUsage {
                                input_tokens: payload.token_usage.last.input_tokens,
                                cached_input_tokens: payload.token_usage.last.cached_input_tokens,
                                cache_write_input_tokens: payload
                                    .token_usage
                                    .last
                                    .cache_write_input_tokens,
                                output_tokens: payload.token_usage.last.output_tokens,
                                reasoning_output_tokens: payload
                                    .token_usage
                                    .last
                                    .reasoning_output_tokens,
                                total_tokens: payload.token_usage.last.total_tokens,
                                codex_rollout_budget_units: None,
                            },
                            model_context_window: payload.token_usage.model_context_window,
                        }),
                        rate_limits: None,
                    }),
                }))
            }
            ServerNotification::ThreadRealtimeSdp(_) => Ok(None),
            ServerNotification::TurnPlanUpdated(payload) => {
                let submission_id = active_submission_id_for_turn(self, &payload.turn_id).await;
                Ok(submission_id.map(|id| Event {
                    id,
                    msg: EventMsg::PlanUpdate(UpdatePlanArgs {
                        explanation: payload.explanation,
                        plan: payload
                            .plan
                            .into_iter()
                            .map(|step| PlanItemArg {
                                step: step.step,
                                status: match step.status {
                                    codex_app_server_protocol::TurnPlanStepStatus::Pending => {
                                        StepStatus::Pending
                                    }
                                    codex_app_server_protocol::TurnPlanStepStatus::InProgress => {
                                        StepStatus::InProgress
                                    }
                                    codex_app_server_protocol::TurnPlanStepStatus::Completed => {
                                        StepStatus::Completed
                                    }
                                },
                            })
                            .collect(),
                    }),
                }))
            }
            ServerNotification::AgentMessageDelta(payload) => {
                Ok(active_submission_id_for_turn(self, &payload.turn_id)
                    .await
                    .map(|id| Event {
                        id,
                        msg: EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent {
                            thread_id: payload.thread_id,
                            turn_id: payload.turn_id,
                            item_id: payload.item_id,
                            delta: payload.delta,
                        }),
                    }))
            }
            ServerNotification::ReasoningSummaryTextDelta(payload) => {
                Ok(active_submission_id_for_turn(self, &payload.turn_id)
                    .await
                    .map(|id| Event {
                        id,
                        msg: EventMsg::ReasoningContentDelta(
                            codex_protocol::protocol::ReasoningContentDeltaEvent {
                                thread_id: payload.thread_id,
                                turn_id: payload.turn_id,
                                item_id: payload.item_id,
                                delta: payload.delta,
                                summary_index: payload.summary_index,
                            },
                        ),
                    }))
            }
            ServerNotification::ReasoningTextDelta(payload) => {
                Ok(active_submission_id_for_turn(self, &payload.turn_id)
                    .await
                    .map(|id| Event {
                        id,
                        msg: EventMsg::ReasoningRawContentDelta(
                            codex_protocol::protocol::ReasoningRawContentDeltaEvent {
                                thread_id: payload.thread_id,
                                turn_id: payload.turn_id,
                                item_id: payload.item_id,
                                delta: payload.delta,
                                content_index: payload.content_index,
                            },
                        ),
                    }))
            }
            ServerNotification::TerminalInteraction(payload) => {
                Ok(active_submission_id_for_turn(self, &payload.turn_id)
                    .await
                    .map(|id| Event {
                        id,
                        msg: EventMsg::TerminalInteraction(TerminalInteractionEvent {
                            call_id: payload.item_id,
                            process_id: payload.process_id,
                            stdin: payload.stdin,
                        }),
                    }))
            }
            ServerNotification::CommandExecutionOutputDelta(payload) => {
                Ok(active_submission_id_for_turn(self, &payload.turn_id)
                    .await
                    .map(|id| Event {
                        id,
                        msg: EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                            call_id: payload.item_id,
                            stream: codex_protocol::protocol::ExecOutputStream::Stdout,
                            chunk: payload.delta.into_bytes(),
                        }),
                    }))
            }
            ServerNotification::ItemStarted(payload) => {
                let submission_id = active_submission_id_for_turn(self, &payload.turn_id).await;
                Ok(match (submission_id, payload.item) {
                    (
                        Some(id),
                        item @ codex_app_server_protocol::ThreadItem::CommandExecution { .. },
                    ) => {
                        let fallback_cwd = self.state.lock().await.config.cwd.clone();
                        app_server_command_begin_event_from_item(
                            payload.turn_id,
                            payload.started_at_ms,
                            item,
                            &fallback_cwd,
                        )
                        .map(|event| Event {
                            id,
                            msg: EventMsg::ExecCommandBegin(event),
                        })
                    }
                    (
                        Some(id),
                        codex_app_server_protocol::ThreadItem::McpToolCall {
                            id: call_id,
                            server,
                            tool,
                            arguments,
                            app_context,
                            mcp_app_resource_uri,
                            plugin_id,
                            read_only_hint,
                            ..
                        },
                    ) => {
                        let (connector_id, link_id, resource_uri, app_name, action_name) =
                            app_context
                                .map(|context| {
                                    (
                                        Some(context.connector_id),
                                        context.link_id,
                                        context.resource_uri,
                                        context.app_name,
                                        context.action_name,
                                    )
                                })
                                .unwrap_or_default();
                        Some(Event {
                            id,
                            msg: EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                                call_id,
                                invocation: McpInvocation {
                                    server,
                                    tool,
                                    arguments: Some(arguments),
                                },
                                connector_id,
                                mcp_app_resource_uri: resource_uri.or(mcp_app_resource_uri),
                                link_id,
                                app_name,
                                action_name,
                                plugin_id,
                                read_only_hint,
                            }),
                        })
                    }
                    (
                        Some(id),
                        codex_app_server_protocol::ThreadItem::DynamicToolCall {
                            id: call_id,
                            tool,
                            arguments,
                            ..
                        },
                    ) => Some(Event {
                        id,
                        msg: EventMsg::DynamicToolCallRequest(DynamicToolCallRequest {
                            call_id,
                            turn_id: payload.turn_id,
                            started_at_ms: payload.started_at_ms,
                            tool,
                            arguments,
                            namespace: None,
                        }),
                    }),
                    (
                        Some(id),
                        item @ (codex_app_server_protocol::ThreadItem::CollabAgentToolCall {
                            ..
                        }
                        | codex_app_server_protocol::ThreadItem::SubAgentActivity { .. }),
                    ) => app_server_item_started_to_core(
                        payload.thread_id,
                        payload.turn_id,
                        payload.started_at_ms,
                        item,
                    )
                    .map(|msg| Event { id, msg }),
                    (
                        Some(id),
                        codex_app_server_protocol::ThreadItem::FileChange {
                            id: call_id,
                            changes,
                            ..
                        },
                    ) => {
                        let core_changes = app_server_changes_to_core(changes);
                        let mut state = self.state.lock().await;
                        state
                            .pending_patch_changes
                            .insert(call_id.clone(), core_changes.clone());
                        Some(Event {
                            id,
                            msg: EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                                call_id,
                                turn_id: payload.turn_id,
                                auto_approved: true,
                                changes: core_changes,
                            }),
                        })
                    }
                    (Some(id), codex_app_server_protocol::ThreadItem::WebSearch(item)) => {
                        Some(Event {
                            id,
                            msg: EventMsg::WebSearchBegin(WebSearchBeginEvent { call_id: item.id }),
                        })
                    }
                    (Some(id), codex_app_server_protocol::ThreadItem::ImageGeneration(item)) => {
                        Some(Event {
                            id,
                            msg: EventMsg::ImageGenerationBegin(ImageGenerationBeginEvent {
                                call_id: item.id,
                            }),
                        })
                    }
                    (
                        Some(id),
                        codex_app_server_protocol::ThreadItem::ImageView { id: call_id, path },
                    ) => path.to_inferred_path_uri().map(|path| Event {
                        id,
                        msg: EventMsg::ViewImageToolCall(ViewImageToolCallEvent { call_id, path }),
                    }),
                    _ => None,
                })
            }
            ServerNotification::ItemCompleted(payload) => {
                let submission_id = active_submission_id_for_turn(self, &payload.turn_id).await;
                Ok(match (submission_id, payload.item) {
                    (
                        Some(id),
                        codex_app_server_protocol::ThreadItem::AgentMessage {
                            text,
                            phase,
                            memory_citation,
                            delivery,
                            ..
                        },
                    ) => {
                        let mut state = self.state.lock().await;
                        if let Some(active_turn) = state.active_turn.as_mut() {
                            active_turn.last_agent_message = Some(text.clone());
                        }
                        Some(Event {
                            id,
                            msg: EventMsg::AgentMessage(AgentMessageEvent {
                                message: text,
                                phase,
                                memory_citation: memory_citation
                                    .map(app_server_memory_citation_to_core),
                                delivery,
                            }),
                        })
                    }
                    (
                        Some(id),
                        codex_app_server_protocol::ThreadItem::Reasoning {
                            summary, content, ..
                        },
                    ) => Some(Event {
                        id,
                        msg: if !content.is_empty() {
                            EventMsg::AgentReasoningRawContent(
                                codex_protocol::protocol::AgentReasoningRawContentEvent {
                                    text: content.join(""),
                                },
                            )
                        } else {
                            EventMsg::AgentReasoning(
                                codex_protocol::protocol::AgentReasoningEvent {
                                    text: summary.join(""),
                                },
                            )
                        },
                    }),
                    (
                        Some(id),
                        codex_app_server_protocol::ThreadItem::EnteredReviewMode { review, .. },
                    ) => Some(Event {
                        id,
                        msg: EventMsg::EnteredReviewMode(app_server_entered_review_mode_to_core(
                            review,
                        )),
                    }),
                    (
                        Some(id),
                        codex_app_server_protocol::ThreadItem::ExitedReviewMode { review, .. },
                    ) => Some(Event {
                        id,
                        msg: EventMsg::ExitedReviewMode(app_server_exited_review_mode_to_core(
                            review,
                        )),
                    }),
                    (
                        Some(id),
                        item @ codex_app_server_protocol::ThreadItem::CommandExecution { .. },
                    ) => {
                        let fallback_cwd = self.state.lock().await.config.cwd.clone();
                        app_server_command_end_event_from_item(
                            payload.turn_id,
                            payload.completed_at_ms,
                            item,
                            &fallback_cwd,
                        )
                        .map(|event| Event {
                            id,
                            msg: EventMsg::ExecCommandEnd(event),
                        })
                    }
                    (
                        Some(id),
                        codex_app_server_protocol::ThreadItem::FileChange {
                            id: call_id,
                            changes,
                            status,
                        },
                    ) => {
                        let core_changes = app_server_changes_to_core(changes);
                        let mut state = self.state.lock().await;
                        state.pending_patch_changes.remove(&call_id);
                        Some(Event {
                            id,
                            msg: EventMsg::PatchApplyEnd(PatchApplyEndEvent {
                                call_id,
                                turn_id: payload.turn_id,
                                stdout: String::new(),
                                stderr: String::new(),
                                success: status
                                    == codex_app_server_protocol::PatchApplyStatus::Completed,
                                changes: core_changes,
                                status: app_server_patch_status_to_core(status),
                            }),
                        })
                    }
                    (
                        Some(id),
                        codex_app_server_protocol::ThreadItem::McpToolCall {
                            id: call_id,
                            server,
                            tool,
                            arguments,
                            app_context,
                            mcp_app_resource_uri,
                            plugin_id,
                            read_only_hint,
                            result,
                            error,
                            duration_ms,
                            ..
                        },
                    ) => {
                        let (connector_id, link_id, resource_uri, app_name, action_name) =
                            app_context
                                .map(|context| {
                                    (
                                        Some(context.connector_id),
                                        context.link_id,
                                        context.resource_uri,
                                        context.app_name,
                                        context.action_name,
                                    )
                                })
                                .unwrap_or_default();
                        let result = match (result, error) {
                            (Some(result), None) => Ok(CallToolResult {
                                content: result.content,
                                structured_content: result.structured_content,
                                is_error: Some(false),
                                meta: None,
                            }),
                            (_, Some(err)) => Err(err.message),
                            (None, None) => Ok(CallToolResult {
                                content: Vec::new(),
                                structured_content: None,
                                is_error: Some(false),
                                meta: None,
                            }),
                        };
                        Some(Event {
                            id,
                            msg: EventMsg::McpToolCallEnd(McpToolCallEndEvent {
                                call_id,
                                invocation: McpInvocation {
                                    server,
                                    tool,
                                    arguments: Some(arguments),
                                },
                                connector_id,
                                mcp_app_resource_uri: resource_uri.or(mcp_app_resource_uri),
                                link_id,
                                app_name,
                                action_name,
                                plugin_id,
                                read_only_hint,
                                duration: duration_ms
                                    .and_then(|ms| u64::try_from(ms).ok())
                                    .map(Duration::from_millis)
                                    .unwrap_or_default(),
                                result,
                            }),
                        })
                    }
                    (
                        Some(id),
                        codex_app_server_protocol::ThreadItem::DynamicToolCall {
                            id: call_id,
                            tool,
                            arguments,
                            content_items,
                            success,
                            duration_ms,
                            ..
                        },
                    ) => Some(Event {
                        id,
                        msg: EventMsg::DynamicToolCallResponse(
                            codex_protocol::protocol::DynamicToolCallResponseEvent {
                                call_id,
                                turn_id: payload.turn_id,
                                completed_at_ms: payload.completed_at_ms,
                                tool,
                                arguments,
                                namespace: None,
                                content_items: content_items
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(Into::into)
                                    .collect(),
                                success: success.unwrap_or(false),
                                error: None,
                                duration: duration_ms
                                    .and_then(|ms| u64::try_from(ms).ok())
                                    .map(Duration::from_millis)
                                    .unwrap_or_default(),
                            },
                        ),
                    }),
                    (
                        Some(id),
                        item @ (codex_app_server_protocol::ThreadItem::CollabAgentToolCall {
                            ..
                        }
                        | codex_app_server_protocol::ThreadItem::SubAgentActivity { .. }),
                    ) => app_server_item_completed_to_core(
                        payload.thread_id,
                        payload.turn_id,
                        payload.completed_at_ms,
                        item,
                    )
                    .map(|msg| Event { id, msg }),
                    (Some(id), codex_app_server_protocol::ThreadItem::WebSearch(item)) => {
                        Some(Event {
                            id,
                            msg: EventMsg::WebSearchEnd(WebSearchEndEvent {
                                call_id: item.id,
                                query: item.query,
                                action: item
                                    .action
                                    .map(app_server_web_search_action_to_core)
                                    .unwrap_or(codex_protocol::models::WebSearchAction::Other),
                                results: item.results,
                            }),
                        })
                    }
                    (Some(id), codex_app_server_protocol::ThreadItem::ImageGeneration(item)) => {
                        Some(Event {
                            id,
                            msg: EventMsg::ImageGenerationEnd(ImageGenerationEndEvent {
                                call_id: item.id,
                                status: item.status,
                                revised_prompt: item.revised_prompt,
                                result: item.result,
                                transparent_background: item.transparent_background,
                                failure: item.failure,
                                saved_path: item.saved_path,
                            }),
                        })
                    }
                    (Some(id), codex_app_server_protocol::ThreadItem::ContextCompaction { .. }) => {
                        Some(Event {
                            id,
                            msg: EventMsg::ContextCompacted(ContextCompactedEvent),
                        })
                    }
                    _ => None,
                })
            }
            ServerNotification::TurnCompleted(payload) => {
                let event = {
                    let mut state = self.state.lock().await;
                    let submission_id = state
                        .active_turn
                        .as_ref()
                        .map(|turn| turn.submission_id.clone())
                        .unwrap_or_else(|| payload.turn.id.clone());
                    let last_agent_message = state
                        .active_turn
                        .as_ref()
                        .and_then(|turn| turn.last_agent_message.clone());
                    clear_active_turn_state(&mut state);

                    Event {
                        id: submission_id,
                        msg: turn_completed_event_msg(&payload.turn, last_agent_message),
                    }
                };
                self.start_next_queued_submissions().await;
                Ok(Some(event))
            }
            ServerNotification::ServerRequestResolved(payload) => {
                let mut state = self.state.lock().await;
                state
                    .pending_exec_requests
                    .retain(|_, request_id| request_id != &payload.request_id);
                state
                    .pending_patch_requests
                    .retain(|_, request_id| request_id != &payload.request_id);
                state
                    .pending_permissions_requests
                    .retain(|_, request_id| request_id != &payload.request_id);
                state
                    .pending_user_input_requests
                    .retain(|_, request_id| request_id != &payload.request_id);
                state
                    .pending_elicitation_requests
                    .retain(|_, request_id| request_id != &payload.request_id);
                Ok(None)
            }
            ServerNotification::Error(payload) => {
                Ok(active_submission_id_for_turn(self, &payload.turn_id)
                    .await
                    .map(|id| Event {
                        id,
                        msg: if payload.will_retry {
                            EventMsg::StreamError(StreamErrorEvent {
                                message: payload.error.message,
                                codex_error_info: None,
                                additional_details: payload.error.additional_details,
                            })
                        } else {
                            EventMsg::Error(ErrorEvent {
                                message: payload.error.message,
                                codex_error_info: None,
                            })
                        },
                    }))
            }
            ServerNotification::ModelRerouted(payload) => {
                Ok(active_submission_id_for_turn(self, &payload.turn_id)
                    .await
                    .map(|id| Event {
                        id,
                        msg: EventMsg::ModelReroute(ModelRerouteEvent {
                            from_model: payload.from_model,
                            to_model: payload.to_model,
                            reason: app_server_model_reroute_reason_to_core(payload.reason),
                        }),
                    }))
            }
            ServerNotification::DeprecationNotice(payload) => Ok(Some(Event {
                id: noop_submission_id(),
                msg: EventMsg::DeprecationNotice(DeprecationNoticeEvent {
                    summary: payload.summary,
                    details: payload.details,
                }),
            })),
            ServerNotification::ConfigWarning(payload) => Ok(Some(Event {
                id: noop_submission_id(),
                msg: EventMsg::Warning(WarningEvent {
                    message: format_config_warning_message(payload),
                }),
            })),
            ServerNotification::Warning(payload) => {
                let submission_id = {
                    let state = self.state.lock().await;
                    active_submission_id(&state).unwrap_or_else(noop_submission_id)
                };
                Ok(Some(Event {
                    id: submission_id,
                    msg: EventMsg::Warning(WarningEvent {
                        message: payload.message,
                    }),
                }))
            }
            ServerNotification::ThreadClosed(_) => {
                let mut state = self.state.lock().await;
                let Some(active_turn) = state.active_turn.clone() else {
                    return Ok(None);
                };
                clear_active_turn_state(&mut state);
                Ok(Some(Event {
                    id: active_turn.submission_id,
                    msg: EventMsg::ShutdownComplete,
                }))
            }
            ServerNotification::ThreadStarted(_)
            | ServerNotification::ThreadStatusChanged(_)
            | ServerNotification::ThreadArchived(_)
            | ServerNotification::ThreadUnarchived(_)
            | ServerNotification::ThreadReverted(_)
            | ServerNotification::ThreadQueueChanged(_)
            | ServerNotification::ProjectChanged(_)
            | ServerNotification::ThreadProjectUpdated(_)
            | ServerNotification::StrictReviewRequired(_)
            | ServerNotification::SkillsChanged(_)
            | ServerNotification::HookStarted(_)
            | ServerNotification::HookCompleted(_)
            | ServerNotification::ItemGuardianApprovalReviewStarted(_)
            | ServerNotification::ItemGuardianApprovalReviewCompleted(_)
            | ServerNotification::PlanDelta(_)
            | ServerNotification::ReasoningSummaryPartAdded(_)
            | ServerNotification::FileChangeOutputDelta(_)
            | ServerNotification::McpToolCallProgress(_)
            | ServerNotification::WindowsWorldWritableWarning(_)
            | ServerNotification::WindowsSandboxSetupCompleted(_)
            | ServerNotification::ContextCompacted(_)
            | ServerNotification::McpServerStatusUpdated(_)
            | ServerNotification::AccountUpdated(_)
            | ServerNotification::ThreadRealtimeStarted(_)
            | ServerNotification::ThreadRealtimeItemAdded(_)
            | ServerNotification::ThreadRealtimeTranscriptDelta(_)
            | ServerNotification::ThreadRealtimeTranscriptDone(_)
            | ServerNotification::ThreadRealtimeOutputAudioDelta(_)
            | ServerNotification::ThreadRealtimeError(_)
            | ServerNotification::ThreadRealtimeClosed(_)
            | ServerNotification::CommandExecOutputDelta(_)
            | ServerNotification::ProcessOutputDelta(_)
            | ServerNotification::ProcessExited(_)
            | ServerNotification::McpServerOauthLoginCompleted(_)
            | ServerNotification::AccountRateLimitsUpdated(_)
            | ServerNotification::AppListUpdated(_)
            | ServerNotification::ExternalAgentConfigImportCompleted(_)
            | ServerNotification::ExternalAgentConfigImportProgress(_)
            | ServerNotification::ModelSafetyBufferingUpdated(_)
            | ServerNotification::ThreadGoalUpdated(_)
            | ServerNotification::ThreadGoalCleared(_)
            | ServerNotification::RemoteControlStatusChanged(_)
            | ServerNotification::FsChanged(_)
            | ServerNotification::FuzzyFileSearchSessionUpdated(_)
            | ServerNotification::FuzzyFileSearchSessionCompleted(_)
            | ServerNotification::AccountLoginCompleted(_) => Ok(None),
            ServerNotification::RawResponseItemCompleted(payload) => {
                let mut state = self.state.lock().await;
                record_raw_response_item(&mut state, &payload.turn_id, &payload.item);
                Ok(None)
            }
            ServerNotification::TurnDiffUpdated(payload) => {
                let submission_id = {
                    let mut state = self.state.lock().await;
                    state
                        .pending_turn_diffs
                        .insert(payload.turn_id.clone(), payload.diff.clone());
                    submission_id_for_turn_or_fallback(&state, &payload.turn_id)
                };
                Ok(submission_id.map(|id| Event {
                    id,
                    msg: EventMsg::TurnDiff(codex_protocol::protocol::TurnDiffEvent {
                        unified_diff: payload.diff,
                    }),
                }))
            }
        }
    }
}

#[async_trait::async_trait]
impl CodexThreadImpl for AppServerCodexThread {
    async fn submit(&self, submission_id: String, op: Op) -> Result<String, CodexErr> {
        match op {
            op @ (Op::TurnInput { .. }
            | Op::Review { .. }
            | Op::Compact
            | Op::ThreadRollback { .. }) => self.submit_prompt_like(submission_id, op).await,
            Op::Interrupt => self.interrupt_active_turn().await,
            Op::Shutdown => self.shutdown_thread().await,
            Op::ThreadSettings { thread_settings } => {
                self.override_turn_context(OverrideTurnContextArgs {
                    cwd: thread_settings
                        .environments
                        .map(|environments| environments.legacy_fallback_cwd.to_path_buf()),
                    workspace_roots: None,
                    profile_workspace_roots: thread_settings.profile_workspace_roots,
                    approval_policy: thread_settings.approval_policy,
                    approvals_reviewer: thread_settings.approvals_reviewer,
                    sandbox_policy: thread_settings.sandbox_policy,
                    permission_profile: thread_settings.permission_profile,
                    active_permission_profile: thread_settings.active_permission_profile,
                    windows_sandbox_level: thread_settings.windows_sandbox_level,
                    model: thread_settings.model,
                    effort: thread_settings.effort,
                    summary: thread_settings.summary,
                    collaboration_mode: thread_settings.collaboration_mode,
                    personality: thread_settings.personality,
                    service_tier: thread_settings.service_tier,
                })
                .await
            }
            Op::ExecApproval { id, decision, .. } => self.resolve_exec_request(id, decision).await,
            Op::PatchApproval { id, decision } => self.resolve_patch_request(id, decision).await,
            Op::RequestPermissionsResponse { id, response } => {
                self.resolve_permissions_request(id, response).await
            }
            Op::UserInputAnswer { id, response } => {
                self.resolve_user_input_request(id, response).await
            }
            Op::ResolveElicitation {
                server_name,
                request_id,
                decision,
                content,
                meta,
            } => {
                self.resolve_elicitation(server_name, request_id, decision, content, meta)
                    .await
            }
            unsupported => Err(CodexErr::UnsupportedOperation(format!(
                "app-server thread does not support {}",
                unsupported.kind()
            ))),
        }
    }

    async fn next_event(&self) -> Result<Event, CodexErr> {
        loop {
            if let Some(event) = self.pop_local_event().await {
                return Ok(event);
            }

            let server_event = loop {
                let poll_result = {
                    let mut guard = self.client.lock().await;
                    let Some(client) = guard.as_mut() else {
                        return Err(CodexErr::InternalAgentDied);
                    };
                    tokio::time::timeout(APP_SERVER_EVENT_POLL_INTERVAL, client.next_event()).await
                };

                match poll_result {
                    Ok(Some(event)) => break event,
                    Ok(None) => return Err(CodexErr::InternalAgentDied),
                    Err(_) => {
                        if let Some(event) = self.pop_local_event().await {
                            return Ok(event);
                        }
                    }
                }
            };

            if let Some(event) = self.translate_server_event(server_event).await? {
                return Ok(event);
            }
        }
    }
}

fn cancel_queued_submissions(state: &mut AppServerState) {
    while let Some(queued) = state.queued_submissions.pop_front() {
        state.local_events.push_back(Event {
            id: queued.submission_id,
            msg: EventMsg::TurnAborted(TurnAbortedEvent {
                turn_id: None,
                reason: TurnAbortReason::Interrupted,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            }),
        });
    }
}

fn mark_active_turn_interrupted(
    state: &mut AppServerState,
) -> (String, Option<PendingTurnInterrupt>) {
    cancel_queued_submissions(state);

    let Some(active_turn) = state.active_turn.clone() else {
        return (noop_submission_id(), None);
    };

    let Some(turn_id) = active_turn.turn_id.clone() else {
        state.interrupt_after_turn_starts = true;
        return (active_turn.submission_id, None);
    };

    let pending_interrupt = PendingTurnInterrupt {
        request_id: next_request_id(state),
        thread_id: state.thread_id.clone(),
        turn_id,
    };
    clear_active_turn_state(state);
    (active_turn.submission_id, Some(pending_interrupt))
}

fn clear_active_turn_state(state: &mut AppServerState) {
    state.active_turn = None;
    state.pending_exec_requests.clear();
    state.pending_patch_requests.clear();
    state.pending_patch_changes.clear();
    state.pending_permissions_requests.clear();
    state.pending_user_input_requests.clear();
    state.pending_elicitation_requests.clear();
    state.pending_turn_diffs.clear();
    state.interrupt_after_turn_starts = false;
}

fn resumed_active_turn(thread_status: &ThreadStatus, turns: &[Turn]) -> Option<ActiveTurn> {
    if !matches!(thread_status, ThreadStatus::Active { .. }) {
        return None;
    }

    turns
        .iter()
        .rev()
        .find(|turn| turn.status == TurnStatus::InProgress)
        .map(|turn| ActiveTurn {
            submission_id: turn.id.clone(),
            turn_id: Some(turn.id.clone()),
            steerable: resumed_turn_is_steerable(turn),
            last_agent_message: resumed_last_agent_message(turn),
        })
}

fn resumed_turn_is_steerable(turn: &Turn) -> bool {
    let mut review_depth = 0usize;
    for item in &turn.items {
        match item {
            ThreadItem::EnteredReviewMode { .. } => review_depth += 1,
            ThreadItem::ExitedReviewMode { .. } => {
                review_depth = review_depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    review_depth == 0
}

fn resumed_last_agent_message(turn: &Turn) -> Option<String> {
    turn.items.iter().rev().find_map(|item| match item {
        ThreadItem::AgentMessage { text, .. } => Some(text.clone()),
        _ => None,
    })
}

fn app_server_request_user_input_to_core(
    params: codex_app_server_protocol::ToolRequestUserInputParams,
) -> RequestUserInputEvent {
    RequestUserInputEvent {
        call_id: params.item_id,
        turn_id: params.turn_id,
        questions: params
            .questions
            .into_iter()
            .map(app_server_request_user_input_question_to_core)
            .collect(),
        is_blocking: params.is_blocking,
        auto_resolution_ms: params.auto_resolution_ms,
    }
}

fn app_server_request_user_input_question_to_core(
    question: codex_app_server_protocol::ToolRequestUserInputQuestion,
) -> RequestUserInputQuestion {
    RequestUserInputQuestion {
        id: question.id,
        header: question.header,
        question: question.question,
        is_other: question.is_other,
        is_secret: question.is_secret,
        options: question.options.map(|options| {
            options
                .into_iter()
                .map(
                    |option| codex_protocol::request_user_input::RequestUserInputQuestionOption {
                        label: option.label,
                        description: option.description,
                    },
                )
                .collect()
        }),
    }
}

fn request_user_input_response_to_app_server(
    response: RequestUserInputResponse,
) -> codex_app_server_protocol::ToolRequestUserInputResponse {
    codex_app_server_protocol::ToolRequestUserInputResponse {
        answers: response
            .answers
            .into_iter()
            .map(|(question_id, answer)| {
                (
                    question_id,
                    codex_app_server_protocol::ToolRequestUserInputAnswer {
                        answers: answer.answers,
                    },
                )
            })
            .collect(),
    }
}

fn app_server_entered_review_mode_to_core(review: String) -> EnteredReviewModeEvent {
    EnteredReviewModeEvent {
        target: ReviewTarget::Custom {
            instructions: review.clone(),
        },
        user_facing_hint: Some(review),
        turn_id: None,
        item_id: None,
    }
}

fn app_server_exited_review_mode_to_core(review: String) -> ExitedReviewModeEvent {
    ExitedReviewModeEvent {
        turn_id: None,
        item_id: None,
        review_output: Some(ReviewOutputEvent {
            findings: Vec::new(),
            overall_correctness: String::new(),
            overall_explanation: review,
            overall_confidence_score: 0.0,
        }),
    }
}

fn app_server_model_reroute_reason_to_core(
    reason: codex_app_server_protocol::ModelRerouteReason,
) -> codex_protocol::protocol::ModelRerouteReason {
    match reason {
        codex_app_server_protocol::ModelRerouteReason::HighRiskCyberActivity => {
            codex_protocol::protocol::ModelRerouteReason::HighRiskCyberActivity
        }
    }
}

fn format_config_warning_message(
    payload: codex_app_server_protocol::ConfigWarningNotification,
) -> String {
    let mut message = format!("Config warning: {}", payload.summary);

    if let Some(details) = payload.details
        && !details.trim().is_empty()
    {
        message.push_str(": ");
        message.push_str(details.trim());
    }

    if let Some(path) = payload.path {
        message.push_str(" (");
        message.push_str(&path);
        if let Some(range) = payload.range {
            message.push(':');
            message.push_str(&(range.start.line + 1).to_string());
            message.push(':');
            message.push_str(&(range.start.column + 1).to_string());
        }
        message.push(')');
    }

    message
}

fn active_submission_id(state: &AppServerState) -> Option<String> {
    state
        .active_turn
        .as_ref()
        .map(|turn| turn.submission_id.clone())
}

async fn active_submission_id_for_turn(
    thread: &AppServerCodexThread,
    turn_id: &str,
) -> Option<String> {
    let state = thread.state.lock().await;
    submission_id_for_turn_or_fallback(&state, turn_id)
}

fn next_request_id(state: &mut AppServerState) -> RequestId {
    let request_id = state.next_request_id;
    state.next_request_id += 1;
    RequestId::Integer(request_id)
}

fn noop_submission_id() -> String {
    "app-server".to_string()
}

fn shell_command_vec(command: &str) -> Vec<String> {
    // ACP approvals still model shell commands as argv, while the app-server sends
    // a shell script string. This wrapper only preserves that legacy ACP shape.
    cfg_select! {
        windows => vec![
            "powershell.exe".to_string(),
            "-NoLogo".to_string(),
            "-NoProfile".to_string(),
            "-Command".to_string(),
            command.to_string(),
        ],
        _ => vec!["bash".to_string(), "-lc".to_string(), command.to_string()],
    }
}

fn submission_id_for_turn_or_fallback(state: &AppServerState, turn_id: &str) -> Option<String> {
    state
        .active_turn
        .as_ref()
        .and_then(|turn| {
            turn.turn_id
                .as_deref()
                .filter(|active_turn_id| *active_turn_id == turn_id)
                .map(|_| turn.submission_id.clone())
        })
        .or_else(|| (!turn_id.is_empty()).then(|| turn_id.to_string()))
        .or_else(|| active_submission_id(state))
}

fn active_turn_matches(current: &ActiveTurn, expected: &ActiveTurn) -> bool {
    current.submission_id == expected.submission_id && current.turn_id == expected.turn_id
}

fn reused_submission_id_after_successful_turn_steer(
    state: &AppServerState,
    active_turn: &ActiveTurn,
) -> Option<String> {
    state
        .active_turn
        .as_ref()
        .filter(|current| active_turn_matches(current, active_turn))
        .map(|current| current.submission_id.clone())
}

fn pending_patch_changes_for_request(
    state: &AppServerState,
    item_id: &str,
    turn_id: &str,
) -> HashMap<PathBuf, FileChange> {
    state
        .pending_patch_changes
        .get(item_id)
        .cloned()
        .or_else(|| {
            state
                .pending_turn_diffs
                .get(turn_id)
                .map(|diff| parse_turn_diff_to_core_changes(diff))
        })
        .unwrap_or_default()
}

fn pending_patch_changes_from_turns(
    turns: &[Turn],
) -> HashMap<String, HashMap<PathBuf, FileChange>> {
    turns
        .iter()
        .flat_map(|turn| turn.items.iter())
        .filter_map(|item| match item {
            ThreadItem::FileChange {
                id,
                changes,
                status,
            } if *status == codex_app_server_protocol::PatchApplyStatus::InProgress => {
                Some((id.clone(), app_server_changes_to_core(changes.clone())))
            }
            _ => None,
        })
        .collect()
}

fn record_raw_response_item(state: &mut AppServerState, _turn_id: &str, item: &ResponseItem) {
    match item {
        ResponseItem::CustomToolCall { call_id, .. } => {
            state.pending_custom_tool_calls.insert(call_id.clone());
        }
        ResponseItem::CustomToolCallOutput { call_id, .. } => {
            state.pending_custom_tool_calls.remove(call_id);
        }
        _ => {}
    }
}

fn prepare_submission_start(
    state: &mut AppServerState,
    submission_id: &str,
    op: &Op,
) -> Result<Option<PreparedSubmissionStart>, PrepareSubmissionStartError> {
    match op {
        Op::TurnInput { request, .. } => {
            let ProtocolTurnInput::UserInput { content: items, .. } = &request.input else {
                return Ok(None);
            };
            if let Some(missing) = MissingCustomToolOutputs::from_state(state) {
                return Err(PrepareSubmissionStartError::MissingCustomToolOutputs(
                    missing,
                ));
            }
            state.active_turn = Some(ActiveTurn {
                submission_id: submission_id.to_string(),
                turn_id: None,
                steerable: true,
                last_agent_message: None,
            });
            Ok(Some(PreparedSubmissionStart::TurnStart {
                request_id: next_request_id(state),
                params: Box::new(TurnStartParams {
                    thread_id: state.thread_id.clone(),
                    client_user_message_id: None,
                    input: items.clone().into_iter().map(Into::into).collect(),
                    additional_context: None,
                    cwd: Some(state.config.cwd.to_path_buf()),
                    runtime_workspace_roots: None,
                    approval_policy: Some(state.config.permissions.approval_policy.value().into()),
                    approvals_reviewer: Some(state.config.approvals_reviewer.into()),
                    sandbox_policy: Some(
                        state
                            .config
                            .permissions
                            .legacy_sandbox_policy(state.config.cwd.as_path())
                            .into(),
                    ),
                    model: state.config.model.clone(),
                    service_tier: Some(state.config.service_tier.clone()),
                    effort: state.config.model_reasoning_effort.clone(),
                    summary: state.config.model_reasoning_summary,
                    personality: state.config.personality,
                    output_schema: request.start.final_output_json_schema.clone(),
                    responsesapi_client_metadata: request.responsesapi_client_metadata.clone(),
                    collaboration_mode: None,
                    environments: None,
                    permissions: None,
                    multi_agent_mode: None,
                }),
            }))
        }
        Op::Review { review_request } => {
            if let Some(missing) = MissingCustomToolOutputs::from_state(state) {
                return Err(PrepareSubmissionStartError::MissingCustomToolOutputs(
                    missing,
                ));
            }
            state.active_turn = Some(ActiveTurn {
                submission_id: submission_id.to_string(),
                turn_id: None,
                steerable: false,
                last_agent_message: None,
            });
            Ok(Some(PreparedSubmissionStart::ReviewStart {
                request_id: next_request_id(state),
                params: Box::new(ReviewStartParams {
                    thread_id: state.thread_id.clone(),
                    target: review_target_to_app_server(review_request.target.clone()),
                    delivery: Some(ReviewDelivery::Inline),
                }),
            }))
        }
        Op::Compact => {
            if let Some(missing) = MissingCustomToolOutputs::from_state(state) {
                return Err(PrepareSubmissionStartError::MissingCustomToolOutputs(
                    missing,
                ));
            }
            state.active_turn = Some(ActiveTurn {
                submission_id: submission_id.to_string(),
                turn_id: None,
                steerable: false,
                last_agent_message: None,
            });
            Ok(Some(PreparedSubmissionStart::CompactStart {
                request_id: next_request_id(state),
                params: Box::new(ThreadCompactStartParams {
                    thread_id: state.thread_id.clone(),
                }),
            }))
        }
        Op::ThreadRollback { num_turns } => {
            state.pending_custom_tool_calls.clear();
            Ok(Some(PreparedSubmissionStart::Rollback {
                request_id: next_request_id(state),
                params: Box::new(ThreadRollbackParams {
                    thread_id: state.thread_id.clone(),
                    num_turns: *num_turns,
                }),
            }))
        }
        _ => Ok(None),
    }
}

fn set_active_turn_id(
    state: &mut AppServerState,
    submission_id: &str,
    turn_id: Option<String>,
    steerable: bool,
) {
    if let Some(active_turn) = state
        .active_turn
        .as_mut()
        .filter(|turn| turn.submission_id == submission_id)
    {
        active_turn.turn_id = turn_id;
        active_turn.steerable = steerable;
    }
}

fn clear_submission_if_active(state: &mut AppServerState, submission_id: &str) {
    if state
        .active_turn
        .as_ref()
        .is_some_and(|turn| turn.submission_id == submission_id)
    {
        clear_active_turn_state(state);
    }
}

fn app_server_item_started_to_core(
    thread_id: String,
    turn_id: String,
    started_at_ms: i64,
    item: ThreadItem,
) -> Option<EventMsg> {
    Some(EventMsg::ItemStarted(ItemStartedEvent {
        thread_id: ThreadId::from_string(&thread_id).ok()?,
        turn_id,
        item: app_server_collab_item_to_core(item)?,
        started_at_ms,
    }))
}

fn app_server_item_completed_to_core(
    thread_id: String,
    turn_id: String,
    completed_at_ms: i64,
    item: ThreadItem,
) -> Option<EventMsg> {
    Some(EventMsg::ItemCompleted(ItemCompletedEvent {
        thread_id: ThreadId::from_string(&thread_id).ok()?,
        turn_id,
        item: app_server_collab_item_to_core(item)?,
        started_at_ms: None,
        completed_at_ms,
    }))
}

fn app_server_collab_item_to_core(item: ThreadItem) -> Option<CoreTurnItem> {
    match item {
        ThreadItem::CollabAgentToolCall {
            id,
            tool,
            status,
            sender_thread_id,
            receiver_thread_ids,
            prompt,
            model,
            reasoning_effort,
            agents_states,
        } => {
            let receiver_thread_ids = receiver_thread_ids
                .into_iter()
                .map(|id| ThreadId::from_string(&id).ok())
                .collect::<Option<Vec<_>>>()?;
            let agents_states = agents_states
                .into_iter()
                .map(|(id, state)| {
                    Some((
                        ThreadId::from_string(&id).ok()?,
                        app_server_collab_agent_state_to_core(state),
                    ))
                })
                .collect::<Option<HashMap<_, _>>>()?;
            Some(CoreTurnItem::CollabAgentToolCall(
                CoreCollabAgentToolCallItem {
                    id,
                    tool: match tool {
                        codex_app_server_protocol::CollabAgentTool::SpawnAgent => {
                            CoreCollabAgentTool::SpawnAgent
                        }
                        codex_app_server_protocol::CollabAgentTool::SendInput => {
                            CoreCollabAgentTool::SendInput
                        }
                        codex_app_server_protocol::CollabAgentTool::ResumeAgent => {
                            CoreCollabAgentTool::ResumeAgent
                        }
                        codex_app_server_protocol::CollabAgentTool::Wait => {
                            CoreCollabAgentTool::Wait
                        }
                        codex_app_server_protocol::CollabAgentTool::CloseAgent => {
                            CoreCollabAgentTool::CloseAgent
                        }
                        codex_app_server_protocol::CollabAgentTool::SendMessage => {
                            CoreCollabAgentTool::SendMessage
                        }
                        codex_app_server_protocol::CollabAgentTool::FollowupTask => {
                            CoreCollabAgentTool::FollowupTask
                        }
                        codex_app_server_protocol::CollabAgentTool::InterruptAgent => {
                            CoreCollabAgentTool::InterruptAgent
                        }
                        codex_app_server_protocol::CollabAgentTool::ListAgents => {
                            CoreCollabAgentTool::ListAgents
                        }
                    },
                    status: match status {
                        codex_app_server_protocol::CollabAgentToolCallStatus::InProgress => {
                            CoreCollabAgentToolCallStatus::InProgress
                        }
                        codex_app_server_protocol::CollabAgentToolCallStatus::Completed => {
                            CoreCollabAgentToolCallStatus::Completed
                        }
                        codex_app_server_protocol::CollabAgentToolCallStatus::Failed => {
                            CoreCollabAgentToolCallStatus::Failed
                        }
                        codex_app_server_protocol::CollabAgentToolCallStatus::Interrupted => {
                            CoreCollabAgentToolCallStatus::Interrupted
                        }
                    },
                    sender_thread_id: ThreadId::from_string(&sender_thread_id).ok()?,
                    receiver_thread_ids,
                    receiver_agents: Vec::new(),
                    prompt,
                    model,
                    reasoning_effort,
                    agents_states,
                },
            ))
        }
        ThreadItem::SubAgentActivity {
            id,
            kind,
            agent_thread_id,
            agent_path,
        } => Some(CoreTurnItem::SubAgentActivity(CoreSubAgentActivityItem {
            id,
            kind: match kind {
                codex_app_server_protocol::SubAgentActivityKind::Started => {
                    CoreSubAgentActivityKind::Started
                }
                codex_app_server_protocol::SubAgentActivityKind::Interacted => {
                    CoreSubAgentActivityKind::Interacted
                }
                codex_app_server_protocol::SubAgentActivityKind::Interrupted => {
                    CoreSubAgentActivityKind::Interrupted
                }
                codex_app_server_protocol::SubAgentActivityKind::Completed => {
                    CoreSubAgentActivityKind::Completed
                }
            },
            agent_thread_id: ThreadId::from_string(&agent_thread_id).ok()?,
            agent_path: AgentPath::try_from(agent_path).ok()?,
        })),
        _ => None,
    }
}

fn app_server_collab_agent_state_to_core(
    state: codex_app_server_protocol::CollabAgentState,
) -> AgentStatus {
    match state.status {
        codex_app_server_protocol::CollabAgentStatus::PendingInit => AgentStatus::PendingInit,
        codex_app_server_protocol::CollabAgentStatus::Running => AgentStatus::Running,
        codex_app_server_protocol::CollabAgentStatus::Interrupted => AgentStatus::Interrupted,
        codex_app_server_protocol::CollabAgentStatus::Completed => {
            AgentStatus::Completed(state.message)
        }
        codex_app_server_protocol::CollabAgentStatus::Errored => {
            AgentStatus::Errored(state.message.unwrap_or_default())
        }
        codex_app_server_protocol::CollabAgentStatus::Shutdown => AgentStatus::Shutdown,
        codex_app_server_protocol::CollabAgentStatus::NotFound => AgentStatus::NotFound,
    }
}

fn app_server_changes_to_core(changes: Vec<FileUpdateChange>) -> HashMap<PathBuf, FileChange> {
    changes
        .into_iter()
        .map(|change| {
            let path = PathBuf::from(change.path);
            let change_kind = match change.kind {
                codex_app_server_protocol::PatchChangeKind::Add => FileChange::Add {
                    content: change.diff,
                },
                codex_app_server_protocol::PatchChangeKind::Delete => FileChange::Delete {
                    content: change.diff,
                },
                codex_app_server_protocol::PatchChangeKind::Update { move_path } => {
                    FileChange::Update {
                        unified_diff: change.diff,
                        move_path,
                    }
                }
            };
            (path, change_kind)
        })
        .collect()
}

fn parse_turn_diff_to_core_changes(diff: &str) -> HashMap<PathBuf, FileChange> {
    split_turn_diff_blocks(diff)
        .into_iter()
        .filter_map(|block| turn_diff_block_to_core_change(&block))
        .collect()
}

fn split_turn_diff_blocks(diff: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = String::new();

    for line in diff.lines() {
        if line.starts_with("diff --git ") && !current.is_empty() {
            if !current.ends_with('\n') {
                current.push('\n');
            }
            blocks.push(current);
            current = String::new();
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }

    if !current.is_empty() {
        if !current.ends_with('\n') {
            current.push('\n');
        }
        blocks.push(current);
    }

    blocks
}

fn turn_diff_block_to_core_change(block: &str) -> Option<(PathBuf, FileChange)> {
    let header = block.lines().next()?.trim();
    let (old_raw, new_raw) = parse_diff_git_paths(header.strip_prefix("diff --git ")?)?;
    let old_path = normalize_diff_path(&old_raw, "a/");
    let new_path = normalize_diff_path(&new_raw, "b/");
    let key_path = old_path.clone().or(new_path.clone()).map(PathBuf::from)?;
    let move_path = match (old_path, new_path.as_ref()) {
        (Some(old_path), Some(new_path)) if old_path != *new_path => Some(PathBuf::from(new_path)),
        _ => None,
    };
    Some((
        key_path,
        FileChange::Update {
            unified_diff: block.to_string(),
            move_path,
        },
    ))
}

fn parse_diff_git_paths(line: &str) -> Option<(String, String)> {
    let mut chars = line.chars().peekable();
    let first = read_diff_git_token(&mut chars)?;
    let second = read_diff_git_token(&mut chars)?;
    Some((first, second))
}

fn read_diff_git_token(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
    while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
        chars.next();
    }
    let quote = match chars.peek().copied() {
        Some('"') | Some('\'') => chars.next(),
        _ => None,
    };
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            if c == q {
                break;
            }
            if c == '\\' {
                out.push('\\');
                if let Some(next) = chars.next() {
                    out.push(next);
                }
                continue;
            }
        } else if c.is_whitespace() {
            break;
        }
        out.push(c);
    }
    if out.is_empty() && quote.is_none() {
        None
    } else {
        Some(match quote {
            Some(_) => unescape_c_string(&out),
            None => out,
        })
    }
}

fn normalize_diff_path(raw: &str, prefix: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "/dev/null" || trimmed == format!("{prefix}dev/null") {
        return None;
    }
    let trimmed = trimmed.strip_prefix(prefix).unwrap_or(trimmed);
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

fn unescape_c_string(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(next) = chars.next() else {
            out.push('\\');
            break;
        };
        match next {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'b' => out.push('\u{0008}'),
            'f' => out.push('\u{000C}'),
            'a' => out.push('\u{0007}'),
            'v' => out.push('\u{000B}'),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            '\'' => out.push('\''),
            '0'..='7' => {
                let mut value = next.to_digit(8).unwrap_or(0);
                for _ in 0..2 {
                    match chars.peek() {
                        Some('0'..='7') => {
                            if let Some(digit) = chars.next() {
                                value = value * 8 + digit.to_digit(8).unwrap_or(0);
                            } else {
                                break;
                            }
                        }
                        _ => break,
                    }
                }
                if let Some(ch) = std::char::from_u32(value) {
                    out.push(ch);
                }
            }
            other => out.push(other),
        }
    }
    out
}

fn review_target_to_app_server(target: ReviewTarget) -> AppReviewTarget {
    match target {
        ReviewTarget::UncommittedChanges => AppReviewTarget::UncommittedChanges,
        ReviewTarget::BaseBranch { branch } => AppReviewTarget::BaseBranch { branch },
        ReviewTarget::Commit { sha, title } => AppReviewTarget::Commit { sha, title },
        ReviewTarget::Custom { instructions } => AppReviewTarget::Custom { instructions },
    }
}

fn classify_turn_steer_failure(err: &TypedRequestError) -> TurnSteerFailure {
    let TypedRequestError::Server { source, .. } = err else {
        return TurnSteerFailure::Other;
    };

    if source.message == "no active turn to steer"
        || source.message.starts_with("expected active turn id `")
    {
        return TurnSteerFailure::StaleActiveTurn;
    }

    let Some(data) = source.data.clone() else {
        return TurnSteerFailure::Other;
    };
    let Ok(turn_error) = serde_json::from_value::<AppServerTurnError>(data) else {
        return TurnSteerFailure::Other;
    };

    if matches!(
        turn_error.codex_error_info,
        Some(AppServerCodexErrorInfo::ActiveTurnNotSteerable { .. })
    ) {
        return TurnSteerFailure::ActiveTurnNotSteerable;
    }

    TurnSteerFailure::Other
}

fn typed_request_error_to_codex(err: TypedRequestError) -> CodexErr {
    CodexErr::Fatal(err.to_string())
}

fn app_server_internal_error(err: TypedRequestError) -> Error {
    Error::internal_error().data(err.to_string())
}

fn review_decision_to_app_server(decision: ReviewDecision) -> CommandExecutionApprovalDecision {
    match decision {
        ReviewDecision::Approved => CommandExecutionApprovalDecision::Accept,
        ReviewDecision::ApprovedForSession => CommandExecutionApprovalDecision::AcceptForSession,
        ReviewDecision::ApprovedMcpPolicyAmendment => CommandExecutionApprovalDecision::Decline,
        ReviewDecision::ApprovedExecpolicyAmendment {
            proposed_execpolicy_amendment,
        } => CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment {
            execpolicy_amendment: proposed_execpolicy_amendment.into(),
        },
        ReviewDecision::NetworkPolicyAmendment {
            network_policy_amendment,
        } => CommandExecutionApprovalDecision::ApplyNetworkPolicyAmendment {
            network_policy_amendment: network_policy_amendment.into(),
        },
        ReviewDecision::Denied { .. } => CommandExecutionApprovalDecision::Decline,
        ReviewDecision::TimedOut => CommandExecutionApprovalDecision::Decline,
        ReviewDecision::Abort => CommandExecutionApprovalDecision::Cancel,
    }
}

fn patch_review_decision_to_app_server(decision: ReviewDecision) -> FileChangeApprovalDecision {
    match decision {
        ReviewDecision::Approved => FileChangeApprovalDecision::Accept,
        ReviewDecision::ApprovedForSession => FileChangeApprovalDecision::AcceptForSession,
        ReviewDecision::Denied { .. } => FileChangeApprovalDecision::Decline,
        ReviewDecision::TimedOut => FileChangeApprovalDecision::Decline,
        ReviewDecision::Abort => FileChangeApprovalDecision::Cancel,
        ReviewDecision::ApprovedMcpPolicyAmendment => FileChangeApprovalDecision::Decline,
        ReviewDecision::ApprovedExecpolicyAmendment { .. }
        | ReviewDecision::NetworkPolicyAmendment { .. } => FileChangeApprovalDecision::Accept,
    }
}

fn app_server_review_decision_to_core(
    decision: CommandExecutionApprovalDecision,
) -> ReviewDecision {
    match decision {
        CommandExecutionApprovalDecision::Accept => ReviewDecision::Approved,
        CommandExecutionApprovalDecision::AcceptForSession => ReviewDecision::ApprovedForSession,
        CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment {
            execpolicy_amendment,
        } => ReviewDecision::ApprovedExecpolicyAmendment {
            proposed_execpolicy_amendment: execpolicy_amendment.into_core(),
        },
        CommandExecutionApprovalDecision::ApplyNetworkPolicyAmendment {
            network_policy_amendment,
        } => ReviewDecision::NetworkPolicyAmendment {
            network_policy_amendment: network_policy_amendment.into_core(),
        },
        CommandExecutionApprovalDecision::Decline => ReviewDecision::denied("declined"),
        CommandExecutionApprovalDecision::Cancel => ReviewDecision::Abort,
    }
}

fn app_server_command_source_to_core(
    source: codex_app_server_protocol::CommandExecutionSource,
) -> ExecCommandSource {
    match source {
        codex_app_server_protocol::CommandExecutionSource::Agent => ExecCommandSource::Agent,
        codex_app_server_protocol::CommandExecutionSource::UserShell => {
            ExecCommandSource::UserShell
        }
        codex_app_server_protocol::CommandExecutionSource::UnifiedExecStartup => {
            ExecCommandSource::UnifiedExecStartup
        }
        codex_app_server_protocol::CommandExecutionSource::UnifiedExecInteraction => {
            ExecCommandSource::UnifiedExecInteraction
        }
    }
}

fn app_server_command_begin_event_from_item(
    turn_id: String,
    started_at_ms: i64,
    item: codex_app_server_protocol::ThreadItem,
    fallback_cwd: &codex_utils_absolute_path::AbsolutePathBuf,
) -> Option<ExecCommandBeginEvent> {
    let codex_app_server_protocol::ThreadItem::CommandExecution {
        id: call_id,
        plugin_id,
        script_path,
        command,
        cwd,
        process_id,
        source,
        ..
    } = item
    else {
        return None;
    };

    let command_vec = shell_command_vec(&command);
    Some(ExecCommandBeginEvent {
        call_id,
        plugin_id,
        script_path,
        process_id,
        turn_id,
        started_at_ms,
        command: command_vec.clone(),
        cwd: app_server_exec_cwd_to_core(cwd, fallback_cwd),
        parsed_cmd: parse_command(&command_vec),
        source: app_server_command_source_to_core(source),
        interaction_input: None,
    })
}

fn app_server_command_end_event_from_item(
    turn_id: String,
    completed_at_ms: i64,
    item: codex_app_server_protocol::ThreadItem,
    fallback_cwd: &codex_utils_absolute_path::AbsolutePathBuf,
) -> Option<ExecCommandEndEvent> {
    let codex_app_server_protocol::ThreadItem::CommandExecution {
        id: call_id,
        plugin_id,
        script_path,
        command,
        cwd,
        process_id,
        source,
        aggregated_output,
        exit_code,
        duration_ms,
        status,
        ..
    } = item
    else {
        return None;
    };

    let command_vec = shell_command_vec(&command);
    Some(ExecCommandEndEvent {
        call_id,
        plugin_id,
        script_path,
        process_id,
        turn_id,
        completed_at_ms,
        command: command_vec.clone(),
        cwd: app_server_exec_cwd_to_core(cwd, fallback_cwd),
        parsed_cmd: parse_command(&command_vec),
        source: app_server_command_source_to_core(source),
        interaction_input: None,
        stdout: String::new(),
        stderr: String::new(),
        aggregated_output: aggregated_output.unwrap_or_default(),
        exit_code: exit_code.unwrap_or_default(),
        duration: duration_ms
            .and_then(|ms| u64::try_from(ms).ok())
            .map(Duration::from_millis)
            .unwrap_or_default(),
        formatted_output: String::new(),
        status: app_server_command_status_to_core(status),
    })
}

fn app_server_exec_cwd_to_core(
    cwd: codex_utils_path_uri::LegacyAppPathString,
    fallback_cwd: &codex_utils_absolute_path::AbsolutePathBuf,
) -> codex_utils_path_uri::PathUri {
    cwd.try_into().unwrap_or_else(|err| {
        tracing::warn!(
            ?err,
            "legacy exec cwd is not a parseable path; falling back to configured cwd"
        );
        fallback_cwd.clone().into()
    })
}

fn app_server_codex_error_info_to_core(error: AppServerCodexErrorInfo) -> CodexErrorInfo {
    match error {
        AppServerCodexErrorInfo::ContextWindowExceeded => CodexErrorInfo::ContextWindowExceeded,
        AppServerCodexErrorInfo::SessionBudgetExceeded => CodexErrorInfo::SessionBudgetExceeded,
        AppServerCodexErrorInfo::UsageLimitExceeded => CodexErrorInfo::UsageLimitExceeded,
        AppServerCodexErrorInfo::ServerOverloaded => CodexErrorInfo::ServerOverloaded,
        AppServerCodexErrorInfo::CyberPolicy => CodexErrorInfo::CyberPolicy,
        AppServerCodexErrorInfo::MisalignmentPolicyViolation => {
            CodexErrorInfo::MisalignmentPolicyViolation
        }
        AppServerCodexErrorInfo::HttpConnectionFailed { http_status_code } => {
            CodexErrorInfo::HttpConnectionFailed { http_status_code }
        }
        AppServerCodexErrorInfo::ResponseStreamConnectionFailed { http_status_code } => {
            CodexErrorInfo::ResponseStreamConnectionFailed { http_status_code }
        }
        AppServerCodexErrorInfo::InternalServerError => CodexErrorInfo::InternalServerError,
        AppServerCodexErrorInfo::Unauthorized => CodexErrorInfo::Unauthorized,
        AppServerCodexErrorInfo::BadRequest => CodexErrorInfo::BadRequest,
        AppServerCodexErrorInfo::ThreadRollbackFailed => CodexErrorInfo::ThreadRollbackFailed,
        AppServerCodexErrorInfo::SandboxError => CodexErrorInfo::SandboxError,
        AppServerCodexErrorInfo::ResponseStreamDisconnected { http_status_code } => {
            CodexErrorInfo::ResponseStreamDisconnected { http_status_code }
        }
        AppServerCodexErrorInfo::ResponseTooManyFailedAttempts { http_status_code } => {
            CodexErrorInfo::ResponseTooManyFailedAttempts { http_status_code }
        }
        AppServerCodexErrorInfo::ActiveTurnNotSteerable { turn_kind } => {
            CodexErrorInfo::ActiveTurnNotSteerable {
                turn_kind: match turn_kind {
                    codex_app_server_protocol::NonSteerableTurnKind::Review => {
                        NonSteerableTurnKind::Review
                    }
                    codex_app_server_protocol::NonSteerableTurnKind::Compact => {
                        NonSteerableTurnKind::Compact
                    }
                },
            }
        }
        AppServerCodexErrorInfo::Other => CodexErrorInfo::Other,
    }
}

fn app_server_command_status_to_core(
    status: codex_app_server_protocol::CommandExecutionStatus,
) -> ExecCommandStatus {
    match status {
        codex_app_server_protocol::CommandExecutionStatus::Completed => {
            ExecCommandStatus::Completed
        }
        codex_app_server_protocol::CommandExecutionStatus::Failed => ExecCommandStatus::Failed,
        codex_app_server_protocol::CommandExecutionStatus::Declined => ExecCommandStatus::Declined,
        codex_app_server_protocol::CommandExecutionStatus::InProgress => {
            warn!("received unexpected in-progress command completion status from app server");
            ExecCommandStatus::Failed
        }
    }
}

fn app_server_patch_status_to_core(
    status: codex_app_server_protocol::PatchApplyStatus,
) -> PatchApplyStatus {
    match status {
        codex_app_server_protocol::PatchApplyStatus::Completed => PatchApplyStatus::Completed,
        codex_app_server_protocol::PatchApplyStatus::Failed => PatchApplyStatus::Failed,
        codex_app_server_protocol::PatchApplyStatus::Declined => PatchApplyStatus::Declined,
        codex_app_server_protocol::PatchApplyStatus::InProgress => {
            warn!("received unexpected in-progress patch completion status from app server");
            PatchApplyStatus::Failed
        }
    }
}

fn turn_completed_event_msg(turn: &Turn, last_agent_message: Option<String>) -> EventMsg {
    match turn.status {
        TurnStatus::Completed => EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: turn.id.clone(),
            last_agent_message,
            error: turn.error.as_ref().map(|error| ErrorEvent {
                message: error.message.clone(),
                codex_error_info: error
                    .codex_error_info
                    .clone()
                    .map(app_server_codex_error_info_to_core),
            }),
            started_at: turn.started_at,
            completed_at: turn.completed_at,
            duration_ms: turn.duration_ms,
            time_to_first_token_ms: None,
        }),
        TurnStatus::Interrupted => EventMsg::TurnAborted(TurnAbortedEvent {
            turn_id: Some(turn.id.clone()),
            reason: TurnAbortReason::Interrupted,
            started_at: turn.started_at,
            completed_at: turn.completed_at,
            duration_ms: turn.duration_ms,
        }),
        TurnStatus::Failed => EventMsg::Error(ErrorEvent {
            message: turn
                .error
                .as_ref()
                .map(|error| error.message.clone())
                .unwrap_or_else(|| "turn failed".to_string()),
            codex_error_info: None,
        }),
        // A completed notification that still claims in-progress status would otherwise
        // leave ACP-side prompt state hanging, so treat it as a terminal protocol error.
        TurnStatus::InProgress => EventMsg::Error(ErrorEvent {
            message: format!(
                "received turn/completed while turn {} is still marked in_progress",
                turn.id
            ),
            codex_error_info: None,
        }),
    }
}

fn app_server_network_approval_context_to_core(
    context: codex_app_server_protocol::NetworkApprovalContext,
) -> codex_protocol::approvals::NetworkApprovalContext {
    codex_protocol::approvals::NetworkApprovalContext {
        host: context.host,
        protocol: context.protocol.to_core(),
    }
}

fn app_server_memory_citation_to_core(
    citation: codex_app_server_protocol::MemoryCitation,
) -> codex_protocol::memory_citation::MemoryCitation {
    codex_protocol::memory_citation::MemoryCitation {
        entries: citation
            .entries
            .into_iter()
            .map(
                |entry| codex_protocol::memory_citation::MemoryCitationEntry {
                    path: entry.path,
                    line_start: entry.line_start,
                    line_end: entry.line_end,
                    note: entry.note,
                },
            )
            .collect(),
        rollout_ids: citation.thread_ids,
    }
}

fn app_server_web_search_action_to_core(
    action: codex_app_server_protocol::WebSearchAction,
) -> codex_protocol::models::WebSearchAction {
    match action {
        codex_app_server_protocol::WebSearchAction::Search { query, queries } => {
            codex_protocol::models::WebSearchAction::Search { query, queries }
        }
        codex_app_server_protocol::WebSearchAction::OpenPage { url } => {
            codex_protocol::models::WebSearchAction::OpenPage { url }
        }
        codex_app_server_protocol::WebSearchAction::FindInPage { url, pattern } => {
            codex_protocol::models::WebSearchAction::FindInPage { url, pattern }
        }
        codex_app_server_protocol::WebSearchAction::Other => {
            codex_protocol::models::WebSearchAction::Other
        }
    }
}

async fn start_client(config: &Config) -> Result<InProcessAppServerClient, Error> {
    InProcessAppServerClient::start(InProcessClientStartArgs {
        arg0_paths: Arg0DispatchPaths::default(),
        config: Arc::new(config.clone()),
        cli_overrides: Vec::new(),
        loader_overrides: LoaderOverrides::default(),
        strict_config: false,
        cloud_config_bundle: CloudConfigBundleLoader::default(),
        feedback: CodexFeedback::new(),
        log_db: None,
        state_db: None,
        environment_manager: build_environment_manager(config).await?,
        config_warnings: Vec::new(),
        session_source: codex_protocol::protocol::SessionSource::Unknown,
        enable_codex_api_key_env: false,
        client_name: ACP_CLIENT_NAME.to_string(),
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        experimental_api: true,
        // We do not handle `openai/form` elicitation requests in the ACP
        // adapter, so do not advertise support for them.
        mcp_server_openai_form_elicitation: false,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    })
    .await
    .map_err(|err| Error::internal_error().data(err.to_string()))
}

fn permissions_request_event_from_params(
    params: codex_app_server_protocol::PermissionsRequestApprovalParams,
) -> Result<RequestPermissionsEvent, CodexErr> {
    Ok(RequestPermissionsEvent {
        call_id: params.item_id,
        turn_id: params.turn_id,
        environment_id: params.environment_id,
        started_at_ms: params.started_at_ms,
        reason: params.reason,
        permissions: RequestPermissionProfile::try_from(params.permissions)
            .map_err(|err| CodexErr::Fatal(err.to_string()))?,
        cwd: Some(params.cwd),
    })
}

fn file_change_patch_updated_to_core(
    payload: codex_app_server_protocol::FileChangePatchUpdatedNotification,
) -> codex_protocol::protocol::PatchApplyUpdatedEvent {
    codex_protocol::protocol::PatchApplyUpdatedEvent {
        call_id: payload.item_id,
        changes: payload
            .changes
            .into_iter()
            .map(|change| {
                (
                    PathBuf::from(change.path.clone()),
                    file_update_change_to_core(change),
                )
            })
            .collect(),
    }
}

fn file_update_change_to_core(
    change: codex_app_server_protocol::FileUpdateChange,
) -> codex_protocol::protocol::FileChange {
    match change.kind {
        codex_app_server_protocol::PatchChangeKind::Add => {
            codex_protocol::protocol::FileChange::Add {
                content: change.diff,
            }
        }
        codex_app_server_protocol::PatchChangeKind::Delete => {
            codex_protocol::protocol::FileChange::Delete {
                content: change.diff,
            }
        }
        codex_app_server_protocol::PatchChangeKind::Update { move_path } => {
            codex_protocol::protocol::FileChange::Update {
                unified_diff: change.diff,
                move_path,
            }
        }
    }
}

fn guardian_warning_to_core(
    payload: codex_app_server_protocol::GuardianWarningNotification,
) -> codex_protocol::protocol::WarningEvent {
    codex_protocol::protocol::WarningEvent {
        message: payload.message,
    }
}

fn model_verification_to_core(
    payload: codex_app_server_protocol::ModelVerificationNotification,
) -> codex_protocol::protocol::ModelVerificationEvent {
    codex_protocol::protocol::ModelVerificationEvent {
        verifications: payload
            .verifications
            .into_iter()
            .map(|verification| match verification {
                codex_app_server_protocol::ModelVerification::TrustedAccessForCyber => {
                    codex_protocol::protocol::ModelVerification::TrustedAccessForCyber
                }
            })
            .collect(),
    }
}

fn thread_start_params_from_config(config: &Config) -> ThreadStartParams {
    ThreadStartParams {
        model: config.model.clone(),
        model_provider: Some(config.model_provider_id.clone()),
        cwd: Some(config.cwd.to_string_lossy().to_string()),
        approval_policy: Some(config.permissions.approval_policy.value().into()),
        approvals_reviewer: Some(config.approvals_reviewer.into()),
        sandbox: sandbox_mode_from_policy(
            config
                .permissions
                .legacy_sandbox_policy(config.cwd.as_path()),
        ),
        config: config_request_overrides_from_config(config),
        ephemeral: Some(config.ephemeral),
        ..ThreadStartParams::default()
    }
}

fn thread_resume_params_from_config(config: &Config, session_id: &SessionId) -> ThreadResumeParams {
    ThreadResumeParams {
        thread_id: session_id.0.to_string(),
        model: config.model.clone(),
        model_provider: Some(config.model_provider_id.clone()),
        cwd: Some(config.cwd.to_string_lossy().to_string()),
        approval_policy: Some(config.permissions.approval_policy.value().into()),
        approvals_reviewer: Some(config.approvals_reviewer.into()),
        sandbox: sandbox_mode_from_policy(
            config
                .permissions
                .legacy_sandbox_policy(config.cwd.as_path()),
        ),
        config: config_request_overrides_from_config(config),
        ..ThreadResumeParams::default()
    }
}

fn sandbox_mode_from_policy(
    policy: codex_protocol::protocol::SandboxPolicy,
) -> Option<SandboxMode> {
    match policy {
        codex_protocol::protocol::SandboxPolicy::DangerFullAccess => {
            Some(SandboxMode::DangerFullAccess)
        }
        codex_protocol::protocol::SandboxPolicy::ReadOnly { .. } => Some(SandboxMode::ReadOnly),
        codex_protocol::protocol::SandboxPolicy::WorkspaceWrite { .. } => {
            Some(SandboxMode::WorkspaceWrite)
        }
        codex_protocol::protocol::SandboxPolicy::ExternalSandbox { .. } => None,
    }
}

fn config_request_overrides_from_config(
    config: &Config,
) -> Option<HashMap<String, serde_json::Value>> {
    config
        .config_layer_stack
        .get_active_user_layer()
        .and_then(|layer| match &layer.name {
            codex_config::ConfigLayerSource::User {
                profile: Some(profile),
                ..
            } => Some(profile),
            _ => None,
        })
        .map(|profile| {
            HashMap::from([(
                "profile".to_string(),
                serde_json::Value::String(profile.clone()),
            )])
        })
}

fn elicitation_request_key(server_name: &str, request_id: &McpRequestId) -> String {
    format!(
        "{server_name}:{}",
        serde_json::to_string(request_id).unwrap_or_default()
    )
}

fn server_request_id_to_mcp_request_id(request_id: &RequestId) -> McpRequestId {
    match request_id {
        RequestId::Integer(value) => McpRequestId::Integer(*value),
        RequestId::String(value) => McpRequestId::String(value.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::config::ConfigBuilder;
    use codex_protocol::protocol::ReviewRequest;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use std::fs;
    use uuid::Uuid;

    async fn test_state_with_active_turn(active_turn: Option<ActiveTurn>) -> AppServerState {
        let codex_home =
            std::env::temp_dir().join(format!("agenthub-codex-home-{}", Uuid::new_v4()));
        fs::create_dir_all(&codex_home).expect("create codex home");
        let config = ConfigBuilder::default()
            .codex_home(codex_home.clone())
            .fallback_cwd(Some(codex_home))
            .build()
            .await
            .expect("build config");
        AppServerState {
            config,
            next_request_id: 1,
            thread_id: "thread-1".to_string(),
            active_turn,
            queued_submissions: VecDeque::new(),
            local_events: VecDeque::new(),
            pending_exec_requests: HashMap::new(),
            pending_patch_requests: HashMap::new(),
            pending_patch_changes: HashMap::new(),
            pending_permissions_requests: HashMap::new(),
            pending_user_input_requests: HashMap::new(),
            pending_elicitation_requests: HashMap::new(),
            pending_turn_diffs: HashMap::new(),
            pending_custom_tool_calls: HashSet::new(),
            interrupt_after_turn_starts: false,
        }
    }

    #[test]
    fn resumed_regular_turn_is_steerable() {
        let turn = Turn {
            id: "turn-1".to_string(),
            items: vec![ThreadItem::AgentMessage {
                id: "msg-1".to_string(),
                text: "hello".to_string(),
                phase: None,
                memory_citation: None,
                delivery: None,
            }],
            items_view: codex_app_server_protocol::TurnItemsView::Full,
            status: TurnStatus::InProgress,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        };

        let active_turn = resumed_active_turn(
            &ThreadStatus::Active {
                active_flags: Vec::new(),
            },
            &[turn],
        )
        .expect("active turn");

        assert!(active_turn.steerable);
        assert_eq!(active_turn.turn_id.as_deref(), Some("turn-1"));
        assert_eq!(active_turn.last_agent_message.as_deref(), Some("hello"));
    }

    #[test]
    fn subagent_activity_items_translate_to_core_lifecycle_events() {
        let parent_thread_id = ThreadId::new();
        let child_thread_id = ThreadId::new();
        let event = app_server_item_completed_to_core(
            parent_thread_id.to_string(),
            "turn-1".to_string(),
            42,
            ThreadItem::SubAgentActivity {
                id: "activity-1".to_string(),
                kind: codex_app_server_protocol::SubAgentActivityKind::Started,
                agent_thread_id: child_thread_id.to_string(),
                agent_path: "/root/reviewer".to_string(),
            },
        )
        .expect("subagent activity event");

        let EventMsg::ItemCompleted(ItemCompletedEvent {
            thread_id,
            turn_id,
            item: CoreTurnItem::SubAgentActivity(item),
            completed_at_ms,
            ..
        }) = event
        else {
            panic!("expected completed subagent activity");
        };
        assert_eq!(thread_id, parent_thread_id);
        assert_eq!(turn_id, "turn-1");
        assert_eq!(completed_at_ms, 42);
        assert_eq!(item.agent_thread_id, child_thread_id);
        assert_eq!(item.agent_path.as_str(), "/root/reviewer");
        assert_eq!(item.kind, CoreSubAgentActivityKind::Started);
    }

    #[test]
    fn completed_subagent_activity_translates_to_core_completion() {
        let child_thread_id = ThreadId::new();
        let item = app_server_collab_item_to_core(ThreadItem::SubAgentActivity {
            id: "activity-completed".to_string(),
            kind: codex_app_server_protocol::SubAgentActivityKind::Completed,
            agent_thread_id: child_thread_id.to_string(),
            agent_path: "/root/reviewer".to_string(),
        })
        .expect("completed subagent activity");

        let CoreTurnItem::SubAgentActivity(item) = item else {
            panic!("expected subagent activity");
        };
        assert_eq!(item.agent_thread_id, child_thread_id);
        assert_eq!(item.kind, CoreSubAgentActivityKind::Completed);
    }

    #[test]
    fn control_agent_tools_and_interrupted_status_translate_to_core() {
        let sender_thread_id = ThreadId::new();
        let cases = [
            (
                codex_app_server_protocol::CollabAgentTool::SendMessage,
                CoreCollabAgentTool::SendMessage,
            ),
            (
                codex_app_server_protocol::CollabAgentTool::FollowupTask,
                CoreCollabAgentTool::FollowupTask,
            ),
            (
                codex_app_server_protocol::CollabAgentTool::InterruptAgent,
                CoreCollabAgentTool::InterruptAgent,
            ),
            (
                codex_app_server_protocol::CollabAgentTool::ListAgents,
                CoreCollabAgentTool::ListAgents,
            ),
        ];

        for (tool, expected_tool) in cases {
            let item = app_server_collab_item_to_core(ThreadItem::CollabAgentToolCall {
                id: "control-call".to_string(),
                tool,
                status: codex_app_server_protocol::CollabAgentToolCallStatus::Interrupted,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: Vec::new(),
                prompt: None,
                model: None,
                reasoning_effort: None,
                agents_states: HashMap::new(),
            })
            .expect("control-agent tool call");

            let CoreTurnItem::CollabAgentToolCall(item) = item else {
                panic!("expected collab-agent tool call");
            };
            assert_eq!(item.tool, expected_tool);
            assert_eq!(item.status, CoreCollabAgentToolCallStatus::Interrupted);
        }
    }

    #[test]
    fn resumed_review_turn_is_not_steerable() {
        let turn = Turn {
            id: "turn-review".to_string(),
            items: vec![ThreadItem::EnteredReviewMode {
                id: "review-1".to_string(),
                review: "pending review".to_string(),
            }],
            items_view: codex_app_server_protocol::TurnItemsView::Full,
            status: TurnStatus::InProgress,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        };

        let active_turn = resumed_active_turn(
            &ThreadStatus::Active {
                active_flags: Vec::new(),
            },
            &[turn],
        )
        .expect("active turn");

        assert!(!active_turn.steerable);
    }

    #[test]
    fn classify_turn_steer_failure_detects_stale_turn() {
        let err = TypedRequestError::Server {
            method: "turn/steer".to_string(),
            source: codex_app_server_protocol::JSONRPCErrorError {
                code: -32602,
                data: None,
                message: "no active turn to steer".to_string(),
            },
        };

        assert!(matches!(
            classify_turn_steer_failure(&err),
            TurnSteerFailure::StaleActiveTurn
        ));
    }

    #[test]
    fn classify_turn_steer_failure_detects_nonsteerable_turn() {
        let err = TypedRequestError::Server {
            method: "turn/steer".to_string(),
            source: codex_app_server_protocol::JSONRPCErrorError {
                code: -32602,
                data: Some(
                    serde_json::to_value(AppServerTurnError {
                        message: "cannot steer a review turn".to_string(),
                        codex_error_info: Some(AppServerCodexErrorInfo::ActiveTurnNotSteerable {
                            turn_kind: codex_app_server_protocol::NonSteerableTurnKind::Review,
                        }),
                        additional_details: None,
                    })
                    .expect("turn error json"),
                ),
                message: "cannot steer a review turn".to_string(),
            },
        };

        assert!(matches!(
            classify_turn_steer_failure(&err),
            TurnSteerFailure::ActiveTurnNotSteerable
        ));
    }

    #[test]
    fn entered_review_mode_translation_preserves_hint() {
        let review = app_server_entered_review_mode_to_core("Review requested.".to_string());

        assert_eq!(
            review.user_facing_hint.as_deref(),
            Some("Review requested.")
        );
        assert!(matches!(
            review.target,
            ReviewTarget::Custom { instructions } if instructions == "Review requested."
        ));
    }

    #[test]
    fn exited_review_mode_translation_preserves_rendered_text() {
        let event = app_server_exited_review_mode_to_core("Reviewer explanation".to_string());

        let review_output = event.review_output.expect("review output");
        assert!(review_output.findings.is_empty());
        assert_eq!(review_output.overall_explanation, "Reviewer explanation");
    }

    #[test]
    fn model_reroute_reason_translation_preserves_reason() {
        let reason = app_server_model_reroute_reason_to_core(
            codex_app_server_protocol::ModelRerouteReason::HighRiskCyberActivity,
        );

        assert_eq!(
            reason,
            codex_protocol::protocol::ModelRerouteReason::HighRiskCyberActivity
        );
    }

    #[test]
    fn app_server_codex_error_info_translation_preserves_variants() {
        let variants = [
            (
                AppServerCodexErrorInfo::ContextWindowExceeded,
                CodexErrorInfo::ContextWindowExceeded,
            ),
            (
                AppServerCodexErrorInfo::SessionBudgetExceeded,
                CodexErrorInfo::SessionBudgetExceeded,
            ),
            (
                AppServerCodexErrorInfo::UsageLimitExceeded,
                CodexErrorInfo::UsageLimitExceeded,
            ),
            (
                AppServerCodexErrorInfo::ServerOverloaded,
                CodexErrorInfo::ServerOverloaded,
            ),
            (
                AppServerCodexErrorInfo::CyberPolicy,
                CodexErrorInfo::CyberPolicy,
            ),
            (
                AppServerCodexErrorInfo::HttpConnectionFailed {
                    http_status_code: Some(502),
                },
                CodexErrorInfo::HttpConnectionFailed {
                    http_status_code: Some(502),
                },
            ),
            (
                AppServerCodexErrorInfo::ResponseStreamConnectionFailed {
                    http_status_code: Some(503),
                },
                CodexErrorInfo::ResponseStreamConnectionFailed {
                    http_status_code: Some(503),
                },
            ),
            (
                AppServerCodexErrorInfo::InternalServerError,
                CodexErrorInfo::InternalServerError,
            ),
            (
                AppServerCodexErrorInfo::Unauthorized,
                CodexErrorInfo::Unauthorized,
            ),
            (
                AppServerCodexErrorInfo::BadRequest,
                CodexErrorInfo::BadRequest,
            ),
            (
                AppServerCodexErrorInfo::ThreadRollbackFailed,
                CodexErrorInfo::ThreadRollbackFailed,
            ),
            (
                AppServerCodexErrorInfo::SandboxError,
                CodexErrorInfo::SandboxError,
            ),
            (
                AppServerCodexErrorInfo::ResponseStreamDisconnected {
                    http_status_code: Some(504),
                },
                CodexErrorInfo::ResponseStreamDisconnected {
                    http_status_code: Some(504),
                },
            ),
            (
                AppServerCodexErrorInfo::ResponseTooManyFailedAttempts {
                    http_status_code: Some(429),
                },
                CodexErrorInfo::ResponseTooManyFailedAttempts {
                    http_status_code: Some(429),
                },
            ),
            (AppServerCodexErrorInfo::Other, CodexErrorInfo::Other),
        ];

        for (input, expected) in variants {
            assert_eq!(app_server_codex_error_info_to_core(input), expected);
        }
    }

    #[test]
    fn app_server_codex_error_info_translation_preserves_nonsteerable_kind() {
        let review =
            app_server_codex_error_info_to_core(AppServerCodexErrorInfo::ActiveTurnNotSteerable {
                turn_kind: codex_app_server_protocol::NonSteerableTurnKind::Review,
            });
        assert!(matches!(
            review,
            CodexErrorInfo::ActiveTurnNotSteerable {
                turn_kind: NonSteerableTurnKind::Review
            }
        ));

        let compact =
            app_server_codex_error_info_to_core(AppServerCodexErrorInfo::ActiveTurnNotSteerable {
                turn_kind: codex_app_server_protocol::NonSteerableTurnKind::Compact,
            });
        assert!(matches!(
            compact,
            CodexErrorInfo::ActiveTurnNotSteerable {
                turn_kind: NonSteerableTurnKind::Compact
            }
        ));
    }

    #[test]
    fn turn_completed_event_msg_preserves_error_and_timing() {
        let started_at = 100;
        let completed_at = 101;
        let turn = Turn {
            id: "turn-1".to_string(),
            items: Vec::new(),
            items_view: codex_app_server_protocol::TurnItemsView::Full,
            status: TurnStatus::Completed,
            error: Some(AppServerTurnError {
                message: "context window exceeded".to_string(),
                codex_error_info: Some(AppServerCodexErrorInfo::ContextWindowExceeded),
                additional_details: None,
            }),
            started_at: Some(started_at),
            completed_at: Some(completed_at),
            duration_ms: Some(250),
        };

        let event = turn_completed_event_msg(&turn, Some("done".to_string()));

        let EventMsg::TurnComplete(event) = event else {
            panic!("expected turn complete");
        };
        assert_eq!(event.turn_id, "turn-1");
        assert_eq!(event.last_agent_message.as_deref(), Some("done"));
        assert_eq!(event.started_at, Some(started_at));
        assert_eq!(event.completed_at, Some(completed_at));
        assert_eq!(event.duration_ms, Some(250));
        let error = event.error.expect("completion error");
        assert_eq!(error.message, "context window exceeded");
        assert_eq!(
            error.codex_error_info,
            Some(CodexErrorInfo::ContextWindowExceeded)
        );
    }

    #[test]
    fn turn_completed_event_msg_preserves_interrupted_timing() {
        let started_at = 200;
        let completed_at = 201;
        let turn = Turn {
            id: "turn-2".to_string(),
            items: Vec::new(),
            items_view: codex_app_server_protocol::TurnItemsView::Full,
            status: TurnStatus::Interrupted,
            error: None,
            started_at: Some(started_at),
            completed_at: Some(completed_at),
            duration_ms: Some(500),
        };

        let event = turn_completed_event_msg(&turn, None);

        let EventMsg::TurnAborted(event) = event else {
            panic!("expected turn aborted");
        };
        assert_eq!(event.turn_id.as_deref(), Some("turn-2"));
        assert_eq!(event.reason, TurnAbortReason::Interrupted);
        assert_eq!(event.started_at, Some(started_at));
        assert_eq!(event.completed_at, Some(completed_at));
        assert_eq!(event.duration_ms, Some(500));
    }

    #[test]
    fn review_decision_translation_maps_denied_payloads() {
        assert_eq!(
            review_decision_to_app_server(ReviewDecision::denied("declined")),
            CommandExecutionApprovalDecision::Decline
        );
        assert_eq!(
            patch_review_decision_to_app_server(ReviewDecision::denied("declined")),
            FileChangeApprovalDecision::Decline
        );
        assert!(matches!(
            app_server_review_decision_to_core(CommandExecutionApprovalDecision::Decline),
            ReviewDecision::Denied { .. }
        ));
    }

    #[test]
    fn config_warning_message_includes_details_and_location() {
        let message =
            format_config_warning_message(codex_app_server_protocol::ConfigWarningNotification {
                summary: "Invalid profile".to_string(),
                details: Some("unknown field".to_string()),
                path: Some("/tmp/codex.toml".to_string()),
                range: Some(codex_app_server_protocol::TextRange {
                    start: codex_app_server_protocol::TextPosition { line: 3, column: 7 },
                    end: codex_app_server_protocol::TextPosition {
                        line: 3,
                        column: 12,
                    },
                }),
            });

        assert_eq!(
            message,
            "Config warning: Invalid profile: unknown field (/tmp/codex.toml:4:8)"
        );
    }

    #[test]
    fn turn_completed_in_progress_translates_to_error() {
        let turn = Turn {
            id: "turn-1".to_string(),
            items: Vec::new(),
            items_view: codex_app_server_protocol::TurnItemsView::Full,
            status: TurnStatus::InProgress,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        };

        let event = turn_completed_event_msg(&turn, None);
        assert!(matches!(
            event,
            EventMsg::Error(ErrorEvent { message, codex_error_info: None })
                if message.contains("turn-1") && message.contains("in_progress")
        ));
    }

    #[test]
    fn shell_command_vec_uses_platform_wrapper() {
        let command = "echo hello";
        let wrapped = shell_command_vec(command);

        #[cfg(windows)]
        assert_eq!(
            wrapped,
            vec![
                "powershell.exe".to_string(),
                "-NoLogo".to_string(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                command.to_string(),
            ]
        );

        #[cfg(not(windows))]
        assert_eq!(
            wrapped,
            vec!["bash".to_string(), "-lc".to_string(), command.to_string(),]
        );
    }

    #[test]
    fn request_user_input_translation_preserves_questions() {
        let event = app_server_request_user_input_to_core(
            codex_app_server_protocol::ToolRequestUserInputParams {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "item-1".to_string(),
                is_blocking: true,
                auto_resolution_ms: None,
                questions: vec![codex_app_server_protocol::ToolRequestUserInputQuestion {
                    id: "question-1".to_string(),
                    header: "Clarify".to_string(),
                    question: "Pick one".to_string(),
                    is_other: true,
                    is_secret: false,
                    options: Some(vec![
                        codex_app_server_protocol::ToolRequestUserInputOption {
                            label: "Yes".to_string(),
                            description: "Proceed".to_string(),
                        },
                    ]),
                }],
            },
        );

        assert_eq!(event.call_id, "item-1");
        assert_eq!(event.turn_id, "turn-1");
        assert!(event.is_blocking);
        assert_eq!(event.questions.len(), 1);
        assert_eq!(event.questions[0].header, "Clarify");
        assert!(event.questions[0].is_other);
        assert_eq!(
            event.questions[0].options.as_ref().expect("options")[0].label,
            "Yes"
        );
    }

    #[test]
    fn request_user_input_response_translation_preserves_answers() {
        let response = request_user_input_response_to_app_server(RequestUserInputResponse {
            answers: HashMap::from([(
                "question-1".to_string(),
                codex_protocol::request_user_input::RequestUserInputAnswer {
                    answers: vec!["approved".to_string()],
                },
            )]),
        });

        assert_eq!(
            response
                .answers
                .get("question-1")
                .expect("question answer")
                .answers,
            vec!["approved".to_string()]
        );
    }

    #[test]
    fn permissions_request_event_preserves_cwd() {
        let cwd = AbsolutePathBuf::try_from(std::env::temp_dir()).expect("valid temp dir");
        let event = permissions_request_event_from_params(
            codex_app_server_protocol::PermissionsRequestApprovalParams {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                environment_id: Some("env-1".to_string()),
                item_id: "call-1".to_string(),
                started_at_ms: 0,
                cwd: cwd.clone(),
                reason: Some("need write access".to_string()),
                permissions: codex_app_server_protocol::RequestPermissionProfile {
                    network: Some(codex_app_server_protocol::AdditionalNetworkPermissions {
                        enabled: Some(true),
                    }),
                    file_system: None,
                },
            },
        )
        .expect("valid permissions request event");

        assert_eq!(event.call_id, "call-1");
        assert_eq!(event.turn_id, "turn-1");
        assert_eq!(event.environment_id.as_deref(), Some("env-1"));
        assert_eq!(event.started_at_ms, 0);
        assert_eq!(event.cwd, Some(cwd));
        assert_eq!(event.reason.as_deref(), Some("need write access"));
        assert!(event.permissions.file_system.is_none());
        assert_eq!(
            event.permissions.network,
            Some(codex_protocol::models::NetworkPermissions {
                enabled: Some(true),
            })
        );
    }

    #[test]
    fn file_change_patch_updated_translation_preserves_call_id_and_changes() {
        let event = file_change_patch_updated_to_core(
            codex_app_server_protocol::FileChangePatchUpdatedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "call-1".to_string(),
                changes: vec![codex_app_server_protocol::FileUpdateChange {
                    path: "src/main.rs".to_string(),
                    kind: codex_app_server_protocol::PatchChangeKind::Update { move_path: None },
                    diff: "@@ -1 +1 @@\n-old\n+new\n".to_string(),
                }],
            },
        );

        assert_eq!(event.call_id, "call-1");
        assert_eq!(event.changes.len(), 1);
        assert_eq!(
            event.changes.get(&PathBuf::from("src/main.rs")),
            Some(&codex_protocol::protocol::FileChange::Update {
                unified_diff: "@@ -1 +1 @@\n-old\n+new\n".to_string(),
                move_path: None,
            })
        );
    }

    #[test]
    fn guardian_warning_translation_preserves_message() {
        let event =
            guardian_warning_to_core(codex_app_server_protocol::GuardianWarningNotification {
                thread_id: "thread-1".to_string(),
                message: "unsafe operation blocked".to_string(),
            });

        assert_eq!(event.message, "unsafe operation blocked");
    }

    #[test]
    fn model_verification_translation_preserves_verifications() {
        let event =
            model_verification_to_core(codex_app_server_protocol::ModelVerificationNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                verifications: vec![
                    codex_app_server_protocol::ModelVerification::TrustedAccessForCyber,
                ],
            });

        assert_eq!(
            event.verifications,
            vec![codex_protocol::protocol::ModelVerification::TrustedAccessForCyber]
        );
    }

    #[test]
    fn unsupported_server_request_messages_stay_explicit() {
        assert_eq!(
            DYNAMIC_TOOL_CALLBACK_UNSUPPORTED_MESSAGE,
            "dynamic tool callbacks are not supported by agenthub-codex-acp"
        );
        assert_eq!(
            ATTESTATION_GENERATION_UNSUPPORTED_MESSAGE,
            "attestation generation is not supported by agenthub-codex-acp"
        );
    }

    #[tokio::test]
    async fn attestation_generate_requests_are_rejected() {
        let state = test_state_with_active_turn(None).await;
        let config = state.config.clone();
        let client = start_client(&config).await.expect("start client");
        let request_handle = client.request_handle();
        let thread = AppServerCodexThread::new(
            client,
            request_handle,
            "thread-1".to_string(),
            config,
            1,
            ThreadStatus::Idle,
            Vec::new(),
        );

        let result = thread
            .translate_server_request(ServerRequest::AttestationGenerate {
                request_id: RequestId::Integer(7),
                params: codex_app_server_protocol::AttestationGenerateParams {},
            })
            .await;

        assert!(matches!(result, Ok(None)));
    }

    #[tokio::test]
    async fn submission_id_for_turn_falls_back_to_turn_id_when_local_state_is_missing() {
        let state = test_state_with_active_turn(None).await;

        assert_eq!(
            submission_id_for_turn_or_fallback(&state, "resumed-turn").as_deref(),
            Some("resumed-turn")
        );
    }

    #[tokio::test]
    async fn successful_turn_steer_reuses_existing_submission_when_local_active_turn_matches() {
        let active_turn = ActiveTurn {
            submission_id: "shared-submission".to_string(),
            turn_id: Some("turn-1".to_string()),
            steerable: true,
            last_agent_message: None,
        };
        let state = test_state_with_active_turn(Some(active_turn.clone())).await;

        assert_eq!(
            reused_submission_id_after_successful_turn_steer(&state, &active_turn).as_deref(),
            Some("shared-submission")
        );
    }

    #[tokio::test]
    async fn successful_turn_steer_starts_fresh_turn_when_local_active_turn_changed() {
        let state = test_state_with_active_turn(Some(ActiveTurn {
            submission_id: "new-submission".to_string(),
            turn_id: Some("turn-2".to_string()),
            steerable: true,
            last_agent_message: None,
        }))
        .await;
        let old_active_turn = ActiveTurn {
            submission_id: "shared-submission".to_string(),
            turn_id: Some("turn-1".to_string()),
            steerable: true,
            last_agent_message: None,
        };

        assert_eq!(
            reused_submission_id_after_successful_turn_steer(&state, &old_active_turn),
            None
        );
    }

    #[tokio::test]
    async fn interrupt_marks_active_turn_inactive_before_provider_ack() {
        let mut state = test_state_with_active_turn(Some(ActiveTurn {
            submission_id: "active-submission".to_string(),
            turn_id: Some("turn-1".to_string()),
            steerable: true,
            last_agent_message: None,
        }))
        .await;

        let (submission_id, pending_interrupt) = mark_active_turn_interrupted(&mut state);

        assert_eq!(submission_id, "active-submission");
        let pending_interrupt = pending_interrupt.expect("interrupt request");
        assert_eq!(pending_interrupt.thread_id, "thread-1");
        assert_eq!(pending_interrupt.turn_id, "turn-1");
        assert!(
            state.active_turn.is_none(),
            "local active turn should be released before provider ack"
        );
    }

    #[tokio::test]
    async fn raw_response_item_tracking_clears_custom_tool_call_after_output() {
        let mut state = test_state_with_active_turn(None).await;
        let call = ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "call-1".to_string(),
            name: "apply_patch".to_string(),
            namespace: None,
            input: "*** Begin Patch".to_string(),
            internal_chat_message_metadata_passthrough: None,
        };
        record_raw_response_item(&mut state, "turn-1", &call);

        assert!(state.pending_custom_tool_calls.contains("call-1"));

        let output = ResponseItem::CustomToolCallOutput {
            id: None,
            call_id: "call-1".to_string(),
            name: Some("apply_patch".to_string()),
            output: codex_protocol::models::FunctionCallOutputPayload::from_text("ok".to_string()),
            internal_chat_message_metadata_passthrough: None,
        };
        record_raw_response_item(&mut state, "turn-1", &output);

        assert!(state.pending_custom_tool_calls.is_empty());
    }

    #[tokio::test]
    async fn raw_response_item_tracking_ignores_non_custom_tool_items() {
        let mut state = test_state_with_active_turn(None).await;
        let item = ResponseItem::FunctionCall {
            id: None,
            name: "read_file".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: "function-call-1".to_string(),
            encrypted_function_args: None,
            internal_chat_message_metadata_passthrough: None,
        };

        record_raw_response_item(&mut state, "turn-1", &item);

        assert!(state.pending_custom_tool_calls.is_empty());
    }

    #[tokio::test]
    async fn missing_custom_tool_outputs_are_sorted_and_convert_to_codex_error() {
        let mut state = test_state_with_active_turn(None).await;
        assert_eq!(MissingCustomToolOutputs::from_state(&state), None);

        state.pending_custom_tool_calls.insert("call-z".to_string());
        state.pending_custom_tool_calls.insert("call-a".to_string());

        let missing = MissingCustomToolOutputs::from_state(&state).expect("missing outputs");
        assert_eq!(
            missing,
            MissingCustomToolOutputs {
                call_ids: vec!["call-a".to_string(), "call-z".to_string()],
            }
        );

        let err = PrepareSubmissionStartError::MissingCustomToolOutputs(missing).into_codex_err();
        let message = err.to_string();
        assert!(message.contains("CustomToolCallOutput"));
        assert!(message.contains("call-a, call-z"));
    }

    #[tokio::test]
    async fn prepare_submission_blocks_new_turn_when_custom_tool_output_is_missing() {
        let mut state = test_state_with_active_turn(None).await;
        state
            .pending_custom_tool_calls
            .insert("call-missing".to_string());

        let err = prepare_submission_start(
            &mut state,
            "submission-2",
            &crate::thread::user_input_op(vec![codex_protocol::user_input::UserInput::Text {
                text: "continue".to_string(),
                text_elements: Vec::new(),
            }]),
        )
        .expect_err("dirty custom tool history should block new turns");

        assert_eq!(
            err,
            PrepareSubmissionStartError::MissingCustomToolOutputs(MissingCustomToolOutputs {
                call_ids: vec!["call-missing".to_string()],
            })
        );
        assert!(state.active_turn.is_none());
    }

    #[tokio::test]
    async fn prepare_submission_blocks_review_and_compact_when_custom_tool_output_is_missing() {
        let mut state = test_state_with_active_turn(None).await;
        state
            .pending_custom_tool_calls
            .insert("call-missing".to_string());

        let review_err = prepare_submission_start(
            &mut state,
            "review-submission",
            &Op::Review {
                review_request: ReviewRequest {
                    target: ReviewTarget::UncommittedChanges,
                    user_facing_hint: None,
                },
            },
        )
        .expect_err("dirty custom tool history should block review");
        assert!(matches!(
            review_err,
            PrepareSubmissionStartError::MissingCustomToolOutputs(_)
        ));
        assert!(state.active_turn.is_none());

        let compact_err = prepare_submission_start(&mut state, "compact-submission", &Op::Compact)
            .expect_err("dirty custom tool history should block compaction");
        assert!(matches!(
            compact_err,
            PrepareSubmissionStartError::MissingCustomToolOutputs(_)
        ));
        assert!(state.active_turn.is_none());
    }

    #[tokio::test]
    async fn prepare_submission_allows_undo_when_custom_tool_output_is_missing() {
        let mut state = test_state_with_active_turn(None).await;
        state
            .pending_custom_tool_calls
            .insert("call-missing".to_string());

        let prepared = prepare_submission_start(
            &mut state,
            "undo-submission",
            &Op::ThreadRollback { num_turns: 1 },
        )
        .expect("undo should remain available")
        .expect("prepared rollback");

        assert!(matches!(prepared, PreparedSubmissionStart::Rollback { .. }));
        assert!(state.pending_custom_tool_calls.is_empty());

        let prepared = prepare_submission_start(
            &mut state,
            "submission-after-undo",
            &crate::thread::user_input_op(Vec::new()),
        )
        .expect("undo should clear the local pending tool guard");
        assert!(matches!(
            prepared,
            Some(PreparedSubmissionStart::TurnStart { .. })
        ));
    }

    #[tokio::test]
    async fn prepare_submission_start_leaves_runtime_workspace_roots_unset() {
        let mut state = test_state_with_active_turn(None).await;

        let prepared = prepare_submission_start(
            &mut state,
            "submission-runtime-roots",
            &crate::thread::user_input_op(Vec::new()),
        )
        .expect("prepare submission")
        .expect("turn start");

        let PreparedSubmissionStart::TurnStart { params, .. } = prepared else {
            panic!("expected turn start");
        };
        assert_eq!(params.cwd, Some(state.config.cwd.to_path_buf()));
        assert_eq!(params.runtime_workspace_roots, None);
    }

    #[test]
    fn pending_patch_changes_from_turns_recovers_in_progress_patch_items() {
        let turns = vec![Turn {
            id: "turn-1".to_string(),
            items: vec![ThreadItem::FileChange {
                id: "patch-1".to_string(),
                changes: vec![FileUpdateChange {
                    path: "README.md".to_string(),
                    kind: codex_app_server_protocol::PatchChangeKind::Update { move_path: None },
                    diff: "diff --git a/README.md b/README.md\n".to_string(),
                }],
                status: codex_app_server_protocol::PatchApplyStatus::InProgress,
            }],
            items_view: codex_app_server_protocol::TurnItemsView::Full,
            status: TurnStatus::InProgress,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        }];

        let pending = pending_patch_changes_from_turns(&turns);
        let changes = pending.get("patch-1").expect("pending patch changes");
        assert!(matches!(
            changes.get(&PathBuf::from("README.md")),
            Some(FileChange::Update { unified_diff, move_path: None })
                if unified_diff == "diff --git a/README.md b/README.md\n"
        ));
    }

    fn command_execution_item_with_plugin_fields(
        cwd: &AbsolutePathBuf,
    ) -> codex_app_server_protocol::ThreadItem {
        codex_app_server_protocol::ThreadItem::CommandExecution {
            id: "exec-1".to_string(),
            plugin_id: Some("plugin-1".to_string()),
            script_path: Some("scripts/run.sh".to_string()),
            command: "echo done".to_string(),
            cwd: codex_utils_path_uri::LegacyAppPathString::from_abs_path(cwd),
            process_id: Some("pid-1".to_string()),
            source: codex_app_server_protocol::CommandExecutionSource::Agent,
            status: codex_app_server_protocol::CommandExecutionStatus::Completed,
            command_actions: Vec::new(),
            aggregated_output: Some("done\n".to_string()),
            exit_code: Some(0),
            duration_ms: Some(5),
        }
    }

    #[test]
    fn command_execution_begin_translation_preserves_plugin_fields() {
        let cwd = AbsolutePathBuf::try_from(std::env::temp_dir()).expect("valid temp dir");
        let event = app_server_command_begin_event_from_item(
            "turn-1".to_string(),
            100,
            command_execution_item_with_plugin_fields(&cwd),
            &cwd,
        )
        .expect("command begin event");

        assert_eq!(event.call_id, "exec-1");
        assert_eq!(event.plugin_id.as_deref(), Some("plugin-1"));
        assert_eq!(event.script_path.as_deref(), Some("scripts/run.sh"));
        assert_eq!(event.process_id.as_deref(), Some("pid-1"));
        assert_eq!(event.turn_id, "turn-1");
        assert_eq!(event.started_at_ms, 100);
    }

    #[test]
    fn command_execution_end_translation_preserves_plugin_fields() {
        let cwd = AbsolutePathBuf::try_from(std::env::temp_dir()).expect("valid temp dir");
        let event = app_server_command_end_event_from_item(
            "turn-1".to_string(),
            200,
            command_execution_item_with_plugin_fields(&cwd),
            &cwd,
        )
        .expect("command end event");

        assert_eq!(event.call_id, "exec-1");
        assert_eq!(event.plugin_id.as_deref(), Some("plugin-1"));
        assert_eq!(event.script_path.as_deref(), Some("scripts/run.sh"));
        assert_eq!(event.process_id.as_deref(), Some("pid-1"));
        assert_eq!(event.turn_id, "turn-1");
        assert_eq!(event.completed_at_ms, 200);
        assert_eq!(event.aggregated_output, "done\n");
        assert_eq!(event.exit_code, 0);
        assert_eq!(event.duration, Duration::from_millis(5));
        assert_eq!(event.status, ExecCommandStatus::Completed);
    }

    #[test]
    fn parse_turn_diff_to_core_changes_splits_multi_file_diff_and_tracks_rename() {
        let diff = concat!(
            "diff --git a/src.txt b/dst.txt\n",
            "rename from src.txt\n",
            "rename to dst.txt\n",
            "--- a/src.txt\n",
            "+++ b/dst.txt\n",
            "@@ -1 +1 @@\n",
            "-old\n",
            "+new\n",
            "diff --git a/README.md b/README.md\n",
            "--- a/README.md\n",
            "+++ b/README.md\n",
            "@@ -1 +1 @@\n",
            "-before\n",
            "+after\n",
        );

        let changes = parse_turn_diff_to_core_changes(diff);
        assert_eq!(changes.len(), 2);
        assert!(matches!(
            changes.get(&PathBuf::from("src.txt")),
            Some(FileChange::Update { move_path: Some(path), unified_diff })
                if path == &PathBuf::from("dst.txt")
                    && unified_diff.contains("rename from src.txt")
        ));
        assert!(matches!(
            changes.get(&PathBuf::from("README.md")),
            Some(FileChange::Update { move_path: None, unified_diff })
                if unified_diff.contains("diff --git a/README.md b/README.md")
        ));
    }

    #[test]
    fn command_status_in_progress_maps_to_failed() {
        assert_eq!(
            app_server_command_status_to_core(
                codex_app_server_protocol::CommandExecutionStatus::InProgress,
            ),
            ExecCommandStatus::Failed
        );
    }

    #[test]
    fn patch_status_in_progress_maps_to_failed() {
        assert_eq!(
            app_server_patch_status_to_core(
                codex_app_server_protocol::PatchApplyStatus::InProgress
            ),
            PatchApplyStatus::Failed
        );
    }
}
