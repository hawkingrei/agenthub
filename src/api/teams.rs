use std::collections::{HashMap, HashSet, VecDeque};

mod errors;

use self::errors::{
    map_actor_service_api_error, map_create_team_error, map_not_found_error, map_resume_run_error,
    map_runtime_start_error, map_submit_step_error, map_team_internal_error,
};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, ActorAckRequest, ActorInboxRequest,
    ActorMailboxService, ActorMessageStatus, ActorSendRequest, ActorServiceErrorCode,
    actor_inbox_with_auto_ack, parse_actor_transport,
};
use agenthub_team_prompts::default_team_prompt_for_role;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::api::authz::require_user;
use crate::api::error::ApiError;
use crate::auth::UserRecord;
use crate::state::AppState;
use crate::team::{
    TEAM_RUN_STATUS_VALUES, TEAM_TASK_STATUS_VALUES, TeamActorMessageRecord,
    TeamActorMessageTransport, TeamConversationMessageRecord, TeamConversationRecord,
    TeamDefinitionConfig, TeamDefinitionRecord, TeamMemoryFlushRequest, TeamRunEventRecord,
    TeamRunRecord, TeamRunStatus, TeamRuntimeRecord, TeamStepRecord, TeamStepStatus,
    TeamTaskRecord, TeamTaskStatus, build_actor_mailbox_immediate_hint_prompt,
    ensure_team_runtime_started, force_team_member_new_session, plan_actor_mailbox_immediate_hint,
    stop_team_runtime,
};

const TEAM_SPEC_VERSION_V1: i64 = 1;
const SQLITE_CONSTRAINT_UNIQUE_CODE: &str = "2067";
const MAX_TEAM_SPEC_STEPS: usize = 2048;
const DEFAULT_TEAM_PLAN_STEP_KEY: &str = "leader_plan";
const DEFAULT_TEAM_SYNTH_STEP_KEY: &str = "leader_synthesize";
const DEFAULT_TEAM_LEADER_SKILLS: [&str; 5] = [
    "agenthub-actor-runtime",
    "team-agents-index",
    "team-task-lifecycle",
    "team-leader-orchestrator",
    "team-actor-mailbox",
];
const DEFAULT_TEAM_WORKER_SKILLS: [&str; 5] = [
    "agenthub-actor-runtime",
    "team-agents-index",
    "team-task-lifecycle",
    "team-worker-executor",
    "team-actor-mailbox",
];
const REQUIRED_TEAM_LEADER_SKILLS: [&str; 5] = [
    "agenthub-actor-runtime",
    "team-agents-index",
    "team-task-lifecycle",
    "team-leader-orchestrator",
    "team-actor-mailbox",
];
const REQUIRED_TEAM_WORKER_SKILLS: [&str; 5] = [
    "agenthub-actor-runtime",
    "team-agents-index",
    "team-task-lifecycle",
    "team-worker-executor",
    "team-actor-mailbox",
];
const TEAM_CONVERSATION_MODE_VALUES: [&str; 3] = ["to_leader", "to_member", "group_chat"];
const TEAM_CONVERSATION_ROUTE_VALUES: [&str; 3] = ["to_leader", "to_member", "group_chat"];
const TEAM_SPECIAL_USER_ACTOR_ALIAS: &str = "user";
const TEAM_SPECIAL_USER_ACTOR_PREFIX: &str = "user:";
const TEAM_SHARED_THREAD_BOOTSTRAP_KIND: &str = "shared_thread";
const TEAM_TASK_COMPILE_VERSION: i64 = 1;
const TEAM_TASK_COMPILE_MESSAGE_LIMIT: i64 = 500;
const DEFAULT_TEAM_TASK_ACCEPTANCE_CRITERION: &str =
    "All assigned steps complete and leader synthesis is delivered.";
