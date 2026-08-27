use std::collections::HashMap;
use std::process::ExitStatus;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Context;
use futures::future::join_all;
#[cfg(windows)]
use process_wrap::tokio::JobObject;
#[cfg(unix)]
use process_wrap::tokio::ProcessGroup;
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use tokio::process::Command;
use tokio::sync::{Mutex, OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

pub(super) type SupervisedChild = Box<dyn ChildWrapper>;
pub(super) type SharedSupervisedChild = Arc<Mutex<Option<SupervisedChild>>>;

const DEFAULT_GRACEFUL_STOP_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_FORCE_STOP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
pub(super) struct AgentProcessSupervisor {
    graceful_stop_timeout: Duration,
    force_stop_timeout: Duration,
    lifecycle: Arc<RwLock<()>>,
    shutting_down: Arc<AtomicBool>,
    processes: Arc<RwLock<HashMap<String, StopTarget>>>,
}

#[derive(Clone, Debug)]
pub(super) struct StopTarget {
    pub agent_id: String,
    pub session_id: String,
    pub child: SharedSupervisedChild,
}

pub(super) struct PendingProcessRegistration {
    supervisor: AgentProcessSupervisor,
    session_id: String,
    child: SharedSupervisedChild,
    committed: bool,
}

impl PendingProcessRegistration {
    pub(super) fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for PendingProcessRegistration {
    fn drop(&mut self) {
        if self.committed {
            return;
        }

        let supervisor = self.supervisor.clone();
        let session_id = self.session_id.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                if let Err(error) = supervisor.stop_session(&session_id).await {
                    tracing::error!(
                        session_id = %session_id,
                        error = %error,
                        "failed to stop uncommitted supervised process"
                    );
                }
            });
        } else if let Ok(mut guard) = self.child.try_lock()
            && let Some(process) = guard.as_mut()
            && let Err(error) = process.start_kill()
        {
            tracing::error!(
                session_id = %self.session_id,
                error = %error,
                "failed to synchronously kill uncommitted supervised process"
            );
        }
    }
}

impl Default for AgentProcessSupervisor {
    fn default() -> Self {
        Self::new(DEFAULT_GRACEFUL_STOP_TIMEOUT, DEFAULT_FORCE_STOP_TIMEOUT)
    }
}

