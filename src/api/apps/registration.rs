use agenthub_agent_domain::app_tools::{AppConnection, AppManifest};
use agenthub_auth_domain::UserCapability;
use agenthub_db::app_registry::{AppRegistry, AppVersion, RegisterApp, RegisteredApp};
use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::Deserialize;

use crate::{api::authz::require_capability, state::AppState};

use super::{ApiError, Page, Revision, payload, revision, store_error};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Registration {
    name: String,
    owner_user_id: Option<String>,
    connection: AppConnection,
    manifest: AppManifest,
}

pub(super) async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<Json<Registration>, JsonRejection>,
) -> Result<(StatusCode, Json<RegisteredApp>), ApiError> {
    // An ordinary operator must never choose a daemon environment secret and exfiltration endpoint.
    let user = require_capability(&headers, &state, UserCapability::InstanceConfigure).await?;
    let request = payload(request)?;
    if request.name.trim().is_empty() || request.name.len() > 128 {
        return Err(ApiError::bad_request("invalid app name"));
    }
    let owner = request.owner_user_id.as_deref().unwrap_or(&user.id);
    agenthub_agent_domain::loop_runtime::validate_loop_id(owner)
        .map_err(|_| ApiError::bad_request("invalid app owner"))?;
    request
        .connection
        .validate()
        .map_err(|_| ApiError::bad_request("invalid app connection"))?;
    request
        .manifest
        .compile()
        .map_err(|_| ApiError::bad_request("invalid app manifest"))?;
    let app = AppRegistry::new(state.db)
        .register(
            RegisterApp {
                owner_user_id: owner,
                name: &request.name,
                connection: &request.connection,
                manifest: &request.manifest,
            },
            Utc::now().timestamp(),
        )
        .await
        .map_err(store_error)?;
    Ok((StatusCode::CREATED, Json(app)))
}

pub(super) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(page): Query<Page>,
) -> Result<Json<Vec<RegisteredApp>>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let limit = page.validate()?;
    Ok(Json(
        AppRegistry::new(state.db)
            .list_owned(&user.id, page.after.as_deref(), limit)
            .await
            .map_err(store_error)?,
    ))
}

pub(super) async fn get_app(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(app_id): Path<String>,
) -> Result<Json<RegisteredApp>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    Ok(Json(
        owned(&AppRegistry::new(state.db), &app_id, &user.id).await?,
    ))
}

pub(super) async fn get_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((app_id, version)): Path<(String, i64)>,
) -> Result<Json<AppVersion>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let store = AppRegistry::new(state.db);
    owned(&store, &app_id, &user.id).await?;
    Ok(Json(
        store
            .version(&app_id, version)
            .await
            .map_err(store_error)?
            .ok_or_else(|| ApiError::not_found("app version not found"))?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Publication {
    expected_revision: i64,
    manifest: AppManifest,
}

pub(super) async fn publish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(app_id): Path<String>,
    request: Result<Json<Publication>, JsonRejection>,
) -> Result<(StatusCode, Json<AppVersion>), ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let store = AppRegistry::new(state.db);
    owned(&store, &app_id, &user.id).await?;
    let request = payload(request)?;
    revision(request.expected_revision, false)?;
    request
        .manifest
        .compile()
        .map_err(|_| ApiError::bad_request("invalid app manifest"))?;
    let version = store
        .publish_version(
            &app_id,
            &user.id,
            request.expected_revision,
            &request.manifest,
            Utc::now().timestamp(),
        )
        .await
        .map_err(store_error)?;
    Ok((StatusCode::CREATED, Json(version)))
}

pub(super) async fn revoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(app_id): Path<String>,
    request: Result<Json<Revision>, JsonRejection>,
) -> Result<Json<RegisteredApp>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let store = AppRegistry::new(state.db);
    owned(&store, &app_id, &user.id).await?;
    let request = payload(request)?;
    revision(request.expected_revision, false)?;
    Ok(Json(
        store
            .revoke_app(
                &app_id,
                &user.id,
                request.expected_revision,
                Utc::now().timestamp(),
            )
            .await
            .map_err(store_error)?,
    ))
}

pub(super) async fn owned(
    store: &AppRegistry,
    app_id: &str,
    user_id: &str,
) -> Result<RegisteredApp, ApiError> {
    store
        .app(app_id)
        .await
        .map_err(store_error)?
        .filter(|app| app.owner_user_id == user_id)
        .ok_or_else(|| ApiError::not_found("app not found"))
}
