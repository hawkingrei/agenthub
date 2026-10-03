use std::collections::{HashMap, hash_map::Entry};

use agenthub_db::runtime_events::{
    RuntimeEventError, RuntimeEventStore, RuntimeHistory, RuntimeRequestKind, RuntimeRequestStatus,
};
use serde_json::Value;
use sqlx::SqlitePool;

use super::AgentManager;
use crate::agent::{AgentEvent, OutputStream};

impl AgentManager {
    /// Receipts remain authoritative if the daemon exits before emitting their history event.
    /// Hydrate only retained rows so recovery neither rewrites history nor bypasses retention.
    pub(in crate::agent::manager) async fn reconcile_native_input_history(
        pool: &SqlitePool,
        events: &mut [AgentEvent],
    ) -> anyhow::Result<()> {
        let mut owners = HashMap::new();
        for event in events {
            if !matches!(event.stream, OutputStream::Acp) {
                continue;
            }
            let Ok(mut message) = serde_json::from_str::<Value>(&event.message) else {
                continue;
            };
            let is_input = message["type"] == "user_message";
            if !is_input && message["type"] != "input_receipt" {
                continue;
            }
            let provider = &message["meta"]["provider_runtime"];
            let (Some(request_id), Some(runtime_id), Some(native_session_id)) = (
                message["message_id"].as_str(),
                provider["runtime_id"].as_str(),
                provider["native_session_id"].as_str(),
            ) else {
                continue;
            };
            if provider["provider"] != "rara" || provider["request_id"] != request_id {
                continue;
            }
            if let Entry::Vacant(entry) = owners.entry(event.session_id.clone()) {
                let owner = match RuntimeEventStore::load(pool.clone(), &event.session_id).await {
                    Err(error)
                        if matches!(
                            error.downcast_ref::<RuntimeEventError>(),
                            Some(RuntimeEventError::InvalidIdentity)
                        ) =>
                    {
                        continue;
                    }
                    result => result?,
                };
                entry.insert(owner);
            }
            let Some(store) = owners.get(&event.session_id).and_then(Option::as_ref) else {
                continue;
            };
            if store.runtime_id() != runtime_id {
                continue;
            }
            let receipt = match store.request_receipt(request_id).await {
                Err(error)
                    if matches!(
                        error.downcast_ref::<RuntimeEventError>(),
                        Some(RuntimeEventError::InvalidIdentity)
                    ) =>
                {
                    continue;
                }
                result => result?,
            };
            let Some(receipt) = receipt else { continue };
            if receipt.target_session_id.as_deref() != Some(native_session_id)
                || !matches!(
                    receipt.kind,
                    RuntimeRequestKind::Prompt
                        | RuntimeRequestKind::FollowUp
                        | RuntimeRequestKind::UserAnswer
                )
                || matches!(
                    receipt.status,
                    RuntimeRequestStatus::Prepared | RuntimeRequestStatus::Sent
                )
            {
                continue;
            }
            if is_input {
                message["meta"]["delivery"] = serde_json::to_value(receipt.status)?;
            } else {
                message["receipt"] = serde_json::to_value(receipt)?;
            }
            event.message = message.to_string();
        }
        Ok(())
    }

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
            let agents: Vec<String> = sqlx::query_scalar(
                "SELECT id FROM agents WHERE id > ? AND target_node_id IS NULL \
                 ORDER BY id LIMIT 100",
            )
            .bind(&after)
            .fetch_all(&self.db)
            .await?;
            if agents.is_empty() {
                break;
            }
            for agent_id in &agents {
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
                self.recover_runtime_receipts_for_agent(agent_id, pool)
                    .await?;
            }
            after = agents.last().expect("nonempty agent page").clone();
        }
        Ok(())
    }

    async fn recover_runtime_receipts_for_agent(
        &self,
        agent_id: &str,
        pool: SqlitePool,
    ) -> anyhow::Result<()> {
        let mut after = None;
        loop {
            let owners = RuntimeEventStore::load_open_page(pool.clone(), after.as_deref()).await?;
            if owners.is_empty() {
                return Ok(());
            }
            for store in owners {
                let session_id = store.local_session_id();
                after = Some(session_id.to_owned());
                let owned: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE id = ? AND agent_id = ?)",
                )
                .bind(session_id)
                .bind(agent_id)
                .fetch_one(&self.db)
                .await?;
                if !owned {
                    continue;
                }
                sqlx::query(
                    "UPDATE agent_sessions SET status = 'exited', ended_at = ? \
                     WHERE id = ? AND agent_id = ? AND ended_at IS NULL \
                     AND NOT EXISTS (SELECT 1 FROM loop_execution_reservations r \
                                     WHERE r.session_id = agent_sessions.id)",
                )
                .bind(chrono::Utc::now().timestamp())
                .bind(session_id)
                .bind(agent_id)
                .execute(&self.db)
                .await?;
                self.permissions
                    .interrupt_session_permissions(session_id)
                    .await?;
                // Close last: interrupted control-plane cleanup must leave an open owner
                // for the next startup. Retirement never proves descendant cleanup.
                store.close(chrono::Utc::now().timestamp()).await?;
            }
            tokio::task::yield_now().await;
        }
    }
}
