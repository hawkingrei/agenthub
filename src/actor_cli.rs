use agent_client_protocol::{RequestPermissionOutcome, SelectedPermissionOutcome};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, ActorAckRequest, ActorInboxRequest,
    ActorMailboxService, ActorMessageStatus, ActorServiceError, actor_inbox_with_auto_ack,
    build_default_actor_channel_idempotency_key, build_default_actor_message_idempotency_key,
    parse_actor_transport,
};
use anyhow::Context;
use chrono::Utc;
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};

use crate::acp::{AcpPermissionRespondResult, AcpPermissionService};
use crate::actor_runtime_env::{
    ACTOR_RUNTIME_ACTOR_ID_ENV, ACTOR_RUNTIME_AGENT_ID_ENV, ACTOR_RUNTIME_CHANNEL_ENV,
    ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, ACTOR_RUNTIME_TEAM_ID_ENV, maybe_remote_mailbox_service,
    normalized_env_var,
};
use crate::agent::{AgentTimeTriggerCreateInput, AgentTimeTriggerManager};
use crate::agent::AGENT_NODE_MAIN_ID;
use crate::internal::client::InternalGrpcPeerClientConfig;
use crate::internal::tls::{InternalGrpcSecurityMode, ensure_shared_secret, ensure_tls_material};
use crate::team::{TEAM_TASK_STATUS_VALUES, TeamActorMessageTransport, TeamManager, TeamTaskStatus};

