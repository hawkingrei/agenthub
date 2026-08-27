use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path as StdPath, PathBuf};

mod errors;

use self::errors::{
    map_actor_service_api_error, map_channel_create_error, map_channel_delete_error,
    map_create_team_error, map_goal_fork_error, map_not_found_error, map_reply_thread_error,
    map_resume_run_error, map_runtime_start_error, map_submit_step_error, map_task_message_error,
    map_team_internal_error,
};
use agenthub_message_archive::{MessageDocumentKind, MessageSearchHit, MessageSearchQuery};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, ActorAckRequest, ActorInboxRequest,
    ActorMailboxService, ActorMessageHandlingDisposition, ActorMessageKind, ActorMessageStatus,
    ActorSendRequest, ActorServiceErrorCode, ActorTriageRequest, canonical_json,
    normalize_actor_message_envelope_payload, parse_actor_transport,
};
use agenthub_team_prompts::{
    DEFAULT_TEAM_COORDINATOR_PROMPT, DEFAULT_TEAM_WORKER_PROMPT, default_team_prompt_for_role,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{delete, get, post, put},
};
use futures::future::try_join_all;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use agenthub_auth_domain::UserCapability;

use crate::agent::{AgentConfig, AgentStatus, WorktreeMode};
use crate::api::authz::require_capability;
use crate::api::error::ApiError;
use crate::api::uploads::{
    DownloadRequest, UploadRequest, download_scoped_object, upload_scoped_object,
};
use crate::auth::UserRecord;
use crate::object_upload::{ObjectUploadKind, ObjectUploadOwnerScope};
use crate::state::AppState;
use crate::team::{
    TEAM_RUN_STATUS_VALUES, TeamActorMessageRecord, TeamActorMessageTransport, TeamChannelRecord,
    TeamConversationMessageRecord, TeamConversationRecord, TeamDefinitionConfig,
    TeamDefinitionRecord, TeamMailboxRuntimeDeliveryWorker, TeamMemoryFlushRequest,
    TeamReplyObligationRecord, TeamRunEventRecord, TeamRunRecord, TeamRunStatus, TeamRuntimeRecord,
    TeamStepRecord, TeamStepStatus, TeamTaskCreateInput, TeamTaskExecutionPlan, TeamTaskNoteRecord,
    TeamTaskPriority, TeamTaskRecord, TeamTaskStepExecutionSpec, TeamThreadReplyRecord,
    effective_team_member_skills, ensure_team_runtime_started, force_team_member_new_session,
    normalize_optional_idempotency_key_input, parse_task_execution_plan,
    plan_actor_mailbox_immediate_hint, stop_team_runtime,
};

const TEAM_SPEC_VERSION_V1: i64 = 1;
const MAX_TEAM_SPEC_STEPS: usize = 2048;
const DEFAULT_TEAM_PLAN_STEP_KEY: &str = "coordinator_plan";
const DEFAULT_TEAM_SYNTH_STEP_KEY: &str = "coordinator_synthesize";
#[cfg(test)]
const TEAM_CONVERSATION_MODE_VALUES: [&str; 3] = ["to_coordinator", "to_member", "group_chat"];
const TEAM_CONVERSATION_ROUTE_VALUES: [&str; 3] = ["to_coordinator", "to_member", "group_chat"];
const TEAM_SPECIAL_USER_ACTOR_ALIAS: &str = "user";
const TEAM_SPECIAL_USER_ACTOR_PREFIX: &str = "user:";
const TEAM_SHARED_THREAD_BOOTSTRAP_KIND: &str = "shared_thread";
const TEAM_TASK_COMPILE_VERSION: i64 = 1;
const TEAM_TASK_COMPILE_MESSAGE_LIMIT: i64 = 500;
const DEFAULT_TEAM_TASK_ACCEPTANCE_CRITERION: &str =
    "All assigned steps complete and coordinator synthesis is delivered.";
const TEAM_TASK_COMPILE_MAX_LIST_ITEMS: usize = 32;
const TEAM_MESSAGE_SUMMARY_MAX_CHARS: usize = 240;
const TEAM_MEMORY_FLUSH_TRIGGER_VALUES: [&str; 3] = ["manual", "soft_threshold", "hard_error"];
const TEAM_TASK_COMPILE_MAX_TEXT_LEN: usize = 280;
const TEAM_TASK_COMPILE_MAX_DEADLINE_LEN: usize = 40;
const ADOPTED_MEMBER_ID_PLACEHOLDER: &str = "__agenthub_adopted_member__";

fn team_owner_matches_user(team: &TeamDefinitionRecord, user: &UserRecord) -> bool {
    match team.owner_user_id.as_deref() {
        Some(owner_user_id) => owner_user_id == user.id,
        // Backward compatibility for legacy rows created before owner_user_id was introduced.
        None => true,
    }
}

pub(crate) async fn load_team_for_user(
    state: &AppState,
    team_id: &str,
    user: &UserRecord,
) -> Result<TeamDefinitionRecord, ApiError> {
    let team = state
        .teams
        .get_team(team_id)
        .await
        .map_err(|err| map_not_found_error(err, "team not found"))?;
    if !team_owner_matches_user(&team, user)
        && !state.teams.is_teamspace_member(team_id, &user.id).await?
    {
        return Err(ApiError::not_found("team not found"));
    }
    Ok(team)
}

async fn require_teamspace_role(
    state: &AppState,
    team: &TeamDefinitionRecord,
    user: &UserRecord,
    allowed_roles: &[&str],
) -> Result<(), ApiError> {
    if team_owner_matches_user(team, user) {
        return Ok(());
    }
    let role = state
        .teams
        .teamspace_role_for_user(&team.id, &user.id)
        .await?
        .ok_or_else(|| ApiError::not_found("team not found"))?;
    if allowed_roles.contains(&role.as_str()) {
        Ok(())
    } else {
        Err(ApiError::forbidden(
            "Teamspace role cannot perform this action",
        ))
    }
}

fn sanitize_team_spec_for_response(spec: &mut Value) {
    let Some(spec_obj) = spec.as_object_mut() else {
        return;
    };
    let Some(members) = spec_obj.get_mut("members").and_then(Value::as_array_mut) else {
        return;
    };
    for member in members {
        let Some(member_obj) = member.as_object_mut() else {
            continue;
        };
        member_obj.remove("skills");
    }
}

fn sanitize_team_definition_for_response(mut team: TeamDefinitionRecord) -> TeamDefinitionRecord {
    sanitize_team_spec_for_response(&mut team.spec);
    team
}

async fn load_run_for_user(
    state: &AppState,
    run_id: &str,
    user: &UserRecord,
) -> Result<TeamRunRecord, ApiError> {
    let run = state
        .teams
        .get_run(run_id)
        .await
        .map_err(|err| map_not_found_error(err, "run not found"))?;
    load_team_for_user(state, &run.team_id, user).await?;
    Ok(run)
}

async fn load_run_and_team_for_user(
    state: &AppState,
    run_id: &str,
    user: &UserRecord,
) -> Result<(TeamRunRecord, TeamDefinitionRecord), ApiError> {
    let run = state
        .teams
        .get_run(run_id)
        .await
        .map_err(|err| map_not_found_error(err, "run not found"))?;
    let team = load_team_for_user(state, &run.team_id, user).await?;
    Ok((run, team))
}

