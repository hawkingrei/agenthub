use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use async_trait::async_trait;
use chrono::Utc;
use futures::{StreamExt, stream};
use sqlx::{Row, SqlitePool};

use super::AgentManager;

const LEASE_SECONDS: i64 = 90;
const DELIVERY_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BATCH: i64 = 32;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentTimeTriggerStatus {
    Scheduled,
    Dispatching,
    /// Input was submitted to the runtime; this is not an execution receipt.
    Fired,
    Canceled,
}

/// Correlation metadata, never new authority for the resumed agent.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AgentReminderSource {
    #[serde(default)]
    pub scope_bound: bool,
    pub session_id: Option<String>,
    pub team_id: Option<String>,
    pub run_id: Option<String>,
    pub reference: Option<String>,
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
    #[serde(default)]
    pub attempt: i64,
    #[serde(default)]
    pub next_attempt_at: i64,
    #[serde(default)]
    pub lease_expires_at: Option<i64>,
    #[serde(default)]
    pub source: AgentReminderSource,
}

#[derive(Debug, Clone)]
pub enum AgentTimeTriggerSchedule {
    At(i64),
    After(i64),
}

#[derive(Debug, Clone)]
pub struct AgentTimeTriggerCreateInput {
    pub agent_id: String,
    pub created_by_actor_id: String,
    pub message_text: String,
    pub schedule: AgentTimeTriggerSchedule,
    pub source: AgentReminderSource,
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
            max_dispatch_per_tick: MAX_BATCH,
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
        source: &AgentReminderSource,
    ) -> anyhow::Result<()>;
}

#[async_trait]
impl AgentTimeTriggerDelivery for AgentManager {
    async fn deliver_trigger_message(
        &self,
        agent_id: &str,
        message: &str,
        message_id: &str,
        source: &AgentReminderSource,
    ) -> anyhow::Result<()> {
        self.send_reminder_input(agent_id, message, message_id, source)
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

    pub fn spawn(
        self,
        daemon_tasks: &crate::daemon_tasks::DaemonTaskGroup,
        settings: AgentTimeTriggerWorkerSettings,
    ) -> anyhow::Result<()> {
        let cancellation = daemon_tasks.background_cancellation();
        daemon_tasks.spawn_background_worker("agent-time-triggers", async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(
                settings.poll_interval_secs.max(1) as u64,
            ));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = cancellation.cancelled() => break,
                    _ = ticker.tick() => {}
                }
                tokio::select! {
                    _ = cancellation.cancelled() => break,
                    result = self.dispatch_once(settings.max_dispatch_per_tick) => {
                        if let Err(error) = result {
                            tracing::warn!(?error, "agent time trigger worker tick failed");
                        }
                    }
                }
            }
            Ok(())
        })
    }

    pub async fn dispatch_once(&self, max_dispatch_per_tick: i64) -> anyhow::Result<usize> {
        let claimed = self
            .triggers
            .claim_due_triggers(Utc::now().timestamp(), max_dispatch_per_tick)
            .await?;
        // Bounded concurrency prevents a slow or stopped agent from blocking the batch.
        // The largest batch completes within the lease even if every attempt times out.
        let results = stream::iter(claimed)
            .map(|trigger| async move {
                if !self
                    .triggers
                    .owns_lease(&trigger, Utc::now().timestamp())
                    .await?
                {
                    return Ok(false);
                }
                let message_id = format!("time-trigger:{}", trigger.id);
                let prompt = build_time_trigger_prompt(&trigger);
                let result = tokio::time::timeout(
                    DELIVERY_TIMEOUT,
                    self.delivery.deliver_trigger_message(
                        &trigger.agent_id,
                        &prompt,
                        &message_id,
                        &trigger.source,
                    ),
                )
                .await
                .context("reminder submission timed out")
                .and_then(|result| result);
                let now = Utc::now().timestamp();
                match result {
                    Ok(()) => self.triggers.mark_trigger_fired(&trigger, now).await,
                    Err(error) => {
                        self.triggers
                            .requeue_trigger(&trigger, now, &error.to_string())
                            .await?;
                        Ok(false)
                    }
                }
            })
            .buffer_unordered(8)
            .collect::<Vec<anyhow::Result<bool>>>()
            .await;
        let mut delivered = 0;
        for result in results {
            delivered += usize::from(result?);
        }
        Ok(delivered)
    }
}

