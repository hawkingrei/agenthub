use agent_client_protocol::{RequestPermissionOutcome, SelectedPermissionOutcome};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, ActorAckRequest, ActorInboxRequest, ActorInboxResponse,
    ActorMailboxService, ActorMessageStatus, ActorServiceError, actor_inbox_with_auto_ack,
    build_default_actor_channel_idempotency_key, build_default_actor_message_idempotency_key,
    parse_actor_transport,
};
use anyhow::Context;
use chrono::Utc;
use serde_json::Value;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use crate::acp::{AcpPermissionRespondResult, AcpPermissionService};
use crate::actor_runtime_env::{
    ACTOR_RUNTIME_ACTOR_ID_ENV, ACTOR_RUNTIME_AGENT_ID_ENV, ACTOR_RUNTIME_CHANNEL_ENV,
    ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, ACTOR_RUNTIME_TEAM_ID_ENV,
    connect_runtime_internal_mailbox_service, maybe_remote_mailbox_service, normalized_env_var,
};
use crate::agent::AGENT_NODE_MAIN_ID;
use crate::agent::{AgentTimeTriggerCreateInput, AgentTimeTriggerManager};
use crate::internal::auth::InternalAction;
use crate::internal::client::{InternalGrpcMailboxClient, InternalGrpcPeerClientConfig};
use crate::internal::tls::{InternalGrpcSecurityMode, ensure_shared_secret, ensure_tls_material};
use crate::team::{
    TEAM_TASK_STATUS_VALUES, TeamActorMessageTransport, TeamManager, TeamTaskStatus,
    build_actor_mailbox_immediate_hint_prompt, plan_actor_mailbox_immediate_hint,
};

const TEAM_SHARED_THREAD_TITLE: &str = "all";
const TEAM_SHARED_THREAD_BOOTSTRAP_KIND: &str = "shared_thread";
const MAX_TIME_TRIGGER_DELAY_SECONDS: i64 = 30 * 24 * 60 * 60;
const TIME_TRIGGER_FUTURE_SAFETY_MARGIN_SECONDS: i64 = 1;
const ACTOR_HELP_TOPIC_INBOX: &str = "inbox";
const ACTOR_HELP_TOPIC_ACK: &str = "ack";
const ACTOR_HELP_TOPIC_SEND: &str = "send";
const ACTOR_HELP_TOPIC_PERMISSION_REVIEW_RESPOND: &str = "permission-review-respond";
const ACTOR_HELP_TOPICS: &[&str] = &[
    "team-members",
    "team-tasks",
    "team-task-create",
    "team-task-update",
    ACTOR_HELP_TOPIC_INBOX,
    ACTOR_HELP_TOPIC_ACK,
    ACTOR_HELP_TOPIC_SEND,
    "time-trigger-set",
    "time-trigger-list",
    "time-trigger-cancel",
    ACTOR_HELP_TOPIC_PERMISSION_REVIEW_RESPOND,
];

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
    Help {
        topic: Option<&'static str>,
    },
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
        auto_ack: bool,
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

fn is_help_flag(arg: &str) -> bool {
    matches!(arg.trim(), "--help" | "-h")
}

fn is_help_subcommand(arg: &str) -> bool {
    arg.trim() == "help"
}

fn normalize_help_topic(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn resolve_actor_help_topic(raw: &str) -> anyhow::Result<&'static str> {
    let normalized = normalize_help_topic(raw);
    anyhow::ensure!(
        !normalized.is_empty(),
        "actor help topic must be a non-empty string"
    );
    let mut matches = ACTOR_HELP_TOPICS
        .iter()
        .copied()
        .filter(|topic| {
            let topic_normalized = normalize_help_topic(topic);
            topic_normalized == normalized || topic_normalized.starts_with(&normalized)
        })
        .collect::<Vec<_>>();
    matches.sort_unstable();
    matches.dedup();
    match matches.as_slice() {
        [topic] => Ok(*topic),
        [] => Err(anyhow::anyhow!(
            "unknown actor help topic '{}'; try one of: {}",
            raw.trim(),
            ACTOR_HELP_TOPICS.join(", ")
        )),
        _ => Err(anyhow::anyhow!(
            "ambiguous actor help topic '{}'; matches: {}",
            raw.trim(),
            matches.join(", ")
        )),
    }
}