#[derive(Debug, Deserialize)]
pub struct CreateTeamRequest {
    pub name: String,
    pub description: Option<String>,
    pub spec: Value,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTeamSpecRequest {
    pub spec: Value,
    pub expected_updated_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeamspaceInviteRequest {
    pub role: String,
    pub expires_in_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AcceptTeamspaceInviteRequest {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct HandoffTeamTaskRequest {
    pub assigned_member_id: String,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateGoalForkRequest {
    pub question: String,
    pub acceptance_criteria: String,
    pub result_schema: Option<Value>,
    pub expires_in_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CompleteGoalForkRequest {
    pub result: Value,
}

#[derive(Debug, Serialize)]
pub struct CreateTeamspaceInviteResponse {
    pub invite: crate::team::TeamspaceInviteRecord,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct MoveExistingAgentRequest {
    pub agent_id: String,
    pub spec: Value,
    pub expected_updated_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct AdoptExistingAgentRequest {
    pub source_agent_id: String,
    pub name: String,
    pub spec: Value,
    pub expected_updated_at: i64,
    pub workspace_copy_destination: Option<String>,
    pub memory_seed: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AdoptExistingAgentResponse {
    pub agent: crate::agent::AgentRecord,
    pub team: TeamDefinitionRecord,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeamRunRequest {
    pub context_id: Option<String>,
    pub input: Option<Value>,
}

#[cfg(test)]
#[derive(Debug, Deserialize)]
pub struct CreateTeamTaskRequest {
    pub title: String,
    pub priority: String,
    pub assigned_member_id: String,
    pub created_by_actor_id: Option<String>,
    pub context: Option<Value>,
    pub conversation_mode: Option<String>,
    pub topic: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeamTaskFromChannelMessageRequest {
    pub title: Option<String>,
    pub priority: Option<String>,
    pub assigned_member_id: Option<String>,
    pub created_by_actor_id: Option<String>,
    pub context: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct ListTeamTasksQuery {
    pub limit: Option<i64>,
    pub priority: Option<String>,
    #[serde(default)]
    pub include_shared_thread: bool,
}

// Keep deserialization compatibility for existing human clients even though
// canonical task mutation is now rejected on the public HTTP path.
#[derive(Debug, Deserialize)]
pub struct UpdateTeamTaskRequest {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub assigned_member_id: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
pub struct SendTeamTaskMessageRequest {
    pub from_actor_id: Option<String>,
    pub to_actor_id: Option<String>,
    pub route: Option<String>,
    pub payload: Value,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListTeamTaskMessagesQuery {
    pub limit: Option<i64>,
    pub before_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SearchTeamMessagesQuery {
    #[serde(alias = "q")]
    pub query: String,
    pub limit: Option<usize>,
    pub authority_message_id: Option<i64>,
    pub correlation_id: Option<String>,
    pub group_id: Option<String>,
    pub run_id: Option<String>,
    pub conversation_id: Option<String>,
    pub task_id: Option<String>,
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub source_kind: Option<String>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct TeamMessageSearchHitResponse {
    pub document_id: String,
    pub source_kind: MessageDocumentKind,
    pub body_text: String,
    pub score: Option<f32>,
    pub authority_message_id: Option<i64>,
    pub correlation_id: Option<String>,
    pub group_id: Option<String>,
    pub team_id: Option<String>,
    pub run_id: Option<String>,
    pub conversation_id: Option<String>,
    pub task_id: Option<String>,
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReplyTeamThreadRequest {
    pub text: String,
    #[serde(default)]
    pub mention_actor_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeamChannelRequest {
    pub channel_id: String,
    pub description: Option<String>,
}

pub type TeamUploadRequest = UploadRequest;
pub type TeamDownloadRequest = DownloadRequest;

#[derive(Debug, Deserialize)]
pub struct CompileTeamTaskRunPreviewRequest {
    pub context_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListTeamRunsQuery {
    pub limit: Option<i64>,
    pub status: Option<String>,
    pub before_created_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ListTeamRunEventsQuery {
    pub limit: Option<i64>,
    pub before_id: Option<i64>,
}

const TEAM_PAGE_SNAPSHOT_LIMIT: i64 = 20;
const TEAM_PAGE_EVENT_LIMIT: i64 = TEAM_PAGE_SNAPSHOT_LIMIT;
const TEAM_PAGE_MESSAGE_LIMIT: i64 = TEAM_PAGE_SNAPSHOT_LIMIT;

#[derive(Debug, Deserialize)]
pub struct TeamRunSnapshotQuery {
    pub event_limit: Option<i64>,
    pub message_limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitTeamRunStepRequest {
    pub step_key: String,
    pub member_id: String,
    pub depends_on: Option<Vec<String>>,
    pub input: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct StartTeamRunStepRequest {
    /// Runtime executor handle supplied by external orchestrators.
    ///
    /// In the current implementation this is typically the member agent
    /// session id. The request accepts the legacy `remote_task_id` name for
    /// wire compatibility.
    #[serde(default, alias = "remote_task_id")]
    pub runtime_handle_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompleteTeamRunStepRequest {
    pub output: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct FailTeamRunStepRequest {
    pub error_text: String,
}

#[derive(Debug, Deserialize)]
pub struct SetTeamRunStepInputRequiredRequest {
    pub reason: Option<String>,
    pub input: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct ResumeTeamRunStepRequest {
    pub input: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct SendTeamRunMessageRequest {
    pub from_actor_id: String,
    pub from_peer_id: Option<String>,
    pub to_actor_id: String,
    pub to_peer_id: Option<String>,
    pub channel: Option<String>,
    pub transport: Option<String>,
    pub route: Option<Value>,
    pub payload: Value,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListTeamRunInboxQuery {
    pub actor_id: String,
    pub limit: Option<i64>,
    pub after_id: Option<i64>,
    pub include_delivered: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct FlushTeamRunContextRequest {
    pub member_id: String,
    pub session_id: Option<String>,
    pub trigger: Option<String>,
    pub max_events: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct FlushTeamRunContextResponse {
    pub status: String,
    pub run_id: String,
    pub team_id: String,
    pub member_id: String,
    pub session_id: Option<String>,
    pub trigger: String,
    pub reason: Option<String>,
    pub artifact_pointer: Option<Value>,
    pub event_id_from: Option<i64>,
    pub event_id_to: Option<i64>,
    pub flushed_events: i64,
}

#[derive(Debug, Deserialize)]
pub struct AckTeamRunMessageRequest {
    pub actor_id: String,
}

#[derive(Debug, Deserialize)]
pub struct TriageTeamRunMessageRequest {
    pub actor_id: String,
    pub disposition: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EscalateTeamRunMessageRequest {
    pub actor_id: String,
}

#[derive(Debug, Deserialize)]
pub struct TransferTeamRunMessageRequest {
    pub actor_id: String,
    pub target_actor_id: String,
}

#[derive(Debug, Deserialize)]
pub struct TakeoverTeamRunMessageRequest {
    pub actor_id: String,
    pub target_actor_id: String,
}

#[derive(Debug, Serialize)]
pub struct TeamRunSnapshotResponse {
    pub run: TeamRunRecord,
    pub team: TeamDefinitionRecord,
    pub coordinator_member_id: Option<String>,
    pub members: Vec<TeamMemberSnapshot>,
    pub steps: Vec<TeamStepRecord>,
    pub latest_events: Vec<TeamRunEventRecord>,
    pub mailbox: TeamMailboxSnapshot,
}

#[derive(Debug, Serialize)]
pub struct TeamMemberSnapshot {
    pub member_id: String,
    pub role: String,
    pub model: Option<String>,
    pub description: Option<String>,
    pub prompt: Option<String>,
    pub skills: Vec<String>,
    pub pending_inbox_count: i64,
    pub reply_obligation_count: i64,
    pub status: String,
    pub latest_step: Option<TeamStepRecord>,
    pub session_id: Option<String>,
    pub session_status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TeamMailboxSnapshot {
    pub pending: i64,
    pub delivered: i64,
    pub dead_letter: i64,
    pub open_reply_obligation_count: i64,
    pub open_reply_obligations: Vec<TeamReplyObligationRecord>,
    pub recent_messages: Vec<TeamActorMessageRecord>,
}

#[derive(Debug, Serialize)]
pub struct TeamTaskDetailResponse {
    pub task: TeamTaskRecord,
    pub conversation: TeamConversationRecord,
    pub latest_run: Option<TeamRunRecord>,
    pub notes: Vec<TeamTaskNoteRecord>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct TeamTaskRunCompilePreviewResponse {
    pub task_id: String,
    pub conversation_id: String,
    pub run_payload: TeamRunPayloadPreview,
    pub plan: TeamTaskCompiledPlan,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct TeamRunPayloadPreview {
    pub context_id: String,
    pub input: Value,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct TeamTaskCompiledPlan {
    pub task_list: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub deadline: Option<String>,
    pub step_template: Vec<TeamCompiledStepTemplate>,
    pub role_assignments: Vec<TeamCompiledRoleAssignment>,
    pub source_message_id: Option<i64>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct TeamCompiledStepTemplate {
    pub step_key: String,
    pub member_id: String,
    pub role: String,
    pub depends_on: Vec<String>,
    pub goal: Option<String>,
    pub acceptance: Vec<String>,
    pub execution: TeamTaskStepExecutionSpec,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct TeamCompiledRoleAssignment {
    pub member_id: String,
    pub role: String,
    pub step_keys: Vec<String>,
}

pub type TeamRuntimeControlResponse = crate::team::TeamRuntimeControlRecord;

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct TeamPromptDefaultsResponse {
    pub coordinator_prompt: String,
    pub worker_prompt: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", post(create_team).get(list_teams))
        .route("/prompt_defaults", get(get_team_prompt_defaults))
        .route("/{id}", get(get_team).delete(delete_team))
        .route("/{id}/spec", put(update_team_spec))
        .route("/{id}/members", get(list_teamspace_members))
        .route("/{id}/members/{user_id}", delete(revoke_teamspace_member))
        .route("/{id}/invites", post(create_teamspace_invite))
        .route("/invites/accept", post(accept_teamspace_invite))
        .route("/{id}/members/adopt", post(adopt_existing_agent_to_team))
        .route("/{id}/members/move", post(move_existing_agent_to_team))
        .route("/{id}/runtime", get(get_team_runtime))
        .route(
            "/{id}/shared_thread",
            get(get_team_shared_thread).post(ensure_team_shared_thread),
        )
        .route("/{id}/start", post(start_team))
        .route("/{id}/stop", post(stop_team))
        .route(
            "/{id}/members/{member_id}/force_new_session",
            post(force_new_session_for_team_member),
        )
        .route("/{id}/tasks", get(list_team_tasks))
        .route("/{id}/tasks/{task_id}/handoff", post(handoff_team_task))
        .route(
            "/{id}/tasks/{task_id}/forks",
            get(list_goal_forks).post(create_goal_fork),
        )
        .route(
            "/{id}/tasks/{task_id}/forks/{fork_id}/complete",
            post(complete_goal_fork),
        )
        .route(
            "/{id}/tasks/{task_id}",
            get(get_team_task).patch(update_team_task),
        )
        .route(
            "/{id}/tasks/{task_id}/messages",
            post(send_team_task_message).get(list_team_task_messages),
        )
        .route("/{id}/messages/search", get(search_team_messages))
        .route(
            "/{id}/channels/{channel_id}/threads/{root_message_id}/replies",
            post(reply_team_thread),
        )
        .route(
            "/{id}/channels/{channel_id}/messages/{message_id}/tasks",
            post(create_team_task_from_channel_message),
        )
        .route(
            "/{id}/channels",
            get(list_team_channels).post(create_team_channel),
        )
        .route("/{id}/uploads", post(upload_team_object))
        .route("/{id}/uploads/downloads", post(download_team_object))
        .route("/{id}/images", post(upload_team_image))
        .route(
            "/{id}/tasks/{task_id}/uploads",
            post(upload_team_task_object),
        )
        .route(
            "/{id}/tasks/{task_id}/uploads/downloads",
            post(download_team_task_object),
        )
        .route("/{id}/tasks/{task_id}/images", post(upload_team_task_image))
        .route("/{id}/channels/{channel_id}", delete(delete_team_channel))
        .route(
            "/{id}/tasks/{task_id}/compile_run_preview",
            post(compile_team_task_run_preview),
        )
        .route("/{id}/runs", post(create_team_run).get(list_team_runs))
        .route("/runs/{run_id}", get(get_team_run))
        .route("/runs/{run_id}/cancel", post(cancel_team_run))
        .route("/runs/{run_id}/resume", post(resume_team_run))
        .route("/runs/{run_id}/restart", post(restart_team_run))
        .route("/runs/{run_id}/snapshot", get(get_team_run_snapshot))
        .route("/runs/{run_id}/events", get(list_team_run_events))
        .route("/runs/{run_id}/context/flush", post(flush_team_run_context))
        .route(
            "/runs/{run_id}/steps",
            post(submit_team_run_step).get(list_team_run_steps),
        )
        .route(
            "/runs/{run_id}/steps/{step_id}/start",
            post(start_team_run_step),
        )
        .route(
            "/runs/{run_id}/steps/{step_id}/complete",
            post(complete_team_run_step),
        )
        .route(
            "/runs/{run_id}/steps/{step_id}/fail",
            post(fail_team_run_step),
        )
        .route(
            "/runs/{run_id}/steps/{step_id}/input_required",
            post(set_team_run_step_input_required),
        )
        .route(
            "/runs/{run_id}/steps/{step_id}/resume",
            post(resume_team_run_step),
        )
        .route("/runs/{run_id}/messages/send", post(send_team_run_message))
        .route("/runs/{run_id}/messages/inbox", get(list_team_run_inbox))
        .route(
            "/runs/{run_id}/messages/{message_id}/ack",
            post(ack_team_run_message),
        )
        .route(
            "/runs/{run_id}/messages/{message_id}/triage",
            post(triage_team_run_message),
        )
        .route(
            "/runs/{run_id}/messages/{message_id}/escalate",
            post(escalate_team_run_message),
        )
        .route(
            "/runs/{run_id}/messages/{message_id}/transfer",
            post(transfer_team_run_message),
        )
        .route(
            "/runs/{run_id}/messages/{message_id}/takeover",
            post(takeover_team_run_message),
        )
        .with_state(state)
}

async fn get_team_prompt_defaults(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TeamPromptDefaultsResponse>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    Ok(Json(TeamPromptDefaultsResponse {
        coordinator_prompt: DEFAULT_TEAM_COORDINATOR_PROMPT.to_string(),
        worker_prompt: DEFAULT_TEAM_WORKER_PROMPT.to_string(),
    }))
}

async fn create_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateTeamRequest>,
) -> Result<Json<TeamDefinitionRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad_request("team name is required"));
    }
    let mut spec = payload.spec;
    normalize_team_spec(&mut spec)?;
    validate_team_spec(&spec)?;
    let team = state
        .teams
        .create_team_with_owner(
            TeamDefinitionConfig {
                name,
                description: payload.description,
                spec,
            },
            Some(user.id.as_str()),
        )
        .await
        .map_err(map_create_team_error)?;
    if has_configured_team_members(&team.spec)?
        && let Err(err) = ensure_team_runtime_started(state.agents.as_ref(), &team).await
    {
        let member_ids = parse_member_ids(team.spec.get("members"))?;
        let _ = state.teams.delete_team(&team.id, &member_ids).await;
        return Err(map_runtime_start_error(err));
    }
    Ok(Json(sanitize_team_definition_for_response(team)))
}

async fn update_team_spec(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Json(payload): Json<UpdateTeamSpecRequest>,
) -> Result<Json<TeamDefinitionRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    let current = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &current, &user, &["owner"]).await?;
    let previous_member_ids = parse_member_ids(current.spec.get("members"))?;
    let mut spec = payload.spec;
    normalize_team_spec(&mut spec)?;
    validate_team_spec(&spec)?;
    let updated = state
        .teams
        .update_team_spec_if_unchanged(&current.id, payload.expected_updated_at, spec)
        .await
        .map_err(map_team_internal_error)?
        .ok_or_else(|| {
            ApiError::conflict("team definition changed concurrently; reload and retry")
        })?;
    let next_member_ids = parse_member_ids(updated.spec.get("members"))?;
    for removed_member_id in previous_member_ids.difference(&next_member_ids) {
        let _ = state.agents.stop_agent(removed_member_id).await;
    }
    if has_configured_team_members(&updated.spec)? {
        ensure_team_runtime_started(state.agents.as_ref(), &updated)
            .await
            .map_err(map_runtime_start_error)?;
    }
    Ok(Json(sanitize_team_definition_for_response(updated)))
}

async fn move_existing_agent_to_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Json(payload): Json<MoveExistingAgentRequest>,
) -> Result<Json<TeamDefinitionRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    let current = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &current, &user, &["owner"]).await?;
    let agent_id = payload.agent_id.trim();
    if agent_id.is_empty() {
        return Err(ApiError::bad_request("agent_id is required"));
    }

    let agent = state
        .agents
        .get_agent(agent_id)
        .await
        .map_err(|err| map_not_found_error(err, "agent not found"))?;
    if !matches!(agent.status, AgentStatus::Created | AgentStatus::Stopped) {
        return Err(ApiError::conflict(
            "agent must be created or stopped before moving it into a team",
        ));
    }
    if !state
        .teams
        .list_teams_referencing_member(agent_id)
        .await
        .map_err(map_team_internal_error)?
        .is_empty()
    {
        return Err(ApiError::conflict("agent already belongs to a team"));
    }

    let previous_member_ids = parse_member_ids(current.spec.get("members"))?;
    let mut spec = payload.spec;
    normalize_team_spec(&mut spec)?;
    validate_team_spec(&spec)?;
    let next_member_ids = parse_member_ids(spec.get("members"))?;
    let added_member_ids = next_member_ids
        .difference(&previous_member_ids)
        .collect::<Vec<_>>();
    if !previous_member_ids.is_subset(&next_member_ids) {
        return Err(ApiError::bad_request(
            "move must not remove existing team members",
        ));
    }
    if added_member_ids.len() != 1 || added_member_ids[0].as_str() != agent_id {
        return Err(ApiError::bad_request(
            "move must add exactly the selected agent to the current team spec",
        ));
    }

    let updated = state
        .teams
        .update_team_spec_if_unchanged(&current.id, payload.expected_updated_at, spec)
        .await
        .map_err(map_team_internal_error)?
        .ok_or_else(|| {
            ApiError::conflict("team definition changed concurrently; reload and retry")
        })?;
    if has_configured_team_members(&updated.spec)? {
        ensure_team_runtime_started(state.agents.as_ref(), &updated)
            .await
            .map_err(map_runtime_start_error)?;
    }
    Ok(Json(sanitize_team_definition_for_response(updated)))
}

async fn adopt_existing_agent_to_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Json(payload): Json<AdoptExistingAgentRequest>,
) -> Result<Json<AdoptExistingAgentResponse>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    let current = load_team_for_user(&state, &team_id, &_user).await?;
    require_teamspace_role(&state, &current, &_user, &["owner"]).await?;
    let source_id = payload.source_agent_id.trim();
    if source_id.is_empty() || payload.name.trim().is_empty() {
        return Err(ApiError::bad_request(
            "source_agent_id and name are required",
        ));
    }
    let source = state
        .agents
        .get_agent(source_id)
        .await
        .map_err(|err| map_not_found_error(err, "agent not found"))?;
    if !state
        .teams
        .list_teams_referencing_member(source_id)
        .await
        .map_err(map_team_internal_error)?
        .is_empty()
    {
        return Err(ApiError::conflict("agent already belongs to a team"));
    }
    if source.target_node_id.is_some() && payload.workspace_copy_destination.is_some() {
        return Err(ApiError::bad_request(
            "workspace-content copy is only available for local agents",
        ));
    }
    let destination = payload
        .workspace_copy_destination
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let seed = payload
        .memory_seed
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if seed.is_some() && destination.is_none() {
        return Err(ApiError::bad_request(
            "memory seed requires a copied workspace destination",
        ));
    }
    let workdir = destination.unwrap_or(&source.workdir).to_string();
    let mut spec = payload.spec;
    let member_id = Uuid::new_v4().to_string();
    replace_adopted_member_placeholder(&mut spec, &member_id);
    normalize_team_spec(&mut spec)?;
    validate_team_spec(&spec)?;
    let previous_member_ids = parse_member_ids(current.spec.get("members"))?;
    let next_member_ids = parse_member_ids(spec.get("members"))?;
    if !previous_member_ids.is_subset(&next_member_ids)
        || next_member_ids
            .difference(&previous_member_ids)
            .collect::<Vec<_>>()
            != vec![&member_id]
    {
        return Err(ApiError::bad_request(
            "adoption must add exactly one new member",
        ));
    }
    if let Some(destination) = destination {
        copy_adoption_workspace(&source.workdir, destination, &source.id, &current.id, seed)?;
    }
    let agent_result = state
        .agents
        .create_agent_with_source(
            AgentConfig {
                name: payload.name.trim().to_string(),
                workdir,
                command: source.command.clone(),
                args: source.args.clone(),
                target_node_id: source.target_node_id.clone(),
                worktree_mode: WorktreeMode::UseExisting,
                worktree_repo: None,
                worktree_ref: None,
                code_mode: source.code_mode,
                codex_acp_default_mode: source.codex_acp_default_mode.clone(),
                runtime_model: source.runtime_model.clone(),
                thinking_level: source.thinking_level.clone(),
                agent_loop_enabled: false,
                agent_loop_idle_seconds: None,
                agent_loop_prompt: None,
            },
            "team_forge",
        )
        .await;
    let agent = match agent_result {
        Ok(agent) => agent,
        Err(err) => {
            if let Some(destination) = destination {
                let _ = fs::remove_dir_all(destination);
            }
            return Err(map_team_internal_error(err));
        }
    };
    let updated = match state
        .teams
        .update_team_spec_if_unchanged(&current.id, payload.expected_updated_at, spec)
        .await
        .map_err(map_team_internal_error)?
    {
        Some(team) => team,
        None => {
            let _ = state.agents.delete_agent(&agent.id).await;
            if let Some(destination) = destination {
                let _ = fs::remove_dir_all(destination);
            }
            return Err(ApiError::conflict(
                "team definition changed concurrently; reload and retry",
            ));
        }
    };
    if let Err(err) = ensure_team_runtime_started(state.agents.as_ref(), &updated).await {
        let _ = state.agents.stop_agent(&agent.id).await;
        return Err(map_runtime_start_error(err));
    }
    Ok(Json(AdoptExistingAgentResponse {
        agent,
        team: sanitize_team_definition_for_response(updated),
    }))
}

fn replace_adopted_member_placeholder(value: &mut Value, member_id: &str) {
    match value {
        Value::String(current) if current == ADOPTED_MEMBER_ID_PLACEHOLDER => {
            *current = member_id.to_string()
        }
        Value::Array(values) => values
            .iter_mut()
            .for_each(|value| replace_adopted_member_placeholder(value, member_id)),
        Value::Object(values) => values
            .values_mut()
            .for_each(|value| replace_adopted_member_placeholder(value, member_id)),
        _ => {}
    }
}

fn copy_adoption_workspace(
    source: &str,
    destination: &str,
    source_agent_id: &str,
    team_id: &str,
    seed: Option<&str>,
) -> Result<(), ApiError> {
    let source = fs::canonicalize(source)
        .map_err(|_| ApiError::bad_request("source workspace must exist"))?;
    let destination = PathBuf::from(destination);
    if destination.exists() {
        return Err(ApiError::conflict(
            "workspace copy destination must not already exist",
        ));
    }
    let parent = destination.parent().ok_or_else(|| {
        ApiError::bad_request("workspace copy destination must have a parent directory")
    })?;
    fs::create_dir_all(parent).map_err(ApiError::from)?;
    let temporary = parent.join(format!(".agenthub-adoption-{}", Uuid::new_v4()));
    let result = (|| {
        copy_adoption_directory(&source, &temporary)?;
        let manifest = serde_json::json!({"source_agent_id": source_agent_id, "team_id": team_id, "source": source, "destination": destination, "excluded": [".git", ".agenthub", ".agenthubmemory", ".cache", ".env", "*.sock", "*.lock"]});
        fs::write(
            temporary.join(".agenthub-adoption-manifest.json"),
            serde_json::to_vec_pretty(&manifest).map_err(ApiError::from)?,
        )
        .map_err(ApiError::from)?;
        if let Some(seed) = seed {
            fs::write(temporary.join(".agenthub-team-seed.json"), serde_json::to_vec_pretty(&serde_json::json!({"source_agent_id": source_agent_id, "team_id": team_id, "seed": seed})).map_err(ApiError::from)?).map_err(ApiError::from)?;
        }
        fs::rename(&temporary, &destination).map_err(ApiError::from)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&temporary);
    }
    result
}

fn copy_adoption_directory(source: &StdPath, destination: &StdPath) -> Result<(), ApiError> {
    fs::create_dir_all(destination).map_err(ApiError::from)?;
    for entry in fs::read_dir(source).map_err(ApiError::from)? {
        let entry = entry.map_err(ApiError::from)?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if [".git", ".agenthub", ".agenthubmemory", ".cache", ".env"].contains(&name.as_ref())
            || name.ends_with(".sock")
            || name.ends_with(".lock")
        {
            continue;
        }
        let target = destination.join(entry.file_name());
        let file_type = entry.file_type().map_err(ApiError::from)?;
        if file_type.is_dir() {
            copy_adoption_directory(&path, &target)?;
        } else if file_type.is_file() {
            fs::copy(&path, target).map_err(ApiError::from)?;
        }
    }
    Ok(())
}

async fn start_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Json<TeamRuntimeControlResponse>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &team, &user, &["owner"]).await?;
    ensure_team_execution_ready(&team.spec)?;
    let runtime = ensure_team_runtime_started(state.agents.as_ref(), &team)
        .await
        .map_err(map_runtime_start_error)?;
    Ok(Json(runtime))
}

async fn stop_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Json<TeamRuntimeControlResponse>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &team, &user, &["owner"]).await?;
    let runtime = stop_team_runtime(state.agents.as_ref(), &team)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(runtime))
}

pub(crate) async fn prune_deleted_agent_from_team_specs(
    state: &AppState,
    agent_id: &str,
) -> Result<(), ApiError> {
    let teams = state.teams.list_teams_referencing_member(agent_id).await?;
    for team in teams {
        let Some(next_spec) = prune_deleted_member_from_team_spec(&team.spec, agent_id)? else {
            continue;
        };
        let updated = match state
            .teams
            .update_team_spec_if_unchanged(&team.id, team.updated_at, next_spec.clone())
            .await?
        {
            Some(updated) => updated,
            None => {
                let latest = state.teams.get_team(&team.id).await?;
                let Some(latest_spec) =
                    prune_deleted_member_from_team_spec(&latest.spec, agent_id)?
                else {
                    continue;
                };
                state
                    .teams
                    .update_team_spec_if_unchanged(&latest.id, latest.updated_at, latest_spec)
                    .await?
                    .ok_or_else(|| {
                        ApiError::conflict(
                            "team definition changed concurrently while pruning deleted agent",
                        )
                    })?
            }
        };
        if has_configured_team_members(&updated.spec)? {
            ensure_team_runtime_started(state.agents.as_ref(), &updated)
                .await
                .map_err(map_runtime_start_error)?;
        }
    }
    Ok(())
}

async fn force_new_session_for_team_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, member_id)): Path<(String, String)>,
) -> Result<Json<TeamRuntimeControlResponse>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &team, &user, &["owner"]).await?;
    ensure_team_execution_ready(&team.spec)?;
    let runtime = force_team_member_new_session(state.agents.as_ref(), &team, &member_id)
        .await
        .map_err(map_runtime_start_error)?;
    Ok(Json(runtime))
}

async fn list_teams(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<TeamDefinitionRecord>>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let teams = state
        .teams
        .list_teams_for_user(&user.id)
        .await?
        .into_iter()
        .map(sanitize_team_definition_for_response)
        .collect::<Vec<_>>();
    Ok(Json(teams))
}

async fn list_teamspace_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Json<Vec<crate::team::TeamspaceMemberRecord>>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    Ok(Json(state.teams.list_teamspace_members(&team_id).await?))
}

async fn revoke_teamspace_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, user_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &team, &user, &["owner"]).await?;
    if user_id.trim().is_empty() {
        return Err(ApiError::bad_request("user_id cannot be empty"));
    }
    if user_id == user.id {
        return Err(ApiError::bad_request(
            "Teamspace owners cannot revoke themselves",
        ));
    }
    state
        .teams
        .revoke_teamspace_member(&team_id, &user_id, &user.id)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(serde_json::json!({"status": "revoked"})))
}

async fn create_teamspace_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Json(payload): Json<CreateTeamspaceInviteRequest>,
) -> Result<Json<CreateTeamspaceInviteResponse>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &team, &user, &["owner"]).await?;
    let expires_in_seconds = payload.expires_in_seconds.unwrap_or(7 * 24 * 60 * 60);
    if !(60..=30 * 24 * 60 * 60).contains(&expires_in_seconds) {
        return Err(ApiError::bad_request(
            "invite expiry must be between 60 seconds and 30 days",
        ));
    }
    let expires_at = chrono::Utc::now().timestamp() + expires_in_seconds;
    let (invite, token) = state
        .teams
        .create_teamspace_invite(&team_id, &payload.role, &user.id, expires_at)
        .await
        .map_err(map_team_internal_error)?;
    let base = state.auth.rp_origin();
    Ok(Json(CreateTeamspaceInviteResponse {
        invite,
        url: format!("{}/join#{}", base.trim_end_matches('/'), token),
    }))
}

async fn accept_teamspace_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AcceptTeamspaceInviteRequest>,
) -> Result<Json<crate::team::TeamspaceMemberRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    if payload.token.trim().is_empty() {
        return Err(ApiError::bad_request("invite token cannot be empty"));
    }
    let member = state
        .teams
        .accept_teamspace_invite(&payload.token, &user.id)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(member))
}

async fn get_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Json<TeamDefinitionRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    Ok(Json(sanitize_team_definition_for_response(team)))
}

async fn get_team_runtime(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Json<TeamRuntimeRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    reconcile_team_member_runtime_absence(&state, &team).await?;
    let runtime = state
        .teams
        .describe_team_runtime(team.id.as_str())
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(runtime))
}

async fn delete_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Json<TeamDefinitionRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    let member_ids = match parse_member_ids(team.spec.get("members")) {
        Ok(ids) => ids,
        Err(err) => {
            tracing::warn!(
                "delete_team fallback: failed to parse member ids for team {}: {:?}",
                team_id,
                err
            );
            HashSet::new()
        }
    };

    for member_id in &member_ids {
        let _ = state.agents.stop_agent(member_id).await;
    }

    let team = state
        .teams
        .delete_team(&team_id, &member_ids)
        .await
        .map_err(|err| map_not_found_error(err, "team not found"))?;
    Ok(Json(sanitize_team_definition_for_response(team)))
}

async fn get_team_shared_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Json<TeamTaskDetailResponse>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let Some((task, conversation, latest_run)) = state
        .teams
        .get_shared_thread_detail_for_team(&team_id)
        .await
        .map_err(map_team_internal_error)?
    else {
        return Err(ApiError::not_found("shared thread not found"));
    };
    let notes = state
        .teams
        .list_task_notes(&task.id, 100)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(TeamTaskDetailResponse {
        task,
        conversation,
        latest_run,
        notes,
    }))
}

async fn ensure_team_shared_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Json<TeamTaskDetailResponse>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &team, &user, &["owner", "planner"]).await?;
    let (task, conversation, latest_run) = state
        .teams
        .ensure_shared_thread_detail_for_team(&team_id, &canonical_user_actor_id(&user))
        .await
        .map_err(map_team_internal_error)?;
    let notes = state
        .teams
        .list_task_notes(&task.id, 100)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(TeamTaskDetailResponse {
        task,
        conversation,
        latest_run,
        notes,
    }))
}

async fn list_team_tasks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Query(query): Query<ListTeamTasksQuery>,
) -> Result<Json<Vec<TeamTaskRecord>>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let priority = query
        .priority
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "all")
        .map(|raw| {
            raw.parse::<TeamTaskPriority>().map_err(|_| {
                ApiError::bad_request(
                    "invalid task priority; expected one of: critical, high, medium, low",
                )
            })
        })
        .transpose()?;
    let tasks = state
        .teams
        .list_tasks_with_query(crate::team::TeamTaskListQuery {
            team_id: Some(team_id),
            limit: query.limit.unwrap_or(100).clamp(1, 500),
            priority,
            include_shared_thread: query.include_shared_thread,
            ..crate::team::TeamTaskListQuery::default()
        })
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(tasks))
}

async fn create_team_task_from_channel_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, channel_id, message_id)): Path<(String, String, i64)>,
    Json(payload): Json<CreateTeamTaskFromChannelMessageRequest>,
) -> Result<Json<TeamTaskDetailResponse>, ApiError> {
    if message_id <= 0 {
        return Err(ApiError::bad_request("message_id must be positive"));
    }
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &team, &user, &["owner", "planner"]).await?;
    let channel_id = channel_id.trim().to_lowercase();
    if channel_id.is_empty() {
        return Err(ApiError::bad_request("channel_id is required"));
    }
    let source_message = state
        .teams
        .get_channel_conversation_message(&team_id, &channel_id, message_id)
        .await
        .map_err(|err| map_not_found_error(err, "channel message not found"))?;
    if source_message.route == "team_thread_reply" {
        return Err(ApiError::bad_request(
            "thread replies cannot be converted into tasks directly; convert the root channel message",
        ));
    }

    let source_excerpt = summarize_task_source_message(&source_message.payload);
    let title = payload
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| source_excerpt.clone());
    if title.trim().is_empty() {
        return Err(ApiError::bad_request(
            "title is required when the source message has no readable text",
        ));
    }
    let priority = normalize_task_priority(payload.priority.as_deref())?;
    let assigned_member_id = payload
        .assigned_member_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let created_by_actor_id =
        normalize_task_created_by_actor_id(payload.created_by_actor_id.as_deref(), &user)?;
    let mut context = match payload.context {
        Some(Value::Object(map)) => map,
        None | Some(Value::Null) => Map::new(),
        Some(_) => {
            return Err(ApiError::bad_request(
                "context must be a JSON object when provided",
            ));
        }
    };
    context.insert(
        "bootstrap_kind".to_string(),
        Value::String("channel_message_task".to_string()),
    );
    context.insert(
        "source".to_string(),
        serde_json::json!({
            "kind": "team_channel_message",
            "team_id": team_id,
            "channel_id": channel_id,
            "conversation_id": source_message.conversation_id,
            "task_id": source_message.task_id,
            "message_id": source_message.message_id,
            "from_actor_id": source_message.from_actor_id,
            "created_at": source_message.created_at,
            "excerpt": source_excerpt,
        }),
    );

    let (task, conversation) = state
        .teams
        .create_task_with_metadata(TeamTaskCreateInput {
            team_id: &team_id,
            title: title.trim(),
            created_by_actor_id: &created_by_actor_id,
            priority,
            assigned_member_id,
            context: Value::Object(context),
            conversation_mode: "group_chat",
            topic: Some(&channel_id),
        })
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(TeamTaskDetailResponse {
        task,
        conversation,
        latest_run: None,
        notes: Vec::new(),
    }))
}