const TEAM_SHARED_THREAD_TITLE: &str = "all";
const TEAM_SHARED_THREAD_BOOTSTRAP_KIND: &str = "shared_thread";
const MAX_TIME_TRIGGER_DELAY_SECONDS: i64 = 30 * 24 * 60 * 60;
const TIME_TRIGGER_FUTURE_SAFETY_MARGIN_SECONDS: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActorOutputMode {
    Default,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActorOutputPreference {
    ToonPreferred,
    JsonPreferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActorOutputFormat {
    Toon,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActorSendPayloadSource {
    Text,
    Payload,
}

#[derive(Debug)]
enum ActorCommand {
    Help,
    TeamMembers {
        team_id: Option<String>,
        run_id: Option<String>,
    },
    Inbox {
        run_id: String,
        actor_id: String,
        limit: i64,
        after_id: Option<i64>,
        include_delivered: bool,
    },
    Ack {
        run_id: String,
        actor_id: String,
        message_id: i64,
    },
    TeamTasks {
        team_id: String,
        actor_id: String,
        limit: i64,
        status: Option<TeamTaskStatus>,
        include_shared_thread: bool,
    },
    TeamTaskCreate {
        team_id: String,
        actor_id: String,
        title: String,
        status: TeamTaskStatus,
        topic: Option<String>,
        context: Value,
    },
    TeamTaskUpdate {
        team_id: String,
        actor_id: String,
        task_id: String,
        status: TeamTaskStatus,
    },
    TimeTriggerSet {
        actor_id: String,
        delay_seconds: i64,
        message: String,
    },
    TimeTriggerList {
        actor_id: String,
        limit: i64,
    },
    TimeTriggerCancel {
        actor_id: String,
        trigger_id: String,
    },
    PermissionReviewRespond {
        team_id: String,
        actor_id: String,
        permission_id: String,
        option_id: Option<String>,
        outcome: Option<String>,
    },
    Send {
        run_id: String,
        from_actor_id: String,
        to_actor_id: Option<String>,
        channel_id: Option<String>,
        channel: String,
        transport: TeamActorMessageTransport,
        route: Option<Value>,
        payload: Box<Value>,
        payload_source: ActorSendPayloadSource,
        idempotency_key: Option<String>,
    },
}

fn actor_usage() -> String {
    format!(
        "Usage:\n  agenthub actor [--json] team-members [--team-id <team_id>] [--run-id <run_id>]\n  agenthub actor [--json] team-tasks [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--limit <n>] [--status <all|open|in_progress|in_review|completed|canceled>] [--include-shared-thread]\n  agenthub actor [--json] team-task-create --title <title> [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--status <open|in_progress|in_review|completed|canceled>] [--topic <topic>] [--context-json <json>]\n  agenthub actor [--json] team-task-update --task-id <task_id> --status <open|in_progress|in_review|completed|canceled> [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>]\n  agenthub actor [--json] inbox [--run-id <run_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--limit <n>] [--after-id <id>] [--include-delivered]\n  agenthub actor [--json] ack --message-id <id> [--run-id <run_id>] [--actor-id <actor_id> | --agent-id <agent_id>]\n  agenthub actor [--json] send (--to-actor-id <actor_id> | --to-agent-id <agent_id> | --channel-id <channel_id>) (--text <markdown> | --payload-json <json>) [--run-id <run_id>] [--from-actor-id <actor_id> | --from-agent-id <agent_id>] [--channel <name>] [--transport <local|remote>] [--route-json <json>] [--idempotency-key <key>] [--allow-duplicate]\n  agenthub actor [--json] time-trigger-set --delay-seconds <seconds> --message <text> [--actor-id <actor_id> | --agent-id <agent_id>]\n  agenthub actor [--json] time-trigger-list [--actor-id <actor_id> | --agent-id <agent_id>] [--limit <n>]\n  agenthub actor [--json] time-trigger-cancel --trigger-id <trigger_id> [--actor-id <actor_id> | --agent-id <agent_id>]\n  agenthub actor [--json] permission-review-respond --permission-id <id> [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--option-id <option_id> | --outcome cancelled]\n\nOutput:\n  Read-heavy results (`team-members`, `team-tasks`, `inbox`, `time-trigger-list`) default to TOON on stdout.\n  Confirmation results (`team-task-create`, `team-task-update`, `ack`, `send`, `time-trigger-set`, `time-trigger-cancel`, `permission-review-respond`) default to compact JSON for script compatibility.\n  `--json` forces JSON output for all structured success results.\n\nEnvironment fallback:\n  {}\n  {}\n  {}\n  {}\n  {}\n",
        ACTOR_RUNTIME_TEAM_ID_ENV,
        ACTOR_RUNTIME_CURRENT_RUN_ID_ENV,
        ACTOR_RUNTIME_ACTOR_ID_ENV,
        ACTOR_RUNTIME_AGENT_ID_ENV,
        ACTOR_RUNTIME_CHANNEL_ENV,
    )
}

fn resolve_actor_output_format(
    mode: ActorOutputMode,
    preference: ActorOutputPreference,
) -> ActorOutputFormat {
    match mode {
        ActorOutputMode::Json => ActorOutputFormat::Json,
        ActorOutputMode::Default => match preference {
            ActorOutputPreference::ToonPreferred => ActorOutputFormat::Toon,
            ActorOutputPreference::JsonPreferred => ActorOutputFormat::Json,
        },
    }
}

fn encode_toon_output<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
    toon_format::encode_default(value).context("failed to encode TOON output")
}

fn encode_json_output<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
    serde_json::to_string(value).context("failed to encode JSON output")
}

fn encode_actor_output<T: serde::Serialize>(
    value: &T,
    mode: ActorOutputMode,
    preference: ActorOutputPreference,
) -> anyhow::Result<String> {
    match resolve_actor_output_format(mode, preference) {
        ActorOutputFormat::Toon => encode_toon_output(value),
        ActorOutputFormat::Json => encode_json_output(value),
    }
}

fn write_actor_output<T: serde::Serialize>(
    value: &T,
    mode: ActorOutputMode,
    preference: ActorOutputPreference,
) -> anyhow::Result<()> {
    let output = encode_actor_output(value, mode, preference)?;
    println!("{output}");
    Ok(())
}

fn parse_i64(value: &str, field: &str) -> anyhow::Result<i64> {
    value
        .parse::<i64>()
        .map_err(|err| anyhow::anyhow!("invalid {}: {} ({err})", field, value))
}

fn parse_json(value: &str, field: &str) -> anyhow::Result<Value> {
    serde_json::from_str::<Value>(value)
        .map_err(|err| anyhow::anyhow!("invalid {} JSON: {}", field, err))
}

fn configure_actor_cli_internal_grpc(
    manager: &TeamManager,
    config: &agenthub_config::AppConfig,
) -> anyhow::Result<()> {
    if !config.internal_grpc_enabled() {
        return Ok(());
    }
    let cert_dir = PathBuf::from(config.internal_grpc_cert_dir());
    let security_mode = InternalGrpcSecurityMode::parse(&config.internal_grpc_security_mode())?;
    let shared_secret = ensure_shared_secret(&cert_dir, config.internal_grpc_auth_shared_secret())?;
    let _ = ensure_tls_material(&cert_dir, security_mode)?;
    manager.configure_internal_grpc_relay(&cert_dir, security_mode);
    manager.configure_internal_grpc_peer_client(Some(InternalGrpcPeerClientConfig {
        shared_secret,
        expected_issuer: config.internal_grpc_auth_issuer(),
        expected_audience: config.internal_grpc_auth_audience(),
        source_node_id: AGENT_NODE_MAIN_ID.to_string(),
        cert_dir: cert_dir.to_string_lossy().to_string(),
        security_mode,
    }));
    Ok(())
}

async fn init_team_manager() -> anyhow::Result<TeamManager> {
    let db = agenthub_db::init_db().await?;
    let manager = TeamManager::new(db);
    let (config, _) = agenthub_config::AppConfig::load_with_info()?;
    configure_actor_cli_internal_grpc(&manager, &config)?;
    Ok(manager)
}

async fn init_actor_mailbox_service(
    manager: &TeamManager,
) -> anyhow::Result<Arc<dyn ActorMailboxService>> {
    let service: Arc<dyn ActorMailboxService> = match maybe_remote_mailbox_service().await? {
        Some(client) => Arc::new(client),
        None => Arc::new(manager.actor_mailbox_service()),
    };
    Ok(service)
}

fn map_actor_service_error(operation: &str, err: ActorServiceError) -> anyhow::Error {
    anyhow::anyhow!("{operation} failed ({:?}): {}", err.code, err.message)
}

fn resolve_actor_send_payload(
    text: Option<String>,
    payload: Option<Value>,
) -> anyhow::Result<(Value, ActorSendPayloadSource)> {
    match (text, payload) {
        (Some(text), None) => {
            if text.trim().is_empty() {
                return Err(anyhow::anyhow!("text must be a non-empty string"));
            }
            Ok((Value::String(text), ActorSendPayloadSource::Text))
        }
        (None, Some(payload)) => Ok((payload, ActorSendPayloadSource::Payload)),
        (Some(_), Some(_)) => Err(anyhow::anyhow!(
            "--text and --payload-json cannot be used together"
        )),
        (None, None) => Err(anyhow::anyhow!("--text or --payload-json is required")),
    }
}

fn resolve_actor_send_target(
    to_actor_id: Option<String>,
    channel_id: Option<String>,
) -> anyhow::Result<(Option<String>, Option<String>)> {
    let to_actor_id = take_optional(to_actor_id);
    let channel_id = take_optional(channel_id);
    match (to_actor_id, channel_id) {
        (Some(to_actor_id), None) => Ok((Some(to_actor_id), None)),
        (None, Some(channel_id)) => Ok((None, Some(channel_id))),
        (Some(_), Some(_)) => Err(anyhow::anyhow!(
            "to_actor_id and channel_id cannot be used together"
        )),
        (None, None) => Err(anyhow::anyhow!("to_actor_id or channel_id is required")),
    }
}

fn take_required_with_env_keys(
    value: Option<String>,
    env_keys: &[&str],
    field: &str,
) -> anyhow::Result<String> {
    if let Some(value) = value {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    for env_key in env_keys {
        if let Some(value) = normalized_env_var(env_key) {
            return Ok(value);
        }
    }
    Err(anyhow::anyhow!(
        "{} is required (flag or env fallback)",
        field
    ))
}

fn take_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn take_run_id(value: Option<String>) -> anyhow::Result<String> {
    take_required_with_env_keys(value, &[ACTOR_RUNTIME_CURRENT_RUN_ID_ENV], "run_id")
}

fn take_team_id(value: Option<String>) -> anyhow::Result<String> {
    take_required_with_env_keys(value, &[ACTOR_RUNTIME_TEAM_ID_ENV], "team_id")
}

fn take_actor_id(value: Option<String>) -> anyhow::Result<String> {
    take_required_with_env_keys(
        value,
        &[ACTOR_RUNTIME_ACTOR_ID_ENV, ACTOR_RUNTIME_AGENT_ID_ENV],
        "actor_id",
    )
}

fn parse_team_task_status_argument(raw: &str) -> anyhow::Result<TeamTaskStatus> {
    match raw.trim() {
        "open" => Ok(TeamTaskStatus::Open),
        "in_progress" => Ok(TeamTaskStatus::InProgress),
        "in_review" => Ok(TeamTaskStatus::InReview),
        "completed" => Ok(TeamTaskStatus::Completed),
        "canceled" => Ok(TeamTaskStatus::Canceled),
        other => Err(anyhow::anyhow!(
            "invalid task status '{}', expected one of: {}",
            other,
            TEAM_TASK_STATUS_VALUES.join(", ")
        )),
    }
}

fn resolve_team_leader_member_id(spec: &Value) -> anyhow::Result<String> {
    if let Some(leader_member_id) = spec
        .as_object()
        .and_then(|obj| obj.get("leader_member_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(leader_member_id.to_string());
    }

    if let Some(members) = spec
        .as_object()
        .and_then(|obj| obj.get("members"))
        .and_then(Value::as_array)
    {
        for member in members {
            let Some(member_obj) = member.as_object() else {
                continue;
            };
            let role = member_obj
                .get("role")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            let member_id = member_obj
                .get("member_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            if role.eq_ignore_ascii_case("leader") && !member_id.is_empty() {
                return Ok(member_id.to_string());
            }
        }
    }

    if let Some(entrypoint) = spec
        .as_object()
        .and_then(|obj| obj.get("entrypoint"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(entrypoint.to_string());
    }

    Err(anyhow::anyhow!("team has no leader configured"))
}

fn compute_time_trigger_fire_at(now_ts: i64, delay_seconds: i64) -> i64 {
    now_ts + delay_seconds + TIME_TRIGGER_FUTURE_SAFETY_MARGIN_SECONDS
}

async fn load_team_for_context(
    manager: &TeamManager,
    team_id: &str,
    actor_id: &str,
) -> anyhow::Result<crate::team::TeamDefinitionRecord> {
    let team = manager
        .get_team(team_id)
        .await
        .with_context(|| format!("load team failed: {team_id}"))?;
    let is_member = manager
        .team_has_member(&team.id, actor_id)
        .await
        .context("load team members failed")?;
    anyhow::ensure!(is_member, "current actor is not a member of this team");
    Ok(team)
}

async fn ensure_leader_team_access(
    manager: &TeamManager,
    team_id: &str,
    actor_id: &str,
) -> anyhow::Result<crate::team::TeamDefinitionRecord> {
    let team = load_team_for_context(manager, team_id, actor_id).await?;
    let leader_member_id = resolve_team_leader_member_id(&team.spec)?;
    anyhow::ensure!(
        actor_id == leader_member_id,
        "only leader may create or update Team tasks"
    );
    Ok(team)
}

fn is_shared_thread_task(task: &crate::team::TeamTaskRecord) -> bool {
    if task.title.trim().eq_ignore_ascii_case(TEAM_SHARED_THREAD_TITLE) {
        return true;
    }
    task.context
        .as_object()
        .and_then(|obj| obj.get("bootstrap_kind"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| value.eq_ignore_ascii_case(TEAM_SHARED_THREAD_BOOTSTRAP_KIND))
}

struct ParsedActorCommand {
    output_mode: ActorOutputMode,
    command: ActorCommand,
}

fn parse_actor_args(args: &[String]) -> anyhow::Result<ParsedActorCommand> {
    let mut output_mode = ActorOutputMode::Default;
    let mut command_start = 0usize;
    while let Some(arg) = args.get(command_start) {
        if arg != "--json" {
            break;
        }
        output_mode = ActorOutputMode::Json;
        command_start += 1;
    }
    let command = parse_actor_command(&args[command_start..], &mut output_mode)?;
    Ok(ParsedActorCommand {
        output_mode,
        command,
    })
}

fn parse_actor_command(
    args: &[String],
    output_mode: &mut ActorOutputMode,
) -> anyhow::Result<ActorCommand> {
    let sub = args
        .first()
        .ok_or_else(|| anyhow::anyhow!("missing actor subcommand\n{}", actor_usage()))?;
    match sub.as_str() {
        "team-members" => {
            let mut team_id = None;
            let mut run_id = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => {
                        *output_mode = ActorOutputMode::Json;
                    }
                    "--team-id" => {
                        idx += 1;
                        team_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--team-id requires a value"))?,
                        );
                    }
                    "--run-id" => {
                        idx += 1;
                        run_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--run-id requires a value"))?,
                        );
                    }
                    other => {
                        return Err(anyhow::anyhow!("unknown flag for team-members: {}", other));
                    }
                }
                idx += 1;
            }
            let explicit_team_id = take_optional(team_id);
            let explicit_run_id = take_optional(run_id);
            let team_id = explicit_team_id.clone().or_else(|| {
                if explicit_run_id.is_some() {
                    return None;
                }
                normalized_env_var(ACTOR_RUNTIME_TEAM_ID_ENV)
            });
            let run_id = explicit_run_id.or_else(|| {
                if explicit_team_id.is_some() {
                    return None;
                }
                normalized_env_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV)
            });
            if team_id.is_none() && run_id.is_none() {
                return Err(anyhow::anyhow!(
                    "team-members requires --team-id, --run-id, or actor runtime env fallback"
                ));
            }
            Ok(ActorCommand::TeamMembers { team_id, run_id })
        }
        "team-tasks" => {
            let mut team_id = None;
            let mut actor_id = None;
            let mut limit = 100_i64;
            let mut status = None;
            let mut include_shared_thread = false;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => {
                        *output_mode = ActorOutputMode::Json;
                    }
                    "--team-id" => {
                        idx += 1;
                        team_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--team-id requires a value"))?,
                        );
                    }
                    flag @ ("--actor-id" | "--agent-id") => {
                        idx += 1;
                        actor_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?,
                        );
                    }
                    "--limit" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--limit requires a value"))?;
                        limit = parse_i64(raw, "limit")?;
                    }
                    "--status" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--status requires a value"))?;
                        status = Some(raw.clone());
                    }
                    "--include-shared-thread" => {
                        include_shared_thread = true;
                    }
                    other => return Err(anyhow::anyhow!("unknown flag for team-tasks: {}", other)),
                }
                idx += 1;
            }
            let status = match status.as_deref().map(str::trim) {
                Some("all") | None => None,
                Some(raw) => Some(parse_team_task_status_argument(raw)?),
            };
            Ok(ActorCommand::TeamTasks {
                team_id: take_team_id(team_id)?,
                actor_id: take_actor_id(actor_id)?,
                limit: limit.clamp(1, 500),
                status,
                include_shared_thread,
            })
        }
        "team-task-create" => {
            let mut team_id = None;
            let mut actor_id = None;
            let mut title = None;
            let mut status = TeamTaskStatus::Open;
            let mut topic = None;
            let mut context = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => {
                        *output_mode = ActorOutputMode::Json;
                    }
                    "--team-id" => {
                        idx += 1;
                        team_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--team-id requires a value"))?,
                        );
                    }
                    flag @ ("--actor-id" | "--agent-id") => {
                        idx += 1;
                        actor_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?,
                        );
                    }
                    "--title" => {
                        idx += 1;
                        title = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--title requires a value"))?,
                        );
                    }
                    "--status" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--status requires a value"))?;
                        status = parse_team_task_status_argument(raw)?;
                    }
                    "--topic" => {
                        idx += 1;
                        topic = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--topic requires a value"))?,
                        );
                    }
                    "--context-json" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--context-json requires a value"))?;
                        context = Some(parse_json(raw, "context_json")?);
                    }
                    other => {
                        return Err(anyhow::anyhow!("unknown flag for team-task-create: {}", other));
                    }
                }
                idx += 1;
            }
            let title = take_optional(title)
                .ok_or_else(|| anyhow::anyhow!("title is required"))?;
            Ok(ActorCommand::TeamTaskCreate {
                team_id: take_team_id(team_id)?,
                actor_id: take_actor_id(actor_id)?,
                title,
                status,
                topic: take_optional(topic),
                context: context.unwrap_or_else(|| serde_json::json!({})),
            })
        }
        "team-task-update" => {
            let mut team_id = None;
            let mut actor_id = None;
            let mut task_id = None;
            let mut status = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => {
                        *output_mode = ActorOutputMode::Json;
                    }
                    "--team-id" => {
                        idx += 1;
                        team_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--team-id requires a value"))?,
                        );
                    }
                    flag @ ("--actor-id" | "--agent-id") => {
                        idx += 1;
                        actor_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?,
                        );
                    }
                    "--task-id" => {
                        idx += 1;
                        task_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--task-id requires a value"))?,
                        );
                    }
                    "--status" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--status requires a value"))?;
                        status = Some(parse_team_task_status_argument(raw)?);
                    }
                    other => {
                        return Err(anyhow::anyhow!("unknown flag for team-task-update: {}", other));
                    }
                }
                idx += 1;
            }
            Ok(ActorCommand::TeamTaskUpdate {
                team_id: take_team_id(team_id)?,
                actor_id: take_actor_id(actor_id)?,
                task_id: take_optional(task_id)
                    .ok_or_else(|| anyhow::anyhow!("task_id is required"))?,
                status: status.ok_or_else(|| anyhow::anyhow!("status is required"))?,
            })
        }
        "inbox" => {
            let mut run_id = None;
            let mut actor_id = None;
            let mut limit = 100_i64;
            let mut after_id = None;
            let mut include_delivered = false;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => {
                        *output_mode = ActorOutputMode::Json;
                    }
                    "--run-id" => {
                        idx += 1;
                        run_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--run-id requires a value"))?,
                        );
                    }
                    flag @ ("--actor-id" | "--agent-id") => {
                        idx += 1;
                        actor_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?,
                        );
                    }
                    "--limit" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--limit requires a value"))?;
                        limit = parse_i64(raw, "limit")?;
                    }
                    "--after-id" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--after-id requires a value"))?;
                        after_id = Some(parse_i64(raw, "after_id")?);
                    }
                    "--include-delivered" => {
                        include_delivered = true;
                    }
                    other => {
                        return Err(anyhow::anyhow!("unknown flag for inbox: {}", other));
                    }
                }
                idx += 1;
            }
            Ok(ActorCommand::Inbox {
                run_id: take_run_id(run_id)?,
                actor_id: take_actor_id(actor_id)?,
                limit: limit.max(1),
                after_id,
                include_delivered,
            })
        }
        "ack" => {
            let mut run_id = None;
            let mut actor_id = None;
            let mut message_id = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => {
                        *output_mode = ActorOutputMode::Json;
                    }
                    "--run-id" => {
                        idx += 1;
                        run_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--run-id requires a value"))?,
                        );
                    }
                    flag @ ("--actor-id" | "--agent-id") => {
                        idx += 1;
                        actor_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?,
                        );
                    }
                    "--message-id" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--message-id requires a value"))?;
                        message_id = Some(parse_i64(raw, "message_id")?);
                    }
                    other => return Err(anyhow::anyhow!("unknown flag for ack: {}", other)),
                }
                idx += 1;
            }
            Ok(ActorCommand::Ack {
                run_id: take_run_id(run_id)?,
                actor_id: take_actor_id(actor_id)?,
                message_id: message_id.ok_or_else(|| anyhow::anyhow!("message_id is required"))?,
            })
        }
        "send" => {
            let mut run_id = None;
            let mut from_actor_id = None;
            let mut to_actor_id = None;
            let mut channel_id = None;
            let mut channel = None;
            let mut transport = None;
            let mut route = None;
            let mut text = None;
            let mut payload = None;
            let mut idempotency_key = None;
            let mut allow_duplicate = false;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => {
                        *output_mode = ActorOutputMode::Json;
                    }
                    "--run-id" => {
                        idx += 1;
                        run_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--run-id requires a value"))?,
                        );
                    }
                    "--from-actor-id" => {
                        idx += 1;
                        from_actor_id =
                            Some(args.get(idx).cloned().ok_or_else(|| {
                                anyhow::anyhow!("--from-actor-id requires a value")
                            })?);
                    }
                    "--from-agent-id" => {
                        idx += 1;
                        from_actor_id =
                            Some(args.get(idx).cloned().ok_or_else(|| {
                                anyhow::anyhow!("--from-agent-id requires a value")
                            })?);
                    }
                    "--to-actor-id" => {
                        idx += 1;
                        to_actor_id =
                            Some(args.get(idx).cloned().ok_or_else(|| {
                                anyhow::anyhow!("--to-actor-id requires a value")
                            })?);
                    }
                    "--to-agent-id" => {
                        idx += 1;
                        to_actor_id =
                            Some(args.get(idx).cloned().ok_or_else(|| {
                                anyhow::anyhow!("--to-agent-id requires a value")
                            })?);
                    }
                    "--channel" => {
                        idx += 1;
                        channel = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--channel requires a value"))?,
                        );
                    }
                    "--channel-id" => {
                        idx += 1;
                        channel_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--channel-id requires a value"))?,
                        );
                    }
                    "--transport" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--transport requires a value"))?;
                        transport =
                            Some(parse_actor_transport(Some(raw.as_str())).map_err(|_| {
                                anyhow::anyhow!("transport must be local or remote")
                            })?);
                    }
                    "--route-json" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--route-json requires a value"))?;
                        route = Some(parse_json(raw, "route_json")?);
                    }
                    "--text" => {
                        idx += 1;
                        text = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--text requires a value"))?,
                        );
                    }
                    "--payload-json" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--payload-json requires a value"))?;
                        payload = Some(parse_json(raw, "payload_json")?);
                    }
                    "--idempotency-key" => {
                        idx += 1;
                        idempotency_key = Some(args.get(idx).cloned().ok_or_else(|| {
                            anyhow::anyhow!("--idempotency-key requires a value")
                        })?);
                    }
                    "--allow-duplicate" => {
                        allow_duplicate = true;
                    }
                    other => return Err(anyhow::anyhow!("unknown flag for send: {}", other)),
                }
                idx += 1;
            }

            let transport = transport.unwrap_or(TeamActorMessageTransport::Local);
            if transport == TeamActorMessageTransport::Remote && route.is_none() {
                return Err(anyhow::anyhow!(
                    "route_json is required for remote transport"
                ));
            }
            if transport == TeamActorMessageTransport::Local && route.is_some() {
                return Err(anyhow::anyhow!(
                    "route_json is not supported for local transport"
                ));
            }

            let fallback_channel = normalized_env_var(ACTOR_RUNTIME_CHANNEL_ENV)
                .unwrap_or_else(|| "default".to_string());
            let run_id = take_run_id(run_id)?;
            let from_actor_id = take_required_with_env_keys(
                from_actor_id,
                &[ACTOR_RUNTIME_ACTOR_ID_ENV, ACTOR_RUNTIME_AGENT_ID_ENV],
                "from_actor_id",
            )?;
            let (to_actor_id, channel_id) = resolve_actor_send_target(to_actor_id, channel_id)?;
            let channel = take_optional(channel).unwrap_or(fallback_channel);
            let (payload, payload_source) = resolve_actor_send_payload(text, payload)?;
            let explicit_idempotency_key = match idempotency_key {
                Some(raw) => {
                    let trimmed = raw.trim();
                    if trimmed.is_empty() {
                        return Err(anyhow::anyhow!(
                            "idempotency_key must be a non-empty string"
                        ));
                    }
                    if trimmed.len() > 128 {
                        return Err(anyhow::anyhow!(
                            "idempotency_key must be at most 128 characters"
                        ));
                    }
                    Some(trimmed.to_string())
                }
                None => None,
            };
            if allow_duplicate && explicit_idempotency_key.is_some() {
                return Err(anyhow::anyhow!(
                    "--allow-duplicate cannot be used with --idempotency-key"
                ));
            }
            let to_peer_id = if transport == TeamActorMessageTransport::Remote {
                ACTOR_NODE_PEER_ID
            } else {
                ACTOR_MAIN_PEER_ID
            };
            let resolved_idempotency_key = if allow_duplicate {
                None
            } else {
                Some(explicit_idempotency_key.unwrap_or_else(|| {
                    match (to_actor_id.as_deref(), channel_id.as_deref()) {
                        (Some(to_actor_id), None) => build_default_actor_message_idempotency_key(
                            &run_id,
                            &from_actor_id,
                            ACTOR_MAIN_PEER_ID,
                            to_actor_id,
                            to_peer_id,
                            &channel,
                            transport.as_str(),
                            route.as_ref(),
                            &payload,
                        ),
                        (None, Some(channel_id)) => build_default_actor_channel_idempotency_key(
                            &run_id,
                            &from_actor_id,
                            ACTOR_MAIN_PEER_ID,
                            channel_id,
                            &channel,
                            transport.as_str(),
                            route.as_ref(),
                            &payload,
                        ),
                        _ => unreachable!("actor send target already validated"),
                    }
                }))
            };

            Ok(ActorCommand::Send {
                run_id,
                from_actor_id,
                to_actor_id,
                channel_id,
                channel,
                transport,
                route,
                payload: Box::new(payload),
                payload_source,
                idempotency_key: resolved_idempotency_key,
            })
        }
        "time-trigger-set" => {
            let mut actor_id = None;
            let mut delay_seconds = None;
            let mut message = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => {
                        *output_mode = ActorOutputMode::Json;
                    }
                    flag @ ("--actor-id" | "--agent-id") => {
                        idx += 1;
                        actor_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?,
                        );
                    }
                    "--delay-seconds" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--delay-seconds requires a value"))?;
                        delay_seconds = Some(parse_i64(raw, "delay_seconds")?);
                    }
                    "--message" => {
                        idx += 1;
                        message = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--message requires a value"))?,
                        );
                    }
                    other => {
                        return Err(anyhow::anyhow!("unknown flag for time-trigger-set: {}", other));
                    }
                }
                idx += 1;
            }
            Ok(ActorCommand::TimeTriggerSet {
                actor_id: take_actor_id(actor_id)?,
                delay_seconds: delay_seconds
                    .ok_or_else(|| anyhow::anyhow!("delay_seconds is required"))?,
                message: take_optional(message)
                    .ok_or_else(|| anyhow::anyhow!("message is required"))?,
            })
        }
        "time-trigger-list" => {
            let mut actor_id = None;
            let mut limit = 20_i64;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => {
                        *output_mode = ActorOutputMode::Json;
                    }
                    flag @ ("--actor-id" | "--agent-id") => {
                        idx += 1;
                        actor_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?,
                        );
                    }
                    "--limit" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--limit requires a value"))?;
                        limit = parse_i64(raw, "limit")?;
                    }
                    other => {
                        return Err(anyhow::anyhow!("unknown flag for time-trigger-list: {}", other));
                    }
                }
                idx += 1;
            }
            Ok(ActorCommand::TimeTriggerList {
                actor_id: take_actor_id(actor_id)?,
                limit: limit.clamp(1, 500),
            })
        }
        "time-trigger-cancel" => {
            let mut actor_id = None;
            let mut trigger_id = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => {
                        *output_mode = ActorOutputMode::Json;
                    }
                    flag @ ("--actor-id" | "--agent-id") => {
                        idx += 1;
                        actor_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?,
                        );
                    }
                    "--trigger-id" => {
                        idx += 1;
                        trigger_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--trigger-id requires a value"))?,
                        );
                    }
                    other => {
                        return Err(anyhow::anyhow!(
                            "unknown flag for time-trigger-cancel: {}",
                            other
                        ));
                    }
                }
                idx += 1;
            }
            Ok(ActorCommand::TimeTriggerCancel {
                actor_id: take_actor_id(actor_id)?,
                trigger_id: take_optional(trigger_id)
                    .ok_or_else(|| anyhow::anyhow!("trigger_id is required"))?,
            })
        }
        "permission-review-respond" => {
            let mut team_id = None;
            let mut actor_id = None;
            let mut permission_id = None;
            let mut option_id = None;
            let mut outcome = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => {
                        *output_mode = ActorOutputMode::Json;
                    }
                    "--team-id" => {
                        idx += 1;
                        team_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--team-id requires a value"))?,
                        );
                    }
                    flag @ ("--actor-id" | "--agent-id") => {
                        idx += 1;
                        actor_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?,
                        );
                    }
                    "--permission-id" => {
                        idx += 1;
                        permission_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--permission-id requires a value"))?,
                        );
                    }
                    "--option-id" => {
                        idx += 1;
                        option_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--option-id requires a value"))?,
                        );
                    }
                    "--outcome" => {
                        idx += 1;
                        outcome = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--outcome requires a value"))?,
                        );
                    }
                    other => {
                        return Err(anyhow::anyhow!(
                            "unknown flag for permission-review-respond: {}",
                            other
                        ));
                    }
                }
                idx += 1;
            }
            if option_id.is_some() && outcome.is_some() {
                return Err(anyhow::anyhow!(
                    "--option-id and --outcome cannot be used together"
                ));
            }
            Ok(ActorCommand::PermissionReviewRespond {
                team_id: take_team_id(team_id)?,
                actor_id: take_actor_id(actor_id)?,
                permission_id: take_optional(permission_id)
                    .ok_or_else(|| anyhow::anyhow!("permission_id is required"))?,
                option_id: take_optional(option_id),
                outcome: take_optional(outcome),
            })
        }
        "help" | "--help" | "-h" => Ok(ActorCommand::Help),
        other => Err(anyhow::anyhow!(
            "unknown actor subcommand: {}\n{}",
            other,
            actor_usage()
        )),
    }
}