impl AgentTimeTriggerManager {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn reset_inflight_on_startup(&self) -> anyhow::Result<u64> {
        // A second daemon must not steal a live lease. Legacy rows have no lease.
        let result = sqlx::query(
            r#"
            UPDATE agent_time_triggers
            SET status = 'scheduled', lease_expires_at = NULL, updated_at = ?1
             WHERE status = 'dispatching' AND (lease_expires_at IS NULL OR lease_expires_at <= ?1)
            "#,
        )
        .bind(Utc::now().timestamp())
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn create_time_trigger(
        &self,
        input: AgentTimeTriggerCreateInput,
    ) -> anyhow::Result<AgentTimeTriggerRecord> {
        let message_text = input.message_text.trim();
        anyhow::ensure!(
            !message_text.is_empty(),
            "time trigger message must not be empty"
        );
        anyhow::ensure!(
            message_text.len() <= 16_384,
            "time trigger message exceeds 16384 bytes"
        );
        let now = Utc::now().timestamp();
        let fire_at = match input.schedule {
            AgentTimeTriggerSchedule::At(fire_at) => fire_at,
            AgentTimeTriggerSchedule::After(delay_seconds) => {
                anyhow::ensure!(
                    (1..=2_592_000).contains(&delay_seconds),
                    "delay_seconds must be between 1 and 2592000"
                );
                now + delay_seconds
            }
        };
        anyhow::ensure!(fire_at > now, "time trigger fire_at must be in the future");
        anyhow::ensure!(
            fire_at <= now + 2_592_000,
            "time trigger must be within 30 days"
        );
        anyhow::ensure!(
            input
                .source
                .reference
                .as_ref()
                .is_none_or(|value| value.len() <= 1024),
            "source reference exceeds 1024 bytes"
        );
        let trigger_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO agent_time_triggers (
                id, agent_id, kind, created_by_actor_id, message_text,
                fire_at, status, created_at, updated_at, source_json
            )
             VALUES (?1, ?2, 'time', ?3, ?4, ?5, 'scheduled', ?6, ?6, ?7)
            "#,
        )
        .bind(&trigger_id)
        .bind(input.agent_id.trim())
        .bind(input.created_by_actor_id.trim())
        .bind(message_text)
        .bind(fire_at)
        .bind(now)
        .bind(serde_json::to_string(&input.source)?)
        .execute(&self.db)
        .await?;
        self.get_trigger(&trigger_id).await
    }