async fn list_team_channels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Json<Vec<TeamChannelRecord>>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let channels = state
        .teams
        .list_channels(&team_id)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(channels))
}

async fn create_team_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Json(payload): Json<CreateTeamChannelRequest>,
) -> Result<Json<TeamChannelRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let channel = state
        .teams
        .create_channel(
            &team_id,
            &payload.channel_id,
            payload.description.as_deref(),
            &canonical_user_actor_id(&user),
        )
        .await
        .map_err(map_channel_create_error)?;
    Ok(Json(channel))
}

async fn delete_team_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, channel_id)): Path<(String, String)>,
) -> Result<Json<TeamChannelRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let deleted = state
        .teams
        .delete_channel(&team_id, &channel_id)
        .await
        .map_err(map_channel_delete_error)?;
    Ok(Json(deleted))
}

async fn upload_team_object(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Json(payload): Json<TeamUploadRequest>,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    upload_team_scoped_object(state, headers, team_id, payload, ObjectUploadKind::Object).await
}

async fn upload_team_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Json(payload): Json<TeamUploadRequest>,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    upload_team_scoped_object(state, headers, team_id, payload, ObjectUploadKind::Image).await
}

async fn download_team_object(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Json(payload): Json<TeamDownloadRequest>,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    download_scoped_object(
        State(state),
        &user,
        ObjectUploadOwnerScope::Team(team.id),
        payload,
    )
    .await
}

async fn upload_team_scoped_object(
    state: AppState,
    headers: HeaderMap,
    team_id: String,
    payload: TeamUploadRequest,
    kind: ObjectUploadKind,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    upload_scoped_object(
        State(state),
        &user,
        ObjectUploadOwnerScope::Team(team.id),
        payload,
        kind,
    )
    .await
}

async fn upload_team_task_object(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id)): Path<(String, String)>,
    Json(payload): Json<TeamUploadRequest>,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    upload_team_task_scoped_object(
        state,
        headers,
        team_id,
        task_id,
        payload,
        ObjectUploadKind::Object,
    )
    .await
}

async fn upload_team_task_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id)): Path<(String, String)>,
    Json(payload): Json<TeamUploadRequest>,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    upload_team_task_scoped_object(
        state,
        headers,
        team_id,
        task_id,
        payload,
        ObjectUploadKind::Image,
    )
    .await
}

async fn upload_team_task_scoped_object(
    state: AppState,
    headers: HeaderMap,
    team_id: String,
    task_id: String,
    payload: TeamUploadRequest,
    kind: ObjectUploadKind,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let task = state
        .teams
        .get_task(&task_id)
        .await
        .map_err(|err| map_not_found_error(err, "task not found"))?;
    if task.team_id != team_id {
        return Err(ApiError::not_found("task not found"));
    }
    upload_scoped_object(
        State(state),
        &user,
        ObjectUploadOwnerScope::Task(task.id),
        payload,
        kind,
    )
    .await
}

async fn download_team_task_object(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id)): Path<(String, String)>,
    Json(payload): Json<TeamDownloadRequest>,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let task = state
        .teams
        .get_task(&task_id)
        .await
        .map_err(|err| map_not_found_error(err, "task not found"))?;
    if task.team_id != team_id {
        return Err(ApiError::not_found("task not found"));
    }
    download_scoped_object(
        State(state),
        &user,
        ObjectUploadOwnerScope::Task(task.id),
        payload,
    )
    .await
}

async fn get_team_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id)): Path<(String, String)>,
) -> Result<Json<TeamTaskDetailResponse>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let detail = state
        .teams
        .get_task_detail(&task_id, 100)
        .await
        .map_err(|err| map_not_found_error(err, "task not found"))?;
    if detail.task.team_id != team_id {
        return Err(ApiError::not_found("task not found"));
    }
    Ok(Json(TeamTaskDetailResponse {
        task: detail.task,
        conversation: detail.conversation,
        latest_run: detail.latest_run,
        notes: detail.notes,
    }))
}

async fn update_team_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id)): Path<(String, String)>,
    Json(payload): Json<UpdateTeamTaskRequest>,
) -> Result<Json<TeamTaskRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let task = state
        .teams
        .get_task(&task_id)
        .await
        .map_err(|err| map_not_found_error(err, "task not found"))?;
    if task.team_id != team_id {
        return Err(ApiError::not_found("task not found"));
    }
    let error_message = if payload.status.is_some() || payload.assigned_member_id.is_some() {
        "canonical Team task status/owner updates are agent-only; use actor runtime controls"
    } else {
        "canonical Team task updates are agent-only; use actor runtime controls"
    };
    Err(ApiError::forbidden(error_message))
}

async fn handoff_team_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id)): Path<(String, String)>,
    Json(payload): Json<HandoffTeamTaskRequest>,
) -> Result<Json<TeamTaskRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &team, &user, &["owner"]).await?;
    let existing = state
        .teams
        .get_task(&task_id)
        .await
        .map_err(|err| map_not_found_error(err, "task not found"))?;
    if existing.team_id != team_id {
        return Err(ApiError::not_found("task not found"));
    }
    let task = state
        .teams
        .handoff_task_execution(
            &task_id,
            &payload.assigned_member_id,
            &user.id,
            &payload.reason,
        )
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(task))
}

async fn create_goal_fork(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id)): Path<(String, String)>,
    Json(payload): Json<CreateGoalForkRequest>,
) -> Result<Json<crate::team::TeamGoalForkRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &team, &user, &["owner", "planner"]).await?;
    let task = state
        .teams
        .get_task(&task_id)
        .await
        .map_err(|err| map_not_found_error(err, "task not found"))?;
    if task.team_id != team_id {
        return Err(ApiError::not_found("task not found"));
    }
    if payload.question.trim().is_empty() || payload.acceptance_criteria.trim().is_empty() {
        return Err(ApiError::bad_request(
            "fork question and acceptance criteria are required",
        ));
    }
    let expires_in_seconds = payload.expires_in_seconds.unwrap_or(15 * 60);
    if !(60..=60 * 60).contains(&expires_in_seconds) {
        return Err(ApiError::bad_request(
            "fork expiry must be between 60 seconds and one hour",
        ));
    }
    let fork = state
        .teams
        .create_goal_fork(
            &task_id,
            &payload.question,
            &payload.acceptance_criteria,
            payload
                .result_schema
                .unwrap_or_else(|| serde_json::json!({"type": "object"})),
            chrono::Utc::now().timestamp() + expires_in_seconds,
            Some(&user.id),
        )
        .await
        .map_err(map_goal_fork_error)?;
    Ok(Json(fork))
}

async fn list_goal_forks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id)): Path<(String, String)>,
) -> Result<Json<Vec<crate::team::TeamGoalForkRecord>>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let task = state
        .teams
        .get_task(&task_id)
        .await
        .map_err(|err| map_not_found_error(err, "task not found"))?;
    if task.team_id != team_id {
        return Err(ApiError::not_found("task not found"));
    }
    let forks = state
        .teams
        .list_goal_forks(&task_id, 100)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(forks))
}

async fn complete_goal_fork(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id, fork_id)): Path<(String, String, String)>,
    Json(payload): Json<CompleteGoalForkRequest>,
) -> Result<Json<crate::team::TeamGoalForkRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::TeamsManage).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &team, &user, &["owner", "planner"]).await?;
    let task = state
        .teams
        .get_task(&task_id)
        .await
        .map_err(|err| map_not_found_error(err, "task not found"))?;
    if task.team_id != team_id {
        return Err(ApiError::not_found("task not found"));
    }
    let existing = state
        .teams
        .get_goal_fork(&fork_id)
        .await
        .map_err(|err| map_not_found_error(err, "fork not found"))?;
    if existing.parent_task_id != task_id {
        return Err(ApiError::not_found("fork not found"));
    }
    let fork = state
        .teams
        .complete_goal_fork(&fork_id, payload.result, Some(&user.id))
        .await
        .map_err(map_goal_fork_error)?;
    Ok(Json(fork))
}

async fn send_team_task_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id)): Path<(String, String)>,
    Json(payload): Json<SendTeamTaskMessageRequest>,
) -> Result<Json<TeamConversationMessageRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    let SendTeamTaskMessageRequest {
        from_actor_id,
        to_actor_id,
        route,
        payload,
        idempotency_key,
    } = payload;
    let task = state
        .teams
        .get_task(&task_id)
        .await
        .map_err(|err| map_not_found_error(err, "task not found"))?;
    if task.team_id != team_id {
        return Err(ApiError::not_found("task not found"));
    }
    let actor_scope = parse_task_actor_scope(&team.spec, &user)?;
    let from_actor_id = from_actor_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|raw| normalize_task_actor_id(raw, "from_actor_id", &user))
        .transpose()?
        .unwrap_or_else(|| canonical_user_actor_id(&user));
    let to_actor_id = to_actor_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|raw| normalize_task_actor_id(raw, "to_actor_id", &user))
        .transpose()?;
    let route = infer_task_message_route(
        &actor_scope,
        route.as_deref(),
        to_actor_id.as_deref(),
        &payload,
    )?;
    validate_task_message_sender(&actor_scope, &from_actor_id)?;
    let resolved_to_actor_id =
        resolve_task_message_target(&actor_scope, &route, to_actor_id, &payload)?;
    let idempotency_key = normalize_optional_idempotency_key(idempotency_key.as_deref())?;
    let payload = ensure_task_message_correlation_id(
        normalize_task_message_payload(payload),
        Some(TaskMessageCorrelationSeed {
            task_id: &task_id,
            from_actor_id: &from_actor_id,
            to_actor_id: resolved_to_actor_id.as_deref(),
            route: &route,
            idempotency_key: idempotency_key.as_deref(),
        }),
    );
    let (message, created) = state
        .teams
        .append_task_conversation_message_with_created(
            &task_id,
            &from_actor_id,
            resolved_to_actor_id.as_deref(),
            &route,
            payload,
            idempotency_key.as_deref(),
        )
        .await
        .map_err(map_task_message_error)?;
    if created
        && from_actor_id == actor_scope.user_actor_id
        && let Err(err) =
            maybe_forward_task_message_to_mailbox(&state, &team, &task, &actor_scope, &message)
                .await
    {
        tracing::warn!(
            team_id = %team.id,
            task_id = %task_id,
            message_id = message.message_id,
            "task conversation mailbox forwarding failed: {:?}",
            err
        );
    }
    Ok(Json(message))
}

async fn list_team_task_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id)): Path<(String, String)>,
    Query(query): Query<ListTeamTaskMessagesQuery>,
) -> Result<Json<Vec<TeamConversationMessageRecord>>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let task = state
        .teams
        .get_task(&task_id)
        .await
        .map_err(|err| map_not_found_error(err, "task not found"))?;
    if task.team_id != team_id {
        return Err(ApiError::not_found("task not found"));
    }
    let messages = state
        .teams
        .list_task_conversation_messages(
            &task_id,
            query.limit.unwrap_or(200).clamp(1, 500),
            query.before_id,
        )
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(messages))
}

async fn search_team_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Query(query): Query<SearchTeamMessagesQuery>,
) -> Result<Json<Vec<TeamMessageSearchHitResponse>>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let query_text = normalize_optional_string(Some(query.query))
        .ok_or_else(|| ApiError::bad_request("query is required"))?;
    let source_kind = normalize_optional_string(query.source_kind)
        .as_deref()
        .map(parse_message_archive_source_kind)
        .transpose()?;
    let archive_query = MessageSearchQuery {
        query_text,
        limit: query.limit.unwrap_or(20).clamp(1, 100),
        authority_message_id: query.authority_message_id,
        correlation_id: normalize_optional_string(query.correlation_id),
        group_id: normalize_optional_string(query.group_id),
        team_id: Some(team_id),
        run_id: normalize_optional_string(query.run_id),
        conversation_id: normalize_optional_string(query.conversation_id),
        task_id: normalize_optional_string(query.task_id),
        agent_id: normalize_optional_string(query.agent_id),
        session_id: normalize_optional_string(query.session_id),
        source_kind,
    };
    let hits = state
        .teams
        .search_message_archive(&archive_query)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(
        hits.into_iter()
            .map(TeamMessageSearchHitResponse::from)
            .collect(),
    ))
}

async fn reply_team_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, channel_id, root_message_id)): Path<(String, String, i64)>,
    Json(payload): Json<ReplyTeamThreadRequest>,
) -> Result<Json<TeamThreadReplyRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    let actor_scope = parse_task_actor_scope(&team.spec, &user)?;
    if root_message_id <= 0 {
        return Err(ApiError::bad_request("root_message_id must be positive"));
    }
    let text = payload.text.trim();
    if text.is_empty() {
        return Err(ApiError::bad_request("text is required"));
    }
    let mention_actor_ids = normalize_thread_reply_mention_actor_ids(
        payload.mention_actor_ids,
        &actor_scope.member_ids,
    );
    let reply = state
        .teams
        .reply_thread(
            &team.id,
            &channel_id,
            root_message_id,
            &canonical_user_actor_id(&user),
            text,
            mention_actor_ids.as_slice(),
        )
        .await
        .map_err(map_reply_thread_error)?;
    let task = state
        .teams
        .get_task(&reply.thread.task_id)
        .await
        .map_err(|err| map_not_found_error(err, "task not found"))?;
    if let Err(err) = maybe_forward_thread_reply_to_mailbox(
        &state,
        &team,
        &task,
        &actor_scope,
        root_message_id,
        &reply.message,
    )
    .await
    {
        tracing::warn!(
            team_id = %team.id,
            task_id = %reply.thread.task_id,
            message_id = reply.message.message_id,
            root_message_id,
            "thread reply mailbox forwarding failed: {:?}",
            err
        );
    }
    Ok(Json(reply))
}

async fn compile_team_task_run_preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id)): Path<(String, String)>,
    Json(payload): Json<CompileTeamTaskRunPreviewRequest>,
) -> Result<Json<TeamTaskRunCompilePreviewResponse>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    ensure_team_execution_ready(&team.spec)?;
    let task = state
        .teams
        .get_task(&task_id)
        .await
        .map_err(|err| map_not_found_error(err, "task not found"))?;
    if task.team_id != team_id {
        return Err(ApiError::not_found("task not found"));
    }
    let conversation = state
        .teams
        .get_task_conversation(&task_id)
        .await
        .map_err(|err| map_not_found_error(err, "conversation not found"))?;
    let messages = state
        .teams
        .list_task_conversation_messages(&task_id, TEAM_TASK_COMPILE_MESSAGE_LIMIT, None)
        .await
        .map_err(map_team_internal_error)?;
    let preview = compile_task_run_preview_response(
        &team.spec,
        &task,
        &conversation,
        &messages,
        payload.context_id.as_deref(),
    )?;
    Ok(Json(preview))
}

async fn create_team_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Json(payload): Json<CreateTeamRunRequest>,
) -> Result<Json<TeamRunRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &team, &user, &["owner", "planner"]).await?;
    ensure_team_execution_ready(&team.spec)?;
    let run = state
        .teams
        .create_run(
            &team.id,
            payload.context_id.as_deref(),
            payload.input.unwrap_or_else(|| serde_json::json!({})),
        )
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(run))
}

async fn list_team_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Query(query): Query<ListTeamRunsQuery>,
) -> Result<Json<Vec<TeamRunRecord>>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let status = normalize_optional_run_status_filter(query.status.as_deref())?;
    let runs = state
        .teams
        .list_runs(&team_id, limit, status.as_deref(), query.before_created_at)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(runs))
}

async fn get_team_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Result<Json<TeamRunRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let run = load_run_for_user(&state, &run_id, &user).await?;
    Ok(Json(run))
}

async fn cancel_team_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Result<Json<TeamRunRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    ensure_run_access_for_user(&state, &run_id, &user).await?;
    let run = state
        .teams
        .cancel_run(&run_id)
        .await
        .map_err(|err| map_not_found_error(err, "run not found"))?;
    Ok(Json(run))
}

async fn resume_team_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Result<Json<TeamRunRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    ensure_run_access_for_user(&state, &run_id, &user).await?;
    let run = state
        .teams
        .resume_run(&run_id)
        .await
        .map_err(map_resume_run_error)?;
    Ok(Json(run))
}

async fn restart_team_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Result<Json<TeamRunRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    ensure_run_access_for_user(&state, &run_id, &user).await?;
    let run = state
        .teams
        .restart_run(&run_id)
        .await
        .map_err(|err| map_not_found_error(err, "run not found"))?;
    Ok(Json(run))
}

async fn get_team_run_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Query(query): Query<TeamRunSnapshotQuery>,
) -> Result<Json<TeamRunSnapshotResponse>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let (run, team) = load_run_and_team_for_user(&state, &run_id, &user).await?;
    validate_team_spec(&team.spec)?;

    let spec_obj = team
        .spec
        .as_object()
        .ok_or_else(|| ApiError::bad_request("spec must be an object"))?;
    let member_specs = parse_member_specs(spec_obj.get("members"))?;
    let coordinator_member_id = parse_spec_coordinator_member_id(spec_obj, &member_specs)?;

    let steps = state
        .teams
        .list_steps(&run_id)
        .await
        .map_err(map_team_internal_error)?;
    let latest_step_by_member = index_latest_steps_by_member(&steps);
    let pending_counts = state
        .teams
        .list_actor_pending_counts_by_actor(&run_id)
        .await
        .map_err(map_team_internal_error)?;
    let status_counts = state
        .teams
        .list_actor_message_status_counts(&run_id)
        .await
        .map_err(map_team_internal_error)?;

    let event_limit = query
        .event_limit
        .unwrap_or(TEAM_PAGE_EVENT_LIMIT)
        .clamp(1, TEAM_PAGE_EVENT_LIMIT);
    let message_limit = query
        .message_limit
        .unwrap_or(TEAM_PAGE_MESSAGE_LIMIT)
        .clamp(1, TEAM_PAGE_MESSAGE_LIMIT);
    let latest_events = state
        .teams
        .list_run_events(&run_id, event_limit, None)
        .await
        .map_err(map_team_internal_error)?;
    let recent_messages = state
        .teams
        .list_actor_messages_for_run(&run_id, message_limit)
        .await
        .map_err(map_team_internal_error)?;
    let reply_obligation_summary = state
        .teams
        .summarize_open_reply_obligations(&run_id)
        .await
        .map_err(map_team_internal_error)?;
    let run_member_overrides = extract_run_member_profile_overrides(&run.input);

    let mut members = Vec::with_capacity(member_specs.len());
    for mut member in member_specs {
        let latest_step = latest_step_by_member
            .get(member.member_id.as_str())
            .cloned();
        let live_session = state
            .teams
            .get_live_member_session(member.member_id.as_str())
            .await
            .map_err(map_team_internal_error)?;
        let (session_id, session_status) = live_session
            .map(|(session_id, status)| (Some(session_id), Some(status)))
            .unwrap_or((None, None));
        let status = latest_step
            .as_ref()
            .map(|step| step_status_to_str(&step.status).to_string())
            .unwrap_or_else(|| "idle".to_string());
        let role = if coordinator_member_id.as_deref() == Some(member.member_id.as_str()) {
            "coordinator"
        } else {
            member.role.as_str()
        };
        let mut prompt = member.prompt.clone();
        if let Some(override_item) = run_member_overrides.get(member.member_id.as_str()) {
            if let Some(prompt_append) = override_item.prompt_append.as_deref() {
                prompt = Some(merge_prompt_append(prompt.as_deref(), Some(prompt_append)));
            }
            if let Some(description) = override_item.description.as_deref() {
                member.description = Some(description.to_string());
            }
        }
        let effective_skills = effective_team_member_skills(role);
        members.push(TeamMemberSnapshot {
            member_id: member.member_id.clone(),
            role: role.to_string(),
            model: member.model.clone(),
            description: member.description.clone(),
            prompt,
            skills: effective_skills,
            pending_inbox_count: pending_counts.get(&member.member_id).copied().unwrap_or(0),
            reply_obligation_count: reply_obligation_summary
                .open_by_actor
                .get(&member.member_id)
                .copied()
                .unwrap_or(0),
            status,
            latest_step,
            session_id,
            session_status,
        });
    }

    let mailbox = TeamMailboxSnapshot {
        pending: status_counts.get("pending").copied().unwrap_or(0),
        delivered: status_counts.get("delivered").copied().unwrap_or(0),
        dead_letter: status_counts.get("dead_letter").copied().unwrap_or(0),
        open_reply_obligation_count: reply_obligation_summary.open_total,
        open_reply_obligations: reply_obligation_summary.open_items.clone(),
        recent_messages,
    };

    let team = sanitize_team_definition_for_response(team);

    Ok(Json(TeamRunSnapshotResponse {
        run,
        team,
        coordinator_member_id,
        members,
        steps,
        latest_events,
        mailbox,
    }))
}

