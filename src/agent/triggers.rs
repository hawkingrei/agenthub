use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Row, SqlitePool};

use super::AgentManager;

const AGENT_TIME_TRIGGER_KIND_TIME: &str = "time";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentTimeTriggerStatus {
    Scheduled,
    Dispatching,
    Fired,
    Canceled,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AgentTimeTriggerRecord {
    pub id: String,
    pub agent_id: String,
    pub kind: String,
    pub created_by_actor_id: String,
    pub message_text: String,
    pub fire_at: i64,
    pub status: AgentTimeTriggerStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub fired_at: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AgentTimeTriggerCreateInput {
    pub agent_id: String,
    pub created_by_actor_id: String,
    pub message_text: String,
    pub fire_at: i64,
}

#[derive(Clone)]
pub struct AgentTimeTriggerManager {
    db: SqlitePool,
}

#[derive(Debug, Clone, Copy)]
pub struct AgentTimeTriggerWorkerSettings {
    pub poll_interval_secs: i64,
    pub max_dispatch_per_tick: i64,
}

impl Default for AgentTimeTriggerWorkerSettings {
    fn default() -> Self {
        Self {
            poll_interval_secs: 2,
            max_dispatch_per_tick: 32,
        }
    }
}

#[async_trait]
pub trait AgentTimeTriggerDelivery: Send + Sync {
    async fn deliver_trigger_message(
        &self,
        agent_id: &str,
        message: &str,
        message_id: &str,
    ) -> anyhow::Result<()>;
}

#[async_trait]
impl AgentTimeTriggerDelivery for AgentManager {
    async fn deliver_trigger_message(
        &self,
        agent_id: &str,
        message: &str,
        message_id: &str,
    ) -> anyhow::Result<()> {
        self.send_input(agent_id, message, Some(message_id), None)
            .await
    }
}

#[derive(Clone)]
pub struct AgentTimeTriggerWorker {
    triggers: Arc<AgentTimeTriggerManager>,
    delivery: Arc<dyn AgentTimeTriggerDelivery>,
}

impl AgentTimeTriggerWorker {
    pub fn new(
        triggers: Arc<AgentTimeTriggerManager>,
        delivery: Arc<dyn AgentTimeTriggerDelivery>,
    ) -> Self {
        Self { triggers, delivery }
    }

    pub fn spawn(self, settings: AgentTimeTriggerWorkerSettings) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(
                settings.poll_interval_secs.max(1) as u64,
            ));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                if let Err(error) = self.dispatch_once(settings.max_dispatch_per_tick).await {
                    tracing::warn!(?error, "agent time trigger worker tick failed");
                }
            }
        })
    }

    pub async fn dispatch_once(&self, max_dispatch_per_tick: i64) -> anyhow::Result<usize> {
        let now = Utc::now().timestamp();
        let claimed = self
            .triggers
            .claim_due_triggers(now, max_dispatch_per_tick.max(1))
            .await?;
        let mut delivered = 0_usize;
        for trigger in claimed {
            let message_id = format!("time-trigger:{}", trigger.id);
            let prompt = build_time_trigger_prompt(&trigger);
            match self
                .delivery
                .deliver_trigger_message(trigger.agent_id.as_str(), &prompt, &message_id)
                .await
            {
                Ok(()) => {
                    self.triggers
                        .mark_trigger_fired(trigger.id.as_str(), now)
                        .await?;
                    delivered += 1;
                }
                Err(error) => {
                    self.triggers
                        .requeue_trigger(trigger.id.as_str(), now, error.to_string().as_str())
                        .await?;
                }
            }
        }
        Ok(delivered)
    }
}

