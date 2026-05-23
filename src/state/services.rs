use std::sync::Arc;

use sqlx::SqlitePool;

use crate::agent::AgentManager;
use crate::team::TeamManager;

use super::AppState;

impl AppState {
    pub(super) async fn initialize_services(
        config: &agenthub_config::AppConfig,
        db: SqlitePool,
        event_dbs: agenthub_db::AgentEventDbRouter,
    ) -> anyhow::Result<(
        Arc<AgentManager>,
        Arc<TeamManager>,
        Arc<crate::push::PushService>,
        Arc<crate::auth::AuthService>,
        Arc<crate::acp::AcpPermissionService>,
    )> {
        let internal_peer_client = Self::build_internal_grpc_peer_client(config)?;
        let idle_gc = super::history::build_agent_event_idle_gc(config, &event_dbs);

        let push = super::service_factories::build_push_service(config, &db)?;
        let acp_permissions = super::service_factories::build_acp_permission_service(&db);
        let auth = super::service_factories::build_auth_service(&db, config).await?;
        let message_archive = Self::initialize_message_archive(config).await?;
        let agents = super::service_factories::build_agent_manager(
            super::service_factories::AgentManagerBuildArgs {
                db: &db,
                event_dbs: &event_dbs,
                idle_gc,
                push: push.clone(),
                config,
                acp_permissions: acp_permissions.clone(),
                auth: auth.clone(),
                internal_peer_client: internal_peer_client.clone(),
            },
        );

        let teams = super::service_factories::build_team_manager(&db, &event_dbs, message_archive);
        super::team_wiring::configure_team_services(
            &teams,
            &agents,
            &acp_permissions,
            internal_peer_client.as_ref(),
        );

        Ok((agents, teams, push, auth, acp_permissions))
    }

    pub(super) async fn run_startup_cleanup(
        agents: &AgentManager,
        teams: &TeamManager,
    ) -> anyhow::Result<()> {
        agents.mark_exited_on_startup().await?;
        // Startup policy: never auto-resume in-flight team runs after a process restart.
        // We force manual restart so users can re-confirm intent and avoid duplicate execution.
        let startup_canceled_runs = teams.cancel_active_runs_on_startup().await?;
        if startup_canceled_runs > 0 {
            tracing::info!(
                canceled_run_count = startup_canceled_runs,
                "team startup policy: canceled active runs to require manual start"
            );
        }
        Ok(())
    }
}