const TEAM_TASK_COMPILE_MAX_LIST_ITEMS: usize = 32;
const TEAM_MEMORY_FLUSH_TRIGGER_VALUES: [&str; 3] = ["manual", "soft_threshold", "hard_error"];
const TEAM_TASK_COMPILE_MAX_TEXT_LEN: usize = 280;
const TEAM_TASK_COMPILE_MAX_DEADLINE_LEN: usize = 40;

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
    if !team_owner_matches_user(&team, user) {
        return Err(ApiError::not_found("team not found"));
    }
    Ok(team)
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
pub struct CreateTeamRunRequest {
    pub context_id: Option<String>,
    pub input: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeamTaskRequest {
    pub title: String,
    pub created_by_actor_id: Option<String>,
    pub context: Option<Value>,
    pub conversation_mode: Option<String>,
    pub topic: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListTeamTasksQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTeamTaskRequest {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct SendTeamTaskMessageRequest {
    pub from_actor_id: Option<String>,
    pub to_actor_id: Option<String>,
    pub route: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Deserialize)]
pub struct ListTeamTaskMessagesQuery {
    pub limit: Option<i64>,
    pub before_id: Option<i64>,
}

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

#[derive(Debug, Serialize)]
pub struct TeamRunSnapshotResponse {
    pub run: TeamRunRecord,
    pub team: TeamDefinitionRecord,
    pub leader_member_id: Option<String>,
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
    pub status: String,
    pub latest_step: Option<TeamStepRecord>,
    pub session_status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TeamMailboxSnapshot {
    pub pending: i64,
    pub delivered: i64,
    pub dead_letter: i64,
    pub recent_messages: Vec<TeamActorMessageRecord>,
}

#[derive(Debug, Serialize)]
pub struct TeamTaskDetailResponse {
    pub task: TeamTaskRecord,
    pub conversation: TeamConversationRecord,
    pub latest_run: Option<TeamRunRecord>,
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
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct TeamCompiledRoleAssignment {
    pub member_id: String,
    pub role: String,
    pub step_keys: Vec<String>,
}

pub type TeamRuntimeControlResponse = crate::team::TeamRuntimeControlRecord;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", post(create_team).get(list_teams))
        .route("/{id}", get(get_team).delete(delete_team))
        .route("/{id}/spec", put(update_team_spec))
        .route("/{id}/runtime", get(get_team_runtime))
        .route("/{id}/start", post(start_team))
        .route("/{id}/stop", post(stop_team))
        .route(
            "/{id}/members/{member_id}/force_new_session",
            post(force_new_session_for_team_member),
        )
        .route("/{id}/tasks", post(create_team_task).get(list_team_tasks))
        .route(
            "/{id}/tasks/{task_id}",
            get(get_team_task).patch(update_team_task),
        )
        .route(
            "/{id}/tasks/{task_id}/messages",
            post(send_team_task_message).get(list_team_task_messages),
        )
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
        .with_state(state)
}

async fn create_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateTeamRequest>,
) -> Result<Json<TeamDefinitionRecord>, ApiError> {
    let user = require_user(&headers, &state).await?;
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
    Ok(Json(team))
}

async fn update_team_spec(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Json(payload): Json<UpdateTeamSpecRequest>,
) -> Result<Json<TeamDefinitionRecord>, ApiError> {
    let user = require_user(&headers, &state).await?;
    let current = load_team_for_user(&state, &team_id, &user).await?;
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
    Ok(Json(updated))
}

async fn start_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Json<TeamRuntimeControlResponse>, ApiError> {
    let user = require_user(&headers, &state).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
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
    let user = require_user(&headers, &state).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    let runtime = stop_team_runtime(state.agents.as_ref(), &team)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(runtime))
}

async fn force_new_session_for_team_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, member_id)): Path<(String, String)>,
) -> Result<Json<TeamRuntimeControlResponse>, ApiError> {
    let user = require_user(&headers, &state).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
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
    let user = require_user(&headers, &state).await?;
    let teams = state
        .teams
        .list_teams()
        .await?
        .into_iter()
        .filter(|team| team_owner_matches_user(team, &user))
        .collect::<Vec<_>>();
    Ok(Json(teams))
}

async fn get_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Json<TeamDefinitionRecord>, ApiError> {
    let user = require_user(&headers, &state).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    Ok(Json(team))
}

async fn get_team_runtime(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Json<TeamRuntimeRecord>, ApiError> {
    let user = require_user(&headers, &state).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
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
    let user = require_user(&headers, &state).await?;
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
    Ok(Json(team))
}

async fn create_team_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Json(payload): Json<CreateTeamTaskRequest>,
) -> Result<Json<TeamTaskDetailResponse>, ApiError> {
    let user = require_user(&headers, &state).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let title = payload.title.trim().to_string();
    if title.is_empty() {
        return Err(ApiError::bad_request("title is required"));
    }
    let created_by_actor_id =
        normalize_task_created_by_actor_id(payload.created_by_actor_id.as_deref(), &user)?;
    let conversation_mode = normalize_conversation_mode(payload.conversation_mode.as_deref())?;
    let raw_context = payload.context.unwrap_or_else(|| serde_json::json!({}));
    let (task, conversation) = state
        .teams
        .create_task(
            &team_id,
            &title,
            &created_by_actor_id,
            raw_context,
            &conversation_mode,
            payload.topic.as_deref(),
        )
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(TeamTaskDetailResponse {
        task,
        conversation,
        latest_run: None,
    }))
}

