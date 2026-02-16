use std::collections::{HashMap, HashSet, VecDeque};

use agenthub_team_actor::parse_actor_transport;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Error as SqlxError;

use crate::api::authz::require_user;
use crate::api::error::ApiError;
use crate::state::AppState;
use crate::team::{
    TEAM_RUN_STATUS_VALUES, TeamActorMessageRecord, TeamActorMessageTransport,
    TeamDefinitionConfig, TeamDefinitionRecord, TeamManager, TeamRunEventRecord, TeamRunRecord,
    TeamStepRecord, TeamStepStatus,
};

const TEAM_SPEC_VERSION_V1: i64 = 1;
const SQLITE_CONSTRAINT_UNIQUE_CODE: &str = "2067";
const MAX_TEAM_SPEC_STEPS: usize = 2048;

#[derive(Debug, Deserialize)]
pub struct CreateTeamRequest {
    pub name: String,
    pub description: Option<String>,
    pub spec: Value,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeamRunRequest {
    pub context_id: Option<String>,
    pub input: Option<Value>,
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
    pub remote_task_id: Option<String>,
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
    pub to_actor_id: String,
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

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", post(create_team).get(list_teams))
        .route("/:id", get(get_team))
        .route("/:id/runs", post(create_team_run).get(list_team_runs))
        .route("/runs/:run_id", get(get_team_run))
        .route("/runs/:run_id/cancel", post(cancel_team_run))
        .route("/runs/:run_id/snapshot", get(get_team_run_snapshot))
        .route("/runs/:run_id/events", get(list_team_run_events))
        .route(
            "/runs/:run_id/steps",
            post(submit_team_run_step).get(list_team_run_steps),
        )
        .route(
            "/runs/:run_id/steps/:step_id/start",
            post(start_team_run_step),
        )
        .route(
            "/runs/:run_id/steps/:step_id/complete",
            post(complete_team_run_step),
        )
        .route(
            "/runs/:run_id/steps/:step_id/fail",
            post(fail_team_run_step),
        )
        .route(
            "/runs/:run_id/steps/:step_id/input_required",
            post(set_team_run_step_input_required),
        )
        .route(
            "/runs/:run_id/steps/:step_id/resume",
            post(resume_team_run_step),
        )
        .route("/runs/:run_id/messages/send", post(send_team_run_message))
        .route("/runs/:run_id/messages/inbox", get(list_team_run_inbox))
        .route(
            "/runs/:run_id/messages/:message_id/ack",
            post(ack_team_run_message),
        )
        .with_state(state)
}

async fn create_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateTeamRequest>,
) -> Result<Json<TeamDefinitionRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad_request("team name is required"));
    }
    let mut spec = payload.spec;
    normalize_team_spec(&mut spec)?;
    validate_team_spec(&spec)?;
    let team = state
        .teams
        .create_team(TeamDefinitionConfig {
            name,
            description: payload.description,
            spec,
        })
        .await
        .map_err(map_create_team_error)?;
    Ok(Json(team))
}

async fn list_teams(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<TeamDefinitionRecord>>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let teams = state.teams.list_teams().await?;
    Ok(Json(teams))
}

async fn get_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Json<TeamDefinitionRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let team = state
        .teams
        .get_team(&team_id)
        .await
        .map_err(|err| map_not_found_error(err, "team not found"))?;
    Ok(Json(team))
}

async fn create_team_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Json(payload): Json<CreateTeamRunRequest>,
) -> Result<Json<TeamRunRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let team = state
        .teams
        .get_team(&team_id)
        .await
        .map_err(|err| map_not_found_error(err, "team not found"))?;
    validate_team_spec(&team.spec)?;
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
    let _user = require_user(&headers, &state).await?;
    state
        .teams
        .get_team(&team_id)
        .await
        .map_err(|err| map_not_found_error(err, "team not found"))?;
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
    let _user = require_user(&headers, &state).await?;
    let run = state
        .teams
        .get_run(&run_id)
        .await
        .map_err(|err| map_not_found_error(err, "run not found"))?;
    Ok(Json(run))
}

