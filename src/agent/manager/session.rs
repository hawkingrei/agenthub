use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use chrono::Utc;
use sqlx::Row;
use tokio::sync::{Mutex, broadcast};
use uuid::Uuid;

use super::acp_provider::{
    ACP_PROVIDER_CODEX, AcpDefaultModeBehavior, AcpProviderSpec,
    codex_reasoning_effort_for_thinking_level, default_env_for_acp_provider,
};
use super::executor::{LocalExecutionRequest, SpawnedLocalProcess};
use super::start_plan::{AgentStartPlan, build_agent_start_plan};
use super::supervisor::SharedSupervisedChild;
use super::{
    AgentHandle, AgentInput, AgentManager, build_runtime_start_policy,
    ensure_team_runtime_workspace_layout, normalize_agent_loop_config, spawn_agent_loop_controller,
};
use crate::acp::{
    AcpActorSkillContext, AgenthubAcpEventSink, SpawnAcpSessionRequest, normalize_actor_context,
    spawn_acp_session,
};
use crate::agent::event_message_codec::persist_agent_event;
use crate::agent::{AgentStatus, OutputStream};
use agent_client_protocol::schema::v1::Implementation;

const RESUMED_ACP_SESSION_GRACE_PERIOD: Duration = Duration::from_secs(2);
const RESUMED_ACP_SESSION_POLL_INTERVAL: Duration = Duration::from_millis(100);
const RESUMED_ACP_SESSION_FAILURES_BEFORE_FRESH_RETRY: i64 = 3;
const TEAM_CODEX_ACP_DEFAULT_MODE: &str = "full-access";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResumedSessionStartupState {
    Running,
    Exited { success: bool },
}

fn should_retry_resumed_acp_session(
    resumed_session_id: Option<&str>,
    actual_session_id: &str,
    startup_state: ResumedSessionStartupState,
) -> bool {
    matches!(
        startup_state,
        ResumedSessionStartupState::Exited { success: false }
    ) && resumed_session_id == Some(actual_session_id)
}

fn should_force_fresh_session_after_resume_failures(failure_count: i64) -> bool {
    failure_count >= RESUMED_ACP_SESSION_FAILURES_BEFORE_FRESH_RETRY
}

pub(super) fn effective_acp_default_mode<'a>(
    provider: AcpProviderSpec,
    agent_mode: Option<&'a str>,
    configured_mode: Option<&'a str>,
    has_actor_context: bool,
) -> Option<&'a str> {
    if provider.id == ACP_PROVIDER_CODEX && has_actor_context {
        if let Some(agent_mode) = agent_mode {
            return Some(agent_mode);
        }
        return Some(TEAM_CODEX_ACP_DEFAULT_MODE);
    }
    if provider.id == ACP_PROVIDER_CODEX && agent_mode.is_some() {
        return agent_mode;
    }
    configured_mode
}

async fn observe_resumed_session_startup(
    child: &SharedSupervisedChild,
    grace_period: Duration,
    poll_interval: Duration,
) -> ResumedSessionStartupState {
    let deadline = tokio::time::Instant::now() + grace_period;
    loop {
        let poll_result = {
            let mut child_guard = child.lock().await;
            match child_guard.as_mut() {
                Some(child) => child.try_wait(),
                None => return ResumedSessionStartupState::Exited { success: false },
            }
        };

        match poll_result {
            Ok(Some(status)) => {
                return ResumedSessionStartupState::Exited {
                    success: status.success(),
                };
            }
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "failed to poll resumed acp session child during startup grace window"
                );
                return ResumedSessionStartupState::Exited { success: false };
            }
        }

        if tokio::time::Instant::now() >= deadline {
            return ResumedSessionStartupState::Running;
        }

        tokio::time::sleep(poll_interval).await;
    }
}

