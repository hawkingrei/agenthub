use agenthub_agent_domain::loop_runtime::validate_loop_id;
use agenthub_agent_domain::loop_scheduling::{
    LoopRegistration, LoopRegistrationDetail, LoopRegistrationPage, LoopRegistrationReceipt,
    LoopScheduleRequest,
};
use agenthub_db::loop_runtime::LoopStore;

use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScheduleListQuery {
    after_registration_id: Option<String>,
    limit: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScheduleDetailQuery {
    after_firing_cursor: Option<i64>,
    limit: Option<u32>,
}

pub(super) async fn create_loop_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id)): Path<(String, String)>,
    Json(request): Json<LoopScheduleRequest>,
) -> Result<Json<LoopRegistrationReceipt>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &team, &user, &["owner"]).await?;
    super::loop_configuration::require_member(&team, &actor_id)?;
    request
        .validate()
        .map_err(|_| ApiError::bad_request("invalid bounded schedule"))?;
    let context = crate::team::loop_context::LoopSchedulingContext {
        user_id: Some(user.id),
        ..Default::default()
    };
    let receipt = crate::team::loop_context::with_scheduling_context(
        context,
        state
            .teams
            .request_loop_schedule(&team_id, &actor_id, &request),
    )
    .await
    .map_err(map_team_internal_error)?;
    Ok(Json(receipt))
}

pub(super) async fn list_loop_schedules(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id)): Path<(String, String)>,
    Query(query): Query<ScheduleListQuery>,
) -> Result<Json<LoopRegistrationPage>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    super::loop_configuration::require_member(&team, &actor_id)?;
    if let Some(id) = &query.after_registration_id {
        validate_loop_id(id).map_err(|_| ApiError::bad_request("invalid registration cursor"))?;
    }
    let page = LoopStore::new(state.db.clone())
        .registrations(
            &team_id,
            &actor_id,
            query.after_registration_id.as_deref(),
            page_limit(query.limit)?,
        )
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(page))
}

pub(super) async fn get_loop_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id, id)): Path<(String, String, String)>,
    Query(query): Query<ScheduleDetailQuery>,
) -> Result<Json<LoopRegistrationDetail>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    super::loop_configuration::require_member(&team, &actor_id)?;
    if query.after_firing_cursor.is_some_and(|cursor| cursor < 0) {
        return Err(ApiError::bad_request("invalid firing cursor"));
    }
    let detail = LoopStore::new(state.db.clone())
        .registration_detail(
            &team_id,
            &id,
            query.after_firing_cursor,
            page_limit(query.limit)?,
        )
        .await
        .map_err(map_team_internal_error)?;
    if detail.registration.input.actor_id != actor_id {
        return Err(ApiError::not_found("registration not found for member"));
    }
    Ok(Json(detail))
}

pub(super) async fn revoke_loop_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id, id)): Path<(String, String, String)>,
) -> Result<Json<LoopRegistration>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &team, &user, &["owner"]).await?;
    super::loop_configuration::require_member(&team, &actor_id)?;
    let store = LoopStore::new(state.db.clone());
    let registration = store
        .registration(&team_id, &id)
        .await
        .map_err(map_team_internal_error)?
        .ok_or_else(|| ApiError::not_found("registration not found"))?;
    if registration.input.actor_id != actor_id {
        return Err(ApiError::not_found("registration not found for member"));
    }
    Ok(Json(
        store
            .revoke_schedule(&team_id, &id, chrono::Utc::now().timestamp())
            .await
            .map_err(map_team_internal_error)?,
    ))
}

fn page_limit(limit: Option<u32>) -> Result<u32, ApiError> {
    let limit = limit.unwrap_or(64);
    if !(1..=256).contains(&limit) {
        return Err(ApiError::bad_request("limit must be between 1 and 256"));
    }
    Ok(limit)
}
