use agenthub_team_actor::parse_actor_transport;
use serde_json::Value;

use crate::team::{TeamActorMessageTransport, TeamManager};

const ACTOR_RUNTIME_RUN_ID_ENV: &str = "AGENTHUB_ACTOR_RUN_ID";
const ACTOR_RUNTIME_ACTOR_ID_ENV: &str = "AGENTHUB_ACTOR_ID";
const ACTOR_RUNTIME_CHANNEL_ENV: &str = "AGENTHUB_ACTOR_CHANNEL";

enum ActorCommand {
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
    Send {
        run_id: String,
        from_actor_id: String,
        to_actor_id: String,
        channel: String,
        transport: TeamActorMessageTransport,
        route: Option<Value>,
        payload: Value,
        idempotency_key: Option<String>,
    },
}

fn actor_usage() -> &'static str {
    r#"Usage:
  agenthub actor inbox [--run-id <run_id>] [--actor-id <actor_id>] [--limit <n>] [--after-id <id>] [--include-delivered]
  agenthub actor ack --message-id <id> [--run-id <run_id>] [--actor-id <actor_id>]
  agenthub actor send --to-actor-id <actor_id> --payload-json <json> [--run-id <run_id>] [--from-actor-id <actor_id>] [--channel <name>] [--transport <local|remote>] [--route-json <json>] [--idempotency-key <key>]

Environment fallback:
  AGENTHUB_ACTOR_RUN_ID
  AGENTHUB_ACTOR_ID
  AGENTHUB_ACTOR_CHANNEL
"#
}

fn normalized_env_var(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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

fn take_required(
    value: Option<String>,
    env_key: Option<&str>,
    field: &str,
) -> anyhow::Result<String> {
    if let Some(value) = value {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    if let Some(env_key) = env_key {
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

fn parse_actor_command(args: &[String]) -> anyhow::Result<ActorCommand> {
    let sub = args
        .first()
        .ok_or_else(|| anyhow::anyhow!("missing actor subcommand\n{}", actor_usage()))?;
    match sub.as_str() {
        "inbox" => {
            let mut run_id = None;
            let mut actor_id = None;
            let mut limit = 100_i64;
            let mut after_id = None;
            let mut include_delivered = false;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
                    "--run-id" => {
                        idx += 1;
                        run_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--run-id requires a value"))?,
                        );
                    }
                    "--actor-id" => {
                        idx += 1;
                        actor_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--actor-id requires a value"))?,
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
                run_id: take_required(run_id, Some(ACTOR_RUNTIME_RUN_ID_ENV), "run_id")?,
                actor_id: take_required(actor_id, Some(ACTOR_RUNTIME_ACTOR_ID_ENV), "actor_id")?,
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
                    "--run-id" => {
                        idx += 1;
                        run_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--run-id requires a value"))?,
                        );
                    }
                    "--actor-id" => {
                        idx += 1;
                        actor_id = Some(
                            args.get(idx)
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("--actor-id requires a value"))?,
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
                run_id: take_required(run_id, Some(ACTOR_RUNTIME_RUN_ID_ENV), "run_id")?,
                actor_id: take_required(actor_id, Some(ACTOR_RUNTIME_ACTOR_ID_ENV), "actor_id")?,
                message_id: message_id.ok_or_else(|| anyhow::anyhow!("message_id is required"))?,
            })
        }
        "send" => {
            let mut run_id = None;
            let mut from_actor_id = None;
            let mut to_actor_id = None;
            let mut channel = None;
            let mut transport = None;
            let mut route = None;
            let mut payload = None;
            let mut idempotency_key = None;
            let mut idx = 1;
            while idx < args.len() {
                match args[idx].as_str() {
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
                    "--to-actor-id" => {
                        idx += 1;
                        to_actor_id =
                            Some(args.get(idx).cloned().ok_or_else(|| {
                                anyhow::anyhow!("--to-actor-id requires a value")
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
            Ok(ActorCommand::Send {
                run_id: take_required(run_id, Some(ACTOR_RUNTIME_RUN_ID_ENV), "run_id")?,
                from_actor_id: take_required(
                    from_actor_id,
                    Some(ACTOR_RUNTIME_ACTOR_ID_ENV),
                    "from_actor_id",
                )?,
                to_actor_id: take_required(to_actor_id, None, "to_actor_id")?,
                channel: take_optional(channel).unwrap_or(fallback_channel),
                transport,
                route,
                payload: payload.ok_or_else(|| anyhow::anyhow!("payload_json is required"))?,
                idempotency_key: take_optional(idempotency_key),
            })
        }
        "help" | "--help" | "-h" => Err(anyhow::anyhow!("{}", actor_usage())),
        other => Err(anyhow::anyhow!(
            "unknown actor subcommand: {}\n{}",
            other,
            actor_usage()
        )),
    }
}

async fn run_actor_command(command: ActorCommand) -> anyhow::Result<()> {
    let db = crate::db::init_db().await?;
    let manager = TeamManager::new(db);
    match command {
        ActorCommand::Inbox {
            run_id,
            actor_id,
            limit,
            after_id,
            include_delivered,
        } => {
            let messages = manager
                .list_actor_inbox(&run_id, &actor_id, limit, after_id, include_delivered)
                .await?;
            println!("{}", serde_json::to_string(&messages)?);
        }
        ActorCommand::Ack {
            run_id,
            actor_id,
            message_id,
        } => {
            let message = manager
                .ack_actor_message(&run_id, &actor_id, message_id)
                .await?;
            println!("{}", serde_json::to_string(&message)?);
        }
        ActorCommand::Send {
            run_id,
            from_actor_id,
            to_actor_id,
            channel,
            transport,
            route,
            payload,
            idempotency_key,
        } => {
            let message = manager
                .send_actor_message(
                    &run_id,
                    &from_actor_id,
                    &to_actor_id,
                    &channel,
                    transport,
                    route,
                    payload,
                    idempotency_key.as_deref(),
                )
                .await?;
            println!("{}", serde_json::to_string(&message)?);
        }
    }
    Ok(())
}

pub async fn maybe_run_from_args() -> Option<anyhow::Result<()>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) != Some("actor") {
        return None;
    }
    let parsed = parse_actor_command(&args[1..]);
    Some(match parsed {
        Ok(command) => run_actor_command(command).await,
        Err(err) => Err(err),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let prev_run = std::env::var(ACTOR_RUNTIME_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_RUN_ID_ENV, "run-x");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
        }
        let args = vec!["inbox".to_string(), "--limit".to_string(), "5".to_string()];
        let parsed = parse_actor_command(&args).expect("parse inbox");
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
        restore_env(ACTOR_RUNTIME_RUN_ID_ENV, prev_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_send_validates_remote_route() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let prev_run = std::env::var(ACTOR_RUNTIME_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_RUN_ID_ENV, "run-x");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "remote-peer".to_string(),
            "--transport".to_string(),
            "remote".to_string(),
            "--payload-json".to_string(),
            r#"{"text":"hi"}"#.to_string(),
        ];
        assert!(
            parse_actor_command(&args).is_err(),
            "remote transport must require route-json"
        );
        restore_env(ACTOR_RUNTIME_RUN_ID_ENV, prev_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }
}