async fn list_team_tasks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Query(query): Query<ListTeamTasksQuery>,
) -> Result<Json<Vec<TeamTaskRecord>>, ApiError> {
    let user = require_user(&headers, &state).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let tasks = state
        .teams
        .list_tasks(&team_id, query.limit.unwrap_or(100).clamp(1, 500))
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(tasks))
}

async fn get_team_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id)): Path<(String, String)>,
) -> Result<Json<TeamTaskDetailResponse>, ApiError> {
    let user = require_user(&headers, &state).await?;
    load_team_for_user(&state, &team_id, &user).await?;
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
    let latest_run = state
        .teams
        .get_latest_run_for_task(&team_id, &task_id)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(TeamTaskDetailResponse {
        task,
        conversation,
        latest_run,
    }))
}

async fn update_team_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id)): Path<(String, String)>,
    Json(payload): Json<UpdateTeamTaskRequest>,
) -> Result<Json<TeamTaskRecord>, ApiError> {
    let user = require_user(&headers, &state).await?;
    load_team_for_user(&state, &team_id, &user).await?;
    let task = state
        .teams
        .get_task(&task_id)
        .await
        .map_err(|err| map_not_found_error(err, "task not found"))?;
    if task.team_id != team_id {
        return Err(ApiError::not_found("task not found"));
    }
    let status = normalize_task_status(Some(payload.status.as_str()))?;
    let updated = state
        .teams
        .update_task_status(&task_id, status)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(updated))
}

async fn send_team_task_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id)): Path<(String, String)>,
    Json(payload): Json<SendTeamTaskMessageRequest>,
) -> Result<Json<TeamConversationMessageRecord>, ApiError> {
    let user = require_user(&headers, &state).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    let SendTeamTaskMessageRequest {
        from_actor_id,
        to_actor_id,
        route,
        payload,
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
    let route = normalize_conversation_route(route.as_deref())?;
    validate_task_message_sender(&actor_scope, &from_actor_id)?;
    let resolved_to_actor_id = resolve_task_message_target(&actor_scope, &route, to_actor_id)?;
    let payload = ensure_task_message_correlation_id(payload);
    let message = state
        .teams
        .append_task_conversation_message(
            &task_id,
            &from_actor_id,
            resolved_to_actor_id.as_deref(),
            &route,
            payload,
        )
        .await
        .map_err(map_team_internal_error)?;
    if from_actor_id == actor_scope.user_actor_id
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
    let user = require_user(&headers, &state).await?;
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

async fn compile_team_task_run_preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, task_id)): Path<(String, String)>,
    Json(payload): Json<CompileTeamTaskRunPreviewRequest>,
) -> Result<Json<TeamTaskRunCompilePreviewResponse>, ApiError> {
    let user = require_user(&headers, &state).await?;
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
    let user = require_user(&headers, &state).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
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
    let user = require_user(&headers, &state).await?;
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
    let user = require_user(&headers, &state).await?;
    let run = load_run_for_user(&state, &run_id, &user).await?;
    Ok(Json(run))
}

