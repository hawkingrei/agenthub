use std::collections::HashMap;
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
use crate::acp::{AcpHandle, AcpPermissionService, spawn_acp_session};
use crate::auth::AuthService;
use crate::push::PushService;

#[derive(Clone)]
pub struct AgentManager {
    db: SqlitePool,
    push: Arc<PushService>,
    auth: Arc<AuthService>,
    proxy_env: Vec<(String, String)>,
    codex_acp_binary: String,
    permissions: Arc<AcpPermissionService>,
    inner: Arc<RwLock<HashMap<String, AgentHandle>>>,
}

const ACP_PROVIDER_CODEX: &str = "codex";

pub struct AgentHandle {
    child: Arc<Mutex<Option<Child>>>,
    output_tx: broadcast::Sender<AgentOutput>,
    input: AgentInput,
    session_id: String,
    acp_session_id: Option<String>,
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
        permissions: Arc<AcpPermissionService>,
        auth: Arc<AuthService>,
    ) -> Self {
        Self {
            db,
            push,
            auth,
            proxy_env,
            codex_acp_binary,
            permissions,
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
        before_seq: Option<i64>,
    ) -> anyhow::Result<Vec<AgentEvent>> {
        let rows = if let Some(before_seq) = before_seq {
            sqlx::query(
                r#"
                SELECT agent_id, session_id, seq, ts, stream, message
                FROM agent_events
                WHERE agent_id = ?1 AND seq < ?2
                ORDER BY seq DESC
                LIMIT ?3
                "#,
            )
            .bind(agent_id)
            .bind(before_seq)
            .bind(limit)
            .fetch_all(&self.db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT agent_id, session_id, seq, ts, stream, message
                FROM agent_events
                WHERE agent_id = ?1
                ORDER BY seq DESC
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
        before_seq: Option<i64>,
    ) -> anyhow::Result<Vec<AgentEvent>> {
        let rows = if let Some(before_seq) = before_seq {
            sqlx::query(
                r#"
                SELECT agent_id, session_id, seq, ts, stream, message
                FROM agent_events
                WHERE agent_id = ?1 AND session_id = ?2 AND seq < ?3
                ORDER BY seq DESC
                LIMIT ?4
                "#,
            )
            .bind(agent_id)
            .bind(session_id)
            .bind(before_seq)
            .bind(limit)
            .fetch_all(&self.db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT agent_id, session_id, seq, ts, stream, message
                FROM agent_events
                WHERE agent_id = ?1 AND session_id = ?2
                ORDER BY seq DESC
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

    pub async fn start_agent(&self, agent_id: &str) -> anyhow::Result<String> {
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

        let is_acp = self.is_acp_command(&agent.command);
        let command_path = self.resolve_command_path(&agent.command, is_acp);
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

        let (input, acp_session_id) = if is_acp {
            let resume_session_id =
                self.get_persistent_session(&agent.id, ACP_PROVIDER_CODEX).await?;
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
            let handle = match spawn_acp_session(
                self.db.clone(),
                output_tx.clone(),
                self.permissions.clone(),
                agent.id.clone(),
                session_id.clone(),
                resume_session_id,
                workdir.clone(),
                stdout,
                stdin,
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
                .set_persistent_session(&agent.id, ACP_PROVIDER_CODEX, &handle.session_id)
                .await
            {
                tracing::error!("persist acp session failed: {}", err);
            }
            (AgentInput::Acp(handle.clone()), Some(handle.session_id.clone()))
        } else {
            (AgentInput::Stdin(stdin.clone()), None)
        };

        let handle = AgentHandle {
            child: child.clone(),
            output_tx: output_tx.clone(),
            input,
            session_id: session_id.clone(),
            acp_session_id,
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
        let mut guard = self.inner.write().await;
        if let Some(handle) = guard.get_mut(agent_id) {
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
        let seq = now_nanos();
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
        let (stdin, acp, output_tx, session_id) = {
            let guard = self.inner.read().await;
            let handle = guard
                .get(agent_id)
                .ok_or_else(|| anyhow::anyhow!("agent not running"))?;
            match &handle.input {
                AgentInput::Stdin(stdin) => (Some(stdin.clone()), None, None, None),
                AgentInput::Acp(acp) => (
                    None,
                    Some(acp.clone()),
                    Some(handle.output_tx.clone()),
                    Some(handle.session_id.clone()),
                ),
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

        let seq = Utc::now().timestamp_nanos_opt().unwrap_or(0);
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
        let output = AgentOutput {
            agent_id: agent_id.to_string(),
            session_id: session_id.clone(),
            seq,
            ts: Utc::now().timestamp(),
            stream: OutputStream::Acp,
            message: message.clone(),
        };
        let _ = output_tx.send(output);
        let _ = sqlx::query(
            r#"
            INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(agent_id)
        .bind(&session_id)
        .bind(seq)
        .bind(Utc::now().timestamp())
        .bind(stream_to_str(&OutputStream::Acp))
        .bind(message)
        .execute(&self.db)
        .await;

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
        acp.set_config(config_id.to_string(), value.to_string()).await
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
        let seq = now_nanos();
        let output = AgentOutput {
            agent_id: agent_id.clone(),
            session_id: session_id.clone(),
            seq,
            ts: Utc::now().timestamp(),
            stream: OutputStream::Acp,
            message: message.clone(),
        };
        let _ = output_tx.send(output);
        let _ = sqlx::query(
            r#"
            INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&agent_id)
        .bind(&session_id)
        .bind(seq)
        .bind(Utc::now().timestamp())
        .bind(stream_to_str(&OutputStream::Acp))
        .bind(message)
        .execute(&self.db)
        .await;
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
                let seq = now_nanos();
                let output = AgentOutput {
                    agent_id: agent_id.clone(),
                    session_id: session_id.clone(),
                    seq,
                    ts: Utc::now().timestamp(),
                    stream: stream.clone(),
                    message: line.clone(),
                };
                let stream_name = stream_to_str(&output.stream);

                let _ = output_tx.send(output);

                let _ = sqlx::query(
                    r#"
                    INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    "#,
                )
                .bind(&agent_id)
                .bind(&session_id)
                .bind(seq)
                .bind(Utc::now().timestamp())
                .bind(stream_name)
                .bind(&line)
                .execute(&db)
                .await;
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
                let mut child_guard = child_mutex.lock().await;
                if let Some(child) = child_guard.as_mut() {
                    let status = child.wait().await;
                    let now = Utc::now().timestamp();
                    let state = if status.is_ok() {
                        "completed"
                    } else {
                        "failed"
                    };

                    let row = sqlx::query(
                        r#"
                        SELECT ended_at FROM agent_sessions WHERE id = ?1
                        "#,
                    )
                    .bind(&session_id)
                    .fetch_optional(&db)
                    .await
                    .ok()
                    .flatten();
                    let ended_at: Option<i64> = row.map(|r| r.get("ended_at"));
                    if ended_at.is_some() {
                        return;
                    }

                    let _ = sqlx::query(
                        r#"
                        UPDATE agent_sessions
                        SET status = ?1, ended_at = ?2
                        WHERE id = ?3
                        "#,
                    )
                    .bind(state)
                    .bind(now)
                    .bind(&session_id)
                    .execute(&db)
                    .await;

                    let _ = sqlx::query(
                        r#"
                        UPDATE agents
                        SET status = ?1, updated_at = ?2
                        WHERE id = ?3
                        "#,
                    )
                    .bind(if status.is_ok() { "stopped" } else { "failed" })
                    .bind(now)
                    .bind(&agent_id)
                    .execute(&db)
                    .await;

                    let seq = now_nanos();
                    let output = AgentOutput {
                        agent_id: agent_id_clone.clone(),
                        session_id: session_id.clone(),
                        seq,
                        ts: Utc::now().timestamp(),
                        stream: OutputStream::Acp,
                        message: serde_json::json!({
                            "type": "run_status",
                            "status": state,
                            "session_id": session_id.clone(),
                        })
                        .to_string(),
                    };
                    if let Some(handle) = inner.read().await.get(&agent_id_clone) {
                        let _ = handle.output_tx.send(output);
                    }
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                        "#,
                    )
                    .bind(&agent_id_clone)
                    .bind(&session_id)
                    .bind(seq)
                    .bind(Utc::now().timestamp())
                    .bind(stream_to_str(&OutputStream::Acp))
                    .bind(
                        serde_json::json!({
                            "type": "run_status",
                            "status": state,
                            "session_id": session_id.clone(),
                        })
                        .to_string(),
                    )
                    .execute(&db)
                    .await;

                    let _ = push.notify_agent_completed(&agent_id, &session_id).await;
                }
            }
        });
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
        let _ = sqlx::query("DELETE FROM agent_sessions WHERE agent_id = ?1")
            .bind(agent_id)
            .execute(&self.db)
            .await;
        let _ = sqlx::query("DELETE FROM agent_events WHERE agent_id = ?1")
            .bind(agent_id)
            .execute(&self.db)
            .await;
        let _ = sqlx::query("DELETE FROM agent_persistent_sessions WHERE agent_id = ?1")
            .bind(agent_id)
            .execute(&self.db)
            .await;
        let _ = sqlx::query("DELETE FROM agents WHERE id = ?1")
            .bind(agent_id)
            .execute(&self.db)
            .await;
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
    fn is_acp_command(&self, command: &str) -> bool {
        if command == self.codex_acp_binary {
            return true;
        }
        let command_name = Path::new(command).file_name().and_then(|n| n.to_str());
        let target_name = Path::new(&self.codex_acp_binary)
            .file_name()
            .and_then(|n| n.to_str());
        matches!(
            (command_name, target_name),
            (Some(cmd), Some(target)) if cmd == target
        )
    }

    fn resolve_command_path(&self, command: &str, is_acp: bool) -> String {
        if !is_acp {
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

fn now_nanos() -> i64 {
    Utc::now().timestamp_nanos_opt().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{
        AgentStatus, OutputStream, expand_tilde, is_path_allowed, normalize_path, status_from_str,
        status_to_str, stream_from_str, stream_to_str,
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
}
