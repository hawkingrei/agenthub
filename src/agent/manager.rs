use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use chrono::Utc;
use sqlx::{Row, SqlitePool};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{Mutex, RwLock, broadcast};
use uuid::Uuid;

use super::{
    AgentConfig, AgentEvent, AgentOutput, AgentRecord, AgentStatus, OutputStream, WorktreeMode,
};
use crate::acp::{
    AcpHandle, AcpPermissionService, AgenthubAcpEventSink, load_safe_paths, spawn_acp_session,
};
use agent_client_protocol::Implementation;
use crate::auth::AuthService;
use crate::push::PushService;

#[derive(Clone)]
pub struct AgentManager {
    db: SqlitePool,
    push: Arc<PushService>,
    auth: Arc<AuthService>,
    proxy_env: Vec<(String, String)>,
    codex_acp_binary: String,
    acp_default_mode: Option<String>,
    permissions: Arc<AcpPermissionService>,
    starting: Arc<Mutex<HashSet<String>>>,
    inner: Arc<RwLock<HashMap<String, AgentHandle>>>,
}

const ACP_PROVIDER_CODEX: &str = "codex";
const ACP_PROVIDER_GEMINI: &str = "gemini";
const ACP_PROVIDER_KIMI: &str = "kimi";

pub struct AgentHandle {
    child: Arc<Mutex<Option<Child>>>,
    output_tx: broadcast::Sender<AgentOutput>,
    input: AgentInput,
    session_id: String,
}

pub enum AgentInput {
    Stdin(Arc<Mutex<Option<ChildStdin>>>),
    Acp(AcpHandle),
}

