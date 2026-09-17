use std::collections::BTreeSet;

use agenthub_agent_domain::app_tools::valid_credential_reference;
use agenthub_auth_domain::UserCapability;
use agenthub_db::app_registry::{AppEventKey, AppEventRoute, AppEventRouteUpdate, AppRegistry};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
};
use chrono::Utc;
use serde::Deserialize;

use crate::{api::authz::require_capability, state::AppState};

use super::{
    ApiError, Revision, bindings, payload, registration::owned, revision, scopes, store_error,
};

pub(super) async fn event_audit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(app_id): Path<String>,
) -> Result<Json<Vec<agenthub_db::app_registry::AppEventDenial>>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let registry = AppRegistry::new(state.db);
    owned(&registry, &app_id, &user.id).await?;
    Ok(Json(
        registry.event_denials(&app_id).await.map_err(store_error)?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct KeyConfiguration {
    expected_version: i64,
    credential_env: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct KeyRevocation {
    expected_version: i64,
}

pub(super) async fn get_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(app_id): Path<String>,
) -> Result<Json<AppEventKey>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let registry = AppRegistry::new(state.db);
    owned(&registry, &app_id, &user.id).await?;
    Ok(Json(
        registry
            .event_key(&app_id)
            .await
            .map_err(store_error)?
            .ok_or_else(|| ApiError::not_found("app event key not configured"))?,
    ))
}

pub(super) async fn configure_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(app_id): Path<String>,
    request: Result<Json<KeyConfiguration>, JsonRejection>,
) -> Result<Json<AppEventKey>, ApiError> {
    // App ownership alone must not permit selecting daemon environment secrets.
    require_capability(&headers, &state, UserCapability::InstanceConfigure).await?;
    let request = payload(request)?;
    revision(request.expected_version, true)?;
    if !valid_credential_reference(&request.credential_env) {
        return Err(ApiError::bad_request("invalid app event key reference"));
    }
    Ok(Json(
        AppRegistry::new(state.db)
            .configure_event_key(
                &app_id,
                request.expected_version,
                Some(&request.credential_env),
                Utc::now().timestamp(),
            )
            .await
            .map_err(store_error)?,
    ))
}

pub(super) async fn revoke_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(app_id): Path<String>,
    request: Result<Json<KeyRevocation>, JsonRejection>,
) -> Result<Json<AppEventKey>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let registry = AppRegistry::new(state.db);
    owned(&registry, &app_id, &user.id).await?;
    let request = payload(request)?;
    revision(request.expected_version, false)?;
    Ok(Json(
        registry
            .configure_event_key(
                &app_id,
                request.expected_version,
                None,
                Utc::now().timestamp(),
            )
            .await
            .map_err(store_error)?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RouteConfiguration {
    expected_revision: i64,
    classes: BTreeSet<String>,
}

pub(super) async fn get_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id, app_id)): Path<(String, String, String)>,
) -> Result<Json<AppEventRoute>, ApiError> {
    bindings::inspect(&state, &headers, &team_id).await?;
    Ok(Json(
        AppRegistry::new(state.db)
            .event_route(&app_id, &team_id, &actor_id)
            .await
            .map_err(store_error)?
            .ok_or_else(|| ApiError::not_found("app event route not found"))?,
    ))
}

pub(super) async fn configure_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id, app_id)): Path<(String, String, String)>,
    request: Result<Json<RouteConfiguration>, JsonRejection>,
) -> Result<Json<AppEventRoute>, ApiError> {
    bindings::owner(&state, &headers, &team_id).await?;
    let request = payload(request)?;
    revision(request.expected_revision, true)?;
    scopes(&request.classes)?;
    Ok(Json(
        AppRegistry::new(state.db)
            .configure_event_route(
                AppEventRouteUpdate {
                    app_id: &app_id,
                    team_id: &team_id,
                    actor_id: &actor_id,
                    expected_revision: request.expected_revision,
                    classes: &request.classes,
                },
                Utc::now().timestamp(),
            )
            .await
            .map_err(store_error)?,
    ))
}

pub(super) async fn revoke_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id, app_id)): Path<(String, String, String)>,
    request: Result<Json<Revision>, JsonRejection>,
) -> Result<Json<AppEventRoute>, ApiError> {
    bindings::owner(&state, &headers, &team_id).await?;
    let request = payload(request)?;
    revision(request.expected_revision, false)?;
    Ok(Json(
        AppRegistry::new(state.db)
            .revoke_event_route(
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
