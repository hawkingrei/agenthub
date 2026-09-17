use std::collections::BTreeSet;

use agenthub_auth_domain::{UserCapability, UserRecord};
use agenthub_db::app_registry::{
    AppBindingUpdate, AppGrantUpdate, AppMemberBinding, AppRegistry, AppTeamGrant,
};
use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection},
    http::HeaderMap,
};
use chrono::Utc;
use serde::Deserialize;

use crate::{
    api::{
        authz::require_capability,
        teams::{load_team_for_user, require_teamspace_role},
    },
    state::AppState,
    team::TeamDefinitionRecord,
};

use super::{ApiError, Page, Revision, payload, revision, scopes, store_error};

async fn owner(
    state: &AppState,
    headers: &HeaderMap,
    team_id: &str,
) -> Result<UserRecord, ApiError> {
    let user = require_capability(headers, state, UserCapability::RuntimeOperate).await?;
    let team = load_team_for_user(state, team_id, &user).await?;
    // Legacy unowned rows are readable through compatibility paths, but cannot grant new authority.
    if team.owner_user_id.is_none() {
        return Err(ApiError::conflict(
            "app grants require an explicitly owned Team",
        ));
    }
    require_teamspace_role(state, &team, &user, &["owner"]).await?;
    Ok(user)
}

async fn inspect(
    state: &AppState,
    headers: &HeaderMap,
    team_id: &str,
) -> Result<TeamDefinitionRecord, ApiError> {
    let user = require_capability(headers, state, UserCapability::RuntimeInspect).await?;
    load_team_for_user(state, team_id, &user).await
}

pub(super) async fn get_grant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, app_id)): Path<(String, String)>,
) -> Result<Json<AppTeamGrant>, ApiError> {
    inspect(&state, &headers, &team_id).await?;
    Ok(Json(
        AppRegistry::new(state.db)
            .team_grant(&app_id, &team_id)
            .await
            .map_err(store_error)?
            .ok_or_else(|| ApiError::not_found("app grant not found"))?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Approval {
    expected_revision: i64,
    scopes: BTreeSet<String>,
}

pub(super) async fn approve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, app_id)): Path<(String, String)>,
    request: Result<Json<Approval>, JsonRejection>,
) -> Result<Json<AppTeamGrant>, ApiError> {
    let user = owner(&state, &headers, &team_id).await?;
    let request = payload(request)?;
    revision(request.expected_revision, true)?;
    scopes(&request.scopes)?;
    Ok(Json(
        AppRegistry::new(state.db)
            .approve_team(
                &user.id,
                AppGrantUpdate {
                    app_id: &app_id,
                    team_id: &team_id,
                    expected_revision: request.expected_revision,
                    scopes: &request.scopes,
                },
                Utc::now().timestamp(),
            )
            .await
            .map_err(store_error)?,
    ))
}

pub(super) async fn revoke_grant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, app_id)): Path<(String, String)>,
    request: Result<Json<Revision>, JsonRejection>,
) -> Result<Json<AppTeamGrant>, ApiError> {
    owner(&state, &headers, &team_id).await?;
    let request = payload(request)?;
    revision(request.expected_revision, false)?;
    Ok(Json(
        AppRegistry::new(state.db)
            .revoke_team_grant(
                &app_id,
                &team_id,
                request.expected_revision,
                Utc::now().timestamp(),
            )
            .await
            .map_err(store_error)?,
    ))
}

pub(super) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id)): Path<(String, String)>,
    Query(page): Query<Page>,
) -> Result<Json<Vec<AppMemberBinding>>, ApiError> {
    let team = inspect(&state, &headers, &team_id).await?;
    if !crate::api::teams::parse_member_ids(team.spec.get("members"))?.contains(&actor_id) {
        return Err(ApiError::not_found("team member not found"));
    }
    let limit = page.validate()?;
    Ok(Json(
        AppRegistry::new(state.db)
            .member_bindings(&team_id, &actor_id, page.after.as_deref(), limit)
            .await
            .map_err(store_error)?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Binding {
    expected_revision: i64,
    version: i64,
    scopes: BTreeSet<String>,
}

pub(super) async fn bind(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id, app_id)): Path<(String, String, String)>,
    request: Result<Json<Binding>, JsonRejection>,
) -> Result<Json<AppMemberBinding>, ApiError> {
    owner(&state, &headers, &team_id).await?;
    let request = payload(request)?;
    revision(request.expected_revision, true)?;
    scopes(&request.scopes)?;
    if request.version <= 0 {
        return Err(ApiError::bad_request("invalid app version"));
    }
    Ok(Json(
        AppRegistry::new(state.db)
            .bind_member(
                AppBindingUpdate {
                    app_id: &app_id,
                    team_id: &team_id,
                    actor_id: &actor_id,
                    version: request.version,
                    expected_revision: request.expected_revision,
                    scopes: &request.scopes,
                },
                Utc::now().timestamp(),
            )
            .await
            .map_err(store_error)?,
    ))
}

pub(super) async fn revoke_binding(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id, app_id)): Path<(String, String, String)>,
    request: Result<Json<Revision>, JsonRejection>,
) -> Result<Json<AppMemberBinding>, ApiError> {
    owner(&state, &headers, &team_id).await?;
    let request = payload(request)?;
    revision(request.expected_revision, false)?;
    Ok(Json(
        AppRegistry::new(state.db)
            .revoke_member_binding(
                &app_id,
                &team_id,
                &actor_id,
                request.expected_revision,
                Utc::now().timestamp(),
            )
            .await
            .map_err(store_error)?,
    ))
}