impl AgentManager {
    async fn get_persistent_session(
        &self,
        agent_id: &str,
        provider: &str,
    ) -> anyhow::Result<Option<String>> {
        // This is provider-managed continuity state (for example a persisted Codex thread/session),
        // not the per-launch AgentHub runtime session recorded in `agent_sessions.id`.
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
        // Persist the provider continuity session separately so ordinary restarts can resume ACP
        // memory without reusing the local runtime launch id.
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
        if !self.has_agent_persistent_session_failures_table().await? {
            return Ok(());
        }
        sqlx::query(
            r#"
            DELETE FROM agent_persistent_session_failures
            WHERE agent_id = ?1 AND provider = ?2
            "#,
        )
        .bind(agent_id)
        .bind(provider)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn clear_persistent_session(
        &self,
        agent_id: &str,
        provider: &str,
    ) -> anyhow::Result<()> {
        // This intentionally drops provider continuity only; it does not rewrite historical
        // `agent_sessions` launch records.
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
        if !self.has_agent_persistent_session_failures_table().await? {
            return Ok(());
        }
        sqlx::query(
            r#"
            DELETE FROM agent_persistent_session_failures
            WHERE agent_id = ?1 AND provider = ?2
            "#,
        )
        .bind(agent_id)
        .bind(provider)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn increment_persistent_session_failure(
        &self,
        agent_id: &str,
        provider: &str,
    ) -> anyhow::Result<i64> {
        if !self.has_agent_persistent_session_failures_table().await? {
            anyhow::bail!(
                "agent_persistent_session_failures table is unavailable; cannot track resume failures for fresh-session fallback"
            );
        }
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO agent_persistent_session_failures (agent_id, provider, failure_count, updated_at)
            VALUES (?1, ?2, 1, ?3)
            ON CONFLICT(agent_id, provider)
            DO UPDATE SET
                failure_count = agent_persistent_session_failures.failure_count + 1,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(agent_id)
        .bind(provider)
        .bind(now)
        .execute(&self.db)
        .await?;
        let row = sqlx::query(
            r#"
            SELECT failure_count
            FROM agent_persistent_session_failures
            WHERE agent_id = ?1 AND provider = ?2
            "#,
        )
        .bind(agent_id)
        .bind(provider)
        .fetch_one(&self.db)
        .await?;
        Ok(row.get::<i64, _>("failure_count"))
    }

    pub async fn live_session_id_for_agent(
        &self,
        agent_id: &str,
    ) -> anyhow::Result<Option<String>> {
        // `agent_sessions.id` is the latest AgentHub runtime launch id with an open lifetime.
        let row = sqlx::query(
            r#"
            SELECT id
            FROM agent_sessions
            WHERE agent_id = ?1
              AND ended_at IS NULL
            ORDER BY started_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(agent_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.map(|row| row.get::<String, _>("id")))
    }

    async fn get_running_session_id(&self, agent_id: &str) -> Option<String> {
        // Running-session lookups return the active AgentHub launch id only. Provider continuity
        // lives in `agent_persistent_sessions` and may survive across multiple launch ids.
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
        match build_agent_start_plan(agent, actor_context, running_session_id.as_deref())? {
            AgentStartPlan::ReuseRunningSession { session_id } => Ok(session_id),
            AgentStartPlan::StartLocal {
                agent,
                actor_context,
            } => {
                self.reserve_agent_start(agent_id).await?;
                let result = async {
                    let admission = self.start_scheduler.acquire(agent_id).await?;
                    let session_tracker = Arc::new(Mutex::new(None));
                    match tokio::time::timeout(
                        admission.start_timeout(),
                        async {
                            let _start_permit =
                                self.process_supervisor.acquire_start_permit().await?;
                            self.start_local_agent(
                                agent,
                                actor_context,
                                session_tracker.clone(),
                            )
                            .await
                        },
                    )
                    .await
                    {
                        Ok(result) => result,
                        Err(_) => {
                            self.cleanup_timed_out_start(agent_id, &session_tracker)
                                .await
                                .with_context(|| {
                                    format!(
                                        "agent start timed out and cleanup failed: agent_id={agent_id}"
                                    )
                                })?;
                            anyhow::bail!(
                                "agent start timed out: agent_id={agent_id} timeout_ms={}",
                                admission.start_timeout().as_millis()
                            );
                        }
                    }
                }
                .await;
                self.release_agent_start(agent_id).await;
                result
            }
            AgentStartPlan::StartRemote {
                agent,
                target_node_id,
                actor_context,
            } => {
                self.reserve_agent_start(agent_id).await?;
                let result = self
                    .start_remote_agent(&agent, &target_node_id, actor_context.as_ref())
                    .await;
                self.release_agent_start(agent_id).await;
                result
            }
        }
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
        session_tracker: Arc<Mutex<Option<String>>>,
    ) -> anyhow::Result<String> {
        self.start_local_agent_with_resume_fallback(agent, actor_context, true, session_tracker)
            .await
    }

    async fn start_local_agent_with_resume_fallback(
        &self,
        agent: crate::agent::AgentRecord,
        actor_context: Option<AcpActorSkillContext>,
        allow_resume_retry: bool,
        session_tracker: Arc<Mutex<Option<String>>>,
    ) -> anyhow::Result<String> {
        // This UUID tracks the current AgentHub runtime launch only. ACP/Codex continuity
        // can still resume a previously persisted provider session later in this method.
        let session_id = Uuid::new_v4().to_string();
        *session_tracker.lock().await = Some(session_id.clone());
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
        if let Err(err) =
            ensure_team_runtime_workspace_layout(actor_context.as_ref(), &start_policy.workdir)
                .await
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
        let (command_path, command_args) =
            self.resolve_launch_command(&agent.command, &agent.args, acp_provider);
        let spawn_summary = format!(
            "command={} workdir={} args={:?}",
            command_path, start_policy.workdir, command_args
        );
        let local_execution_request = LocalExecutionRequest {
            agent_id: agent.id.clone(),
            session_id: session_id.clone(),
            command_path: command_path.clone(),
            args: command_args,
            workdir: start_policy.workdir.clone(),
            actor_context: actor_context.clone(),
            extra_env: default_env_for_acp_provider(
                acp_provider,
                self.codex_acp_multi_agent_enabled,
                agent.runtime_model.as_deref(),
                agent.thinking_level.as_deref(),
            ),
        };
        let local_execution = match self
            .local_executor
            .spawn_process(local_execution_request)
            .await
        {
            Ok(execution) => execution,
            Err(err) => {
                let backoff = self.start_scheduler.record_spawn_failure(&agent.id).await;
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
                tracing::error!("spawn failed: {} error={}", spawn_summary, err);
                return Err(anyhow::anyhow!(
                    "spawn failed: {} error={} retry_after_ms={}",
                    spawn_summary,
                    err,
                    backoff.as_millis()
                ));
            }
        };
        self.start_scheduler.clear_spawn_failures(&agent.id).await;
        let SpawnedLocalProcess {
            child,
            registration,
            runtime_location,
        } = local_execution;
        let (mut stdout, stderr, stdin) = {
            let mut child_guard = child.lock().await;
            let process = child_guard
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("spawned agent process is missing"))?;
            (
                process.stdout().take(),
                process.stderr().take(),
                process.stdin().take(),
            )
        };

        let (output_tx, _rx) = broadcast::channel(256);
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
            if let Err(stop_err) = self.process_supervisor.stop_session(&session_id).await {
                tracing::error!(
                    agent_id = %agent.id,
                    session_id = %session_id,
                    error = %stop_err,
                    "failed to stop agent process after session-insert failure"
                );
            }
            return Err(err.into());
        }

        if let Err(err) = self
            .update_agent_status(&agent.id, AgentStatus::Running)
            .await
        {
            if let Err(stop_err) = self.process_supervisor.stop_session(&session_id).await {
                tracing::error!(
                    agent_id = %agent.id,
                    session_id = %session_id,
                    error = %stop_err,
                    "failed to stop agent process after running-status update failure"
                );
            }
            return Err(err);
        }

        let mut loop_controller = None;
        let mut resumed_provider_id = None::<String>;
        let mut resumed_persistent_session_id = None::<String>;
        let mut active_acp_session_id = None::<String>;
        let mut acp_prompt_delivery_policy = None;
        let input = if let Some(provider) = acp_provider {
            acp_prompt_delivery_policy = Some(provider.prompt_delivery_policy);
            // Provider continuity is stored separately from the AgentHub runtime launch id
            // above so restarts can keep ACP memory while still recording a new local start.
            let resume_session_id = match self.get_persistent_session(&agent.id, provider.id).await
            {
                Ok(session_id) => session_id,
                Err(err) => {
                    let _ = self.process_supervisor.stop_session(&session_id).await;
                    return Err(err);
                }
            };
            resumed_provider_id = Some(provider.id.to_string());
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
                    let _ = self.process_supervisor.stop_session(&session_id).await;
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
                    let _ = self.process_supervisor.stop_session(&session_id).await;
                    return Err(anyhow::anyhow!("acp stdin missing"));
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
                self_reminders_enabled: self.internal_peer_client.is_some(),
                provider_id: provider.id.to_string(),
                event_sink,
                permissions: self.permissions.clone(),
                permission_review_dispatcher,
                agent_id: agent.id.clone(),
                agent_session_id: session_id.clone(),
                resume_session_id: resume_session_id.clone(),
                workdir: start_policy.workdir.clone(),
                client_info,
                stdout,
                stdin,
                actor_context: actor_context.clone(),
                prompt_delivery_policy: provider.prompt_delivery_policy,
                runtime_location,
            })
            .await
            {
                Ok(handle) => handle,
                Err(err) => {
                    let err_message = err.to_string();
                    if let Err(record_err) = self
                        .record_failed_session(&agent.id, &session_id, &err_message)
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
                    let _ = self.process_supervisor.stop_session(&session_id).await;
                    return Err(err);
                }
            };
            active_acp_session_id = Some(handle.session_id.clone());
            resumed_persistent_session_id =
                resume_session_id.filter(|resume_id| resume_id == &handle.session_id);
            if let Err(err) = self
                .set_persistent_session(&agent.id, provider.id, &handle.session_id)
                .await
            {
                tracing::error!("persist acp session failed: {}", err);
            }
            let default_mode = effective_acp_default_mode(
                provider,
                agent.codex_acp_default_mode.as_deref(),
                self.acp_default_mode.as_deref(),
                actor_context.is_some(),
            );
            if provider.uses_default_mode_config() {
                if let Some(mode_id) = default_mode
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
            ) && default_mode.is_some()
            {
                tracing::debug!(
                    "acp default mode ignored for provider {} (agent_id={})",
                    provider.id,
                    agent.id
                );
            }
            // Apply the agent runtime profile (model + thinking level) for providers that accept it as
            // ACP session config — currently Codex. Unset fields are skipped so the provider default
            // stays authoritative; failures are logged but never abort the launch. Claude takes the
            // profile as spawn env instead (handled where the launch environment is built).
            if provider.applies_runtime_profile_via_session_config() {
                if let Some(model) = agent.runtime_model.as_deref()
                    && let Err(err) = handle.set_model(model.to_string()).await
                {
                    tracing::warn!(
                        "set acp runtime model failed: agent_id={}, model={}, error={}",
                        agent.id,
                        model,
                        err
                    );
                }
                if let Some(level) = agent.thinking_level.as_deref() {
                    match codex_reasoning_effort_for_thinking_level(level) {
                        Some(effort) => {
                            if let Err(err) = handle
                                .set_config("reasoning_effort".to_string(), effort.to_string())
                                .await
                            {
                                tracing::warn!(
                                    "set acp reasoning effort failed: agent_id={}, level={}, error={}",
                                    agent.id,
                                    level,
                                    err
                                );
                            }
                        }
                        None => {
                            // An unmapped level means manual or stale data; surface it rather than
                            // silently using the default.
                            tracing::warn!(
                                "unmapped thinking level for codex reasoning effort, leaving provider default: agent_id={}, level={}",
                                agent.id,
                                level
                            );
                        }
                    }
                }
            }
            if let Some(config) = normalize_agent_loop_config(
                agent.agent_loop_enabled,
                agent.agent_loop_idle_seconds,
                agent.agent_loop_prompt.as_deref(),
            ) {
                loop_controller = Some(spawn_agent_loop_controller(
                    &self.daemon_tasks,
                    self.event_dbs.clone(),
                    self.idle_gc.clone(),
                    output_tx.clone(),
                    handle.clone(),
                    agent.id.clone(),
                    session_id.clone(),
                    config,
                )?);
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
            actor_context: actor_context.clone(),
            acp_prompt_delivery_policy,
            loop_controller,
        };

        {
            let mut guard = self.inner.write().await;
            guard.insert(agent.id.clone(), handle);
        }
        registration.commit();

        if !is_acp && let Some(stdout) = stdout {
            self.spawn_output_reader(
                agent.id.clone(),
                session_id.clone(),
                OutputStream::Stdout,
                stdout,
                output_tx.clone(),
                false,
            )
            .await?;
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
            .await?;
        }

        self.spawn_exit_watcher(agent.id.clone(), session_id.clone())
            .await?;

        self.emit_run_status(
            output_tx.clone(),
            agent.id.clone(),
            session_id.clone(),
            "running",
        )
        .await;

        if allow_resume_retry
            && let (Some(provider_id), Some(resume_id), Some(acp_session_id)) = (
                resumed_provider_id.as_deref(),
                resumed_persistent_session_id.as_deref(),
                active_acp_session_id.as_deref(),
            )
        {
            let startup_state = observe_resumed_session_startup(
                &child,
                RESUMED_ACP_SESSION_GRACE_PERIOD,
                RESUMED_ACP_SESSION_POLL_INTERVAL,
            )
            .await;
            if should_retry_resumed_acp_session(Some(resume_id), acp_session_id, startup_state) {
                let failure_count = match self
                    .increment_persistent_session_failure(&agent.id, provider_id)
                    .await
                {
                    Ok(failure_count) => failure_count,
                    Err(err) => {
                        let fallback_failure_count =
                            RESUMED_ACP_SESSION_FAILURES_BEFORE_FRESH_RETRY;
                        tracing::warn!(
                            agent_id = %agent.id,
                            provider = %provider_id,
                            resumed_session_id = %resume_id,
                            error = %err,
                            fallback_failure_count,
                            "failed to persist resumed acp session failure count; using fallback"
                        );
                        fallback_failure_count
                    }
                };
                tracing::warn!(
                    agent_id = %agent.id,
                    session_id = %session_id,
                    acp_session_id = %acp_session_id,
                    resumed_session_id = %resume_id,
                    failure_count,
                    "resumed acp session exited during startup"
                );
                if !should_force_fresh_session_after_resume_failures(failure_count) {
                    return Err(anyhow::anyhow!(
                        "resumed acp session exited during startup (attempt {}/{})",
                        failure_count,
                        RESUMED_ACP_SESSION_FAILURES_BEFORE_FRESH_RETRY
                    ));
                }
                tracing::warn!(
                    agent_id = %agent.id,
                    session_id = %session_id,
                    acp_session_id = %acp_session_id,
                    resumed_session_id = %resume_id,
                    failure_count,
                    "resumed acp session exited during startup repeatedly; clearing persistent session and retrying with a new session"
                );
                Self::finalize_process_exit(
                    &self.db,
                    &self.event_dbs,
                    self.idle_gc.clone(),
                    &self.inner,
                    &self.push,
                    &agent.id,
                    &session_id,
                    false,
                )
                .await;
                if let Err(err) = self.clear_persistent_session(&agent.id, provider_id).await {
                    tracing::warn!(
                        agent_id = %agent.id,
                        provider = %provider_id,
                        acp_session_id = %resume_id,
                        error = %err,
                        "failed to clear dirty persistent acp session before retry"
                    );
                }
                return Box::pin(self.start_local_agent_with_resume_fallback(
                    agent,
                    actor_context,
                    false,
                    session_tracker,
                ))
                .await;
            }
        }

        Ok(session_id)
    }

