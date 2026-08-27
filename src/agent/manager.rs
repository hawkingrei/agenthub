mod acp_provider;
mod codec;
mod executor;
mod nodes;
mod process;
mod runtime;
mod session;
mod start_plan;
mod start_scheduler;
mod store;
mod supervisor;
mod worktree;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};
use tokio::io::AsyncWriteExt;
use tokio::process::{ChildStdin, Command};
use tokio::sync::{Mutex, RwLock, broadcast, mpsc};
use uuid::Uuid;

use self::codec::{status_to_str, stream_from_str, worktree_mode_to_str};
use self::executor::{AgentExecutor, LocalExecutor};
use self::nodes::{
    AgentNodeInsertRecord, AgentNodeSchemaCaps, AgentNodeUpdateRecord, decode_agent_node_record,
    delete_agent_node_record, get_agent_node_row, insert_agent_node_record, list_agent_node_rows,
    touch_agent_node_last_seen, update_agent_node_record,
};
use self::start_scheduler::AgentStartScheduler;
pub(crate) use self::start_scheduler::AgentStartSchedulerSettings;
use self::store::{
    AgentInsertRecord, AgentSchemaCaps, RemoteManagedAgentUpsert, decode_agent_record,
    get_agent_row, insert_agent_record, list_agent_rows, upsert_remote_managed_agent_record,
};
use self::supervisor::{AgentProcessSupervisor, SharedSupervisedChild};
use self::worktree::git_command_without_fsmonitor;
use super::event_message_codec::{decode_message_from_storage, persist_agent_event};
use super::{
    AGENT_NODE_MAIN_ID, AgentConfig, AgentEvent, AgentNodeConfig, AgentNodeRecord, AgentOutput,
    AgentRecord, AgentStatus, OutputStream, WorktreeMode, build_main_agent_node_record,
    normalize_target_node_id, validate_agent_node_config_input, validate_agent_node_update_input,
};
use crate::acp::{
    AcpActorSkillContext, AcpHandle, AcpHandleDiagnostics, AcpPermissionReviewDispatcher,
    AcpPermissionService, AcpPromptDeliveryPolicy, AcpPromptImage,
};
use crate::auth::AuthService;
use crate::internal::client::{InternalGrpcMailboxClient, InternalGrpcPeerClientConfig};
use crate::internal::p2p::{
    MembershipView, ResolvedNodeEndpoint, derive_cluster_id,
    resolved_node_endpoint_from_agent_node_record,
};
use crate::path_utils::expand_tilde;
use crate::push::PushService;
use agenthub_config::{
    normalize_codex_acp_mode_id, normalize_optional_codex_acp_mode_id,
    normalize_optional_runtime_model, normalize_optional_thinking_level,
};
use agenthub_db::{AgentEventDbRouter, AgentEventIdleGc};

#[derive(Clone)]
pub struct AgentManager {
    db: SqlitePool,
    event_dbs: AgentEventDbRouter,
    idle_gc: Option<AgentEventIdleGc>,
    push: Arc<PushService>,
    auth: Arc<AuthService>,
    local_executor: Arc<dyn AgentExecutor>,
    process_supervisor: AgentProcessSupervisor,
    start_scheduler: AgentStartScheduler,
    codex_acp_binary: String,
    acp_default_mode: Option<String>,
    codex_acp_multi_agent_enabled: bool,
    permissions: Arc<AcpPermissionService>,
    message_index: Option<crate::message_body_store::SharedIndexStore>,
    read_repair: Option<crate::message_body_store::SharedReadRepairScheduler>,
    permission_review_dispatcher: Arc<StdRwLock<Option<Arc<dyn AcpPermissionReviewDispatcher>>>>,
    internal_peer_client: Option<InternalGrpcPeerClientConfig>,
    starting: Arc<Mutex<HashSet<String>>>,
    inner: Arc<RwLock<HashMap<String, AgentHandle>>>,
}

const ACTOR_RUNTIME_TEAM_ID_ENV: &str = "AGENTHUB_ACTOR_TEAM_ID";
const ACTOR_RUNTIME_CURRENT_RUN_ID_ENV: &str = "AGENTHUB_ACTOR_CURRENT_RUN_ID";
const ACTOR_RUNTIME_ACTOR_ID_ENV: &str = "AGENTHUB_ACTOR_ID";
const ACTOR_RUNTIME_CHANNEL_ENV: &str = "AGENTHUB_ACTOR_CHANNEL";
const AGENT_SOURCE_MANUAL: &str = "manual";
const AGENT_SOURCE_TEAM_FORGE: &str = "team_forge";
const INTERNAL_AGENT_MANAGE_PERMISSION: &str = "agent:manage";
const TEAM_MEMBER_ROLE_COORDINATOR: &str = "coordinator";
const TEAM_MEMBER_ROLE_WORKER: &str = "worker";
const AGENT_LOOP_MESSAGE_ID_PREFIX: &str = "agent-loop:";
const PER_AGENT_EVENT_SOURCE: &str = "per_agent_agent_events";
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AgentSendInputError {
    #[error("agent session mismatch: expected={expected} running={running}")]
    SessionMismatch { expected: String, running: String },
    #[error("image input is only supported by local ACP agents")]
    MultimodalUnsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentInputImage {
    pub file_name: String,
    pub mime_type: String,
    pub data: String,
}

#[derive(Debug, Clone)]
struct AgentLoopConfig {
    idle_seconds: i64,
    prompt: String,
}

#[derive(Debug)]
enum AgentLoopCommand {
    Reconfigure(AgentLoopConfig),
    Stop,
}

#[derive(Debug, Clone)]
struct AgentLoopController {
    tx: mpsc::UnboundedSender<AgentLoopCommand>,
}

impl AgentLoopController {
    fn reconfigure(&self, config: AgentLoopConfig) -> anyhow::Result<()> {
        self.tx
            .send(AgentLoopCommand::Reconfigure(config))
            .map_err(|_| anyhow::anyhow!("agent loop controller is no longer running"))
    }

    fn stop(&self) {
        let _ = self.tx.send(AgentLoopCommand::Stop);
    }
}

fn agent_event_from_row(agent_id: &str, row: &sqlx::sqlite::SqliteRow) -> AgentEvent {
    let stream_str: String = row.get("stream");
    AgentEvent {
        event_id: row.get("id"),
        agent_id: agent_id.to_string(),
        session_id: row.get("session_id"),
        seq: row.get("seq"),
        ts: row.get("ts"),
        stream: stream_from_str(&stream_str),
        // Decode compressed ACP rows while keeping legacy plain rows untouched.
        message: decode_message_from_storage(row.get::<Vec<u8>, _>("message").as_slice()),
    }
}

fn agent_event_id_from_delivery_id<'a>(
    agent_id: &str,
    delivery_id: &'a str,
) -> Option<(&'a str, i64)> {
    let rest = delivery_id.strip_prefix(&format!("agent_event:{agent_id}:"))?;
    let (session_id, event_id) = rest.rsplit_once(':')?;
    Some((session_id, event_id.parse().ok()?))
}

#[derive(Debug, Clone)]
struct RuntimeStartPolicy {
    workdir: String,
    worktree_repo: Option<String>,
    worktree_mode: WorktreeMode,
    worktree_ref: Option<String>,
    worker_branch: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ProxyPolicy {
    env_pairs: Vec<(String, String)>,
}

impl ProxyPolicy {
    fn new(env_pairs: Vec<(String, String)>) -> Self {
        Self { env_pairs }
    }