async fn list_team_run_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Query(query): Query<ListTeamRunEventsQuery>,
) -> Result<Json<Vec<TeamRunEventRecord>>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    ensure_run_access_for_user(&state, &run_id, &user).await?;
    let limit = query
        .limit
        .unwrap_or(TEAM_PAGE_EVENT_LIMIT)
        .clamp(1, TEAM_PAGE_EVENT_LIMIT);
    let events = state
        .teams
        .list_run_events(&run_id, limit, query.before_id)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(events))
}

async fn flush_team_run_context(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Json(payload): Json<FlushTeamRunContextRequest>,
) -> Result<Json<FlushTeamRunContextResponse>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    ensure_run_access_for_user(&state, &run_id, &user).await?;
    let member_id = payload.member_id.trim().to_string();
    if member_id.is_empty() {
        return Err(ApiError::bad_request("member_id is required"));
    }
    let trigger = normalize_memory_flush_trigger(payload.trigger.as_deref())?;

    let result = state
        .teams
        .flush_run_context(
            &run_id,
            TeamMemoryFlushRequest {
                member_id,
                session_id: payload.session_id,
                trigger: trigger.to_string(),
                max_events: payload.max_events,
            },
        )
        .await
        .map_err(|err| map_not_found_error(err, "run not found"))?;

    Ok(Json(FlushTeamRunContextResponse {
        status: result.status,
        run_id: result.run_id,
        team_id: result.team_id,
        member_id: result.member_id,
        session_id: result.session_id,
        trigger: result.trigger,
        reason: result.reason,
        artifact_pointer: result.artifact_pointer,
        event_id_from: result.event_id_from,
        event_id_to: result.event_id_to,
        flushed_events: result.flushed_events,
    }))
}

async fn list_team_run_steps(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Result<Json<Vec<TeamStepRecord>>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    ensure_run_access_for_user(&state, &run_id, &user).await?;
    let steps = state
        .teams
        .list_steps(&run_id)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(steps))
}

async fn submit_team_run_step(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Json(payload): Json<SubmitTeamRunStepRequest>,
) -> Result<Json<TeamStepRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    ensure_run_access_for_user(&state, &run_id, &user).await?;

    let step_key = payload.step_key.trim().to_string();
    if step_key.is_empty() {
        return Err(ApiError::bad_request("step_key is required"));
    }
    let member_id = payload.member_id.trim().to_string();
    if member_id.is_empty() {
        return Err(ApiError::bad_request("member_id is required"));
    }
    let depends_on = parse_depends_on_keys(payload.depends_on)?;

    let step = state
        .teams
        .submit_step(&run_id, &step_key, &member_id, depends_on, payload.input)
        .await
        .map_err(map_submit_step_error)?;
    Ok(Json(step))
}

async fn start_team_run_step(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((run_id, step_id)): Path<(String, String)>,
    Json(payload): Json<StartTeamRunStepRequest>,
) -> Result<Json<TeamStepRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    ensure_run_access_for_user(&state, &run_id, &user).await?;
    ensure_step_in_run(&state, &run_id, &step_id).await?;
    let runtime_handle_id = payload
        .runtime_handle_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let step = state
        .teams
        .start_step(&step_id, runtime_handle_id)
        .await
        .map_err(map_team_internal_error)?;
    if let Err(err) =
        crate::team::maybe_nudge_reconcile_step_prompt(&state.teams, &state.agents, &step).await
    {
        tracing::warn!(
            run_id = %run_id,
            step_id = %step.id,
            member_id = %step.member_id,
            "failed to auto-nudge reconcile step prompt: {}",
            err
        );
    }
    Ok(Json(step))
}

async fn complete_team_run_step(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((run_id, step_id)): Path<(String, String)>,
    Json(payload): Json<CompleteTeamRunStepRequest>,
) -> Result<Json<TeamStepRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    ensure_run_access_for_user(&state, &run_id, &user).await?;
    ensure_step_in_run(&state, &run_id, &step_id).await?;
    let step = state
        .teams
        .complete_step(&step_id, payload.output)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(step))
}

async fn fail_team_run_step(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((run_id, step_id)): Path<(String, String)>,
    Json(payload): Json<FailTeamRunStepRequest>,
) -> Result<Json<TeamStepRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    ensure_run_access_for_user(&state, &run_id, &user).await?;
    ensure_step_in_run(&state, &run_id, &step_id).await?;
    let error_text = payload.error_text.trim();
    if error_text.is_empty() {
        return Err(ApiError::bad_request("error_text is required"));
    }
    let step = state
        .teams
        .fail_step(&step_id, error_text)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(step))
}

async fn set_team_run_step_input_required(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((run_id, step_id)): Path<(String, String)>,
    Json(payload): Json<SetTeamRunStepInputRequiredRequest>,
) -> Result<Json<TeamStepRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    ensure_run_access_for_user(&state, &run_id, &user).await?;
    ensure_step_in_run(&state, &run_id, &step_id).await?;
    let reason = normalize_optional_non_empty(payload.reason.as_deref())?.map(str::to_string);
    let step = state
        .teams
        .set_step_input_required(&step_id, reason.as_deref(), payload.input)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(step))
}

async fn resume_team_run_step(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((run_id, step_id)): Path<(String, String)>,
    Json(payload): Json<ResumeTeamRunStepRequest>,
) -> Result<Json<TeamStepRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    ensure_run_access_for_user(&state, &run_id, &user).await?;
    ensure_step_in_run(&state, &run_id, &step_id).await?;
    let step = state
        .teams
        .resume_step(&step_id, payload.input)
        .await
        .map_err(map_team_internal_error)?;
    if let Err(err) =
        crate::team::maybe_nudge_reconcile_step_prompt(&state.teams, &state.agents, &step).await
    {
        tracing::warn!(
            run_id = %run_id,
            step_id = %step.id,
            member_id = %step.member_id,
            "failed to auto-nudge reconcile step prompt: {}",
            err
        );
    }
    Ok(Json(step))
}

async fn send_team_run_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Json(payload): Json<SendTeamRunMessageRequest>,
) -> Result<Json<TeamActorMessageRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let (run, member_ids) = load_run_and_member_ids_for_user(&state, &run_id, &user).await?;
    let SendTeamRunMessageRequest {
        from_actor_id,
        from_peer_id,
        to_actor_id,
        to_peer_id,
        channel,
        transport,
        route,
        payload,
        idempotency_key,
    } = payload;
    let from_actor_id = normalize_required_field(from_actor_id, "from_actor_id")?;
    let to_actor_id = normalize_required_field(to_actor_id, "to_actor_id")?;
    let channel = channel
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("default")
        .to_string();
    let idempotency_key = normalize_optional_idempotency_key(idempotency_key.as_deref())?;
    let transport = parse_message_transport(transport.as_deref())?;
    let from_peer_id =
        normalize_message_peer_id(from_peer_id.as_deref(), "from_peer_id", ACTOR_MAIN_PEER_ID)?;
    let to_peer_id_default = if transport == TeamActorMessageTransport::Remote {
        ACTOR_NODE_PEER_ID
    } else {
        ACTOR_MAIN_PEER_ID
    };
    let to_peer_id =
        normalize_message_peer_id(to_peer_id.as_deref(), "to_peer_id", to_peer_id_default)?;
    validate_message_actors(
        &member_ids,
        &from_actor_id,
        &from_peer_id,
        &to_actor_id,
        &to_peer_id,
        &transport,
        route.as_ref(),
    )?;
    let patch_proposal = parse_profile_patch_proposal(&payload, &from_actor_id)?;
    let message = state
        .teams
        .actor_mailbox_service()
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: from_actor_id.clone(),
            from_peer_id: Some(from_peer_id.clone()),
            to_actor_id: Some(to_actor_id.clone()),
            channel_id: None,
            to_peer_id: Some(to_peer_id.clone()),
            channel: Some(channel),
            transport: Some(transport),
            route,
            payload,
            idempotency_key: idempotency_key.clone(),
            message_kind: None,
        })
        .await
        .map_err(map_actor_service_api_error)?;
    if let Err(err) = maybe_notify_actor_new_mailbox_message_type(&state, &run.id, &message).await {
        tracing::warn!(
            run_id = %run.id,
            to_actor_id = %message.message.to_actor_id,
            message_id = message.message_id,
            "team mailbox type hint notify failed: {}",
            err
        );
    }
    if !message.deduped
        && let Some(proposal) = patch_proposal
    {
        apply_profile_patch_proposal(
            &state,
            &run,
            &member_ids,
            &from_actor_id,
            message.message_id,
            &proposal,
        )
        .await?;
    }
    Ok(Json(message.message))
}

async fn maybe_notify_actor_new_mailbox_message_type(
    state: &AppState,
    run_id: &str,
    send_result: &agenthub_team_actor::ActorSendResponse,
) -> anyhow::Result<()> {
    let Some(plan) = plan_actor_mailbox_immediate_hint(&state.teams, run_id, send_result).await?
    else {
        return Ok(());
    };
    let reason_label = match plan.reason {
        crate::team::ActorMailboxImmediateHintReason::DirectAgentMessage => "direct_agent_message",
        crate::team::ActorMailboxImmediateHintReason::CoordinatorChannelMention => {
            "coordinator_channel_mention"
        }
    };
    let delivery = TeamMailboxRuntimeDeliveryWorker::new(state.teams.clone(), state.agents.clone())
        .enqueue_and_dispatch(run_id, send_result.message_id, &plan)
        .await?;
    append_actor_mailbox_type_hint_event(
        state,
        run_id,
        serde_json::json!({
            "status": if delivery.failed_actor_ids.is_empty() { "sent" } else if delivery.sent_actor_ids.is_empty() { "send_failed" } else { "partial" },
            "message_id": send_result.message_id,
            "delivery_ids": delivery.delivery_ids,
            "reason": reason_label,
            "target_actor_ids": plan.target_actor_ids,
            "sent_actor_ids": delivery.sent_actor_ids,
            "failed_actor_ids": delivery.failed_actor_ids,
        }),
    )
    .await;
    Ok(())
}

async fn append_actor_mailbox_type_hint_event(state: &AppState, run_id: &str, payload: Value) {
    if let Err(err) = state
        .teams
        .append_run_event(run_id, "actor_mailbox_type_hint", payload)
        .await
    {
        tracing::warn!(
            run_id = %run_id,
            "failed to append actor_mailbox_type_hint event: {}",
            err
        );
    }
}

#[cfg(test)]
fn build_actor_mailbox_immediate_hint_prompt_for_test(
    run_id: &str,
    reason: &'static str,
) -> String {
    let reason = if reason == "coordinator_channel_mention" {
        crate::team::ActorMailboxImmediateHintReason::CoordinatorChannelMention
    } else {
        crate::team::ActorMailboxImmediateHintReason::DirectAgentMessage
    };
    crate::team::build_actor_mailbox_immediate_hint_prompt(run_id, reason)
}

async fn list_team_run_inbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Query(query): Query<ListTeamRunInboxQuery>,
) -> Result<Json<Vec<TeamActorMessageRecord>>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let (_run, member_ids) = load_run_and_member_ids_for_user(&state, &run_id, &user).await?;
    let actor_ids =
        resolve_run_mailbox_query_actor_ids(query.actor_id.as_str(), &member_ids, &user)?;
    let limit = query.limit.unwrap_or(500).clamp(1, 1000);
    let states = if query.include_delivered.unwrap_or(false) {
        Some(vec![
            ActorMessageStatus::Pending,
            ActorMessageStatus::Delivered,
            ActorMessageStatus::DeadLetter,
        ])
    } else {
        None
    };
    let service = state.teams.actor_mailbox_service();
    let mut messages = Vec::new();
    for actor_id in actor_ids {
        let actor_messages = service
            .actor_inbox(ActorInboxRequest {
                run_id: run_id.clone(),
                actor_id,
                cursor: query.after_id,
                limit: Some(limit),
                states: states.clone(),
            })
            .await
            .map_err(map_actor_service_api_error)?
            .messages;
        messages.extend(actor_messages);
    }
    messages.sort_by_key(|message| message.message_id);
    messages.dedup_by_key(|message| message.message_id);
    if messages.len() > limit as usize {
        messages.truncate(limit as usize);
    }
    Ok(Json(messages))
}

async fn ack_team_run_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((run_id, message_id)): Path<(String, i64)>,
    Json(payload): Json<AckTeamRunMessageRequest>,
) -> Result<Json<TeamActorMessageRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let (_run, member_ids) = load_run_and_member_ids_for_user(&state, &run_id, &user).await?;
    let actor_ids =
        resolve_run_mailbox_query_actor_ids(payload.actor_id.as_str(), &member_ids, &user)?;
    let service = state.teams.actor_mailbox_service();
    for actor_id in actor_ids {
        let result = service
            .actor_ack(ActorAckRequest {
                run_id: run_id.clone(),
                actor_id,
                message_id,
                ack_token: None,
                result: None,
            })
            .await;
        match result {
            Ok(message) => return Ok(Json(message.message)),
            Err(err) if err.code == ActorServiceErrorCode::NotFound => continue,
            Err(err) => return Err(map_actor_service_api_error(err)),
        }
    }
    Err(ApiError::not_found("message not found"))
}

fn parse_actor_triage_disposition(raw: &str) -> Result<ActorMessageHandlingDisposition, ApiError> {
    match raw.trim() {
        "ignored" => Ok(ActorMessageHandlingDisposition::Ignored),
        "watching" => Ok(ActorMessageHandlingDisposition::Watching),
        "claimed" => Ok(ActorMessageHandlingDisposition::Claimed),
        "completed" => Ok(ActorMessageHandlingDisposition::Completed),
        "released" => Ok(ActorMessageHandlingDisposition::Released),
        _ => Err(ApiError::bad_request(
            "invalid mailbox disposition; expected one of: ignored, watching, claimed, completed, released",
        )),
    }
}

async fn triage_team_run_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((run_id, message_id)): Path<(String, i64)>,
    Json(payload): Json<TriageTeamRunMessageRequest>,
) -> Result<Json<TeamActorMessageRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let (_run, member_ids) = load_run_and_member_ids_for_user(&state, &run_id, &user).await?;
    let actor_ids =
        resolve_run_mailbox_query_actor_ids(payload.actor_id.as_str(), &member_ids, &user)?;
    let disposition = parse_actor_triage_disposition(&payload.disposition)?;
    let service = state.teams.actor_mailbox_service();
    for actor_id in actor_ids {
        let result = service
            .actor_triage(ActorTriageRequest {
                run_id: run_id.clone(),
                actor_id,
                message_id,
                disposition: disposition.clone(),
                reason: payload.reason.clone(),
            })
            .await;
        match result {
            Ok(message) => return Ok(Json(message.message)),
            Err(err) if err.code == ActorServiceErrorCode::NotFound => continue,
            Err(err) => return Err(map_actor_service_api_error(err)),
        }
    }
    Err(ApiError::not_found("message not found"))
}

async fn escalate_team_run_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((run_id, message_id)): Path<(String, i64)>,
    Json(payload): Json<EscalateTeamRunMessageRequest>,
) -> Result<Json<TeamActorMessageRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let (_run, member_ids) = load_run_and_member_ids_for_user(&state, &run_id, &user).await?;
    let actor_ids =
        resolve_run_mailbox_query_actor_ids(payload.actor_id.as_str(), &member_ids, &user)?;
    let service = state.teams.actor_mailbox_service();
    for actor_id in actor_ids {
        let result = service
            .escalate_reply_required_message_to_coordinator(
                &run_id,
                &actor_id,
                ACTOR_MAIN_PEER_ID,
                message_id,
            )
            .await;
        match result {
            Ok(message) => return Ok(Json(message)),
            Err(err) if err.code == ActorServiceErrorCode::NotFound => continue,
            Err(err) => return Err(map_actor_service_api_error(err)),
        }
    }
    Err(ApiError::not_found("message not found"))
}

async fn transfer_team_run_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((run_id, message_id)): Path<(String, i64)>,
    Json(payload): Json<TransferTeamRunMessageRequest>,
) -> Result<Json<TeamActorMessageRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let (_run, member_ids) = load_run_and_member_ids_for_user(&state, &run_id, &user).await?;
    let actor_ids =
        resolve_run_mailbox_query_actor_ids(payload.actor_id.as_str(), &member_ids, &user)?;
    let target_actor_id = payload.target_actor_id.trim();
    if target_actor_id.is_empty() {
        return Err(ApiError::bad_request("target_actor_id is required"));
    }
    let service = state.teams.actor_mailbox_service();
    for actor_id in actor_ids {
        let result = service
            .transfer_reply_required_message(
                &run_id,
                &actor_id,
                ACTOR_MAIN_PEER_ID,
                message_id,
                target_actor_id,
            )
            .await;
        match result {
            Ok(message) => return Ok(Json(message)),
            Err(err) if err.code == ActorServiceErrorCode::NotFound => continue,
            Err(err) => return Err(map_actor_service_api_error(err)),
        }
    }
    Err(ApiError::not_found("message not found"))
}

async fn takeover_team_run_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((run_id, message_id)): Path<(String, i64)>,
    Json(payload): Json<TakeoverTeamRunMessageRequest>,
) -> Result<Json<TeamActorMessageRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let (_run, member_ids) = load_run_and_member_ids_for_user(&state, &run_id, &user).await?;
    let actor_ids =
        resolve_run_mailbox_query_actor_ids(payload.actor_id.as_str(), &member_ids, &user)?;
    let target_actor_id = payload.target_actor_id.trim();
    if target_actor_id.is_empty() {
        return Err(ApiError::bad_request("target_actor_id is required"));
    }
    let service = state.teams.actor_mailbox_service();
    for actor_id in actor_ids {
        let result = service
            .takeover_reply_required_message(
                &run_id,
                &actor_id,
                ACTOR_MAIN_PEER_ID,
                message_id,
                target_actor_id,
            )
            .await;
        match result {
            Ok(message) => return Ok(Json(message)),
            Err(err) if err.code == ActorServiceErrorCode::NotFound => continue,
            Err(err) => return Err(map_actor_service_api_error(err)),
        }
    }
    Err(ApiError::not_found("message not found"))
}

fn parse_profile_patch_proposal(
    payload: &Value,
    default_member_id: &str,
) -> Result<Option<ProfilePatchProposal>, ApiError> {
    let Some(payload_obj) = payload.as_object() else {
        return Ok(None);
    };
    let Some(payload_type) = payload_obj.get("type").and_then(Value::as_str) else {
        return Ok(None);
    };
    if payload_type != "profile_patch_proposal" {
        return Ok(None);
    }

    let target = parse_profile_patch_target(payload_obj.get("target"))?;
    let member_id = payload_obj
        .get("member_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_member_id)
        .to_string();
    let prompt_append = payload_obj
        .get("prompt_append")
        .map(parse_optional_prompt_append)
        .transpose()?
        .flatten();
    let description = payload_obj
        .get("description")
        .map(parse_optional_profile_patch_description)
        .transpose()?
        .flatten();
    if payload_obj.contains_key("skills_add") {
        return Err(ApiError::bad_request(
            "profile_patch_proposal.skills_add is not supported; Team skills are system-managed from role",
        ));
    }
    if prompt_append.is_none() && description.is_none() {
        return Err(ApiError::bad_request(
            "profile_patch_proposal requires prompt_append and/or description",
        ));
    }

    Ok(Some(ProfilePatchProposal {
        target,
        member_id,
        prompt_append,
        description,
    }))
}

fn parse_profile_patch_target(value: Option<&Value>) -> Result<ProfilePatchTarget, ApiError> {
    let raw = value
        .and_then(Value::as_str)
        .map(str::trim)
        .ok_or_else(|| {
            ApiError::bad_request("profile_patch_proposal.target must be 'run' or 'team'")
        })?;
    match raw {
        "run" => Ok(ProfilePatchTarget::Run),
        "team" => Ok(ProfilePatchTarget::Team),
        _ => Err(ApiError::bad_request(
            "profile_patch_proposal.target must be 'run' or 'team'",
        )),
    }
}

fn parse_optional_prompt_append(value: &Value) -> Result<Option<String>, ApiError> {
    if value.is_null() {
        return Ok(None);
    }
    let raw = value.as_str().ok_or_else(|| {
        ApiError::bad_request("profile_patch_proposal.prompt_append must be a non-empty string")
    })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(
            "profile_patch_proposal.prompt_append must be a non-empty string",
        ));
    }
    Ok(Some(trimmed.to_string()))
}