impl AgentTimeTriggerManager {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn reset_inflight_on_startup(&self) -> anyhow::Result<u64> {
        let now = Utc::now().timestamp();
        let result = sqlx::query(
            r#"
            UPDATE agent_time_triggers
            SET status = 'scheduled', updated_at = ?1
            WHERE status = 'dispatching'
            "#,
        )
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn create_time_trigger(
        &self,
        input: AgentTimeTriggerCreateInput,
    ) -> anyhow::Result<AgentTimeTriggerRecord> {
        let message_text = input.message_text.trim();
        if message_text.is_empty() {
            anyhow::bail!("time trigger message must not be empty");
        }
        if input.fire_at <= Utc::now().timestamp() {
            anyhow::bail!("time trigger fire_at must be in the future");
        }
        let trigger_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO agent_time_triggers (
                id,
                agent_id,
                kind,
                created_by_actor_id,
                message_text,
                fire_at,
                status,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'scheduled', ?7, ?8)
            "#,
        )
        .bind(&trigger_id)
        .bind(input.agent_id.trim())
        .bind(AGENT_TIME_TRIGGER_KIND_TIME)
        .bind(input.created_by_actor_id.trim())
        .bind(message_text)
        .bind(input.fire_at)
        .bind(now)
        .bind(now)
        .execute(&self.db)
        .await?;
        self.get_trigger(trigger_id.as_str()).await
    }

    pub async fn list_triggers_for_agent(
        &self,
        agent_id: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<AgentTimeTriggerRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                agent_id,
                kind,
                created_by_actor_id,
                message_text,
                fire_at,
                status,
                created_at,
                updated_at,
                fired_at,
                last_error
            FROM agent_time_triggers
            WHERE agent_id = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )
        .bind(agent_id)
        .bind(limit.clamp(1, 500))
        .fetch_all(&self.db)
        .await?;
        rows.into_iter().map(record_from_row).collect()
    }

    pub async fn cancel_trigger(&self, agent_id: &str, trigger_id: &str) -> anyhow::Result<bool> {
        let now = Utc::now().timestamp();
        let result = sqlx::query(
            r#"
            UPDATE agent_time_triggers
            SET status = 'canceled', updated_at = ?1
            WHERE id = ?2 AND agent_id = ?3 AND status IN ('scheduled', 'dispatching')
            "#,
        )
        .bind(now)
        .bind(trigger_id)
        .bind(agent_id)
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn claim_due_triggers(
        &self,
        now: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<AgentTimeTriggerRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id
            FROM agent_time_triggers
            WHERE status = 'scheduled' AND fire_at <= ?1
            ORDER BY fire_at ASC, created_at ASC
            LIMIT ?2
            "#,
        )
        .bind(now)
        .bind(limit.clamp(1, 500))
        .fetch_all(&self.db)
        .await?;
        let mut claimed = Vec::new();
        for row in rows {
            let trigger_id: String = row.get("id");
            let updated = sqlx::query(
                r#"
                UPDATE agent_time_triggers
                SET status = 'dispatching', updated_at = ?1, last_error = NULL
                WHERE id = ?2 AND status = 'scheduled'
                "#,
            )
            .bind(now)
            .bind(&trigger_id)
            .execute(&self.db)
            .await?;
            if updated.rows_affected() == 0 {
                continue;
            }
            claimed.push(self.get_trigger(trigger_id.as_str()).await?);
        }
        Ok(claimed)
    }

    pub async fn mark_trigger_fired(&self, trigger_id: &str, fired_at: i64) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE agent_time_triggers
            SET status = 'fired', fired_at = ?1, updated_at = ?2, last_error = NULL
            WHERE id = ?3
            "#,
        )
        .bind(fired_at)
        .bind(fired_at)
        .bind(trigger_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn requeue_trigger(
        &self,
        trigger_id: &str,
        updated_at: i64,
        last_error: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE agent_time_triggers
            SET status = 'scheduled', updated_at = ?1, last_error = ?2
            WHERE id = ?3
            "#,
        )
        .bind(updated_at)
        .bind(last_error)
        .bind(trigger_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn get_trigger(&self, trigger_id: &str) -> anyhow::Result<AgentTimeTriggerRecord> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                agent_id,
                kind,
                created_by_actor_id,
                message_text,
                fire_at,
                status,
                created_at,
                updated_at,
                fired_at,
                last_error
            FROM agent_time_triggers
            WHERE id = ?1
            "#,
        )
        .bind(trigger_id)
        .fetch_one(&self.db)
        .await
        .with_context(|| format!("time trigger not found: {trigger_id}"))?;
        record_from_row(row)
    }
}

