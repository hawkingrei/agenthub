use agenthub_db::runtime_events::{RuntimeEventStore, RuntimeHistory};

use super::AgentManager;

impl AgentManager {
    pub async fn runtime_history(
        &self,
        agent_id: &str,
        local_session_id: &str,
        limit: i64,
        before_request_id: Option<&str>,
    ) -> anyhow::Result<Option<RuntimeHistory>> {
        // Check the control-plane association before opening an agent's event database.
        // Native IDs never authorize access to a different local launch or remote node.
        let owned: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM agent_sessions s JOIN agents a ON a.id = s.agent_id \
             WHERE s.id = ? AND s.agent_id = ? AND a.target_node_id IS NULL)",
        )
        .bind(local_session_id)
        .bind(agent_id)
        .fetch_one(&self.db)
        .await?;
        if !owned {
            return Ok(None);
        }
        let pool = self.event_dbs.pool_for_agent(agent_id).await?;
        let Some(store) = RuntimeEventStore::load(pool, local_session_id).await? else {
            return Ok(None);
        };
        store.history(limit, before_request_id).await.map(Some)
    }

    /// Called before startup workers, while the new daemon owns its exclusive instance lock.
    pub(crate) async fn recover_runtime_receipts_on_startup(
        &self,
        daemon: &crate::daemon_instance::DaemonInstanceGuard,
    ) -> anyhow::Result<()> {
        daemon.verify_current(&self.db).await?;
        let mut after = String::new();
        loop {
            let sessions: Vec<(String, String)> = sqlx::query_as(
                "SELECT s.id, s.agent_id FROM agent_sessions s JOIN agents a ON a.id = s.agent_id \
                 WHERE s.id > ? AND a.target_node_id IS NULL \
                 AND NOT EXISTS (SELECT 1 FROM loop_execution_reservations r WHERE r.session_id = s.id) \
                 ORDER BY s.id LIMIT 100",
            )
            .bind(&after)
            .fetch_all(&self.db)
            .await?;
            if sessions.is_empty() {
                break;
            }
            for (session_id, agent_id) in &sessions {
                let _configuration = self.configuration_gate(agent_id).await.lock_owned().await;
                anyhow::ensure!(
                    !self.process_supervisor.has_actor_process(agent_id).await
                        && !self.inner.read().await.contains_key(agent_id),
                    "runtime receipt recovery requires a quiescent startup"
                );
                if !tokio::fs::try_exists(self.event_dbs.db_path_for_agent(agent_id)).await? {
                    continue;
                }
                let pool = self.event_dbs.pool_for_agent(agent_id).await?;
                if let Some(store) = RuntimeEventStore::load(pool, session_id).await? {
                    // The previous stdio owner cannot reconnect. Closing receipts does not
                    // prove descendant cleanup, finish a task, or permit replacement execution.
                    store.close(chrono::Utc::now().timestamp()).await?;
                    sqlx::query(
                        "UPDATE agent_sessions SET status = 'exited', ended_at = ? \
                         WHERE id = ? AND agent_id = ? AND ended_at IS NULL",
                    )
                    .bind(chrono::Utc::now().timestamp())
                    .bind(session_id)
                    .bind(agent_id)
                    .execute(&self.db)
                    .await?;
                    self.permissions
                        .interrupt_session_permissions(session_id)
                        .await?;
                }
            }
            after = sessions.last().expect("nonempty session page").0.clone();
        }
        Ok(())
    }
}
