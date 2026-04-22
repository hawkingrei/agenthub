use super::help::{actor_usage, is_help_flag, is_help_subcommand, resolve_actor_help_topic};
use super::{
    ActorCommand, ActorOutputMode, ActorSendIdempotency, ActorSendPayloadSource,
    ActorSendTargetRef, TIME_TRIGGER_FUTURE_SAFETY_MARGIN_SECONDS, TeamTaskNoteKind,
    build_actor_send_default_idempotency_key,
};
use std::fs;

use crate::actor_runtime_env::{
    ACTOR_RUNTIME_ACTOR_ID_ENV, ACTOR_RUNTIME_AGENT_ID_ENV, ACTOR_RUNTIME_CHANNEL_ENV,
    ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, ACTOR_RUNTIME_TEAM_ID_ENV, normalized_env_var,
};
use crate::team::{
    TEAM_TASK_STATUS_VALUES, TeamActorMessageTransport, TeamTaskListQuery, TeamTaskStatus,
};
use agenthub_team_actor::parse_actor_transport;
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) struct ParsedActorCommand {
    pub(super) output_mode: ActorOutputMode,
    pub(super) command: ActorCommand,
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

fn parse_json_file(path: &str, field: &str) -> anyhow::Result<Value> {
    let raw = fs::read_to_string(path)
        .map_err(|err| anyhow::anyhow!("failed to read {} file {}: {}", field, path, err))?;
    parse_json(&raw, field)
}

fn set_unique_json_value(
    slot: &mut Option<Value>,
    value: Value,
    conflict_message: &'static str,
) -> anyhow::Result<()> {
    if slot.is_some() {
        return Err(anyhow::anyhow!(conflict_message));
    }
    *slot = Some(value);
    Ok(())
}

fn parse_team_step_scope_flag(
    args: &[String],
    idx: &mut usize,
    current_flag: &str,
    run_id: &mut Option<String>,
    actor_id: &mut Option<String>,
    step_id: &mut Option<String>,
    runtime_handle_id: &mut Option<String>,
) -> anyhow::Result<bool> {
    match current_flag {
        "--run-id" => {
            *idx += 1;
            *run_id = Some(
                args.get(*idx)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("--run-id requires a value"))?,
            );
            Ok(true)
        }
        flag @ ("--actor-id" | "--agent-id") => {
            *idx += 1;
            *actor_id = Some(
                args.get(*idx)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?,
            );
            Ok(true)
        }
        "--step-id" => {
            *idx += 1;
            *step_id = Some(
                args.get(*idx)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("--step-id requires a value"))?,
            );
            Ok(true)
        }
        "--runtime-handle-id" | "--session-id" => {
            *idx += 1;
            *runtime_handle_id = Some(
                args.get(*idx)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("{current_flag} requires a value"))?,
            );
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn parse_team_step_scope_argument(
    args: &[String],
    idx: &mut usize,
    output_mode: &mut ActorOutputMode,
    run_id: &mut Option<String>,
    actor_id: &mut Option<String>,
    step_id: &mut Option<String>,
    runtime_handle_id: &mut Option<String>,
) -> anyhow::Result<bool> {
    let current_flag = args[*idx].as_str();
    if current_flag == "--json" {
        *output_mode = ActorOutputMode::Json;
        return Ok(true);
    }
    parse_team_step_scope_flag(
        args,
        idx,
        current_flag,
        run_id,
        actor_id,
        step_id,
        runtime_handle_id,
    )
}

fn resolve_team_run_scope(
    team_id: Option<String>,
    run_id: Option<String>,
) -> (Option<String>, Option<String>) {
    let explicit_team_id = take_optional(team_id);
    let explicit_run_id = take_optional(run_id);
    let resolved_team_id = explicit_team_id.clone().or_else(|| {
        if explicit_run_id.is_some() {
            return None;
        }
        normalized_env_var(ACTOR_RUNTIME_TEAM_ID_ENV)
    });
    let resolved_run_id = explicit_run_id.or_else(|| {
        if explicit_team_id.is_some() || resolved_team_id.is_some() {
            return None;
        }
        normalized_env_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV)
    });
    (resolved_team_id, resolved_run_id)
}

fn resolve_team_context_scope(
    team_id: Option<String>,
    run_id: Option<String>,
) -> (Option<String>, Option<String>) {
    let explicit_team_id = take_optional(team_id);
    let explicit_run_id = take_optional(run_id);
    let resolved_team_id = explicit_team_id.clone().or_else(|| {
        if explicit_run_id.is_some() {
            return None;
        }
        normalized_env_var(ACTOR_RUNTIME_TEAM_ID_ENV)
    });
    let resolved_run_id = explicit_run_id.or_else(|| {
        if explicit_team_id.is_some() {
            return None;
        }
        normalized_env_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV)
    });
    (resolved_team_id, resolved_run_id)
}