fn parse_optional_profile_patch_description(value: &Value) -> Result<Option<String>, ApiError> {
    if value.is_null() {
        return Ok(None);
    }
    let raw = value.as_str().ok_or_else(|| {
        ApiError::bad_request("profile_patch_proposal.description must be a non-empty string")
    })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(
            "profile_patch_proposal.description must be a non-empty string",
        ));
    }
    Ok(Some(trimmed.to_string()))
}

async fn apply_profile_patch_proposal(
    state: &AppState,
    run: &TeamRunRecord,
    member_ids: &HashSet<String>,
    from_actor_id: &str,
    message_id: i64,
    proposal: &ProfilePatchProposal,
) -> Result<(), ApiError> {
    if !member_ids.contains(proposal.member_id.as_str()) {
        return Err(ApiError::bad_request(
            "profile_patch_proposal.member_id must reference spec.members[].member_id",
        ));
    }

    match proposal.target {
        ProfilePatchTarget::Team => {
            let mut team = state
                .teams
                .get_team(&run.team_id)
                .await
                .map_err(|err| map_not_found_error(err, "team not found"))?;
            let before =
                extract_member_profile_override_from_spec(&team.spec, &proposal.member_id)?;
            apply_profile_patch_to_team_spec(&mut team.spec, proposal)?;
            validate_team_spec(&team.spec)?;
            let after = extract_member_profile_override_from_spec(&team.spec, &proposal.member_id)?;
            let update = state
                .teams
                .update_team_spec_if_unchanged(&team.id, team.updated_at, team.spec)
                .await
                .map_err(map_team_internal_error)?;
            if update.is_none() {
                return Err(ApiError::conflict(
                    "team profile changed concurrently; retry this profile patch",
                ));
            }
            state
                .teams
                .append_run_event(
                    &run.id,
                    "profile_patch_applied",
                    serde_json::json!({
                        "target": proposal.target.as_str(),
                        "member_id": proposal.member_id,
                        "applied_by": from_actor_id,
                        "message_id": message_id,
                        "prompt_append": proposal.prompt_append,
                        "description": proposal.description,
                        "before": {
                            "prompt": before.prompt_append,
                            "description": before.description,
                        },
                        "after": {
                            "prompt": after.prompt_append,
                            "description": after.description,
                        },
                    }),
                )
                .await
                .map_err(map_team_internal_error)?;
        }
        ProfilePatchTarget::Run => {
            let mut run_input = run.input.clone();
            let mut before_overrides = extract_run_member_profile_overrides(&run_input);
            let before = before_overrides
                .remove(&proposal.member_id)
                .unwrap_or_default();
            apply_profile_patch_to_run_input(&mut run_input, proposal)?;
            let mut after_overrides = extract_run_member_profile_overrides(&run_input);
            let after = after_overrides
                .remove(&proposal.member_id)
                .unwrap_or_default();
            state
                .teams
                .update_run_input(&run.id, run_input)
                .await
                .map_err(map_team_internal_error)?;
            state
                .teams
                .append_run_event(
                    &run.id,
                    "profile_patch_applied",
                    serde_json::json!({
                        "target": proposal.target.as_str(),
                        "member_id": proposal.member_id,
                        "applied_by": from_actor_id,
                        "message_id": message_id,
                        "prompt_append": proposal.prompt_append,
                        "description": proposal.description,
                        "before": {
                            "prompt": before.prompt_append,
                            "description": before.description,
                        },
                        "after": {
                            "prompt": after.prompt_append,
                            "description": after.description,
                        },
                    }),
                )
                .await
                .map_err(map_team_internal_error)?;
        }
    }
    Ok(())
}

fn apply_profile_patch_to_team_spec(
    spec: &mut Value,
    proposal: &ProfilePatchProposal,
) -> Result<(), ApiError> {
    let spec_obj = spec
        .as_object_mut()
        .ok_or_else(|| ApiError::bad_request("spec must be an object"))?;
    let members = spec_obj
        .get_mut("members")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| ApiError::bad_request("spec.members must be an array"))?;
    let Some(member_obj) = members.iter_mut().find_map(|member| {
        let member_obj = member.as_object_mut()?;
        let member_id = member_obj.get("member_id").and_then(Value::as_str)?.trim();
        if member_id == proposal.member_id {
            Some(member_obj)
        } else {
            None
        }
    }) else {
        return Err(ApiError::bad_request(
            "profile_patch_proposal.member_id must reference spec.members[].member_id",
        ));
    };

    if let Some(prompt_append) = proposal.prompt_append.as_deref() {
        let merged = merge_prompt_append(
            member_obj.get("prompt").and_then(Value::as_str),
            Some(prompt_append),
        );
        member_obj.insert("prompt".to_string(), Value::String(merged));
    }
    if let Some(description) = proposal.description.as_deref() {
        member_obj.insert(
            "description".to_string(),
            Value::String(description.to_string()),
        );
    }
    Ok(())
}

fn apply_profile_patch_to_run_input(
    run_input: &mut Value,
    proposal: &ProfilePatchProposal,
) -> Result<(), ApiError> {
    let run_obj = run_input.as_object_mut().ok_or_else(|| {
        ApiError::bad_request(
            "run input must be a JSON object for profile_patch_proposal target='run'",
        )
    })?;
    let profile_overrides_value = run_obj
        .entry("profile_overrides".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let profile_overrides_obj = profile_overrides_value.as_object_mut().ok_or_else(|| {
        ApiError::bad_request("run input profile_overrides must be a JSON object")
    })?;
    let members_value = profile_overrides_obj
        .entry("members".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let members_obj = members_value.as_object_mut().ok_or_else(|| {
        ApiError::bad_request("run input profile_overrides.members must be an object")
    })?;
    let member_value = members_obj
        .entry(proposal.member_id.clone())
        .or_insert_with(|| serde_json::json!({}));
    let member_obj = member_value.as_object_mut().ok_or_else(|| {
        ApiError::bad_request("run input profile_overrides.members entries must be objects")
    })?;

    if let Some(prompt_append) = proposal.prompt_append.as_deref() {
        let merged = merge_prompt_append(
            member_obj.get("prompt_append").and_then(Value::as_str),
            Some(prompt_append),
        );
        member_obj.insert("prompt_append".to_string(), Value::String(merged));
    }
    if let Some(description) = proposal.description.as_deref() {
        member_obj.insert(
            "description".to_string(),
            Value::String(description.to_string()),
        );
    }

    Ok(())
}

fn merge_prompt_append(existing: Option<&str>, incoming: Option<&str>) -> String {
    let existing = existing.unwrap_or("").trim();
    let incoming = incoming.unwrap_or("").trim();
    if existing.is_empty() {
        return incoming.to_string();
    }
    if incoming.is_empty() {
        return existing.to_string();
    }
    format!("{existing}\n\n{incoming}")
}

fn extract_member_profile_override_from_spec(
    spec: &Value,
    member_id: &str,
) -> Result<MemberProfileOverride, ApiError> {
    let spec_obj = spec
        .as_object()
        .ok_or_else(|| ApiError::bad_request("spec must be an object"))?;
    let members = spec_obj
        .get("members")
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::bad_request("spec.members must be an array"))?;
    let Some(member_obj) = members.iter().find_map(|member| {
        let member_obj = member.as_object()?;
        let id = member_obj.get("member_id").and_then(Value::as_str)?.trim();
        if id == member_id {
            Some(member_obj)
        } else {
            None
        }
    }) else {
        return Ok(MemberProfileOverride::default());
    };
    Ok(MemberProfileOverride {
        prompt_append: member_obj
            .get("prompt")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        description: member_obj
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    })
}

fn extract_run_member_profile_overrides(input: &Value) -> HashMap<String, MemberProfileOverride> {
    let Some(input_obj) = input.as_object() else {
        return HashMap::new();
    };
    let Some(profile_overrides_obj) = input_obj
        .get("profile_overrides")
        .and_then(Value::as_object)
    else {
        return HashMap::new();
    };
    let Some(members_obj) = profile_overrides_obj
        .get("members")
        .and_then(Value::as_object)
    else {
        return HashMap::new();
    };

    let mut out = HashMap::with_capacity(members_obj.len());
    for (member_id, member_value) in members_obj {
        let Some(member_obj) = member_value.as_object() else {
            continue;
        };
        let prompt_append = member_obj
            .get("prompt_append")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let description = member_obj
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if prompt_append.is_none() && description.is_none() {
            continue;
        }
        out.insert(
            member_id.clone(),
            MemberProfileOverride {
                prompt_append,
                description,
            },
        );
    }
    out
}

async fn ensure_run_access_for_user(
    state: &AppState,
    run_id: &str,
    user: &UserRecord,
) -> Result<(), ApiError> {
    load_run_for_user(state, run_id, user).await?;
    Ok(())
}

async fn ensure_step_in_run(state: &AppState, run_id: &str, step_id: &str) -> Result<(), ApiError> {
    let step = state
        .teams
        .get_step(step_id)
        .await
        .map_err(|err| map_not_found_error(err, "step not found"))?;
    if step.run_id != run_id {
        return Err(ApiError::not_found("step not found"));
    }
    Ok(())
}

fn parse_depends_on_keys(depends_on: Option<Vec<String>>) -> Result<Vec<String>, ApiError> {
    let depends_on = depends_on.unwrap_or_default();
    let mut seen = HashSet::with_capacity(depends_on.len());
    let mut out = Vec::with_capacity(depends_on.len());
    for dep in depends_on {
        let dep = dep.trim();
        if dep.is_empty() {
            return Err(ApiError::bad_request(
                "depends_on entries must be non-empty strings",
            ));
        }
        if !seen.insert(dep.to_string()) {
            return Err(ApiError::bad_request(
                "depends_on must not contain duplicates",
            ));
        }
        out.push(dep.to_string());
    }
    Ok(out)
}

fn normalize_required_field(value: String, field: &str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::bad_request(&format!("{field} is required")));
    }
    Ok(value.to_string())
}

fn normalize_optional_non_empty(value: Option<&str>) -> Result<Option<&str>, ApiError> {
    match value {
        None => Ok(None),
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(ApiError::bad_request("reason must be a non-empty string"));
            }
            Ok(Some(trimmed))
        }
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn message_archive_source_kind_values() -> [&'static str; 5] {
    [
        MessageDocumentKind::AgentEvent.as_str(),
        MessageDocumentKind::TeamConversationMessage.as_str(),
        MessageDocumentKind::TeamRunEvent.as_str(),
        MessageDocumentKind::TeamActorMessage.as_str(),
        MessageDocumentKind::AggregatedAcpMessage.as_str(),
    ]
}

fn parse_message_archive_source_kind(raw: &str) -> Result<MessageDocumentKind, ApiError> {
    match raw.trim() {
        "agent_event" => Ok(MessageDocumentKind::AgentEvent),
        "team_conversation_message" => Ok(MessageDocumentKind::TeamConversationMessage),
        "team_run_event" => Ok(MessageDocumentKind::TeamRunEvent),
        "team_actor_message" => Ok(MessageDocumentKind::TeamActorMessage),
        "aggregated_acp_message" => Ok(MessageDocumentKind::AggregatedAcpMessage),
        invalid => {
            let message = format!(
                "unsupported message archive source_kind '{invalid}'; expected one of: {}",
                message_archive_source_kind_values().join(", ")
            );
            Err(ApiError::bad_request(&message))
        }
    }
}

fn normalize_optional_idempotency_key(value: Option<&str>) -> Result<Option<String>, ApiError> {
    let normalized = normalize_optional_idempotency_key_input(value);
    if value.is_some() && normalized.is_none() {
        return Err(ApiError::bad_request(
            "idempotency_key must be a non-empty string",
        ));
    }
    let Some(idempotency_key) = normalized else {
        return Ok(None);
    };
    if idempotency_key.len() > 128 {
        return Err(ApiError::bad_request(
            "idempotency_key must be at most 128 characters",
        ));
    }
    Ok(Some(idempotency_key))
}

impl From<MessageSearchHit> for TeamMessageSearchHitResponse {
    fn from(hit: MessageSearchHit) -> Self {
        Self {
            document_id: hit.document_id,
            source_kind: hit.source_kind,
            body_text: hit.body_text,
            score: hit.score,
            authority_message_id: hit.authority_message_id,
            correlation_id: hit.correlation_id,
            group_id: hit.group_id,
            team_id: hit.team_id,
            run_id: hit.run_id,
            conversation_id: hit.conversation_id,
            task_id: hit.task_id,
            agent_id: hit.agent_id,
            session_id: hit.session_id,
        }
    }
}

fn normalize_optional_run_status_filter(value: Option<&str>) -> Result<Option<String>, ApiError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if TEAM_RUN_STATUS_VALUES.contains(&trimmed) {
        return Ok(Some(trimmed.to_string()));
    }
    Err(ApiError::bad_request(&format!(
        "status must be one of: {}",
        TEAM_RUN_STATUS_VALUES.join(", ")
    )))
}

fn normalize_memory_flush_trigger(value: Option<&str>) -> Result<&'static str, ApiError> {
    match value
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
    {
        None => Ok("manual"),
        Some("manual") => Ok("manual"),
        Some("soft_threshold") => Ok("soft_threshold"),
        Some("hard_error") => Ok("hard_error"),
        Some(_) => Err(ApiError::bad_request(&format!(
            "trigger must be one of: {}",
            TEAM_MEMORY_FLUSH_TRIGGER_VALUES.join(", ")
        ))),
    }
}

#[cfg(test)]
fn normalize_conversation_mode(value: Option<&str>) -> Result<String, ApiError> {
    normalize_enum_value(
        value,
        "conversation_mode",
        &TEAM_CONVERSATION_MODE_VALUES,
        "group_chat",
    )
}

fn normalize_conversation_route(value: Option<&str>) -> Result<String, ApiError> {
    normalize_enum_value(
        value,
        "route",
        &TEAM_CONVERSATION_ROUTE_VALUES,
        "group_chat",
    )
}

fn infer_task_message_route(
    actor_scope: &TaskActorScope,
    requested_route: Option<&str>,
    to_actor_id: Option<&str>,
    payload: &Value,
) -> Result<String, ApiError> {
    if requested_route.is_some() {
        return normalize_conversation_route(requested_route);
    }
    if let Some(to_actor_id) = to_actor_id.map(str::trim).filter(|value| !value.is_empty()) {
        if actor_scope.coordinator_member_id.as_deref() == Some(to_actor_id) {
            return Ok("to_coordinator".to_string());
        }
        return Ok("to_member".to_string());
    }
    let Some(target_actor_id) = infer_single_task_message_target(actor_scope, payload) else {
        return Ok("group_chat".to_string());
    };
    if actor_scope.coordinator_member_id.as_deref() == Some(target_actor_id.as_str()) {
        return Ok("to_coordinator".to_string());
    }
    Ok("to_member".to_string())
}

fn canonical_user_actor_id(user: &UserRecord) -> String {
    format!("{TEAM_SPECIAL_USER_ACTOR_PREFIX}{}", user.id)
}

fn normalize_task_actor_id(
    value: &str,
    field_name: &str,
    user: &UserRecord,
) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(&format!("{field_name} is required")));
    }
    if trimmed == TEAM_SPECIAL_USER_ACTOR_ALIAS {
        return Ok(canonical_user_actor_id(user));
    }
    if let Some(user_id) = trimmed.strip_prefix(TEAM_SPECIAL_USER_ACTOR_PREFIX) {
        let user_id = user_id.trim();
        if user_id.is_empty() {
            return Err(ApiError::bad_request(&format!(
                "{field_name} user actor id must be non-empty"
            )));
        }
        if user_id != user.id {
            return Err(ApiError::bad_request(&format!(
                "{field_name} user actor id must match authenticated user"
            )));
        }
        return Ok(canonical_user_actor_id(user));
    }
    Ok(trimmed.to_string())
}

fn normalize_task_created_by_actor_id(
    value: Option<&str>,
    user: &UserRecord,
) -> Result<String, ApiError> {
    let Some(raw) = value else {
        return Ok(canonical_user_actor_id(user));
    };
    normalize_task_actor_id(raw, "created_by_actor_id", user)
}

fn normalize_task_priority(value: Option<&str>) -> Result<TeamTaskPriority, ApiError> {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(TeamTaskPriority::default());
    };
    raw.parse::<TeamTaskPriority>().map_err(|_| {
        ApiError::bad_request("invalid task priority; expected one of: critical, high, medium, low")
    })
}

fn summarize_task_source_message(payload: &Value) -> String {
    let raw = payload
        .get("text")
        .and_then(Value::as_str)
        .or_else(|| payload.as_str())
        .unwrap_or("")
        .trim();
    if raw.is_empty() {
        return String::new();
    }
    summarize_text(raw, TEAM_MESSAGE_SUMMARY_MAX_CHARS)
}

fn summarize_text(raw: &str, max_chars: usize) -> String {
    let normalized = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut summary = normalized
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    summary.push_str("...");
    summary
}

struct TaskMessageCorrelationSeed<'a> {
    task_id: &'a str,
    from_actor_id: &'a str,
    to_actor_id: Option<&'a str>,
    route: &'a str,
    idempotency_key: Option<&'a str>,
}

fn hex_encode(data: &[u8]) -> String {
    const HEX_CHARS: &[u8] = b"0123456789abcdef";
    let mut result = String::with_capacity(data.len() * 2);
    for byte in data {
        result.push(HEX_CHARS[(byte >> 4) as usize] as char);
        result.push(HEX_CHARS[(byte & 0xf) as usize] as char);
    }
    result
}

fn push_task_message_correlation_component(hasher: &mut Sha256, value: &str) {
    hasher.update(value.as_bytes());
    hasher.update([0_u8]);
}

fn derive_task_message_correlation_id(
    payload_obj: &Map<String, Value>,
    seed: &TaskMessageCorrelationSeed<'_>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update("task-message-correlation:v1");
    push_task_message_correlation_component(&mut hasher, seed.task_id);
    push_task_message_correlation_component(&mut hasher, seed.from_actor_id);
    push_task_message_correlation_component(&mut hasher, seed.to_actor_id.unwrap_or(""));
    push_task_message_correlation_component(&mut hasher, seed.route);
    push_task_message_correlation_component(&mut hasher, seed.idempotency_key.unwrap_or(""));
    push_task_message_correlation_component(
        &mut hasher,
        canonical_json(&Value::Object(payload_obj.clone())).as_str(),
    );
    format!("taskmsg:v1:{}", hex_encode(&hasher.finalize()))
}

fn ensure_task_message_correlation_id(
    payload: Value,
    seed: Option<TaskMessageCorrelationSeed<'_>>,
) -> Value {
    let Value::Object(mut payload_obj) = payload else {
        return payload;
    };
    let existing = payload_obj
        .get("correlation_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let correlation_id = existing.unwrap_or_else(|| {
        seed.as_ref()
            .and_then(|seed| {
                seed.idempotency_key
                    .map(|_| derive_task_message_correlation_id(&payload_obj, seed))
            })
            .unwrap_or_else(|| Uuid::now_v7().to_string())
    });
    payload_obj.insert("correlation_id".to_string(), Value::String(correlation_id));
    Value::Object(payload_obj)
}

fn normalize_task_message_payload(payload: Value) -> Value {
    let Value::Object(mut payload_obj) = payload else {
        return payload;
    };
    normalize_task_message_detail_ref(&mut payload_obj);
    ensure_task_message_summary(&mut payload_obj);
    Value::Object(payload_obj)
}

fn normalize_task_message_detail_ref(payload_obj: &mut Map<String, Value>) {
    let Some(detail_ref_value) = payload_obj.get("detail_ref").cloned() else {
        return;
    };
    let normalized = match detail_ref_value {
        Value::String(uri) => normalize_task_message_detail_ref_object(&uri, None, None, None),
        Value::Object(detail_ref_obj) => normalize_task_message_detail_ref_object(
            detail_ref_obj
                .get("uri")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            detail_ref_obj.get("label").and_then(Value::as_str),
            detail_ref_obj.get("kind").and_then(Value::as_str),
            detail_ref_obj.get("content_type").and_then(Value::as_str),
        ),
        _ => None,
    };
    match normalized {
        Some(value) => {
            payload_obj.insert("detail_ref".to_string(), value);
        }
        None => {
            payload_obj.remove("detail_ref");
        }
    }
}