async fn cancel_team_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Result<Json<TeamRunRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let run = state
        .teams
        .cancel_run(&run_id)
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
    let _user = require_user(&headers, &state).await?;

    let run = state
        .teams
        .get_run(&run_id)
        .await
        .map_err(|err| map_not_found_error(err, "run not found"))?;
    let team = state
        .teams
        .get_team(&run.team_id)
        .await
        .map_err(|err| map_not_found_error(err, "team not found"))?;
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

    let mut members = Vec::with_capacity(member_specs.len());
    for member in member_specs {
        let latest_step = latest_step_by_member
            .get(member.member_id.as_str())
            .cloned();
        let session_status = if let Some(session_id) = latest_step
            .as_ref()
            .and_then(|step| step.remote_task_id.as_deref())
        {
            state
                .teams
                .get_agent_session_status(session_id)
                .await
                .map_err(map_team_internal_error)?
        } else {
            None
        };
        let status = latest_step
            .as_ref()
            .map(|step| step_status_to_str(&step.status).to_string())
            .unwrap_or_else(|| "idle".to_string());
        let role = if leader_member_id.as_deref() == Some(member.member_id.as_str()) {
            "leader"
        } else {
            member.role.as_deref().unwrap_or("worker")
        };
        members.push(TeamMemberSnapshot {
            member_id: member.member_id.clone(),
            role: role.to_string(),
            model: member.model.clone(),
            prompt: member.prompt.clone(),
            skills: member.skills.clone(),
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
    let _user = require_user(&headers, &state).await?;
    state
        .teams
        .get_run(&run_id)
        .await
        .map_err(|err| map_not_found_error(err, "run not found"))?;
    let limit = query.limit.unwrap_or(500).clamp(1, 1000);
    let events = state
        .teams
        .list_run_events(&run_id, limit, query.before_id)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(events))
}

async fn list_team_run_steps(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Result<Json<Vec<TeamStepRecord>>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    ensure_run_exists(&state, &run_id).await?;
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
    let _user = require_user(&headers, &state).await?;
    ensure_run_exists(&state, &run_id).await?;

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
    let _user = require_user(&headers, &state).await?;
    ensure_run_exists(&state, &run_id).await?;
    ensure_step_in_run(&state, &run_id, &step_id).await?;
    let remote_task_id = payload
        .remote_task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let step = state
        .teams
        .start_step(&step_id, remote_task_id)
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
    let _user = require_user(&headers, &state).await?;
    ensure_run_exists(&state, &run_id).await?;
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
    let _user = require_user(&headers, &state).await?;
    ensure_run_exists(&state, &run_id).await?;
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
    let _user = require_user(&headers, &state).await?;
    ensure_run_exists(&state, &run_id).await?;
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
    let _user = require_user(&headers, &state).await?;
    ensure_run_exists(&state, &run_id).await?;
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
    let _user = require_user(&headers, &state).await?;
    let (run, member_ids) = load_run_and_member_ids(&state, &run_id).await?;
    let from_actor_id = normalize_required_field(payload.from_actor_id, "from_actor_id")?;
    let to_actor_id = normalize_required_field(payload.to_actor_id, "to_actor_id")?;
    let channel = payload
        .channel
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("default")
        .to_string();
    let idempotency_key = normalize_optional_idempotency_key(payload.idempotency_key.as_deref())?;
    let transport = parse_message_transport(payload.transport.as_deref())?;
    validate_message_actors(
        &member_ids,
        &from_actor_id,
        &to_actor_id,
        &transport,
        payload.route.as_ref(),
    )?;
    let message = state
        .teams
        .send_actor_message(
            &run.id,
            &from_actor_id,
            &to_actor_id,
            &channel,
            transport,
            payload.route,
            payload.payload,
            idempotency_key.as_deref(),
        )
        .await
        .map_err(map_send_actor_message_error)?;
    Ok(Json(message))
}

async fn list_team_run_inbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Query(query): Query<ListTeamRunInboxQuery>,
) -> Result<Json<Vec<TeamActorMessageRecord>>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let (_run, member_ids) = load_run_and_member_ids(&state, &run_id).await?;
    let actor_id = normalize_required_field(query.actor_id, "actor_id")?;
    if !member_ids.contains(actor_id.as_str()) {
        return Err(ApiError::bad_request(
            "actor_id must reference spec.members[].member_id",
        ));
    }
    let limit = query.limit.unwrap_or(500).clamp(1, 1000);
    let messages = state
        .teams
        .list_actor_inbox(
            &run_id,
            &actor_id,
            limit,
            query.after_id,
            query.include_delivered.unwrap_or(false),
        )
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(messages))
}