    async fn cleanup_timed_out_start(
        &self,
        agent_id: &str,
        session_tracker: &Arc<Mutex<Option<String>>>,
    ) -> anyhow::Result<()> {
        let session_id = session_tracker.lock().await.clone();
        if let Some(session_id) = session_id.as_deref() {
            self.process_supervisor
                .stop_session(session_id)
                .await
                .with_context(|| {
                    format!(
                        "failed to stop timed-out agent process: agent_id={agent_id} session_id={session_id}"
                    )
                })?;
            let removed = {
                let mut guard = self.inner.write().await;
                if Self::handle_matches_session(guard.get(agent_id), session_id) {
                    guard.remove(agent_id)
                } else {
                    None
                }
            };
            if let Some(handle) = removed
                && let Some(controller) = handle.loop_controller
            {
                controller.stop();
            }
            self.record_failed_session(agent_id, session_id, "agent start timed out")
                .await
                .with_context(|| {
                    format!(
                        "failed to record timed-out agent start: agent_id={agent_id} session_id={session_id}"
                    )
                })?;
        }
        self.update_agent_status(agent_id, AgentStatus::Failed)
            .await
            .with_context(|| {
                format!("failed to update agent status after start timeout: agent_id={agent_id}")
            })?;
        if let Some(idle_gc) = &self.idle_gc {
            idle_gc.remove_agent(agent_id).await;
        }
        Ok(())
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
        let process = {
            let guard = self.inner.read().await;
            guard.get(agent_id).map(|handle| {
                (
                    handle.child.clone(),
                    handle.output_tx.clone(),
                    handle.session_id.clone(),
                )
            })
        };
        if let Some((child, output_tx, session_id)) = process {
            self.process_supervisor
                .stop_session_or_child(&session_id, &child)
                .await
                .with_context(|| {
                    format!(
                        "failed to stop agent process: agent_id={agent_id} session_id={session_id}"
                    )
                })?;

            let removed = {
                let mut guard = self.inner.write().await;
                if AgentManager::handle_matches_session(guard.get(agent_id), &session_id) {
                    guard.remove(agent_id)
                } else {
                    None
                }
            };
            if let Some(handle) = removed
                && let Some(controller) = handle.loop_controller
            {
                controller.stop();
            }
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
                output_tx,
                agent_id.to_string(),
                session_id.clone(),
                "cancelled",
            )
            .await;
            if let Some(idle_gc) = &self.idle_gc {
                idle_gc.remove_agent(agent_id).await;
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
        let mut transaction = self.db.begin().await.with_context(|| {
            format!(
                "failed to begin failed-session transaction: agent_id={agent_id} session_id={session_id}"
            )
        })?;
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'failed', ?3, ?4)
            "#,
        )
        .bind(session_id)
        .bind(agent_id)
        .bind(now)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .with_context(|| {
            format!(
                "failed to insert failed agent session row: agent_id={agent_id} session_id={session_id}"
            )
        })?;
        sqlx::query(
            r#"
            UPDATE agent_sessions
            SET status = 'failed', ended_at = ?1
            WHERE id = ?2 AND agent_id = ?3
            "#,
        )
        .bind(now)
        .bind(session_id)
        .bind(agent_id)
        .execute(&mut *transaction)
        .await
        .with_context(|| {
            format!(
                "failed to update failed agent session row: agent_id={agent_id} session_id={session_id}"
            )
        })?;
        transaction.commit().await.with_context(|| {
            format!(
                "failed to commit failed-session transaction: agent_id={agent_id} session_id={session_id}"
            )
        })?;

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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use chrono::Utc;
    use sqlx::Row;

