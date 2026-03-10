mod codec;
mod runtime;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use chrono::Utc;
use sqlx::{Row, SqlitePool};
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{Mutex, RwLock, broadcast};
use uuid::Uuid;

#[cfg(test)]
use self::codec::acp_provider_for_agent_with_binary;
#[cfg(test)]
use self::codec::stream_to_str;
use self::codec::{
    expand_tilde, is_path_allowed, normalize_path, status_from_str, status_to_str, stream_from_str,
    worktree_mode_from_opt, worktree_mode_to_str,
};
use super::event_message_codec::{decode_message_from_storage, persist_agent_event};
use super::{
    AgentConfig, AgentEvent, AgentOutput, AgentRecord, AgentStatus, OutputStream, WorktreeMode,
};
use crate::acp::{
    AcpActorSkillContext, AcpHandle, AcpPermissionService, AcpPromptDeliveryPolicy,
    AgenthubAcpEventSink, SpawnAcpSessionRequest, load_safe_paths, normalize_actor_context,
    spawn_acp_session,
};
use crate::auth::AuthService;
use crate::db::{AgentEventDbRouter, AgentEventIdleGc};
use crate::push::PushService;
use agent_client_protocol::Implementation;

#[derive(Clone)]
pub struct AgentManager {
    db: SqlitePool,
    event_dbs: AgentEventDbRouter,
    idle_gc: Option<AgentEventIdleGc>,
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
const ACTOR_RUNTIME_TEAM_ID_ENV: &str = "AGENTHUB_ACTOR_TEAM_ID";
const ACTOR_RUNTIME_CURRENT_RUN_ID_ENV: &str = "AGENTHUB_ACTOR_CURRENT_RUN_ID";
const ACTOR_RUNTIME_RUN_ID_ENV: &str = "AGENTHUB_ACTOR_RUN_ID";
const ACTOR_RUNTIME_ACTOR_ID_ENV: &str = "AGENTHUB_ACTOR_ID";
const ACTOR_RUNTIME_CHANNEL_ENV: &str = "AGENTHUB_ACTOR_CHANNEL";
const ACTOR_RUNTIME_CLI_ENV: &str = "AGENTHUB_ACTOR_CLI";
const AGENT_SOURCE_MANUAL: &str = "manual";
const AGENT_SOURCE_TEAM_FORGE: &str = "team_forge";
const TEAM_MEMBER_ROLE_LEADER: &str = "leader";
const TEAM_MEMBER_ROLE_WORKER: &str = "worker";

fn acp_prompt_delivery_policy(provider: &str) -> AcpPromptDeliveryPolicy {
    match provider {
        ACP_PROVIDER_CODEX => AcpPromptDeliveryPolicy::AllowConcurrentPrompts,
        _ => AcpPromptDeliveryPolicy::StrictFifo,
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AgentSendInputError {
    #[error("agent session mismatch: expected={expected} running={running}")]
    SessionMismatch { expected: String, running: String },
}

#[derive(Debug, Clone)]
struct RuntimeStartPolicy {
    workdir: String,
    worktree_repo: Option<String>,
    worktree_mode: WorktreeMode,
    worktree_ref: Option<String>,
    worker_branch: Option<String>,
}

fn is_empty_or_missing_dir(path: &str) -> anyhow::Result<bool> {
    let dir = Path::new(path);
    if !dir.exists() {
        return Ok(true);
    }
    if !dir.is_dir() {
        return Ok(false);
    }
    let mut entries = std::fs::read_dir(dir)?;
    Ok(entries.next().is_none())
}

fn compact_token(raw: &str, fallback: &str, max_len: usize) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_sep = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_sep = false;
            continue;
        }
        if !prev_sep {
            out.push('-');
            prev_sep = true;
        }
    }
    let normalized = out.trim_matches('-');
    let mut token = if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized.to_string()
    };
    if token.len() > max_len {
        token.truncate(max_len);
        token = token.trim_matches('-').to_string();
        if token.is_empty() {
            return fallback.to_string();
        }
    }
    token
}

fn short_random_token() -> String {
    Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect::<String>()
}

fn derive_worker_runtime_root(workdir: &str) -> String {
    let path = Path::new(workdir);
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| parent.to_string_lossy().to_string())
        .unwrap_or_else(|| workdir.to_string())
}

