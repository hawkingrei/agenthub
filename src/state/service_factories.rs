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

pub(super) fn build_agent_manager(
    db: &SqlitePool,
    event_dbs: &agenthub_db::AgentEventDbRouter,
    idle_gc: Option<agenthub_db::AgentEventIdleGc>,
    push: Arc<PushService>,
    config: &agenthub_config::AppConfig,
    acp_permissions: Arc<AcpPermissionService>,
    auth: Arc<AuthService>,
    internal_peer_client: Option<crate::internal::client::InternalGrpcPeerClientConfig>,
) -> Arc<AgentManager> {
    Arc::new(AgentManager::new_with_internal_grpc(
        db.clone(),
        event_dbs.clone(),
        idle_gc,
        push,
        config.proxy_env(),
        config.codex_acp_binary(),
        config.codex_acp_default_mode(),
        config.codex_acp_multi_agent_enabled(),
        acp_permissions,
        auth,
        internal_peer_client,
    ))
}

pub(super) fn build_team_manager(
    db: &SqlitePool,
    event_dbs: &agenthub_db::AgentEventDbRouter,
    message_archive: Option<agenthub_message_archive::MessageArchiveStoreRef>,
) -> Arc<TeamManager> {
    Arc::new(TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        event_dbs.clone(),
        message_archive,
    ))
}
