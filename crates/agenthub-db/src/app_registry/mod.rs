//! Immutable app manifests and durable grants. HTTP authorization precedes administrative writes.

mod bindings;
mod grants;
mod pins;
mod registration;
mod schema;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use agenthub_agent_domain::app_tools::AppManifest;
use serde::Serialize;
use sqlx::SqlitePool;
use thiserror::Error;

pub use bindings::AppBindingUpdate;
pub use grants::AppGrantUpdate;
pub use registration::RegisterApp;
pub use schema::migrate_app_registry;

#[derive(Debug, Error)]
pub enum AppStoreError {
    #[error("app record was not found")]
    NotFound,
    #[error("app ownership does not match")]
    Forbidden,
    #[error("app record revision changed")]
    RevisionConflict,
    #[error("app authority was revoked")]
    Revoked,
    #[error("app scope does not match the approved binding")]
    ScopeMismatch,
    #[error("app record limit reached")]
    Capacity,
}

#[derive(Clone)]
pub struct AppRegistry {
    pool: SqlitePool,
}

impl AppRegistry {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

/// Safe management projection. Transport configuration and credential references stay daemon-side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RegisteredApp {
    pub id: String,
    pub owner_user_id: String,
    pub name: String,
    pub revision: i64,
    pub latest_version: i64,
    pub revoked_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AppVersion {
    pub app_id: String,
    pub version: i64,
    pub manifest: AppManifest,
    pub created_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AppTeamGrant {
    pub app_id: String,
    pub team_id: String,
    pub scopes: BTreeSet<String>,
    pub revision: i64,
    pub authorization_epoch: i64,
    pub revoked_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AppMemberBinding {
    pub app_id: String,
    pub team_id: String,
    pub actor_id: String,
    pub version: i64,
    pub scopes: BTreeSet<String>,
    pub revision: i64,
    pub authorization_epoch: i64,
    pub revoked_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AppActivationPin {
    pub activation_id: String,
    pub app_id: String,
    pub team_id: String,
    pub actor_id: String,
    pub pinned_generation: i64,
    pub version: i64,
    pub scopes: BTreeSet<String>,
    pub grant_revision: i64,
    pub binding_revision: i64,
    pub grant_epoch: i64,
    pub binding_epoch: i64,
}