fn normalize_task_message_detail_ref_object(
    uri: &str,
    label: Option<&str>,
    kind: Option<&str>,
    content_type: Option<&str>,
) -> Option<Value> {
    let trimmed_uri = uri.trim();
    if trimmed_uri.is_empty() {
        return None;
    }
    let mut detail_ref = Map::new();
    detail_ref.insert("uri".to_string(), Value::String(trimmed_uri.to_string()));
    if let Some(label) = label.map(str::trim).filter(|value| !value.is_empty()) {
        detail_ref.insert("label".to_string(), Value::String(label.to_string()));
    }
    if let Some(kind) = kind.map(str::trim).filter(|value| !value.is_empty()) {
        detail_ref.insert("kind".to_string(), Value::String(kind.to_string()));
    }
    if let Some(content_type) = content_type
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        detail_ref.insert(
            "content_type".to_string(),
            Value::String(content_type.to_string()),
        );
    }
    Some(Value::Object(detail_ref))
}

fn ensure_task_message_summary(payload_obj: &mut Map<String, Value>) {
    if !payload_obj.contains_key("detail_ref") {
        return;
    }
    if payload_obj
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
    {
        return;
    }
    let summary_source = payload_obj
        .get("text")
        .and_then(Value::as_str)
        .or_else(|| payload_obj.get("result").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(summary_source) = summary_source else {
        return;
    };
    payload_obj.insert(
        "summary".to_string(),
        Value::String(truncate_task_message_summary(summary_source)),
    );
}

fn truncate_task_message_summary(value: &str) -> String {
    if value.chars().count() <= TEAM_MESSAGE_SUMMARY_MAX_CHARS {
        return value.to_string();
    }
    const ELLIPSIS: &str = "...";
    let prefix_len = TEAM_MESSAGE_SUMMARY_MAX_CHARS.saturating_sub(ELLIPSIS.chars().count());
    let mut out = String::with_capacity(TEAM_MESSAGE_SUMMARY_MAX_CHARS);
    for ch in value.chars().take(prefix_len) {
        out.push(ch);
    }
    out.push_str(ELLIPSIS);
    out
}

#[derive(Debug)]
struct TaskActorScope {
    user_actor_id: String,
    member_ids: HashSet<String>,
    member_order: Vec<String>,
    coordinator_member_id: Option<String>,
}

fn parse_task_actor_scope(
    team_spec: &Value,
    user: &UserRecord,
) -> Result<TaskActorScope, ApiError> {
    let spec_obj = team_spec
        .as_object()
        .ok_or_else(|| ApiError::bad_request("spec must be an object"))?;
    let member_specs = parse_member_specs(spec_obj.get("members"))?;
    let member_order = member_specs
        .iter()
        .map(|member| member.member_id.clone())
        .collect::<Vec<_>>();
    let member_ids = member_specs
        .iter()
        .map(|member| member.member_id.clone())
        .collect::<HashSet<_>>();
    let coordinator_member_id = parse_spec_coordinator_member_id(spec_obj, &member_specs)?;
    Ok(TaskActorScope {
        user_actor_id: canonical_user_actor_id(user),
        member_ids,
        member_order,
        coordinator_member_id,
    })
}

fn validate_task_message_sender(
    actor_scope: &TaskActorScope,
    from_actor_id: &str,
) -> Result<(), ApiError> {
    if from_actor_id == actor_scope.user_actor_id || actor_scope.member_ids.contains(from_actor_id)
    {
        return Ok(());
    }
    Err(ApiError::bad_request(
        "from_actor_id must be authenticated user actor or spec.members[].member_id",
    ))
}

fn resolve_task_message_target(
    actor_scope: &TaskActorScope,
    route: &str,
    to_actor_id: Option<String>,
    payload: &Value,
) -> Result<Option<String>, ApiError> {
    match route {
        "group_chat" => {
            if to_actor_id.is_some() {
                return Err(ApiError::bad_request(
                    "to_actor_id must be omitted when route=group_chat",
                ));
            }
            Ok(None)
        }
        "to_member" => {
            let to_actor_id = to_actor_id
                .or_else(|| infer_single_task_message_target(actor_scope, payload))
                .ok_or_else(|| {
                    ApiError::bad_request(
                        "to_actor_id is required when route=to_member or payload must mention exactly one member",
                    )
                })?;
            if !actor_scope.member_ids.contains(to_actor_id.as_str()) {
                return Err(ApiError::bad_request(
                    "to_actor_id must reference spec.members[].member_id when route=to_member",
                ));
            }
            Ok(Some(to_actor_id))
        }
        "to_coordinator" => {
            let coordinator_member_id =
                actor_scope
                    .coordinator_member_id
                    .as_deref()
                    .ok_or_else(|| {
                        ApiError::bad_request(
                            "route=to_coordinator requires a coordinator member in spec.members",
                        )
                    })?;
            match to_actor_id {
                None => Ok(Some(coordinator_member_id.to_string())),
                Some(to_actor_id) => {
                    if to_actor_id != coordinator_member_id {
                        return Err(ApiError::bad_request(
                            "to_actor_id must equal coordinator member_id when route=to_coordinator",
                        ));
                    }
                    Ok(Some(to_actor_id))
                }
            }
        }
        _ => Err(ApiError::bad_request("unsupported route")),
    }
}

fn infer_single_task_message_target(
    actor_scope: &TaskActorScope,
    payload: &Value,
) -> Option<String> {
    let mention_actor_ids =
        extract_task_message_mention_actor_ids(payload, &actor_scope.member_ids);
    if mention_actor_ids.len() == 1 {
        return mention_actor_ids.first().cloned();
    }
    None
}

async fn maybe_forward_task_message_to_mailbox(
    state: &AppState,
    team: &TeamDefinitionRecord,
    task: &TeamTaskRecord,
    actor_scope: &TaskActorScope,
    message: &TeamConversationMessageRecord,
) -> Result<(), ApiError> {
    let Some(mailbox_sender) = resolve_task_mailbox_sender(actor_scope) else {
        return Ok(());
    };
    let mention_ids =
        extract_task_message_mention_actor_ids(&message.payload, &actor_scope.member_ids);
    let (recipient_ids, delivery_scope) = resolve_task_mailbox_recipient_ids(
        actor_scope,
        message.route.as_str(),
        message.to_actor_id.as_deref(),
    );
    if recipient_ids.is_empty() {
        return Ok(());
    }
    let Some(run) =
        resolve_task_message_mailbox_run(state, team, task, &message.conversation_id).await?
    else {
        return Ok(());
    };
    let forwarded_payload = build_task_mailbox_forward_payload(
        &message.payload,
        message,
        mention_ids.as_slice(),
        delivery_scope,
    );
    forward_mailbox_payload_to_actor_ids(
        state,
        &run,
        mailbox_sender.as_str(),
        recipient_ids,
        forwarded_payload,
        format!("task:{}:{}", message.task_id, message.message_id),
    )
    .await
}

async fn maybe_forward_thread_reply_to_mailbox(
    state: &AppState,
    team: &TeamDefinitionRecord,
    task: &TeamTaskRecord,
    actor_scope: &TaskActorScope,
    root_message_id: i64,
    message: &TeamConversationMessageRecord,
) -> Result<(), ApiError> {
    let Some(mailbox_sender) = resolve_task_mailbox_sender(actor_scope) else {
        return Ok(());
    };
    let recipient_ids = collect_thread_participant_actor_ids(
        state,
        task,
        actor_scope,
        root_message_id,
        mailbox_sender.as_str(),
        message,
    )
    .await?;
    if recipient_ids.is_empty() {
        return Ok(());
    }
    let Some(run) =
        resolve_task_message_mailbox_run(state, team, task, &message.conversation_id).await?
    else {
        return Ok(());
    };
    let mention_ids =
        extract_task_message_mention_actor_ids(&message.payload, &actor_scope.member_ids);
    let forwarded_payload = build_task_mailbox_forward_payload(
        &message.payload,
        message,
        mention_ids.as_slice(),
        "thread_participants",
    );
    forward_mailbox_payload_to_actor_ids(
        state,
        &run,
        mailbox_sender.as_str(),
        recipient_ids,
        forwarded_payload,
        format!("thread:{}:{}", message.task_id, message.message_id),
    )
    .await
}

async fn collect_thread_participant_actor_ids(
    state: &AppState,
    task: &TeamTaskRecord,
    actor_scope: &TaskActorScope,
    root_message_id: i64,
    mailbox_sender: &str,
    message: &TeamConversationMessageRecord,
) -> Result<Vec<String>, ApiError> {
    let messages =
        query_thread_participant_messages(state, task.id.as_str(), root_message_id).await?;
    let mut participant_ids = Vec::new();
    let mut seen = HashSet::new();
    seen.insert(mailbox_sender.to_string());
    for candidate_message in messages {
        push_member_mention(
            candidate_message.from_actor_id.as_str(),
            &actor_scope.member_ids,
            &mut seen,
            &mut participant_ids,
        );
        for actor_id in extract_task_message_mention_actor_ids(
            &candidate_message.payload,
            &actor_scope.member_ids,
        ) {
            push_member_mention(
                actor_id.as_str(),
                &actor_scope.member_ids,
                &mut seen,
                &mut participant_ids,
            );
        }
    }
    for actor_id in
        extract_task_message_mention_actor_ids(&message.payload, &actor_scope.member_ids)
    {
        push_member_mention(
            actor_id.as_str(),
            &actor_scope.member_ids,
            &mut seen,
            &mut participant_ids,
        );
    }
    Ok(participant_ids)
}

async fn query_thread_participant_messages(
    state: &AppState,
    task_id: &str,
    root_message_id: i64,
) -> Result<Vec<TeamConversationMessageRecord>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT
            id,
            conversation_id,
            task_id,
            from_actor_id,
            to_actor_id,
            route,
            payload_json,
            created_at
        FROM team_conversation_messages
        WHERE id = ?2
        UNION ALL
        SELECT
            id,
            conversation_id,
            task_id,
            from_actor_id,
            to_actor_id,
            route,
            payload_json,
            created_at
        FROM team_conversation_messages
        WHERE task_id = ?1
          AND route = 'team_thread_reply'
          AND thread_root_message_id = ?2
        ORDER BY id ASC
        "#,
    )
    .bind(task_id)
    .bind(root_message_id)
    .fetch_all(&state.db)
    .await
    .map_err(|err| map_team_internal_error(err.into()))?;
    rows.iter()
        .map(parse_thread_participant_message_row)
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_team_internal_error)
}

fn parse_thread_participant_message_row(
    row: &sqlx::sqlite::SqliteRow,
) -> anyhow::Result<TeamConversationMessageRecord> {
    let payload_json: String = row.get("payload_json");
    let payload: Value = serde_json::from_str(&payload_json)?;
    Ok(TeamConversationMessageRecord {
        message_id: row.get("id"),
        conversation_id: row.get("conversation_id"),
        task_id: row.get("task_id"),
        group_id: None,
        from_actor_id: row.get("from_actor_id"),
        to_actor_id: row.get::<Option<String>, _>("to_actor_id"),
        route: row.get("route"),
        payload,
        created_at: row.get("created_at"),
    })
}

fn normalize_thread_reply_mention_actor_ids(
    mention_actor_ids: Vec<String>,
    member_ids: &HashSet<String>,
) -> Vec<String> {
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for actor_id in mention_actor_ids {
        push_member_mention(actor_id.as_str(), member_ids, &mut seen, &mut normalized);
    }
    normalized
}

async fn forward_mailbox_payload_to_actor_ids(
    state: &AppState,
    run: &TeamRunRecord,
    from_actor_id: &str,
    recipient_ids: Vec<String>,
    forwarded_payload: Value,
    idempotency_prefix: String,
) -> Result<(), ApiError> {
    let actor_mailbox_service = state.teams.actor_mailbox_service();
    let recipient_deliveries = state
        .teams
        .resolve_mailbox_recipient_deliveries(&recipient_ids)
        .await
        .map_err(map_team_internal_error)?;
    let run_id = run.id.clone();
    let sender = from_actor_id.to_string();
    try_join_all(recipient_deliveries.into_iter().map(|delivery| {
        let run_id = run_id.clone();
        let sender = sender.clone();
        let payload = forwarded_payload.clone();
        let actor_mailbox_service = actor_mailbox_service.clone();
        let idempotency_key = format!("{idempotency_prefix}:{}", delivery.actor_id);
        async move {
            let normalized_payload = normalize_actor_message_envelope_payload(
                TEAM_SPECIAL_USER_ACTOR_ALIAS,
                delivery.actor_id.as_str(),
                &ActorMessageKind::HumanRequest,
                payload,
            );
            let send_result = actor_mailbox_service
                .actor_send(ActorSendRequest {
                    run_id: run_id.clone(),
                    from_actor_id: sender,
                    from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
                    to_actor_id: Some(delivery.actor_id.clone()),
                    channel_id: None,
                    to_peer_id: Some(delivery.to_peer_id.clone()),
                    channel: Some("default".to_string()),
                    transport: Some(delivery.transport),
                    route: delivery.route,
                    payload: normalized_payload,
                    idempotency_key: Some(idempotency_key),
                    message_kind: None,
                })
                .await
                .map_err(map_actor_service_api_error)?;
            if let Err(err) =
                maybe_notify_actor_new_mailbox_message_type(state, &run_id, &send_result).await
            {
                tracing::warn!(
                    run_id = %run_id,
                    to_actor_id = %delivery.actor_id,
                    message_id = send_result.message_id,
                    "mailbox type hint notify failed: {}",
                    err
                );
            }
            Ok::<(), ApiError>(())
        }
    }))
    .await?;
    Ok(())
}

async fn load_latest_active_run_for_team(
    state: &AppState,
    team_id: &str,
) -> Result<Option<TeamRunRecord>, ApiError> {
    let runs = state
        .teams
        .list_runs(team_id, 50, None, None)
        .await
        .map_err(map_team_internal_error)?;
    Ok(runs
        .into_iter()
        .find(|run| is_team_run_status_active(&run.status)))
}

async fn resolve_task_message_mailbox_run(
    state: &AppState,
    team: &TeamDefinitionRecord,
    task: &TeamTaskRecord,
    conversation_id: &str,
) -> Result<Option<TeamRunRecord>, ApiError> {
    if let Some(run) = load_latest_active_run_for_team(state, &team.id).await? {
        return Ok(Some(run));
    }
    if !is_shared_thread_task(task) {
        return Ok(None);
    }
    let run = state
        .teams
        .ensure_shared_thread_mailbox_run(&team.id, &task.id, conversation_id)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Some(run))
}