fn read_actor_send_file(path: &str, flag: &str) -> anyhow::Result<String> {
    let trimmed = path.trim();
    anyhow::ensure!(!trimmed.is_empty(), "{} requires a non-empty path", flag);
    fs::read_to_string(trimmed)
        .map_err(|err| anyhow::anyhow!("read {} '{}': {}", flag, trimmed, err))
}

fn resolve_actor_send_payload(
    text: Option<String>,
    text_file: Option<String>,
    payload: Option<Value>,
    payload_file: Option<String>,
) -> anyhow::Result<(Value, ActorSendPayloadSource)> {
    if text.is_some() && text_file.is_some() {
        return Err(anyhow::anyhow!(
            "--text and --text-file cannot be used together"
        ));
    }
    if payload.is_some() && payload_file.is_some() {
        return Err(anyhow::anyhow!(
            "--payload-json and --payload-file cannot be used together"
        ));
    }
    let text = match text_file {
        Some(path) => Some(read_actor_send_file(path.as_str(), "--text-file")?),
        None => text,
    };
    let payload = match payload_file {
        Some(path) => {
            let raw = read_actor_send_file(path.as_str(), "--payload-file")?;
            Some(parse_json(raw.as_str(), "--payload-file")?)
        }
        None => payload,
    };
    match (text, payload) {
        (Some(text), None) => {
            if text.trim().is_empty() {
                return Err(anyhow::anyhow!("text must be a non-empty string"));
            }
            Ok((Value::String(text), ActorSendPayloadSource::Text))
        }
        (None, Some(payload)) => Ok((payload, ActorSendPayloadSource::Payload)),
        (Some(_), Some(_)) => Err(anyhow::anyhow!(
            "--text/--text-file and --payload-json/--payload-file cannot be used together"
        )),
        (None, None) => Err(anyhow::anyhow!(
            "--text, --text-file, --payload-json, or --payload-file is required"
        )),
    }
}

fn normalize_actor_send_mentions(raw_mentions: Vec<String>) -> anyhow::Result<Vec<String>> {
    let mut mentions = Vec::new();
    let mut seen = BTreeSet::new();
    for raw in raw_mentions {
        let mention = raw.trim();
        anyhow::ensure!(
            !mention.is_empty(),
            "mention_actor_id must be a non-empty string"
        );
        if seen.insert(mention.to_string()) {
            mentions.push(mention.to_string());
        }
    }
    Ok(mentions)
}