    fn apply_to_command(&self, command: &mut Command) {
        for (key, value) in &self.env_pairs {
            command.env(key, value);
        }
    }
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

fn acp_prompt_submission_failure_event(message_id: &str) -> String {
    serde_json::json!({
        "type": "agent_message",
        "text": "AgentHub could not submit this prompt to the ACP provider. Check agent-trace provider diagnostics for the redacted command error.",
        "chunk": false,
        "message_id": format!("{message_id}:submission-error"),
        "meta": {
            "source": "agenthub",
            "category": "acp_prompt_submission_failed"
        }
    })
    .to_string()
}

pub(crate) fn derive_worker_runtime_root(workdir: &str) -> String {
    let path = Path::new(workdir);
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| parent.to_string_lossy().to_string())
        .unwrap_or_else(|| workdir.to_string())
}

pub(crate) fn derive_coordinator_runtime_workdir(
    workdir: &str,
    actor_context: &AcpActorSkillContext,
) -> String {
    // Team coordinator continuity should survive ordinary runtime restarts, so the derived
    // coordination workspace is keyed by actor + scope and intentionally excludes the
    // per-launch AgentHub runtime session id.
    let actor_token = compact_token(&actor_context.actor_id, "coordinator", 24);
    let scope_token = actor_context
        .current_run_id
        .as_deref()
        .or(actor_context.team_id.as_deref())
        .map(|value| compact_token(value, "scope", 24))
        .unwrap_or_else(|| "scope".to_string());
    let legacy_path = Path::new(workdir)
        .join(".agenthub-team-leader")
        .join(format!("{actor_token}-{scope_token}"));
    if legacy_path.exists() {
        return legacy_path.to_string_lossy().to_string();
    }
    Path::new(workdir)
        .join(".agenthub-team-coordinator")
        .join(format!("{actor_token}-{scope_token}"))
        .to_string_lossy()
        .to_string()
}

pub(crate) fn derive_team_runtime_workdir(
    workdir: &str,
    actor_context: &AcpActorSkillContext,
    worktree_mode: &WorktreeMode,
) -> String {
    match actor_context.member_role.as_deref() {
        Some(TEAM_MEMBER_ROLE_COORDINATOR) => {
            derive_coordinator_runtime_workdir(workdir, actor_context)
        }
        Some(TEAM_MEMBER_ROLE_WORKER) if matches!(worktree_mode, WorktreeMode::CreateWorktree) => {
            let actor_token = compact_token(&actor_context.actor_id, "worker", 24);
            let scope_token = actor_context
                .current_run_id
                .as_deref()
                .or(actor_context.team_id.as_deref())
                .map(|value| compact_token(value, "scope", 24))
                .unwrap_or_else(|| "scope".to_string());
            let root = derive_worker_runtime_root(workdir);
            Path::new(&root)
                .join(format!("{actor_token}-{scope_token}"))
                .to_string_lossy()
                .to_string()
        }
        _ => workdir.to_string(),
    }
}

fn build_runtime_start_policy(
    agent: &AgentRecord,
    actor_context: Option<&AcpActorSkillContext>,
    expanded_workdir: &str,
    expanded_worktree_repo: Option<&str>,
    _start_session_id: Option<&str>,
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
        TEAM_MEMBER_ROLE_COORDINATOR => {
            if !matches!(policy.worktree_mode, WorktreeMode::UseExisting) {
                anyhow::bail!(
                    "team coordinator policy requires worktree_mode=use_existing (agent_id={})",
                    agent.id
                );
            }
            let context = actor_context
                .ok_or_else(|| anyhow::anyhow!("coordinator role policy requires actor context"))?;
            policy.workdir =
                derive_team_runtime_workdir(expanded_workdir, context, &policy.worktree_mode);
            if Path::new(&policy.workdir).exists() && !Path::new(&policy.workdir).is_dir() {
                anyhow::bail!(
                    "team coordinator policy requires directory workdir (agent_id={} workdir={})",
                    agent.id,
                    policy.workdir
                );
            }
        }
        TEAM_MEMBER_ROLE_WORKER => match policy.worktree_mode {
            WorktreeMode::UseExisting => {}
            WorktreeMode::CreateWorktree => {
                let repo = expanded_worktree_repo.ok_or_else(|| {
                    anyhow::anyhow!(
                        "team worker policy requires worktree_repo when worktree_mode=create_worktree (agent_id={})",
                        agent.id
                    )
                })?;
                let context = actor_context
                    .ok_or_else(|| anyhow::anyhow!("worker role policy requires actor context"))?;
                let actor_token = compact_token(&context.actor_id, "worker", 24);
                // Worker runtime workdirs are stable for the current actor + scope and should
                // not change just because AgentHub generated a new per-launch runtime session id.
                let workdir =
                    derive_team_runtime_workdir(expanded_workdir, context, &policy.worktree_mode);
                let branch = format!("worker-{actor_token}-{}", short_random_token());
                policy.workdir = workdir;
                policy.worktree_repo = Some(repo.to_string());
                policy.worktree_mode = WorktreeMode::CreateWorktree;
                policy.worktree_ref = Some("HEAD".to_string());
                policy.worker_branch = Some(branch);
            }
            WorktreeMode::ReuseWorktree => {
                anyhow::bail!(
                    "team worker policy requires worktree_mode=use_existing or create_worktree (agent_id={})",
                    agent.id
                );
            }
        },
        _ => {}
    }

    Ok(policy)
}

async fn ensure_team_runtime_workspace_layout(
    actor_context: Option<&AcpActorSkillContext>,
    workdir: &str,
) -> anyhow::Result<()> {
    let Some(role) = actor_context.and_then(|context| context.member_role.as_deref()) else {
        return Ok(());
    };
    if role != TEAM_MEMBER_ROLE_COORDINATOR && role != TEAM_MEMBER_ROLE_WORKER {
        return Ok(());
    }

    let workdir_path = Path::new(workdir);
    match tokio::fs::metadata(workdir_path).await {
        Ok(metadata) => {
            if !metadata.is_dir() {
                anyhow::bail!("team runtime workdir is not a directory: workdir={workdir}");
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            if role != TEAM_MEMBER_ROLE_COORDINATOR {
                return Ok(());
            }
            tokio::fs::create_dir_all(workdir_path)
                .await
                .map_err(|create_err| {
                    anyhow::anyhow!(
                        "failed to create team runtime workdir: workdir={} error={}",
                        workdir,
                        create_err
                    )
                })?;
        }
        Err(err) => {
            return Err(anyhow::anyhow!(
                "failed to stat team runtime workdir: workdir={} error={}",
                workdir,
                err
            ));
        }
    }

    let context_root = workdir_path.join(".cache").join("context");
    tokio::fs::create_dir_all(context_root.join("run"))
        .await
        .map_err(|err| {
            anyhow::anyhow!(
                "failed to create team runtime context run dir: workdir={} error={}",
                workdir,
                err
            )
        })?;
    tokio::fs::create_dir_all(context_root.join("memory"))
        .await
        .map_err(|err| {
            anyhow::anyhow!(
                "failed to create team runtime context memory dir: workdir={} error={}",
                workdir,
                err
            )
        })?;

    for relative_path in [
        "state.md",
        "decisions.md",
        "errors.md",
        "log.md",
        "memory/profile.md",
        "memory/project_facts.md",
        "memory/decision_journal.md",
        "memory/open_questions.md",
    ] {
        let path = context_root.join(relative_path);
        match tokio::fs::metadata(&path).await {
            Ok(metadata) => {
                if !metadata.is_file() {
                    anyhow::bail!(
                        "team runtime context path is not a file: path={}",
                        path.display()
                    );
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                tokio::fs::write(&path, "").await.map_err(|write_err| {
                    anyhow::anyhow!(
                        "failed to initialize team runtime context file: path={} error={}",
                        path.display(),
                        write_err
                    )
                })?;
            }
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "failed to stat team runtime context file: path={} error={}",
                    path.display(),
                    err
                ));
            }
        }
    }

    Ok(())
}

fn normalize_agent_loop_config(
    enabled: bool,
    idle_seconds: Option<i64>,
    prompt: Option<&str>,
) -> Option<AgentLoopConfig> {
    if !enabled {
        return None;
    }
    let idle_seconds = idle_seconds.filter(|value| (10..=86_400).contains(value))?;
    let prompt = prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)?;
    Some(AgentLoopConfig {
        idle_seconds,
        prompt,
    })
}

fn is_agent_loop_user_message(message: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(message) else {
        return false;
    };
    value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| value == "user_message")
        && value
            .get("message_id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value.starts_with(AGENT_LOOP_MESSAGE_ID_PREFIX))
}

fn is_agent_loop_activity_output(output: &AgentOutput) -> bool {
    matches!(output.stream, OutputStream::Acp) && !is_agent_loop_user_message(&output.message)
}