async fn ack_team_run_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((run_id, message_id)): Path<(String, i64)>,
    Json(payload): Json<AckTeamRunMessageRequest>,
) -> Result<Json<TeamActorMessageRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let (_run, member_ids) = load_run_and_member_ids(&state, &run_id).await?;
    let actor_id = normalize_required_field(payload.actor_id, "actor_id")?;
    if !member_ids.contains(actor_id.as_str()) {
        return Err(ApiError::bad_request(
            "actor_id must reference spec.members[].member_id",
        ));
    }
    let message = state
        .teams
        .ack_actor_message(&run_id, &actor_id, message_id)
        .await
        .map_err(|err| map_not_found_error(err, "message not found"))?;
    Ok(Json(message))
}

async fn ensure_run_exists(state: &AppState, run_id: &str) -> Result<(), ApiError> {
    state
        .teams
        .get_run(run_id)
        .await
        .map_err(|err| map_not_found_error(err, "run not found"))?;
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

fn map_create_team_error(err: anyhow::Error) -> ApiError {
    if is_unique_team_name_violation(&err) {
        return ApiError::conflict("team name already exists");
    }
    map_team_internal_error(err)
}

fn map_submit_step_error(err: anyhow::Error) -> ApiError {
    if is_unique_step_attempt_violation(&err) {
        return ApiError::conflict("step already exists for run");
    }
    map_team_internal_error(err)
}

fn map_send_actor_message_error(err: anyhow::Error) -> ApiError {
    if TeamManager::is_actor_message_idempotency_conflict(&err) {
        return ApiError::conflict("idempotency_key conflicts with an existing message payload");
    }
    map_team_internal_error(err)
}

fn map_not_found_error(err: anyhow::Error, msg: &str) -> ApiError {
    if is_row_not_found(&err) {
        return ApiError::not_found(msg);
    }
    map_team_internal_error(err)
}

fn map_team_internal_error(err: anyhow::Error) -> ApiError {
    tracing::error!("team api internal error: {}", err);
    ApiError::from(anyhow::anyhow!("internal server error"))
}

fn is_row_not_found(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<SqlxError>(),
        Some(SqlxError::RowNotFound)
    )
}

fn is_unique_team_name_violation(err: &anyhow::Error) -> bool {
    is_unique_violation_for(err, "team_definitions.name")
}

fn is_unique_step_attempt_violation(err: &anyhow::Error) -> bool {
    is_unique_violation_for(
        err,
        "team_steps.run_id, team_steps.step_key, team_steps.attempt",
    )
}

