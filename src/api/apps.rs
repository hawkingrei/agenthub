//! Human management of App configuration. Provider calls use the authenticated MCP proxy.

mod bindings;
mod discovery;
mod event_config;
mod registration;
pub(super) use discovery::{AppCapability, member_capabilities};
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use agenthub_agent_domain::{app_tools::valid_name, loop_runtime::validate_loop_id};
use agenthub_db::{app_registry::AppStoreError, loop_runtime::LoopStoreError};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, rejection::JsonRejection},
    routing::{get, post},
};
use serde::Deserialize;

use crate::{api::ApiError, state::AppState};

pub(super) fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(registration::list).post(registration::register))
        .route("/{app_id}", get(registration::get_app))
        .route("/{app_id}/versions", post(registration::publish))
        .route(
            "/{app_id}/versions/{version}",
            get(registration::get_version),
        )
        .route("/{app_id}/revoke", post(registration::revoke))
        .route(
            "/{app_id}/event-key",
            get(event_config::get_key).put(event_config::configure_key),
        )
        .route("/{app_id}/event-key/revoke", post(event_config::revoke_key))
        .layer(DefaultBodyLimit::max(524_288))
        .with_state(state)
}

pub(super) fn team_router(state: AppState) -> Router {
    Router::new()
        .route(
            "/teams/{team_id}/apps/{app_id}",
            get(bindings::get_grant).put(bindings::approve),
        )
        .route(
            "/teams/{team_id}/apps/{app_id}/revoke",
            post(bindings::revoke_grant),
        )
        .route(
            "/teams/{team_id}/members/{actor_id}/apps",
            get(bindings::list),
        )
        .route(
            "/teams/{team_id}/members/{actor_id}/apps/{app_id}",
            axum::routing::put(bindings::bind),
        )
        .route(
            "/teams/{team_id}/members/{actor_id}/apps/{app_id}/revoke",
            post(bindings::revoke_binding),
        )
        .route(
            "/teams/{team_id}/members/{actor_id}/apps/{app_id}/events",
            get(event_config::get_route).put(event_config::configure_route),
        )
        .route(
            "/teams/{team_id}/members/{actor_id}/apps/{app_id}/events/revoke",
            post(event_config::revoke_route),
        )
        .layer(DefaultBodyLimit::max(16_384))
        .with_state(state)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    after: Option<String>,
    limit: Option<u32>,
}

impl Page {
    fn validate(&self) -> Result<u32, ApiError> {
        if let Some(after) = &self.after {
            validate_loop_id(after).map_err(|_| ApiError::bad_request("invalid app cursor"))?;
        }
        let limit = self.limit.unwrap_or(50);
        if !(1..=100).contains(&limit) {
            return Err(ApiError::bad_request(
                "app page limit must be between 1 and 100",
            ));
        }
        Ok(limit)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    expected_revision: i64,
}

fn payload<T>(input: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    // Do not echo connection fields or raw malformed payload fragments through extractor errors.
    input
        .map(|Json(value)| value)
        .map_err(|_| ApiError::bad_request("invalid app request"))
}

fn revision(value: i64, allow_new: bool) -> Result<(), ApiError> {
    if value < i64::from(!allow_new) || value == i64::MAX {
        return Err(ApiError::bad_request("invalid app revision"));
    }
    Ok(())
}

fn scopes(values: &BTreeSet<String>) -> Result<(), ApiError> {
    if values.is_empty() || values.len() > 64 || !values.iter().all(|value| valid_name(value)) {
        return Err(ApiError::bad_request("invalid app scopes"));
    }
    Ok(())
}

fn store_error(error: anyhow::Error) -> ApiError {
    if let Some(error) = error.downcast_ref::<AppStoreError>() {
        return match error {
            AppStoreError::NotFound => ApiError::not_found("app record not found"),
            AppStoreError::Forbidden => ApiError::forbidden("app owner required"),
            AppStoreError::ScopeMismatch => {
                ApiError::forbidden("app scopes exceed the approved grant")
            }
            AppStoreError::Revoked => ApiError::conflict("app authority was revoked"),
            AppStoreError::RevisionConflict => ApiError::conflict("app record revision changed"),
            AppStoreError::Capacity => ApiError::conflict("app record limit reached"),
        };
    }
    if error.downcast_ref::<LoopStoreError>().is_some() {
        return ApiError::conflict("app binding requires a current loop member");
    }
    if let Some(sqlx::Error::Database(database)) = error.downcast_ref::<sqlx::Error>() {
        if database.is_unique_violation() {
            return ApiError::conflict("app authority is already registered");
        }
        if database.is_foreign_key_violation() {
            return ApiError::bad_request("app owner or binding target does not exist");
        }
    }
    // Database and validation internals may carry private connection material.
    anyhow::anyhow!("app registry operation failed").into()
}