async fn cancel_team_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Result<Json<TeamRunRecord>, ApiError> {
    let user = require_user(&headers, &state).await?;
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
    let user = require_user(&headers, &state).await?;
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
    let user = require_user(&headers, &state).await?;
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
    let user = require_user(&headers, &state).await?;
    let (run, team) = load_run_and_team_for_user(&state, &run_id, &user).await?;
    validate_team_spec(&team.spec)?;

    let spec_obj = team
        .spec
        .as_object()
        .ok_or_else(|| ApiError::bad_request("spec must be an object"))?;
    let member_specs = parse_member_specs(spec_obj.get("members"))?;
    let leader_member_id = parse_spec_leader_member_id(spec_obj, &member_specs)?;

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

    let event_limit = query.event_limit.unwrap_or(120).clamp(1, 500);
    let message_limit = query.message_limit.unwrap_or(120).clamp(1, 500);
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
    let run_member_overrides = extract_run_member_profile_overrides(&run.input);

    let mut members = Vec::with_capacity(member_specs.len());
    for mut member in member_specs {
        let latest_step = latest_step_by_member
            .get(member.member_id.as_str())
            .cloned();
        let session_status = state
            .teams
            .get_live_member_session(member.member_id.as_str())
            .await
            .map_err(map_team_internal_error)?
            .map(|(_session_id, status)| status);
        let status = latest_step
            .as_ref()
            .map(|step| step_status_to_str(&step.status).to_string())
            .unwrap_or_else(|| "idle".to_string());
        let role = if leader_member_id.as_deref() == Some(member.member_id.as_str()) {
            "leader"
        } else {
            member.role.as_str()
        };
        let mut prompt = member.prompt.clone();
        let mut skills = member.skills.clone();
        if let Some(override_item) = run_member_overrides.get(member.member_id.as_str()) {
            if let Some(prompt_append) = override_item.prompt_append.as_deref() {
                prompt = Some(merge_prompt_append(prompt.as_deref(), Some(prompt_append)));
            }
            if let Some(description) = override_item.description.as_deref() {
                member.description = Some(description.to_string());
            }
            let _added = merge_skills_unique(&mut skills, &override_item.skills_add);
        }
        members.push(TeamMemberSnapshot {
            member_id: member.member_id.clone(),
            role: role.to_string(),
            model: member.model.clone(),
            description: member.description.clone(),
            prompt,
            skills,
            pending_inbox_count: pending_counts.get(&member.member_id).copied().unwrap_or(0),
            status,
            latest_step,
            session_status,
        });
    }

    let mailbox = TeamMailboxSnapshot {
        pending: status_counts.get("pending").copied().unwrap_or(0),
        delivered: status_counts.get("delivered").copied().unwrap_or(0),
        dead_letter: status_counts.get("dead_letter").copied().unwrap_or(0),
        recent_messages,
    };

    Ok(Json(TeamRunSnapshotResponse {
        run,
        team,
        leader_member_id,
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
    let user = require_user(&headers, &state).await?;
    ensure_run_access_for_user(&state, &run_id, &user).await?;
    let limit = query.limit.unwrap_or(500).clamp(1, 1000);
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
    let user = require_user(&headers, &state).await?;
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
    let user = require_user(&headers, &state).await?;
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
    let user = require_user(&headers, &state).await?;
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
    let user = require_user(&headers, &state).await?;
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
    Ok(Json(step))
}

async fn complete_team_run_step(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((run_id, step_id)): Path<(String, String)>,
    Json(payload): Json<CompleteTeamRunStepRequest>,
) -> Result<Json<TeamStepRecord>, ApiError> {
    let user = require_user(&headers, &state).await?;
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
    let user = require_user(&headers, &state).await?;
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
    let user = require_user(&headers, &state).await?;
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
    let user = require_user(&headers, &state).await?;
    ensure_run_access_for_user(&state, &run_id, &user).await?;
    ensure_step_in_run(&state, &run_id, &step_id).await?;
    let step = state
        .teams
        .resume_step(&step_id, payload.input)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(step))
}

async fn send_team_run_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Json(payload): Json<SendTeamRunMessageRequest>,
) -> Result<Json<TeamActorMessageRecord>, ApiError> {
    let user = require_user(&headers, &state).await?;
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
    let prompt = build_actor_mailbox_immediate_hint_prompt(run_id, plan.reason);
    let reason_label = match plan.reason {
        crate::team::ActorMailboxImmediateHintReason::DirectAgentMessage => "direct_agent_message",
        crate::team::ActorMailboxImmediateHintReason::LeaderChannelMention => {
            "leader_channel_mention"
        }
    };
    let mut sent_targets = Vec::new();
    let mut failed_targets = Vec::new();
    for target_actor_id in &plan.target_actor_ids {
        match state
            .agents
            .send_input(target_actor_id, &prompt, None, None)
            .await
        {
            Ok(()) => sent_targets.push(target_actor_id.clone()),
            Err(err) => {
                tracing::debug!(
                    run_id = %run_id,
                    actor_id = %target_actor_id,
                    reason = ?plan.reason,
                    "skip mailbox hint push because agent input is unavailable: {}",
                    err
                );
                failed_targets.push(target_actor_id.clone());
            }
        }
    }
    append_actor_mailbox_type_hint_event(
        state,
        run_id,
        serde_json::json!({
            "status": if failed_targets.is_empty() { "sent" } else if sent_targets.is_empty() { "send_failed" } else { "partial" },
            "message_id": send_result.message_id,
            "reason": reason_label,
            "target_actor_ids": plan.target_actor_ids,
            "sent_actor_ids": sent_targets,
            "failed_actor_ids": failed_targets,
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
    let reason = if reason == "leader_channel_mention" {
        crate::team::ActorMailboxImmediateHintReason::LeaderChannelMention
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
    let user = require_user(&headers, &state).await?;
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
        let actor_messages = actor_inbox_with_auto_ack(
            &service,
            ActorInboxRequest {
                run_id: run_id.clone(),
                actor_id,
                cursor: query.after_id,
                limit: Some(limit),
                states: states.clone(),
            },
        )
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
    let user = require_user(&headers, &state).await?;
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
    let skills_add = payload_obj
        .get("skills_add")
        .map(parse_profile_patch_skills_add)
        .transpose()?
        .unwrap_or_default();
    if prompt_append.is_none() && description.is_none() && skills_add.is_empty() {
        return Err(ApiError::bad_request(
            "profile_patch_proposal requires prompt_append and/or description and/or skills_add",
        ));
    }

    Ok(Some(ProfilePatchProposal {
        target,
        member_id,
        prompt_append,
        description,
        skills_add,
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

fn parse_profile_patch_skills_add(value: &Value) -> Result<Vec<String>, ApiError> {
    let items = value.as_array().ok_or_else(|| {
        ApiError::bad_request("profile_patch_proposal.skills_add must be an array")
    })?;
    let mut out = Vec::with_capacity(items.len());
    let mut seen = HashSet::with_capacity(items.len());
    for item in items {
        let skill = item
            .as_str()
            .map(str::trim)
            .filter(|skill| !skill.is_empty())
            .ok_or_else(|| {
                ApiError::bad_request(
                    "profile_patch_proposal.skills_add entries must be non-empty strings",
                )
            })?;
        if !seen.insert(skill.to_string()) {
            continue;
        }
        out.push(skill.to_string());
    }
    Ok(out)
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
                        "skills_add": proposal.skills_add,
                        "before": {
                            "prompt": before.prompt_append,
                            "description": before.description,
                            "skills": before.skills_add,
                        },
                        "after": {
                            "prompt": after.prompt_append,
                            "description": after.description,
                            "skills": after.skills_add,
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
                        "skills_add": proposal.skills_add,
                        "before": {
                            "prompt": before.prompt_append,
                            "description": before.description,
                            "skills": before.skills_add,
                        },
                        "after": {
                            "prompt": after.prompt_append,
                            "description": after.description,
                            "skills": after.skills_add,
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
    if !proposal.skills_add.is_empty() {
        let mut current = parse_skills_array(member_obj.get("skills"))?;
        let _added = merge_skills_unique(&mut current, &proposal.skills_add);
        member_obj.insert(
            "skills".to_string(),
            Value::Array(current.into_iter().map(Value::String).collect()),
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

    if !proposal.skills_add.is_empty() {
        let mut current = parse_skills_array(member_obj.get("skills_add"))?;
        let _added = merge_skills_unique(&mut current, &proposal.skills_add);
        member_obj.insert(
            "skills_add".to_string(),
            Value::Array(current.into_iter().map(Value::String).collect()),
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

fn parse_skills_array(value: Option<&Value>) -> Result<Vec<String>, ApiError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Some(items) = value.as_array() else {
        return Err(ApiError::bad_request(
            "profile patch skills field must be an array of strings",
        ));
    };
    let mut out = Vec::with_capacity(items.len());
    let mut seen = HashSet::with_capacity(items.len());
    for item in items {
        let Some(skill) = item.as_str().map(str::trim).filter(|item| !item.is_empty()) else {
            return Err(ApiError::bad_request(
                "profile patch skills field must contain non-empty strings",
            ));
        };
        if !seen.insert(skill.to_string()) {
            continue;
        }
        out.push(skill.to_string());
    }
    Ok(out)
}

fn merge_skills_unique(current: &mut Vec<String>, incoming: &[String]) -> Vec<String> {
    let mut seen = current.iter().cloned().collect::<HashSet<_>>();
    let mut added = Vec::new();
    for skill in incoming {
        if seen.insert(skill.to_string()) {
            current.push(skill.to_string());
            added.push(skill.to_string());
        }
    }
    added
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
        skills_add: parse_skills_array(member_obj.get("skills"))?,
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
        let skills_add = member_obj
            .get("skills_add")
            .and_then(Value::as_array)
            .map(|items| {
                let mut out = Vec::with_capacity(items.len());
                let mut seen = HashSet::with_capacity(items.len());
                for item in items {
                    if let Some(skill) =
                        item.as_str().map(str::trim).filter(|item| !item.is_empty())
                        && seen.insert(skill.to_string())
                    {
                        out.push(skill.to_string());
                    }
                }
                out
            })
            .unwrap_or_default();
        if prompt_append.is_none() && description.is_none() && skills_add.is_empty() {
            continue;
        }
        out.insert(
            member_id.clone(),
            MemberProfileOverride {
                prompt_append,
                description,
                skills_add,
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

fn normalize_optional_idempotency_key(value: Option<&str>) -> Result<Option<String>, ApiError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(
            "idempotency_key must be a non-empty string",
        ));
    }
    if trimmed.len() > 128 {
        return Err(ApiError::bad_request(
            "idempotency_key must be at most 128 characters",
        ));
    }
    Ok(Some(trimmed.to_string()))
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

fn normalize_task_status(value: Option<&str>) -> Result<TeamTaskStatus, ApiError> {
    match value
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
    {
        Some("open") => Ok(TeamTaskStatus::Open),
        Some("in_progress") => Ok(TeamTaskStatus::InProgress),
        Some("in_review") => Ok(TeamTaskStatus::InReview),
        Some("completed") => Ok(TeamTaskStatus::Completed),
        Some("canceled") => Ok(TeamTaskStatus::Canceled),
        _ => Err(ApiError::bad_request(&format!(
            "status must be one of: {}",
            TEAM_TASK_STATUS_VALUES.join(", ")
        ))),
    }
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

fn ensure_task_message_correlation_id(payload: Value) -> Value {
    let Value::Object(mut payload_obj) = payload else {
        return payload;
    };
    let existing = payload_obj
        .get("correlation_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let correlation_id = existing.unwrap_or_else(|| Uuid::now_v7().to_string());
    payload_obj.insert("correlation_id".to_string(), Value::String(correlation_id));
    Value::Object(payload_obj)
}

#[derive(Debug)]
struct TaskActorScope {
    user_actor_id: String,
    member_ids: HashSet<String>,
    member_order: Vec<String>,
    leader_member_id: Option<String>,
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
    let leader_member_id = parse_spec_leader_member_id(spec_obj, &member_specs)?;
    Ok(TaskActorScope {
        user_actor_id: canonical_user_actor_id(user),
        member_ids,
        member_order,
        leader_member_id,
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
            let to_actor_id = to_actor_id.ok_or_else(|| {
                ApiError::bad_request("to_actor_id is required when route=to_member")
            })?;
            if !actor_scope.member_ids.contains(to_actor_id.as_str()) {
                return Err(ApiError::bad_request(
                    "to_actor_id must reference spec.members[].member_id when route=to_member",
                ));
            }
            Ok(Some(to_actor_id))
        }
        "to_leader" => {
            let leader_member_id = actor_scope.leader_member_id.as_deref().ok_or_else(|| {
                ApiError::bad_request("route=to_leader requires a leader member in spec.members")
            })?;
            match to_actor_id {
                None => Ok(Some(leader_member_id.to_string())),
                Some(to_actor_id) => {
                    if to_actor_id != leader_member_id {
                        return Err(ApiError::bad_request(
                            "to_actor_id must equal leader member_id when route=to_leader",
                        ));
                    }
                    Ok(Some(to_actor_id))
                }
            }
        }
        _ => Err(ApiError::bad_request("unsupported route")),
    }
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
    for to_actor_id in recipient_ids {
        let forwarded_payload = build_task_mailbox_forward_payload(
            &message.payload,
            message,
            mention_ids.as_slice(),
            delivery_scope,
        );
        let send_result = state
            .teams
            .actor_mailbox_service()
            .actor_send(ActorSendRequest {
                run_id: run.id.clone(),
                from_actor_id: mailbox_sender.clone(),
                from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
                to_actor_id: Some(to_actor_id.clone()),
                channel_id: None,
                to_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
                channel: Some("default".to_string()),
                transport: Some(TeamActorMessageTransport::Local),
                route: None,
                payload: forwarded_payload,
                idempotency_key: Some(format!(
                    "task:{}:{}:{}",
                    message.task_id, message.message_id, to_actor_id
                )),
            })
            .await
            .map_err(map_actor_service_api_error)?;
        if let Err(err) =
            maybe_notify_actor_new_mailbox_message_type(state, &run.id, &send_result).await
        {
            tracing::warn!(
                run_id = %run.id,
                to_actor_id = %to_actor_id,
                message_id = send_result.message_id,
                "task mailbox type hint notify failed: {}",
                err
            );
        }
    }
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
    let leader_member_id = actor_scope
        .leader_member_id
        .as_deref()
        .filter(|member_id| actor_scope.member_ids.contains(*member_id))
        .map(str::to_string);
    if leader_member_id.is_some() {
        return leader_member_id;
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
        "to_member" | "to_leader" => to_actor_id
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
    if let Some(explicit_mentions) = payload.get("mention_actor_ids").and_then(Value::as_array) {
        for value in explicit_mentions {
            if let Some(candidate) = value.as_str() {
                push_member_mention(candidate, member_ids, &mut seen, &mut out);
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
    let leader_member_id = parse_spec_leader_member_id(spec_obj, &member_specs)?;
    let step_template =
        compile_task_step_template(spec_obj, &member_specs, leader_member_id.as_deref())?;
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

    let role_assignments =
        build_task_role_assignments(&step_template, &member_specs, leader_member_id.as_deref());
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
    leader_member_id: Option<&str>,
) -> Result<Vec<TeamCompiledStepTemplate>, ApiError> {
    let leader_member_id = resolve_effective_leader_member_id(leader_member_id, member_specs)
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
                    leader_member_id.as_str(),
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
            .filter(|member_id| *member_id != leader_member_id.as_str())
            .map(str::to_string)
            .collect::<Vec<_>>();
        generated_steps = build_default_team_steps(leader_member_id.as_str(), &worker_member_ids);
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
        });
    }
    Ok(out)
}

fn resolve_compiled_member_role(
    member_id: &str,
    base_role: &str,
    leader_member_id: &str,
) -> String {
    if member_id == leader_member_id {
        "leader".to_string()
    } else {
        base_role.to_string()
    }
}

fn resolve_effective_leader_member_id<'a>(
    leader_member_id: Option<&'a str>,
    member_specs: &'a [TeamMemberSpec],
) -> Option<&'a str> {
    leader_member_id
        .or_else(|| {
            member_specs
                .iter()
                .find(|member| member.role == "leader")
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
    leader_member_id: Option<&str>,
) -> Vec<TeamCompiledRoleAssignment> {
    let leader_member_id = resolve_effective_leader_member_id(leader_member_id, member_specs);
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
            role: if leader_member_id == Some(member.member_id.as_str()) {
                "leader".to_string()
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
        "leader" => 0,
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
    skills: Vec<String>,
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
    skills_add: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct MemberProfileOverride {
    prompt_append: Option<String>,
    description: Option<String>,
    skills_add: Vec<String>,
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

fn inject_team_spec_defaults(
    spec_obj: &mut serde_json::Map<String, Value>,
) -> Result<(), ApiError> {
    let member_specs = parse_member_specs(spec_obj.get("members"))?;
    if member_specs.is_empty() {
        spec_obj.remove("leader_member_id");
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
    let leader_member_id = parse_spec_leader_member_id(spec_obj, &member_specs)?;
    if let Some(leader_id) = leader_member_id.as_deref()
        && !spec_obj.contains_key("leader_member_id")
    {
        spec_obj.insert(
            "leader_member_id".to_string(),
            Value::String(leader_id.to_string()),
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
            let defaults = default_team_skills_for_role(&member_spec.role);
            let required = required_team_skills_for_role(&member_spec.role);
            let base_skills = if is_missing_or_null(member_obj.get("skills")) {
                defaults.iter().map(|skill| (*skill).to_string()).collect()
            } else {
                member_spec.skills.clone()
            };
            let normalized_skills = ensure_required_role_skills(base_skills, required);
            member_obj.insert(
                "skills".to_string(),
                Value::Array(
                    normalized_skills
                        .into_iter()
                        .map(Value::String)
                        .collect::<Vec<_>>(),
                ),
            );
        }
    }

    let entrypoint_member_id = spec_obj
        .get("entrypoint")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let leader_matches_entrypoint = match (leader_member_id.as_deref(), entrypoint_member_id) {
        (Some(leader), Some(entrypoint)) => leader == entrypoint,
        _ => true,
    };
    let should_generate_steps = spec_obj.get("steps").is_none()
        && entrypoint_member_id.is_some_and(|entrypoint| member_ids.contains(entrypoint))
        && leader_matches_entrypoint;

    if should_generate_steps {
        let leader_id = leader_member_id
            .or_else(|| entrypoint_member_id.map(str::to_string))
            .or_else(|| member_specs.first().map(|member| member.member_id.clone()))
            .ok_or_else(|| ApiError::bad_request("spec.members must not be empty"))?;
        let worker_member_ids = member_specs
            .iter()
            .map(|member| member.member_id.clone())
            .filter(|member_id| member_id != &leader_id)
            .collect::<Vec<_>>();
        let steps = build_default_team_steps(&leader_id, &worker_member_ids);
        spec_obj.insert("steps".to_string(), Value::Array(steps));
        spec_obj.insert(
            "entrypoint".to_string(),
            Value::String(DEFAULT_TEAM_PLAN_STEP_KEY.to_string()),
        );
    }

    Ok(())
}

fn default_team_skills_for_role(role: &str) -> &'static [&'static str] {
    if role == "leader" {
        DEFAULT_TEAM_LEADER_SKILLS.as_slice()
    } else {
        DEFAULT_TEAM_WORKER_SKILLS.as_slice()
    }
}

fn required_team_skills_for_role(role: &str) -> &'static [&'static str] {
    if role == "leader" {
        REQUIRED_TEAM_LEADER_SKILLS.as_slice()
    } else {
        REQUIRED_TEAM_WORKER_SKILLS.as_slice()
    }
}

fn ensure_required_role_skills(mut skills: Vec<String>, required: &[&str]) -> Vec<String> {
    let mut deduped = Vec::with_capacity(skills.len() + required.len());
    let mut seen = HashSet::with_capacity(skills.len() + required.len());
    for skill in required {
        if seen.insert((*skill).to_string()) {
            deduped.push((*skill).to_string());
        }
    }
    for skill in skills.drain(..) {
        if seen.insert(skill.clone()) {
            deduped.push(skill);
        }
    }
    deduped
}

fn is_missing_or_null(value: Option<&Value>) -> bool {
    match value {
        None => true,
        Some(Value::Null) => true,
        Some(_) => false,
    }
}

fn build_default_team_steps(leader_member_id: &str, worker_member_ids: &[String]) -> Vec<Value> {
    let mut steps = Vec::with_capacity(worker_member_ids.len() + 2);
    steps.push(serde_json::json!({
        "step_key": DEFAULT_TEAM_PLAN_STEP_KEY,
        "member_id": leader_member_id,
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
        "member_id": leader_member_id,
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
    let leader_member_id = parse_spec_leader_member_id(spec_obj, &member_specs)?;
    let entrypoint = spec_obj
        .get("entrypoint")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if member_specs.is_empty() {
        if leader_member_id.is_some() {
            return Err(ApiError::bad_request(
                "spec.leader_member_id must be omitted until spec.members is configured",
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
    } else if let Some(leader_id) = leader_member_id.as_deref()
        && entrypoint != leader_id
    {
        return Err(ApiError::bad_request(
            "spec.entrypoint must equal leader_member_id when spec.steps is omitted",
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
        let skills = parse_optional_member_skills(member.get("skills"))?;

        out.push(TeamMemberSpec {
            member_id,
            role,
            model,
            prompt,
            description,
            skills,
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
            "spec.members[].role is required and must be 'leader' or 'worker'",
        ));
    };
    let raw = value
        .as_str()
        .ok_or_else(|| {
            ApiError::bad_request(
                "spec.members[].role is required and must be 'leader' or 'worker'",
            )
        })?
        .trim();
    if raw.is_empty() {
        return Err(ApiError::bad_request(
            "spec.members[].role is required and must be 'leader' or 'worker'",
        ));
    }
    if raw != "leader" && raw != "worker" {
        return Err(ApiError::bad_request(
            "spec.members[].role is required and must be 'leader' or 'worker'",
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

fn parse_optional_member_skills(value: Option<&Value>) -> Result<Vec<String>, ApiError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let skills = value
        .as_array()
        .ok_or_else(|| ApiError::bad_request("spec.members[].skills must be an array"))?;
    let mut out = Vec::with_capacity(skills.len());
    let mut seen = HashSet::with_capacity(skills.len());
    for skill in skills {
        let skill = skill
            .as_str()
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .ok_or_else(|| {
                ApiError::bad_request("spec.members[].skills entries must be non-empty strings")
            })?;
        if !seen.insert(skill.to_string()) {
            return Err(ApiError::bad_request(
                "spec.members[].skills must not contain duplicates",
            ));
        }
        out.push(skill.to_string());
    }
    Ok(out)
}

fn parse_spec_leader_member_id(
    spec_obj: &serde_json::Map<String, Value>,
    member_specs: &[TeamMemberSpec],
) -> Result<Option<String>, ApiError> {
    let member_ids = member_specs
        .iter()
        .map(|member| member.member_id.as_str())
        .collect::<HashSet<_>>();
    let explicit_leader = match spec_obj.get("leader_member_id") {
        None => None,
        Some(value) => {
            let leader = value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ApiError::bad_request("spec.leader_member_id must be a non-empty string")
                })?;
            if !member_ids.contains(leader) {
                return Err(ApiError::bad_request(
                    "spec.leader_member_id must reference spec.members[].member_id",
                ));
            }
            Some(leader.to_string())
        }
    };

    let member_role_leaders = member_specs
        .iter()
        .filter_map(|member| match member.role.as_str() {
            "leader" => Some(member.member_id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if member_role_leaders.len() > 1 {
        return Err(ApiError::bad_request(
            "spec.members[].role may include at most one 'leader'",
        ));
    }

    if let Some(explicit) = explicit_leader.as_deref() {
        if let Some(role_leader) = member_role_leaders.first()
            && explicit != *role_leader
        {
            return Err(ApiError::bad_request(
                "spec.leader_member_id must match spec.members[].role='leader'",
            ));
        }
        return Ok(Some(explicit.to_string()));
    }

    if let Some(role_leader) = member_role_leaders.first() {
        return Ok(Some((*role_leader).to_string()));
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