fn actor_usage() -> String {
    format!(
        "Usage:\n  agenthub actor help [topic]\n  agenthub actor [--json] team-members [--team-id <team_id>] [--run-id <run_id>]\n  agenthub actor [--json] team-tasks [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--limit <n>] [--status <all|open|in_progress|in_review|completed|canceled>] [--include-shared-thread]\n  agenthub actor [--json] team-task-create --title <title> [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--status <open|in_progress|in_review|completed|canceled>] [--topic <topic>] [--context-json <json>]\n  agenthub actor [--json] team-task-update --task-id <task_id> --status <open|in_progress|in_review|completed|canceled> [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>]\n  agenthub actor [--json] inbox [--run-id <run_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--limit <n>] [--after-id <id>] [--include-delivered] [--auto-ack]\n  agenthub actor [--json] ack --message-id <id> [--run-id <run_id>] [--actor-id <actor_id> | --agent-id <agent_id>]\n  agenthub actor [--json] send (--to-actor-id <actor_id> | --to-agent-id <agent_id> | --channel-id <channel_id>) (--text <markdown> | --payload-json <json>) [--run-id <run_id>] [--from-actor-id <actor_id> | --from-agent-id <agent_id>] [--channel <name>] [--transport <local|remote>] [--route-json <json>] [--idempotency-key <key>] [--allow-duplicate]\n  agenthub actor [--json] time-trigger-set --delay-seconds <seconds> --message <text> [--actor-id <actor_id> | --agent-id <agent_id>]\n  agenthub actor [--json] time-trigger-list [--actor-id <actor_id> | --agent-id <agent_id>] [--limit <n>]\n  agenthub actor [--json] time-trigger-cancel --trigger-id <trigger_id> [--actor-id <actor_id> | --agent-id <agent_id>]\n  agenthub actor [--json] permission-review-respond --permission-id <id> [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--option-id <option_id> | --outcome cancelled]\n\nQuick start:\n  agenthub actor inbox\n  agenthub actor ack --message-id <id>\n  agenthub actor send --to-actor-id <actor_id> --text \"<markdown>\"\n\nHelp:\n  `agenthub actor help inbox`\n  `agenthub actor help perm`\n  `agenthub actor ack --help`\n  Topic matching is fuzzy for help only; command execution remains strict.\n\nOutput:\n  Read-heavy results (`team-members`, `team-tasks`, `inbox`, `time-trigger-list`) default to TOON on stdout.\n  Human-oriented task and trigger confirmations (`team-task-create`, `team-task-update`, `time-trigger-set`, `time-trigger-cancel`) default to TOON on stdout.\n  Machine-oriented confirmations (`ack`, `send`, `permission-review-respond`) default to compact JSON for script compatibility.\n  `--json` forces JSON output for all structured success results.\n\nMailbox note:\n  `actor inbox` is read-only by default. Use `actor ack` to mark consumed messages delivered, or pass `--auto-ack` explicitly when you want inbox reads to consume pending messages.\n\nEnvironment fallback:\n  {}\n  {}\n  {}\n  {}\n  {}\n",
        ACTOR_RUNTIME_TEAM_ID_ENV,
        ACTOR_RUNTIME_CURRENT_RUN_ID_ENV,
        ACTOR_RUNTIME_ACTOR_ID_ENV,
        ACTOR_RUNTIME_AGENT_ID_ENV,
        ACTOR_RUNTIME_CHANNEL_ENV,
    )
}