fn derive_leader_runtime_workdir(
    workdir: &str,
    actor_context: &AcpActorSkillContext,
    session_id: &str,
) -> String {
    let actor_token = compact_token(&actor_context.actor_id, "leader", 24);
    let scope_token = actor_context
        .current_run_id
        .as_deref()
        .or(actor_context.team_id.as_deref())
        .map(|value| compact_token(value, "scope", 24))
        .unwrap_or_else(|| "scope".to_string());
    let session_token = compact_token(session_id, "session", 24);
    Path::new(workdir)
        .join(".agenthub-team-leader")
        .join(format!("{actor_token}-{scope_token}-{session_token}"))
        .to_string_lossy()
        .to_string()
}

fn build_runtime_start_policy(
    agent: &AgentRecord,
    actor_context: Option<&AcpActorSkillContext>,
    expanded_workdir: &str,
    expanded_worktree_repo: Option<&str>,
    start_session_id: Option<&str>,
) -> anyhow::Result<RuntimeStartPolicy> {
    let mut policy = RuntimeStartPolicy {
        workdir: expanded_workdir.to_string(),
        worktree_repo: expanded_worktree_repo.map(str::to_string),
        worktree_mode: agent.worktree_mode.clone(),
        worktree_ref: agent.worktree_ref.clone(),
        worker_branch: None,
    };

    let Some(role) = actor_context.and_then(|context| context.member_role.as_deref()) else {
        return Ok(policy);
    };

    match role {
        TEAM_MEMBER_ROLE_LEADER => {
            if !matches!(policy.worktree_mode, WorktreeMode::UseExisting) {
                anyhow::bail!(
                    "team leader policy requires worktree_mode=use_existing (agent_id={})",
                    agent.id
                );
            }
            if !is_empty_or_missing_dir(expanded_workdir)? {
                let context = actor_context
                    .ok_or_else(|| anyhow::anyhow!("leader role policy requires actor context"))?;
                let session_id = start_session_id.ok_or_else(|| {
                    anyhow::anyhow!("leader role policy requires start session id")
                })?;
                policy.workdir =
                    derive_leader_runtime_workdir(expanded_workdir, context, session_id);
            }
            if !is_empty_or_missing_dir(&policy.workdir)? {
                anyhow::bail!(
                    "team leader policy requires empty workdir (agent_id={} workdir={})",
                    agent.id,
                    policy.workdir
                );
            }
        }
        TEAM_MEMBER_ROLE_WORKER => {
            if !matches!(policy.worktree_mode, WorktreeMode::CreateWorktree) {
                anyhow::bail!(
                    "team worker policy requires worktree_mode=create_worktree (agent_id={})",
                    agent.id
                );
            }
            let repo = expanded_worktree_repo.ok_or_else(|| {
                anyhow::anyhow!(
                    "team worker policy requires worktree_repo when worktree_mode=create_worktree (agent_id={})",
                    agent.id
                )
            })?;
            let context = actor_context
                .ok_or_else(|| anyhow::anyhow!("worker role policy requires actor context"))?;
            let actor_token = compact_token(&context.actor_id, "worker", 24);
            let scope_token = context
                .current_run_id
                .as_deref()
                .or(context.team_id.as_deref())
                .map(|value| compact_token(value, "scope", 24))
                .unwrap_or_else(|| "scope".to_string());
            let root = derive_worker_runtime_root(expanded_workdir);
            let workdir = Path::new(&root)
                .join(format!("{actor_token}-{scope_token}"))
                .to_string_lossy()
                .to_string();
            let branch = format!("worker-{actor_token}-{}", short_random_token());
            policy.workdir = workdir;
            policy.worktree_repo = Some(repo.to_string());
            policy.worktree_mode = WorktreeMode::CreateWorktree;
            policy.worktree_ref = Some("HEAD".to_string());
            policy.worker_branch = Some(branch);
        }
        _ => {}
    }

    Ok(policy)
}

fn ensure_team_leader_workdir_exists(
    actor_context: Option<&AcpActorSkillContext>,
    workdir: &str,
) -> anyhow::Result<()> {
    let is_team_leader = matches!(
        actor_context.and_then(|context| context.member_role.as_deref()),
        Some(TEAM_MEMBER_ROLE_LEADER)
    );
    if is_team_leader
        && !Path::new(workdir).exists()
        && let Err(err) = std::fs::create_dir_all(workdir)
    {
        return Err(anyhow::anyhow!(
            "failed to create leader workdir: workdir={} error={}",
            workdir,
            err
        ));
    }
    Ok(())
}