async fn run_actor_command(
    command: ActorCommand,
    output_mode: ActorOutputMode,
) -> anyhow::Result<()> {
    match command {
        ActorCommand::Help => {
            println!("{}", actor_usage());
        }
        ActorCommand::TeamMembers { team_id, run_id } => {
            let db = agenthub_db::init_db().await?;
            let manager = TeamManager::new(db);
            let team_context = manager
                .describe_team_context(team_id.as_deref(), run_id.as_deref())
                .await?;
            write_actor_output(
                &team_context,
                output_mode,
                ActorOutputPreference::ToonPreferred,
            )?;
        }
        ActorCommand::TeamTasks {
            team_id,
            actor_id,
            limit,
            status,
            include_shared_thread,
        } => {
            let db = agenthub_db::init_db().await?;
            let manager = TeamManager::new(db);
            let _team = load_team_for_context(&manager, &team_id, &actor_id).await?;
            let mut tasks = manager.list_tasks(&team_id, limit).await?;
            if !include_shared_thread {
                tasks.retain(|task| !is_shared_thread_task(task));
            }
            if let Some(status) = status {
                tasks.retain(|task| task.status == status);
            }
            write_actor_output(&tasks, output_mode, ActorOutputPreference::ToonPreferred)?;
        }
        ActorCommand::TeamTaskCreate {
            team_id,
            actor_id,
            title,
            status,
            topic,
            context,
        } => {
            let db = agenthub_db::init_db().await?;
            let manager = TeamManager::new(db);
            let _team = ensure_leader_team_access(&manager, &team_id, &actor_id).await?;
            let (task, conversation) = manager
                .create_task(&team_id, &title, &actor_id, context, "group_chat", topic.as_deref())
                .await?;
            let task = if status == TeamTaskStatus::Open {
                task
            } else {
                manager.update_task_status(&task.id, status).await?
            };
            let output = serde_json::json!({
                "task": task,
                "conversation": conversation,
            });
            write_actor_output(&output, output_mode, ActorOutputPreference::JsonPreferred)?;
        }
        ActorCommand::TeamTaskUpdate {
            team_id,
            actor_id,
            task_id,
            status,
        } => {
            let db = agenthub_db::init_db().await?;
            let manager = TeamManager::new(db);
            let _team = ensure_leader_team_access(&manager, &team_id, &actor_id).await?;
            let existing = manager.get_task(&task_id).await?;
            anyhow::ensure!(
                existing.team_id == team_id,
                "task does not belong to this team"
            );
            let task = manager.update_task_status(&task_id, status).await?;
            write_actor_output(&task, output_mode, ActorOutputPreference::JsonPreferred)?;
        }
        ActorCommand::Inbox {
            run_id,
            actor_id,
            limit,
            after_id,
            include_delivered,
        } => {
            let manager = init_team_manager().await?;
            let service = init_actor_mailbox_service(&manager).await?;
            let states = if include_delivered {
                Some(vec![
                    ActorMessageStatus::Pending,
                    ActorMessageStatus::Delivered,
                    ActorMessageStatus::DeadLetter,
                ])
            } else {
                Some(vec![ActorMessageStatus::Pending])
            };
            let inbox = actor_inbox_with_auto_ack(
                service.as_ref(),
                ActorInboxRequest {
                    run_id,
                    actor_id,
                    cursor: after_id,
                    limit: Some(limit),
                    states,
                },
            )
            .await
            .map_err(|err| map_actor_service_error("actor inbox", err))?;
            write_actor_output(&inbox, output_mode, ActorOutputPreference::ToonPreferred)?;
        }
        ActorCommand::Ack {
            run_id,
            actor_id,
            message_id,
        } => {
            let manager = init_team_manager().await?;
            let service = init_actor_mailbox_service(&manager).await?;
            let message = service
                .actor_ack(ActorAckRequest {
                    run_id,
                    actor_id,
                    message_id,
                    ack_token: None,
                    result: None,
                })
                .await
                .map_err(|err| map_actor_service_error("actor ack", err))?;
            write_actor_output(&message, output_mode, ActorOutputPreference::JsonPreferred)?;
        }
        ActorCommand::Send {
            run_id,
            from_actor_id,
            to_actor_id,
            channel_id,
            channel,
            transport,
            route,
            payload,
            payload_source,
            idempotency_key,
        } => {
            let manager = init_team_manager().await?;
            let service = init_actor_mailbox_service(&manager).await?;
            let message = service
                .actor_send(agenthub_team_actor::ActorSendRequest {
                    run_id,
                    from_actor_id,
                    from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
                    to_actor_id,
                    channel_id,
                    to_peer_id: Some(
                        if transport == TeamActorMessageTransport::Remote {
                            ACTOR_NODE_PEER_ID
                        } else {
                            ACTOR_MAIN_PEER_ID
                        }
                        .to_string(),
                    ),
                    channel: Some(channel),
                    transport: Some(transport),
                    route,
                    payload: *payload,
                    idempotency_key,
                })
                .await
                .map_err(|err| map_actor_service_error("actor send", err))?;
            if payload_source == ActorSendPayloadSource::Payload {
                eprintln!(
                    "warning: prefer --text for markdown-rich mailbox messages; --payload-json is best reserved for structured machine-readable coordination"
                );
            }
            write_actor_output(&message, output_mode, ActorOutputPreference::JsonPreferred)?;
        }
        ActorCommand::TimeTriggerSet {
            actor_id,
            delay_seconds,
            message,
        } => {
            anyhow::ensure!(
                (1..=MAX_TIME_TRIGGER_DELAY_SECONDS).contains(&delay_seconds),
                "delay_seconds must be between 1 and {}",
                MAX_TIME_TRIGGER_DELAY_SECONDS
            );
            let db = agenthub_db::init_db().await?;
            let manager = AgentTimeTriggerManager::new(db);
            let now_ts = Utc::now().timestamp();
            let record = manager
                .create_time_trigger(AgentTimeTriggerCreateInput {
                    agent_id: actor_id.clone(),
                    created_by_actor_id: actor_id,
                    message_text: message,
                    fire_at: compute_time_trigger_fire_at(now_ts, delay_seconds),
                })
                .await?;
            write_actor_output(&record, output_mode, ActorOutputPreference::JsonPreferred)?;
        }
        ActorCommand::TimeTriggerList { actor_id, limit } => {
            let db = agenthub_db::init_db().await?;
            let manager = AgentTimeTriggerManager::new(db);
            let records = manager.list_triggers_for_agent(actor_id.as_str(), limit).await?;
            write_actor_output(&records, output_mode, ActorOutputPreference::ToonPreferred)?;
        }
        ActorCommand::TimeTriggerCancel {
            actor_id,
            trigger_id,
        } => {
            let db = agenthub_db::init_db().await?;
            let manager = AgentTimeTriggerManager::new(db);
            let canceled = manager
                .cancel_trigger(actor_id.as_str(), trigger_id.as_str())
                .await?;
            anyhow::ensure!(canceled, "time trigger not found");
            let output = serde_json::json!({
                "status": "ok",
                "trigger_id": trigger_id,
            });
            write_actor_output(&output, output_mode, ActorOutputPreference::JsonPreferred)?;
        }
        ActorCommand::PermissionReviewRespond {
            team_id,
            actor_id,
            permission_id,
            option_id,
            outcome,
        } => {
            let db = agenthub_db::init_db().await?;
            let permissions = AcpPermissionService::new(db.clone());
            let manager = TeamManager::new(db);
            let Some(record) = permissions.get(&permission_id).await? else {
                anyhow::bail!("permission request not found");
            };
            anyhow::ensure!(
                record.team_id.as_deref() == Some(team_id.as_str()),
                "permission request does not belong to this team"
            );
            anyhow::ensure!(
                manager.team_has_member(&team_id, actor_id.as_str()).await?,
                "current actor is not a member of this team"
            );
            let team = manager.get_team(&team_id).await?;
            let leader_member_id = resolve_team_leader_member_id(&team.spec)?;
            anyhow::ensure!(
                record.requester_actor_id.as_deref() != Some(actor_id.as_str()),
                "requester cannot review its own permission request"
            );
            let worker_originated_request = record
                .requester_role
                .as_deref()
                .is_some_and(|role| role.eq_ignore_ascii_case("worker"));
            let active_reviewer =
                record
                    .review_target_actor_id
                    .as_deref()
                    .or(if worker_originated_request {
                        Some(leader_member_id.as_str())
                    } else {
                        None
                    });
            anyhow::ensure!(
                active_reviewer == Some(actor_id.as_str()),
                if worker_originated_request {
                    "leader is the only reviewer for worker-originated permission requests"
                } else {
                    "current actor is not the active reviewer for this permission request"
                }
            );
            if record.status != "pending" {
                let output = serde_json::json!({
                    "status": "already_resolved",
                    "permission_id": permission_id,
                    "request_status": record.status,
                });
                write_actor_output(&output, output_mode, ActorOutputPreference::JsonPreferred)?;
                return Ok(());
            }

            let response_outcome = if let Some(option_id) = option_id.as_ref() {
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option_id.clone()))
            } else {
                match outcome.as_deref() {
                    Some("cancelled") | None => RequestPermissionOutcome::Cancelled,
                    Some(other) => {
                        anyhow::bail!(
                            "unsupported outcome '{}', expected 'cancelled'",
                            other
                        );
                    }
                }
            };
            let responded = permissions
                .respond(
                    &permission_id,
                    response_outcome,
                    option_id.clone(),
                    Some(actor_id.clone()),
                )
                .await?;
            if matches!(responded, AcpPermissionRespondResult::AlreadyResolved) {
                let request_status = permissions
                    .get(&permission_id)
                    .await?
                    .map(|current| current.status)
                    .unwrap_or_else(|| "resolved".to_string());
                let output = serde_json::json!({
                    "status": "already_resolved",
                    "permission_id": permission_id,
                    "request_status": request_status,
                });
                write_actor_output(&output, output_mode, ActorOutputPreference::JsonPreferred)?;
                return Ok(());
            }
            let output = serde_json::json!({
                "status": "ok",
                "permission_id": permission_id,
                "reviewed_by_actor_id": actor_id,
            });
            write_actor_output(&output, output_mode, ActorOutputPreference::JsonPreferred)?;
        }
    }
    Ok(())
}