fn actor_topic_usage(topic: &str) -> String {
    match topic {
        ACTOR_HELP_TOPIC_INBOX => "Usage:\n  agenthub actor inbox [--run-id <run_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--limit <n>] [--after-id <id>] [--include-delivered] [--auto-ack]\n\nExamples:\n  agenthub actor inbox\n  agenthub actor inbox --include-delivered\n  agenthub actor inbox --auto-ack\n\nNotes:\n  `actor inbox` is read-only by default.\n  Use `--auto-ack` only when you want inbox reads to consume pending messages in bulk.\n  In team runtime, omitting `--run-id` and `--actor-id` uses actor runtime env fallback.\n".to_string(),
        ACTOR_HELP_TOPIC_ACK => "Usage:\n  agenthub actor ack --message-id <id> [--run-id <run_id>] [--actor-id <actor_id> | --agent-id <agent_id>]\n\nExamples:\n  agenthub actor ack --message-id 123\n  agenthub actor ack --run-id <run_id> --actor-id <actor_id> --message-id 123\n\nNotes:\n  `actor ack` marks one mailbox message delivered after you have processed it.\n  In team runtime, omitting `--run-id` and `--actor-id` uses actor runtime env fallback.\n".to_string(),
        ACTOR_HELP_TOPIC_SEND => "Usage:\n  agenthub actor send (--to-actor-id <actor_id> | --to-agent-id <agent_id> | --channel-id <channel_id>) (--text <markdown> | --payload-json <json>) [--run-id <run_id>] [--from-actor-id <actor_id> | --from-agent-id <agent_id>] [--channel <name>] [--transport <local|remote>] [--route-json <json>] [--idempotency-key <key>] [--allow-duplicate]\n\nExamples:\n  agenthub actor send --to-actor-id reviewer --text \"Please review this.\"\n  agenthub actor send --channel-id all --text \"@leader build passed\"\n\nNotes:\n  Prefer `--text` for markdown-rich coordination messages.\n  Use `--payload-json` only for structured machine-readable envelopes.\n".to_string(),
        ACTOR_HELP_TOPIC_PERMISSION_REVIEW_RESPOND => "Usage:\n  agenthub actor permission-review-respond --permission-id <id> [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--option-id <option_id> | --outcome cancelled]\n\nExamples:\n  agenthub actor permission-review-respond --permission-id <id> --option-id allow\n  agenthub actor permission-review-respond --permission-id <id> --outcome cancelled\n\nNotes:\n  This command is for the currently assigned reviewer only.\n  In team runtime, review writes should go through local authority internal gRPC instead of direct sqlite writes.\n".to_string(),
        "team-members" => "Usage:\n  agenthub actor team-members [--team-id <team_id>] [--run-id <run_id>]\n".to_string(),
        "team-tasks" => "Usage:\n  agenthub actor team-tasks [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--limit <n>] [--status <all|open|in_progress|in_review|completed|canceled>] [--include-shared-thread]\n".to_string(),
        "team-task-create" => "Usage:\n  agenthub actor team-task-create --title <title> [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--status <open|in_progress|in_review|completed|canceled>] [--topic <topic>] [--context-json <json>]\n".to_string(),
        "team-task-update" => "Usage:\n  agenthub actor team-task-update --task-id <task_id> --status <open|in_progress|in_review|completed|canceled> [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>]\n".to_string(),
        "time-trigger-set" => "Usage:\n  agenthub actor time-trigger-set --delay-seconds <seconds> --message <text> [--actor-id <actor_id> | --agent-id <agent_id>]\n".to_string(),
        "time-trigger-list" => "Usage:\n  agenthub actor time-trigger-list [--actor-id <actor_id> | --agent-id <agent_id>] [--limit <n>]\n".to_string(),
        "time-trigger-cancel" => "Usage:\n  agenthub actor time-trigger-cancel --trigger-id <trigger_id> [--actor-id <actor_id> | --agent-id <agent_id>]\n".to_string(),
        _ => actor_usage(),
    }
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

fn actor_output_preference_for_command(command: &ActorCommand) -> ActorOutputPreference {
    match command {
        ActorCommand::Help { .. } => ActorOutputPreference::ToonPreferred,
        ActorCommand::TeamMembers { .. }
        | ActorCommand::TeamTasks { .. }
        | ActorCommand::Inbox { .. }
        | ActorCommand::TeamTaskCreate { .. }
        | ActorCommand::TeamTaskUpdate { .. }
        | ActorCommand::TimeTriggerList { .. }
        | ActorCommand::TimeTriggerSet { .. }
        | ActorCommand::TimeTriggerCancel { .. } => ActorOutputPreference::ToonPreferred,
        ActorCommand::Ack { .. }
        | ActorCommand::Send { .. }
        | ActorCommand::PermissionReviewRespond { .. } => ActorOutputPreference::JsonPreferred,
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
    peer_client: Option<InternalGrpcPeerClientConfig>,
) {
    if let Some(peer_client) = peer_client {
        manager.configure_internal_grpc_relay(
            PathBuf::from(&peer_client.cert_dir).as_path(),
            peer_client.security_mode,
        );
        manager.configure_internal_grpc_peer_client(Some(peer_client));
    }
}

fn build_actor_cli_internal_grpc_peer_client(
    config: &agenthub_config::AppConfig,
) -> anyhow::Result<Option<InternalGrpcPeerClientConfig>> {
    if !config.internal_grpc_enabled() {
        return Ok(None);
    }
    let cert_dir = PathBuf::from(config.internal_grpc_cert_dir());
    let security_mode = InternalGrpcSecurityMode::parse(&config.internal_grpc_security_mode())?;
    let shared_secret = ensure_shared_secret(&cert_dir, config.internal_grpc_auth_shared_secret())?;
    let _ = ensure_tls_material(&cert_dir, security_mode)?;
    Ok(Some(InternalGrpcPeerClientConfig {
        shared_secret,
        expected_issuer: config.internal_grpc_auth_issuer(),
        expected_audience: config.internal_grpc_auth_audience(),
        source_node_id: AGENT_NODE_MAIN_ID.to_string(),
        cert_dir: cert_dir.to_string_lossy().to_string(),
        security_mode,
    }))
}

fn actor_cli_internal_grpc_hint_target(listen_addr: &str) -> Option<String> {
    let parsed = listen_addr.parse::<SocketAddr>().ok()?;
    Some(format!("https://127.0.0.1:{}", parsed.port()))
}

async fn init_actor_mailbox_hint_client_from_config(
    config: &agenthub_config::AppConfig,
) -> anyhow::Result<Option<InternalGrpcMailboxClient>> {
    match maybe_remote_mailbox_service().await {
        Ok(Some(client)) => return Ok(Some(client)),
        Ok(None) => {}
        Err(err) => {
            tracing::debug!(
                "skip mailbox hint client because remote mailbox service init failed: {err}"
            );
            return Ok(None);
        }
    }
    let Some(peer_client) = (match build_actor_cli_internal_grpc_peer_client(config) {
        Ok(peer_client) => peer_client,
        Err(err) => {
            tracing::debug!(
                "skip mailbox hint client because internal grpc peer config is unavailable: {err}"
            );
            return Ok(None);
        }
    }) else {
        return Ok(None);
    };
    let Some(target) = actor_cli_internal_grpc_hint_target(&config.internal_grpc_listen_addr())
    else {
        tracing::debug!(
            listen_addr = %config.internal_grpc_listen_addr(),
            "skip mailbox hint client because internal grpc listen address is not a socket address"
        );
        return Ok(None);
    };
    let client = match InternalGrpcMailboxClient::connect_peer(
        &peer_client,
        &target,
        Some("localhost"),
        vec![InternalAction::AgentManage.as_str().to_string()],
    )
    .await
    {
        Ok(client) => client,
        Err(err) => {
            tracing::debug!(
                target = %target,
                "skip mailbox hint client because internal grpc connect failed: {err}"
            );
            return Ok(None);
        }
    };
    Ok(Some(client))
}

async fn maybe_notify_actor_new_mailbox_message_type_from_cli(
    manager: &TeamManager,
    config: &agenthub_config::AppConfig,
    send_result: &agenthub_team_actor::ActorSendResponse,
) -> anyhow::Result<()> {
    let Some(plan) = plan_actor_mailbox_immediate_hint(
        manager,
        send_result.message.run_id.as_str(),
        send_result,
    )
    .await?
    else {
        return Ok(());
    };
    let Some(client) = init_actor_mailbox_hint_client_from_config(config).await? else {
        tracing::debug!(
            run_id = %send_result.message.run_id,
            targets = ?plan.target_actor_ids,
            reason = ?plan.reason,
            "skip mailbox hint push because no agent input channel is available"
        );
        return Ok(());
    };
    let prompt =
        build_actor_mailbox_immediate_hint_prompt(send_result.message.run_id.as_str(), plan.reason);
    for target_actor_id in plan.target_actor_ids {
        if let Err(err) = client
            .send_agent_input(&target_actor_id, &prompt, None, None)
            .await
        {
            tracing::debug!(
                run_id = %send_result.message.run_id,
                actor_id = %target_actor_id,
                reason = ?plan.reason,
                "skip mailbox hint push because agent input is unavailable: {}",
                err
            );
        }
    }
    Ok(())
}

async fn init_team_manager() -> anyhow::Result<(TeamManager, agenthub_config::AppConfig)> {
    let db = agenthub_db::init_db().await?;
    let manager = TeamManager::new(db);
    let (config, _) = agenthub_config::AppConfig::load_with_info()?;
    let peer_client = build_actor_cli_internal_grpc_peer_client(&config)?;
    configure_actor_cli_internal_grpc(&manager, peer_client);
    Ok((manager, config))
}

fn actor_runtime_internal_control_requested() -> bool {
    normalized_env_var(ACTOR_RUNTIME_ACTOR_ID_ENV).is_some()
        && normalized_env_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).is_some()
}

async fn init_actor_mailbox_service(
    manager: &TeamManager,
    config: &agenthub_config::AppConfig,
    actor_id: &str,
    run_id: &str,
) -> anyhow::Result<Arc<dyn ActorMailboxService>> {
    if actor_runtime_internal_control_requested() {
        let client = connect_runtime_internal_mailbox_service(
            actor_id,
            Some(run_id),
            &[
                InternalAction::MessageSend,
                InternalAction::InboxList,
                InternalAction::MessageAck,
            ],
        )
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "team runtime mailbox control is unavailable because internal gRPC is not configured"
            )
        })?;
        return Ok(Arc::new(client));
    }
    let _ = config;
    Ok(Arc::new(manager.actor_mailbox_service()))
}

