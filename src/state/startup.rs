use std::sync::Arc;

use sqlx::SqlitePool;

use crate::agent::{
    AgentManager, AgentTimeTriggerManager, AgentTimeTriggerWorker, AgentTimeTriggerWorkerSettings,
};
use crate::team::{TeamMailboxUnreadHintWorker, TeamMailboxUnreadHintWorkerSettings, TeamManager};

use super::AppState;

impl AppState {
    pub(super) async fn spawn_startup_workers(
        db: &SqlitePool,
        agents: &Arc<AgentManager>,
        teams: &Arc<TeamManager>,
    ) -> anyhow::Result<()> {
        let _mailbox_hint_handle = TeamMailboxUnreadHintWorker::new(teams.clone(), agents.clone())
            .spawn(TeamMailboxUnreadHintWorkerSettings::default());
        let trigger_manager = Arc::new(AgentTimeTriggerManager::new(db.clone()));
        let recovered_dispatching = trigger_manager.reset_inflight_on_startup().await?;
        if recovered_dispatching > 0 {
            tracing::info!(
                recovered_dispatching,
                "agent time triggers reset to scheduled on startup"
            );
        }
        let _agent_trigger_handle = AgentTimeTriggerWorker::new(trigger_manager, agents.clone())
            .spawn(AgentTimeTriggerWorkerSettings::default());
        Ok(())
    }
}