fn should_rearm_agent_loop_for_output(session_id: &str, output: &AgentOutput) -> bool {
    output.session_id == session_id && is_agent_loop_activity_output(output)
}

fn acp_accepts_best_effort_hint(
    diagnostics: &AcpHandleDiagnostics,
    prompt_delivery_policy: AcpPromptDeliveryPolicy,
) -> bool {
    let accepts_active_prompt = diagnostics.active_prompt_count == 0
        || matches!(
            prompt_delivery_policy,
            AcpPromptDeliveryPolicy::AllowConcurrentPrompts
        );

    !diagnostics.command_channel_closed
        && diagnostics.command_channel_capacity > 0
        && accepts_active_prompt
        && diagnostics.pending_command_count == 0
        && diagnostics.pending_permission_count == 0
        && diagnostics.pending_tool_call_count == 0
        && diagnostics.stale_prompt.is_none()
}

#[cfg(any(debug_assertions, test))]
fn safe_acp_provider_diagnostics_details(diagnostics: &AcpHandleDiagnostics) -> serde_json::Value {
    serde_json::json!({
        "session_id": &diagnostics.session_id,
        "command_channel_closed": diagnostics.command_channel_closed,
        "command_channel_capacity": diagnostics.command_channel_capacity,
        "command_channel_max_capacity": diagnostics.command_channel_max_capacity,
        "active_prompt_count": diagnostics.active_prompt_count,
        "pending_command_count": diagnostics.pending_command_count,
        "pending_permission_count": diagnostics.pending_permission_count,
        "active_submission_ids": &diagnostics.active_submission_ids,
        "last_submission_id": &diagnostics.last_submission_id,
        "last_provider_event_type": &diagnostics.last_provider_event_type,
        "last_provider_event_at": diagnostics.last_provider_event_at,
        "pending_tool_call_count": diagnostics.pending_tool_call_count,
        "pending_tool_calls": diagnostics
            .pending_tool_calls
            .iter()
            .map(|tool_call| {
                serde_json::json!({
                    "tool_call_id": &tool_call.tool_call_id,
                    "status": &tool_call.status,
                    "updated_at": tool_call.updated_at,
                })
            })
            .collect::<Vec<_>>(),
        "stale_prompt": diagnostics.stale_prompt.as_ref().map(|stale| {
            serde_json::json!({
                "active_prompt_count": stale.active_prompt_count,
                "pending_permission_count": stale.pending_permission_count,
                "stale_for_seconds": stale.stale_for_seconds,
                "last_activity_at": stale.last_activity_at,
                "active_submission_ids": &stale.active_submission_ids,
            })
        }),
        "last_command_error": diagnostics.last_command_error.as_ref().map(|error| {
            serde_json::json!({
                "command_kind": &error.command_kind,
            })
        }),
        "last_command_error_at": diagnostics.last_command_error_at,
    })
}

fn is_agent_user_message(message: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(message)
        .ok()
        .and_then(|value| value.get("type").cloned())
        .and_then(|value| value.as_str().map(str::to_string))
        .is_some_and(|value| value == "user_message")
}

async fn emit_agent_loop_prompt(
    event_dbs: &AgentEventDbRouter,
    idle_gc: Option<&AgentEventIdleGc>,
    output_tx: &broadcast::Sender<AgentOutput>,
    acp: &AcpHandle,
    agent_id: &str,
    session_id: &str,
    prompt: &str,
) -> anyhow::Result<()> {
    let seq = Uuid::now_v7().to_string();
    let message_id = format!("{AGENT_LOOP_MESSAGE_ID_PREFIX}{seq}");
    let message = serde_json::json!({
        "type": "user_message",
        "text": prompt,
        "chunk": false,
        "message_id": message_id
    })
    .to_string();
    let ts = Utc::now().timestamp();
    let event_id = persist_agent_event(
        event_dbs,
        idle_gc,
        agent_id,
        session_id,
        &seq,
        ts,
        &OutputStream::Acp,
        &message,
    )
    .await?;
    let _ = output_tx.send(AgentOutput {
        event_id,
        agent_id: agent_id.to_string(),
        session_id: session_id.to_string(),
        seq,
        ts,
        stream: OutputStream::Acp,
        message,
    });
    acp.prompt(prompt.to_string()).await?;
    Ok(())
}

fn spawn_agent_loop_controller(
    event_dbs: AgentEventDbRouter,
    idle_gc: Option<AgentEventIdleGc>,
    output_tx: broadcast::Sender<AgentOutput>,
    acp: AcpHandle,
    agent_id: String,
    session_id: String,
    initial: AgentLoopConfig,
) -> AgentLoopController {
    let (tx, mut rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut config = initial;
        let mut output_rx = output_tx.subscribe();
        let mut deadline =
            tokio::time::Instant::now() + Duration::from_secs(config.idle_seconds as u64);
        let mut injected_for_current_idle = false;

        loop {
            tokio::select! {
                command = rx.recv() => match command {
                    Some(AgentLoopCommand::Reconfigure(next)) => {
                        config = next;
                        deadline = tokio::time::Instant::now() + Duration::from_secs(config.idle_seconds as u64);
                        injected_for_current_idle = false;
                    }
                    Some(AgentLoopCommand::Stop) | None => break,
                },
                event = output_rx.recv() => match event {
                    Ok(output) => {
                        if !should_rearm_agent_loop_for_output(session_id.as_str(), &output) {
                            continue;
                        }
                        deadline = tokio::time::Instant::now() + Duration::from_secs(config.idle_seconds as u64);
                        injected_for_current_idle = false;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        deadline = tokio::time::Instant::now() + Duration::from_secs(config.idle_seconds as u64);
                        injected_for_current_idle = false;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                },
                _ = tokio::time::sleep_until(deadline), if !injected_for_current_idle => {
                    if let Err(err) = emit_agent_loop_prompt(
                        &event_dbs,
                        idle_gc.as_ref(),
                        &output_tx,
                        &acp,
                        &agent_id,
                        &session_id,
                        &config.prompt,
                    ).await {
                        tracing::warn!(
                            agent_id = %agent_id,
                            session_id = %session_id,
                            error = %err,
                            "agent loop prompt injection failed"
                        );
                    }
                    injected_for_current_idle = true;
                }
            }
        }
    });
    AgentLoopController { tx }
}

pub struct AgentHandle {
    child: SharedSupervisedChild,
    output_tx: broadcast::Sender<AgentOutput>,
    input: AgentInput,
    session_id: String,
    actor_context: Option<AcpActorSkillContext>,
    acp_prompt_delivery_policy: Option<AcpPromptDeliveryPolicy>,
    loop_controller: Option<AgentLoopController>,
}

pub enum AgentInput {
    Stdin(Arc<Mutex<Option<ChildStdin>>>),
    Acp(AcpHandle),
}

struct AgentManagerMembershipView<'a> {
    manager: &'a AgentManager,
    cluster_id: String,
}

#[async_trait]
impl MembershipView for AgentManagerMembershipView<'_> {
    async fn resolve_node(&self, node_id: &str) -> anyhow::Result<ResolvedNodeEndpoint> {
        let normalized = node_id.trim();
        if normalized.is_empty() || normalized == AGENT_NODE_MAIN_ID {
            return Ok(resolved_node_endpoint_from_agent_node_record(
                &self.cluster_id,
                build_main_agent_node_record(),
            ));
        }
        let record = self.manager.get_agent_node(normalized).await?;
        Ok(resolved_node_endpoint_from_agent_node_record(
            &self.cluster_id,
            record,
        ))
    }
}