fn merge_actor_send_mentions(
    payload: Value,
    mention_actor_ids: &[String],
) -> anyhow::Result<Value> {
    if mention_actor_ids.is_empty() {
        return Ok(payload);
    }
    let mention_values = mention_actor_ids
        .iter()
        .cloned()
        .map(Value::String)
        .collect::<Vec<_>>();
    match payload {
        Value::String(text) => Ok(serde_json::json!({
            "type": "chat_message",
            "text": text,
            "mention_actor_ids": mention_values,
        })),
        Value::Object(mut obj) => {
            obj.insert(
                "mention_actor_ids".to_string(),
                Value::Array(mention_values),
            );
            Ok(Value::Object(obj))
        }
        other => Err(anyhow::anyhow!(
            "explicit channel mentions require a string or object payload, got {}",
            match other {
                Value::Null => "null",
                Value::Bool(_) => "bool",
                Value::Number(_) => "number",
                Value::Array(_) => "array",
                _ => unreachable!("string/object already handled"),
            }
        )),
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
        "waiting" => Ok(TeamTaskStatus::Waiting),
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

fn parse_team_task_note_kind(raw: &str) -> anyhow::Result<TeamTaskNoteKind> {
    match raw.trim() {
        "comment" => Ok(TeamTaskNoteKind::Comment),
        "decision" => Ok(TeamTaskNoteKind::Decision),
        "result" => Ok(TeamTaskNoteKind::Result),
        other => Err(anyhow::anyhow!(
            "invalid task note kind '{}', expected one of: comment, decision, result",
            other
        )),
    }
}

pub(super) fn compute_time_trigger_fire_at(now_ts: i64, delay_seconds: i64) -> i64 {
    now_ts + delay_seconds + TIME_TRIGGER_FUTURE_SAFETY_MARGIN_SECONDS
}

pub(super) fn parse_actor_args(args: &[String]) -> anyhow::Result<ParsedActorCommand> {
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

pub(super) fn parse_actor_command(
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
            let mut actor_id = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => *output_mode = ActorOutputMode::Json,
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
                    flag @ ("--actor-id" | "--agent-id") => {
                        idx += 1;
                        actor_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?,
                        );
                    }
                    other => {
                        return Err(anyhow::anyhow!("unknown flag for team-members: {}", other));
                    }
                }
                idx += 1;
            }
            let (team_id, run_id) = resolve_team_context_scope(team_id, run_id);
            if team_id.is_none() && run_id.is_none() {
                return Err(anyhow::anyhow!(
                    "team-members requires --team-id, --run-id, or actor runtime env fallback"
                ));
            }
            let actor_id = take_actor_id(actor_id)?;
            Ok(ActorCommand::TeamMembers {
                team_id,
                run_id,
                actor_id,
            })
        }
        "team-tasks" => {
            let mut team_id = None;
            let mut run_id = None;
            let mut actor_id = None;
            let mut limit = 100_i64;
            let mut status = None;
            let mut task_id = None;
            let mut assigned_member_id = None;
            let mut topic = None;
            let mut include_shared_thread = false;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => *output_mode = ActorOutputMode::Json,
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
                    "--task-id" => {
                        idx += 1;
                        task_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--task-id requires a value"))?,
                        );
                    }
                    "--assigned-member-id" => {
                        idx += 1;
                        assigned_member_id = Some(args.get(idx).cloned().ok_or_else(|| {
                            anyhow::anyhow!("--assigned-member-id requires a value")
                        })?);
                    }
                    "--topic" => {
                        idx += 1;
                        topic = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--topic requires a value"))?,
                        );
                    }
                    "--include-shared-thread" => include_shared_thread = true,
                    other => return Err(anyhow::anyhow!("unknown flag for team-tasks: {}", other)),
                }
                idx += 1;
            }
            let status = match status.as_deref().map(str::trim) {
                Some("all") | None => None,
                Some(raw) => Some(parse_team_task_status_argument(raw)?),
            };
            let (team_id, run_id) = resolve_team_run_scope(team_id, run_id);
            if team_id.is_none() && run_id.is_none() {
                return Err(anyhow::anyhow!(
                    "team-tasks requires --team-id, --run-id, or actor runtime env fallback"
                ));
            }
            Ok(ActorCommand::TeamTasks {
                query: TeamTaskListQuery {
                    team_id,
                    run_id,
                    limit: limit.clamp(1, 500),
                    status,
                    task_id: take_optional(task_id),
                    assigned_member_id: take_optional(assigned_member_id),
                    topic: take_optional(topic),
                    include_shared_thread,
                },
                actor_id: take_actor_id(actor_id)?,
            })
        }
        "team-task-create" => {
            let mut team_id = None;
            let mut actor_id = None;
            let mut title = None;
            let mut status = TeamTaskStatus::Open;
            let mut topic = None;
            let mut context = None;
            let mut context_file = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => *output_mode = ActorOutputMode::Json,
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
                    "--context-json-file" => {
                        idx += 1;
                        let raw = args.get(idx).ok_or_else(|| {
                            anyhow::anyhow!("--context-json-file requires a value")
                        })?;
                        context_file = Some(parse_json_file(raw, "context_json")?);
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
            if context.is_some() && context_file.is_some() {
                return Err(anyhow::anyhow!(
                    "--context-json and --context-json-file cannot be used together"
                ));
            }
            let title = take_optional(title).ok_or_else(|| anyhow::anyhow!("title is required"))?;
            Ok(ActorCommand::TeamTaskCreate {
                team_id: take_team_id(team_id)?,
                actor_id: take_actor_id(actor_id)?,
                title,
                status,
                topic: take_optional(topic),
                context: context
                    .or(context_file)
                    .unwrap_or_else(|| serde_json::json!({})),
            })
        }
        "team-task-show" | "team-task-get" => {
            let mut team_id = None;
            let mut run_id = None;
            let mut actor_id = None;
            let mut task_id = None;
            let mut message_limit = 20_i64;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => *output_mode = ActorOutputMode::Json,
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
                    "--message-limit" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--message-limit requires a value"))?;
                        message_limit = parse_i64(raw, "message_limit")?;
                    }
                    other => {
                        return Err(anyhow::anyhow!(
                            "unknown flag for team-task-show: {}",
                            other
                        ));
                    }
                }
                idx += 1;
            }
            let (team_id, run_id) = resolve_team_run_scope(team_id, run_id);
            if team_id.is_none() && run_id.is_none() {
                return Err(anyhow::anyhow!(
                    "team-task-show requires --team-id, --run-id, or actor runtime env fallback"
                ));
            }
            Ok(ActorCommand::TeamTaskShow {
                team_id,
                run_id,
                actor_id: take_actor_id(actor_id)?,
                task_id: take_optional(task_id)
                    .ok_or_else(|| anyhow::anyhow!("task_id is required"))?,
                message_limit: message_limit.clamp(1, 200),
            })
        }
        "team-task-update" => {
            let mut team_id = None;
            let mut actor_id = None;
            let mut task_ids = Vec::new();
            let mut status = None;
            let mut assigned_member_id = None;
            let mut clear_assigned_member_id = false;
            let mut context = None;
            let mut context_file = None;
            let mut context_merge = None;
            let mut context_merge_file = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => *output_mode = ActorOutputMode::Json,
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
                        task_ids.push(
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
                    "--assigned-member-id" => {
                        idx += 1;
                        assigned_member_id = Some(args.get(idx).cloned().ok_or_else(|| {
                            anyhow::anyhow!("--assigned-member-id requires a value")
                        })?);
                    }
                    "--unassign" => {
                        clear_assigned_member_id = true;
                    }
                    "--context-json" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--context-json requires a value"))?;
                        set_unique_json_value(
                            &mut context,
                            parse_json(raw, "context_json")?,
                            "--context-json, --context-json-file, and --context-file cannot be used together",
                        )?;
                    }
                    "--context-json-file" => {
                        idx += 1;
                        let raw = args.get(idx).ok_or_else(|| {
                            anyhow::anyhow!("--context-json-file requires a value")
                        })?;
                        set_unique_json_value(
                            &mut context_file,
                            parse_json_file(raw, "context_json")?,
                            "--context-json, --context-json-file, and --context-file cannot be used together",
                        )?;
                    }
                    "--context-file" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--context-file requires a value"))?;
                        set_unique_json_value(
                            &mut context_file,
                            parse_json_file(raw, "context_json")?,
                            "--context-json, --context-json-file, and --context-file cannot be used together",
                        )?;
                    }
                    "--context-merge-json" => {
                        idx += 1;
                        let raw = args.get(idx).ok_or_else(|| {
                            anyhow::anyhow!("--context-merge-json requires a value")
                        })?;
                        set_unique_json_value(
                            &mut context_merge,
                            parse_json(raw, "context_merge_json")?,
                            "--context-merge-json, --context-merge-json-file, and --context-merge-file cannot be used together",
                        )?;
                    }
                    "--context-merge-json-file" => {
                        idx += 1;
                        let raw = args.get(idx).ok_or_else(|| {
                            anyhow::anyhow!("--context-merge-json-file requires a value")
                        })?;
                        set_unique_json_value(
                            &mut context_merge_file,
                            parse_json_file(raw, "context_merge_json")?,
                            "--context-merge-json, --context-merge-json-file, and --context-merge-file cannot be used together",
                        )?;
                    }
                    "--context-merge-file" => {
                        idx += 1;
                        let raw = args.get(idx).ok_or_else(|| {
                            anyhow::anyhow!("--context-merge-file requires a value")
                        })?;
                        set_unique_json_value(
                            &mut context_merge_file,
                            parse_json_file(raw, "context_merge_json")?,
                            "--context-merge-json, --context-merge-json-file, and --context-merge-file cannot be used together",
                        )?;
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
            if assigned_member_id.is_some() && clear_assigned_member_id {
                return Err(anyhow::anyhow!(
                    "--assigned-member-id and --unassign cannot be used together"
                ));
            }
            if task_ids.is_empty() {
                return Err(anyhow::anyhow!("task_id is required"));
            }
            if context.is_some() && context_file.is_some() {
                return Err(anyhow::anyhow!(
                    "--context-json, --context-json-file, and --context-file cannot be used together"
                ));
            }
            if context_merge.is_some() && context_merge_file.is_some() {
                return Err(anyhow::anyhow!(
                    "--context-merge-json, --context-merge-json-file, and --context-merge-file cannot be used together"
                ));
            }
            if (context.is_some() || context_file.is_some())
                && (context_merge.is_some() || context_merge_file.is_some())
            {
                return Err(anyhow::anyhow!(
                    "--context-json/--context-json-file/--context-file and --context-merge-json/--context-merge-json-file/--context-merge-file cannot be used together"
                ));
            }
            let task_ids = task_ids
                .into_iter()
                .filter_map(|task_id| take_optional(Some(task_id)))
                .collect::<Vec<_>>();
            if task_ids.is_empty() {
                return Err(anyhow::anyhow!("task_id is required"));
            }
            Ok(ActorCommand::TeamTaskUpdate {
                team_id: take_team_id(team_id)?,
                actor_id: take_actor_id(actor_id)?,
                task_ids,
                status,
                assigned_member_id: take_optional(assigned_member_id),
                clear_assigned_member_id,
                context: context.or(context_file),
                context_merge: context_merge.or(context_merge_file),
            })
        }
        "team-task-note" => {
            let mut team_id = None;
            let mut run_id = None;
            let mut actor_id = None;
            let mut task_id = None;
            let mut shared_thread = false;
            let mut kind = TeamTaskNoteKind::Comment;
            let mut text = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => *output_mode = ActorOutputMode::Json,
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
                    "--shared-thread" => {
                        shared_thread = true;
                    }
                    "--kind" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--kind requires a value"))?;
                        kind = parse_team_task_note_kind(raw)?;
                    }
                    "--text" => {
                        idx += 1;
                        text = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--text requires a value"))?,
                        );
                    }
                    other => {
                        return Err(anyhow::anyhow!(
                            "unknown flag for team-task-note: {}",
                            other
                        ));
                    }
                }
                idx += 1;
            }
            let (team_id, run_id) = resolve_team_run_scope(team_id, run_id);
            if team_id.is_none() && run_id.is_none() {
                return Err(anyhow::anyhow!(
                    "team-task-note requires --team-id, --run-id, or actor runtime env fallback"
                ));
            }
            if shared_thread && task_id.is_some() {
                return Err(anyhow::anyhow!(
                    "--shared-thread and --task-id cannot be used together"
                ));
            }
            let task_id = take_optional(task_id);
            if !shared_thread && task_id.is_none() {
                return Err(anyhow::anyhow!(
                    "team-task-note requires --task-id or --shared-thread"
                ));
            }
            Ok(ActorCommand::TeamTaskNote {
                team_id,
                run_id,
                actor_id: take_actor_id(actor_id)?,
                task_id,
                shared_thread,
                kind,
                text: take_optional(text).ok_or_else(|| anyhow::anyhow!("text is required"))?,
            })
        }
        "team-step-transition" => {
            let mut run_id = None;
            let mut actor_id = None;
            let mut step_id = None;
            let mut action = None;
            let mut runtime_handle_id = None;
            let mut output = None;
            let mut input = None;
            let mut reason = None;
            let mut error_text = None;
            let mut idx = 1;
            while idx < args.len() {
                let current_flag = args[idx].as_str();
                if parse_team_step_scope_argument(
                    args,
                    &mut idx,
                    output_mode,
                    &mut run_id,
                    &mut actor_id,
                    &mut step_id,
                    &mut runtime_handle_id,
                )? {
                } else {
                    match current_flag {
                        "--action" => {
                            idx += 1;
                            action = Some(
                                args.get(idx)
                                    .cloned()
                                    .ok_or_else(|| anyhow::anyhow!("--action requires a value"))?,
                            );
                        }
                        "--output-json" => {
                            idx += 1;
                            set_unique_json_value(
                                &mut output,
                                parse_json(
                                    args.get(idx).ok_or_else(|| {
                                        anyhow::anyhow!("--output-json requires a value")
                                    })?,
                                    "--output-json",
                                )?,
                                "--output-json and --output-json-file cannot be used together",
                            )?;
                        }
                        "--output-json-file" => {
                            idx += 1;
                            set_unique_json_value(
                                &mut output,
                                parse_json_file(
                                    args.get(idx).ok_or_else(|| {
                                        anyhow::anyhow!("--output-json-file requires a value")
                                    })?,
                                    "--output-json-file",
                                )?,
                                "--output-json and --output-json-file cannot be used together",
                            )?;
                        }
                        "--input-json" => {
                            idx += 1;
                            set_unique_json_value(
                                &mut input,
                                parse_json(
                                    args.get(idx).ok_or_else(|| {
                                        anyhow::anyhow!("--input-json requires a value")
                                    })?,
                                    "--input-json",
                                )?,
                                "--input-json and --input-json-file cannot be used together",
                            )?;
                        }
                        "--input-json-file" => {
                            idx += 1;
                            set_unique_json_value(
                                &mut input,
                                parse_json_file(
                                    args.get(idx).ok_or_else(|| {
                                        anyhow::anyhow!("--input-json-file requires a value")
                                    })?,
                                    "--input-json-file",
                                )?,
                                "--input-json and --input-json-file cannot be used together",
                            )?;
                        }
                        "--reason" => {
                            idx += 1;
                            anyhow::ensure!(
                                reason.is_none(),
                                "--reason and --reason-file cannot be used together"
                            );
                            reason = Some(
                                args.get(idx)
                                    .cloned()
                                    .ok_or_else(|| anyhow::anyhow!("--reason requires a value"))?,
                            );
                        }
                        "--reason-file" => {
                            idx += 1;
                            anyhow::ensure!(
                                reason.is_none(),
                                "--reason and --reason-file cannot be used together"
                            );
                            reason = Some(read_actor_send_file(
                                args.get(idx).ok_or_else(|| {
                                    anyhow::anyhow!("--reason-file requires a value")
                                })?,
                                "--reason-file",
                            )?);
                        }
                        "--error-text" => {
                            idx += 1;
                            anyhow::ensure!(
                                error_text.is_none(),
                                "--error-text and --error-text-file cannot be used together"
                            );
                            error_text =
                                Some(args.get(idx).cloned().ok_or_else(|| {
                                    anyhow::anyhow!("--error-text requires a value")
                                })?);
                        }
                        "--error-text-file" => {
                            idx += 1;
                            anyhow::ensure!(
                                error_text.is_none(),
                                "--error-text and --error-text-file cannot be used together"
                            );
                            error_text = Some(read_actor_send_file(
                                args.get(idx).ok_or_else(|| {
                                    anyhow::anyhow!("--error-text-file requires a value")
                                })?,
                                "--error-text-file",
                            )?);
                        }
                        other => {
                            return Err(anyhow::anyhow!(
                                "unknown flag for team-step-transition: {}",
                                other
                            ));
                        }
                    }
                }
                idx += 1;
            }

            let step_id =
                take_optional(step_id).ok_or_else(|| anyhow::anyhow!("step_id is required"))?;
            let action =
                take_optional(action).ok_or_else(|| anyhow::anyhow!("action is required"))?;
            anyhow::ensure!(
                matches!(
                    action.as_str(),
                    "start" | "continue" | "complete" | "input_required" | "resume" | "fail"
                ),
                "invalid action '{}', expected one of: start, continue, complete, input_required, resume, fail",
                action
            );
            if action == "fail" {
                anyhow::ensure!(
                    error_text
                        .as_deref()
                        .map(str::trim)
                        .is_some_and(|value| !value.is_empty()),
                    "team-step-transition action=fail requires --error-text or --error-text-file"
                );
            }
            Ok(ActorCommand::TeamStepTransition {
                run_id: take_optional(run_id),
                actor_id: take_actor_id(actor_id)?,
                step_id,
                action,
                runtime_handle_id: take_optional(runtime_handle_id),
                output,
                error_text: take_optional(error_text),
                input,
                reason: take_optional(reason),
            })
        }
        "team-step-decision" => {
            let mut run_id = None;
            let mut actor_id = None;
            let mut step_id = None;
            let mut runtime_handle_id = None;
            let mut decision = None;
            let mut idx = 1;
            while idx < args.len() {
                let current_flag = args[idx].as_str();
                if parse_team_step_scope_argument(
                    args,
                    &mut idx,
                    output_mode,
                    &mut run_id,
                    &mut actor_id,
                    &mut step_id,
                    &mut runtime_handle_id,
                )? {
                } else {
                    match current_flag {
                        "--decision-json" => {
                            idx += 1;
                            set_unique_json_value(
                                &mut decision,
                                parse_json(
                                    args.get(idx).ok_or_else(|| {
                                        anyhow::anyhow!("--decision-json requires a value")
                                    })?,
                                    "--decision-json",
                                )?,
                                "--decision-json and --decision-json-file cannot be used together",
                            )?;
                        }
                        "--decision-json-file" => {
                            idx += 1;
                            set_unique_json_value(
                                &mut decision,
                                parse_json_file(
                                    args.get(idx).ok_or_else(|| {
                                        anyhow::anyhow!("--decision-json-file requires a value")
                                    })?,
                                    "--decision-json-file",
                                )?,
                                "--decision-json and --decision-json-file cannot be used together",
                            )?;
                        }
                        other => {
                            return Err(anyhow::anyhow!(
                                "unknown flag for team-step-decision: {}",
                                other
                            ));
                        }
                    }
                }
                idx += 1;
            }

            let step_id =
                take_optional(step_id).ok_or_else(|| anyhow::anyhow!("step_id is required"))?;
            let decision = if let Some(decision) = decision {
                let decision_obj = decision
                    .as_object()
                    .ok_or_else(|| anyhow::anyhow!("decision_json must be a JSON object"))?;
                let action = decision_obj
                    .get("action")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| anyhow::anyhow!("decision_json.action is required"))?;
                anyhow::ensure!(
                    matches!(
                        action,
                        "start" | "continue" | "complete" | "input_required" | "resume" | "fail"
                    ),
                    "invalid decision_json.action '{}', expected one of: start, continue, complete, input_required, resume, fail",
                    action
                );
                let output = decision_obj.get("output").cloned();
                let input = decision_obj.get("input").cloned();
                let reason = decision_obj
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let error_text = decision_obj
                    .get("error_text")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if action == "fail" {
                    anyhow::ensure!(
                        error_text
                            .as_deref()
                            .map(str::trim)
                            .is_some_and(|value| !value.is_empty()),
                        "team-step-decision action=fail requires decision_json.error_text"
                    );
                }
                let mut normalized_decision = serde_json::Map::new();
                normalized_decision.insert("action".to_string(), Value::String(action.to_string()));
                if let Some(output) = output {
                    normalized_decision.insert("output".to_string(), output);
                }
                if let Some(input) = input {
                    normalized_decision.insert("input".to_string(), input);
                }
                if let Some(reason) = reason {
                    normalized_decision.insert("reason".to_string(), Value::String(reason));
                }
                if let Some(error_text) = error_text {
                    normalized_decision.insert("error_text".to_string(), Value::String(error_text));
                }
                Value::Object(normalized_decision)
            } else {
                Value::Null
            };
            Ok(ActorCommand::TeamStepDecision {
                run_id: take_optional(run_id),
                actor_id: take_actor_id(actor_id)?,
                step_id,
                runtime_handle_id: take_optional(runtime_handle_id),
                decision,
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
                    "--json" => *output_mode = ActorOutputMode::Json,
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
                    "--include-delivered" => include_delivered = true,
                    other => return Err(anyhow::anyhow!("unknown flag for inbox: {}", other)),
                }
                idx += 1;
            }
            Ok(ActorCommand::Inbox {
                run_id: take_optional(run_id)
                    .or_else(|| normalized_env_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV)),
                actor_id: take_mailbox_actor_id(actor_id)?,
                limit: limit.max(1),
                after_id,
                include_delivered,
            })
        }
        "receive" => {
            let mut run_id = None;
            let mut actor_id = None;
            let mut limit = 100_i64;
            let mut after_id = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => *output_mode = ActorOutputMode::Json,
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
                    other => return Err(anyhow::anyhow!("unknown flag for receive: {}", other)),
                }
                idx += 1;
            }
            Ok(ActorCommand::Receive {
                run_id: take_optional(run_id)
                    .or_else(|| normalized_env_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV)),
                actor_id: take_mailbox_actor_id(actor_id)?,
                limit: limit.max(1),
                after_id,
            })
        }
        "ack" => {
            let mut run_id = None;
            let mut actor_id = None;
            let mut message_ids = Vec::new();
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => *output_mode = ActorOutputMode::Json,
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
                        message_ids.push(parse_i64(raw, "message_id")?);
                    }
                    raw if !raw.starts_with('-') => {
                        message_ids.push(parse_i64(raw, "message_id")?);
                    }
                    other => return Err(anyhow::anyhow!("unknown flag for ack: {}", other)),
                }
                idx += 1;
            }
            Ok(ActorCommand::Ack {
                run_id: take_optional(run_id)
                    .or_else(|| normalized_env_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV)),
                actor_id: take_mailbox_actor_id(actor_id)?,
                message_ids: (!message_ids.is_empty())
                    .then_some(message_ids)
                    .ok_or_else(|| anyhow::anyhow!("at least one message_id is required"))?,
            })
        }
        "send" => {
            let mut run_id = None;
            let mut from_actor_id = None;
            let mut to_actor_id = None;
            let mut channel_id = None;
            let mut mention_actor_ids = Vec::new();
            let mut channel = None;
            let mut transport = None;
            let mut route = None;
            let mut text = None;
            let mut text_file = None;
            let mut payload = None;
            let mut payload_file = None;
            let mut idempotency_key = None;
            let mut allow_duplicate = false;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => *output_mode = ActorOutputMode::Json,
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
                    flag @ ("--to" | "--direct") => {
                        idx += 1;
                        to_actor_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?,
                        );
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
                    "--shared" => {
                        channel_id = Some("all".to_string());
                    }
                    "--mention" | "--mention-actor-id" => {
                        idx += 1;
                        mention_actor_ids.push(args.get(idx).cloned().ok_or_else(|| {
                            anyhow::anyhow!("{} requires a value", args[idx - 1])
                        })?);
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
                    "--text-file" => {
                        idx += 1;
                        text_file = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--text-file requires a value"))?,
                        );
                    }
                    "--payload-json" => {
                        idx += 1;
                        let raw = args
                            .get(idx)
                            .ok_or_else(|| anyhow::anyhow!("--payload-json requires a value"))?;
                        payload = Some(parse_json(raw, "payload_json")?);
                    }
                    "--payload-file" => {
                        idx += 1;
                        payload_file =
                            Some(args.get(idx).cloned().ok_or_else(|| {
                                anyhow::anyhow!("--payload-file requires a value")
                            })?);
                    }
                    "--idempotency-key" => {
                        idx += 1;
                        idempotency_key = Some(args.get(idx).cloned().ok_or_else(|| {
                            anyhow::anyhow!("--idempotency-key requires a value")
                        })?);
                    }
                    "--allow-duplicate" => allow_duplicate = true,
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
            let run_id = take_optional(run_id)
                .or_else(|| normalized_env_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV));
            let from_actor_id = take_required_with_env_keys(
                from_actor_id,
                &[ACTOR_RUNTIME_ACTOR_ID_ENV],
                "from_actor_id",
            )?;
            let (to_actor_id, channel_id) = resolve_actor_send_target(to_actor_id, channel_id)?;
            let mention_actor_ids = normalize_actor_send_mentions(mention_actor_ids)?;
            anyhow::ensure!(
                mention_actor_ids.is_empty() || channel_id.is_some(),
                "--mention and --mention-actor-id are supported only for channel send"
            );
            let channel = take_optional(channel).unwrap_or(fallback_channel);
            let (payload, payload_source) =
                resolve_actor_send_payload(text, text_file, payload, payload_file)?;
            let payload = merge_actor_send_mentions(payload, &mention_actor_ids)?;
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
            let idempotency = if allow_duplicate {
                ActorSendIdempotency::Disabled
            } else if let Some(explicit_idempotency_key) = explicit_idempotency_key {
                ActorSendIdempotency::Resolved(explicit_idempotency_key)
            } else if let Some(run_id) = run_id.as_deref() {
                ActorSendIdempotency::Resolved(build_actor_send_default_idempotency_key(
                    run_id,
                    &from_actor_id,
                    match (to_actor_id.as_deref(), channel_id.as_deref()) {
                        (Some(to_actor_id), None) => ActorSendTargetRef::Direct { to_actor_id },
                        (None, Some(channel_id)) => ActorSendTargetRef::Channel { channel_id },
                        _ => unreachable!("actor send target already validated"),
                    },
                    &channel,
                    &transport,
                    route.as_ref(),
                    &payload,
                ))
            } else {
                ActorSendIdempotency::DeferredDefault
            };

            Ok(ActorCommand::Send {
                run_id,
                from_actor_id,
                to_actor_id,
                channel_id,
                mention_actor_ids,
                channel,
                transport,
                route,
                payload: Box::new(payload),
                payload_source,
                idempotency,
            })
        }
        "time-trigger-set" => {
            let mut actor_id = None;
            let mut delay_seconds = None;
            let mut message = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => *output_mode = ActorOutputMode::Json,
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
                    "--json" => *output_mode = ActorOutputMode::Json,
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
                    "--json" => *output_mode = ActorOutputMode::Json,
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
            let mut permission_ids = Vec::new();
            let mut option_id = None;
            let mut outcome = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--json" => *output_mode = ActorOutputMode::Json,
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
                        permission_ids.push(
                            args.get(idx).cloned().ok_or_else(|| {
                                anyhow::anyhow!("--permission-id requires a value")
                            })?,
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
            let permission_ids = permission_ids
                .into_iter()
                .filter_map(|permission_id| {
                    let trimmed = permission_id.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                })
                .collect::<Vec<_>>();
            Ok(ActorCommand::PermissionReviewRespond {
                team_id: take_team_id(team_id)?,
                actor_id: take_actor_id(actor_id)?,
                permission_ids: (!permission_ids.is_empty())
                    .then_some(permission_ids)
                    .ok_or_else(|| anyhow::anyhow!("at least one --permission-id is required"))?,
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

#[cfg(test)]
mod tests {
    use super::parse_actor_args;
    use crate::actor_cli::ActorCommand;
    use serde_json::json;

    #[test]
    fn parse_send_channel_mentions_into_payload_and_dedupes() {
        let parsed = parse_actor_args(&[
            "send".to_string(),
            "--run-id".to_string(),
            "run-1".to_string(),
            "--from-actor-id".to_string(),
            "planner".to_string(),
            "--shared".to_string(),
            "--mention".to_string(),
            " reviewer ".to_string(),
            "--mention-actor-id".to_string(),
            "reviewer".to_string(),
            "--mention".to_string(),
            "worker".to_string(),
            "--text".to_string(),
            "please check".to_string(),
        ])
        .expect("parse actor send with mentions");

        match parsed.command {
            ActorCommand::Send {
                channel_id,
                mention_actor_ids,
                payload,
                ..
            } => {
                assert_eq!(channel_id.as_deref(), Some("all"));
                assert_eq!(
                    mention_actor_ids,
                    vec!["reviewer".to_string(), "worker".to_string()]
                );
                assert_eq!(
                    *payload,
                    json!({
                        "type": "chat_message",
                        "text": "please check",
                        "mention_actor_ids": ["reviewer", "worker"],
                    })
                );
            }
            other => panic!("expected send command, got {other:?}"),
        }
    }

    #[test]
    fn parse_send_rejects_direct_mentions() {
        let result = parse_actor_args(&[
            "send".to_string(),
            "--run-id".to_string(),
            "run-1".to_string(),
            "--from-actor-id".to_string(),
            "planner".to_string(),
            "--to".to_string(),
            "reviewer".to_string(),
            "--mention".to_string(),
            "worker".to_string(),
            "--text".to_string(),
            "please check".to_string(),
        ]);
        let err = match result {
            Ok(_) => panic!("direct send mentions should fail"),
            Err(err) => err,
        };

        assert!(
            err.to_string()
                .contains("--mention and --mention-actor-id are supported only for channel send")
        );
    }
}