    pub async fn list_triggers_for_agent(
        &self,
        agent_id: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<AgentTimeTriggerRecord>> {
        let rows = sqlx::query(
            "SELECT * FROM agent_time_triggers WHERE agent_id = ?1
             ORDER BY created_at DESC, id DESC LIMIT ?2",
        )
        .bind(agent_id)
        .bind(limit.clamp(1, 500))
        .fetch_all(&self.db)
        .await?;
        rows.into_iter().map(record_from_row).collect()
    }

    pub async fn cancel_trigger(&self, agent_id: &str, trigger_id: &str) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE agent_time_triggers
            SET status = 'canceled', lease_expires_at = NULL, updated_at = ?1
             WHERE id = ?2 AND agent_id = ?3 AND status IN ('scheduled', 'dispatching')
            "#,
        )
        .bind(Utc::now().timestamp())
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
        // Selection and ownership transfer are one statement, including expired leases.
        let rows = sqlx::query(
            r#"
            UPDATE agent_time_triggers
             SET status = 'dispatching', attempt = attempt + 1, lease_expires_at = ?3, updated_at = ?1
             WHERE id IN (
                 SELECT id FROM agent_time_triggers WHERE fire_at <= ?1 AND (
                     (status = 'scheduled' AND next_attempt_at <= ?1) OR
                     (status = 'dispatching' AND (lease_expires_at IS NULL OR lease_expires_at <= ?1))
                 ) ORDER BY next_attempt_at, fire_at, created_at, id LIMIT ?2
             ) RETURNING *
            "#,
        )
        .bind(now)
        .bind(limit.clamp(1, MAX_BATCH))
        .bind(now + LEASE_SECONDS)
        .fetch_all(&self.db)
        .await?;
        rows.into_iter().map(record_from_row).collect()
    }

    async fn owns_lease(&self, trigger: &AgentTimeTriggerRecord, now: i64) -> anyhow::Result<bool> {
        Ok(sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM agent_time_triggers
                WHERE id = ?1 AND status = 'dispatching'
                  AND attempt = ?2 AND lease_expires_at > ?3
            )
            "#,
        )
        .bind(&trigger.id)
        .bind(trigger.attempt)
        .bind(now)
        .fetch_one(&self.db)
        .await?)
    }

    async fn mark_trigger_fired(
        &self,
        trigger: &AgentTimeTriggerRecord,
        now: i64,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE agent_time_triggers
            SET status = 'fired', fired_at = ?1, updated_at = ?1,
                last_error = NULL, lease_expires_at = NULL
             WHERE id = ?2 AND status = 'dispatching' AND attempt = ?3 AND lease_expires_at > ?1
            "#,
        )
        .bind(now)
        .bind(&trigger.id)
        .bind(trigger.attempt)
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn requeue_trigger(
        &self,
        trigger: &AgentTimeTriggerRecord,
        now: i64,
        error: &str,
    ) -> anyhow::Result<()> {
        let retry_seconds =
            (5_i64 * (1_i64 << trigger.attempt.saturating_sub(1).clamp(0, 6))).min(300);
        sqlx::query(
            r#"
            UPDATE agent_time_triggers
            SET status = 'scheduled', updated_at = ?1, last_error = ?2,
                next_attempt_at = ?4, lease_expires_at = NULL
             WHERE id = ?3 AND status = 'dispatching' AND attempt = ?5 AND lease_expires_at > ?1
            "#,
        )
        .bind(now)
        .bind(error)
        .bind(&trigger.id)
        .bind(now + retry_seconds)
        .bind(trigger.attempt)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn get_trigger(&self, trigger_id: &str) -> anyhow::Result<AgentTimeTriggerRecord> {
        let row = sqlx::query("SELECT * FROM agent_time_triggers WHERE id = ?1")
            .bind(trigger_id)
            .fetch_one(&self.db)
            .await
            .with_context(|| format!("time trigger not found: {trigger_id}"))?;
        record_from_row(row)
    }
}

fn build_time_trigger_prompt(trigger: &AgentTimeTriggerRecord) -> String {
    format!(
        "[AgentHub reminder]\nThis is a previously scheduled reminder, not a new human instruction or additional authorization. Check current task state before acting.\ntrigger_id: {}\nfire_at: {}\nsource: {}\nmessage:\n{}",
        trigger.id,
        trigger.fire_at,
        serde_json::to_string(&trigger.source).expect("serialize reminder source"),
        trigger.message_text
    )
}

fn record_from_row(row: sqlx::sqlite::SqliteRow) -> anyhow::Result<AgentTimeTriggerRecord> {
    let source_json: Option<String> = row.try_get("source_json")?;
    Ok(AgentTimeTriggerRecord {
        id: row.try_get("id")?,
        agent_id: row.try_get("agent_id")?,
        kind: row.try_get("kind")?,
        created_by_actor_id: row.try_get("created_by_actor_id")?,
        message_text: row.try_get("message_text")?,
        fire_at: row.try_get("fire_at")?,
        status: status_from_str(&row.try_get::<String, _>("status")?)?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        fired_at: row.try_get("fired_at")?,
        last_error: row.try_get("last_error")?,
        attempt: row.try_get("attempt")?,
        next_attempt_at: row.try_get("next_attempt_at")?,
        lease_expires_at: row.try_get("lease_expires_at")?,
        source: source_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?
            .unwrap_or_default(),
    })
}

