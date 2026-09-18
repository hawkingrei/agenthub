use std::time::Duration;

use agenthub_config::RaraLaunchConfig;
use agenthub_rara::{Client, Connection, ConnectionError, ConnectionOptions, ConnectionStatus};
use anyhow::Context;
use tokio::io::{AsyncRead, AsyncReadExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use uuid::Uuid;

use super::supervisor::SharedSupervisedChild;
use super::{AgentInput, AgentManager};
use crate::acp::AcpActorSkillContext;
use crate::agent::AgentRecord;

mod events;
mod receipts;
mod session;
pub use session::RaraHandle;

const PROCESS_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(all(test, unix))]
mod tests;

pub(super) struct RaraPipes {
    pub child: SharedSupervisedChild,
    pub stdout: ChildStdout,
    pub stdin: ChildStdin,
}

pub(super) async fn exit_success(success: bool, client: Option<&RaraHandle>) -> bool {
    let Some(client) = client else {
        return success;
    };
    let semantic = tokio::time::timeout(PROCESS_DRAIN_TIMEOUT, client.closed()).await;
    client.abort();
    success && matches!(semantic, Ok(Ok(_)))
}

impl AgentManager {
    pub(super) fn rara_launch_configuration(
        &self,
        agent: &AgentRecord,
        actor_context: Option<&AcpActorSkillContext>,
    ) -> anyhow::Result<Option<RaraLaunchConfig>> {
        // A logical selector keeps custom executable paths in explicit runtime config.
        // Existing ACP and arbitrary command selections never read these overrides.
        if agent.command != "rara" {
            return Ok(None);
        }
        anyhow::ensure!(
            agent.args.is_empty(),
            "direct runtime arguments are managed by the transport"
        );
        anyhow::ensure!(
            agent.target_node_id.is_none(),
            "direct runtime remote placement is unavailable"
        );
        anyhow::ensure!(
            actor_context.is_none(),
            "direct runtime Team binding is unavailable"
        );
        anyhow::ensure!(
            !agent.agent_loop_enabled,
            "direct runtime loop execution is unavailable"
        );
        anyhow::ensure!(
            agent.thinking_level.is_none(),
            "direct runtime thinking configuration is unsupported"
        );
        let mut config = self.loop_app_config.rara_launch_config()?;
        if let Some(model) = &agent.runtime_model {
            config.default_model = Some(model.clone());
        }
        Ok(Some(config))
    }

    pub(super) fn spawn_rara_stderr_drain<R>(
        &self,
        agent_id: String,
        session_id: String,
        mut stderr: R,
    ) -> anyhow::Result<()>
    where
        R: AsyncRead + Unpin + Send + 'static,
    {
        let cancellation = self.daemon_tasks.runtime_cancellation();
        self.daemon_tasks.spawn_runtime_task(
            format!("direct-runtime-stderr:{agent_id}:{session_id}"),
            async move {
                let mut buffer = [0; 4096];
                let mut diagnostic_bytes = 0_u64;
                loop {
                    let read = tokio::select! {
                        _ = cancellation.cancelled() => break,
                        read = stderr.read(&mut buffer) => read,
                    };
                    match read {
                        Ok(0) | Err(_) => break,
                        Ok(count) => diagnostic_bytes = diagnostic_bytes.saturating_add(count as u64),
                    }
                }
                // Native diagnostic bodies can contain credentials and protocol-like text.
                // Only a bounded counter reaches shared diagnostics; stdout owns readiness.
                tracing::debug!(%agent_id, %session_id, diagnostic_bytes, "direct runtime stderr drained");
                Ok(())
            },
        )
    }

