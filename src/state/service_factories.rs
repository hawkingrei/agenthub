use std::sync::Arc;

use agenthub_config::ServerRole;
use sqlx::SqlitePool;

use crate::acp::AcpPermissionService;
use crate::agent::AgentManager;
use crate::auth::AuthService;
use crate::push::PushService;
use crate::team::TeamManager;

pub(super) fn build_push_service(
    config: &agenthub_config::AppConfig,
    db: &SqlitePool,
) -> anyhow::Result<Arc<PushService>> {
    Ok(if config.server_role() == ServerRole::Node {
        Arc::new(PushService::disabled(db.clone()))
    } else {
        Arc::new(PushService::new(db.clone(), config)?)
    })
}

pub(super) fn build_acp_permission_service(db: &SqlitePool) -> Arc<AcpPermissionService> {
    Arc::new(AcpPermissionService::new(db.clone()))
}

pub(super) async fn build_auth_service(
    db: &SqlitePool,
    config: &agenthub_config::AppConfig,
) -> anyhow::Result<Arc<AuthService>> {
    Ok(Arc::new(AuthService::new(db.clone(), config).await?))
}

pub(super) struct AgentManagerBuildArgs<'a> {
    pub(super) db: &'a SqlitePool,
    pub(super) event_dbs: &'a agenthub_db::AgentEventDbRouter,
    pub(super) idle_gc: Option<agenthub_db::AgentEventIdleGc>,
    pub(super) push: Arc<PushService>,
    pub(super) config: &'a agenthub_config::AppConfig,
    pub(super) acp_permissions: Arc<AcpPermissionService>,
    pub(super) auth: Arc<AuthService>,
    pub(super) internal_peer_client: Option<crate::internal::client::InternalGrpcPeerClientConfig>,
    pub(super) message_index: Option<crate::message_body_store::SharedIndexStore>,
    pub(super) read_repair: Option<crate::message_body_store::SharedReadRepairScheduler>,
}

pub(super) fn build_agent_manager(args: AgentManagerBuildArgs<'_>) -> Arc<AgentManager> {
    Arc::new(
        AgentManager::new_with_internal_grpc(
            args.db.clone(),
            args.event_dbs.clone(),
            args.idle_gc,
            args.push,
            args.config.proxy_env(),
            args.config.codex_acp_binary(),
            args.config.codex_acp_default_mode(),
            args.config.codex_acp_multi_agent_enabled(),
            args.acp_permissions,
            args.auth,
            args.internal_peer_client,
        )
        .with_message_index(args.message_index)
        .with_read_repair_scheduler(args.read_repair),
    )
}

pub(super) fn build_team_manager(
    db: &SqlitePool,
    event_dbs: &agenthub_db::AgentEventDbRouter,
    message_archive: Option<agenthub_message_archive::MessageArchiveStoreRef>,
    body_store: Option<crate::message_body_store::SharedBodyStore>,
    message_index: Option<crate::message_body_store::SharedIndexStore>,
    read_repair: Option<crate::message_body_store::SharedReadRepairScheduler>,
) -> Arc<TeamManager> {
    Arc::new(
        TeamManager::new_with_event_dbs_and_message_archive(
            db.clone(),
            event_dbs.clone(),
            message_archive,
        )
        .with_body_store(body_store)
        .with_message_index(message_index)
        .with_read_repair_scheduler(read_repair),
    )
}
