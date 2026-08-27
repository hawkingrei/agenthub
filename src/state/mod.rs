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
use crate::object_upload::ObjectUploadService;
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
    /// Shared object upload publication service. API routes will read this once browser/image
    /// upload surfaces are added; actor CLI uses the same service construction path directly.
    #[allow(dead_code)]
    pub object_uploads: Arc<ObjectUploadService>,
    pub agent_node_join_bootstrap: AgentNodeJoinBootstrapInfo,
    pub default_worktree_root: String,
    /// Tiered message body store, when enabled and compiled in. Held here so the read path can fetch
    /// bodies from it.
    #[allow(dead_code)]
    pub body_store: Option<crate::message_body_store::SharedBodyStore>,
    /// Rebuildable message index store, when enabled and compiled in.
    #[allow(dead_code)]
    pub message_index: Option<crate::message_body_store::SharedIndexStore>,
    /// Queue for lagging index projections discovered by guarded reads.
    #[allow(dead_code)]
    pub message_read_repair: Option<crate::message_body_store::SharedReadRepairScheduler>,
}

impl AppState {
    pub(crate) async fn init_for_daemon(
        config: agenthub_config::AppConfig,
        daemon_instance: &mut crate::daemon_instance::DaemonInstanceGuard,
    ) -> anyhow::Result<Self> {
        Self::init_inner(config, daemon_instance).await
    }

    async fn init_inner(
        config: agenthub_config::AppConfig,
        daemon_instance: &mut crate::daemon_instance::DaemonInstanceGuard,
    ) -> anyhow::Result<Self> {
        let db = Self::open_database().await?;
        daemon_instance.claim_generation(&db).await?;
        if config.server_role() == agenthub_config::ServerRole::Main {
            Self::ensure_root(&db).await?;
        }
        if let Err(error) = Self::ensure_global_gitignore_agenthubmemory() {
            tracing::warn!(
                ?error,
                "failed to ensure global gitignore entry for .agenthubmemory"
            );
        }
        let linker_http = crate::linkers::AppLinkerService::default_http_client();
        let event_dbs = agenthub_db::AgentEventDbRouter::with_default_base_dir();

        let message_stores = crate::message_body_store::init_message_stores(&config);
        let body_store = message_stores.body.clone();
        let message_read_repair = message_stores.read_repair.clone();
        let object_uploads = Arc::new(ObjectUploadService::from_config(db.clone(), &config)?);

        let (agents, teams, push, auth, acp_permissions) = Self::initialize_services(
            &config,
            db.clone(),
            event_dbs.clone(),
            message_stores.clone(),
        )
        .await?;
        let agent_node_join_bootstrap = Self::build_agent_node_join_bootstrap(&config)?;

        Self::run_startup_cleanup(&agents, &teams).await?;
        Self::spawn_startup_workers(
            &db,
            &agents,
            &teams,
            &body_store,
            config.message_body_store_auto_migrate(),
        )
        .await?;

        let default_worktree_root = config.default_worktree_root();
        Ok(Self {
            db,
            linker_http,
            agents,
            teams,
            push,
            auth,
            acp_permissions,
            object_uploads,
            agent_node_join_bootstrap,
            default_worktree_root,
            body_store,
            message_index: message_stores.index,
            message_read_repair,
        })
    }

    fn ensure_global_gitignore_agenthubmemory() -> anyhow::Result<()> {
        gitignore::ensure_global_gitignore_agenthubmemory()
    }
}