impl AgentManager {
    #[cfg(debug_assertions)]
    pub(crate) fn event_db_base_dir(&self) -> &std::path::Path {
        self.event_dbs.base_dir()
    }

    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: SqlitePool,
        event_dbs: AgentEventDbRouter,
        idle_gc: Option<AgentEventIdleGc>,
        push: Arc<PushService>,
        proxy_env: Vec<(String, String)>,
        codex_acp_binary: String,
        acp_default_mode: Option<String>,
        codex_acp_multi_agent_enabled: bool,
        permissions: Arc<AcpPermissionService>,
        auth: Arc<AuthService>,
    ) -> Self {
        Self::new_with_internal_grpc(
            db,
            event_dbs,
            idle_gc,
            push,
            proxy_env,
            codex_acp_binary,
            acp_default_mode,
            codex_acp_multi_agent_enabled,
            permissions,
            auth,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_internal_grpc(
        db: SqlitePool,
        event_dbs: AgentEventDbRouter,
        idle_gc: Option<AgentEventIdleGc>,
        push: Arc<PushService>,
        proxy_env: Vec<(String, String)>,
        codex_acp_binary: String,
        acp_default_mode: Option<String>,
        codex_acp_multi_agent_enabled: bool,
        permissions: Arc<AcpPermissionService>,
        auth: Arc<AuthService>,
        internal_peer_client: Option<InternalGrpcPeerClientConfig>,
    ) -> Self {
        let process_supervisor = AgentProcessSupervisor::default();
        Self {
            db,
            event_dbs,
            idle_gc,
            push,
            auth,
            local_executor: Arc::new(LocalExecutor::new(
                ProxyPolicy::new(proxy_env),
                process_supervisor.clone(),
            )),
            process_supervisor,
            start_scheduler: AgentStartScheduler::default(),
            codex_acp_binary,
            acp_default_mode,
            codex_acp_multi_agent_enabled,
            permissions,
            message_index: None,
            read_repair: None,
            internal_peer_client,
            permission_review_dispatcher: Arc::new(StdRwLock::new(None)),
            starting: Arc::new(Mutex::new(HashSet::new())),
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub(crate) fn with_start_scheduler_settings(
        mut self,
        settings: AgentStartSchedulerSettings,
    ) -> Self {
        self.start_scheduler = AgentStartScheduler::new(settings);
        self
    }

    /// Attach the rebuildable message index store. Agent event reads use it only when the per-agent
    /// projection is fresh through SQLite authority and fall back to SQLite otherwise.
    pub fn with_message_index(
        mut self,
        message_index: Option<crate::message_body_store::SharedIndexStore>,
    ) -> Self {
        self.message_index = message_index;
        self
    }

    /// Attach a scheduler for lagging index projections discovered by guarded reads.
    pub fn with_read_repair_scheduler(
        mut self,
        read_repair: Option<crate::message_body_store::SharedReadRepairScheduler>,
    ) -> Self {
        self.read_repair = read_repair;
        self
    }

    pub fn set_permission_review_dispatcher(
        &self,
        dispatcher: Option<Arc<dyn AcpPermissionReviewDispatcher>>,
    ) {
        if let Ok(mut guard) = self.permission_review_dispatcher.write() {
            *guard = dispatcher;
        }
    }

    fn schedule_index_read_repair(
        &self,
        stream_id: impl Into<String>,
        authority_max: u64,
        freshness: agenthub_message_store::IndexFreshness,
    ) {
        let agenthub_message_store::IndexFreshness::Lagging {
            indexed_through, ..
        } = freshness
        else {
            return;
        };
        let Some(scheduler) = self.read_repair.as_deref() else {
            return;
        };
        self.schedule_index_read_repair_request(
            scheduler,
            stream_id.into(),
            authority_max,
            agenthub_message_store::IndexReadRepairReason::Lagging { indexed_through },
        );
    }

    fn schedule_incomplete_index_repair(
        &self,
        stream_id: impl Into<String>,
        authority_max: u64,
        indexed_through: u64,
    ) {
        let Some(scheduler) = self.read_repair.as_deref() else {
            return;
        };
        self.schedule_index_read_repair_request(
            scheduler,
            stream_id.into(),
            authority_max,
            agenthub_message_store::IndexReadRepairReason::Incomplete { indexed_through },
        );
    }

    fn schedule_index_read_repair_request(
        &self,
        scheduler: &dyn crate::message_body_store::IndexReadRepairScheduler,
        stream_id: String,
        authority_max: u64,
        reason: agenthub_message_store::IndexReadRepairReason,
    ) {
        if let Err(error) =
            scheduler.schedule_read_repair(agenthub_message_store::IndexReadRepairRequest {
                stream_id: stream_id.clone(),
                authority_max,
                reason,
            })
        {
            tracing::warn!(
                ?error,
                stream_id,
                authority_max,
                "failed to schedule agent event index read repair"
            );
        }
    }

    async fn has_agent_nodes_table(&self) -> anyhow::Result<bool> {
        Ok(self.agent_node_schema_caps().await?.has_agent_nodes_table)
    }

    async fn has_agents_target_node_id_column(&self) -> anyhow::Result<bool> {
        Ok(self.agent_schema_caps().await?.has_target_node_id_column)
    }

    async fn has_agent_persistent_sessions_table(&self) -> anyhow::Result<bool> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM sqlite_master
            WHERE type = 'table' AND name = 'agent_persistent_sessions'
            "#,
        )
        .fetch_one(&self.db)
        .await?;
        Ok(count > 0)
    }

    async fn has_agent_persistent_session_failures_table(&self) -> anyhow::Result<bool> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM sqlite_master
            WHERE type = 'table' AND name = 'agent_persistent_session_failures'
            "#,
        )
        .fetch_one(&self.db)
        .await?;
        Ok(count > 0)
    }

    async fn validate_target_node_id(&self, raw: Option<&str>) -> anyhow::Result<Option<String>> {
        let normalized = normalize_target_node_id(raw);
        let Some(node_id) = normalized.as_deref() else {
            return Ok(None);
        };
        if !self.has_agent_nodes_table().await? {
            anyhow::bail!("agent node '{}' not found", node_id);
        }
        let exists: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM agent_nodes
            WHERE id = ?1
            "#,
        )
        .bind(node_id)
        .fetch_one(&self.db)
        .await?;
        if exists == 0 {
            anyhow::bail!("agent node '{}' not found", node_id);
        }
        Ok(Some(node_id.to_string()))
    }

    fn ensure_remote_agent_control_available(
        &self,
        target_node_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let Some(target_node_id) = target_node_id else {
            return Ok(());
        };
        if self.internal_peer_client.is_none() {
            anyhow::bail!(
                "remote-target agents require internal gRPC peer config; cannot target agent node '{}'",
                target_node_id
            );
        }
        Ok(())
    }

    fn ensure_remote_target_persistable(
        target_node_id: Option<&str>,
        has_target_node_id_column: bool,
    ) -> anyhow::Result<()> {
        let Some(target_node_id) = target_node_id else {
            return Ok(());
        };
        if !has_target_node_id_column {
            anyhow::bail!(
                "remote-target agents require agents.target_node_id column; cannot target agent node '{}' on a legacy schema",
                target_node_id
            );
        }
        Ok(())
    }

    async fn agent_schema_caps(&self) -> anyhow::Result<AgentSchemaCaps> {
        AgentSchemaCaps::load(&self.db).await
    }

    async fn agent_node_schema_caps(&self) -> anyhow::Result<AgentNodeSchemaCaps> {
        AgentNodeSchemaCaps::load(&self.db).await
    }

    async fn agent_source_for(&self, agent_id: &str) -> anyhow::Result<String> {
        if !self.has_agents_source_column().await? {
            return Ok(AGENT_SOURCE_MANUAL.to_string());
        }
        let source = sqlx::query_scalar::<_, Option<String>>(
            r#"
            SELECT source
            FROM agents
            WHERE id = ?1
            "#,
        )
        .bind(agent_id)
        .fetch_optional(&self.db)
        .await?
        .flatten()
        .unwrap_or_else(|| AGENT_SOURCE_MANUAL.to_string());
        Ok(source)
    }

    async fn remote_control_client_for_target_node(
        &self,
        target_node_id: &str,
    ) -> anyhow::Result<InternalGrpcMailboxClient> {
        let peer_config = self.internal_peer_client.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "remote agent control is unavailable because internal gRPC peer config is missing"
            )
        })?;
        let membership = AgentManagerMembershipView {
            manager: self,
            cluster_id: derive_cluster_id(peer_config.expected_issuer.as_deref()),
        };
        let node = membership.resolve_node(target_node_id).await?;
        let grpc_target = node
            .grpc_target
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "agent node '{}' does not have a valid gRPC target",
                    target_node_id
                )
            })?;
        InternalGrpcMailboxClient::connect_peer(
            peer_config,
            grpc_target,
            node.tls_server_name.as_deref(),
            vec![INTERNAL_AGENT_MANAGE_PERMISSION.to_string()],
        )
        .await
    }

    fn build_remote_managed_agent_config(agent: &AgentRecord) -> AgentConfig {
        AgentConfig {
            name: agent.name.clone(),
            workdir: agent.workdir.clone(),
            command: agent.command.clone(),
            args: agent.args.clone(),
            target_node_id: None,
            worktree_mode: agent.worktree_mode.clone(),
            worktree_repo: agent.worktree_repo.clone(),
            worktree_ref: agent.worktree_ref.clone(),
            code_mode: agent.code_mode,
            codex_acp_default_mode: agent.codex_acp_default_mode.clone(),
            runtime_model: agent.runtime_model.clone(),
            thinking_level: agent.thinking_level.clone(),
            agent_loop_enabled: agent.agent_loop_enabled,
            agent_loop_idle_seconds: agent.agent_loop_idle_seconds,
            agent_loop_prompt: agent.agent_loop_prompt.clone(),
        }
    }

    async fn ensure_remote_agent_synced(
        &self,
        client: &InternalGrpcMailboxClient,
        agent: &AgentRecord,
    ) -> anyhow::Result<AgentRecord> {
        let source = self.agent_source_for(&agent.id).await?;
        client
            .ensure_agent_record(
                &agent.id,
                &Self::build_remote_managed_agent_config(agent),
                &source,
            )
            .await
    }

    async fn start_remote_agent(
        &self,
        agent: &AgentRecord,
        target_node_id: &str,
        actor_context: Option<&AcpActorSkillContext>,
    ) -> anyhow::Result<String> {
        let client = self
            .remote_control_client_for_target_node(target_node_id)
            .await?;
        self.ensure_remote_agent_synced(&client, agent).await?;
        let session_id = client.start_managed_agent(&agent.id, actor_context).await?;
        self.update_agent_status(&agent.id, AgentStatus::Running)
            .await?;
        Ok(session_id)
    }

    pub async fn create_agent_node(
        &self,
        config: AgentNodeConfig,
    ) -> anyhow::Result<AgentNodeRecord> {
        let schema_caps = self.agent_node_schema_caps().await?;
        let validated = validate_agent_node_config_input(&config)?;
        let now = Utc::now().timestamp();
        insert_agent_node_record(
            &self.db,
            schema_caps,
            AgentNodeInsertRecord {
                id: &validated.id,
                name: &validated.name,
                grpc_target: &validated.grpc_target,
                tls_server_name: validated.tls_server_name.as_deref(),
                default_worktree_root: validated.default_worktree_root.as_deref(),
                group_id: validated.group_id.as_deref(),
                now,
            },
        )
        .await
    }

    pub async fn update_agent_node(
        &self,
        node_id: &str,
        config: crate::agent::AgentNodeUpdate,
    ) -> anyhow::Result<AgentNodeRecord> {
        let normalized = node_id.trim();
        if normalized.is_empty() || normalized == AGENT_NODE_MAIN_ID {
            anyhow::bail!(
                "agent node '{}' is reserved and cannot be updated",
                AGENT_NODE_MAIN_ID
            );
        }
        let schema_caps = self.agent_node_schema_caps().await?;
        let validated = validate_agent_node_update_input(&config)?;
        let now = Utc::now().timestamp();
        update_agent_node_record(
            &self.db,
            schema_caps,
            AgentNodeUpdateRecord {
                node_id: normalized,
                name: &validated.name,
                grpc_target: &validated.grpc_target,
                tls_server_name: validated.tls_server_name.as_deref(),
                default_worktree_root: validated.default_worktree_root.as_deref(),
                group_id: validated.group_id.as_deref(),
                now,
            },
        )
        .await
    }

    pub async fn list_agent_nodes(&self) -> anyhow::Result<Vec<AgentNodeRecord>> {
        let mut nodes = vec![build_main_agent_node_record()];
        let schema_caps = self.agent_node_schema_caps().await?;
        if !schema_caps.has_agent_nodes_table {
            return Ok(nodes);
        }
        let rows = list_agent_node_rows(&self.db, schema_caps).await?;
        for row in rows {
            nodes.push(decode_agent_node_record(&row, schema_caps));
        }
        Ok(nodes)
    }

    pub async fn get_agent_node(&self, node_id: &str) -> anyhow::Result<AgentNodeRecord> {
        let normalized = node_id.trim();
        if normalized.is_empty() || normalized == AGENT_NODE_MAIN_ID {
            return Ok(build_main_agent_node_record());
        }
        let schema_caps = self.agent_node_schema_caps().await?;
        let row = get_agent_node_row(&self.db, schema_caps, normalized).await?;
        Ok(decode_agent_node_record(&row, schema_caps))
    }

    pub async fn touch_agent_node_last_seen(&self, node_id: &str) -> anyhow::Result<bool> {
        let normalized = node_id.trim();
        if normalized.is_empty() || normalized == AGENT_NODE_MAIN_ID {
            return Ok(false);
        }
        let schema_caps = self.agent_node_schema_caps().await?;
        let now = Utc::now().timestamp();
        touch_agent_node_last_seen(&self.db, schema_caps, normalized, now).await
    }

    pub async fn delete_agent_node(&self, node_id: &str) -> anyhow::Result<()> {
        let normalized = node_id.trim();
        if normalized.is_empty() || normalized == AGENT_NODE_MAIN_ID {
            anyhow::bail!(
                "agent node '{}' is reserved and cannot be deleted",
                AGENT_NODE_MAIN_ID
            );
        }
        if self.has_agents_target_node_id_column().await? {
            let bound_agents: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM agents
                WHERE target_node_id = ?1
                "#,
            )
            .bind(normalized)
            .fetch_one(&self.db)
            .await?;
            if bound_agents > 0 {
                anyhow::bail!(
                    "agent node '{}' is still referenced by {} agent(s)",
                    normalized,
                    bound_agents
                );
            }
        }
        if delete_agent_node_record(&self.db, normalized).await? == 0 {
            anyhow::bail!("agent node '{}' not found", normalized);
        }
        Ok(())
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
        let target_node_id = self
            .validate_target_node_id(config.target_node_id.as_deref())
            .await?;
        let is_local_target = target_node_id.is_none();
        let workdir = if is_local_target {
            expand_tilde(&config.workdir)
        } else {
            config.workdir.trim().to_string()
        };
        let worktree_repo = config.worktree_repo.as_deref().map(|path| {
            if is_local_target {
                expand_tilde(path)
            } else {
                path.trim().to_string()
            }
        });
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        let args_json = serde_json::to_string(&config.args)?;
        let status = AgentStatus::Created;
        let schema_caps = self.agent_schema_caps().await?;
        self.ensure_remote_agent_control_available(target_node_id.as_deref())?;
        Self::ensure_remote_target_persistable(
            target_node_id.as_deref(),
            schema_caps.has_target_node_id_column,
        )?;
        insert_agent_record(
            &self.db,
            schema_caps,
            AgentInsertRecord {
                id: &id,
                config: &config,
                workdir: &workdir,
                args_json: &args_json,
                target_node_id: target_node_id.as_deref(),
                worktree_repo: worktree_repo.as_deref(),
                source,
                status: &status,
                now,
            },
        )
        .await?;

        Ok(AgentRecord {
            id,
            name: config.name,
            workdir,
            command: config.command,
            args: config.args,
            target_node_id,
            worktree_mode: config.worktree_mode,
            worktree_repo,
            worktree_ref: config.worktree_ref,
            code_mode: config.code_mode,
            codex_acp_default_mode: normalize_optional_codex_acp_mode_id(
                config.codex_acp_default_mode.as_deref(),
            ),
            runtime_model: normalize_optional_runtime_model(config.runtime_model.as_deref()),
            thinking_level: normalize_optional_thinking_level(config.thinking_level.as_deref()),
            agent_loop_enabled: config.agent_loop_enabled,
            agent_loop_idle_seconds: config.agent_loop_idle_seconds,
            agent_loop_prompt: config.agent_loop_prompt.and_then(|value| {
                let trimmed = value.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            }),
            status,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn ensure_remote_managed_agent(
        &self,
        agent_id: &str,
        config: AgentConfig,
        source: &str,
    ) -> anyhow::Result<AgentRecord> {
        if source != AGENT_SOURCE_MANUAL && source != AGENT_SOURCE_TEAM_FORGE {
            return Err(anyhow::anyhow!("invalid agent source: {source}"));
        }
        if normalize_target_node_id(config.target_node_id.as_deref()).is_some() {
            anyhow::bail!("remote managed agent config cannot target another agent node");
        }

        let workdir = expand_tilde(&config.workdir);
        let worktree_repo = config.worktree_repo.as_deref().map(expand_tilde);
        let args_json = serde_json::to_string(&config.args)?;
        let now = Utc::now().timestamp();
        let existing = self.get_agent(agent_id).await.ok();
        let schema_caps = self.agent_schema_caps().await?;
        upsert_remote_managed_agent_record(
            &self.db,
            schema_caps,
            RemoteManagedAgentUpsert {
                agent_id,
                config: &config,
                workdir: &workdir,
                args_json: &args_json,
                worktree_repo: worktree_repo.as_deref(),
                source,
                exists: existing.is_some(),
                now,
            },
        )
        .await?;

        self.get_agent(agent_id).await
    }

    pub async fn list_agents(&self) -> anyhow::Result<Vec<AgentRecord>> {
        self.reconcile_stale_running_agents().await?;
        let active_team_member_agents = self.list_active_team_member_agents().await?;
        let schema_caps = self.agent_schema_caps().await?;
        let rows = list_agent_rows(
            &self.db,
            schema_caps,
            schema_caps
                .has_source_column
                .then_some(AGENT_SOURCE_TEAM_FORGE),
        )
        .await?;

        let mut agents = Vec::with_capacity(rows.len());
        for row in rows {
            let agent = decode_agent_record(&row)?;
            if active_team_member_agents.contains(&agent.id) {
                continue;
            }
            agents.push(agent);
        }
        Ok(agents)
    }

    pub async fn reconcile_runtime_absence(&self, agent_id: &str) -> anyhow::Result<bool> {
        let requested_ids = [agent_id.to_string()];
        let reconciled = self.reconcile_runtime_absence_batch(&requested_ids).await?;
        Ok(!reconciled.is_empty())
    }

    pub async fn reconcile_runtime_absence_batch(
        &self,
        agent_ids: &[String],
    ) -> anyhow::Result<Vec<String>> {
        if agent_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut requested_ids = Vec::with_capacity(agent_ids.len());
        let mut seen = HashSet::with_capacity(agent_ids.len());
        for agent_id in agent_ids {
            let trimmed = agent_id.trim();
            if trimmed.is_empty() {
                continue;
            }
            if seen.insert(trimmed.to_string()) {
                requested_ids.push(trimmed.to_string());
            }
        }
        let local_running_ids = self
            .load_local_running_agent_ids(Some(&requested_ids))
            .await?;
        self.reconcile_running_agents_without_runtime_handles(local_running_ids)
            .await
    }

    async fn reconcile_stale_running_agents(&self) -> anyhow::Result<()> {
        let running_ids = self.load_local_running_agent_ids(None).await?;
        let _ = self
            .reconcile_running_agents_without_runtime_handles(running_ids)
            .await?;
        Ok(())
    }

    async fn load_local_running_agent_ids(
        &self,
        requested_ids: Option<&[String]>,
    ) -> anyhow::Result<Vec<String>> {
        if requested_ids.is_some_and(|ids| ids.is_empty()) {
            return Ok(Vec::new());
        }

        let schema_caps = self.agent_schema_caps().await?;
        let mut builder =
            QueryBuilder::<Sqlite>::new("SELECT id FROM agents WHERE status = 'running'");
        if schema_caps.has_target_node_id_column {
            builder.push(
                r#"
                AND (
                    target_node_id IS NULL
                    OR trim(target_node_id) = ''
                    OR trim(target_node_id) = 
                "#,
            );
            builder.push_bind(AGENT_NODE_MAIN_ID);
            builder.push(")");
        }
        if let Some(requested_ids) = requested_ids {
            builder.push(" AND id IN (");
            let mut separated = builder.separated(", ");
            for agent_id in requested_ids {
                separated.push_bind(agent_id.as_str());
            }
            separated.push_unseparated(")");
        }
        let rows = builder.build().fetch_all(&self.db).await?;
        let matched_ids = rows
            .into_iter()
            .map(|row| row.get::<String, _>("id"))
            .collect::<HashSet<_>>();

        if let Some(requested_ids) = requested_ids {
            return Ok(requested_ids
                .iter()
                .filter(|agent_id| matched_ids.contains(agent_id.as_str()))
                .cloned()
                .collect());
        }

        Ok(matched_ids.into_iter().collect())
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
        let mut session_builder = QueryBuilder::<Sqlite>::new(
            r#"
            UPDATE agent_sessions
            SET status = 'exited', ended_at = 
            "#,
        );
        session_builder.push_bind(now);
        session_builder.push(
            r#"
            WHERE status = 'running'
              AND ended_at IS NULL
              AND agent_id IN (
            "#,
        );
        let mut session_ids = session_builder.separated(", ");
        for agent_id in &stale_ids {
            session_ids.push_bind(agent_id.as_str());
        }
        session_ids.push_unseparated(")");
        session_builder.build().execute(&mut *tx).await?;

        let mut agent_builder = QueryBuilder::<Sqlite>::new(
            r#"
            UPDATE agents
            SET status = 'exited', updated_at = 
            "#,
        );
        agent_builder.push_bind(now);
        agent_builder.push(
            r#"
            WHERE status = 'running'
              AND id IN (
            "#,
        );
        let mut agent_ids = agent_builder.separated(", ");
        for agent_id in &stale_ids {
            agent_ids.push_bind(agent_id.as_str());
        }
        agent_ids.push_unseparated(")");
        agent_builder.build().execute(&mut *tx).await?;
        tx.commit().await?;

        tracing::warn!(
            stale_agent_count = stale_ids.len(),
            stale_agent_ids = ?stale_ids,
            "reconciled stale running agents without runtime handles"
        );
        Ok(stale_ids)
    }

    async fn has_agents_source_column(&self) -> anyhow::Result<bool> {
        Ok(self.agent_schema_caps().await?.has_source_column)
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
        let row = get_agent_row(&self.db, self.agent_schema_caps().await?, agent_id).await?;
        decode_agent_record(&row)
    }

    pub async fn list_events(
        &self,
        agent_id: &str,
        limit: i64,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<AgentEvent>> {
        let agent = self.get_agent(agent_id).await?;
        if let Some(target_node_id) = agent.target_node_id.as_deref() {
            let client = self
                .remote_control_client_for_target_node(target_node_id)
                .await?;
            return client
                .list_agent_events(agent_id, limit, None, before_id)
                .await;
        }
        let event_db = self.event_dbs.pool_for_agent(agent_id).await?;
        match self
            .try_list_agent_events_from_index(agent_id, &event_db, limit, None, before_id)
            .await
        {
            Ok(Some(events)) => return Ok(events),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    ?error,
                    agent_id,
                    "falling back to SQLite after agent event index read failed"
                );
            }
        }
        self.list_agent_events_from_sqlite(agent_id, &event_db, limit, None, before_id)
            .await
    }

    async fn list_agent_events_from_sqlite(
        &self,
        agent_id: &str,
        event_db: &SqlitePool,
        limit: i64,
        session_id: Option<&str>,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<AgentEvent>> {
        let rows = if let Some(before_id) = before_id {
            if let Some(session_id) = session_id {
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
                .fetch_all(event_db)
                .await?
            } else {
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
                .fetch_all(event_db)
                .await?
            }
        } else if let Some(session_id) = session_id {
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
            .fetch_all(event_db)
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
            .fetch_all(event_db)
            .await?
        };

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            events.push(agent_event_from_row(agent_id, &row));
        }
        events.reverse();
        Ok(events)
    }

    async fn try_list_agent_events_from_index(
        &self,
        agent_id: &str,
        event_db: &SqlitePool,
        limit: i64,
        session_id: Option<&str>,
        before_id: Option<i64>,
    ) -> anyhow::Result<Option<Vec<AgentEvent>>> {
        let Some(index) = self.message_index.as_deref() else {
            return Ok(None);
        };

        let authority_max = self
            .max_agent_event_id_for_page(event_db, session_id, before_id)
            .await?;
        if authority_max == 0 {
            return Ok(Some(Vec::new()));
        }
        let stream_id = format!("agent_events:agent:{agent_id}");
        let freshness =
            agenthub_message_store::check_index_freshness(index, &stream_id, authority_max as u64)?;
        if !freshness.is_fresh() {
            self.schedule_index_read_repair(stream_id, authority_max as u64, freshness);
            return Ok(None);
        }

        let refs = index.scan_prefix(&agenthub_message_store::keys::agent_prefix(agent_id))?;
        let mut ids = Vec::new();
        let limit = limit.max(1) as usize;
        for message_ref in refs {
            if message_ref.source_kind != PER_AGENT_EVENT_SOURCE
                || message_ref.agent_id.as_deref() != Some(agent_id)
            {
                continue;
            }
            let Some((delivery_session_id, event_id)) =
                agent_event_id_from_delivery_id(agent_id, message_ref.message_id.as_str())
            else {
                self.schedule_incomplete_index_repair(
                    stream_id.clone(),
                    authority_max as u64,
                    authority_max as u64,
                );
                return Ok(None);
            };
            if session_id.is_some_and(|session_id| session_id != delivery_session_id) {
                continue;
            }
            if before_id.is_some_and(|before_id| event_id >= before_id) {
                continue;
            }
            ids.push(event_id);
        }
        if ids.len() > limit {
            ids.drain(0..ids.len() - limit);
        }
        if ids
            != self
                .expected_agent_event_ids(event_db, limit, session_id, before_id)
                .await?
        {
            self.schedule_incomplete_index_repair(
                stream_id,
                authority_max as u64,
                authority_max as u64,
            );
            return Ok(None);
        }

        self.load_agent_events_by_ids(agent_id, event_db, &ids)
            .await
            .map(Some)
    }

    async fn max_agent_event_id_for_page(
        &self,
        event_db: &SqlitePool,
        session_id: Option<&str>,
        before_id: Option<i64>,
    ) -> anyhow::Result<i64> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT COALESCE(MAX(id), 0) FROM agent_events WHERE 1 = 1",
        );
        if let Some(session_id) = session_id {
            builder.push(" AND session_id = ");
            builder.push_bind(session_id);
        }
        if let Some(before_id) = before_id {
            builder.push(" AND id < ");
            builder.push_bind(before_id);
        }
        Ok(builder.build_query_scalar().fetch_one(event_db).await?)
    }

    async fn expected_agent_event_ids(
        &self,
        event_db: &SqlitePool,
        limit: usize,
        session_id: Option<&str>,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<i64>> {
        let mut builder = QueryBuilder::<Sqlite>::new("SELECT id FROM agent_events WHERE 1 = 1");
        if let Some(session_id) = session_id {
            builder.push(" AND session_id = ");
            builder.push_bind(session_id);
        }
        if let Some(before_id) = before_id {
            builder.push(" AND id < ");
            builder.push_bind(before_id);
        }
        builder.push(" ORDER BY id DESC LIMIT ");
        builder.push_bind(limit as i64);
        let mut ids = builder.build_query_scalar().fetch_all(event_db).await?;
        ids.reverse();
        Ok(ids)
    }

    async fn load_agent_events_by_ids(
        &self,
        agent_id: &str,
        event_db: &SqlitePool,
        ids: &[i64],
    ) -> anyhow::Result<Vec<AgentEvent>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT id, session_id, seq, ts, stream, message FROM agent_events WHERE id IN (",
        );
        let mut separated = builder.separated(", ");
        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");

        let rows = builder.build().fetch_all(event_db).await?;
        if rows.len() != ids.len() {
            anyhow::bail!(
                "agent event index referenced {} rows for agent {}, but SQLite hydrated {}",
                ids.len(),
                agent_id,
                rows.len()
            );
        }

        let mut by_id = HashMap::with_capacity(rows.len());
        for row in rows {
            let event = agent_event_from_row(agent_id, &row);
            by_id.insert(event.event_id, event);
        }

        let mut events = Vec::with_capacity(ids.len());
        for id in ids {
            let Some(event) = by_id.remove(id) else {
                anyhow::bail!("agent event index referenced row {id} that SQLite did not return");
            };
            events.push(event);
        }
        Ok(events)
    }

    pub async fn get_event(&self, agent_id: &str, event_id: i64) -> anyhow::Result<AgentEvent> {
        let agent = self.get_agent(agent_id).await?;
        if let Some(target_node_id) = agent.target_node_id.as_deref() {
            let client = self
                .remote_control_client_for_target_node(target_node_id)
                .await?;
            return client
                .list_agent_events(agent_id, 1, None, Some(event_id.saturating_add(1)))
                .await?
                .into_iter()
                .find(|event| event.event_id == event_id)
                .ok_or_else(|| anyhow::anyhow!("agent event not found"));
        }
        let event_db = self.event_dbs.pool_for_agent(agent_id).await?;
        let row = sqlx::query(
            r#"
            SELECT id, session_id, seq, ts, stream, message
            FROM agent_events
            WHERE id = ?1
            LIMIT 1
            "#,
        )
        .bind(event_id)
        .fetch_optional(&event_db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("agent event not found"))?;
        let stream_str: String = row.get("stream");
        Ok(AgentEvent {
            event_id: row.get("id"),
            agent_id: agent_id.to_string(),
            session_id: row.get("session_id"),
            seq: row.get("seq"),
            ts: row.get("ts"),
            stream: stream_from_str(&stream_str),
            message: decode_message_from_storage(row.get::<Vec<u8>, _>("message").as_slice()),
        })
    }

    #[cfg(test)]
    pub(crate) async fn test_event_pool_for_agent(
        &self,
        agent_id: &str,
    ) -> anyhow::Result<SqlitePool> {
        self.event_dbs.pool_for_agent(agent_id).await
    }

    #[cfg(test)]
    pub(crate) fn test_event_db_path_for_agent(&self, agent_id: &str) -> std::path::PathBuf {
        self.event_dbs.db_path_for_agent(agent_id)
    }

    async fn record_agent_activity(&self, agent_id: &str) {
        if let Some(idle_gc) = &self.idle_gc {
            idle_gc.record_activity(agent_id).await;
        }
    }

    pub async fn mailbox_idle_anchor_ts(
        &self,
        agent_id: &str,
        session_id: &str,
    ) -> anyhow::Result<Option<i64>> {
        let session = sqlx::query(
            r#"
            SELECT status, started_at
            FROM agent_sessions
            WHERE id = ?1
            LIMIT 1
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.db)
        .await?;
        let Some(session) = session else {
            return Ok(None);
        };
        let status: String = session.get("status");
        if status.trim() != "running" {
            return Ok(None);
        }
        let started_at: i64 = session.get("started_at");
        let events = self
            .list_events_for_session(agent_id, session_id, 64, None)
            .await?;
        let latest_visible_acp_ts = events
            .iter()
            .rev()
            .find(|event| {
                matches!(event.stream, OutputStream::Acp) && !is_agent_user_message(&event.message)
            })
            .map(|event| event.ts);
        Ok(Some(latest_visible_acp_ts.unwrap_or(started_at)))
    }

    pub async fn list_events_for_session(
        &self,
        agent_id: &str,
        session_id: &str,
        limit: i64,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<AgentEvent>> {
        let agent = self.get_agent(agent_id).await?;
        if let Some(target_node_id) = agent.target_node_id.as_deref() {
            let client = self
                .remote_control_client_for_target_node(target_node_id)
                .await?;
            return client
                .list_agent_events(agent_id, limit, Some(session_id), before_id)
                .await;
        }
        let event_db = self.event_dbs.pool_for_agent(agent_id).await?;
        match self
            .try_list_agent_events_from_index(
                agent_id,
                &event_db,
                limit,
                Some(session_id),
                before_id,
            )
            .await
        {
            Ok(Some(events)) => return Ok(events),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    ?error,
                    agent_id,
                    session_id,
                    "falling back to SQLite after session agent event index read failed"
                );
            }
        }
        self.list_agent_events_from_sqlite(agent_id, &event_db, limit, Some(session_id), before_id)
            .await
    }

    async fn checkout_team_worker_branch(&self, workdir: &str, branch: &str) -> anyhow::Result<()> {
        let output = git_command_without_fsmonitor()
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

    #[cfg(debug_assertions)]
    pub(crate) async fn collect_agent_trace_live_overlay(
        &self,
        agent_id: &str,
    ) -> crate::diagnostics::agent_trace::AgentTraceLiveOverlay {
        use crate::diagnostics::agent_trace::{
            AgentTraceAvailability, AgentTraceLiveOverlay, AgentTraceRuntimeSummary,
        };

        let guard = self.inner.read().await;
        let Some(handle) = guard.get(agent_id) else {
            return AgentTraceLiveOverlay {
                runtime: AgentTraceRuntimeSummary {
                    ownership: "local".to_string(),
                    active_session_id: None,
                    live_state_source: "live_backend_no_handle".to_string(),
                },
                provider_adapter: AgentTraceAvailability {
                    status: "not_running".to_string(),
                    note: "no live local runtime handle is registered".to_string(),
                    details: serde_json::Value::Null,
                },
                sse: AgentTraceAvailability {
                    status: "not_running".to_string(),
                    note: "no live local runtime handle is registered".to_string(),
                    details: serde_json::Value::Null,
                },
            };
        };

        let (provider_status, provider_details) = match &handle.input {
            AgentInput::Acp(acp) => {
                let diagnostics = acp.diagnostics();
                let status = if diagnostics.command_channel_closed {
                    "closed"
                } else if diagnostics.stale_prompt.is_some() {
                    "prompt_stale"
                } else if diagnostics.active_prompt_count > 0 {
                    "prompt_active"
                } else if diagnostics.pending_command_count > 0 {
                    "commands_pending"
                } else if diagnostics.pending_tool_call_count > 0 {
                    "tool_calls_pending"
                } else if diagnostics.last_command_error.is_some() {
                    "last_command_error"
                } else {
                    "idle"
                };
                (status, safe_acp_provider_diagnostics_details(&diagnostics))
            }
            AgentInput::Stdin(_) => (
                "non_acp",
                serde_json::json!({
                    "input_kind": "stdin"
                }),
            ),
        };
        let subscriber_count = handle.output_tx.receiver_count();
        let sse_diagnostics = crate::sse::agent_sse_diagnostics(agent_id);
        let sse_status = if subscriber_count > 0 {
            "subscribers_active"
        } else if sse_diagnostics
            .as_ref()
            .and_then(|snapshot| snapshot.last_error.as_ref())
            .is_some()
        {
            "last_error"
        } else {
            "no_subscribers"
        };

        AgentTraceLiveOverlay {
            runtime: AgentTraceRuntimeSummary {
                ownership: "local".to_string(),
                active_session_id: Some(handle.session_id.clone()),
                live_state_source: "live_backend".to_string(),
            },
            provider_adapter: AgentTraceAvailability {
                status: provider_status.to_string(),
                note: "redacted live provider command-channel snapshot".to_string(),
                details: provider_details,
            },
            sse: AgentTraceAvailability {
                status: sse_status.to_string(),
                note: "redacted live output broadcast and SSE delivery snapshot".to_string(),
                details: serde_json::json!({
                    "output_subscriber_count": subscriber_count,
                    "sse": sse_diagnostics,
                }),
            },
        }
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
        self.send_input_with_images(agent_id, input, &[], message_id, expected_session_id)
            .await
    }

    #[tracing::instrument(
        skip(self, input, images, message_id),
        fields(agent_id = %agent_id, expected_session_id = ?expected_session_id, image_count = images.len()),
        err
    )]
    pub async fn send_input_with_images(
        &self,
        agent_id: &str,
        input: &str,
        images: &[AgentInputImage],
        message_id: Option<&str>,
        expected_session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let agent = self.get_agent(agent_id).await?;
        if let Some(target_node_id) = agent.target_node_id.as_deref() {
            if !images.is_empty() {
                return Err(AgentSendInputError::MultimodalUnsupported.into());
            }
            let client = self
                .remote_control_client_for_target_node(target_node_id)
                .await?;
            return client
                .send_agent_input(agent_id, input, message_id, expected_session_id)
                .await;
        }
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
            if !images.is_empty() {
                return Err(AgentSendInputError::MultimodalUnsupported.into());
            }
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
            "message_id": message_id,
            "attachments": images.iter().map(|image| serde_json::json!({
                "type": "image",
                "file_name": image.file_name,
                "mime_type": image.mime_type,
                "data": image.data,
            })).collect::<Vec<_>>()
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

        let prompt_images = images
            .iter()
            .map(|image| AcpPromptImage::new(image.data.clone(), image.mime_type.clone()))
            .collect();
        match acp
            .prompt_with_images_with_submission(
                input.to_string(),
                prompt_images,
                message_id.clone(),
            )
            .await
        {
            Ok(()) => Ok(()),
            Err(err) => {
                if let Err(persist_err) = self
                    .persist_acp_prompt_submission_failure(
                        agent_id,
                        &session_id,
                        &message_id,
                        &output_tx,
                    )
                    .await
                {
                    tracing::warn!(
                        agent_id = %agent_id,
                        error = %persist_err,
                        "failed to persist ACP prompt submission failure event"
                    );
                }
                Err(err)
            }
        }
    }

    pub(crate) async fn send_mailbox_hint_input(
        &self,
        agent_id: &str,
        input: &str,
        expected_session_id: Option<&str>,
        delivery_id: &str,
    ) -> anyhow::Result<()> {
        let agent = self.get_agent(agent_id).await?;
        if agent.target_node_id.is_some() {
            return self
                .send_input(agent_id, input, Some(delivery_id), expected_session_id)
                .await;
        }

        {
            let guard = self.inner.read().await;
            let handle = guard
                .get(agent_id)
                .ok_or_else(|| anyhow::anyhow!("agent not running"))?;
            if let Some(expected_session_id) = expected_session_id
                && expected_session_id != handle.session_id
            {
                return Err(AgentSendInputError::SessionMismatch {
                    expected: expected_session_id.to_string(),
                    running: handle.session_id.clone(),
                }
                .into());
            }
            if let AgentInput::Acp(acp) = &handle.input {
                let diagnostics = acp.diagnostics();
                let prompt_delivery_policy = handle
                    .acp_prompt_delivery_policy
                    .unwrap_or(AcpPromptDeliveryPolicy::StrictFifo);
                if !acp_accepts_best_effort_hint(&diagnostics, prompt_delivery_policy) {
                    anyhow::bail!("agent ACP input is busy; defer mailbox hint");
                }
            }
        }

        self.send_input(agent_id, input, Some(delivery_id), expected_session_id)
            .await
    }

    async fn persist_acp_prompt_submission_failure(
        &self,
        agent_id: &str,
        session_id: &str,
        message_id: &str,
        output_tx: &broadcast::Sender<AgentOutput>,
    ) -> anyhow::Result<()> {
        let seq = Uuid::now_v7().to_string();
        let ts = Utc::now().timestamp();
        let message = acp_prompt_submission_failure_event(message_id);
        let event_id = persist_agent_event(
            &self.event_dbs,
            self.idle_gc.as_ref(),
            agent_id,
            session_id,
            &seq,
            ts,
            &OutputStream::Acp,
            &message,
        )
        .await?;
        let _ = output_tx.send(AgentOutput {
            event_id,
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            seq,
            ts,
            stream: OutputStream::Acp,
            message,
        });
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
        acp.set_mode(normalize_codex_acp_mode_id(mode_id)).await
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