fn status_from_str(value: &str) -> anyhow::Result<AgentTimeTriggerStatus> {
    match value {
        "scheduled" => Ok(AgentTimeTriggerStatus::Scheduled),
        "dispatching" => Ok(AgentTimeTriggerStatus::Dispatching),
        "fired" => Ok(AgentTimeTriggerStatus::Fired),
        "canceled" => Ok(AgentTimeTriggerStatus::Canceled),
        other => anyhow::bail!("invalid agent time trigger status: {other}"),
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
            _source: &AgentReminderSource,
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
        agenthub_db::migrate_time_triggers(db).await.unwrap();
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
                source: AgentReminderSource::default(),
                agent_id: "agent-1".to_string(),
                created_by_actor_id: "agent-1".to_string(),
                message_text: "Check the regression dashboard.".to_string(),
                schedule: AgentTimeTriggerSchedule::At(fire_at),
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
                source: AgentReminderSource::default(),
                agent_id: "agent-1".to_string(),
                created_by_actor_id: "agent-1".to_string(),
                message_text: "Re-check the issue queue.".to_string(),
                schedule: AgentTimeTriggerSchedule::At(fire_at),
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
        assert!(deliveries[0].1.contains("AgentHub reminder"));
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
                source: AgentReminderSource::default(),
                agent_id: "agent-1".to_string(),
                created_by_actor_id: "agent-1".to_string(),
                message_text: "Retry later.".to_string(),
                schedule: AgentTimeTriggerSchedule::At(Utc::now().timestamp() + 60),
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
    async fn due_trigger(
        manager: &AgentTimeTriggerManager,
        db: &SqlitePool,
    ) -> AgentTimeTriggerRecord {
        let trigger = manager
            .create_time_trigger(AgentTimeTriggerCreateInput {
                agent_id: "agent-1".into(),
                created_by_actor_id: "agent-1".into(),
                message_text: "Check task status".into(),
                schedule: AgentTimeTriggerSchedule::At(Utc::now().timestamp() + 60),
                source: AgentReminderSource {
                    scope_bound: true,
                    session_id: Some("old-session".into()),
                    reference: Some("task:123".into()),
                    ..Default::default()
                },
            })
            .await
            .unwrap();
        sqlx::query("UPDATE agent_time_triggers SET fire_at = ?1 WHERE id = ?2")
            .bind(Utc::now().timestamp() - 1)
            .bind(&trigger.id)
            .execute(db)
            .await
            .unwrap();
        trigger
    }

    #[tokio::test]
    async fn reminder_cancel_fences_both_success_and_failure_of_claimed_attempt() {
        let state = build_test_state().await;
        init_trigger_test_fixtures(&state.db).await;
        let manager = AgentTimeTriggerManager::new(state.db.clone());
        let trigger = due_trigger(&manager, &state.db).await;
        let now = Utc::now().timestamp();
        let claimed = manager.claim_due_triggers(now, 1).await.unwrap().remove(0);
        assert!(
            !manager
                .cancel_trigger("another-agent", &trigger.id)
                .await
                .unwrap()
        );
        assert!(
            manager
                .cancel_trigger("agent-1", &trigger.id)
                .await
                .unwrap()
        );
        assert!(!manager.owns_lease(&claimed, now).await.unwrap());
        assert!(!manager.mark_trigger_fired(&claimed, now).await.unwrap());
        manager
            .requeue_trigger(&claimed, now, "late failure")
            .await
            .unwrap();
        let canceled = manager.get_trigger(&trigger.id).await.unwrap();
        assert_eq!(canceled.status, AgentTimeTriggerStatus::Canceled);
        assert_eq!(canceled.fired_at, None);
        assert_eq!(canceled.last_error, None);
    }

    #[tokio::test]
    async fn reminder_recovery_does_not_steal_live_leases_and_fences_stale_attempts() {
        let state = build_test_state().await;
        init_trigger_test_fixtures(&state.db).await;
        let manager = AgentTimeTriggerManager::new(state.db.clone());
        due_trigger(&manager, &state.db).await;
        let now = Utc::now().timestamp();
        let first = manager.claim_due_triggers(now, 1).await.unwrap().remove(0);
        assert_eq!(manager.reset_inflight_on_startup().await.unwrap(), 0);
        assert!(
            manager
                .claim_due_triggers(now + 1, 1)
                .await
                .unwrap()
                .is_empty()
        );
        let next = manager
            .claim_due_triggers(now + LEASE_SECONDS, 1)
            .await
            .unwrap()
            .remove(0);
        assert_eq!(next.attempt, first.attempt + 1);
        assert_eq!(next.source.reference.as_deref(), Some("task:123"));
        assert!(
            !manager
                .mark_trigger_fired(&first, now + LEASE_SECONDS)
                .await
                .unwrap()
        );
        manager
            .requeue_trigger(&first, now + LEASE_SECONDS, "stale failure")
            .await
            .unwrap();
        assert!(
            manager
                .mark_trigger_fired(&next, now + LEASE_SECONDS)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn reminder_backoff_preserves_deadline_and_allows_other_due_work() {
        let state = build_test_state().await;
        init_trigger_test_fixtures(&state.db).await;
        let manager = AgentTimeTriggerManager::new(state.db.clone());
        due_trigger(&manager, &state.db).await;
        let now = Utc::now().timestamp();
        let first = manager.claim_due_triggers(now, 1).await.unwrap().remove(0);
        manager
            .requeue_trigger(&first, now, "agent stopped")
            .await
            .unwrap();
        let retry = manager.get_trigger(&first.id).await.unwrap();
        assert_eq!(retry.fire_at, first.fire_at);
        assert_eq!(retry.next_attempt_at, now + 5);
        assert!(
            manager
                .claim_due_triggers(now + 4, 1)
                .await
                .unwrap()
                .is_empty()
        );
        let second = due_trigger(&manager, &state.db).await;
        let claimed = manager.claim_due_triggers(now + 4, 1).await.unwrap();
        assert_eq!(claimed[0].id, second.id);
        assert_eq!(
            manager.claim_due_triggers(now + 5, 1).await.unwrap()[0].id,
            first.id
        );
    }

    #[tokio::test]
    async fn reminder_workers_claim_each_due_row_only_once() {
        let state = build_test_state().await;
        init_trigger_test_fixtures(&state.db).await;
        let manager = AgentTimeTriggerManager::new(state.db.clone());
        due_trigger(&manager, &state.db).await;
        let now = Utc::now().timestamp();
        let (a, b) = tokio::join!(
            manager.claim_due_triggers(now, 1),
            manager.claim_due_triggers(now, 1)
        );
        assert_eq!(a.unwrap().len() + b.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn reminder_delivery_does_not_start_stopped_agent() {
        let state = build_test_state().await;
        init_trigger_test_fixtures(&state.db).await;
        let manager = Arc::new(AgentTimeTriggerManager::new(state.db.clone()));
        let trigger = due_trigger(&manager, &state.db).await;
        let worker = AgentTimeTriggerWorker::new(manager.clone(), state.agents.clone());
        assert_eq!(worker.dispatch_once(1).await.unwrap(), 0);
        assert!(
            state
                .agents
                .running_session_id_for_agent("agent-1")
                .await
                .is_none()
        );
        assert_eq!(
            manager.get_trigger(&trigger.id).await.unwrap().status,
            AgentTimeTriggerStatus::Scheduled
        );
    }
    #[tokio::test]
    async fn reminder_slow_delivery_does_not_block_other_work_or_overwrite_cancel() {
        struct GatedDelivery {
            blocked_id: String,
            entered: tokio::sync::Notify,
            release: tokio::sync::Notify,
            healthy: tokio::sync::Notify,
        }
        #[async_trait]
        impl AgentTimeTriggerDelivery for GatedDelivery {
            async fn deliver_trigger_message(
                &self,
                _: &str,
                _: &str,
                message_id: &str,
                _: &AgentReminderSource,
            ) -> anyhow::Result<()> {
                if message_id == self.blocked_id {
                    self.entered.notify_one();
                    self.release.notified().await;
                } else {
                    self.healthy.notify_one();
                }
                Ok(())
            }
        }
        let state = build_test_state().await;
        init_trigger_test_fixtures(&state.db).await;
        let manager = Arc::new(AgentTimeTriggerManager::new(state.db.clone()));
        let blocked = due_trigger(&manager, &state.db).await;
        let healthy = due_trigger(&manager, &state.db).await;
        let delivery = Arc::new(GatedDelivery {
            blocked_id: format!("time-trigger:{}", blocked.id),
            entered: Default::default(),
            release: Default::default(),
            healthy: Default::default(),
        });
        let worker = AgentTimeTriggerWorker::new(manager.clone(), delivery.clone());
        let dispatch = tokio::spawn(async move { worker.dispatch_once(2).await });
        tokio::time::timeout(Duration::from_secs(2), delivery.entered.notified())
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), delivery.healthy.notified())
            .await
            .unwrap();
        assert!(
            manager
                .cancel_trigger("agent-1", &blocked.id)
                .await
                .unwrap()
        );
        delivery.release.notify_one();
        assert_eq!(dispatch.await.unwrap().unwrap(), 1);
        assert_eq!(
            manager.get_trigger(&blocked.id).await.unwrap().status,
            AgentTimeTriggerStatus::Canceled
        );
        assert_eq!(
            manager.get_trigger(&healthy.id).await.unwrap().status,
            AgentTimeTriggerStatus::Fired
        );
    }

    #[tokio::test]
    async fn reminder_submission_timeout_releases_lease_with_backoff() {
        struct NeverReady;
        #[async_trait]
        impl AgentTimeTriggerDelivery for NeverReady {
            async fn deliver_trigger_message(
                &self,
                _: &str,
                _: &str,
                _: &str,
                _: &AgentReminderSource,
            ) -> anyhow::Result<()> {
                std::future::pending().await
            }
        }
        let state = build_test_state().await;
        init_trigger_test_fixtures(&state.db).await;
        let manager = Arc::new(AgentTimeTriggerManager::new(state.db.clone()));
        let trigger = due_trigger(&manager, &state.db).await;
        let worker = AgentTimeTriggerWorker::new(manager.clone(), Arc::new(NeverReady));
        let result = tokio::time::timeout(
            DELIVERY_TIMEOUT + Duration::from_secs(5),
            worker.dispatch_once(1),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result, 0);
        let retry = manager.get_trigger(&trigger.id).await.unwrap();
        assert_eq!(retry.status, AgentTimeTriggerStatus::Scheduled);
        assert_eq!(retry.lease_expires_at, None);
        assert!(retry.next_attempt_at > retry.updated_at);
        assert!(retry.last_error.unwrap().contains("submission timed out"));
    }
    #[tokio::test]
    async fn reminder_relative_deadline_uses_the_creation_timestamp() {
        let state = build_test_state().await;
        init_trigger_test_fixtures(&state.db).await;
        let manager = AgentTimeTriggerManager::new(state.db.clone());
        let trigger = manager
            .create_time_trigger(AgentTimeTriggerCreateInput {
                agent_id: "agent-1".into(),
                created_by_actor_id: "agent-1".into(),
                message_text: "Recheck shortly".into(),
                schedule: AgentTimeTriggerSchedule::After(1),
                source: AgentReminderSource::default(),
            })
            .await
            .unwrap();
        assert_eq!(trigger.fire_at, trigger.created_at + 1);
    }
}