fn is_shared_thread_task(task: &TeamTaskRecord) -> bool {
    if task.title.trim().eq_ignore_ascii_case("all") {
        return true;
    }
    task.context
        .as_object()
        .and_then(|obj| obj.get("bootstrap_kind"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| value.eq_ignore_ascii_case(TEAM_SHARED_THREAD_BOOTSTRAP_KIND))
}

fn is_team_run_status_active(status: &TeamRunStatus) -> bool {
    matches!(
        status,
        TeamRunStatus::Submitted | TeamRunStatus::Working | TeamRunStatus::InputRequired
    )
}

fn resolve_task_mailbox_sender(actor_scope: &TaskActorScope) -> Option<String> {
    let coordinator_member_id = actor_scope
        .coordinator_member_id
        .as_deref()
        .filter(|member_id| actor_scope.member_ids.contains(*member_id))
        .map(str::to_string);
    if coordinator_member_id.is_some() {
        return coordinator_member_id;
    }
    actor_scope.member_order.first().cloned()
}

fn resolve_task_mailbox_recipient_ids(
    actor_scope: &TaskActorScope,
    route: &str,
    to_actor_id: Option<&str>,
) -> (Vec<String>, &'static str) {
    match route {
        "group_chat" => {
            let Some(sender) = resolve_task_mailbox_sender(actor_scope) else {
                return (Vec::new(), "broadcast");
            };
            let mut recipient_ids = Vec::new();
            recipient_ids.push(sender.clone());
            for member_id in &actor_scope.member_order {
                if member_id != &sender {
                    recipient_ids.push(member_id.clone());
                }
            }
            (recipient_ids, "broadcast")
        }
        "to_member" | "to_coordinator" => to_actor_id
            .map(|target| (vec![target.to_string()], "direct"))
            .unwrap_or_else(|| (Vec::new(), "direct")),
        _ => (Vec::new(), "broadcast"),
    }
}

fn extract_task_message_mention_actor_ids(
    payload: &Value,
    member_ids: &HashSet<String>,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for key in ["mention_actor_ids", "mentioned_actor_ids"] {
        if let Some(explicit_mentions) = payload.get(key).and_then(Value::as_array) {
            for value in explicit_mentions {
                if let Some(candidate) = value.as_str() {
                    push_member_mention(candidate, member_ids, &mut seen, &mut out);
                }
            }
        }
    }
    if let Some(text) = payload.get("text").and_then(Value::as_str) {
        for candidate in extract_mentions_from_text(text) {
            push_member_mention(candidate.as_str(), member_ids, &mut seen, &mut out);
        }
    }
    out
}

fn push_member_mention(
    raw_candidate: &str,
    member_ids: &HashSet<String>,
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
) {
    let candidate = raw_candidate.trim();
    if candidate.is_empty()
        || !member_ids.contains(candidate)
        || !seen.insert(candidate.to_string())
    {
        return;
    }
    out.push(candidate.to_string());
}

fn extract_mentions_from_text(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut cursor = 0usize;
    while let Some(open_index) = text[cursor..].find("<at>") {
        let mention_start = cursor + open_index + 4;
        let Some(close_index) = text[mention_start..].find("</at>") else {
            break;
        };
        let mention_end = mention_start + close_index;
        let candidate = text[mention_start..mention_end].trim();
        if !candidate.is_empty()
            && candidate
                .as_bytes()
                .iter()
                .all(|raw| is_valid_mention_char(*raw))
        {
            out.push(candidate.to_string());
        }
        cursor = mention_end + 5;
    }
    let mut raw_cursor = 0usize;
    while raw_cursor < bytes.len() {
        if bytes[raw_cursor] != b'@'
            || (raw_cursor > 0 && is_email_local_char(bytes[raw_cursor - 1]))
        {
            raw_cursor += 1;
            continue;
        }
        let start = raw_cursor + 1;
        let mut end = start;
        while end < bytes.len() && is_valid_mention_char(bytes[end]) {
            end += 1;
        }
        if end > start {
            out.push(text[start..end].to_string());
        }
        raw_cursor = end;
    }
    out
}

fn is_valid_mention_char(raw: u8) -> bool {
    matches!(raw, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b':' | b'-')
}

fn is_email_local_char(raw: u8) -> bool {
    matches!(
        raw,
        b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'%' | b'+' | b'-'
    )
}

fn build_task_mailbox_forward_payload(
    source_payload: &Value,
    message: &TeamConversationMessageRecord,
    mention_actor_ids: &[String],
    delivery_scope: &str,
) -> Value {
    let mut payload_obj = match source_payload {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    let mention_values = mention_actor_ids
        .iter()
        .cloned()
        .map(Value::String)
        .collect::<Vec<_>>();
    payload_obj.insert(
        "mention_actor_ids".to_string(),
        Value::Array(mention_values.clone()),
    );
    payload_obj.insert(
        "mentioned_actor_ids".to_string(),
        Value::Array(mention_values),
    );
    payload_obj.insert(
        "human_actor_id".to_string(),
        Value::String(TEAM_SPECIAL_USER_ACTOR_ALIAS.to_string()),
    );
    payload_obj.insert(
        "delivery_scope".to_string(),
        Value::String(delivery_scope.to_string()),
    );
    payload_obj.insert(
        "task_id".to_string(),
        Value::String(message.task_id.clone()),
    );
    payload_obj.insert(
        "task_message_id".to_string(),
        Value::Number(serde_json::Number::from(message.message_id)),
    );
    payload_obj.insert(
        "task_conversation_id".to_string(),
        Value::String(message.conversation_id.clone()),
    );
    Value::Object(payload_obj)
}

#[derive(Debug, Default)]
struct TaskCompileExtraction {
    task_list: Vec<String>,
    acceptance_criteria: Vec<String>,
    deadline: Option<String>,
    source_message_id: Option<i64>,
}

fn compile_task_run_preview_response(
    team_spec: &Value,
    task: &TeamTaskRecord,
    conversation: &TeamConversationRecord,
    messages: &[TeamConversationMessageRecord],
    requested_context_id: Option<&str>,
) -> Result<TeamTaskRunCompilePreviewResponse, ApiError> {
    let spec_obj = team_spec
        .as_object()
        .ok_or_else(|| ApiError::bad_request("spec must be an object"))?;
    let member_specs = parse_member_specs(spec_obj.get("members"))?;
    let coordinator_member_id = parse_spec_coordinator_member_id(spec_obj, &member_specs)?;
    let execution_plan = parse_task_execution_plan(&task.context)
        .map_err(|_| ApiError::bad_request("task context contains an invalid execution_plan"))?;
    let step_template = if let Some(plan) = execution_plan.as_ref() {
        compile_task_step_template_from_execution_plan(
            plan,
            &member_specs,
            coordinator_member_id.as_deref(),
        )?
    } else {
        compile_task_step_template(spec_obj, &member_specs, coordinator_member_id.as_deref())?
    };
    let mut extraction = extract_task_compile_extraction(&task.context);
    for message in messages {
        if apply_task_compile_message_update(&message.payload, &mut extraction) {
            extraction.source_message_id = Some(message.message_id);
        }
    }
    if extraction.task_list.is_empty() {
        extraction.task_list.push(task.title.clone());
    }
    if extraction.acceptance_criteria.is_empty() {
        extraction
            .acceptance_criteria
            .push(DEFAULT_TEAM_TASK_ACCEPTANCE_CRITERION.to_string());
    }

    let role_assignments = build_task_role_assignments(
        &step_template,
        &member_specs,
        coordinator_member_id.as_deref(),
    );
    let task_list = extraction.task_list.clone();
    let acceptance_criteria = extraction.acceptance_criteria.clone();
    let deadline = extraction.deadline.clone();
    let source_message_id = extraction.source_message_id;
    let step_template_for_input = step_template.clone();
    let role_assignments_for_input = role_assignments.clone();
    let context_id = requested_context_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| task.id.clone());
    let run_input = serde_json::json!({
        "task_compile_version": TEAM_TASK_COMPILE_VERSION,
        "task_id": task.id.as_str(),
        "conversation_id": conversation.id.as_str(),
        "task_title": task.title.as_str(),
        "task_list": task_list,
        "acceptance_criteria": acceptance_criteria,
        "deadline": deadline,
        "compiled_from_message_id": source_message_id,
        "step_template": step_template_for_input,
        "role_assignments": role_assignments_for_input,
    });
    let run_payload = TeamRunPayloadPreview {
        context_id,
        input: run_input,
    };
    let plan = TeamTaskCompiledPlan {
        task_list,
        acceptance_criteria,
        deadline,
        step_template,
        role_assignments,
        source_message_id,
    };
    Ok(TeamTaskRunCompilePreviewResponse {
        task_id: task.id.clone(),
        conversation_id: conversation.id.clone(),
        run_payload,
        plan,
    })
}

fn has_configured_team_members(spec: &Value) -> Result<bool, ApiError> {
    let spec_obj = spec
        .as_object()
        .ok_or_else(|| ApiError::bad_request("spec must be an object"))?;
    Ok(!parse_member_specs(spec_obj.get("members"))?.is_empty())
}

fn ensure_team_execution_ready(spec: &Value) -> Result<(), ApiError> {
    validate_team_spec(spec)?;
    if !has_configured_team_members(spec)? {
        return Err(ApiError::bad_request(
            "team has no members configured; add at least one agent first",
        ));
    }
    Ok(())
}

fn compile_task_step_template(
    spec_obj: &serde_json::Map<String, Value>,
    member_specs: &[TeamMemberSpec],
    coordinator_member_id: Option<&str>,
) -> Result<Vec<TeamCompiledStepTemplate>, ApiError> {
    let coordinator_member_id =
        resolve_effective_coordinator_member_id(coordinator_member_id, member_specs)
            .map(str::to_string)
            .ok_or_else(|| ApiError::bad_request("spec.members must not be empty"))?;

    let member_role_by_id = member_specs
        .iter()
        .map(|member| {
            (
                member.member_id.clone(),
                resolve_compiled_member_role(
                    member.member_id.as_str(),
                    member.role.as_str(),
                    coordinator_member_id.as_str(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();

    let generated_steps;
    let step_values = if let Some(steps) = spec_obj.get("steps").and_then(Value::as_array) {
        steps.as_slice()
    } else {
        let worker_member_ids = member_specs
            .iter()
            .map(|member| member.member_id.as_str())
            .filter(|member_id| *member_id != coordinator_member_id.as_str())
            .map(str::to_string)
            .collect::<Vec<_>>();
        generated_steps =
            build_default_team_steps(coordinator_member_id.as_str(), &worker_member_ids);
        generated_steps.as_slice()
    };

    let mut out = Vec::with_capacity(step_values.len());
    for step in step_values {
        let step_obj = step
            .as_object()
            .ok_or_else(|| ApiError::bad_request("spec.steps entries must be objects"))?;
        let step_key = step_obj
            .get("step_key")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::bad_request("spec.steps[].step_key is required"))?;
        let member_id = step_obj
            .get("member_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::bad_request("spec.steps[].member_id is required"))?;
        let role = member_role_by_id.get(member_id).ok_or_else(|| {
            ApiError::bad_request("spec.steps[].member_id must reference spec.members[].member_id")
        })?;
        let depends_on = parse_compile_step_depends_on(step_obj.get("depends_on"))?;
        out.push(TeamCompiledStepTemplate {
            step_key: step_key.to_string(),
            member_id: member_id.to_string(),
            role: role.to_string(),
            depends_on,
            goal: None,
            acceptance: Vec::new(),
            execution: TeamTaskStepExecutionSpec::default(),
        });
    }
    Ok(out)
}

fn compile_task_step_template_from_execution_plan(
    plan: &TeamTaskExecutionPlan,
    member_specs: &[TeamMemberSpec],
    coordinator_member_id: Option<&str>,
) -> Result<Vec<TeamCompiledStepTemplate>, ApiError> {
    let coordinator_member_id =
        resolve_effective_coordinator_member_id(coordinator_member_id, member_specs)
            .map(str::to_string)
            .ok_or_else(|| ApiError::bad_request("spec.members must not be empty"))?;

    let member_role_by_id = member_specs
        .iter()
        .map(|member| {
            (
                member.member_id.clone(),
                resolve_compiled_member_role(
                    member.member_id.as_str(),
                    member.role.as_str(),
                    coordinator_member_id.as_str(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();

    let mut out = Vec::with_capacity(plan.steps.len());
    for step in &plan.steps {
        let role = member_role_by_id
            .get(step.member_id.trim())
            .ok_or_else(|| {
                ApiError::bad_request(
                    "task context execution_plan.steps[].member_id must reference spec.members[].member_id",
                )
            })?;
        out.push(TeamCompiledStepTemplate {
            step_key: step.step_key.trim().to_string(),
            member_id: step.member_id.trim().to_string(),
            role: role.to_string(),
            depends_on: step
                .depends_on
                .iter()
                .map(|value| value.trim().to_string())
                .collect(),
            goal: step
                .goal
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            acceptance: step
                .acceptance
                .iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect(),
            execution: step.execution.clone(),
        });
    }
    Ok(out)
}

fn resolve_compiled_member_role(
    member_id: &str,
    base_role: &str,
    coordinator_member_id: &str,
) -> String {
    if member_id == coordinator_member_id {
        "coordinator".to_string()
    } else {
        base_role.to_string()
    }
}

fn resolve_effective_coordinator_member_id<'a>(
    coordinator_member_id: Option<&'a str>,
    member_specs: &'a [TeamMemberSpec],
) -> Option<&'a str> {
    coordinator_member_id
        .or_else(|| {
            member_specs
                .iter()
                .find(|member| member.role == "coordinator")
                .map(|member| member.member_id.as_str())
        })
        .or_else(|| member_specs.first().map(|member| member.member_id.as_str()))
}

fn parse_compile_step_depends_on(value: Option<&Value>) -> Result<Vec<String>, ApiError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let depends_on = value
        .as_array()
        .ok_or_else(|| ApiError::bad_request("spec.steps[].depends_on must be an array"))?;
    let mut out = Vec::with_capacity(depends_on.len());
    let mut seen = HashSet::with_capacity(depends_on.len());
    for dep in depends_on {
        let dep = dep
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ApiError::bad_request("spec.steps[].depends_on entries must be non-empty strings")
            })?;
        if !seen.insert(dep.to_string()) {
            return Err(ApiError::bad_request(
                "spec.steps[].depends_on must not contain duplicates",
            ));
        }
        out.push(dep.to_string());
    }
    Ok(out)
}

fn extract_task_compile_extraction(context: &Value) -> TaskCompileExtraction {
    let Some(context_obj) = context.as_object() else {
        return TaskCompileExtraction::default();
    };
    let mut extraction = TaskCompileExtraction::default();
    let _changed = apply_task_compile_patch(context_obj, &mut extraction);
    extraction
}

fn apply_task_compile_message_update(
    payload: &Value,
    extraction: &mut TaskCompileExtraction,
) -> bool {
    if let Some(patch) = payload
        .as_object()
        .and_then(|obj| obj.get("plan_update"))
        .and_then(Value::as_object)
    {
        return apply_task_compile_patch(patch, extraction);
    }
    payload
        .as_object()
        .map(|patch| apply_task_compile_patch(patch, extraction))
        .unwrap_or(false)
}

fn apply_task_compile_patch(
    patch: &serde_json::Map<String, Value>,
    extraction: &mut TaskCompileExtraction,
) -> bool {
    let mut changed = false;
    if let Some(task_list) = parse_compile_string_list_patch(patch, &["task_list", "tasks"]) {
        extraction.task_list = task_list;
        changed = true;
    }
    if let Some(acceptance_criteria) =
        parse_compile_string_list_patch(patch, &["acceptance_criteria", "acceptance"])
    {
        extraction.acceptance_criteria = acceptance_criteria;
        changed = true;
    }
    if let Some(deadline) = parse_compile_optional_text_patch(patch, "deadline") {
        extraction.deadline = deadline;
        changed = true;
    }
    changed
}

fn parse_compile_string_list_patch(
    patch: &serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<Vec<String>> {
    let value = keys.iter().find_map(|key| patch.get(*key))?;
    if value.is_null() {
        return Some(Vec::new());
    }
    let items = value.as_array()?;
    let mut out = Vec::with_capacity(items.len());
    let mut seen = HashSet::with_capacity(items.len());
    for item in items {
        let Some(item) = item
            .as_str()
            .and_then(|value| sanitize_compile_text(value, TEAM_TASK_COMPILE_MAX_TEXT_LEN))
        else {
            continue;
        };
        if out.len() >= TEAM_TASK_COMPILE_MAX_LIST_ITEMS {
            break;
        }
        if seen.insert(item.to_string()) {
            out.push(item);
        }
    }
    Some(out)
}

fn parse_compile_optional_text_patch(
    patch: &serde_json::Map<String, Value>,
    key: &str,
) -> Option<Option<String>> {
    let value = patch.get(key)?;
    if value.is_null() {
        return Some(None);
    }
    let text = value.as_str()?;
    let deadline = sanitize_compile_text(text, TEAM_TASK_COMPILE_MAX_DEADLINE_LEN)?;
    if !is_valid_compile_deadline(deadline.as_str()) {
        return Some(None);
    }
    Some(Some(deadline))
}

fn build_task_role_assignments(
    step_template: &[TeamCompiledStepTemplate],
    member_specs: &[TeamMemberSpec],
    coordinator_member_id: Option<&str>,
) -> Vec<TeamCompiledRoleAssignment> {
    let coordinator_member_id =
        resolve_effective_coordinator_member_id(coordinator_member_id, member_specs);
    let mut step_keys_by_member: HashMap<&str, Vec<String>> = HashMap::new();
    for step in step_template {
        step_keys_by_member
            .entry(step.member_id.as_str())
            .or_default()
            .push(step.step_key.clone());
    }
    let mut assignments = member_specs
        .iter()
        .map(|member| TeamCompiledRoleAssignment {
            member_id: member.member_id.clone(),
            role: if coordinator_member_id == Some(member.member_id.as_str()) {
                "coordinator".to_string()
            } else {
                member.role.clone()
            },
            step_keys: step_keys_by_member
                .remove(member.member_id.as_str())
                .unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    assignments.sort_by(|left, right| {
        role_sort_order(left.role.as_str())
            .cmp(&role_sort_order(right.role.as_str()))
            .then_with(|| left.member_id.cmp(&right.member_id))
    });
    assignments
}

fn sanitize_compile_text(value: &str, max_len: usize) -> Option<String> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    let mut cleaned = String::with_capacity(text.len().min(max_len));
    for ch in text.chars() {
        if ch.is_control() {
            if ch == '\n' || ch == '\r' || ch == '\t' {
                cleaned.push(' ');
            }
            continue;
        }
        if !is_allowed_compile_char(ch) {
            continue;
        }
        cleaned.push(ch);
        if cleaned.len() >= max_len {
            break;
        }
    }
    let normalized = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return None;
    }
    Some(normalized)
}

fn is_allowed_compile_char(ch: char) -> bool {
    ch.is_alphanumeric()
        || ch.is_whitespace()
        || matches!(
            ch,
            '.' | ','
                | ':'
                | ';'
                | '!'
                | '?'
                | '-'
                | '_'
                | '/'
                | '+'
                | '#'
                | '('
                | ')'
                | '['
                | ']'
                | '\''
                | '"'
                | '&'
                | '@'
        )
}

fn is_valid_compile_deadline(value: &str) -> bool {
    let mut parts = value.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    if year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return false;
    }
    let Ok(year) = year.parse::<i32>() else {
        return false;
    };
    let Ok(month) = month.parse::<u32>() else {
        return false;
    };
    let Ok(day) = day.parse::<u32>() else {
        return false;
    };
    chrono::NaiveDate::from_ymd_opt(year, month, day).is_some()
}

fn role_sort_order(role: &str) -> i32 {
    match role {
        "coordinator" => 0,
        "worker" => 1,
        _ => 2,
    }
}

fn normalize_enum_value(
    value: Option<&str>,
    field_name: &str,
    allowed_values: &[&str],
    default_value: &str,
) -> Result<String, ApiError> {
    let value = value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .unwrap_or(default_value);
    if allowed_values.contains(&value) {
        return Ok(value.to_string());
    }
    Err(ApiError::bad_request(&format!(
        "{field_name} must be one of: {}",
        allowed_values.join(", ")
    )))
}

async fn load_run_and_member_ids_for_user(
    state: &AppState,
    run_id: &str,
    user: &UserRecord,
) -> Result<(TeamRunRecord, HashSet<String>), ApiError> {
    let (run, team) = load_run_and_team_for_user(state, run_id, user).await?;
    let member_ids = parse_member_ids(team.spec.get("members"))?;
    Ok((run, member_ids))
}

fn resolve_run_mailbox_query_actor_ids(
    actor_id: &str,
    member_ids: &HashSet<String>,
    user: &UserRecord,
) -> Result<Vec<String>, ApiError> {
    let actor_id = normalize_required_field(actor_id.to_string(), "actor_id")?;
    if member_ids.contains(&actor_id) {
        return Ok(vec![actor_id.to_string()]);
    }
    if actor_id == TEAM_SPECIAL_USER_ACTOR_ALIAS {
        return Ok(vec![
            TEAM_SPECIAL_USER_ACTOR_ALIAS.to_string(),
            canonical_user_actor_id(user),
        ]);
    }
    if let Some(user_id) = actor_id.strip_prefix(TEAM_SPECIAL_USER_ACTOR_PREFIX) {
        let user_id = user_id.trim();
        if user_id.is_empty() {
            return Err(ApiError::bad_request(
                "actor_id user actor id must be non-empty",
            ));
        }
        if user_id != user.id {
            return Err(ApiError::bad_request(
                "actor_id user actor id must match authenticated user",
            ));
        }
        return Ok(vec![
            canonical_user_actor_id(user),
            TEAM_SPECIAL_USER_ACTOR_ALIAS.to_string(),
        ]);
    }
    Err(ApiError::bad_request(
        "actor_id must reference spec.members[].member_id or authenticated user actor",
    ))
}

fn parse_message_transport(raw: Option<&str>) -> Result<TeamActorMessageTransport, ApiError> {
    parse_actor_transport(raw)
        .map_err(|_| ApiError::bad_request("transport must be either 'local' or 'remote'"))
}

fn normalize_message_peer_id(
    raw: Option<&str>,
    field_name: &str,
    default_value: &str,
) -> Result<String, ApiError> {
    let normalized = raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_value);
    if normalized.len() > 128 {
        return Err(ApiError::bad_request(&format!(
            "{field_name} must be at most 128 characters"
        )));
    }
    Ok(normalized.to_string())
}

fn validate_message_actors(
    member_ids: &HashSet<String>,
    from_actor_id: &str,
    from_peer_id: &str,
    to_actor_id: &str,
    to_peer_id: &str,
    transport: &TeamActorMessageTransport,
    route: Option<&Value>,
) -> Result<(), ApiError> {
    if !member_ids.contains(from_actor_id) {
        return Err(ApiError::bad_request(
            "from_actor_id must reference spec.members[].member_id",
        ));
    }
    if from_peer_id != ACTOR_MAIN_PEER_ID {
        return Err(ApiError::bad_request(
            "from_peer_id must be 'main' for team mailbox send API",
        ));
    }
    match transport {
        TeamActorMessageTransport::Local => {
            if !member_ids.contains(to_actor_id) {
                return Err(ApiError::bad_request(
                    "to_actor_id must reference spec.members[].member_id for local transport",
                ));
            }
            if to_peer_id != ACTOR_MAIN_PEER_ID {
                return Err(ApiError::bad_request(
                    "to_peer_id must be 'main' for local transport",
                ));
            }
            if route.is_some() {
                return Err(ApiError::bad_request(
                    "route is not supported for local transport",
                ));
            }
        }
        TeamActorMessageTransport::Remote => {
            if route.is_none() {
                return Err(ApiError::bad_request(
                    "route is required for remote transport",
                ));
            }
            if to_peer_id == ACTOR_MAIN_PEER_ID {
                return Err(ApiError::bad_request(
                    "to_peer_id must not be 'main' for remote transport",
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
struct TeamStepSpec {
    step_key: String,
    member_id: String,
    depends_on: Vec<String>,
}

#[derive(Debug, Clone)]
struct TeamMemberSpec {
    member_id: String,
    role: String,
    model: Option<String>,
    prompt: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum ProfilePatchTarget {
    Run,
    Team,
}

impl ProfilePatchTarget {
    fn as_str(self) -> &'static str {
        match self {
            ProfilePatchTarget::Run => "run",
            ProfilePatchTarget::Team => "team",
        }
    }
}

#[derive(Debug, Clone)]
struct ProfilePatchProposal {
    target: ProfilePatchTarget,
    member_id: String,
    prompt_append: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct MemberProfileOverride {
    prompt_append: Option<String>,
    description: Option<String>,
}

fn normalize_team_spec(spec: &mut Value) -> Result<(), ApiError> {
    let spec_obj = spec
        .as_object_mut()
        .ok_or_else(|| ApiError::bad_request("spec must be an object"))?;
    let version = parse_team_spec_version(spec_obj.get("spec_version"))?;
    spec_obj.insert("spec_version".to_string(), Value::from(version));
    inject_team_spec_defaults(spec_obj)?;
    Ok(())
}

fn prune_deleted_member_from_team_spec(
    spec: &Value,
    member_id: &str,
) -> Result<Option<Value>, ApiError> {
    let spec_map = spec
        .as_object()
        .ok_or_else(|| ApiError::bad_request("spec must be an object"))?;
    let current_members = parse_member_specs(spec.get("members"))?;
    if !current_members
        .iter()
        .any(|member| member.member_id == member_id)
    {
        return Ok(None);
    }

    let mut next = spec.clone();
    let spec_obj = next
        .as_object_mut()
        .ok_or_else(|| ApiError::bad_request("spec must be an object"))?;
    let members = spec_obj
        .get_mut("members")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| ApiError::bad_request("spec.members must be an array"))?;
    members.retain(|member| {
        member
            .get("member_id")
            .and_then(Value::as_str)
            .map(str::trim)
            != Some(member_id)
    });

    let remaining_members = parse_member_specs(spec_obj.get("members"))?;
    if remaining_members.is_empty() {
        normalize_team_spec(&mut next)?;
        validate_team_spec(&next)?;
        return Ok(Some(next));
    }

    let previous_coordinator_id = parse_spec_coordinator_member_id(spec_map, &current_members)?;
    let coordinator_changed = previous_coordinator_id.as_deref() == Some(member_id);
    let next_coordinator_id = resolve_pruned_team_coordinator_id(spec_obj, &remaining_members)?;

    if coordinator_changed {
        promote_pruned_team_coordinator(spec_obj, next_coordinator_id.as_str())?;
    }
    spec_obj.insert(
        "coordinator_member_id".to_string(),
        Value::String(next_coordinator_id.clone()),
    );

    let remaining_member_ids = remaining_members
        .iter()
        .map(|member| member.member_id.clone())
        .collect::<HashSet<_>>();
    let mut regenerate_default_steps = coordinator_changed;
    let current_entrypoint = spec_obj
        .get("entrypoint")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string();
    if let Some(steps) = spec_obj.get_mut("steps").and_then(Value::as_array_mut) {
        let mut removed_step_keys = HashSet::new();
        steps.retain(|step| {
            let member_matches = step
                .get("member_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .is_some_and(|value| value == member_id);
            if member_matches && let Some(step_key) = step.get("step_key").and_then(Value::as_str) {
                removed_step_keys.insert(step_key.trim().to_string());
            }
            !member_matches
        });

        let surviving_step_keys = steps
            .iter()
            .filter_map(|step| step.get("step_key").and_then(Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect::<HashSet<_>>();
        for step in steps.iter_mut() {
            if let Some(depends_on) = step.get_mut("depends_on").and_then(Value::as_array_mut) {
                depends_on.retain(|dependency| {
                    dependency.as_str().map(str::trim).is_some_and(|dep| {
                        !removed_step_keys.contains(dep) && surviving_step_keys.contains(dep)
                    })
                });
            }
        }

        regenerate_default_steps = regenerate_default_steps
            || steps.is_empty()
            || !surviving_step_keys.contains(current_entrypoint.as_str())
            || validate_steps(
                current_entrypoint.as_str(),
                &Value::Array(steps.clone()),
                &remaining_member_ids,
            )
            .is_err();
    } else if spec_obj
        .get("entrypoint")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|entrypoint| entrypoint == member_id)
    {
        regenerate_default_steps = true;
    }

    if regenerate_default_steps {
        spec_obj.remove("steps");
        spec_obj.insert("entrypoint".to_string(), Value::String(next_coordinator_id));
    }

    normalize_team_spec(&mut next)?;
    validate_team_spec(&next)?;
    Ok(Some(next))
}

fn resolve_pruned_team_coordinator_id(
    spec_obj: &Map<String, Value>,
    remaining_members: &[TeamMemberSpec],
) -> Result<String, ApiError> {
    let remaining_member_ids = remaining_members
        .iter()
        .map(|member| member.member_id.as_str())
        .collect::<HashSet<_>>();
    if let Some(explicit) = spec_obj
        .get("coordinator_member_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| remaining_member_ids.contains(value))
    {
        return Ok(explicit.to_string());
    }
    if let Some(role_coordinator) = remaining_members
        .iter()
        .find(|member| member.role == "coordinator")
    {
        return Ok(role_coordinator.member_id.clone());
    }
    remaining_members
        .first()
        .map(|member| member.member_id.clone())
        .ok_or_else(|| ApiError::bad_request("spec.members must not be empty"))
}

fn promote_pruned_team_coordinator(
    spec_obj: &mut Map<String, Value>,
    coordinator_member_id: &str,
) -> Result<(), ApiError> {
    let members = spec_obj
        .get_mut("members")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| ApiError::bad_request("spec.members must be an array"))?;
    for member in members {
        let Some(member_obj) = member.as_object_mut() else {
            continue;
        };
        let Some(current_member_id) = member_obj
            .get("member_id")
            .and_then(Value::as_str)
            .map(str::trim)
        else {
            continue;
        };
        let next_role = if current_member_id == coordinator_member_id {
            "coordinator"
        } else {
            "worker"
        };
        member_obj.insert("role".to_string(), Value::String(next_role.to_string()));
    }
    Ok(())
}

fn inject_team_spec_defaults(
    spec_obj: &mut serde_json::Map<String, Value>,
) -> Result<(), ApiError> {
    let member_specs = parse_member_specs(spec_obj.get("members"))?;
    if member_specs.is_empty() {
        spec_obj.remove("coordinator_member_id");
        spec_obj.remove("entrypoint");
        spec_obj.remove("steps");
        return Ok(());
    }
    let member_specs_by_id = member_specs
        .iter()
        .map(|member| (member.member_id.as_str(), member))
        .collect::<HashMap<_, _>>();
    let member_ids = member_specs
        .iter()
        .map(|member| member.member_id.as_str())
        .collect::<HashSet<_>>();
    let coordinator_member_id = parse_spec_coordinator_member_id(spec_obj, &member_specs)?;
    if let Some(coordinator_id) = coordinator_member_id.as_deref()
        && !spec_obj.contains_key("coordinator_member_id")
    {
        spec_obj.insert(
            "coordinator_member_id".to_string(),
            Value::String(coordinator_id.to_string()),
        );
    }

    if let Some(members) = spec_obj.get_mut("members").and_then(Value::as_array_mut) {
        for member in members {
            let Some(member_obj) = member.as_object_mut() else {
                continue;
            };
            let Some(member_id) = member_obj
                .get("member_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let Some(member_spec) = member_specs_by_id.get(member_id) else {
                continue;
            };

            if is_missing_or_null(member_obj.get("prompt")) {
                member_obj.insert(
                    "prompt".to_string(),
                    Value::String(default_team_prompt_for_role(&member_spec.role).to_string()),
                );
            }
            member_obj.remove("skills");
        }
    }

    let entrypoint_member_id = spec_obj
        .get("entrypoint")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let coordinator_matches_entrypoint =
        match (coordinator_member_id.as_deref(), entrypoint_member_id) {
            (Some(coordinator), Some(entrypoint)) => coordinator == entrypoint,
            _ => true,
        };
    let should_generate_steps = spec_obj.get("steps").is_none()
        && entrypoint_member_id.is_some_and(|entrypoint| member_ids.contains(entrypoint))
        && coordinator_matches_entrypoint;

    if should_generate_steps {
        let coordinator_id = coordinator_member_id
            .or_else(|| entrypoint_member_id.map(str::to_string))
            .or_else(|| member_specs.first().map(|member| member.member_id.clone()))
            .ok_or_else(|| ApiError::bad_request("spec.members must not be empty"))?;
        let worker_member_ids = member_specs
            .iter()
            .map(|member| member.member_id.clone())
            .filter(|member_id| member_id != &coordinator_id)
            .collect::<Vec<_>>();
        let steps = build_default_team_steps(&coordinator_id, &worker_member_ids);
        spec_obj.insert("steps".to_string(), Value::Array(steps));
        spec_obj.insert(
            "entrypoint".to_string(),
            Value::String(DEFAULT_TEAM_PLAN_STEP_KEY.to_string()),
        );
    }

    Ok(())
}

fn is_missing_or_null(value: Option<&Value>) -> bool {
    match value {
        None => true,
        Some(Value::Null) => true,
        Some(_) => false,
    }
}

fn build_default_team_steps(
    coordinator_member_id: &str,
    worker_member_ids: &[String],
) -> Vec<Value> {
    let mut steps = Vec::with_capacity(worker_member_ids.len() + 2);
    steps.push(serde_json::json!({
        "step_key": DEFAULT_TEAM_PLAN_STEP_KEY,
        "member_id": coordinator_member_id,
        "depends_on": [],
    }));
    if worker_member_ids.is_empty() {
        return steps;
    }

    let mut worker_step_keys = Vec::with_capacity(worker_member_ids.len());
    for (index, worker_id) in worker_member_ids.iter().enumerate() {
        let step_key = format!(
            "worker_{}_{}",
            index + 1,
            sanitize_step_key_token(worker_id)
        );
        worker_step_keys.push(step_key.clone());
        steps.push(serde_json::json!({
            "step_key": step_key,
            "member_id": worker_id,
            "depends_on": [DEFAULT_TEAM_PLAN_STEP_KEY],
        }));
    }
    steps.push(serde_json::json!({
        "step_key": DEFAULT_TEAM_SYNTH_STEP_KEY,
        "member_id": coordinator_member_id,
        "depends_on": worker_step_keys,
    }));
    steps
}

fn sanitize_step_key_token(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_is_sep = false;
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_is_sep = false;
            continue;
        }
        if !prev_is_sep {
            out.push('_');
            prev_is_sep = true;
        }
    }
    let token = out.trim_matches('_');
    if token.is_empty() {
        "worker".to_string()
    } else {
        token.to_string()
    }
}

fn validate_team_spec(spec: &Value) -> Result<(), ApiError> {
    let spec_obj = spec
        .as_object()
        .ok_or_else(|| ApiError::bad_request("spec must be an object"))?;
    let _ = parse_team_spec_version(spec_obj.get("spec_version"))?;
    let member_specs = parse_member_specs(spec_obj.get("members"))?;
    let coordinator_member_id = parse_spec_coordinator_member_id(spec_obj, &member_specs)?;
    let entrypoint = spec_obj
        .get("entrypoint")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if member_specs.is_empty() {
        if coordinator_member_id.is_some() {
            return Err(ApiError::bad_request(
                "spec.coordinator_member_id must be omitted until spec.members is configured",
            ));
        }
        if entrypoint.is_some() {
            return Err(ApiError::bad_request(
                "spec.entrypoint must be omitted until spec.members is configured",
            ));
        }
        if spec_obj.get("steps").is_some() {
            return Err(ApiError::bad_request(
                "spec.steps must be omitted until spec.members is configured",
            ));
        }
        return Ok(());
    }

    let entrypoint =
        entrypoint.ok_or_else(|| ApiError::bad_request("spec.entrypoint is required"))?;
    let member_ids = member_specs
        .iter()
        .map(|member| member.member_id.clone())
        .collect::<HashSet<_>>();

    if let Some(steps_value) = spec_obj.get("steps") {
        validate_steps(entrypoint, steps_value, &member_ids)?;
    } else if !member_ids.contains(entrypoint) {
        return Err(ApiError::bad_request(
            "spec.entrypoint must reference spec.members[].member_id when spec.steps is omitted",
        ));
    } else if let Some(coordinator_id) = coordinator_member_id.as_deref()
        && entrypoint != coordinator_id
    {
        return Err(ApiError::bad_request(
            "spec.entrypoint must equal coordinator_member_id when spec.steps is omitted",
        ));
    }

    Ok(())
}

fn parse_team_spec_version(version_value: Option<&Value>) -> Result<i64, ApiError> {
    let version = match version_value {
        None => TEAM_SPEC_VERSION_V1,
        Some(value) => value
            .as_i64()
            .ok_or_else(|| ApiError::bad_request("spec.spec_version must be an integer"))?,
    };
    if version != TEAM_SPEC_VERSION_V1 {
        return Err(ApiError::bad_request(
            "unsupported spec.spec_version; expected 1",
        ));
    }
    Ok(version)
}

fn parse_member_ids(members_value: Option<&Value>) -> Result<HashSet<String>, ApiError> {
    let member_specs = parse_member_specs(members_value)?;
    Ok(member_specs
        .into_iter()
        .map(|member| member.member_id)
        .collect())
}

async fn reconcile_team_member_runtime_absence(
    state: &AppState,
    team: &TeamDefinitionRecord,
) -> Result<(), ApiError> {
    let member_ids = crate::team::collect_team_member_ids(&team.spec);
    state
        .agents
        .reconcile_runtime_absence_batch(&member_ids)
        .await
        .map_err(map_team_internal_error)?;
    Ok(())
}

fn parse_member_specs(members_value: Option<&Value>) -> Result<Vec<TeamMemberSpec>, ApiError> {
    let members = members_value
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::bad_request("spec.members must be an array"))?;

    let mut seen_member_ids = HashSet::with_capacity(members.len());
    let mut out = Vec::with_capacity(members.len());
    for member in members {
        let member = member
            .as_object()
            .ok_or_else(|| ApiError::bad_request("spec.members entries must be objects"))?;
        let member_id = member
            .get("member_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::bad_request("spec.members[].member_id is required"))?
            .to_string();
        if !seen_member_ids.insert(member_id.clone()) {
            return Err(ApiError::bad_request(
                "spec.members[].member_id must be unique",
            ));
        }

        let role = parse_required_member_role(member.get("role"))?;
        let model = parse_optional_member_text(member.get("model"), "model")?;
        let prompt = parse_optional_member_text(member.get("prompt"), "prompt")?;
        let description = parse_optional_member_description(member.get("description"))?;
        out.push(TeamMemberSpec {
            member_id,
            role,
            model,
            prompt,
            description,
        });
    }
    Ok(out)
}

fn parse_optional_member_text(
    value: Option<&Value>,
    field: &str,
) -> Result<Option<String>, ApiError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let raw = value.as_str().ok_or_else(|| {
        ApiError::bad_request(&format!(
            "spec.members[].{field} must be a non-empty string when provided"
        ))
    })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(&format!(
            "spec.members[].{field} must be a non-empty string when provided"
        )));
    }
    Ok(Some(trimmed.to_string()))
}

fn parse_required_member_role(value: Option<&Value>) -> Result<String, ApiError> {
    let Some(value) = value else {
        return Err(ApiError::bad_request(
            "spec.members[].role is required and must be 'coordinator' or 'worker'",
        ));
    };
    let raw = value
        .as_str()
        .ok_or_else(|| {
            ApiError::bad_request(
                "spec.members[].role is required and must be 'coordinator' or 'worker'",
            )
        })?
        .trim();
    if raw.is_empty() {
        return Err(ApiError::bad_request(
            "spec.members[].role is required and must be 'coordinator' or 'worker'",
        ));
    }
    if raw != "coordinator" && raw != "worker" {
        return Err(ApiError::bad_request(
            "spec.members[].role is required and must be 'coordinator' or 'worker'",
        ));
    }
    Ok(raw.to_string())
}

fn parse_optional_member_description(value: Option<&Value>) -> Result<Option<String>, ApiError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let raw = value
        .as_str()
        .ok_or_else(|| ApiError::bad_request("spec.members[].description must be a string"))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(trimmed.to_string()))
}

fn parse_spec_coordinator_member_id(
    spec_obj: &serde_json::Map<String, Value>,
    member_specs: &[TeamMemberSpec],
) -> Result<Option<String>, ApiError> {
    let member_ids = member_specs
        .iter()
        .map(|member| member.member_id.as_str())
        .collect::<HashSet<_>>();
    let explicit_coordinator = match spec_obj.get("coordinator_member_id") {
        None => None,
        Some(value) => {
            let coordinator = value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ApiError::bad_request("spec.coordinator_member_id must be a non-empty string")
                })?;
            if !member_ids.contains(coordinator) {
                return Err(ApiError::bad_request(
                    "spec.coordinator_member_id must reference spec.members[].member_id",
                ));
            }
            Some(coordinator.to_string())
        }
    };

    let member_role_coordinators = member_specs
        .iter()
        .filter_map(|member| match member.role.as_str() {
            "coordinator" => Some(member.member_id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if member_role_coordinators.len() > 1 {
        return Err(ApiError::bad_request(
            "spec.members[].role may include at most one 'coordinator'",
        ));
    }

    if let Some(explicit) = explicit_coordinator.as_deref() {
        if let Some(role_coordinator) = member_role_coordinators.first()
            && explicit != *role_coordinator
        {
            return Err(ApiError::bad_request(
                "spec.coordinator_member_id must match spec.members[].role='coordinator'",
            ));
        }
        return Ok(Some(explicit.to_string()));
    }

    if let Some(role_coordinator) = member_role_coordinators.first() {
        return Ok(Some((*role_coordinator).to_string()));
    }

    let entrypoint_member = spec_obj
        .get("entrypoint")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| member_ids.contains(value));
    Ok(entrypoint_member.map(str::to_string))
}

fn validate_steps(
    entrypoint: &str,
    steps_value: &Value,
    member_ids: &HashSet<String>,
) -> Result<(), ApiError> {
    let steps = steps_value
        .as_array()
        .ok_or_else(|| ApiError::bad_request("spec.steps must be an array"))?;
    if steps.is_empty() {
        return Err(ApiError::bad_request("spec.steps must not be empty"));
    }
    if steps.len() > MAX_TEAM_SPEC_STEPS {
        return Err(ApiError::bad_request(
            "spec.steps must not exceed 2048 entries",
        ));
    }

    let mut step_specs = Vec::with_capacity(steps.len());
    let mut step_keys = HashSet::with_capacity(steps.len());
    for step in steps {
        let step = step
            .as_object()
            .ok_or_else(|| ApiError::bad_request("spec.steps entries must be objects"))?;
        let step_key = step
            .get("step_key")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::bad_request("spec.steps[].step_key is required"))?
            .to_string();
        if !step_keys.insert(step_key.clone()) {
            return Err(ApiError::bad_request(
                "spec.steps[].step_key must be unique",
            ));
        }

        let member_id = step
            .get("member_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::bad_request("spec.steps[].member_id is required"))?
            .to_string();

        let depends_on = match step.get("depends_on") {
            Some(depends_on_value) => {
                let depends_on = depends_on_value.as_array().ok_or_else(|| {
                    ApiError::bad_request("spec.steps[].depends_on must be an array")
                })?;
                let mut seen_depends = HashSet::with_capacity(depends_on.len());
                let mut keys = Vec::with_capacity(depends_on.len());
                for dep in depends_on {
                    let dep = dep
                        .as_str()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| {
                            ApiError::bad_request(
                                "spec.steps[].depends_on entries must be non-empty strings",
                            )
                        })?;
                    if !seen_depends.insert(dep.to_string()) {
                        return Err(ApiError::bad_request(
                            "spec.steps[].depends_on must not contain duplicates",
                        ));
                    }
                    keys.push(dep.to_string());
                }
                keys
            }
            None => Vec::new(),
        };

        step_specs.push(TeamStepSpec {
            step_key,
            member_id,
            depends_on,
        });
    }

    if !step_keys.contains(entrypoint) {
        return Err(ApiError::bad_request(
            "spec.entrypoint must reference spec.steps[].step_key when spec.steps is provided",
        ));
    }

    for step in &step_specs {
        if !member_ids.contains(&step.member_id) {
            return Err(ApiError::bad_request(
                "spec.steps[].member_id must reference spec.members[].member_id",
            ));
        }
        for dep in &step.depends_on {
            if dep == &step.step_key {
                return Err(ApiError::bad_request(
                    "spec.steps[].depends_on must not include the step itself",
                ));
            }
            if !step_keys.contains(dep) {
                return Err(ApiError::bad_request(
                    "spec.steps[].depends_on must reference existing spec.steps[].step_key",
                ));
            }
        }
    }

    ensure_acyclic_steps(&step_specs)?;
    Ok(())
}

fn ensure_acyclic_steps(steps: &[TeamStepSpec]) -> Result<(), ApiError> {
    let mut indegree: HashMap<&str, usize> = HashMap::with_capacity(steps.len());
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::with_capacity(steps.len());
    for step in steps {
        let step_key = step.step_key.as_str();
        indegree.insert(step_key, step.depends_on.len());
        for dep in &step.depends_on {
            dependents.entry(dep.as_str()).or_default().push(step_key);
        }
    }

    let mut ready: VecDeque<&str> = indegree
        .iter()
        .filter_map(|(key, count)| if *count == 0 { Some(*key) } else { None })
        .collect();
    let mut visited = 0usize;

    while let Some(step_key) = ready.pop_front() {
        visited += 1;
        if let Some(children) = dependents.get(step_key) {
            for child in children {
                if let Some(count) = indegree.get_mut(child) {
                    *count -= 1;
                    if *count == 0 {
                        ready.push_back(child);
                    }
                }
            }
        }
    }

    if visited == steps.len() {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "spec.steps must form an acyclic dependency graph",
        ))
    }
}

fn index_latest_steps_by_member(steps: &[TeamStepRecord]) -> HashMap<&str, TeamStepRecord> {
    let mut by_member: HashMap<&str, TeamStepRecord> = HashMap::new();
    for step in steps {
        let key = step.member_id.as_str();
        match by_member.get(key) {
            None => {
                by_member.insert(key, step.clone());
            }
            Some(existing) => {
                if step_order_tuple(step) > step_order_tuple(existing) {
                    by_member.insert(key, step.clone());
                }
            }
        }
    }
    by_member
}

fn step_order_tuple(step: &TeamStepRecord) -> (i64, i64, i64) {
    let ts = step.ended_at.or(step.started_at).unwrap_or(0);
    let active_rank = match step.status {
        TeamStepStatus::Working | TeamStepStatus::InputRequired => 2,
        TeamStepStatus::Submitted => 1,
        TeamStepStatus::Completed | TeamStepStatus::Failed | TeamStepStatus::Canceled => 0,
    };
    (active_rank, ts, step.attempt)
}

fn step_status_to_str(status: &TeamStepStatus) -> &'static str {
    match status {
        TeamStepStatus::Submitted => "submitted",
        TeamStepStatus::Working => "working",
        TeamStepStatus::InputRequired => "input_required",
        TeamStepStatus::Completed => "completed",
        TeamStepStatus::Failed => "failed",
        TeamStepStatus::Canceled => "canceled",
    }
}

#[cfg(test)]
pub(crate) mod tests;