fn build_time_trigger_prompt(trigger: &AgentTimeTriggerRecord) -> String {
    format!(
        "[AgentHub time trigger fired]\ntrigger_id: {}\nfire_at: {}\nmessage:\n{}",
        trigger.id, trigger.fire_at, trigger.message_text
    )
}

fn record_from_row(row: sqlx::sqlite::SqliteRow) -> anyhow::Result<AgentTimeTriggerRecord> {
    Ok(AgentTimeTriggerRecord {
        id: row.try_get("id")?,
        agent_id: row.try_get("agent_id")?,
        kind: row.try_get("kind")?,
        created_by_actor_id: row.try_get("created_by_actor_id")?,
        message_text: row.try_get("message_text")?,
        fire_at: row.try_get("fire_at")?,
        status: status_from_str(row.try_get::<String, _>("status")?.as_str())?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        fired_at: row.try_get("fired_at").ok(),
        last_error: row.try_get("last_error").ok(),
    })
}

fn status_from_str(value: &str) -> anyhow::Result<AgentTimeTriggerStatus> {
    match value {
        "scheduled" => Ok(AgentTimeTriggerStatus::Scheduled),
        "dispatching" => Ok(AgentTimeTriggerStatus::Dispatching),
        "fired" => Ok(AgentTimeTriggerStatus::Fired),
        "canceled" => Ok(AgentTimeTriggerStatus::Canceled),
        other => Err(anyhow::anyhow!(
            "invalid agent time trigger status: {other}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::team_tests::build_test_state;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockDelivery {
        deliveries: Mutex<Vec<(String, String, String)>>,
        fail: Mutex<bool>,
    }

    #[async_trait]
    impl AgentTimeTriggerDelivery for MockDelivery {
        async fn deliver_trigger_message(
            &self,
            agent_id: &str,
            message: &str,
            message_id: &str,
        ) -> anyhow::Result<()> {
            self.deliveries.lock().expect("lock deliveries").push((
                agent_id.to_string(),
                message.to_string(),
                message_id.to_string(),
            ));
            if *self.fail.lock().expect("lock fail") {
                anyhow::bail!("mock delivery failure");
            }
            Ok(())
        }
    }

    async fn init_trigger_test_fixtures(db: &SqlitePool) {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS agent_time_triggers (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                created_by_actor_id TEXT NOT NULL,
                message_text TEXT NOT NULL,
                fire_at INTEGER NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                fired_at INTEGER,
                last_error TEXT
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create agent_time_triggers");
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'created', ?7, ?8)
            "#,
        )
        .bind("agent-1")
        .bind("agent-1")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(Utc::now().timestamp())
        .bind(Utc::now().timestamp())
        .execute(db)
        .await
        .expect("insert agent fixture");
    }

    #[tokio::test]
    async fn time_trigger_manager_create_list_and_cancel_roundtrip() {
        let state = build_test_state().await;
        init_trigger_test_fixtures(&state.db).await;
        let manager = AgentTimeTriggerManager::new(state.db.clone());
        let fire_at = Utc::now().timestamp() + 60;
        let trigger = manager
            .create_time_trigger(AgentTimeTriggerCreateInput {
                agent_id: "agent-1".to_string(),
                created_by_actor_id: "agent-1".to_string(),
                message_text: "Check the regression dashboard.".to_string(),
                fire_at,
            })
            .await
            .expect("create trigger");
        assert_eq!(trigger.status, AgentTimeTriggerStatus::Scheduled);
        assert_eq!(trigger.fire_at, fire_at);

        let listed = manager
            .list_triggers_for_agent("agent-1", 20)
            .await
            .expect("list triggers");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, trigger.id);

        let canceled = manager
            .cancel_trigger("agent-1", trigger.id.as_str())
            .await
            .expect("cancel trigger");
        assert!(canceled);
        let listed = manager
            .list_triggers_for_agent("agent-1", 20)
            .await
            .expect("list triggers after cancel");
        assert_eq!(listed[0].status, AgentTimeTriggerStatus::Canceled);
    }

    #[tokio::test]
    async fn time_trigger_worker_dispatches_due_trigger_once() {
        let state = build_test_state().await;
        init_trigger_test_fixtures(&state.db).await;
        let manager = Arc::new(AgentTimeTriggerManager::new(state.db.clone()));
        let delivery = Arc::new(MockDelivery::default());
        let fire_at = Utc::now().timestamp() + 60;
        let trigger = manager
            .create_time_trigger(AgentTimeTriggerCreateInput {
                agent_id: "agent-1".to_string(),
                created_by_actor_id: "agent-1".to_string(),
                message_text: "Re-check the issue queue.".to_string(),
                fire_at,
            })
            .await
            .expect("create due trigger");
        sqlx::query("UPDATE agent_time_triggers SET fire_at = ?1 WHERE id = ?2")
            .bind(Utc::now().timestamp() - 1)
            .bind(&trigger.id)
            .execute(&state.db)
            .await
            .expect("backdate trigger");

        let worker = AgentTimeTriggerWorker::new(manager.clone(), delivery.clone());
        let dispatched = worker.dispatch_once(10).await.expect("dispatch");
        assert_eq!(dispatched, 1);

        let listed = manager
            .list_triggers_for_agent("agent-1", 20)
            .await
            .expect("list triggers");
        assert_eq!(listed[0].id, trigger.id);
        assert_eq!(listed[0].status, AgentTimeTriggerStatus::Fired);

        let deliveries = delivery.deliveries.lock().expect("lock deliveries");
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].0, "agent-1");
        assert!(deliveries[0].1.contains("AgentHub time trigger fired"));
        assert_eq!(deliveries[0].2, format!("time-trigger:{}", trigger.id));
    }

    #[tokio::test]
    async fn time_trigger_worker_requeues_failed_delivery() {
        let state = build_test_state().await;
        init_trigger_test_fixtures(&state.db).await;
        let manager = Arc::new(AgentTimeTriggerManager::new(state.db.clone()));
        let delivery = Arc::new(MockDelivery::default());
        *delivery.fail.lock().expect("lock fail") = true;
        let trigger = manager
            .create_time_trigger(AgentTimeTriggerCreateInput {
                agent_id: "agent-1".to_string(),
                created_by_actor_id: "agent-1".to_string(),
                message_text: "Retry later.".to_string(),
                fire_at: Utc::now().timestamp() + 60,
            })
            .await
            .expect("create due trigger");
        sqlx::query("UPDATE agent_time_triggers SET fire_at = ?1 WHERE id = ?2")
            .bind(Utc::now().timestamp() - 1)
            .bind(&trigger.id)
            .execute(&state.db)
            .await
            .expect("backdate trigger");

        let worker = AgentTimeTriggerWorker::new(manager.clone(), delivery);
        let dispatched = worker.dispatch_once(10).await.expect("dispatch");
        assert_eq!(dispatched, 0);

        let listed = manager
            .list_triggers_for_agent("agent-1", 20)
            .await
            .expect("list triggers");
        assert_eq!(listed[0].id, trigger.id);
        assert_eq!(listed[0].status, AgentTimeTriggerStatus::Scheduled);
        assert!(
            listed[0]
                .last_error
                .as_deref()
                .is_some_and(|value| value.contains("mock delivery failure"))
        );
    }
}
