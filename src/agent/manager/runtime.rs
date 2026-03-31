use std::path::Path;

use chrono::Utc;

use super::codec::is_dir_empty;
use super::worktree::{
    git_command_without_fsmonitor, repo_find_worktree_entry, worktree_ref_matches,
};
use super::{
    AgentInput, AgentManager, expand_tilde, normalize_agent_loop_config,
    spawn_agent_loop_controller, worktree_mode_to_str,
};
use crate::agent::{AgentRecord, WorktreeMode};

impl AgentManager {
    #[tracing::instrument(
        skip(self, agent),
        fields(
            agent_id = %agent.id,
            worktree_mode = ?agent.worktree_mode,
            workdir = %workdir,
            worktree_repo = ?worktree_repo
        ),
        err
    )]
    pub(super) async fn prepare_worktree_with_paths(
        &self,
        agent: &AgentRecord,
        workdir: &str,
        worktree_repo: Option<&str>,
    ) -> anyhow::Result<()> {
        match agent.worktree_mode {
            WorktreeMode::UseExisting => Ok(()),
            WorktreeMode::ReuseWorktree => {
                if !Path::new(workdir).exists() {
                    let detail = serde_json::json!({
                        "agent_id": agent.id,
                        "mode": "reuse_worktree",
                        "workdir": workdir,
                        "error": "worktree missing",
                    })
                    .to_string();
                    let _ = self
                        .auth
                        .record_audit(
                            None,
                            None,
                            "worktree_reuse_missing",
                            Some(&detail),
                            None,
                            None,
                        )
                        .await;
                    anyhow::bail!("worktree does not exist");
                }
                let detail = serde_json::json!({
                    "agent_id": agent.id,
                    "mode": "reuse_worktree",
                    "workdir": workdir,
                })
                .to_string();
                let _ = self
                    .auth
                    .record_audit(None, None, "worktree_reuse", Some(&detail), None, None)
                    .await;
                Ok(())
            }
            WorktreeMode::CreateWorktree => {
                let repo =
                    worktree_repo.ok_or_else(|| anyhow::anyhow!("worktree_repo required"))?;
                let ref_name = agent.worktree_ref.as_deref().unwrap_or("HEAD");
                if let Err(err) = self.ensure_safe_path(repo).await {
                    let detail = serde_json::json!({
                        "agent_id": agent.id,
                        "mode": "create_worktree",
                        "repo": repo,
                        "workdir": workdir,
                        "error": err.to_string(),
                    })
                    .to_string();
                    let _ = self
                        .auth
                        .record_audit(
                            None,
                            None,
                            "worktree_create_failed",
                            Some(&detail),
                            None,
                            None,
                        )
                        .await;
                    return Err(err);
                }
                if let Err(err) = self.ensure_safe_path(workdir).await {
                    let detail = serde_json::json!({
                        "agent_id": agent.id,
                        "mode": "create_worktree",
                        "repo": repo,
                        "workdir": workdir,
                        "error": err.to_string(),
                    })
                    .to_string();
                    let _ = self
                        .auth
                        .record_audit(
                            None,
                            None,
                            "worktree_create_failed",
                            Some(&detail),
                            None,
                            None,
                        )
                        .await;
                    return Err(err);
                }
                let workdir_path = Path::new(workdir);
                if workdir_path.exists() && !is_dir_empty(workdir_path)? {
                    let existing_worktree = match repo_find_worktree_entry(repo, workdir).await {
                        Ok(entry) => entry,
                        Err(err) => {
                            let detail = serde_json::json!({
                                "agent_id": agent.id,
                                "mode": "create_worktree",
                                "repo": repo,
                                "workdir": workdir,
                                "error": format!("worktree list failed: {err}"),
                            })
                            .to_string();
                            let _ = self
                                .auth
                                .record_audit(
                                    None,
                                    None,
                                    "worktree_create_failed",
                                    Some(&detail),
                                    None,
                                    None,
                                )
                                .await;
                            return Err(err);
                        }
                    };
                    if let Some(existing_worktree) = existing_worktree {
                        if self
                            .is_workdir_bound_to_other_agent(&agent.id, workdir)
                            .await?
                        {
                            let detail = serde_json::json!({
                                "agent_id": agent.id,
                                "mode": "create_worktree",
                                "repo": repo,
                                "workdir": workdir,
                                "error": "workdir belongs to another agent",
                            })
                            .to_string();
                            let _ = self
                                .auth
                                .record_audit(
                                    None,
                                    None,
                                    "worktree_create_failed",
                                    Some(&detail),
                                    None,
                                    None,
                                )
                                .await;
                            anyhow::bail!("existing worktree belongs to another agent");
                        }

                        if !worktree_ref_matches(&existing_worktree, ref_name) {
                            let existing_branch = existing_worktree.branch.as_deref();
                            let existing_head = existing_worktree.head.as_deref();
                            let detail = serde_json::json!({
                                "agent_id": agent.id,
                                "mode": "create_worktree",
                                "repo": repo,
                                "workdir": workdir,
                                "configured_ref": ref_name,
                                "existing_branch": existing_branch,
                                "existing_head": existing_head,
                                "action": "reuse_existing_worktree",
                            })
                            .to_string();
                            let _ = self
                                .auth
                                .record_audit(
                                    None,
                                    None,
                                    "worktree_reuse_ref_mismatch",
                                    Some(&detail),
                                    None,
                                    None,
                                )
                                .await;
                            tracing::warn!(
                                agent_id = %agent.id,
                                repo = %repo,
                                workdir = %workdir,
                                configured_ref = %ref_name,
                                existing_branch = ?existing_branch,
                                existing_head = ?existing_head,
                                "existing worktree ref mismatched configured ref; reusing worktree"
                            );
                        }

                        let detail = serde_json::json!({
                            "agent_id": agent.id,
                            "mode": "create_worktree",
                            "repo": repo,
                            "workdir": workdir,
                            "source": "existing_worktree",
                        })
                        .to_string();
                        let _ = self
                            .auth
                            .record_audit(None, None, "worktree_reuse", Some(&detail), None, None)
                            .await;
                        return Ok(());
                    }
                    let detail = serde_json::json!({
                        "agent_id": agent.id,
                        "mode": "create_worktree",
                        "repo": repo,
                        "workdir": workdir,
                        "error": "workdir not empty",
                    })
                    .to_string();
                    let _ = self
                        .auth
                        .record_audit(
                            None,
                            None,
                            "worktree_create_failed",
                            Some(&detail),
                            None,
                            None,
                        )
                        .await;
                    anyhow::bail!("workdir is not empty");
                }
                let output = git_command_without_fsmonitor()
                    .arg("-C")
                    .arg(repo)
                    .arg("worktree")
                    .arg("add")
                    .arg(workdir)
                    .arg(ref_name)
                    .output()
                    .await?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let detail = serde_json::json!({
                        "agent_id": agent.id,
                        "mode": "create_worktree",
                        "repo": repo,
                        "workdir": workdir,
                        "ref": ref_name,
                        "error": stderr.trim(),
                    })
                    .to_string();
                    let _ = self
                        .auth
                        .record_audit(
                            None,
                            None,
                            "worktree_create_failed",
                            Some(&detail),
                            None,
                            None,
                        )
                        .await;
                    anyhow::bail!("git worktree add failed: {}", stderr.trim());
                }
                let detail = serde_json::json!({
                    "agent_id": agent.id,
                    "mode": "create_worktree",
                    "repo": repo,
                    "workdir": workdir,
                    "ref": ref_name,
                })
                .to_string();
                let _ = self
                    .auth
                    .record_audit(None, None, "worktree_created", Some(&detail), None, None)
                    .await;
                Ok(())
            }
        }
    }

    async fn is_workdir_bound_to_other_agent(
        &self,
        agent_id: &str,
        workdir: &str,
    ) -> anyhow::Result<bool> {
        let row = sqlx::query(
            r#"
            SELECT id
            FROM agents
            WHERE workdir = ?1 AND id != ?2
            LIMIT 1
            "#,
        )
        .bind(workdir)
        .bind(agent_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.is_some())
    }

    #[tracing::instrument(skip(self), fields(agent_id = %agent_id, code_mode = code_mode), err)]
    pub async fn set_code_mode(&self, agent_id: &str, code_mode: bool) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            UPDATE agents
            SET code_mode = ?1, updated_at = ?2
            WHERE id = ?3
            "#,
        )
        .bind(if code_mode { 1 } else { 0 })
        .bind(now)
        .bind(agent_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    #[tracing::instrument(
        skip(self, prompt),
        fields(agent_id = %agent_id, enabled = enabled, idle_seconds = ?idle_seconds),
        err
    )]
    pub async fn set_agent_loop_config(
        &self,
        agent_id: &str,
        enabled: bool,
        idle_seconds: Option<i64>,
        prompt: Option<&str>,
    ) -> anyhow::Result<()> {
        let current = self.get_agent(agent_id).await?;
        let next_idle_seconds = idle_seconds.or(current.agent_loop_idle_seconds);
        let next_prompt = prompt
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| current.agent_loop_prompt.clone());
        let next_config =
            normalize_agent_loop_config(enabled, next_idle_seconds, next_prompt.as_deref());
        if enabled && next_config.is_none() {
            anyhow::bail!(
                "agent loop requires idle_seconds between 10 and 86400 and a non-empty prompt"
            );
        }

        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            UPDATE agents
            SET agent_loop_enabled = ?1,
                agent_loop_idle_seconds = ?2,
                agent_loop_prompt = ?3,
                updated_at = ?4
            WHERE id = ?5
            "#,
        )
        .bind(if enabled { 1 } else { 0 })
        .bind(next_idle_seconds)
        .bind(next_prompt.as_deref())
        .bind(now)
        .bind(agent_id)
        .execute(&self.db)
        .await?;

        let mut guard = self.inner.write().await;
        if let Some(handle) = guard.get_mut(agent_id) {
            match (&handle.input, next_config) {
                (AgentInput::Acp(acp), Some(config)) => {
                    if let Some(controller) = &handle.loop_controller {
                        controller.reconfigure(config)?;
                    } else {
                        handle.loop_controller = Some(spawn_agent_loop_controller(
                            self.event_dbs.clone(),
                            self.idle_gc.clone(),
                            handle.output_tx.clone(),
                            acp.clone(),
                            agent_id.to_string(),
                            handle.session_id.clone(),
                            config,
                        ));
                    }
                }
                (_, None) => {
                    if let Some(controller) = handle.loop_controller.take() {
                        controller.stop();
                    }
                }
                (AgentInput::Stdin(_), Some(_)) => {}
            }
        }
        Ok(())
    }

    pub(crate) async fn update_team_member_runtime_config(
        &self,
        agent_id: &str,
        workdir: &str,
        worktree_mode: WorktreeMode,
        worktree_repo: Option<&str>,
        worktree_ref: Option<&str>,
    ) -> anyhow::Result<()> {
        let normalized_workdir = expand_tilde(workdir);
        let normalized_worktree_repo = worktree_repo.map(expand_tilde);
        self.ensure_safe_path(&normalized_workdir).await?;
        if let Some(repo) = normalized_worktree_repo.as_deref() {
            self.ensure_safe_path(repo).await?;
        }
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            UPDATE agents
            SET workdir = ?1,
                worktree_mode = ?2,
                worktree_repo = ?3,
                worktree_ref = ?4,
                updated_at = ?5
            WHERE id = ?6
            "#,
        )
        .bind(&normalized_workdir)
        .bind(worktree_mode_to_str(&worktree_mode))
        .bind(&normalized_worktree_repo)
        // worktree_ref is a Git ref, not a filesystem path, so tilde expansion is not applicable.
        .bind(worktree_ref)
        .bind(now)
        .bind(agent_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn mark_exited_on_startup(&self) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            UPDATE agent_sessions
            SET status = 'exited', ended_at = ?1
            WHERE status = 'running' AND ended_at IS NULL
            "#,
        )
        .bind(now)
        .execute(&self.db)
        .await
        .map_err(|err| {
            tracing::error!(
                error = %err,
                "mark_exited_on_startup failed to update running agent sessions"
            );
            err
        })?;

        sqlx::query(
            r#"
            UPDATE agents
            SET status = 'exited', updated_at = ?1
            WHERE status = 'running'
            "#,
        )
        .bind(now)
        .execute(&self.db)
        .await
        .map_err(|err| {
            tracing::error!(
                error = %err,
                "mark_exited_on_startup failed to update running agents"
            );
            err
        })?;

        Ok(())
    }

    #[tracing::instrument(skip(self), fields(agent_id = %agent_id), err)]
    pub async fn delete_agent(&self, agent_id: &str) -> anyhow::Result<()> {
        if let Ok(agent) = self.get_agent(agent_id).await
            && let Some(target_node_id) = agent.target_node_id.as_deref()
        {
            match self
                .remote_control_client_for_target_node(target_node_id)
                .await
            {
                Ok(client) => {
                    let _ = client.stop_managed_agent(agent_id).await;
                    if let Err(err) = client.delete_managed_agent(agent_id).await {
                        tracing::warn!(
                            error = %err,
                            %agent_id,
                            %target_node_id,
                            "remote delete_managed_agent failed during delete_agent; proceeding with local cleanup",
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        %agent_id,
                        %target_node_id,
                        "failed to create remote control client during delete_agent; proceeding with local cleanup",
                    );
                }
            }
        }
        let mut guard = self.inner.write().await;
        guard.remove(agent_id);
        drop(guard);
        let has_persistent_sessions_table = self.has_agent_persistent_sessions_table().await?;
        let mut tx = self.db.begin().await?;
        sqlx::query("DELETE FROM acp_permission_requests WHERE agent_id = ?1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM agent_sessions WHERE agent_id = ?1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        if has_persistent_sessions_table {
            sqlx::query("DELETE FROM agent_persistent_sessions WHERE agent_id = ?1")
                .bind(agent_id)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("DELETE FROM agents WHERE id = ?1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        if let Some(idle_gc) = &self.idle_gc {
            idle_gc.remove_agent(agent_id).await;
        }
        self.event_dbs.remove_agent_db(agent_id).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use sqlx::{Row, SqlitePool};
    use tokio::time::Duration;
    use uuid::Uuid;

    async fn build_test_state_with_idle_gc() -> crate::state::AppState {
        let state = crate::api::team_tests::build_test_state().await;
        let idle_gc = agenthub_db::AgentEventIdleGc::new(
            state.agents.event_dbs.clone(),
            5,
            false,
            100,
            Duration::from_millis(60),
        );
        let agents = std::sync::Arc::new(crate::agent::AgentManager::new(
            state.db.clone(),
            state.agents.event_dbs.clone(),
            Some(idle_gc),
            state.push.clone(),
            Vec::new(),
            "agenthub-codex-acp".to_string(),
            None,
            true,
            state.acp_permissions.clone(),
            state.auth.clone(),
        ));
        crate::state::AppState { agents, ..state }
    }

    async fn insert_agent_and_session(db: &SqlitePool, suffix: &str) -> (String, String) {
        let now = Utc::now().timestamp();
        let agent_id = format!("agent-runtime-{suffix}-{}", Uuid::new_v4());
        let session_id = format!("session-runtime-{suffix}-{}", Uuid::new_v4());
        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, agent_loop_enabled, agent_loop_idle_seconds, agent_loop_prompt, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, 0, NULL, NULL, ?7, ?8, ?9)
            "#,
        )
        .bind(&agent_id)
        .bind(format!("runtime-{suffix}"))
        .bind("/tmp")
        .bind("cat")
        .bind("[]")
        .bind("use_existing")
        .bind("running")
        .bind(now)
        .bind(now)
        .execute(db)
        .await
        .expect("insert test agent");
        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind(&session_id)
        .bind(&agent_id)
        .bind(now)
        .execute(db)
        .await
        .expect("insert test session");
        (agent_id, session_id)
    }

    async fn insert_running_handle(
        state: &crate::state::AppState,
        suffix: &str,
    ) -> (String, String) {
        let (agent_id, session_id) = insert_agent_and_session(&state.db, suffix).await;
        let mut command = tokio::process::Command::new("cat");
        command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let mut child = command.spawn().expect("spawn test child");
        let stdin = child.stdin.take();
        let (output_tx, _output_rx) = tokio::sync::broadcast::channel(8);
        let handle = super::super::AgentHandle {
            child: std::sync::Arc::new(tokio::sync::Mutex::new(Some(child))),
            output_tx,
            input: super::super::AgentInput::Stdin(std::sync::Arc::new(tokio::sync::Mutex::new(
                stdin,
            ))),
            session_id: session_id.clone(),
            actor_context: None,
            loop_controller: None,
        };
        state
            .agents
            .inner
            .write()
            .await
            .insert(agent_id.clone(), handle);
        (agent_id, session_id)
    }

    #[tokio::test]
    async fn send_input_does_not_mark_running_session_exited_while_agent_is_starting() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, session_id) =
            insert_agent_and_session(&state.db, "send-input-starting").await;

        {
            let mut starting = state.agents.starting.lock().await;
            starting.insert(agent_id.clone());
        }

        let err = state
            .agents
            .send_input(&agent_id, "hello", None, None)
            .await
            .expect_err("send_input should fail without a running handle");
        assert!(
            err.to_string().contains("agent not running"),
            "unexpected error: {err}"
        );

        let session_row = sqlx::query(
            r#"
            SELECT status, ended_at
            FROM agent_sessions
            WHERE id = ?1
            "#,
        )
        .bind(&session_id)
        .fetch_one(&state.db)
        .await
        .expect("load session row");
        assert_eq!(
            session_row.get::<String, _>("status"),
            "running",
            "session should stay running during startup window"
        );
        assert!(
            session_row.get::<Option<i64>, _>("ended_at").is_none(),
            "session ended_at should stay NULL during startup window"
        );

        let agent_row = sqlx::query(
            r#"
            SELECT status
            FROM agents
            WHERE id = ?1
            "#,
        )
        .bind(&agent_id)
        .fetch_one(&state.db)
        .await
        .expect("load agent row");
        assert_eq!(
            agent_row.get::<String, _>("status"),
            "running",
            "agent should stay running during startup window"
        );
    }

    #[tokio::test]
    async fn list_agents_reconciles_stale_running_status_without_runtime_handle() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, session_id) =
            insert_agent_and_session(&state.db, "list-agents-reconcile").await;

        let agents = state.agents.list_agents().await.expect("list agents");
        let agent = agents
            .into_iter()
            .find(|item| item.id == agent_id)
            .expect("stale agent should exist");
        assert_eq!(agent.status, crate::agent::AgentStatus::Exited);

        let session_row = sqlx::query(
            r#"
            SELECT status, ended_at
            FROM agent_sessions
            WHERE id = ?1
            "#,
        )
        .bind(&session_id)
        .fetch_one(&state.db)
        .await
        .expect("load reconciled session row");
        assert_eq!(session_row.get::<String, _>("status"), "exited");
        assert!(session_row.get::<Option<i64>, _>("ended_at").is_some());
    }

    #[tokio::test]
    async fn get_agent_reconciles_stale_running_status_without_runtime_handle() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, _session_id) =
            insert_agent_and_session(&state.db, "get-agent-reconcile").await;

        let agent = state
            .agents
            .get_agent(&agent_id)
            .await
            .expect("get agent should reconcile stale running status");
        assert_eq!(agent.status, crate::agent::AgentStatus::Exited);

        let agent_row = sqlx::query(
            r#"
            SELECT status
            FROM agents
            WHERE id = ?1
            "#,
        )
        .bind(&agent_id)
        .fetch_one(&state.db)
        .await
        .expect("load reconciled agent row");
        assert_eq!(agent_row.get::<String, _>("status"), "exited");
    }

    #[tokio::test]
    async fn list_agents_skips_remote_target_running_status_without_local_handle() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, session_id) =
            insert_agent_and_session(&state.db, "list-agents-remote-target").await;

        sqlx::query("ALTER TABLE agents ADD COLUMN target_node_id TEXT")
            .execute(&state.db)
            .await
            .expect("add target_node_id column");
        sqlx::query("UPDATE agents SET target_node_id = ?1 WHERE id = ?2")
            .bind("node-east")
            .bind(&agent_id)
            .execute(&state.db)
            .await
            .expect("mark list-agents target as remote");

        let agents = state.agents.list_agents().await.expect("list agents");
        let agent = agents
            .into_iter()
            .find(|item| item.id == agent_id)
            .expect("remote-target agent should exist");
        assert_eq!(agent.status, crate::agent::AgentStatus::Running);

        let session_row = sqlx::query(
            r#"
            SELECT status, ended_at
            FROM agent_sessions
            WHERE id = ?1
            "#,
        )
        .bind(&session_id)
        .fetch_one(&state.db)
        .await
        .expect("load remote-target running session row");
        assert_eq!(session_row.get::<String, _>("status"), "running");
        assert!(session_row.get::<Option<i64>, _>("ended_at").is_none());
    }

    #[tokio::test]
    async fn list_agents_does_not_reconcile_running_status_while_agent_is_starting() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, session_id) =
            insert_agent_and_session(&state.db, "list-agents-starting").await;

        {
            let mut starting = state.agents.starting.lock().await;
            starting.insert(agent_id.clone());
        }

        let agents = state.agents.list_agents().await.expect("list agents");
        let agent = agents
            .into_iter()
            .find(|item| item.id == agent_id)
            .expect("starting agent should exist");
        assert_eq!(agent.status, crate::agent::AgentStatus::Running);

        let session_row = sqlx::query(
            r#"
            SELECT status, ended_at
            FROM agent_sessions
            WHERE id = ?1
            "#,
        )
        .bind(&session_id)
        .fetch_one(&state.db)
        .await
        .expect("load running session row");
        assert_eq!(session_row.get::<String, _>("status"), "running");
        assert!(session_row.get::<Option<i64>, _>("ended_at").is_none());
    }

    #[tokio::test]
    async fn prepare_worktree_use_existing_mode_succeeds() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, _session_id) = insert_agent_and_session(&state.db, "prepare-existing").await;
        let agent = state
            .agents
            .get_agent(&agent_id)
            .await
            .expect("load inserted agent");

        state
            .agents
            .prepare_worktree_with_paths(&agent, &agent.workdir, None)
            .await
            .expect("use_existing should skip worktree mutations");
    }

    #[tokio::test]
    async fn set_code_mode_updates_agent_row() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, _session_id) = insert_agent_and_session(&state.db, "code-mode").await;

        state
            .agents
            .set_code_mode(&agent_id, true)
            .await
            .expect("set code mode");

        let row = sqlx::query("SELECT code_mode FROM agents WHERE id = ?1")
            .bind(&agent_id)
            .fetch_one(&state.db)
            .await
            .expect("load agent row");
        assert_eq!(row.get::<i64, _>("code_mode"), 1);
    }

    #[tokio::test]
    async fn set_agent_loop_config_updates_agent_row_without_blocking_runtime() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, _session_id) = insert_agent_and_session(&state.db, "agent-loop").await;

        state
            .agents
            .set_agent_loop_config(
                &agent_id,
                true,
                Some(900),
                Some("Resume by checking the current ACP thread and taking the next step."),
            )
            .await
            .expect("set agent loop");

        let row = sqlx::query(
            "SELECT agent_loop_enabled, agent_loop_idle_seconds, agent_loop_prompt FROM agents WHERE id = ?1",
        )
        .bind(&agent_id)
        .fetch_one(&state.db)
        .await
        .expect("load agent row");
        assert_eq!(row.get::<i64, _>("agent_loop_enabled"), 1);
        assert_eq!(row.get::<i64, _>("agent_loop_idle_seconds"), 900);
        assert_eq!(
            row.get::<String, _>("agent_loop_prompt"),
            "Resume by checking the current ACP thread and taking the next step."
        );
    }

    #[tokio::test]
    async fn delete_agent_removes_related_rows() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, session_id) = insert_agent_and_session(&state.db, "delete-agent").await;
        let now = Utc::now().timestamp();

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS agent_persistent_sessions (
                agent_id TEXT NOT NULL,
                provider TEXT NOT NULL,
                session_id TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (agent_id, provider)
            )
            "#,
        )
        .execute(&state.db)
        .await
        .expect("ensure agent_persistent_sessions table");

        let event_db = state
            .agents
            .event_dbs
            .pool_for_agent(&agent_id)
            .await
            .expect("open per-agent event db");
        sqlx::query(
            r#"
            INSERT INTO agent_events (session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(&session_id)
        .bind("seq-delete")
        .bind(now)
        .bind("system")
        .bind("event")
        .execute(&event_db)
        .await
        .expect("insert agent event");
        drop(event_db);

        sqlx::query(
            r#"
            INSERT INTO agent_persistent_sessions (agent_id, provider, session_id, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(&agent_id)
        .bind("codex")
        .bind("persisted-session")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert persistent session");

        state
            .agents
            .delete_agent(&agent_id)
            .await
            .expect("delete agent");

        let remaining_agents: i64 = sqlx::query("SELECT COUNT(*) AS cnt FROM agents WHERE id = ?1")
            .bind(&agent_id)
            .fetch_one(&state.db)
            .await
            .expect("count agents")
            .get("cnt");
        let remaining_sessions: i64 =
            sqlx::query("SELECT COUNT(*) AS cnt FROM agent_sessions WHERE agent_id = ?1")
                .bind(&agent_id)
                .fetch_one(&state.db)
                .await
                .expect("count sessions")
                .get("cnt");
        let remaining_persistent: i64 = sqlx::query(
            "SELECT COUNT(*) AS cnt FROM agent_persistent_sessions WHERE agent_id = ?1",
        )
        .bind(&agent_id)
        .fetch_one(&state.db)
        .await
        .expect("count persistent sessions")
        .get("cnt");

        let event_db_path = state.agents.event_dbs.db_path_for_agent(&agent_id);
        assert_eq!(remaining_agents, 0);
        assert_eq!(remaining_sessions, 0);
        assert_eq!(remaining_persistent, 0);
        assert!(
            !event_db_path.exists(),
            "agent event db should be deleted for removed agent"
        );
    }

    #[tokio::test]
    async fn delete_agent_keeps_local_cleanup_when_remote_client_is_unavailable() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, _) = insert_agent_and_session(&state.db, "delete-agent-remote").await;

        sqlx::query("ALTER TABLE agents ADD COLUMN target_node_id TEXT")
            .execute(&state.db)
            .await
            .expect("add target_node_id column");
        sqlx::query("UPDATE agents SET target_node_id = ?1 WHERE id = ?2")
            .bind("node-east")
            .bind(&agent_id)
            .execute(&state.db)
            .await
            .expect("mark agent as remote-target");

        state
            .agents
            .delete_agent(&agent_id)
            .await
            .expect("delete agent should still clean up local rows");

        let remaining_agents: i64 = sqlx::query("SELECT COUNT(*) AS cnt FROM agents WHERE id = ?1")
            .bind(&agent_id)
            .fetch_one(&state.db)
            .await
            .expect("count remaining agents")
            .get("cnt");
        let remaining_sessions: i64 =
            sqlx::query("SELECT COUNT(*) AS cnt FROM agent_sessions WHERE agent_id = ?1")
                .bind(&agent_id)
                .fetch_one(&state.db)
                .await
                .expect("count remaining sessions")
                .get("cnt");

        assert_eq!(remaining_agents, 0);
        assert_eq!(remaining_sessions, 0);
    }

    #[tokio::test]
    async fn mark_exited_on_startup_returns_error_after_pool_close() {
        let state = crate::api::team_tests::build_test_state().await;
        state.db.close().await;

        let err = state
            .agents
            .mark_exited_on_startup()
            .await
            .expect_err("closed pool should fail mark_exited_on_startup");
        assert!(
            !err.to_string().is_empty(),
            "expected non-empty error after close"
        );
    }

    #[tokio::test]
    async fn reconcile_runtime_absence_marks_stale_running_agent_exited() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, session_id) = insert_agent_and_session(&state.db, "reconcile-runtime").await;

        let reconciled = state
            .agents
            .reconcile_runtime_absence(&agent_id)
            .await
            .expect("reconcile stale runtime absence");
        assert!(reconciled, "expected stale running agent to be reconciled");

        let agent_row = sqlx::query("SELECT status FROM agents WHERE id = ?1")
            .bind(&agent_id)
            .fetch_one(&state.db)
            .await
            .expect("fetch agent row");
        assert_eq!(agent_row.get::<String, _>("status"), "exited");

        let session_row = sqlx::query("SELECT status, ended_at FROM agent_sessions WHERE id = ?1")
            .bind(&session_id)
            .fetch_one(&state.db)
            .await
            .expect("fetch session row");
        assert_eq!(session_row.get::<String, _>("status"), "exited");
        assert!(session_row.get::<Option<i64>, _>("ended_at").is_some());
    }

    #[tokio::test]
    async fn reconcile_runtime_absence_batch_reconciles_requested_agents_only() {
        let state = crate::api::team_tests::build_test_state().await;
        let (first_agent_id, first_session_id) =
            insert_agent_and_session(&state.db, "reconcile-runtime-batch-first").await;
        let (second_agent_id, second_session_id) =
            insert_agent_and_session(&state.db, "reconcile-runtime-batch-second").await;

        let reconciled = state
            .agents
            .reconcile_runtime_absence_batch(std::slice::from_ref(&first_agent_id))
            .await
            .expect("reconcile requested stale runtime absences");
        assert_eq!(reconciled, vec![first_agent_id.clone()]);

        let first_agent_status: String =
            sqlx::query_scalar("SELECT status FROM agents WHERE id = ?1")
                .bind(&first_agent_id)
                .fetch_one(&state.db)
                .await
                .expect("load first agent status");
        assert_eq!(first_agent_status, "exited");
        let first_session_status: String =
            sqlx::query_scalar("SELECT status FROM agent_sessions WHERE id = ?1")
                .bind(&first_session_id)
                .fetch_one(&state.db)
                .await
                .expect("load first session status");
        assert_eq!(first_session_status, "exited");

        let second_agent_status: String =
            sqlx::query_scalar("SELECT status FROM agents WHERE id = ?1")
                .bind(&second_agent_id)
                .fetch_one(&state.db)
                .await
                .expect("load second agent status");
        assert_eq!(second_agent_status, "running");
        let second_session_status: String =
            sqlx::query_scalar("SELECT status FROM agent_sessions WHERE id = ?1")
                .bind(&second_session_id)
                .fetch_one(&state.db)
                .await
                .expect("load second session status");
        assert_eq!(second_session_status, "running");
    }

    #[tokio::test]
    async fn reconcile_runtime_absence_batch_skips_remote_target_agents() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, session_id) =
            insert_agent_and_session(&state.db, "reconcile-runtime-batch-remote").await;

        sqlx::query("ALTER TABLE agents ADD COLUMN target_node_id TEXT")
            .execute(&state.db)
            .await
            .expect("add target_node_id column");
        sqlx::query("UPDATE agents SET target_node_id = ?1 WHERE id = ?2")
            .bind("node-east")
            .bind(&agent_id)
            .execute(&state.db)
            .await
            .expect("mark batch target as remote");

        let reconciled = state
            .agents
            .reconcile_runtime_absence_batch(std::slice::from_ref(&agent_id))
            .await
            .expect("reconcile remote-target stale runtime absences");
        assert!(reconciled.is_empty());

        let agent_status: String = sqlx::query_scalar("SELECT status FROM agents WHERE id = ?1")
            .bind(&agent_id)
            .fetch_one(&state.db)
            .await
            .expect("load remote-target agent status");
        assert_eq!(agent_status, "running");
        let session_status: String =
            sqlx::query_scalar("SELECT status FROM agent_sessions WHERE id = ?1")
                .bind(&session_id)
                .fetch_one(&state.db)
                .await
                .expect("load remote-target session status");
        assert_eq!(session_status, "running");
    }

    #[tokio::test]
    async fn stop_agent_removes_idle_gc_state_even_when_exit_watcher_exits_early() {
        let state = build_test_state_with_idle_gc().await;
        let idle_gc = state
            .agents
            .idle_gc
            .clone()
            .expect("test state should enable idle gc");
        let (agent_id, _session_id) = insert_running_handle(&state, "stop-idle-gc").await;

        idle_gc.record_activity(&agent_id).await;
        assert_eq!(idle_gc.tracked_agent_count().await, 1);

        state
            .agents
            .stop_agent(&agent_id)
            .await
            .expect("stop agent should succeed");

        assert_eq!(idle_gc.tracked_agent_count().await, 0);
    }
}