impl AgentManager {
    pub fn new(
        db: SqlitePool,
        push: Arc<PushService>,
        proxy_env: Vec<(String, String)>,
        codex_acp_binary: String,
        acp_default_mode: Option<String>,
        permissions: Arc<AcpPermissionService>,
        auth: Arc<AuthService>,
    ) -> Self {
        Self {
            db,
            push,
            auth,
            proxy_env,
            codex_acp_binary,
            acp_default_mode,
            permissions,
            starting: Arc::new(Mutex::new(HashSet::new())),
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_agent(&self, config: AgentConfig) -> anyhow::Result<AgentRecord> {
        let workdir = expand_tilde(&config.workdir);
        let worktree_repo = config.worktree_repo.as_deref().map(expand_tilde);
        self.ensure_safe_path(&workdir).await?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        let args_json = serde_json::to_string(&config.args)?;
        let status = AgentStatus::Created;

        sqlx::query(
            r#"
            INSERT INTO agents (
                id,
                name,
                workdir,
                command,
                args,
                worktree_mode,
                worktree_repo,
                worktree_ref,
                code_mode,
                status,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
        )
        .bind(&id)
        .bind(&config.name)
        .bind(&workdir)
        .bind(&config.command)
        .bind(&args_json)
        .bind(worktree_mode_to_str(&config.worktree_mode))
        .bind(&worktree_repo)
        .bind(&config.worktree_ref)
        .bind(if config.code_mode { 1 } else { 0 })
        .bind(status_to_str(&status))
        .bind(now)
        .bind(now)
        .execute(&self.db)
        .await?;

        Ok(AgentRecord {
            id,
            name: config.name,
            workdir,
            command: config.command,
            args: config.args,
            worktree_mode: config.worktree_mode,
            worktree_repo,
            worktree_ref: config.worktree_ref,
            code_mode: config.code_mode,
            status,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn list_agents(&self) -> anyhow::Result<Vec<AgentRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            FROM agents
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.db)
        .await?;

        let mut agents = Vec::with_capacity(rows.len());
        for row in rows {
            let args = serde_json::from_str::<Vec<String>>(row.get("args"))?;
            let worktree_mode = worktree_mode_from_opt(row.try_get("worktree_mode").ok());
            let code_mode: i64 = row.try_get("code_mode").unwrap_or(0);
            agents.push(AgentRecord {
                id: row.get("id"),
                name: row.get("name"),
                workdir: row.get("workdir"),
                command: row.get("command"),
                args,
                worktree_mode,
                worktree_repo: row.try_get("worktree_repo").ok(),
                worktree_ref: row.try_get("worktree_ref").ok(),
                code_mode: code_mode != 0,
                status: status_from_str(row.get::<String, _>("status").as_str()),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }
        Ok(agents)
    }

    pub async fn get_agent(&self, agent_id: &str) -> anyhow::Result<AgentRecord> {
        let row = sqlx::query(
            r#"
            SELECT id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            FROM agents
            WHERE id = ?1
            "#,
        )
        .bind(agent_id)
        .fetch_one(&self.db)
        .await?;

        let args = serde_json::from_str::<Vec<String>>(row.get("args"))?;
        let worktree_mode = worktree_mode_from_opt(row.try_get("worktree_mode").ok());
        let code_mode: i64 = row.try_get("code_mode").unwrap_or(0);
        Ok(AgentRecord {
            id: row.get("id"),
            name: row.get("name"),
            workdir: row.get("workdir"),
            command: row.get("command"),
            args,
            worktree_mode,
            worktree_repo: row.try_get("worktree_repo").ok(),
            worktree_ref: row.try_get("worktree_ref").ok(),
            code_mode: code_mode != 0,
            status: status_from_str(row.get::<String, _>("status").as_str()),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    pub async fn list_events(
        &self,
        agent_id: &str,
        limit: i64,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<AgentEvent>> {
        let rows = if let Some(before_id) = before_id {
            sqlx::query(
                r#"
                SELECT id, agent_id, session_id, seq, ts, stream, message
                FROM agent_events
                WHERE agent_id = ?1 AND id < ?2
                ORDER BY id DESC
                LIMIT ?3
                "#,
            )
            .bind(agent_id)
            .bind(before_id)
            .bind(limit)
            .fetch_all(&self.db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, agent_id, session_id, seq, ts, stream, message
                FROM agent_events
                WHERE agent_id = ?1
                ORDER BY id DESC
                LIMIT ?2
                "#,
            )
            .bind(agent_id)
            .bind(limit)
            .fetch_all(&self.db)
            .await?
        };

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let stream_str: String = row.get("stream");
            events.push(AgentEvent {
                event_id: row.get("id"),
                agent_id: row.get("agent_id"),
                session_id: row.get("session_id"),
                seq: row.get("seq"),
                ts: row.get("ts"),
                stream: stream_from_str(&stream_str),
                message: row.get("message"),
            });
        }
        events.reverse();
        Ok(events)
    }

    pub async fn list_events_for_session(
        &self,
        agent_id: &str,
        session_id: &str,
        limit: i64,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<AgentEvent>> {
        let rows = if let Some(before_id) = before_id {
            sqlx::query(
                r#"
                SELECT id, agent_id, session_id, seq, ts, stream, message
                FROM agent_events
                WHERE agent_id = ?1 AND session_id = ?2 AND id < ?3
                ORDER BY id DESC
                LIMIT ?4
                "#,
            )
            .bind(agent_id)
            .bind(session_id)
            .bind(before_id)
            .bind(limit)
            .fetch_all(&self.db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, agent_id, session_id, seq, ts, stream, message
                FROM agent_events
                WHERE agent_id = ?1 AND session_id = ?2
                ORDER BY id DESC
                LIMIT ?3
                "#,
            )
            .bind(agent_id)
            .bind(session_id)
            .bind(limit)
            .fetch_all(&self.db)
            .await?
        };

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let stream_str: String = row.get("stream");
            events.push(AgentEvent {
                event_id: row.get("id"),
                agent_id: row.get("agent_id"),
                session_id: row.get("session_id"),
                seq: row.get("seq"),
                ts: row.get("ts"),
                stream: stream_from_str(&stream_str),
                message: row.get("message"),
            });
        }
        events.reverse();
        Ok(events)
    }

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

    pub async fn start_agent(&self, agent_id: &str) -> anyhow::Result<String> {
        if let Some(session_id) = self.get_running_session_id(agent_id).await {
            return Ok(session_id);
        }
        self.reserve_agent_start(agent_id).await?;
        let result = self.start_agent_inner(agent_id).await;
        self.release_agent_start(agent_id).await;
        result
    }

    async fn start_agent_inner(&self, agent_id: &str) -> anyhow::Result<String> {
        let agent = self.get_agent(agent_id).await?;
        let session_id = Uuid::new_v4().to_string();
        let workdir = expand_tilde(&agent.workdir);
        let worktree_repo = agent.worktree_repo.as_deref().map(expand_tilde);
        if workdir != agent.workdir || worktree_repo.as_deref() != agent.worktree_repo.as_deref() {
            let _ = sqlx::query(
                r#"
                UPDATE agents
                SET workdir = ?1, worktree_repo = ?2, updated_at = ?3
                WHERE id = ?4
                "#,
            )
            .bind(&workdir)
            .bind(&worktree_repo)
            .bind(Utc::now().timestamp())
            .bind(&agent.id)
            .execute(&self.db)
            .await;
        }
        if let Err(err) = self
            .prepare_worktree_with_paths(&agent, &workdir, worktree_repo.as_deref())
            .await
        {
            let _ = self
                .record_failed_session(&agent.id, &session_id, &err.to_string())
                .await;
            let _ = self
                .update_agent_status(&agent.id, AgentStatus::Failed)
                .await;
            return Err(err);
        }
        if let Err(err) = self.ensure_safe_path(&workdir).await {
            let _ = self
                .record_failed_session(&agent.id, &session_id, &err.to_string())
                .await;
            let _ = self
                .update_agent_status(&agent.id, AgentStatus::Failed)
                .await;
            return Err(err);
        }

        let acp_provider = self.acp_provider_for_agent(&agent.command, &agent.args);
        let is_acp = acp_provider.is_some();
        let command_path = self.resolve_command_path(&agent.command, acp_provider);
        let mut command = Command::new(&command_path);
        command
            .current_dir(&workdir)
            .args(&agent.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in &self.proxy_env {
            command.env(key, value);
        }

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                let _ = self
                    .record_failed_session(&agent.id, &session_id, &err.to_string())
                    .await;
                let _ = self
                    .update_agent_status(&agent.id, AgentStatus::Failed)
                    .await;
                tracing::error!(
                    "spawn failed: command={} workdir={} args={:?} error={}",
                    command_path,
                    workdir,
                    agent.args,
                    err
                );
                return Err(anyhow::anyhow!(
                    "spawn failed: command={} workdir={} args={:?} error={}",
                    command_path,
                    workdir,
                    agent.args,
                    err
                ));
            }
        };
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
            let _ = self
                .record_failed_session(&agent.id, &session_id, "session insert failed")
                .await;
            let _ = self
                .update_agent_status(&agent.id, AgentStatus::Failed)
                .await;
            return Err(err.into());
        }

        if let Err(err) = self
            .update_agent_status(&agent.id, AgentStatus::Running)
            .await
        {
            return Err(err);
        }

        let input = if let Some(provider) = acp_provider {
            let resume_session_id = self
                .get_persistent_session(&agent.id, provider)
                .await?;
            let stdout = match stdout.take() {
                Some(stdout) => stdout,
                None => {
                    let _ = self
                        .record_failed_session(&agent.id, &session_id, "acp stdout missing")
                        .await;
                    let _ = self
                        .update_agent_status(&agent.id, AgentStatus::Failed)
                        .await;
                    return Err(anyhow::anyhow!("acp stdout missing"));
                }
            };
            let stdin = match stdin.lock().await.take() {
                Some(stdin) => stdin,
                None => {
                    let _ = self
                        .record_failed_session(&agent.id, &session_id, "acp stdin missing")
                        .await;
                    let _ = self
                        .update_agent_status(&agent.id, AgentStatus::Failed)
                        .await;
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
                self.db.clone(),
                output_tx.clone(),
                agent.id.clone(),
                session_id.clone(),
            ));
            let client_info = Implementation::new("agenthub", env!("CARGO_PKG_VERSION"));
            let handle = match spawn_acp_session(
                event_sink,
                self.permissions.clone(),
                agent.id.clone(),
                session_id.clone(),
                resume_session_id,
                workdir.clone(),
                client_info,
                stdout,
                stdin,
                safe_paths,
            )
            .await
            {
                Ok(handle) => handle,
                Err(err) => {
                    let _ = self
                        .record_failed_session(&agent.id, &session_id, &err.to_string())
                        .await;
                    let _ = self
                        .update_agent_status(&agent.id, AgentStatus::Failed)
                        .await;
                    return Err(err);
                }
            };
            if let Err(err) = self
                .set_persistent_session(&agent.id, provider, &handle.session_id)
                .await
            {
                tracing::error!("persist acp session failed: {}", err);
            }
            if provider == ACP_PROVIDER_CODEX {
                if let Some(mode_id) = self.acp_default_mode.as_deref() {
                    if let Err(err) = handle.set_mode(mode_id.to_string()).await {
                        tracing::warn!(
                            "set acp default mode failed: agent_id={}, mode_id={}, error={}",
                            agent.id,
                            mode_id,
                            err
                        );
                    }
                }
            } else if self.acp_default_mode.is_some() {
                tracing::debug!(
                    "acp default mode ignored for provider {} (agent_id={})",
                    provider,
                    agent.id
                );
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
        };

        {
            let mut guard = self.inner.write().await;
            guard.insert(agent.id.clone(), handle);
        }

        if !is_acp {
            if let Some(stdout) = stdout {
                self.spawn_output_reader(
                    agent.id.clone(),
                    session_id.clone(),
                    OutputStream::Stdout,
                    stdout,
                    output_tx.clone(),
                )
                .await;
            }
        }

        if let Some(stderr) = stderr {
            self.spawn_output_reader(
                agent.id.clone(),
                session_id.clone(),
                OutputStream::Stderr,
                stderr,
                output_tx.clone(),
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

    async fn ensure_safe_path(&self, workdir: &str) -> anyhow::Result<()> {
        let allow = sqlx::query(
            r#"
            SELECT path
            FROM safe_paths
            ORDER BY id ASC
            "#,
        )
        .fetch_all(&self.db)
        .await?;

        if allow.is_empty() {
            anyhow::bail!("no safe paths configured");
        }

        let target = normalize_path(workdir);
        for row in allow {
            let path: String = row.get("path");
            let allowed = normalize_path(&expand_tilde(&path));
            if is_path_allowed(&target, &allowed) {
                return Ok(());
            }
        }

        anyhow::bail!("workdir not allowed")
    }

    pub async fn stop_agent(&self, agent_id: &str) -> anyhow::Result<()> {
        let handle = {
            let mut guard = self.inner.write().await;
            guard.remove(agent_id)
        };
        if let Some(handle) = handle {
            let session_id = handle.session_id.clone();
            let now = Utc::now().timestamp();
            let _ = sqlx::query(
                r#"
                UPDATE agent_sessions
                SET status = 'cancelled', ended_at = ?1
                WHERE id = ?2
                "#,
            )
            .bind(now)
            .bind(&session_id)
            .execute(&self.db)
            .await;
            let _ = self
                .update_agent_status(agent_id, AgentStatus::Stopped)
                .await;
            self.emit_run_status(
                handle.output_tx.clone(),
                agent_id.to_string(),
                session_id,
                "cancelled",
            )
            .await;
            let mut child_guard = handle.child.lock().await;
            if let Some(child) = child_guard.as_mut() {
                let _ = child.kill().await;
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
        let _ = sqlx::query(
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
        .await;

        let _ = sqlx::query(
            r#"
            INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(agent_id)
        .bind(session_id)
        .bind(seq)
        .bind(now)
        .bind(stream_to_str(&OutputStream::System))
        .bind(format!("start failed: {}", message))
        .execute(&self.db)
        .await;

        Ok(())
    }

    pub async fn subscribe_output(
        &self,
        agent_id: &str,
    ) -> anyhow::Result<broadcast::Receiver<AgentOutput>> {
        let guard = self.inner.read().await;
        let handle = guard
            .get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("agent not running"))?;
        Ok(handle.output_tx.subscribe())
    }

    pub async fn send_input(
        &self,
        agent_id: &str,
        input: &str,
        message_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        let handle_snapshot = {
            let guard = self.inner.read().await;
            guard.get(agent_id).map(|handle| match &handle.input {
                AgentInput::Stdin(stdin) => (Some(stdin.clone()), None, None, None),
                AgentInput::Acp(acp) => (
                    None,
                    Some(acp.clone()),
                    Some(handle.output_tx.clone()),
                    Some(handle.session_id.clone()),
                ),
            })
        };
        let (stdin, acp, output_tx, session_id) = match handle_snapshot {
            Some(snapshot) => snapshot,
            None => {
                let _ = sqlx::query(
                    r#"
                    UPDATE agent_sessions
                    SET status = 'exited', ended_at = ?1
                    WHERE agent_id = ?2 AND status = 'running' AND ended_at IS NULL
                    "#,
                )
                .bind(now)
                .bind(agent_id)
                .execute(&self.db)
                .await;
                let _ = sqlx::query(
                    r#"
                    UPDATE agents
                    SET status = 'exited', updated_at = ?1
                    WHERE id = ?2 AND status = 'running'
                    "#,
                )
                .bind(now)
                .bind(agent_id)
                .execute(&self.db)
                .await;
                return Err(anyhow::anyhow!("agent not running"));
            }
        };

        if let Some(stdin) = stdin {
            let mut stdin_guard = stdin.lock().await;
            if let Some(stdin) = stdin_guard.as_mut() {
                stdin.write_all(format!("{}\n", input).as_bytes()).await?;
                stdin.flush().await?;
                return Ok(());
            }
            return Err(anyhow::anyhow!("agent stdin closed"));
        }

        let acp = acp.ok_or_else(|| anyhow::anyhow!("agent not running"))?;
        let output_tx = output_tx.ok_or_else(|| anyhow::anyhow!("agent output missing"))?;
        let session_id = session_id.ok_or_else(|| anyhow::anyhow!("agent session missing"))?;

        let seq = Uuid::now_v7().to_string();
        let message_id = message_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| seq.to_string());
        let message = serde_json::json!({
            "type": "user_message",
            "text": input,
            "chunk": false,
            "message_id": message_id
        })
        .to_string();
        let ts = Utc::now().timestamp();
        let result = sqlx::query(
            r#"
            INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(agent_id)
        .bind(&session_id)
        .bind(&seq)
        .bind(ts)
        .bind(stream_to_str(&OutputStream::Acp))
        .bind(message.clone())
        .execute(&self.db)
        .await?;
        let output = AgentOutput {
            event_id: result.last_insert_rowid(),
            agent_id: agent_id.to_string(),
            session_id: session_id.clone(),
            seq: seq.clone(),
            ts,
            stream: OutputStream::Acp,
            message: message.clone(),
        };
        let _ = output_tx.send(output);

        acp.prompt(input.to_string()).await?;
        Ok(())
    }

    async fn update_agent_status(&self, agent_id: &str, status: AgentStatus) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            UPDATE agents
            SET status = ?1, updated_at = ?2
            WHERE id = ?3
            "#,
        )
        .bind(status_to_str(&status))
        .bind(now)
        .bind(agent_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn set_acp_mode(&self, agent_id: &str, mode_id: &str) -> anyhow::Result<()> {
        let acp = self.get_acp_handle(agent_id).await?;
        acp.set_mode(mode_id.to_string()).await
    }

    pub async fn set_acp_model(&self, agent_id: &str, model_id: &str) -> anyhow::Result<()> {
        let acp = self.get_acp_handle(agent_id).await?;
        acp.set_model(model_id.to_string()).await
    }

    pub async fn set_acp_config(
        &self,
        agent_id: &str,
        config_id: &str,
        value: &str,
    ) -> anyhow::Result<()> {
        let acp = self.get_acp_handle(agent_id).await?;
        acp.set_config(config_id.to_string(), value.to_string())
            .await
    }

    pub async fn cancel_acp(&self, agent_id: &str) -> anyhow::Result<()> {
        let acp = self.get_acp_handle(agent_id).await?;
        acp.cancel().await
    }

    async fn get_acp_handle(&self, agent_id: &str) -> anyhow::Result<AcpHandle> {
        let guard = self.inner.read().await;
        let handle = guard
            .get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("agent not running"))?;
        match &handle.input {
            AgentInput::Acp(acp) => Ok(acp.clone()),
            _ => Err(anyhow::anyhow!("agent is not acp")),
        }
    }

    async fn emit_run_status(
        &self,
        output_tx: broadcast::Sender<AgentOutput>,
        agent_id: String,
        session_id: String,
        status: &str,
    ) {
        let message = serde_json::json!({
            "type": "run_status",
            "status": status,
            "session_id": session_id,
        })
        .to_string();
        let seq = Uuid::now_v7().to_string();
        let ts = Utc::now().timestamp();
        let result = sqlx::query(
            r#"
            INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&agent_id)
        .bind(&session_id)
        .bind(&seq)
        .bind(ts)
        .bind(stream_to_str(&OutputStream::Acp))
        .bind(message.clone())
        .execute(&self.db)
        .await;
        let Ok(result) = result else {
            tracing::error!("emit_run_status: failed to persist event");
            return;
        };
        let output = AgentOutput {
            event_id: result.last_insert_rowid(),
            agent_id: agent_id.clone(),
            session_id: session_id.clone(),
            seq: seq.clone(),
            ts,
            stream: OutputStream::Acp,
            message: message.clone(),
        };
        let _ = output_tx.send(output);
    }

    async fn spawn_output_reader<R>(
        &self,
        agent_id: String,
        session_id: String,
        stream: OutputStream,
        reader: R,
        output_tx: broadcast::Sender<AgentOutput>,
    ) where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        let db = self.db.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let is_acp = is_acp_message(&line);
                let stream = if is_acp {
                    OutputStream::Acp
                } else {
                    stream.clone()
                };
                let seq = Uuid::now_v7().to_string();
                let ts = Utc::now().timestamp();
                let stream_name = stream_to_str(&stream);
                let result = sqlx::query(
                    r#"
                    INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    "#,
                )
                .bind(&agent_id)
                .bind(&session_id)
                .bind(&seq)
                .bind(ts)
                .bind(stream_name)
                .bind(&line)
                .execute(&db)
                .await;
                let Ok(result) = result else {
                    tracing::error!("spawn_output_reader: failed to persist event");
                    continue;
                };
                let output = AgentOutput {
                    event_id: result.last_insert_rowid(),
                    agent_id: agent_id.clone(),
                    session_id: session_id.clone(),
                    seq: seq.clone(),
                    ts,
                    stream: stream.clone(),
                    message: line.clone(),
                };
                let _ = output_tx.send(output);
            }
        });
    }

    async fn spawn_exit_watcher(&self, agent_id: String, session_id: String) {
        let db = self.db.clone();
        let inner = self.inner.clone();
        let push = self.push.clone();
        let agent_id_clone = agent_id.clone();
        tokio::spawn(async move {
            let child = {
                let guard = inner.read().await;
                guard.get(&agent_id).map(|h| h.child.clone())
            };

            if let Some(child_mutex) = child {
                let success = {
                    let mut child_guard = child_mutex.lock().await;
                    let child = match child_guard.as_mut() {
                        Some(child) => child,
                        None => return,
                    };
                    match child.wait().await {
                        Ok(status) => status.success(),
                        Err(_) => false,
                    }
                };
                Self::finalize_process_exit(
                    &db,
                    &inner,
                    &push,
                    &agent_id_clone,
                    &session_id,
                    success,
                )
                .await;
            }
        });
    }

    async fn finalize_process_exit(
        db: &SqlitePool,
        inner: &Arc<RwLock<HashMap<String, AgentHandle>>>,
        push: &Arc<PushService>,
        agent_id: &str,
        session_id: &str,
        success: bool,
    ) {
        let row = sqlx::query(
            r#"
            SELECT ended_at FROM agent_sessions WHERE id = ?1
            "#,
        )
        .bind(session_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten();
        let ended_at: Option<i64> = row.map(|r| r.get("ended_at"));
        if ended_at.is_some() {
            let mut guard = inner.write().await;
            guard.remove(agent_id);
            return;
        }

        let now = Utc::now().timestamp();
        let state = if success { "completed" } else { "failed" };
        let status = if success { "stopped" } else { "failed" };
        let _ = sqlx::query(
            r#"
            UPDATE agent_sessions
            SET status = ?1, ended_at = ?2
            WHERE id = ?3
            "#,
        )
        .bind(state)
        .bind(now)
        .bind(session_id)
        .execute(db)
        .await;

        let _ = sqlx::query(
            r#"
            UPDATE agents
            SET status = ?1, updated_at = ?2
            WHERE id = ?3
            "#,
        )
        .bind(status)
        .bind(now)
        .bind(agent_id)
        .execute(db)
        .await;

        let seq = Uuid::now_v7().to_string();
        let ts = Utc::now().timestamp();
        let message = serde_json::json!({
            "type": "run_status",
            "status": state,
            "session_id": session_id,
        })
        .to_string();
        let result = sqlx::query(
            r#"
            INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(agent_id)
        .bind(session_id)
        .bind(&seq)
        .bind(ts)
        .bind(stream_to_str(&OutputStream::Acp))
        .bind(message.clone())
        .execute(db)
        .await;

        let Ok(result) = result else {
            tracing::error!("finalize_process_exit: failed to persist event");
            let mut guard = inner.write().await;
            guard.remove(agent_id);
            return;
        };

        let output = AgentOutput {
            event_id: result.last_insert_rowid(),
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            seq,
            ts,
            stream: OutputStream::Acp,
            message,
        };
        if let Some(handle) = inner.read().await.get(agent_id) {
            let _ = handle.output_tx.send(output);
        }

        let _ = push.notify_agent_completed(agent_id, session_id).await;
        let mut guard = inner.write().await;
        guard.remove(agent_id);
    }

    async fn prepare_worktree_with_paths(
        &self,
        agent: &AgentRecord,
        workdir: &str,
        worktree_repo: Option<&str>,
    ) -> anyhow::Result<()> {
        match agent.worktree_mode {
            WorktreeMode::UseExisting => Ok(()),
            WorktreeMode::ReuseWorktree => {
                if !Path::new(workdir).exists() {
                    let detail = format!(
                        "agent_id={}, mode=reuse_worktree, workdir={}, error=worktree missing",
                        agent.id, workdir
                    );
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
                let detail = format!(
                    "agent_id={}, mode=reuse_worktree, workdir={}",
                    agent.id, workdir
                );
                let _ = self
                    .auth
                    .record_audit(None, None, "worktree_reuse", Some(&detail), None, None)
                    .await;
                Ok(())
            }
            WorktreeMode::CreateWorktree => {
                let repo =
                    worktree_repo.ok_or_else(|| anyhow::anyhow!("worktree_repo required"))?;
                if let Err(err) = self.ensure_safe_path(repo).await {
                    let detail = format!(
                        "agent_id={}, mode=create_worktree, repo={}, workdir={}, error={}",
                        agent.id, repo, workdir, err
                    );
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
                    let detail = format!(
                        "agent_id={}, mode=create_worktree, repo={}, workdir={}, error=workdir not empty",
                        agent.id, repo, workdir
                    );
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
                let ref_name = agent.worktree_ref.as_deref().unwrap_or("HEAD");
                let output = Command::new("git")
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
                    let detail = format!(
                        "agent_id={}, mode=create_worktree, repo={}, workdir={}, ref={}, error={}",
                        agent.id,
                        repo,
                        workdir,
                        ref_name,
                        stderr.trim()
                    );
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
                let detail = format!(
                    "agent_id={}, mode=create_worktree, repo={}, workdir={}, ref={}",
                    agent.id, repo, workdir, ref_name
                );
                let _ = self
                    .auth
                    .record_audit(None, None, "worktree_created", Some(&detail), None, None)
                    .await;
                Ok(())
            }
        }
    }

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

    pub async fn mark_exited_on_startup(&self) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        let _ = sqlx::query(
            r#"
            UPDATE agent_sessions
            SET status = 'exited', ended_at = ?1
            WHERE status = 'running' AND ended_at IS NULL
            "#,
        )
        .bind(now)
        .execute(&self.db)
        .await?;

        let _ = sqlx::query(
            r#"
            UPDATE agents
            SET status = 'exited', updated_at = ?1
            WHERE status = 'running'
            "#,
        )
        .bind(now)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn delete_agent(&self, agent_id: &str) -> anyhow::Result<()> {
        let mut guard = self.inner.write().await;
        guard.remove(agent_id);
        drop(guard);
        let mut tx = self.db.begin().await?;
        sqlx::query("DELETE FROM acp_permission_requests WHERE agent_id = ?1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM agent_events WHERE agent_id = ?1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM agent_sessions WHERE agent_id = ?1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM agent_persistent_sessions WHERE agent_id = ?1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM agents WHERE id = ?1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

fn status_to_str(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Created => "created",
        AgentStatus::Running => "running",
        AgentStatus::Stopped => "stopped",
        AgentStatus::Exited => "exited",
        AgentStatus::Failed => "failed",
    }
}

fn status_from_str(status: &str) -> AgentStatus {
    match status {
        "running" => AgentStatus::Running,
        "stopped" => AgentStatus::Stopped,
        "exited" => AgentStatus::Exited,
        "failed" => AgentStatus::Failed,
        _ => AgentStatus::Created,
    }
}

fn stream_to_str(stream: &OutputStream) -> &'static str {
    match stream {
        OutputStream::Stdout => "stdout",
        OutputStream::Stderr => "stderr",
        OutputStream::System => "system",
        OutputStream::Acp => "acp",
    }
}

fn stream_from_str(stream: &str) -> OutputStream {
    match stream {
        "stdout" => OutputStream::Stdout,
        "stderr" => OutputStream::Stderr,
        "acp" => OutputStream::Acp,
        _ => OutputStream::System,
    }
}

fn worktree_mode_to_str(mode: &WorktreeMode) -> &'static str {
    match mode {
        WorktreeMode::UseExisting => "use_existing",
        WorktreeMode::CreateWorktree => "create_worktree",
        WorktreeMode::ReuseWorktree => "reuse_worktree",
    }
}

fn worktree_mode_from_opt(mode: Option<String>) -> WorktreeMode {
    match mode.as_deref() {
        Some("create_worktree") => WorktreeMode::CreateWorktree,
        Some("reuse_worktree") => WorktreeMode::ReuseWorktree,
        _ => WorktreeMode::UseExisting,
    }
}

fn is_dir_empty(path: &Path) -> anyhow::Result<bool> {
    if !path.exists() {
        return Ok(true);
    }
    let mut entries = std::fs::read_dir(path)?;
    Ok(entries.next().is_none())
}

fn is_acp_message(line: &str) -> bool {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('{') {
        return false;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return false;
    };
    let Some(obj) = value.as_object() else {
        return false;
    };
    let Some(ty) = obj.get("type").and_then(|v| v.as_str()) else {
        return false;
    };
    matches!(
        ty,
        "tool_call" | "tool_call_update" | "agent_message" | "agent_thought" | "user_message"
    )
}

impl AgentManager {
    pub fn acp_provider_for_agent(
        &self,
        command: &str,
        args: &[String],
    ) -> Option<&'static str> {
        acp_provider_for_agent_with_binary(&self.codex_acp_binary, command, args)
    }

    fn resolve_command_path(&self, command: &str, provider: Option<&str>) -> String {
        if provider != Some(ACP_PROVIDER_CODEX) {
            return command.to_string();
        }
        let configured = &self.codex_acp_binary;
        if configured == command {
            return command.to_string();
        }
        let configured_path = Path::new(configured);
        if configured_path.is_absolute() || configured_path.exists() {
            return configured.to_string();
        }
        command.to_string()
    }
}

fn acp_provider_for_agent_with_binary(
    codex_acp_binary: &str,
    command: &str,
    args: &[String],
) -> Option<&'static str> {
    let provider = acp_provider_for_command_with_binary(codex_acp_binary, command)?;
    match provider {
        ACP_PROVIDER_GEMINI => {
            let has_flag = args.iter().any(|arg| {
                arg == "--experimental-acp" || arg.starts_with("--experimental-acp=")
            });
            if has_flag {
                Some(ACP_PROVIDER_GEMINI)
            } else {
                None
            }
        }
        ACP_PROVIDER_KIMI => {
            if args.iter().any(|arg| arg == "acp") {
                Some(ACP_PROVIDER_KIMI)
            } else {
                None
            }
        }
        _ => Some(ACP_PROVIDER_CODEX),
    }
}

fn acp_provider_for_command_with_binary(
    codex_acp_binary: &str,
    command: &str,
) -> Option<&'static str> {
    if command == codex_acp_binary {
        return Some(ACP_PROVIDER_CODEX);
    }
    let command_name = Path::new(command).file_name().and_then(|n| n.to_str())?;
    match command_name {
        "gemini" => Some(ACP_PROVIDER_GEMINI),
        "kimi" => Some(ACP_PROVIDER_KIMI),
        "agenthub-codex-acp" | "codex-acp" => Some(ACP_PROVIDER_CODEX),
        name => {
            let target_name = Path::new(codex_acp_binary)
                .file_name()
                .and_then(|n| n.to_str());
            if target_name.map_or(false, |target| name == target) {
                Some(ACP_PROVIDER_CODEX)
            } else {
                None
            }
        }
    }
}

fn expand_tilde(path: &str) -> String {
    if path == "~" {
        return std::env::var("HOME").unwrap_or_else(|_| path.to_string());
    }
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}/{}", home, stripped);
        }
    }
    path.to_string()
}

fn normalize_path(path: &str) -> String {
    let mut parts = Vec::new();
    for comp in std::path::Path::new(path).components() {
        match comp {
            std::path::Component::RootDir => parts.clear(),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::Normal(seg) => {
                parts.push(seg.to_string_lossy().to_string());
            }
            _ => {}
        }
    }
    format!("/{}", parts.join("/"))
}

fn is_path_allowed(target: &str, allowed: &str) -> bool {
    let target = normalize_path(target);
    let allowed = normalize_path(allowed);
    if target == allowed {
        return true;
    }
    if !target.starts_with(&allowed) {
        return false;
    }
    target.chars().nth(allowed.len()) == Some('/')
}

#[cfg(test)]
mod tests {
    use super::{
        ACP_PROVIDER_CODEX, ACP_PROVIDER_GEMINI, ACP_PROVIDER_KIMI, AgentStatus, OutputStream,
        acp_provider_for_agent_with_binary, expand_tilde, is_path_allowed, normalize_path,
        status_from_str, status_to_str, stream_from_str, stream_to_str,
    };
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn normalize_path_resolves_dot_and_parent() {
        assert_eq!(normalize_path("/a/b/./c"), "/a/b/c");
        assert_eq!(normalize_path("/a/b/../c"), "/a/c");
        assert_eq!(normalize_path("/a/./b/../c/."), "/a/c");
    }

    #[test]
    fn is_path_allowed_matches_exact_or_child() {
        assert!(is_path_allowed("/home/foo", "/home/foo"));
        assert!(is_path_allowed("/home/foo/bar", "/home/foo"));
        assert!(is_path_allowed("/home/foo/bar/baz", "/home/foo/bar"));
        assert!(!is_path_allowed("/home/foobar", "/home/foo"));
        assert!(!is_path_allowed("/home/foo/../bar", "/home/foo"));
    }

    #[test]
    fn expand_tilde_uses_home() {
        let _guard = ENV_LOCK.lock().unwrap();
        let prev = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("HOME", "/tmp/test-home");
        }
        assert_eq!(expand_tilde("~"), "/tmp/test-home");
        assert_eq!(expand_tilde("~/work"), "/tmp/test-home/work");
        if let Some(val) = prev {
            unsafe {
                std::env::set_var("HOME", val);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }
    }

    #[test]
    fn status_roundtrip() {
        let statuses = [
            AgentStatus::Created,
            AgentStatus::Running,
            AgentStatus::Stopped,
            AgentStatus::Exited,
            AgentStatus::Failed,
        ];
        for status in statuses {
            let s = status_to_str(&status);
            let parsed = status_from_str(s);
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn stream_roundtrip() {
        let streams = [
            OutputStream::Stdout,
            OutputStream::Stderr,
            OutputStream::System,
            OutputStream::Acp,
        ];
        for stream in streams {
            let s = stream_to_str(&stream);
            let parsed = stream_from_str(s);
            assert_eq!(stream, parsed);
        }
    }

    #[test]
    fn acp_provider_for_agent_requires_expected_args() {
        let codex_bin = "agenthub-codex-acp";
        assert_eq!(
            acp_provider_for_agent_with_binary(codex_bin, "gemini", &[]),
            None
        );
        assert_eq!(
            acp_provider_for_agent_with_binary(
                codex_bin,
                "gemini",
                &["--experimental-acp".to_string()]
            ),
            Some(ACP_PROVIDER_GEMINI)
        );
        assert_eq!(
            acp_provider_for_agent_with_binary(codex_bin, "kimi", &[]),
            None
        );
        assert_eq!(
            acp_provider_for_agent_with_binary(
                codex_bin,
                "kimi",
                &["acp".to_string()]
            ),
            Some(ACP_PROVIDER_KIMI)
        );
        assert_eq!(
            acp_provider_for_agent_with_binary(codex_bin, "codex-acp", &[]),
            Some(ACP_PROVIDER_CODEX)
        );
        assert_eq!(
            acp_provider_for_agent_with_binary(codex_bin, codex_bin, &[]),
            Some(ACP_PROVIDER_CODEX)
        );
    }
}
