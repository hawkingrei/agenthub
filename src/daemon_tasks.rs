use std::any::Any;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::FutureExt;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

/// Owns daemon-lifetime tasks in the order required by process shutdown.
///
/// Background ingress stops before supervised processes. Runtime readers and watchers remain alive
/// until process shutdown completes, then drain in the second phase.
#[derive(Clone, Default)]
pub(crate) struct DaemonTaskGroup {
    background: DaemonTaskPhase,
    runtime: DaemonTaskPhase,
}

#[derive(Clone)]
struct DaemonTaskPhase {
    cancellation: CancellationToken,
    tracker: TaskTracker,
    accepting: Arc<Mutex<bool>>,
    failures: Arc<Mutex<Vec<String>>>,
}

impl Default for DaemonTaskPhase {
    fn default() -> Self {
        Self {
            cancellation: CancellationToken::new(),
            tracker: TaskTracker::new(),
            accepting: Arc::new(Mutex::new(true)),
            failures: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl DaemonTaskGroup {
    pub(crate) fn background_cancellation(&self) -> CancellationToken {
        self.background.cancellation.clone()
    }

    pub(crate) fn runtime_cancellation(&self) -> CancellationToken {
        self.runtime.cancellation.clone()
    }

    pub(crate) fn spawn_background_worker<F>(
        &self,
        name: impl Into<String>,
        future: F,
    ) -> anyhow::Result<()>
    where
        F: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        self.background.spawn(name.into(), future, false)
    }

    pub(crate) fn spawn_background_job<F>(
        &self,
        name: impl Into<String>,
        future: F,
    ) -> anyhow::Result<()>
    where
        F: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        self.background.spawn(name.into(), future, true)
    }

    pub(crate) fn spawn_runtime_task<F>(
        &self,
        name: impl Into<String>,
        future: F,
    ) -> anyhow::Result<()>
    where
        F: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        self.runtime.spawn(name.into(), future, true)
    }

    pub(crate) async fn shutdown_background(&self, timeout: Duration) -> anyhow::Result<()> {
        self.background.shutdown("background", timeout).await
    }

    pub(crate) async fn shutdown_runtime(&self, timeout: Duration) -> anyhow::Result<()> {
        self.runtime.shutdown("runtime", timeout).await
    }

    #[cfg(test)]
    pub(crate) fn background_task_count(&self) -> usize {
        self.background.tracker.len()
    }

    #[cfg(test)]
    pub(crate) fn runtime_task_count(&self) -> usize {
        self.runtime.tracker.len()
    }
}

impl DaemonTaskPhase {
    fn spawn<F>(&self, name: String, future: F, expected_completion: bool) -> anyhow::Result<()>
    where
        F: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let accepting = self
            .accepting
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !*accepting {
            anyhow::bail!("daemon task phase is shutting down; rejected task '{name}'");
        }

        let cancellation = self.cancellation.clone();
        let failures = self.failures.clone();
        drop(self.tracker.spawn(async move {
            let outcome = AssertUnwindSafe(future).catch_unwind().await;
            let failure = match outcome {
                Ok(Ok(())) if expected_completion || cancellation.is_cancelled() => None,
                Ok(Ok(())) => Some(format!("task '{name}' exited unexpectedly")),
                Ok(Err(error)) => Some(format!("task '{name}' failed: {error:#}")),
                Err(payload) => Some(format!(
                    "task '{name}' panicked: {}",
                    panic_payload_message(payload.as_ref())
                )),
            };
            if let Some(failure) = failure {
                tracing::error!(failure, "tracked daemon task failed");
                lock_failures(&failures).push(failure);
            }
        }));
        Ok(())
    }

    async fn shutdown(&self, phase: &str, timeout: Duration) -> anyhow::Result<()> {
        {
            let mut accepting = self
                .accepting
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *accepting = false;
            self.cancellation.cancel();
            self.tracker.close();
        }

        if tokio::time::timeout(timeout, self.tracker.wait())
            .await
            .is_err()
        {
            lock_failures(&self.failures).push(format!(
                "{phase} task join timed out after {} ms with {} task(s) remaining",
                timeout.as_millis(),
                self.tracker.len()
            ));
        }

        let failures = std::mem::take(&mut *lock_failures(&self.failures));
        if failures.is_empty() {
            Ok(())
        } else {
            anyhow::bail!(
                "daemon {phase} task shutdown reported {} failure(s): {}",
                failures.len(),
                failures.join("; ")
            )
        }
    }
}

fn lock_failures(failures: &Mutex<Vec<String>>) -> std::sync::MutexGuard<'_, Vec<String>> {
    failures
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn panic_payload_message(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("non-string panic payload")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn shutdown_cancels_background_before_runtime() {
        let tasks = DaemonTaskGroup::default();
        let (events_tx, mut events_rx) = mpsc::unbounded_channel();

        let background_cancel = tasks.background_cancellation();
        let background_events = events_tx.clone();
        tasks
            .spawn_background_worker("background-probe", async move {
                background_cancel.cancelled().await;
                background_events
                    .send("background")
                    .expect("record background cancellation");
                Ok(())
            })
            .expect("spawn background probe");

        let runtime_cancel = tasks.runtime_cancellation();
        tasks
            .spawn_runtime_task("runtime-probe", async move {
                runtime_cancel.cancelled().await;
                events_tx
                    .send("runtime")
                    .expect("record runtime cancellation");
                Ok(())
            })
            .expect("spawn runtime probe");

        tasks
            .shutdown_background(Duration::from_secs(1))
            .await
            .expect("shutdown background phase");
        assert_eq!(events_rx.recv().await, Some("background"));
        assert_eq!(tasks.background_task_count(), 0);
        assert_eq!(tasks.runtime_task_count(), 1);
        assert!(events_rx.try_recv().is_err());

        tasks
            .shutdown_runtime(Duration::from_secs(1))
            .await
            .expect("shutdown runtime phase");
        assert_eq!(events_rx.recv().await, Some("runtime"));
        assert_eq!(tasks.runtime_task_count(), 0);
    }

    #[tokio::test]
    async fn shutdown_surfaces_panics_and_rejects_new_tasks() {
        let tasks = DaemonTaskGroup::default();
        tasks
            .spawn_background_worker("panic-probe", async move {
                panic!("probe panic");
            })
            .expect("spawn panic probe");

        tokio::task::yield_now().await;
        let error = tasks
            .shutdown_background(Duration::from_secs(1))
            .await
            .expect_err("surface tracked panic");
        assert!(error.to_string().contains("panic-probe"));
        assert!(error.to_string().contains("probe panic"));
        assert!(
            tasks
                .spawn_background_job("late-task", async { Ok(()) })
                .is_err()
        );
    }

    #[tokio::test]
    async fn shutdown_reports_tasks_that_ignore_cancellation() {
        let tasks = DaemonTaskGroup::default();
        tasks
            .spawn_background_job("stuck-probe", std::future::pending())
            .expect("spawn stuck probe");

        let error = tasks
            .shutdown_background(Duration::from_millis(10))
            .await
            .expect_err("surface join timeout");
        assert!(error.to_string().contains("timed out"));
        assert!(error.to_string().contains("1 task(s) remaining"));
    }
}