    use super::{
        RESUMED_ACP_SESSION_FAILURES_BEFORE_FRESH_RETRY, RESUMED_ACP_SESSION_GRACE_PERIOD,
        RESUMED_ACP_SESSION_POLL_INTERVAL, ResumedSessionStartupState,
        observe_resumed_session_startup, should_force_fresh_session_after_resume_failures,
        should_retry_resumed_acp_session,
    };
    use crate::agent::manager::supervisor::SharedSupervisedChild;
    use crate::agent::manager::{
        AgentManager, AgentStartSchedulerSettings,
        executor::{AgentExecutor, LocalExecutionRequest, SpawnedLocalProcess},
    };
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::process::Command;
    use tokio::sync::Mutex;

    #[derive(Debug, Default)]
    struct HangingExecutor;

    #[async_trait]
    impl AgentExecutor for HangingExecutor {
        async fn spawn_process(
            &self,
            _request: LocalExecutionRequest,
        ) -> anyhow::Result<SpawnedLocalProcess> {
            std::future::pending().await
        }
    }

    #[derive(Debug, Default)]
    struct FailingExecutor {
        attempts: AtomicUsize,
    }

    #[async_trait]
    impl AgentExecutor for FailingExecutor {
        async fn spawn_process(
            &self,
            _request: LocalExecutionRequest,
        ) -> anyhow::Result<SpawnedLocalProcess> {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            anyhow::bail!("synthetic spawn failure")
        }
    }