    pub(super) async fn connect_rara(
        &self,
        agent_id: &str,
        session_id: &str,
        pipes: RaraPipes,
        config: &RaraLaunchConfig,
        output_tx: tokio::sync::broadcast::Sender<crate::agent::AgentOutput>,
    ) -> anyhow::Result<std::sync::Arc<RaraHandle>> {
        let Connection { client, output } = Connection::open(
            BufReader::new(pipes.stdout),
            pipes.stdin,
            ConnectionOptions {
                startup_timeout: config.startup_timeout,
                shutdown_timeout: config.shutdown_timeout,
                ..ConnectionOptions::default()
            },
        )
        .await?;
        let store = agenthub_db::runtime_events::RuntimeEventStore::bind(
            self.event_dbs.pool_for_agent(agent_id).await?,
            session_id,
            &client.handshake().runtime_id,
        )
        .await?;
        let startup =
            session::RaraHandle::create(self, client.clone(), store.clone(), agent_id, output_tx)
                .await;
        let handle = match startup {
            Ok(handle) => std::sync::Arc::new(handle),
            Err(error) => {
                client.abort();
                store.close(chrono::Utc::now().timestamp()).await?;
                return Err(error);
            }
        };
        let manager = self.clone();
        let agent_id = agent_id.to_owned();
        let session_id = session_id.to_owned();
        let observer = handle.clone();
        let cancellation = self.daemon_tasks.runtime_cancellation();
        self.daemon_tasks.spawn_runtime_task(
            format!("direct-runtime-transport:{agent_id}:{session_id}"),
            async move {
                let consumed = observer.consume(output, cancellation).await;
                if consumed.is_err() {
                    observer.abort();
                }
                let permissions_closed = observer.expire_permissions().await;
                let closed = store.close(chrono::Utc::now().timestamp()).await;
                observer.finish_delivery(
                    consumed.is_ok() && closed.is_ok() && permissions_closed.is_ok(),
                );
                let result = observer.closed().await;
                if consumed.is_err()
                    || closed.is_err()
                    || permissions_closed.is_err()
                    || result.is_err()
                {
                    tracing::warn!(%agent_id, %session_id, "direct runtime delivery failed");
                    // Serialize with replacement launches; clean only this owned child/session.
                    let _configuration = manager
                        .configuration_gate(&agent_id)
                        .await
                        .lock_owned()
                        .await;
                    manager
                        .process_supervisor
                        .stop_session_or_child(&session_id, &pipes.child)
                        .await
                        .context("failed to clean direct runtime after transport loss")?;
                    let current = {
                        let handles = manager.inner.read().await;
                        Self::handle_matches_session(handles.get(&agent_id), &session_id)
                    };
                    if current {
                        Self::finalize_process_exit(
                            &manager.db,
                            &manager.event_dbs,
                            manager.idle_gc.clone(),
                            &manager.inner,
                            &manager.push,
                            &agent_id,
                            &session_id,
                            false,
                        )
                        .await;
                    }
                }
                Ok(())
            },
        )?;
        Ok(handle)
    }

    pub(super) async fn shutdown_rara_transport(
        &self,
        client: &Client,
        child: &SharedSupervisedChild,
    ) {
        let result = match client.status() {
            ConnectionStatus::Running => {
                match client.shutdown(Uuid::now_v7().to_string()).await {
                    // Concurrent stop callers share the already admitted semantic shutdown.
                    Err(ConnectionError::Closing) => client.closed().await,
                    result => result,
                }
            }
            ConnectionStatus::Closing => client.closed().await,
            ConnectionStatus::Closed(result) => result,
        };
        match result {
            Ok(_) => {
                let drain = async {
                    loop {
                        let done = {
                            let mut child = child.lock().await;
                            match child.as_mut() {
                                Some(child) => child
                                    .try_wait()
                                    .map(|status| status.is_some())
                                    .unwrap_or(true),
                                None => true,
                            }
                        };
                        if done {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                };
                let _ = tokio::time::timeout(PROCESS_DRAIN_TIMEOUT, drain).await;
            }
            Err(error) => {
                tracing::warn!(%error, "direct runtime requires supervised shutdown fallback")
            }
        }
        // The caller always follows with the existing supervisor's cleanup proof.
        client.abort();
    }

    pub(super) async fn shutdown_rara_transports(&self) {
        let runtimes = {
            let handles = self.inner.read().await;
            handles
                .values()
                .filter_map(|handle| match &handle.input {
                    AgentInput::Rara(client) => Some((client.clone(), handle.child.clone())),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        futures::future::join_all(
            runtimes
                .iter()
                .map(|(client, child)| self.shutdown_rara_transport(client, child)),
        )
        .await;
    }
}
