mod archive;
mod database;
mod gitignore;
mod history;
mod internal_grpc;
mod service_factories;
mod services;
mod startup;
mod team_wiring;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_support;

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::acp::AcpPermissionService;
use crate::agent::{AgentManager, AgentNodeJoinBootstrapInfo};
use crate::auth::AuthService;
use crate::push::PushService;
use crate::team::TeamManager;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub linker_http: reqwest::Client,
    pub agents: Arc<AgentManager>,
    pub teams: Arc<TeamManager>,
    pub push: Arc<PushService>,
    pub auth: Arc<AuthService>,
    pub acp_permissions: Arc<AcpPermissionService>,
    pub agent_node_join_bootstrap: AgentNodeJoinBootstrapInfo,
    pub default_worktree_root: String,
    /// Tiered message body store, when enabled and compiled in. Held here so the read path can fetch
    /// bodies from it; the write/read wiring lands in a follow-up.
    #[allow(dead_code)]
    pub body_store: Option<crate::message_body_store::SharedBodyStore>,
}

impl AppState {
    pub async fn init(config: agenthub_config::AppConfig) -> anyhow::Result<Self> {
        if let Err(error) = Self::ensure_global_gitignore_agenthubmemory() {
            tracing::warn!(
                ?error,
                "failed to ensure global gitignore entry for .agenthubmemory"
            );
        }
        let db = Self::setup_database(&config).await?;
        let linker_http = crate::linkers::AppLinkerService::default_http_client();
        let event_dbs = agenthub_db::AgentEventDbRouter::with_default_base_dir();

        let body_store = crate::message_body_store::init_body_store(&config);

        let (agents, teams, push, auth, acp_permissions) =
            Self::initialize_services(&config, db.clone(), event_dbs.clone(), body_store.clone())
                .await?;
        let agent_node_join_bootstrap = Self::build_agent_node_join_bootstrap(&config)?;

        Self::run_startup_cleanup(&agents, &teams).await?;
        Self::spawn_startup_workers(&db, &agents, &teams, &body_store).await?;

        let default_worktree_root = config.default_worktree_root();
        Ok(Self {
            db,
            linker_http,
            agents,
            teams,
            push,
            auth,
            acp_permissions,
            agent_node_join_bootstrap,
            default_worktree_root,
            body_store,
        })
    }

    fn ensure_global_gitignore_agenthubmemory() -> anyhow::Result<()> {
        gitignore::ensure_global_gitignore_agenthubmemory()
    }
}
