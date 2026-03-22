use std::sync::Arc;

use chrono::Utc;
use sqlx::Row;
use tokio::sync::{Mutex, broadcast};
use uuid::Uuid;

use super::acp_provider::AcpDefaultModeBehavior;
use super::executor::LocalExecutionRequest;
use super::start_plan::{AgentStartPlan, build_agent_start_plan};
use super::{
    AGENT_STOP_WAIT_TIMEOUT, AgentHandle, AgentInput, AgentManager, build_runtime_start_policy,
    ensure_team_leader_workdir_exists, normalize_agent_loop_config, spawn_agent_loop_controller,
};
use crate::acp::{
    AcpActorSkillContext, AgenthubAcpEventSink, SpawnAcpSessionRequest, load_safe_paths,
    normalize_actor_context, spawn_acp_session,
};
use crate::agent::event_message_codec::persist_agent_event;
use crate::agent::{AgentStatus, OutputStream};
use agent_client_protocol::Implementation;

impl AgentManager {
    async fn get_persistent_session(
        &self,
        agent_id: &str,
        provider: &str,
    ) -> anyhow::Result<Option<String>> {
        let row = sqlx::query(
            r#"
            SELECT session_id
            FROM agent_persistent_sessions
            WHERE agent_id = ?1 AND provider = ?2
            "#,
        )
        .bind(agent_id)
        .bind(provider)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.map(|row| row.get::<String, _>("session_id")))
    }

    async fn set_persistent_session(
        &self,
        agent_id: &str,
        provider: &str,
        session_id: &str,
    ) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO agent_persistent_sessions (agent_id, provider, session_id, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(agent_id, provider)
            DO UPDATE SET session_id = excluded.session_id, updated_at = excluded.updated_at
            "#,
        )
        .bind(agent_id)
        .bind(provider)
        .bind(session_id)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn clear_persistent_session(
        &self,
        agent_id: &str,
        provider: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            DELETE FROM agent_persistent_sessions
            WHERE agent_id = ?1 AND provider = ?2
            "#,
        )
        .bind(agent_id)
        .bind(provider)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn get_running_session_id(&self, agent_id: &str) -> Option<String> {
        let (child, session_id) = {
            let guard = self.inner.read().await;
            guard
                .get(agent_id)
                .map(|handle| (handle.child.clone(), handle.session_id.clone()))?
        };
        let exit_result = {
            let mut child_guard = child.lock().await;
            let child_ref = child_guard.as_mut()?;
            child_ref.try_wait()
        };

        match exit_result {
            Ok(None) => Some(session_id),
            Ok(Some(status)) => {
                Self::finalize_process_exit(
                    &self.db,
                    &self.event_dbs,
                    self.idle_gc.clone(),
                    &self.inner,
                    &self.push,
                    agent_id,
                    &session_id,
                    status.success(),
                )
                .await;
                None
            }
            Err(err) => {
                tracing::warn!(
                    "start_agent: failed to poll child status: agent_id={}, error={}",
                    agent_id,
                    err
                );
                Self::finalize_process_exit(
                    &self.db,
                    &self.event_dbs,
                    self.idle_gc.clone(),
                    &self.inner,
                    &self.push,
                    agent_id,
                    &session_id,
                    false,
                )
                .await;
                None
            }
        }
    }

    pub async fn running_session_id_for_agent(&self, agent_id: &str) -> Option<String> {
        self.get_running_session_id(agent_id).await
    }

    pub async fn running_actor_context_for_agent(
        &self,
        agent_id: &str,
    ) -> Option<AcpActorSkillContext> {
        let session_id = self.get_running_session_id(agent_id).await?;
        let guard = self.inner.read().await;
        let handle = guard.get(agent_id)?;
        if handle.session_id != session_id {
            return None;
        }
        handle.actor_context.clone()
    }

    async fn reserve_agent_start(&self, agent_id: &str) -> anyhow::Result<()> {
        {
            let guard = self.inner.read().await;
            if guard.contains_key(agent_id) {
                return Err(anyhow::anyhow!("agent already running"));
            }
        }
        let mut starting = self.starting.lock().await;
        if starting.contains(agent_id) {
            return Err(anyhow::anyhow!("agent already running"));
        }
        starting.insert(agent_id.to_string());
        Ok(())
    }

    async fn release_agent_start(&self, agent_id: &str) {
        let mut starting = self.starting.lock().await;
        starting.remove(agent_id);
    }

    #[tracing::instrument(skip(self), fields(agent_id = %agent_id), err)]
    pub async fn start_agent(&self, agent_id: &str) -> anyhow::Result<String> {
        self.start_agent_with_actor_context(agent_id, None).await
    }

    #[tracing::instrument(
        skip(self, actor_context),
        fields(agent_id = %agent_id),
        err
    )]
    pub async fn start_agent_with_actor_context(
        &self,
        agent_id: &str,
        actor_context: Option<AcpActorSkillContext>,
    ) -> anyhow::Result<String> {
        let agent = self.get_agent(agent_id).await?;
        let running_session_id = self.get_running_session_id(agent_id).await;
        let start_plan =
            build_agent_start_plan(agent, actor_context, running_session_id.as_deref())?;
        if let AgentStartPlan::ReuseRunningSession { session_id } = &start_plan {
            return Ok(session_id.clone());
        }
        self.reserve_agent_start(agent_id).await?;
        let result = match start_plan {
            AgentStartPlan::ReuseRunningSession { session_id } => Ok(session_id),
            AgentStartPlan::StartLocal {
                agent,
                actor_context,
            } => self.start_local_agent(agent, actor_context).await,
            AgentStartPlan::StartRemote {
                agent,
                target_node_id,
                actor_context,
            } => {
                self.start_remote_agent(&agent, &target_node_id, actor_context.as_ref())
                    .await
            }
        };
        self.release_agent_start(agent_id).await;
        result
    }

    #[tracing::instrument(
        skip(self, agent, actor_context),
        fields(agent_id = %agent.id),
        err
    )]
    async fn start_local_agent(
        &self,
        agent: crate::agent::AgentRecord,
        actor_context: Option<AcpActorSkillContext>,
    ) -> anyhow::Result<String> {
        let session_id = Uuid::new_v4().to_string();
        let actor_context = actor_context.map(normalize_actor_context).transpose()?;
        let persisted_workdir = super::expand_tilde(&agent.workdir);
        let persisted_worktree_repo = agent.worktree_repo.as_deref().map(super::expand_tilde);
        if (persisted_workdir != agent.workdir
            || persisted_worktree_repo.as_deref() != agent.worktree_repo.as_deref())
            && let Err(err) = sqlx::query(
                r#"
                UPDATE agents
                SET workdir = ?1, worktree_repo = ?2, updated_at = ?3
                WHERE id = ?4
                "#,
            )
            .bind(&persisted_workdir)
            .bind(&persisted_worktree_repo)
            .bind(Utc::now().timestamp())
            .bind(&agent.id)
            .execute(&self.db)
            .await
        {
            tracing::warn!(
                agent_id = %agent.id,
                workdir = %persisted_workdir,
                worktree_repo = ?persisted_worktree_repo,
                error = %err,
                "failed to persist normalized workdir/worktree_repo"
            );
        }
        let start_policy = build_runtime_start_policy(
            &agent,
            actor_context.as_ref(),
            &persisted_workdir,
            persisted_worktree_repo.as_deref(),
            Some(&session_id),
        )?;
        let mut runtime_agent = agent.clone();
        runtime_agent.worktree_mode = start_policy.worktree_mode.clone();
        runtime_agent.worktree_ref = start_policy.worktree_ref.clone();

        if let Err(err) = self
            .prepare_worktree_with_paths(
                &runtime_agent,
                &start_policy.workdir,
                start_policy.worktree_repo.as_deref(),
            )
            .await
        {
            if let Err(record_err) = self
                .record_failed_session(&agent.id, &session_id, &err.to_string())
                .await
            {
                tracing::error!(
                    agent_id = %agent.id,
                    session_id = %session_id,
                    error = %record_err,
                    "failed to record startup failure session"
                );
            }
            if let Err(status_err) = self
                .update_agent_status(&agent.id, AgentStatus::Failed)
                .await
            {
                tracing::error!(
                    agent_id = %agent.id,
                    session_id = %session_id,
                    error = %status_err,
                    "failed to update agent status after startup failure"
                );
            }
            return Err(err);
        }
        if let Err(err) = self.ensure_safe_path(&start_policy.workdir).await {
            if let Err(record_err) = self
                .record_failed_session(&agent.id, &session_id, &err.to_string())
                .await
            {
                tracing::error!(
                    agent_id = %agent.id,
                    session_id = %session_id,
                    error = %record_err,
                    "failed to record safe-path startup failure"
                );
            }
            if let Err(status_err) = self
                .update_agent_status(&agent.id, AgentStatus::Failed)
                .await
            {
                tracing::error!(
                    agent_id = %agent.id,
                    session_id = %session_id,
                    error = %status_err,
                    "failed to update agent status after safe-path failure"
                );
            }
            return Err(err);
        }
        if let Err(err) =
            ensure_team_leader_workdir_exists(actor_context.as_ref(), &start_policy.workdir)
        {
            let message = err.to_string();
            let _ = self
                .record_failed_session(&agent.id, &session_id, &message)
                .await;
            let _ = self
                .update_agent_status(&agent.id, AgentStatus::Failed)
                .await;
            return Err(anyhow::anyhow!(message));
        }
        if let Some(worker_branch) = start_policy.worker_branch.as_deref()
            && let Err(err) = self
                .checkout_team_worker_branch(&start_policy.workdir, worker_branch)
                .await
        {
            if let Err(record_err) = self
                .record_failed_session(&agent.id, &session_id, &err.to_string())
                .await
            {
                tracing::error!(
                    agent_id = %agent.id,
                    session_id = %session_id,
                    error = %record_err,
                    "failed to record worker-branch startup failure"
                );
            }
            if let Err(status_err) = self
                .update_agent_status(&agent.id, AgentStatus::Failed)
                .await
            {
                tracing::error!(
                    agent_id = %agent.id,
                    session_id = %session_id,
                    error = %status_err,
                    "failed to update agent status after worker-branch failure"
                );
            }
            return Err(err);
        }

        let acp_provider = self.acp_provider_spec_for_agent(&agent.command, &agent.args);
        let is_acp = acp_provider.is_some();
        let command_path = self.resolve_command_path(&agent.command, acp_provider);
        let local_execution_request = LocalExecutionRequest {
            command_path: command_path.clone(),
            args: agent.args.clone(),
            workdir: start_policy.workdir.clone(),
            actor_context: actor_context.clone(),
        };
        let local_execution = match self
            .local_executor
            .spawn_process(local_execution_request.clone())
            .await
        {
            Ok(execution) => execution,
            Err(err) => {
                if let Err(record_err) = self
                    .record_failed_session(&agent.id, &session_id, &err.to_string())
                    .await
                {
                    tracing::error!(
                        agent_id = %agent.id,
                        session_id = %session_id,
                        error = %record_err,
                        "failed to record spawn failure session"
                    );
                }
                if let Err(status_err) = self
                    .update_agent_status(&agent.id, AgentStatus::Failed)
                    .await
                {
                    tracing::error!(
                        agent_id = %agent.id,
                        session_id = %session_id,
                        error = %status_err,
                        "failed to update agent status after spawn failure"
                    );
                }
                tracing::error!(
                    "spawn failed: command={} workdir={} args={:?} error={}",
                    local_execution_request.command_path,
                    local_execution_request.workdir,
                    local_execution_request.args,
                    err
                );
                return Err(anyhow::anyhow!(
                    "spawn failed: command={} workdir={} args={:?} error={}",
                    local_execution_request.command_path,
                    local_execution_request.workdir,
                    local_execution_request.args,
                    err
                ));
            }
        };
        let mut child = local_execution.child;
        let mut stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdin = child.stdin.take();

        let (output_tx, _rx) = broadcast::channel(256);
        let child = Arc::new(Mutex::new(Some(child)));
        let stdin = Arc::new(Mutex::new(stdin));

        let now = Utc::now().timestamp();
        if let Err(err) = sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(&session_id)
        .bind(&agent.id)
        .bind("running")
        .bind(now)
        .execute(&self.db)
        .await
        {
            tracing::error!(
                agent_id = %agent.id,
                session_id = %session_id,
                error = %err,
                "failed to insert running agent session"
            );
            if let Err(record_err) = self
                .record_failed_session(&agent.id, &session_id, "session insert failed")
                .await
            {
                tracing::error!(
                    agent_id = %agent.id,
                    session_id = %session_id,
                    error = %record_err,
                    "failed to record session-insert startup failure"
                );
            }
            if let Err(status_err) = self
                .update_agent_status(&agent.id, AgentStatus::Failed)
                .await
            {
                tracing::error!(
                    agent_id = %agent.id,
                    session_id = %session_id,
                    error = %status_err,
                    "failed to update agent status after session-insert failure"
                );
            }
            return Err(err.into());
        }

        self.update_agent_status(&agent.id, AgentStatus::Running)
            .await?;

        let mut loop_controller = None;
        let input = if let Some(provider) = acp_provider {
            let resume_session_id = self.get_persistent_session(&agent.id, provider.id).await?;
            let stdout = match stdout.take() {
                Some(stdout) => stdout,
                None => {
                    if let Err(record_err) = self
                        .record_failed_session(&agent.id, &session_id, "acp stdout missing")
                        .await
                    {
                        tracing::error!(
                            agent_id = %agent.id,
                            session_id = %session_id,
                            error = %record_err,
                            "failed to record missing acp stdout failure"
                        );
                    }
                    if let Err(status_err) = self
                        .update_agent_status(&agent.id, AgentStatus::Failed)
                        .await
                    {
                        tracing::error!(
                            agent_id = %agent.id,
                            session_id = %session_id,
                            error = %status_err,
                            "failed to update agent status after missing acp stdout"
                        );
                    }
                    return Err(anyhow::anyhow!("acp stdout missing"));
                }
            };
            let stdin = match stdin.lock().await.take() {
                Some(stdin) => stdin,
                None => {
                    if let Err(record_err) = self
                        .record_failed_session(&agent.id, &session_id, "acp stdin missing")
                        .await
                    {
                        tracing::error!(
                            agent_id = %agent.id,
                            session_id = %session_id,
                            error = %record_err,
                            "failed to record missing acp stdin failure"
                        );
                    }
                    if let Err(status_err) = self
                        .update_agent_status(&agent.id, AgentStatus::Failed)
                        .await
                    {
                        tracing::error!(
                            agent_id = %agent.id,
                            session_id = %session_id,
                            error = %status_err,
                            "failed to update agent status after missing acp stdin"
                        );
                    }
                    return Err(anyhow::anyhow!("acp stdin missing"));
                }
            };
            let safe_paths = match load_safe_paths(&self.db).await {
                Ok(paths) => paths,
                Err(err) => {
                    tracing::warn!("safe paths load failed: {err}");
                    Vec::new()
                }
            };
            let event_sink = Arc::new(AgenthubAcpEventSink::new(
                self.event_dbs.clone(),
                self.idle_gc.clone(),
                output_tx.clone(),
                agent.id.clone(),
                session_id.clone(),
            ));
            let client_info = Implementation::new("agenthub", env!("CARGO_PKG_VERSION"));
            let permission_review_dispatcher = self
                .permission_review_dispatcher
                .read()
                .ok()
                .and_then(|guard| guard.clone());
            let handle = match spawn_acp_session(SpawnAcpSessionRequest {
                event_sink,
                permissions: self.permissions.clone(),
                permission_review_dispatcher,
                agent_id: agent.id.clone(),
                agent_session_id: session_id.clone(),
                resume_session_id,
                workdir: start_policy.workdir.clone(),
                client_info,
                stdout,
                stdin,
                safe_paths,
                actor_context: actor_context.clone(),
                prompt_delivery_policy: provider.prompt_delivery_policy,
                runtime_location: local_execution.runtime_location,
            })
            .await
            {
                Ok(handle) => handle,
                Err(err) => {
                    if let Err(record_err) = self
                        .record_failed_session(&agent.id, &session_id, &err.to_string())
                        .await
                    {
                        tracing::error!(
                            agent_id = %agent.id,
                            session_id = %session_id,
                            error = %record_err,
                            "failed to record acp session spawn failure"
                        );
                    }
                    if let Err(status_err) = self
                        .update_agent_status(&agent.id, AgentStatus::Failed)
                        .await
                    {
                        tracing::error!(
                            agent_id = %agent.id,
                            session_id = %session_id,
                            error = %status_err,
                            "failed to update agent status after acp session spawn failure"
                        );
                    }
                    return Err(err);
                }
            };
            if let Err(err) = self
                .set_persistent_session(&agent.id, provider.id, &handle.session_id)
                .await
            {
                tracing::error!("persist acp session failed: {}", err);
            }
            if provider.uses_default_mode_config() {
                if let Some(mode_id) = self.acp_default_mode.as_deref()
                    && let Err(err) = handle.set_mode(mode_id.to_string()).await
                {
                    tracing::warn!(
                        "set acp default mode failed: agent_id={}, mode_id={}, error={}",
                        agent.id,
                        mode_id,
                        err
                    );
                }
            } else if matches!(
                provider.default_mode_behavior,
                AcpDefaultModeBehavior::IgnoreConfigured
            ) && self.acp_default_mode.is_some()
            {
                tracing::debug!(
                    "acp default mode ignored for provider {} (agent_id={})",
                    provider.id,
                    agent.id
                );
            }
            if let Some(config) = normalize_agent_loop_config(
                agent.agent_loop_enabled,
                agent.agent_loop_idle_seconds,
                agent.agent_loop_prompt.as_deref(),
            ) {
                loop_controller = Some(spawn_agent_loop_controller(
                    self.event_dbs.clone(),
                    self.idle_gc.clone(),
                    output_tx.clone(),
                    handle.clone(),
                    agent.id.clone(),
                    session_id.clone(),
                    config,
                ));
            }
            AgentInput::Acp(handle.clone())
        } else {
            AgentInput::Stdin(stdin.clone())
        };

        let handle = AgentHandle {
            child: child.clone(),
            output_tx: output_tx.clone(),
            input,
            session_id: session_id.clone(),
            actor_context,
            loop_controller,
        };

        {
            let mut guard = self.inner.write().await;
            guard.insert(agent.id.clone(), handle);
        }

        if !is_acp && let Some(stdout) = stdout {
            self.spawn_output_reader(
                agent.id.clone(),
                session_id.clone(),
                OutputStream::Stdout,
                stdout,
                output_tx.clone(),
                false,
            )
            .await;
        }

        if let Some(stderr) = stderr {
            self.spawn_output_reader(
                agent.id.clone(),
                session_id.clone(),
                OutputStream::Stderr,
                stderr,
                output_tx.clone(),
                is_acp,
            )
            .await;
        }

        self.spawn_exit_watcher(agent.id.clone(), session_id.clone())
            .await;

        self.emit_run_status(
            output_tx.clone(),
            agent.id.clone(),
            session_id.clone(),
            "running",
        )
        .await;

        Ok(session_id)
    }

    #[tracing::instrument(skip(self), fields(agent_id = %agent_id), err)]
    pub async fn stop_agent(&self, agent_id: &str) -> anyhow::Result<()> {
        let agent = self.get_agent(agent_id).await?;
        if let Some(target_node_id) = agent.target_node_id.as_deref() {
            let client = self
                .remote_control_client_for_target_node(target_node_id)
                .await?;
            client.stop_managed_agent(agent_id).await?;
            self.update_agent_status(agent_id, AgentStatus::Stopped)
                .await?;
            return Ok(());
        }
        let handle = {
            let mut guard = self.inner.write().await;
            guard.remove(agent_id)
        };
        if let Some(handle) = handle {
            let session_id = handle.session_id.clone();
            let now = Utc::now().timestamp();
            if let Err(err) = sqlx::query(
                r#"
                UPDATE agent_sessions
                SET status = 'cancelled', ended_at = ?1
                WHERE id = ?2
                "#,
            )
            .bind(now)
            .bind(&session_id)
            .execute(&self.db)
            .await
            {
                tracing::warn!(
                    agent_id = %agent_id,
                    session_id = %session_id,
                    error = %err,
                    "failed to mark agent session as cancelled during stop"
                );
            }
            if let Err(err) = self
                .update_agent_status(agent_id, AgentStatus::Stopped)
                .await
            {
                tracing::warn!(
                    agent_id = %agent_id,
                    session_id = %session_id,
                    error = %err,
                    "failed to update agent status during stop"
                );
            }
            self.emit_run_status(
                handle.output_tx.clone(),
                agent_id.to_string(),
                session_id,
                "cancelled",
            )
            .await;
            if let Some(idle_gc) = &self.idle_gc {
                idle_gc.remove_agent(agent_id).await;
            }
            let mut child_guard = handle.child.lock().await;
            if let Some(mut child) = child_guard.take() {
                if let Err(err) = child.kill().await {
                    tracing::warn!(
                        agent_id = %agent_id,
                        error = %err,
                        "failed to kill agent child process during stop"
                    );
                }
                match tokio::time::timeout(AGENT_STOP_WAIT_TIMEOUT, child.wait()).await {
                    Ok(Ok(_)) => {}
                    Ok(Err(err)) => {
                        tracing::warn!(
                            agent_id = %agent_id,
                            error = %err,
                            "failed to wait for agent child process during stop"
                        );
                    }
                    Err(_) => {
                        tracing::warn!(
                            agent_id = %agent_id,
                            timeout_secs = AGENT_STOP_WAIT_TIMEOUT.as_secs(),
                            "timed out waiting for agent child process during stop"
                        );
                    }
                }
            }
        }
        Ok(())
    }

    async fn record_failed_session(
        &self,
        agent_id: &str,
        session_id: &str,
        message: &str,
    ) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        let seq = Uuid::now_v7().to_string();
        if let Err(err) = sqlx::query(
            r#"
            INSERT OR IGNORE INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'failed', ?3, ?4)
            "#,
        )
        .bind(session_id)
        .bind(agent_id)
        .bind(now)
        .bind(now)
        .execute(&self.db)
        .await
        {
            tracing::warn!(
                agent_id = %agent_id,
                session_id = %session_id,
                error = %err,
                "failed to persist failed agent session row"
            );
        }

        let failure_message = format!("start failed: {}", message);
        if let Err(err) = persist_agent_event(
            &self.event_dbs,
            None,
            agent_id,
            session_id,
            &seq,
            now,
            &OutputStream::System,
            failure_message.as_str(),
        )
        .await
        {
            tracing::warn!(
                agent_id = %agent_id,
                session_id = %session_id,
                error = %err,
                "failed to persist startup failure event"
            );
        }

        Ok(())
    }
}