impl AgentProcessSupervisor {
    fn new(graceful_stop_timeout: Duration, force_stop_timeout: Duration) -> Self {
        Self {
            graceful_stop_timeout,
            force_stop_timeout,
            lifecycle: Arc::new(RwLock::new(())),
            shutting_down: Arc::new(AtomicBool::new(false)),
            processes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub(super) async fn spawn(
        &self,
        agent_id: String,
        session_id: String,
        command: Command,
    ) -> anyhow::Result<(SharedSupervisedChild, PendingProcessRegistration)> {
        if self.shutting_down.load(Ordering::Acquire) {
            anyhow::bail!("agent process supervisor is shutting down");
        }
        let child = Arc::new(Mutex::new(Some(
            spawn_supervised(command).context("failed to spawn supervised agent process")?,
        )));
        let target = StopTarget {
            agent_id,
            session_id: session_id.clone(),
            child: child.clone(),
        };
        if self.track(target).await.is_err() {
            let _ = self.stop(&child).await;
            anyhow::bail!("duplicate supervised agent session: {session_id}");
        }

        Ok((
            child.clone(),
            PendingProcessRegistration {
                supervisor: self.clone(),
                session_id,
                child,
                committed: false,
            },
        ))
    }

    pub(super) async fn acquire_start_permit(&self) -> anyhow::Result<OwnedRwLockReadGuard<()>> {
        if self.shutting_down.load(Ordering::Acquire) {
            anyhow::bail!("agent process supervisor is shutting down");
        }
        let guard = self.lifecycle.clone().read_owned().await;
        if self.shutting_down.load(Ordering::Acquire) {
            anyhow::bail!("agent process supervisor is shutting down");
        }
        Ok(guard)
    }

    pub(super) async fn begin_shutdown(&self) -> OwnedRwLockWriteGuard<()> {
        let guard = self.lifecycle.clone().write_owned().await;
        self.shutting_down.store(true, Ordering::Release);
        guard
    }

    pub(super) async fn stop_all(&self) -> anyhow::Result<()> {
        let targets = self
            .processes
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let results = join_all(targets.into_iter().map(|target| async move {
            let result = self.stop(&target.child).await;
            (target, result)
        }))
        .await;

        let mut failures = Vec::new();
        for (target, result) in results {
            match result {
                Ok(_) => self.forget(&target.session_id).await,
                Err(error) => failures.push(format!(
                    "agent_id={} session_id={}: {error:#}",
                    target.agent_id, target.session_id
                )),
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            anyhow::bail!(
                "failed to stop {} supervised agent process(es): {}",
                failures.len(),
                failures.join("; ")
            )
        }
    }

    pub(super) async fn stop_session(
        &self,
        session_id: &str,
    ) -> anyhow::Result<Option<ExitStatus>> {
        let target = self.processes.read().await.get(session_id).cloned();
        let Some(target) = target else {
            return Ok(None);
        };
        let status = self.stop(&target.child).await?;
        self.forget(session_id).await;
        Ok(status)
    }

    pub(super) async fn stop_session_or_child(
        &self,
        session_id: &str,
        child: &SharedSupervisedChild,
    ) -> anyhow::Result<Option<ExitStatus>> {
        if self.processes.read().await.contains_key(session_id) {
            self.stop_session(session_id).await
        } else {
            self.stop(child).await
        }
    }

    async fn track(&self, target: StopTarget) -> anyhow::Result<()> {
        let session_id = target.session_id.clone();
        let mut processes = self.processes.write().await;
        match processes.entry(session_id.clone()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(target);
                Ok(())
            }
            std::collections::hash_map::Entry::Occupied(_) => {
                anyhow::bail!("duplicate supervised agent session: {session_id}")
            }
        }
    }

    #[cfg(test)]
    pub(super) async fn track_test_process(&self, target: StopTarget) -> anyhow::Result<()> {
        self.track(target).await
    }

    pub(super) async fn forget(&self, session_id: &str) {
        self.processes.write().await.remove(session_id);
    }

    pub(super) async fn stop(
        &self,
        child: &SharedSupervisedChild,
    ) -> anyhow::Result<Option<ExitStatus>> {
        let mut guard = child.lock().await;
        let Some(process) = guard.as_mut() else {
            return Ok(None);
        };

        if let Some(status) = process
            .try_wait()
            .context("failed to poll supervised process before stop")?
        {
            *guard = None;
            return Ok(Some(status));
        }

        // The pipe may already have been handed to an ACP transport. Taking the
        // child-owned copy still helps plain stdin agents observe EOF promptly.
        process.stdin().take();

        #[cfg(unix)]
        {
            if let Err(error) = process.signal(15) {
                tracing::debug!(error = %error, "failed to send SIGTERM to supervised process group");
            }

            match tokio::time::timeout(self.graceful_stop_timeout, process.wait()).await {
                Ok(Ok(status)) => {
                    *guard = None;
                    return Ok(Some(status));
                }
                Ok(Err(error)) => {
                    tracing::warn!(error = %error, "failed to wait for graceful supervised process exit");
                }
                Err(_) => {
                    tracing::warn!(
                        timeout_secs = self.graceful_stop_timeout.as_secs(),
                        "supervised process exceeded graceful stop deadline"
                    );
                }
            }
        }

        process
            .start_kill()
            .context("failed to force-kill supervised process tree")?;
        match tokio::time::timeout(self.force_stop_timeout, process.wait()).await {
            Ok(Ok(status)) => {
                *guard = None;
                return Ok(Some(status));
            }
            Ok(Err(error)) => {
                tracing::warn!(error = %error, "failed to reap force-killed supervised process tree");
            }
            Err(_) => {
                tracing::warn!(
                    timeout_secs = self.force_stop_timeout.as_secs(),
                    "supervised process tree exceeded force-stop deadline"
                );
            }
        }

        // One final kill-and-wait pass is the orphan reaper. A successful
        // process-wrap wait proves the process group/job object is empty.
        process
            .start_kill()
            .context("final orphan reaper failed to kill supervised process tree")?;
        let status = tokio::time::timeout(self.force_stop_timeout, process.wait())
            .await
            .context("final orphan reaper timed out")?
            .context("final orphan reaper failed to wait for process tree")?;
        *guard = None;
        Ok(Some(status))
    }
}

pub(super) fn spawn_supervised(command: Command) -> std::io::Result<SupervisedChild> {
    let mut wrapped = CommandWrap::from(command);
    wrapped.wrap(KillOnDrop);
    #[cfg(unix)]
    wrapped.wrap(ProcessGroup::leader());
    #[cfg(windows)]
    wrapped.wrap(JobObject);
    wrapped.spawn()
}

#[cfg(test)]
mod tests {
    use std::process::Stdio;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;
    use tokio::sync::Mutex;

    use super::{AgentProcessSupervisor, SharedSupervisedChild, StopTarget, spawn_supervised};

    #[tokio::test]
    async fn shutdown_waits_for_active_start_and_rejects_new_starts() {
        let supervisor =
            AgentProcessSupervisor::new(Duration::from_millis(100), Duration::from_millis(100));
        let start_permit = supervisor
            .acquire_start_permit()
            .await
            .expect("acquire start permit");
        let shutdown_supervisor = supervisor.clone();
        let shutdown = tokio::spawn(async move {
            let _guard = shutdown_supervisor.begin_shutdown().await;
        });

        tokio::time::sleep(Duration::from_millis(25)).await;
        assert!(!shutdown.is_finished());
        drop(start_permit);
        shutdown.await.expect("join shutdown gate");

        assert!(supervisor.acquire_start_permit().await.is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stop_all_force_kills_and_reaps_process_groups() {
        let supervisor =
            AgentProcessSupervisor::new(Duration::from_millis(100), Duration::from_secs(2));
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("trap '' TERM; sleep 60 & printf '%s\\n' \"$!\"; wait")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut process = spawn_supervised(command).expect("spawn supervised process group");
        let stdout = process.stdout().take().expect("take child stdout");
        let mut lines = BufReader::new(stdout).lines();
        let descendant_pid = lines
            .next_line()
            .await
            .expect("read descendant pid")
            .expect("descendant pid line");
        let child: SharedSupervisedChild = Arc::new(Mutex::new(Some(process)));

        let started_at = Instant::now();
        supervisor.processes.write().await.insert(
            "session-1".to_string(),
            StopTarget {
                agent_id: "agent-1".to_string(),
                session_id: "session-1".to_string(),
                child,
            },
        );
        supervisor.stop_all().await.expect("stop process group");
        assert!(started_at.elapsed() >= Duration::from_millis(100));

        let status = Command::new("sh")
            .arg("-c")
            .arg("kill -0 \"$1\" 2>/dev/null")
            .arg("agenthub-supervisor-test")
            .arg(descendant_pid)
            .status()
            .await
            .expect("probe descendant process");
        assert!(!status.success(), "descendant process should be reaped");
    }
}