pub struct AgentHandle {
    child: Arc<Mutex<Option<Child>>>,
    output_tx: broadcast::Sender<AgentOutput>,
    input: AgentInput,
    session_id: String,
    actor_context: Option<AcpActorSkillContext>,
}

pub enum AgentInput {
    Stdin(Arc<Mutex<Option<ChildStdin>>>),
    Acp(AcpHandle),
}

impl AgentManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: SqlitePool,
        event_dbs: AgentEventDbRouter,
        idle_gc: Option<AgentEventIdleGc>,
        push: Arc<PushService>,
        proxy_env: Vec<(String, String)>,
        codex_acp_binary: String,
        acp_default_mode: Option<String>,
        permissions: Arc<AcpPermissionService>,
        auth: Arc<AuthService>,
    ) -> Self {
        Self {
            db,
            event_dbs,
            idle_gc,
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
        self.create_agent_with_source(config, AGENT_SOURCE_MANUAL)
            .await
    }

    #[tracing::instrument(
        skip(self, config),
        fields(
            source = %source,
            agent_name = %config.name,
            workdir = %config.workdir
        ),
        err
    )]
    pub async fn create_agent_with_source(
        &self,
        config: AgentConfig,
        source: &str,
    ) -> anyhow::Result<AgentRecord> {
        if source != AGENT_SOURCE_MANUAL && source != AGENT_SOURCE_TEAM_FORGE {
            return Err(anyhow::anyhow!("invalid agent source: {source}"));
        }
        let workdir = expand_tilde(&config.workdir);
        let worktree_repo = config.worktree_repo.as_deref().map(expand_tilde);
        self.ensure_safe_path(&workdir).await?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        let args_json = serde_json::to_string(&config.args)?;
        let status = AgentStatus::Created;

        if self.has_agents_source_column().await? {
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
                    source,
                    status,
                    created_at,
                    updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
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
            .bind(source)
            .bind(status_to_str(&status))
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
        } else {
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
        }

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
        self.reconcile_stale_running_agents().await?;
        let active_team_member_agents = self.list_active_team_member_agents().await?;
        let rows = if self.has_agents_source_column().await? {
            sqlx::query(
                r#"
                SELECT id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
                FROM agents
                WHERE COALESCE(source, 'manual') != ?1
                ORDER BY created_at DESC
                "#,
            )
            .bind(AGENT_SOURCE_TEAM_FORGE)
            .fetch_all(&self.db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
                FROM agents
                ORDER BY created_at DESC
                "#,
            )
            .fetch_all(&self.db)
            .await?
        };

        let mut agents = Vec::with_capacity(rows.len());
        for row in rows {
            let agent_id: String = row.get("id");
            if active_team_member_agents.contains(&agent_id) {
                continue;
            }
            let args = serde_json::from_str::<Vec<String>>(row.get("args"))?;
            let worktree_mode = worktree_mode_from_opt(row.try_get("worktree_mode").ok());
            let code_mode: i64 = row.try_get("code_mode").unwrap_or(0);
            agents.push(AgentRecord {
                id: agent_id,
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

    pub async fn reconcile_runtime_absence(&self, agent_id: &str) -> anyhow::Result<bool> {
        let running: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM agents
            WHERE id = ?1 AND status = 'running'
            "#,
        )
        .bind(agent_id)
        .fetch_one(&self.db)
        .await?;
        if running == 0 {
            return Ok(false);
        }

        let reconciled = self
            .reconcile_running_agents_without_runtime_handles(vec![agent_id.to_string()])
            .await?;
        Ok(!reconciled.is_empty())
    }

    async fn reconcile_stale_running_agents(&self) -> anyhow::Result<()> {
        let running_rows = sqlx::query(
            r#"
            SELECT id
            FROM agents
            WHERE status = 'running'
            "#,
        )
        .fetch_all(&self.db)
        .await?;
        let running_ids = running_rows
            .into_iter()
            .map(|row| row.get::<String, _>("id"))
            .collect::<Vec<_>>();
        let _ = self
            .reconcile_running_agents_without_runtime_handles(running_ids)
            .await?;
        Ok(())
    }

    async fn reconcile_running_agents_without_runtime_handles(
        &self,
        running_ids: Vec<String>,
    ) -> anyhow::Result<Vec<String>> {
        if running_ids.is_empty() {
            return Ok(Vec::new());
        }

        let stale_ids = {
            let active_guard = self.inner.read().await;
            let starting_guard = self.starting.lock().await;
            running_ids
                .into_iter()
                .filter(|agent_id| {
                    !active_guard.contains_key(agent_id) && !starting_guard.contains(agent_id)
                })
                .collect::<Vec<_>>()
        };
        if stale_ids.is_empty() {
            return Ok(Vec::new());
        }

        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        for agent_id in &stale_ids {
            sqlx::query(
                r#"
                UPDATE agent_sessions
                SET status = 'exited', ended_at = ?1
                WHERE agent_id = ?2 AND status = 'running' AND ended_at IS NULL
                "#,
            )
            .bind(now)
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                r#"
                UPDATE agents
                SET status = 'exited', updated_at = ?1
                WHERE id = ?2 AND status = 'running'
                "#,
            )
            .bind(now)
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        tracing::warn!(
            stale_agent_count = stale_ids.len(),
            stale_agent_ids = ?stale_ids,
            "reconciled stale running agents without runtime handles"
        );
        Ok(stale_ids)
    }

    async fn has_agents_source_column(&self) -> anyhow::Result<bool> {
        let rows = sqlx::query(
            r#"
            SELECT name
            FROM pragma_table_info('agents')
            "#,
        )
        .fetch_all(&self.db)
        .await?;
        Ok(rows
            .into_iter()
            .any(|row| row.get::<String, _>("name") == "source"))
    }

    async fn list_active_team_member_agents(&self) -> anyhow::Result<HashSet<String>> {
        let has_team_tables: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM sqlite_master
            WHERE type = 'table'
              AND name IN ('team_runs', 'team_steps')
            "#,
        )
        .fetch_one(&self.db)
        .await?;
        if has_team_tables < 2 {
            return Ok(HashSet::new());
        }

        let rows = sqlx::query(
            r#"
            SELECT DISTINCT agent_id
            FROM (
                SELECT sessions.agent_id AS agent_id
                FROM team_steps AS steps
                JOIN team_runs AS runs ON runs.id = steps.run_id
                JOIN agent_sessions AS sessions ON sessions.id = steps.remote_task_id
                WHERE steps.remote_task_id IS NOT NULL
                  AND steps.status IN ('working', 'input_required')
                  AND runs.status IN ('submitted', 'working', 'input_required')
            )
            "#,
        )
        .fetch_all(&self.db)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| row.get::<String, _>("agent_id"))
            .collect())
    }

    pub async fn get_agent(&self, agent_id: &str) -> anyhow::Result<AgentRecord> {
        self.reconcile_stale_running_agents().await?;
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
        let event_db = self.event_dbs.pool_for_agent(agent_id).await?;
        let rows = if let Some(before_id) = before_id {
            sqlx::query(
                r#"
                SELECT id, session_id, seq, ts, stream, message
                FROM agent_events
                WHERE id < ?1
                ORDER BY id DESC
                LIMIT ?2
                "#,
            )
            .bind(before_id)
            .bind(limit)
            .fetch_all(&event_db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, session_id, seq, ts, stream, message
                FROM agent_events
                ORDER BY id DESC
                LIMIT ?1
                "#,
            )
            .bind(limit)
            .fetch_all(&event_db)
            .await?
        };

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let stream_str: String = row.get("stream");
            events.push(AgentEvent {
                event_id: row.get("id"),
                agent_id: agent_id.to_string(),
                session_id: row.get("session_id"),
                seq: row.get("seq"),
                ts: row.get("ts"),
                stream: stream_from_str(&stream_str),
                // Decode compressed ACP rows while keeping legacy plain rows untouched.
                message: decode_message_from_storage(row.get::<Vec<u8>, _>("message").as_slice()),
            });
        }
        events.reverse();
        Ok(events)
    }

    #[cfg(test)]
    pub(crate) async fn test_event_pool_for_agent(
        &self,
        agent_id: &str,
    ) -> anyhow::Result<SqlitePool> {
        self.event_dbs.pool_for_agent(agent_id).await
    }

    async fn record_agent_activity(&self, agent_id: &str) {
        if let Some(idle_gc) = &self.idle_gc {
            idle_gc.record_activity(agent_id).await;
        }
    }

    pub async fn list_events_for_session(
        &self,
        agent_id: &str,
        session_id: &str,
        limit: i64,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<AgentEvent>> {
        let event_db = self.event_dbs.pool_for_agent(agent_id).await?;
        let rows = if let Some(before_id) = before_id {
            sqlx::query(
                r#"
                SELECT id, session_id, seq, ts, stream, message
                FROM agent_events
                WHERE session_id = ?1 AND id < ?2
                ORDER BY id DESC
                LIMIT ?3
                "#,
            )
            .bind(session_id)
            .bind(before_id)
            .bind(limit)
            .fetch_all(&event_db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, session_id, seq, ts, stream, message
                FROM agent_events
                WHERE session_id = ?1
                ORDER BY id DESC
                LIMIT ?2
                "#,
            )
            .bind(session_id)
            .bind(limit)
            .fetch_all(&event_db)
            .await?
        };

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let stream_str: String = row.get("stream");
            events.push(AgentEvent {
                event_id: row.get("id"),
                agent_id: agent_id.to_string(),
                session_id: row.get("session_id"),
                seq: row.get("seq"),
                ts: row.get("ts"),
                stream: stream_from_str(&stream_str),
                // Decode compressed ACP rows while keeping legacy plain rows untouched.
                message: decode_message_from_storage(row.get::<Vec<u8>, _>("message").as_slice()),
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
        if let Some(session_id) = self.get_running_session_id(agent_id).await {
            if actor_context.is_some() {
                return Err(anyhow::anyhow!(
                    "agent already running with session '{}'; cannot start with new actor context",
                    session_id
                ));
            }
            return Ok(session_id);
        }
        self.reserve_agent_start(agent_id).await?;
        let result = self.start_agent_inner(agent_id, actor_context).await;
        self.release_agent_start(agent_id).await;
        result
    }

    #[tracing::instrument(
        skip(self, actor_context),
        fields(agent_id = %agent_id),
        err
    )]
    async fn start_agent_inner(
        &self,
        agent_id: &str,
        actor_context: Option<AcpActorSkillContext>,
    ) -> anyhow::Result<String> {
        let agent = self.get_agent(agent_id).await?;
        let session_id = Uuid::new_v4().to_string();
        let actor_context = actor_context.map(normalize_actor_context).transpose()?;
        let persisted_workdir = expand_tilde(&agent.workdir);
        let persisted_worktree_repo = agent.worktree_repo.as_deref().map(expand_tilde);
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

        let acp_provider = self.acp_provider_for_agent(&agent.command, &agent.args);
        let is_acp = acp_provider.is_some();
        let command_path = self.resolve_command_path(&agent.command, acp_provider);
        let mut command = Command::new(&command_path);
        command
            .current_dir(&start_policy.workdir)
            .args(&agent.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in &self.proxy_env {
            command.env(key, value);
        }
        if let Some(context) = actor_context.as_ref() {
            command.env(ACTOR_RUNTIME_ACTOR_ID_ENV, &context.actor_id);
            command.env(ACTOR_RUNTIME_CHANNEL_ENV, &context.default_channel);
            command.env(ACTOR_RUNTIME_CLI_ENV, &context.actor_cli_path);
            if let Some(team_id) = context
                .team_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                command.env(ACTOR_RUNTIME_TEAM_ID_ENV, team_id);
            }
            if let Some(run_id) = context
                .current_run_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                command.env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, run_id);
                command.env(ACTOR_RUNTIME_RUN_ID_ENV, run_id);
            }
        }

        let mut child = match command.spawn() {
            Ok(child) => child,
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
                    command_path,
                    start_policy.workdir,
                    agent.args,
                    err
                );
                return Err(anyhow::anyhow!(
                    "spawn failed: command={} workdir={} args={:?} error={}",
                    command_path,
                    start_policy.workdir,
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

        let input = if let Some(provider) = acp_provider {
            let resume_session_id = self.get_persistent_session(&agent.id, provider).await?;
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
            let handle = match spawn_acp_session(SpawnAcpSessionRequest {
                event_sink,
                permissions: self.permissions.clone(),
                agent_id: agent.id.clone(),
                agent_session_id: session_id.clone(),
                resume_session_id,
                workdir: start_policy.workdir.clone(),
                client_info,
                stdout,
                stdin,
                safe_paths,
                actor_context: actor_context.clone(),
                prompt_delivery_policy: acp_prompt_delivery_policy(provider),
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
                .set_persistent_session(&agent.id, provider, &handle.session_id)
                .await
            {
                tracing::error!("persist acp session failed: {}", err);
            }
            if provider == ACP_PROVIDER_CODEX {
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
            actor_context,
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

    async fn checkout_team_worker_branch(&self, workdir: &str, branch: &str) -> anyhow::Result<()> {
        let output = Command::new("git")
            .arg("-C")
            .arg(workdir)
            .arg("checkout")
            .arg("-B")
            .arg(branch)
            .output()
            .await?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            anyhow::bail!(
                "team worker policy failed to prepare branch '{}' in {}",
                branch,
                workdir
            );
        }
        anyhow::bail!(
            "team worker policy failed to prepare branch '{}' in {}: {}",
            branch,
            workdir,
            stderr
        );
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

    #[tracing::instrument(skip(self), fields(agent_id = %agent_id), err)]
    pub async fn stop_agent(&self, agent_id: &str) -> anyhow::Result<()> {
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
            let mut child_guard = handle.child.lock().await;
            if let Some(child) = child_guard.as_mut()
                && let Err(err) = child.kill().await
            {
                tracing::warn!(
                    agent_id = %agent_id,
                    error = %err,
                    "failed to kill agent child process during stop"
                );
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

    #[tracing::instrument(
        skip(self, input, message_id),
        fields(agent_id = %agent_id, expected_session_id = ?expected_session_id),
        err
    )]
    pub async fn send_input(
        &self,
        agent_id: &str,
        input: &str,
        message_id: Option<&str>,
        expected_session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let handle_snapshot = {
            let guard = self.inner.read().await;
            guard.get(agent_id).map(|handle| match &handle.input {
                AgentInput::Stdin(stdin) => (
                    Some(stdin.clone()),
                    None,
                    None,
                    Some(handle.session_id.clone()),
                ),
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
                let is_starting = {
                    let starting = self.starting.lock().await;
                    starting.contains(agent_id)
                };
                if is_starting {
                    tracing::debug!(
                        "send_input: skip exited fallback while agent is in startup window: {}",
                        agent_id
                    );
                    return Err(anyhow::anyhow!("agent not running"));
                }
                if let Err(err) = self.reconcile_runtime_absence(agent_id).await {
                    tracing::warn!(
                        agent_id = %agent_id,
                        error = %err,
                        "send_input fallback failed to reconcile stale running state"
                    );
                }
                return Err(anyhow::anyhow!("agent not running"));
            }
        };
        let session_id = session_id.ok_or_else(|| anyhow::anyhow!("agent session missing"))?;
        if let Some(expected_session_id) = expected_session_id
            && expected_session_id != session_id
        {
            return Err(AgentSendInputError::SessionMismatch {
                expected: expected_session_id.to_string(),
                running: session_id.clone(),
            }
            .into());
        }

        if let Some(stdin) = stdin {
            let mut stdin_guard = stdin.lock().await;
            if let Some(stdin) = stdin_guard.as_mut() {
                stdin.write_all(format!("{}\n", input).as_bytes()).await?;
                stdin.flush().await?;
                self.record_agent_activity(agent_id).await;
                return Ok(());
            }
            return Err(anyhow::anyhow!("agent stdin closed"));
        }

        let acp = acp.ok_or_else(|| anyhow::anyhow!("agent not running"))?;
        let output_tx = output_tx.ok_or_else(|| anyhow::anyhow!("agent output missing"))?;

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
        let event_id = persist_agent_event(
            &self.event_dbs,
            self.idle_gc.as_ref(),
            agent_id,
            &session_id,
            &seq,
            ts,
            &OutputStream::Acp,
            &message,
        )
        .await?;
        let output = AgentOutput {
            event_id,
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

    #[tracing::instrument(skip(self), fields(agent_id = %agent_id, mode_id = %mode_id), err)]
    pub async fn set_acp_mode(&self, agent_id: &str, mode_id: &str) -> anyhow::Result<()> {
        let acp = self.get_acp_handle(agent_id).await?;
        acp.set_mode(mode_id.to_string()).await
    }

    #[tracing::instrument(skip(self), fields(agent_id = %agent_id, model_id = %model_id), err)]
    pub async fn set_acp_model(&self, agent_id: &str, model_id: &str) -> anyhow::Result<()> {
        let acp = self.get_acp_handle(agent_id).await?;
        acp.set_model(model_id.to_string()).await
    }

    #[tracing::instrument(
        skip(self, value),
        fields(agent_id = %agent_id, config_id = %config_id),
        err
    )]
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

    #[tracing::instrument(skip(self), fields(agent_id = %agent_id), err)]
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
}