pub async fn maybe_run_from_args() -> Option<anyhow::Result<()>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) != Some("actor") {
        return None;
    }
    let parsed = parse_actor_args(&args[1..]);
    Some(match parsed {
        Ok(parsed) => run_actor_command(parsed.command, parsed.output_mode).await,
        Err(err) => Err(err),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use agenthub_team_actor::ActorInboxResponse;
    use serde::Serialize;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn restore_env(key: &str, value: Option<String>) {
        if let Some(value) = value {
            unsafe { std::env::set_var(key, value) }
        } else {
            unsafe { std::env::remove_var(key) }
        }
    }

    #[test]
    fn parse_inbox_uses_env_fallback() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-x");
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-x");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec!["inbox".to_string(), "--limit".to_string(), "5".to_string()];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse inbox");
        match parsed {
            ActorCommand::Inbox {
                run_id,
                actor_id,
                limit,
                ..
            } => {
                assert_eq!(run_id, "run-x");
                assert_eq!(actor_id, "planner");
                assert_eq!(limit, 5);
            }
            _ => panic!("expected inbox command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_actor_args_accepts_json_flag_before_subcommand() {
        let args = vec![
            "--json".to_string(),
            "inbox".to_string(),
            "--run-id".to_string(),
            "run-x".to_string(),
            "--actor-id".to_string(),
            "planner".to_string(),
        ];
        let parsed = parse_actor_args(&args).expect("parse actor args");
        assert_eq!(parsed.output_mode, ActorOutputMode::Json);
        assert!(matches!(
            parsed.command,
            ActorCommand::Inbox { ref run_id, ref actor_id, .. }
                if run_id == "run-x" && actor_id == "planner"
        ));
    }

    #[test]
    fn parse_actor_args_accepts_json_flag_after_subcommand() {
        let args = vec![
            "inbox".to_string(),
            "--json".to_string(),
            "--run-id".to_string(),
            "run-y".to_string(),
            "--actor-id".to_string(),
            "planner".to_string(),
        ];
        let parsed = parse_actor_args(&args).expect("parse actor args");
        assert_eq!(parsed.output_mode, ActorOutputMode::Json);
        assert!(matches!(
            parsed.command,
            ActorCommand::Inbox { ref run_id, ref actor_id, .. }
                if run_id == "run-y" && actor_id == "planner"
        ));
    }

    #[test]
    fn parse_team_members_uses_env_fallback() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-members-team");
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-team-members");
        }
        let args = vec!["team-members".to_string()];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse team-members");
        match parsed {
            ActorCommand::TeamMembers { team_id, run_id } => {
                assert_eq!(team_id.as_deref(), Some("team-members-team"));
                assert_eq!(run_id.as_deref(), Some("run-team-members"));
            }
            _ => panic!("expected team-members command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
    }

    #[test]
    fn parse_team_members_accepts_run_id_flag() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-env-should-be-ignored");
            std::env::set_var(
                ACTOR_RUNTIME_CURRENT_RUN_ID_ENV,
                "run-env-should-be-ignored",
            );
        }
        let args = vec![
            "team-members".to_string(),
            "--run-id".to_string(),
            "run-explicit".to_string(),
        ];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse team-members");
        match parsed {
            ActorCommand::TeamMembers { team_id, run_id } => {
                assert!(team_id.is_none());
                assert_eq!(run_id.as_deref(), Some("run-explicit"));
            }
            _ => panic!("expected team-members command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
    }

    #[test]
    fn parse_team_members_accepts_team_id_flag_without_run() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-env");
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-env");
        }
        let args = vec![
            "team-members".to_string(),
            "--team-id".to_string(),
            "team-explicit".to_string(),
        ];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse team-members");
        match parsed {
            ActorCommand::TeamMembers { team_id, run_id } => {
                assert_eq!(team_id.as_deref(), Some("team-explicit"));
                assert!(run_id.is_none());
            }
            _ => panic!("expected team-members command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
    }

    #[test]
    fn parse_inbox_ignores_legacy_run_env_alias() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::remove_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV);
            std::env::set_var("AGENTHUB_ACTOR_RUN_ID", "run-legacy-only");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
        }
        let args = vec!["inbox".to_string()];
        let err = match parse_actor_command(&args, &mut ActorOutputMode::Default) {
            Ok(_) => panic!("legacy run env alias should be ignored"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("run_id is required"),
            "unexpected error: {err}"
        );
        unsafe {
            std::env::remove_var("AGENTHUB_ACTOR_RUN_ID");
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_send_validates_remote_route() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-x");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "remote-peer".to_string(),
            "--transport".to_string(),
            "remote".to_string(),
            "--text".to_string(),
            "hi".to_string(),
        ];
        assert!(
            parse_actor_command(&args, &mut ActorOutputMode::Default).is_err(),
            "remote transport must require route-json"
        );
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_generates_default_idempotency_key() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-default-key");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "reviewer".to_string(),
            "--text".to_string(),
            "hello".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse send");
        match parsed {
            ActorCommand::Send {
                idempotency_key, ..
            } => {
                let idempotency_key = idempotency_key.expect("default idempotency key");
                assert!(idempotency_key.starts_with("auto:v1:"));
            }
            _ => panic!("expected send command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_allow_duplicate_disables_default_idempotency_key() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-allow-duplicate");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "reviewer".to_string(),
            "--text".to_string(),
            "hello".to_string(),
            "--allow-duplicate".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse send");
        match parsed {
            ActorCommand::Send {
                idempotency_key, ..
            } => {
                assert!(
                    idempotency_key.is_none(),
                    "allow duplicate should skip idempotency key"
                );
            }
            _ => panic!("expected send command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_rejects_duplicate_flag_with_explicit_idempotency_key() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-duplicate-invalid");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "reviewer".to_string(),
            "--text".to_string(),
            "hello".to_string(),
            "--idempotency-key".to_string(),
            "k-1".to_string(),
            "--allow-duplicate".to_string(),
        ];
        assert!(
            parse_actor_command(&args, &mut ActorOutputMode::Default).is_err(),
            "allow duplicate and idempotency key should conflict"
        );
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_inbox_accepts_agent_id_alias_flag() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-x");
            std::env::remove_var(ACTOR_RUNTIME_ACTOR_ID_ENV);
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "inbox".to_string(),
            "--agent-id".to_string(),
            "planner-agent".to_string(),
        ];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse inbox");
        match parsed {
            ActorCommand::Inbox { actor_id, .. } => {
                assert_eq!(actor_id, "planner-agent");
            }
            _ => panic!("expected inbox command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_inbox_uses_agent_id_env_fallback() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-x");
            std::env::remove_var(ACTOR_RUNTIME_ACTOR_ID_ENV);
            std::env::set_var(ACTOR_RUNTIME_AGENT_ID_ENV, "planner-agent");
        }
        let args = vec!["inbox".to_string()];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse inbox");
        match parsed {
            ActorCommand::Inbox { actor_id, .. } => {
                assert_eq!(actor_id, "planner-agent");
            }
            _ => panic!("expected inbox command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_accepts_agent_id_alias_flags() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-alias");
            std::env::remove_var(ACTOR_RUNTIME_ACTOR_ID_ENV);
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--from-agent-id".to_string(),
            "leader-agent".to_string(),
            "--to-agent-id".to_string(),
            "worker-agent".to_string(),
            "--text".to_string(),
            "hello".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse send");
        match parsed {
            ActorCommand::Send {
                from_actor_id,
                to_actor_id,
                channel_id,
                payload_source,
                ..
            } => {
                assert_eq!(from_actor_id, "leader-agent");
                assert_eq!(to_actor_id.as_deref(), Some("worker-agent"));
                assert!(channel_id.is_none());
                assert_eq!(payload_source, ActorSendPayloadSource::Text);
            }
            _ => panic!("expected send command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_accepts_text_and_preserves_markdown() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-markdown");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let markdown = "## Review\n\n- keep markdown\n- keep spacing\n";
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--text".to_string(),
            markdown.to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse send");
        match parsed {
            ActorCommand::Send {
                payload,
                payload_source,
                ..
            } => {
                assert_eq!(*payload, Value::String(markdown.to_string()));
                assert_eq!(payload_source, ActorSendPayloadSource::Text);
            }
            _ => panic!("expected send command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_rejects_text_and_payload_json_together() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-conflict");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--text".to_string(),
            "hello".to_string(),
            "--payload-json".to_string(),
            r#"{"text":"hello"}"#.to_string(),
        ];
        let err = match parse_actor_command(&args, &mut ActorOutputMode::Default) {
            Ok(_) => panic!("text and payload should conflict"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("--text and --payload-json"));
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_payload_json_marks_payload_source_for_warning() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-payload");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--payload-json".to_string(),
            r#"{"status":"done","result":"ok"}"#.to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse send");
        match parsed {
            ActorCommand::Send { payload_source, .. } => {
                assert_eq!(payload_source, ActorSendPayloadSource::Payload);
            }
            _ => panic!("expected send command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_accepts_channel_id_target() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-channel");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--channel-id".to_string(),
            "all".to_string(),
            "--text".to_string(),
            "@worker review this".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse send");
        match parsed {
            ActorCommand::Send {
                to_actor_id,
                channel_id,
                payload_source,
                ..
            } => {
                assert!(to_actor_id.is_none());
                assert_eq!(channel_id.as_deref(), Some("all"));
                assert_eq!(payload_source, ActorSendPayloadSource::Text);
            }
            _ => panic!("expected send command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_rejects_conflicting_actor_and_channel_targets() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-conflict-target");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--channel-id".to_string(),
            "all".to_string(),
            "--text".to_string(),
            "hello".to_string(),
        ];
        let err = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect_err("conflicting send targets should fail");
        assert!(err.to_string().contains("cannot be used together"));
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_help_command_is_supported() {
        for arg in ["help", "--help", "-h"] {
            let args = vec![arg.to_string()];
            let parsed =
                parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse help");
            assert!(matches!(parsed, ActorCommand::Help));
        }
    }

    #[test]
    fn parse_team_tasks_uses_env_fallback_and_status_filter() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-kanban");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "team-tasks".to_string(),
            "--status".to_string(),
            "in_review".to_string(),
            "--include-shared-thread".to_string(),
        ];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse team-tasks");
        match parsed {
            ActorCommand::TeamTasks {
                team_id,
                actor_id,
                status,
                include_shared_thread,
                ..
            } => {
                assert_eq!(team_id, "team-kanban");
                assert_eq!(actor_id, "leader");
                assert_eq!(status, Some(TeamTaskStatus::InReview));
                assert!(include_shared_thread);
            }
            _ => panic!("expected team-tasks command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_team_task_create_accepts_context_and_status() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-create");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "team-task-create".to_string(),
            "--title".to_string(),
            "Investigate relay drift".to_string(),
            "--status".to_string(),
            "in_progress".to_string(),
            "--context-json".to_string(),
            r#"{"area":"relay"}"#.to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse team-task-create");
        match parsed {
            ActorCommand::TeamTaskCreate {
                team_id,
                actor_id,
                title,
                status,
                context,
                ..
            } => {
                assert_eq!(team_id, "team-create");
                assert_eq!(actor_id, "leader");
                assert_eq!(title, "Investigate relay drift");
                assert_eq!(status, TeamTaskStatus::InProgress);
                assert_eq!(context["area"], "relay");
            }
            _ => panic!("expected team-task-create command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_time_trigger_set_uses_actor_env_fallback() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "worker");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "time-trigger-set".to_string(),
            "--delay-seconds".to_string(),
            "120".to_string(),
            "--message".to_string(),
            "follow up".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse time-trigger-set");
        match parsed {
            ActorCommand::TimeTriggerSet {
                actor_id,
                delay_seconds,
                message,
            } => {
                assert_eq!(actor_id, "worker");
                assert_eq!(delay_seconds, 120);
                assert_eq!(message, "follow up");
            }
            _ => panic!("expected time-trigger-set command"),
        }
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn compute_time_trigger_fire_at_adds_future_safety_margin() {
        assert_eq!(compute_time_trigger_fire_at(1_700_000_000, 1), 1_700_000_002);
        assert_eq!(
            compute_time_trigger_fire_at(1_700_000_000, MAX_TIME_TRIGGER_DELAY_SECONDS),
            1_700_000_000
                + MAX_TIME_TRIGGER_DELAY_SECONDS
                + TIME_TRIGGER_FUTURE_SAFETY_MARGIN_SECONDS
        );
    }

    #[test]
    fn parse_permission_review_respond_rejects_conflicting_outcome_flags() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-review");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "permission-review-respond".to_string(),
            "--permission-id".to_string(),
            "perm-1".to_string(),
            "--option-id".to_string(),
            "allow".to_string(),
            "--outcome".to_string(),
            "cancelled".to_string(),
        ];
        let err = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect_err("conflicting permission review flags should fail");
        assert!(err.to_string().contains("cannot be used together"));
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[derive(Serialize)]
    struct OutputFixture {
        name: &'static str,
        count: i32,
    }

    #[test]
    fn encode_actor_output_defaults_read_results_to_toon() {
        let output = encode_actor_output(
            &OutputFixture {
                name: "alpha",
                count: 2,
            },
            ActorOutputMode::Default,
            ActorOutputPreference::ToonPreferred,
        )
        .expect("encode toon output");
        assert!(output.contains("name: alpha"));
        assert!(output.contains("count: 2"));
        assert!(!output.starts_with('{'));
    }

    #[test]
    fn encode_actor_output_defaults_confirmation_results_to_json() {
        let output = encode_actor_output(
            &OutputFixture {
                name: "alpha",
                count: 2,
            },
            ActorOutputMode::Default,
            ActorOutputPreference::JsonPreferred,
        )
        .expect("encode json output");
        assert_eq!(output, r#"{"name":"alpha","count":2}"#);
    }

    #[test]
    fn encode_actor_output_json_flag_forces_json() {
        let output = encode_actor_output(
            &OutputFixture {
                name: "alpha",
                count: 2,
            },
            ActorOutputMode::Json,
            ActorOutputPreference::ToonPreferred,
        )
        .expect("encode forced json output");
        assert_eq!(output, r#"{"name":"alpha","count":2}"#);
    }

    #[test]
    fn encode_actor_output_keeps_inbox_cursor_visible() {
        let output = encode_actor_output(
            &ActorInboxResponse {
                messages: Vec::new(),
                next_cursor: Some(42),
            },
            ActorOutputMode::Default,
            ActorOutputPreference::ToonPreferred,
        )
        .expect("encode inbox response");
        assert!(output.contains("next_cursor: 42"));
    }
}