fn is_unique_violation_for(err: &anyhow::Error, constraint: &str) -> bool {
    match err.downcast_ref::<SqlxError>() {
        Some(SqlxError::Database(db_err)) => {
            db_err.code().as_deref() == Some(SQLITE_CONSTRAINT_UNIQUE_CODE)
                && db_err.message().contains(constraint)
        }
        _ => false,
    }
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

async fn load_run_and_member_ids(
    state: &AppState,
    run_id: &str,
) -> Result<(TeamRunRecord, HashSet<String>), ApiError> {
    let run = state
        .teams
        .get_run(run_id)
        .await
        .map_err(|err| map_not_found_error(err, "run not found"))?;
    let team = state
        .teams
        .get_team(&run.team_id)
        .await
        .map_err(|err| map_not_found_error(err, "team not found"))?;
    let member_ids = parse_member_ids(team.spec.get("members"))?;
    Ok((run, member_ids))
}

fn parse_message_transport(raw: Option<&str>) -> Result<TeamActorMessageTransport, ApiError> {
    parse_actor_transport(raw)
        .map_err(|_| ApiError::bad_request("transport must be either 'local' or 'remote'"))
}

fn validate_message_actors(
    member_ids: &HashSet<String>,
    from_actor_id: &str,
    to_actor_id: &str,
    transport: &TeamActorMessageTransport,
    route: Option<&Value>,
) -> Result<(), ApiError> {
    if !member_ids.contains(from_actor_id) {
        return Err(ApiError::bad_request(
            "from_actor_id must reference spec.members[].member_id",
        ));
    }
    match transport {
        TeamActorMessageTransport::Local => {
            if !member_ids.contains(to_actor_id) {
                return Err(ApiError::bad_request(
                    "to_actor_id must reference spec.members[].member_id for local transport",
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
    role: Option<String>,
    model: Option<String>,
    prompt: Option<String>,
    skills: Vec<String>,
}

fn normalize_team_spec(spec: &mut Value) -> Result<(), ApiError> {
    let spec_obj = spec
        .as_object_mut()
        .ok_or_else(|| ApiError::bad_request("spec must be an object"))?;
    let version = parse_team_spec_version(spec_obj.get("spec_version"))?;
    spec_obj.insert("spec_version".to_string(), Value::from(version));
    Ok(())
}

fn validate_team_spec(spec: &Value) -> Result<(), ApiError> {
    let spec_obj = spec
        .as_object()
        .ok_or_else(|| ApiError::bad_request("spec must be an object"))?;
    let _ = parse_team_spec_version(spec_obj.get("spec_version"))?;
    let entrypoint = spec_obj
        .get("entrypoint")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::bad_request("spec.entrypoint is required"))?;
    let member_specs = parse_member_specs(spec_obj.get("members"))?;
    let member_ids = member_specs
        .iter()
        .map(|member| member.member_id.clone())
        .collect::<HashSet<_>>();
    let leader_member_id = parse_spec_leader_member_id(spec_obj, &member_specs)?;

    if let Some(steps_value) = spec_obj.get("steps") {
        validate_steps(entrypoint, steps_value, &member_ids)?;
    } else if !member_ids.contains(entrypoint) {
        return Err(ApiError::bad_request(
            "spec.entrypoint must reference spec.members[].member_id when spec.steps is omitted",
        ));
    } else if let Some(leader_id) = leader_member_id.as_deref() {
        if entrypoint != leader_id {
            return Err(ApiError::bad_request(
                "spec.entrypoint must equal leader_member_id when spec.steps is omitted",
            ));
        }
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
    if members.is_empty() {
        return Err(ApiError::bad_request("spec.members must not be empty"));
    }

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

        let role = parse_optional_member_role(member.get("role"))?;
        let model = parse_optional_member_text(member.get("model"), "model")?;
        let prompt = parse_optional_member_text(member.get("prompt"), "prompt")?;
        let skills = parse_optional_member_skills(member.get("skills"))?;

        out.push(TeamMemberSpec {
            member_id,
            role,
            model,
            prompt,
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

fn parse_optional_member_role(value: Option<&Value>) -> Result<Option<String>, ApiError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let raw = value
        .as_str()
        .ok_or_else(|| ApiError::bad_request("spec.members[].role must be 'leader' or 'worker'"))?
        .trim();
    if raw.is_empty() {
        return Err(ApiError::bad_request(
            "spec.members[].role must be 'leader' or 'worker'",
        ));
    }
    if raw != "leader" && raw != "worker" {
        return Err(ApiError::bad_request(
            "spec.members[].role must be 'leader' or 'worker'",
        ));
    }
    Ok(Some(raw.to_string()))
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
        .filter_map(|member| match member.role.as_deref() {
            Some("leader") => Some(member.member_id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if member_role_leaders.len() > 1 {
        return Err(ApiError::bad_request(
            "spec.members[].role may include at most one 'leader'",
        ));
    }

    if let Some(explicit) = explicit_leader.as_deref() {
        if let Some(role_leader) = member_role_leaders.first() {
            if explicit != *role_leader {
                return Err(ApiError::bad_request(
                    "spec.leader_member_id must match spec.members[].role='leader'",
                ));
            }
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