    async fn build_scheduled_test_manager(
        executor: Arc<dyn AgentExecutor>,
        settings: AgentStartSchedulerSettings,
    ) -> (AgentManager, String) {
        let state = crate::api::team_tests::build_test_state().await;
        let mut agents = AgentManager::new(
            state.db.clone(),
            state.agents.event_dbs.clone(),
            None,
            state.push.clone(),
            Vec::new(),
            "agenthubd".to_string(),
            None,
            true,
            state.acp_permissions.clone(),
            state.auth.clone(),
        )
        .with_start_scheduler_settings(settings);
        agents.local_executor = executor;

        let agent_id = format!("scheduled-agent-{}", uuid::Uuid::new_v4());
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, workdir, command, args, worktree_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, '/tmp', 'synthetic-agent', '[]', 'use_existing', 'stopped', ?3, ?3)
            "#,
        )
        .bind(&agent_id)
        .bind(&agent_id)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert scheduled test agent");
        (agents, agent_id)
    }

    #[test]
    fn should_retry_resumed_acp_session_only_on_failed_matching_resume() {
        assert!(should_retry_resumed_acp_session(
            Some("resume-1"),
            "resume-1",
            ResumedSessionStartupState::Exited { success: false },
        ));
        assert!(!should_retry_resumed_acp_session(
            None,
            "resume-1",
            ResumedSessionStartupState::Exited { success: false },
        ));
        assert!(!should_retry_resumed_acp_session(
            Some("resume-1"),
            "new-session",
            ResumedSessionStartupState::Exited { success: false },
        ));
        assert!(!should_retry_resumed_acp_session(
            Some("resume-1"),
            "resume-1",
            ResumedSessionStartupState::Exited { success: true },
        ));
        assert!(!should_retry_resumed_acp_session(
            Some("resume-1"),
            "resume-1",
            ResumedSessionStartupState::Running,
        ));
    }

    #[test]
    fn fresh_session_retry_only_triggers_after_three_resume_failures() {
        assert!(!should_force_fresh_session_after_resume_failures(1));
        assert!(!should_force_fresh_session_after_resume_failures(
            RESUMED_ACP_SESSION_FAILURES_BEFORE_FRESH_RETRY - 1
        ));
        assert!(should_force_fresh_session_after_resume_failures(
            RESUMED_ACP_SESSION_FAILURES_BEFORE_FRESH_RETRY
        ));
        assert!(should_force_fresh_session_after_resume_failures(
            RESUMED_ACP_SESSION_FAILURES_BEFORE_FRESH_RETRY + 1
        ));
    }

    #[tokio::test]
    async fn start_timeout_persists_failure_before_releasing_reservation() {
        let settings = AgentStartSchedulerSettings {
            max_concurrent_starts: 1,
            queue_timeout: Duration::from_millis(100),
            start_timeout: Duration::from_millis(25),
            spawn_backoff_initial: Duration::from_millis(20),
            spawn_backoff_max: Duration::from_millis(40),
        };
        let (agents, agent_id) =
            build_scheduled_test_manager(Arc::new(HangingExecutor), settings).await;

        let error = agents
            .start_agent(&agent_id)
            .await
            .expect_err("hanging start must time out");
        assert!(error.to_string().contains("agent start timed out"));
        assert!(!agents.starting.lock().await.contains(&agent_id));

        let agent_status: String = sqlx::query_scalar("SELECT status FROM agents WHERE id = ?1")
            .bind(&agent_id)
            .fetch_one(&agents.db)
            .await
            .expect("read timed-out agent status");
        assert_eq!(agent_status, "failed");
        let session = sqlx::query(
            "SELECT status, ended_at FROM agent_sessions WHERE agent_id = ?1 ORDER BY started_at DESC LIMIT 1",
        )
        .bind(&agent_id)
        .fetch_one(&agents.db)
        .await
        .expect("read timed-out session");
        assert_eq!(session.get::<String, _>("status"), "failed");
        assert!(session.get::<Option<i64>, _>("ended_at").is_some());
    }

    #[tokio::test]
    async fn spawn_failure_backoff_rejects_immediate_retry_without_respawn() {
        let settings = AgentStartSchedulerSettings {
            max_concurrent_starts: 1,
            queue_timeout: Duration::from_millis(100),
            start_timeout: Duration::from_secs(1),
            spawn_backoff_initial: Duration::from_secs(1),
            spawn_backoff_max: Duration::from_secs(2),
        };
        let executor = Arc::new(FailingExecutor::default());
        let (agents, agent_id) = build_scheduled_test_manager(executor.clone(), settings).await;

        let first_error = agents
            .start_agent(&agent_id)
            .await
            .expect_err("synthetic spawn must fail");
        assert!(first_error.to_string().contains("retry_after_ms"));
        let retry_error = agents
            .start_agent(&agent_id)
            .await
            .expect_err("immediate retry must hit spawn backoff");
        assert!(retry_error.to_string().contains("spawn backoff active"));
        assert_eq!(executor.attempts.load(Ordering::SeqCst), 1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn observe_resumed_session_startup_detects_early_failure() {
        let child = Command::new("sh")
            .arg("-lc")
            .arg("exit 1")
            .spawn()
            .expect("spawn failing child");
        let child: SharedSupervisedChild = Arc::new(Mutex::new(Some(Box::new(child))));

        let startup_state = observe_resumed_session_startup(
            &child,
            Duration::from_secs(1),
            Duration::from_millis(50),
        )
        .await;
        assert_eq!(
            startup_state,
            ResumedSessionStartupState::Exited { success: false }
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn observe_resumed_session_startup_treats_running_child_as_healthy() {
        let child = Command::new("sh")
            .arg("-lc")
            .arg("sleep 1")
            .spawn()
            .expect("spawn sleeping child");
        let child: SharedSupervisedChild = Arc::new(Mutex::new(Some(Box::new(child))));

        let startup_state = observe_resumed_session_startup(
            &child,
            RESUMED_ACP_SESSION_POLL_INTERVAL,
            Duration::from_millis(10),
        )
        .await;
        assert_eq!(startup_state, ResumedSessionStartupState::Running);

        let mut child_guard = child.lock().await;
        if let Some(mut child) = child_guard.take() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
    }

    #[test]
    fn resumed_session_grace_period_stays_short() {
        assert!(RESUMED_ACP_SESSION_GRACE_PERIOD >= Duration::from_secs(1));
        assert!(RESUMED_ACP_SESSION_GRACE_PERIOD <= Duration::from_secs(5));
    }
}
