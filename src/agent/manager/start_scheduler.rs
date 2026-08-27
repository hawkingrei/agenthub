use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

const DEFAULT_MAX_CONCURRENT_STARTS: usize = 4;
const DEFAULT_QUEUE_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_START_TIMEOUT: Duration = Duration::from_secs(120);
const DEFAULT_SPAWN_BACKOFF_INITIAL: Duration = Duration::from_millis(250);
const DEFAULT_SPAWN_BACKOFF_MAX: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AgentStartSchedulerSettings {
    pub max_concurrent_starts: usize,
    pub queue_timeout: Duration,
    pub start_timeout: Duration,
    pub spawn_backoff_initial: Duration,
    pub spawn_backoff_max: Duration,
}

impl Default for AgentStartSchedulerSettings {
    fn default() -> Self {
        Self {
            max_concurrent_starts: DEFAULT_MAX_CONCURRENT_STARTS,
            queue_timeout: DEFAULT_QUEUE_TIMEOUT,
            start_timeout: DEFAULT_START_TIMEOUT,
            spawn_backoff_initial: DEFAULT_SPAWN_BACKOFF_INITIAL,
            spawn_backoff_max: DEFAULT_SPAWN_BACKOFF_MAX,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct AgentStartScheduler {
    settings: AgentStartSchedulerSettings,
    permits: Arc<Semaphore>,
    spawn_failures: Arc<Mutex<HashMap<String, SpawnFailureState>>>,
}

#[derive(Debug, Clone, Copy)]
struct SpawnFailureState {
    consecutive_failures: u32,
    retry_at: Instant,
}

#[derive(Debug)]
pub(super) struct AgentStartAdmission {
    _permit: OwnedSemaphorePermit,
    start_timeout: Duration,
}

impl AgentStartAdmission {
    pub(super) fn start_timeout(&self) -> Duration {
        self.start_timeout
    }
}

impl AgentStartScheduler {
    pub(super) fn new(settings: AgentStartSchedulerSettings) -> Self {
        let max_concurrent_starts = settings.max_concurrent_starts.max(1);
        let settings = AgentStartSchedulerSettings {
            max_concurrent_starts,
            spawn_backoff_max: settings
                .spawn_backoff_max
                .max(settings.spawn_backoff_initial),
            ..settings
        };
        Self {
            settings,
            permits: Arc::new(Semaphore::new(max_concurrent_starts)),
            spawn_failures: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(super) async fn acquire(&self, agent_id: &str) -> anyhow::Result<AgentStartAdmission> {
        self.reject_active_backoff(agent_id).await?;
        let queued_at = Instant::now();
        let permit = tokio::time::timeout(
            self.settings.queue_timeout,
            self.permits.clone().acquire_owned(),
        )
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "agent start queue timed out: agent_id={agent_id} timeout_ms={}",
                self.settings.queue_timeout.as_millis()
            )
        })?
        .map_err(|_| anyhow::anyhow!("agent start scheduler is closed"))?;
        self.reject_active_backoff(agent_id).await?;

        tracing::debug!(
            agent_id,
            queue_wait_ms = queued_at.elapsed().as_millis(),
            max_concurrent_starts = self.settings.max_concurrent_starts,
            "agent start admitted"
        );
        Ok(AgentStartAdmission {
            _permit: permit,
            start_timeout: self.settings.start_timeout,
        })
    }

    pub(super) async fn record_spawn_failure(&self, agent_id: &str) -> Duration {
        let mut failures = self.spawn_failures.lock().await;
        let consecutive_failures = failures
            .get(agent_id)
            .map(|state| state.consecutive_failures.saturating_add(1))
            .unwrap_or(1);
        let exponent = consecutive_failures.saturating_sub(1).min(31);
        let multiplier = 1_u32 << exponent;
        let delay = self
            .settings
            .spawn_backoff_initial
            .saturating_mul(multiplier)
            .min(self.settings.spawn_backoff_max);
        failures.insert(
            agent_id.to_string(),
            SpawnFailureState {
                consecutive_failures,
                retry_at: Instant::now() + delay,
            },
        );
        delay
    }

    pub(super) async fn clear_spawn_failures(&self, agent_id: &str) {
        self.spawn_failures.lock().await.remove(agent_id);
    }

    async fn reject_active_backoff(&self, agent_id: &str) -> anyhow::Result<()> {
        let now = Instant::now();
        let failures = self.spawn_failures.lock().await;
        let Some(state) = failures.get(agent_id).copied() else {
            return Ok(());
        };
        if state.retry_at <= now {
            return Ok(());
        }
        anyhow::bail!(
            "agent spawn backoff active: agent_id={agent_id} retry_after_ms={} consecutive_failures={}",
            state.retry_at.saturating_duration_since(now).as_millis(),
            state.consecutive_failures
        )
    }
}

impl Default for AgentStartScheduler {
    fn default() -> Self {
        Self::new(AgentStartSchedulerSettings::default())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{AgentStartScheduler, AgentStartSchedulerSettings};

    fn test_settings() -> AgentStartSchedulerSettings {
        AgentStartSchedulerSettings {
            max_concurrent_starts: 1,
            queue_timeout: Duration::from_millis(25),
            start_timeout: Duration::from_millis(50),
            spawn_backoff_initial: Duration::from_millis(20),
            spawn_backoff_max: Duration::from_millis(40),
        }
    }

    #[tokio::test]
    async fn bounds_concurrent_start_admission_and_times_out_queue_waiters() {
        let scheduler = AgentStartScheduler::new(test_settings());
        let first = scheduler.acquire("agent-1").await.expect("first admission");

        let error = scheduler
            .acquire("agent-2")
            .await
            .expect_err("second admission should time out");
        assert!(error.to_string().contains("start queue timed out"));

        drop(first);
        let second = scheduler
            .acquire("agent-2")
            .await
            .expect("permit released after first admission");
        assert_eq!(second.start_timeout(), Duration::from_millis(50));
    }

    #[tokio::test]
    async fn spawn_backoff_grows_caps_and_clears_after_success() {
        let scheduler = AgentStartScheduler::new(test_settings());

        assert_eq!(
            scheduler.record_spawn_failure("agent-1").await,
            Duration::from_millis(20)
        );
        assert!(scheduler.acquire("agent-1").await.is_err());
        tokio::time::sleep(Duration::from_millis(25)).await;
        assert_eq!(
            scheduler.record_spawn_failure("agent-1").await,
            Duration::from_millis(40)
        );
        assert_eq!(
            scheduler.record_spawn_failure("agent-1").await,
            Duration::from_millis(40)
        );

        scheduler.clear_spawn_failures("agent-1").await;
        scheduler
            .acquire("agent-1")
            .await
            .expect("successful spawn clears backoff");
    }
}