async fn init_actor_permission_review_client(
    actor_id: &str,
) -> anyhow::Result<Option<InternalGrpcMailboxClient>> {
    if !actor_runtime_internal_control_requested() {
        return Ok(None);
    }
    connect_runtime_internal_mailbox_service(
        actor_id,
        normalized_env_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).as_deref(),
        &[InternalAction::PermissionReview],
    )
    .await
}

async fn load_actor_inbox<S: ActorMailboxService + ?Sized>(
    service: &S,
    request: ActorInboxRequest,
    auto_ack: bool,
) -> Result<ActorInboxResponse, ActorServiceError> {
    if auto_ack {
        actor_inbox_with_auto_ack(service, request).await
    } else {
        service.actor_inbox(request).await
    }
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

fn take_mailbox_actor_id(value: Option<String>) -> anyhow::Result<String> {
    take_required_with_env_keys(value, &[ACTOR_RUNTIME_ACTOR_ID_ENV], "actor_id")
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
    if task
        .title
        .trim()
        .eq_ignore_ascii_case(TEAM_SHARED_THREAD_TITLE)
    {
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
    if is_help_subcommand(sub) || is_help_flag(sub) {
        let mut topic = None;
        for arg in args.iter().skip(1) {
            if arg == "--json" {
                *output_mode = ActorOutputMode::Json;
                continue;
            }
            if is_help_flag(arg) {
                continue;
            }
            anyhow::ensure!(
                topic.is_none(),
                "actor help accepts at most one topic argument"
            );
            topic = Some(resolve_actor_help_topic(arg)?);
        }
        return Ok(ActorCommand::Help { topic });
    }
    let positional_help = args
        .get(1)
        .is_some_and(|arg| is_help_subcommand(arg.as_str()));
    if positional_help || args.iter().skip(1).any(|arg| is_help_flag(arg)) {
        return Ok(ActorCommand::Help {
            topic: Some(resolve_actor_help_topic(sub)?),
        });
    }
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
                        return Err(anyhow::anyhow!(
                            "unknown flag for team-task-create: {}",
                            other
                        ));
                    }
                }
                idx += 1;
            }
            let title = take_optional(title).ok_or_else(|| anyhow::anyhow!("title is required"))?;
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
                        return Err(anyhow::anyhow!(
                            "unknown flag for team-task-update: {}",
                            other
                        ));
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
            let mut auto_ack = false;
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
                    "--auto-ack" => {
                        auto_ack = true;
                    }
                    other => {
                        return Err(anyhow::anyhow!("unknown flag for inbox: {}", other));
                    }
                }
                idx += 1;
            }
            Ok(ActorCommand::Inbox {
                run_id: take_run_id(run_id)?,
                actor_id: take_mailbox_actor_id(actor_id)?,
                limit: limit.max(1),
                after_id,
                include_delivered,
                auto_ack,
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
                actor_id: take_mailbox_actor_id(actor_id)?,
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
                &[ACTOR_RUNTIME_ACTOR_ID_ENV],
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
                        return Err(anyhow::anyhow!(
                            "unknown flag for time-trigger-set: {}",
                            other
                        ));
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
                        return Err(anyhow::anyhow!(
                            "unknown flag for time-trigger-list: {}",
                            other
                        ));
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
                        permission_id =
                            Some(args.get(idx).cloned().ok_or_else(|| {
                                anyhow::anyhow!("--permission-id requires a value")
                            })?);
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
    let output_preference = actor_output_preference_for_command(&command);
    match command {
        ActorCommand::Help { topic } => {
            let help = match topic {
                Some(topic) => actor_topic_usage(topic),
                None => actor_usage(),
            };
            println!("{help}");
        }
        ActorCommand::TeamMembers { team_id, run_id } => {
            let db = agenthub_db::init_db().await?;
            let manager = TeamManager::new(db);
            let team_context = manager
                .describe_team_context(team_id.as_deref(), run_id.as_deref())
                .await?;
            write_actor_output(&team_context, output_mode, output_preference)?;
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
            write_actor_output(&tasks, output_mode, output_preference)?;
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
                .create_task(
                    &team_id,
                    &title,
                    &actor_id,
                    context,
                    "group_chat",
                    topic.as_deref(),
                )
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
            write_actor_output(&output, output_mode, output_preference)?;
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
            write_actor_output(&task, output_mode, output_preference)?;
        }
        ActorCommand::Inbox {
            run_id,
            actor_id,
            limit,
            after_id,
            include_delivered,
            auto_ack,
        } => {
            let (manager, config) = init_team_manager().await?;
            let service = init_actor_mailbox_service(&manager, &config, &actor_id, &run_id).await?;
            let states = if include_delivered {
                Some(vec![
                    ActorMessageStatus::Pending,
                    ActorMessageStatus::Delivered,
                    ActorMessageStatus::DeadLetter,
                ])
            } else {
                Some(vec![ActorMessageStatus::Pending])
            };
            let inbox = load_actor_inbox(
                service.as_ref(),
                ActorInboxRequest {
                    run_id,
                    actor_id,
                    cursor: after_id,
                    limit: Some(limit),
                    states,
                },
                auto_ack,
            )
            .await
            .map_err(|err| map_actor_service_error("actor inbox", err))?;
            write_actor_output(&inbox, output_mode, output_preference)?;
        }
        ActorCommand::Ack {
            run_id,
            actor_id,
            message_id,
        } => {
            let (manager, config) = init_team_manager().await?;
            let service = init_actor_mailbox_service(&manager, &config, &actor_id, &run_id).await?;
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
            write_actor_output(&message, output_mode, output_preference)?;
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
            let (manager, config) = init_team_manager().await?;
            let service =
                init_actor_mailbox_service(&manager, &config, &from_actor_id, &run_id).await?;
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
            if let Err(err) =
                maybe_notify_actor_new_mailbox_message_type_from_cli(&manager, &config, &message)
                    .await
            {
                tracing::warn!(
                    run_id = %message.message.run_id,
                    message_id = message.message.message_id,
                    "failed to process actor mailbox type hint: {}",
                    err
                );
            }
            if payload_source == ActorSendPayloadSource::Payload {
                eprintln!(
                    "warning: prefer --text for markdown-rich mailbox messages; --payload-json is best reserved for structured machine-readable coordination"
                );
            }
            write_actor_output(&message, output_mode, output_preference)?;
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
            write_actor_output(&record, output_mode, output_preference)?;
        }
        ActorCommand::TimeTriggerList { actor_id, limit } => {
            let db = agenthub_db::init_db().await?;
            let manager = AgentTimeTriggerManager::new(db);
            let records = manager
                .list_triggers_for_agent(actor_id.as_str(), limit)
                .await?;
            write_actor_output(&records, output_mode, output_preference)?;
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
            write_actor_output(&output, output_mode, output_preference)?;
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
            if let Some(client) = init_actor_permission_review_client(&actor_id).await? {
                let response = client
                    .respond_permission_review(
                        &team_id,
                        &actor_id,
                        &permission_id,
                        option_id.as_deref(),
                        outcome.as_deref(),
                    )
                    .await?;
                write_actor_output(&response, output_mode, output_preference)?;
                return Ok(());
            }
            if record.status != "pending" {
                let output = serde_json::json!({
                    "status": "already_resolved",
                    "permission_id": permission_id,
                    "request_status": record.status,
                });
                write_actor_output(&output, output_mode, output_preference)?;
                return Ok(());
            }

            let response_outcome = if let Some(option_id) = option_id.as_ref() {
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                    option_id.clone(),
                ))
            } else {
                match outcome.as_deref() {
                    Some("cancelled") | None => RequestPermissionOutcome::Cancelled,
                    Some(other) => {
                        anyhow::bail!("unsupported outcome '{}', expected 'cancelled'", other);
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
                write_actor_output(&output, output_mode, output_preference)?;
                return Ok(());
            }
            let output = serde_json::json!({
                "status": "ok",
                "permission_id": permission_id,
                "reviewed_by_actor_id": actor_id,
            });
            write_actor_output(&output, output_mode, output_preference)?;
        }
    }
    Ok(())
}

fn maybe_reject_legacy_actor_mcp_args(args: &[String]) -> Option<anyhow::Result<()>> {
    if args.first().map(String::as_str) == Some("actor-mcp") {
        return Some(Err(anyhow::anyhow!(
            "`agenthub actor-mcp` has been removed. Use `agenthub actor ...` instead."
        )));
    }
    None
}

pub async fn maybe_run_from_args() -> Option<anyhow::Result<()>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Some(result) = maybe_reject_legacy_actor_mcp_args(&args) {
        return Some(result);
    }
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
    use agenthub_team_actor::{
        ActorAckRequest, ActorAckResponse, ActorInboxResponse, ActorSendRequest, ActorSendResponse,
    };
    use serde::Serialize;
    use std::sync::{Arc, Mutex as StdMutex, OnceLock};
    use tokio::sync::Mutex;

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn restore_env(key: &str, value: Option<String>) {
        if let Some(value) = value {
            unsafe { std::env::set_var(key, value) }
        } else {
            unsafe { std::env::remove_var(key) }
        }
    }

    #[derive(Clone)]
    struct MockMailboxService {
        inbox: Vec<agenthub_team_actor::ActorMessageRecord>,
        acked_ids: Arc<StdMutex<Vec<i64>>>,
    }

    #[async_trait::async_trait]
    impl ActorMailboxService for MockMailboxService {
        async fn actor_send(
            &self,
            _request: ActorSendRequest,
        ) -> Result<ActorSendResponse, ActorServiceError> {
            unreachable!("send is not used in this test")
        }

        async fn actor_inbox(
            &self,
            _request: ActorInboxRequest,
        ) -> Result<ActorInboxResponse, ActorServiceError> {
            Ok(ActorInboxResponse {
                messages: self.inbox.clone(),
                next_cursor: self.inbox.last().map(|item| item.message_id),
                pending_count: self
                    .inbox
                    .iter()
                    .filter(|message| message.status == ActorMessageStatus::Pending)
                    .count() as i64,
            })
        }

        async fn actor_ack(
            &self,
            request: ActorAckRequest,
        ) -> Result<ActorAckResponse, ActorServiceError> {
            self.acked_ids
                .lock()
                .expect("acquire acked_ids mutex")
                .push(request.message_id);
            let message = self
                .inbox
                .iter()
                .find(|item| item.message_id == request.message_id)
                .expect("find acked message")
                .clone();
            Ok(ActorAckResponse {
                message_id: message.message_id,
                state: ActorMessageStatus::Delivered,
                acked_at: 100,
                message: agenthub_team_actor::ActorMessageRecord {
                    status: ActorMessageStatus::Delivered,
                    delivered_at: Some(100),
                    ..message
                },
            })
        }
    }

    fn mock_inbox_message(
        message_id: i64,
        status: ActorMessageStatus,
    ) -> agenthub_team_actor::ActorMessageRecord {
        agenthub_team_actor::ActorMessageRecord {
            message_id,
            run_id: "run-1".to_string(),
            from_actor_id: "leader".to_string(),
            from_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            from_actor_kind: agenthub_team_actor::ActorIdentityKind::Agent,
            to_actor_id: "worker".to_string(),
            to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            to_actor_kind: agenthub_team_actor::ActorIdentityKind::Agent,
            channel: "default".to_string(),
            transport: agenthub_team_actor::ActorMessageTransport::Local,
            route: None,
            payload: serde_json::json!({"type":"chat_message","text":"hello"}),
            status,
            created_at: 1,
            delivered_at: None,
        }
    }

    fn test_internal_grpc_config(
        listen: &str,
        cert_dir: &std::path::Path,
    ) -> agenthub_config::AppConfig {
        agenthub_config::AppConfig {
            internal_grpc: Some(agenthub_config::InternalGrpcConfig {
                enabled: Some(true),
                listen: Some(listen.to_string()),
                security: Some(agenthub_config::InternalGrpcSecurityConfig {
                    mode: Some("disabled".to_string()),
                    cert_dir: Some(cert_dir.to_string_lossy().to_string()),
                }),
                auth: None,
                bootstrap: None,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn parse_inbox_uses_env_fallback() {
        let _guard = env_lock().blocking_lock();
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
                auto_ack,
                ..
            } => {
                assert_eq!(run_id, "run-x");
                assert_eq!(actor_id, "planner");
                assert_eq!(limit, 5);
                assert!(!auto_ack);
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
    fn parse_inbox_accepts_auto_ack_flag() {
        let _guard = env_lock().blocking_lock();
        let prev_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-auto-ack");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "worker");
        }
        let args = vec!["inbox".to_string(), "--auto-ack".to_string()];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse inbox");
        match parsed {
            ActorCommand::Inbox { auto_ack, .. } => assert!(auto_ack),
            _ => panic!("expected inbox command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_team_members_uses_env_fallback() {
        let _guard = env_lock().blocking_lock();
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
        let _guard = env_lock().blocking_lock();
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
        let _guard = env_lock().blocking_lock();
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
        let _guard = env_lock().blocking_lock();
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
        let _guard = env_lock().blocking_lock();
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
        let _guard = env_lock().blocking_lock();
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
        let _guard = env_lock().blocking_lock();
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
        let _guard = env_lock().blocking_lock();
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
        let _guard = env_lock().blocking_lock();
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
    fn parse_inbox_rejects_agent_id_env_fallback() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-x");
            std::env::remove_var(ACTOR_RUNTIME_ACTOR_ID_ENV);
            std::env::set_var(ACTOR_RUNTIME_AGENT_ID_ENV, "planner-agent");
        }
        let args = vec!["inbox".to_string()];
        let err =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect_err("reject inbox");
        assert!(
            err.to_string().contains("actor_id is required"),
            "unexpected error: {err}"
        );
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_ack_rejects_agent_id_env_fallback() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-x");
            std::env::remove_var(ACTOR_RUNTIME_ACTOR_ID_ENV);
            std::env::set_var(ACTOR_RUNTIME_AGENT_ID_ENV, "planner-agent");
        }
        let args = vec![
            "ack".to_string(),
            "--message-id".to_string(),
            "42".to_string(),
        ];
        let err =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect_err("reject ack");
        assert!(
            err.to_string().contains("actor_id is required"),
            "unexpected error: {err}"
        );
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_accepts_agent_id_alias_flags() {
        let _guard = env_lock().blocking_lock();
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
    fn parse_send_rejects_agent_id_env_fallback_for_from_actor() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-env");
            std::env::remove_var(ACTOR_RUNTIME_ACTOR_ID_ENV);
            std::env::set_var(ACTOR_RUNTIME_AGENT_ID_ENV, "leader-agent");
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--text".to_string(),
            "hello".to_string(),
        ];
        let err =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect_err("reject send");
        assert!(
            err.to_string().contains("from_actor_id is required"),
            "unexpected error: {err}"
        );
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_accepts_text_and_preserves_markdown() {
        let _guard = env_lock().blocking_lock();
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
        let _guard = env_lock().blocking_lock();
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
        let _guard = env_lock().blocking_lock();
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
        let _guard = env_lock().blocking_lock();
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

    #[tokio::test]
    async fn actor_send_type_hint_is_best_effort_without_internal_grpc_client() {
        let _guard = env_lock().lock().await;
        let prev_target =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV).ok();
        let prev_token =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV).ok();
        unsafe {
            std::env::remove_var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV);
            std::env::remove_var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV);
        }
        let (manager, config) = init_team_manager().await.expect("init team manager");
        let send_result = agenthub_team_actor::ActorSendResponse {
            message_id: 42,
            state: agenthub_team_actor::ActorMessageStatus::Pending,
            deduped: false,
            created_at: 1_700_000_000,
            message: agenthub_team_actor::ActorMessageRecord {
                message_id: 42,
                run_id: "run-cli-hint".to_string(),
                from_actor_id: "leader".to_string(),
                from_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
                from_actor_kind: agenthub_team_actor::ActorIdentityKind::Agent,
                to_actor_id: "worker".to_string(),
                to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
                to_actor_kind: agenthub_team_actor::ActorIdentityKind::Agent,
                channel: "default".to_string(),
                transport: agenthub_team_actor::ActorMessageTransport::Local,
                route: None,
                payload: serde_json::json!({
                    "type": "worker_status",
                    "status": "ready"
                }),
                status: agenthub_team_actor::ActorMessageStatus::Pending,
                created_at: 1_700_000_000,
                delivered_at: None,
            },
        };
        maybe_notify_actor_new_mailbox_message_type_from_cli(&manager, &config, &send_result)
            .await
            .expect("best-effort mailbox hint should not fail");
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV,
            prev_target,
        );
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV,
            prev_token,
        );
    }

    #[tokio::test]
    async fn actor_runtime_internal_control_requested_requires_actor_and_run_env() {
        let _guard = env_lock().lock().await;
        let prev_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_target =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV).ok();
        let prev_token =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV).ok();
        unsafe {
            std::env::remove_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV);
            std::env::remove_var(ACTOR_RUNTIME_ACTOR_ID_ENV);
            std::env::set_var(
                crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV,
                "https://127.0.0.1:9",
            );
            std::env::set_var(
                crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV,
                "test-token",
            );
        }

        assert!(!actor_runtime_internal_control_requested());
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "worker");
        }
        assert!(!actor_runtime_internal_control_requested());
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-1");
        }
        assert!(actor_runtime_internal_control_requested());

        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV,
            prev_target,
        );
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV,
            prev_token,
        );
    }

    #[tokio::test]
    async fn load_actor_inbox_keeps_pending_messages_read_only_by_default() {
        let service = MockMailboxService {
            inbox: vec![mock_inbox_message(1, ActorMessageStatus::Pending)],
            acked_ids: Arc::new(StdMutex::new(Vec::new())),
        };
        let response = load_actor_inbox(
            &service,
            ActorInboxRequest {
                run_id: "run-1".to_string(),
                actor_id: "worker".to_string(),
                cursor: None,
                limit: Some(20),
                states: Some(vec![ActorMessageStatus::Pending]),
            },
            false,
        )
        .await
        .expect("load inbox without auto-ack");
        assert_eq!(response.pending_count, 1);
        assert_eq!(response.messages.len(), 1);
        assert_eq!(response.messages[0].status, ActorMessageStatus::Pending);
        assert!(
            service
                .acked_ids
                .lock()
                .expect("acquire acked ids")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn load_actor_inbox_auto_ack_consumes_pending_messages() {
        let service = MockMailboxService {
            inbox: vec![mock_inbox_message(7, ActorMessageStatus::Pending)],
            acked_ids: Arc::new(StdMutex::new(Vec::new())),
        };
        let response = load_actor_inbox(
            &service,
            ActorInboxRequest {
                run_id: "run-1".to_string(),
                actor_id: "worker".to_string(),
                cursor: None,
                limit: Some(20),
                states: Some(vec![ActorMessageStatus::Pending]),
            },
            true,
        )
        .await
        .expect("load inbox with auto-ack");
        assert_eq!(response.pending_count, 0);
        assert_eq!(response.messages.len(), 1);
        assert_eq!(response.messages[0].status, ActorMessageStatus::Delivered);
        assert_eq!(
            *service.acked_ids.lock().expect("acquire acked ids"),
            vec![7]
        );
    }

    #[test]
    fn legacy_actor_mcp_entrypoint_is_rejected() {
        let args = vec!["actor-mcp".to_string()];
        let err = maybe_reject_legacy_actor_mcp_args(&args)
            .expect("legacy actor-mcp should be rejected")
            .expect_err("legacy actor-mcp should return an error");
        assert_eq!(
            err.to_string(),
            "`agenthub actor-mcp` has been removed. Use `agenthub actor ...` instead."
        );
    }

    #[tokio::test]
    async fn init_actor_mailbox_hint_client_from_config_skips_missing_remote_token() {
        let _guard = env_lock().lock().await;
        let prev_target =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV).ok();
        let prev_token =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV).ok();
        unsafe {
            std::env::set_var(
                crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV,
                "https://127.0.0.1:50051",
            );
            std::env::remove_var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV);
        }
        assert!(
            init_actor_mailbox_hint_client_from_config(&agenthub_config::AppConfig::default())
                .await
                .expect("missing token should degrade to None")
                .is_none()
        );
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV,
            prev_target,
        );
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV,
            prev_token,
        );
    }

    #[tokio::test]
    async fn init_actor_mailbox_hint_client_from_config_skips_when_internal_grpc_disabled() {
        let _guard = env_lock().lock().await;
        let prev_target =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV).ok();
        let prev_token =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV).ok();
        unsafe {
            std::env::remove_var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV);
            std::env::remove_var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV);
        }
        assert!(
            init_actor_mailbox_hint_client_from_config(&agenthub_config::AppConfig::default())
                .await
                .expect("disabled internal grpc should return None")
                .is_none()
        );
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV,
            prev_target,
        );
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV,
            prev_token,
        );
    }

    #[tokio::test]
    async fn init_actor_mailbox_hint_client_from_config_skips_invalid_listen_addr() {
        let _guard = env_lock().lock().await;
        let prev_target =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV).ok();
        let prev_token =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV).ok();
        unsafe {
            std::env::remove_var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV);
            std::env::remove_var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV);
        }
        let tempdir = std::env::temp_dir().join(format!(
            "agenthub-actor-cli-invalid-listen-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&tempdir).expect("create temp cert dir");
        let config = test_internal_grpc_config("not-an-addr", &tempdir);
        assert!(
            init_actor_mailbox_hint_client_from_config(&config)
                .await
                .expect("invalid listen addr should return None")
                .is_none()
        );
        let _ = std::fs::remove_dir_all(&tempdir);
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV,
            prev_target,
        );
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV,
            prev_token,
        );
    }

    #[test]
    fn parse_send_rejects_conflicting_actor_and_channel_targets() {
        let _guard = env_lock().blocking_lock();
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
            assert!(matches!(parsed, ActorCommand::Help { topic: None }));
        }
    }

    #[test]
    fn parse_help_topic_supports_fuzzy_match() {
        let args = vec!["help".to_string(), "perm".to_string()];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse help topic");
        assert!(matches!(
            parsed,
            ActorCommand::Help {
                topic: Some(ACTOR_HELP_TOPIC_PERMISSION_REVIEW_RESPOND)
            }
        ));
    }

    #[test]
    fn parse_subcommand_help_is_supported() {
        let args = vec!["ack".to_string(), "--help".to_string()];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse subcommand help");
        assert!(matches!(
            parsed,
            ActorCommand::Help {
                topic: Some(ACTOR_HELP_TOPIC_ACK)
            }
        ));
    }

    #[test]
    fn parse_subcommand_positional_help_is_supported() {
        let args = vec!["ack".to_string(), "help".to_string()];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse positional subcommand help");
        assert!(matches!(
            parsed,
            ActorCommand::Help {
                topic: Some(ACTOR_HELP_TOPIC_ACK)
            }
        ));
    }

    #[test]
    fn parse_team_members_allows_help_as_flag_value() {
        let args = vec![
            "team-members".to_string(),
            "--team-id".to_string(),
            "help".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse team-members with help value");
        match parsed {
            ActorCommand::TeamMembers { team_id, run_id } => {
                assert_eq!(team_id.as_deref(), Some("help"));
                assert!(run_id.is_none());
            }
            other => panic!("expected team-members command, got {other:?}"),
        }
    }

    #[test]
    fn parse_team_tasks_uses_env_fallback_and_status_filter() {
        let _guard = env_lock().blocking_lock();
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
        let _guard = env_lock().blocking_lock();
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
        let _guard = env_lock().blocking_lock();
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
        assert_eq!(
            compute_time_trigger_fire_at(1_700_000_000, 1),
            1_700_000_002
        );
        assert_eq!(
            compute_time_trigger_fire_at(1_700_000_000, MAX_TIME_TRIGGER_DELAY_SECONDS),
            1_700_000_000
                + MAX_TIME_TRIGGER_DELAY_SECONDS
                + TIME_TRIGGER_FUTURE_SAFETY_MARGIN_SECONDS
        );
    }

    #[test]
    fn actor_cli_internal_grpc_hint_target_forces_loopback() {
        assert_eq!(
            actor_cli_internal_grpc_hint_target("0.0.0.0:50051").as_deref(),
            Some("https://127.0.0.1:50051")
        );
        assert_eq!(
            actor_cli_internal_grpc_hint_target("127.0.0.1:50052").as_deref(),
            Some("https://127.0.0.1:50052")
        );
        assert!(actor_cli_internal_grpc_hint_target("not-an-addr").is_none());
    }

    #[test]
    fn parse_permission_review_respond_rejects_conflicting_outcome_flags() {
        let _guard = env_lock().blocking_lock();
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
                pending_count: 3,
            },
            ActorOutputMode::Default,
            ActorOutputPreference::ToonPreferred,
        )
        .expect("encode inbox response");
        assert!(output.contains("next_cursor: 42"));
        assert!(output.contains("pending_count: 3"));
    }

    #[test]
    fn actor_output_preference_contract_covers_all_command_variants() {
        let cases = vec![
            (
                ActorCommand::Help {
                    topic: Some(ACTOR_HELP_TOPIC_INBOX),
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::TeamMembers {
                    team_id: Some("team-1".to_string()),
                    run_id: Some("run-1".to_string()),
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::TeamTasks {
                    team_id: "team-1".to_string(),
                    actor_id: "leader".to_string(),
                    status: Some(TeamTaskStatus::Open),
                    limit: 10,
                    include_shared_thread: true,
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::TeamTaskCreate {
                    team_id: "team-1".to_string(),
                    actor_id: "leader".to_string(),
                    title: "Create task".to_string(),
                    status: TeamTaskStatus::Open,
                    topic: None,
                    context: Value::Object(Default::default()),
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::TeamTaskUpdate {
                    team_id: "team-1".to_string(),
                    actor_id: "leader".to_string(),
                    task_id: "task-1".to_string(),
                    status: TeamTaskStatus::InProgress,
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::Inbox {
                    run_id: "run-1".to_string(),
                    actor_id: "worker".to_string(),
                    limit: 20,
                    after_id: None,
                    include_delivered: false,
                    auto_ack: false,
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::Ack {
                    run_id: "run-1".to_string(),
                    actor_id: "worker".to_string(),
                    message_id: 42,
                },
                ActorOutputPreference::JsonPreferred,
            ),
            (
                ActorCommand::Send {
                    run_id: "run-1".to_string(),
                    from_actor_id: "leader".to_string(),
                    to_actor_id: Some("worker".to_string()),
                    channel_id: None,
                    channel: "default".to_string(),
                    transport: TeamActorMessageTransport::Local,
                    route: None,
                    payload: Box::new(Value::String("hello".to_string())),
                    payload_source: ActorSendPayloadSource::Text,
                    idempotency_key: None,
                },
                ActorOutputPreference::JsonPreferred,
            ),
            (
                ActorCommand::TimeTriggerSet {
                    actor_id: "leader".to_string(),
                    delay_seconds: 60,
                    message: "follow up".to_string(),
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::TimeTriggerList {
                    actor_id: "leader".to_string(),
                    limit: 5,
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::TimeTriggerCancel {
                    actor_id: "leader".to_string(),
                    trigger_id: "trigger-1".to_string(),
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::PermissionReviewRespond {
                    team_id: "team-1".to_string(),
                    actor_id: "leader".to_string(),
                    permission_id: "perm-1".to_string(),
                    option_id: Some("allow".to_string()),
                    outcome: None,
                },
                ActorOutputPreference::JsonPreferred,
            ),
        ];

        for (command, expected) in cases {
            assert_eq!(
                actor_output_preference_for_command(&command),
                expected,
                "unexpected output preference for command variant: {command:?}"
            );
        }

        let toon_output = encode_actor_output(
            &OutputFixture {
                name: "alpha",
                count: 2,
            },
            ActorOutputMode::Default,
            actor_output_preference_for_command(&ActorCommand::TeamMembers {
                team_id: Some("team-1".to_string()),
                run_id: Some("run-1".to_string()),
            }),
        )
        .expect("encode default team-members output");
        assert!(toon_output.contains("name: alpha"));
        assert!(toon_output.contains("count: 2"));
        assert!(!toon_output.starts_with('{'));
    }

    #[test]
    fn json_flag_still_forces_team_members_json_output() {
        let output = encode_actor_output(
            &OutputFixture {
                name: "alpha",
                count: 2,
            },
            ActorOutputMode::Json,
            actor_output_preference_for_command(&ActorCommand::TeamMembers {
                team_id: Some("team-1".to_string()),
                run_id: Some("run-1".to_string()),
            }),
        )
        .expect("encode forced json team-members output");
        assert_eq!(output, r#"{"name":"alpha","count":2}"#);
    }
}
