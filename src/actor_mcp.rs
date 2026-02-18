use agenthub_team_actor::{
    ActorAckRequest, ActorInboxRequest, ActorMailboxService, ActorMessageStatus,
    ActorMessageTransport, ActorSendRequest, ActorServiceError,
    build_default_actor_message_idempotency_key, parse_actor_transport,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::team::TeamManager;

const ACTOR_RUNTIME_RUN_ID_ENV: &str = "AGENTHUB_ACTOR_RUN_ID";
const ACTOR_RUNTIME_ACTOR_ID_ENV: &str = "AGENTHUB_ACTOR_ID";
const ACTOR_RUNTIME_CHANNEL_ENV: &str = "AGENTHUB_ACTOR_CHANNEL";
const DEFAULT_ACTOR_CHANNEL: &str = "default";

const JSONRPC_PARSE_ERROR: i32 = -32700;
const JSONRPC_INVALID_REQUEST: i32 = -32600;
const JSONRPC_METHOD_NOT_FOUND: i32 = -32601;
const JSONRPC_INVALID_PARAMS: i32 = -32602;

#[derive(Debug, Clone)]
struct ActorMcpContext {
    run_id: String,
    actor_id: String,
    default_channel: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ActorInboxToolArgs {
    limit: Option<i64>,
    cursor: Option<i64>,
    include_delivered: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ActorAckToolArgs {
    message_id: i64,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ActorSendToolArgs {
    to_actor_id: Option<String>,
    payload: Option<Value>,
    channel: Option<String>,
    transport: Option<String>,
    route: Option<Value>,
    idempotency_key: Option<String>,
    allow_duplicate: Option<bool>,
}

fn actor_mcp_usage() -> &'static str {
    r#"Usage:
  agenthub actor-mcp [--run-id <run_id>] [--actor-id <actor_id>] [--channel <name>]

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
    if let Some(env_key) = env_key
        && let Some(value) = normalized_env_var(env_key)
    {
        return Ok(value);
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

fn parse_actor_mcp_context_with_env<F>(
    args: &[String],
    mut env_lookup: F,
) -> anyhow::Result<ActorMcpContext>
where
    F: FnMut(&str) -> Option<String>,
{
    let mut run_id = None;
    let mut actor_id = None;
    let mut channel = None;

    let mut idx = 0;
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
            "--channel" => {
                idx += 1;
                channel = Some(
                    args.get(idx)
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("--channel requires a value"))?,
                );
            }
            "--help" | "-h" | "help" => {
                return Err(anyhow::anyhow!("{}", actor_mcp_usage()));
            }
            other => {
                return Err(anyhow::anyhow!(
                    "unknown flag for actor-mcp: {}\n{}",
                    other,
                    actor_mcp_usage()
                ));
            }
        }
        idx += 1;
    }

    let run_id = take_required(
        run_id.or_else(|| env_lookup(ACTOR_RUNTIME_RUN_ID_ENV)),
        None,
        "run_id",
    )?;
    let actor_id = take_required(
        actor_id.or_else(|| env_lookup(ACTOR_RUNTIME_ACTOR_ID_ENV)),
        None,
        "actor_id",
    )?;
    let default_channel = take_optional(channel)
        .or_else(|| env_lookup(ACTOR_RUNTIME_CHANNEL_ENV))
        .unwrap_or_else(|| DEFAULT_ACTOR_CHANNEL.to_string());

    Ok(ActorMcpContext {
        run_id,
        actor_id,
        default_channel,
    })
}

fn parse_actor_mcp_context(args: &[String]) -> anyhow::Result<ActorMcpContext> {
    parse_actor_mcp_context_with_env(args, normalized_env_var)
}

fn actor_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "actor_inbox",
            "description": "List messages for current actor mailbox.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 20 },
                    "cursor": { "type": "integer" },
                    "include_delivered": { "type": "boolean", "default": false }
                }
            }
        }),
        json!({
            "name": "actor_ack",
            "description": "Acknowledge one mailbox message as delivered.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["message_id"],
                "properties": {
                    "message_id": { "type": "integer", "minimum": 1 }
                }
            }
        }),
        json!({
            "name": "actor_send",
            "description": "Send a mailbox message from current actor to another actor.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["to_actor_id", "payload"],
                "properties": {
                    "to_actor_id": { "type": "string", "minLength": 1 },
                    "payload": {},
                    "channel": { "type": "string" },
                    "transport": { "type": "string", "enum": ["local", "remote"], "default": "local" },
                    "route": { "type": "object" },
                    "idempotency_key": { "type": "string", "minLength": 1, "maxLength": 128 },
                    "allow_duplicate": { "type": "boolean", "default": false }
                }
            }
        }),
    ]
}

fn jsonrpc_response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn jsonrpc_error(id: Value, code: i32, message: impl Into<String>, data: Option<Value>) -> Value {
    let mut error = json!({
        "code": code,
        "message": message.into(),
    });
    if let Some(data) = data {
        error["data"] = data;
    }
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": error,
    })
}

fn tool_result_success(structured_content: Value) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": structured_content.to_string()
            }
        ],
        "structuredContent": structured_content,
        "isError": false
    })
}

fn tool_result_error(message: impl Into<String>, structured_content: Option<Value>) -> Value {
    let message = message.into();
    let mut result = json!({
        "content": [
            {
                "type": "text",
                "text": message
            }
        ],
        "isError": true
    });
    if let Some(structured_content) = structured_content {
        result["structuredContent"] = structured_content;
    }
    result
}

fn tool_result_actor_service_error(err: ActorServiceError) -> Value {
    let structured = json!({
        "code": err.code,
        "message": err.message,
    });
    tool_result_error("actor mailbox request failed", Some(structured))
}

fn request_id(value: Option<&Value>) -> Option<Value> {
    match value {
        Some(Value::String(raw)) => Some(Value::String(raw.clone())),
        Some(Value::Number(raw)) => Some(Value::Number(raw.clone())),
        _ => None,
    }
}

fn parse_tool_args<T>(arguments: Option<&Map<String, Value>>) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let raw = Value::Object(arguments.cloned().unwrap_or_default());
    serde_json::from_value(raw).map_err(|err| format!("invalid tool arguments: {err}"))
}

struct ResolveIdempotencyKeyInput<'a> {
    run_id: &'a str,
    from_actor_id: &'a str,
    to_actor_id: &'a str,
    channel: &'a str,
    transport: &'a ActorMessageTransport,
    route: Option<&'a Value>,
    payload: &'a Value,
    explicit: Option<String>,
    allow_duplicate: bool,
}

fn resolve_idempotency_key(
    input: ResolveIdempotencyKeyInput<'_>,
) -> Result<Option<String>, String> {
    let explicit_idempotency_key = match input.explicit {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err("idempotency_key must be a non-empty string".to_string());
            }
            if trimmed.len() > 128 {
                return Err("idempotency_key must be at most 128 characters".to_string());
            }
            Some(trimmed.to_string())
        }
        None => None,
    };
    if input.allow_duplicate && explicit_idempotency_key.is_some() {
        return Err("allow_duplicate cannot be used with idempotency_key".to_string());
    }
    if input.allow_duplicate {
        return Ok(None);
    }
    Ok(Some(explicit_idempotency_key.unwrap_or_else(|| {
        build_default_actor_message_idempotency_key(
            input.run_id,
            input.from_actor_id,
            input.to_actor_id,
            input.channel,
            input.transport.as_str(),
            input.route,
            input.payload,
        )
    })))
}

async fn tool_actor_inbox<S: ActorMailboxService>(
    service: &S,
    context: &ActorMcpContext,
    arguments: Option<&Map<String, Value>>,
) -> Value {
    let args = match parse_tool_args::<ActorInboxToolArgs>(arguments) {
        Ok(args) => args,
        Err(err) => return tool_result_error(err, None),
    };
    let limit = args.limit.unwrap_or(20).clamp(1, 200);
    let states = if args.include_delivered.unwrap_or(false) {
        Some(vec![
            ActorMessageStatus::Pending,
            ActorMessageStatus::Delivered,
        ])
    } else {
        Some(vec![ActorMessageStatus::Pending])
    };
    let response = service
        .actor_inbox(ActorInboxRequest {
            run_id: context.run_id.clone(),
            actor_id: context.actor_id.clone(),
            cursor: args.cursor,
            limit: Some(limit),
            states,
        })
        .await;
    match response {
        Ok(response) => tool_result_success(json!({
            "messages": response.messages,
            "next_cursor": response.next_cursor,
        })),
        Err(err) => tool_result_actor_service_error(err),
    }
}

async fn tool_actor_ack<S: ActorMailboxService>(
    service: &S,
    context: &ActorMcpContext,
    arguments: Option<&Map<String, Value>>,
) -> Value {
    let args = match parse_tool_args::<ActorAckToolArgs>(arguments) {
        Ok(args) => args,
        Err(err) => return tool_result_error(err, None),
    };
    let response = service
        .actor_ack(ActorAckRequest {
            run_id: context.run_id.clone(),
            actor_id: context.actor_id.clone(),
            message_id: args.message_id,
            ack_token: None,
            result: None,
        })
        .await;
    match response {
        Ok(response) => tool_result_success(json!({
            "message_id": response.message_id,
            "state": response.state,
            "acked_at": response.acked_at,
            "message": response.message,
        })),
        Err(err) => tool_result_actor_service_error(err),
    }
}

async fn tool_actor_send<S: ActorMailboxService>(
    service: &S,
    context: &ActorMcpContext,
    arguments: Option<&Map<String, Value>>,
) -> Value {
    let args = match parse_tool_args::<ActorSendToolArgs>(arguments) {
        Ok(args) => args,
        Err(err) => return tool_result_error(err, None),
    };
    let to_actor_id = match take_required(args.to_actor_id, None, "to_actor_id") {
        Ok(value) => value,
        Err(err) => return tool_result_error(err.to_string(), None),
    };
    let payload = match args.payload {
        Some(payload) => payload,
        None => return tool_result_error("payload is required", None),
    };
    let transport = match parse_actor_transport(args.transport.as_deref()) {
        Ok(transport) => transport,
        Err(err) => return tool_result_error(err.to_string(), None),
    };
    let route = args.route;
    if transport == ActorMessageTransport::Remote && route.is_none() {
        return tool_result_error("route is required for remote transport", None);
    }
    if transport == ActorMessageTransport::Local && route.is_some() {
        return tool_result_error("route is not supported for local transport", None);
    }
    let channel =
        take_optional(args.channel).unwrap_or_else(|| context.default_channel.to_string());
    let allow_duplicate = args.allow_duplicate.unwrap_or(false);
    let idempotency_key = match resolve_idempotency_key(ResolveIdempotencyKeyInput {
        run_id: &context.run_id,
        from_actor_id: &context.actor_id,
        to_actor_id: &to_actor_id,
        channel: &channel,
        transport: &transport,
        route: route.as_ref(),
        payload: &payload,
        explicit: args.idempotency_key,
        allow_duplicate,
    }) {
        Ok(idempotency_key) => idempotency_key,
        Err(err) => return tool_result_error(err, None),
    };
    let response = service
        .actor_send(ActorSendRequest {
            run_id: context.run_id.clone(),
            from_actor_id: context.actor_id.clone(),
            to_actor_id,
            channel: Some(channel),
            transport: Some(transport),
            route,
            payload,
            idempotency_key,
        })
        .await;
    match response {
        Ok(response) => tool_result_success(json!({
            "message_id": response.message_id,
            "state": response.state,
            "deduped": response.deduped,
            "created_at": response.created_at,
            "message": response.message,
        })),
        Err(err) => tool_result_actor_service_error(err),
    }
}

async fn handle_tool_call<S: ActorMailboxService>(
    service: &S,
    context: &ActorMcpContext,
    params: Option<&Value>,
) -> Result<Value, Value> {
    let params = params.and_then(Value::as_object).ok_or_else(|| {
        jsonrpc_error(
            Value::Null,
            JSONRPC_INVALID_PARAMS,
            "params must be an object",
            None,
        )
    })?;
    let name = params.get("name").and_then(Value::as_str).ok_or_else(|| {
        jsonrpc_error(
            Value::Null,
            JSONRPC_INVALID_PARAMS,
            "params.name must be a string",
            None,
        )
    })?;
    let arguments = params.get("arguments").and_then(Value::as_object);
    let result = match name {
        "actor_inbox" => tool_actor_inbox(service, context, arguments).await,
        "actor_ack" => tool_actor_ack(service, context, arguments).await,
        "actor_send" => tool_actor_send(service, context, arguments).await,
        other => tool_result_error(format!("unknown tool: {}", other), None),
    };
    Ok(result)
}

async fn handle_jsonrpc_request<S: ActorMailboxService>(
    service: &S,
    context: &ActorMcpContext,
    initialized: &mut bool,
    method: &str,
    id: Value,
    params: Option<&Value>,
) -> Value {
    if !*initialized && method != "initialize" && method != "ping" {
        return jsonrpc_error(
            id,
            JSONRPC_INVALID_REQUEST,
            "initialize must be called before other methods",
            None,
        );
    }
    match method {
        "initialize" => {
            if *initialized {
                return jsonrpc_error(
                    id,
                    JSONRPC_INVALID_REQUEST,
                    "initialize called more than once",
                    None,
                );
            }
            *initialized = true;
            let protocol_version = params
                .and_then(Value::as_object)
                .and_then(|obj| obj.get("protocolVersion"))
                .and_then(Value::as_str)
                .unwrap_or("2025-03-26");
            jsonrpc_response(
                id,
                json!({
                    "protocolVersion": protocol_version,
                    "capabilities": {
                        "tools": { "listChanged": false }
                    },
                    "serverInfo": {
                        "name": "agenthub-actor-mailbox",
                        "title": "AgentHub Actor Mailbox",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "instructions": "Use actor_inbox / actor_ack / actor_send for actor mailbox coordination."
                }),
            )
        }
        "ping" => jsonrpc_response(id, json!({})),
        "tools/list" => jsonrpc_response(
            id,
            json!({
                "tools": actor_tools()
            }),
        ),
        "tools/call" => match handle_tool_call(service, context, params).await {
            Ok(result) => jsonrpc_response(id, result),
            Err(mut err) => {
                err["id"] = id;
                err
            }
        },
        _ => jsonrpc_error(
            id,
            JSONRPC_METHOD_NOT_FOUND,
            format!("method not found: {}", method),
            None,
        ),
    }
}

fn handle_jsonrpc_notification(initialized: &mut bool, method: &str) {
    if method == "notifications/initialized" {
        *initialized = true;
    }
}

async fn run_actor_mcp_server(context: ActorMcpContext) -> anyhow::Result<()> {
    let db = crate::db::init_db().await?;
    let manager = TeamManager::new(db);
    let service = manager.actor_mailbox_service();

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();
    let mut initialized = false;

    while let Some(line) = reader.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value = match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => value,
            Err(err) => {
                let response = jsonrpc_error(
                    Value::Null,
                    JSONRPC_PARSE_ERROR,
                    "invalid JSON-RPC payload",
                    Some(json!({ "detail": err.to_string() })),
                );
                stdout.write_all(response.to_string().as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
                continue;
            }
        };
        let obj = match value.as_object() {
            Some(obj) => obj,
            None => {
                let response = jsonrpc_error(
                    Value::Null,
                    JSONRPC_INVALID_REQUEST,
                    "JSON-RPC payload must be an object",
                    None,
                );
                stdout.write_all(response.to_string().as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
                continue;
            }
        };
        let method = match obj.get("method").and_then(Value::as_str) {
            Some(method) => method,
            None => {
                continue;
            }
        };
        if let Some(id) = request_id(obj.get("id")) {
            let response = handle_jsonrpc_request(
                &service,
                &context,
                &mut initialized,
                method,
                id,
                obj.get("params"),
            )
            .await;
            stdout.write_all(response.to_string().as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        } else {
            handle_jsonrpc_notification(&mut initialized, method);
        }
    }

    Ok(())
}

pub async fn maybe_run_from_args() -> Option<anyhow::Result<()>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) != Some("actor-mcp") {
        return None;
    }
    let parsed = parse_actor_mcp_context(&args[1..]);
    Some(match parsed {
        Ok(context) => run_actor_mcp_server(context).await,
        Err(err) => Err(err),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::team_tests::build_test_state;
    use crate::team::TeamDefinitionConfig;
    use uuid::Uuid;

    #[test]
    fn parse_actor_mcp_context_uses_env_fallback() {
        let env = [
            (ACTOR_RUNTIME_RUN_ID_ENV.to_string(), "run-x".to_string()),
            (
                ACTOR_RUNTIME_ACTOR_ID_ENV.to_string(),
                "planner".to_string(),
            ),
            (ACTOR_RUNTIME_CHANNEL_ENV.to_string(), "coord".to_string()),
        ]
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>();
        let context = parse_actor_mcp_context_with_env(&[], |key| env.get(key).cloned())
            .expect("parse actor mcp context");
        assert_eq!(context.run_id, "run-x");
        assert_eq!(context.actor_id, "planner");
        assert_eq!(context.default_channel, "coord");
    }

    #[test]
    fn parse_actor_mcp_context_requires_run_id_without_env_fallback() {
        let err = parse_actor_mcp_context_with_env(&[], |_| None).expect_err("run_id is required");
        assert!(
            err.to_string().contains("run_id is required"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_actor_mcp_context_defaults_channel_when_missing() {
        let env = [
            (ACTOR_RUNTIME_RUN_ID_ENV.to_string(), "run-x".to_string()),
            (
                ACTOR_RUNTIME_ACTOR_ID_ENV.to_string(),
                "planner".to_string(),
            ),
        ]
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>();
        let context = parse_actor_mcp_context_with_env(&[], |key| env.get(key).cloned())
            .expect("parse actor mcp context");
        assert_eq!(context.default_channel, DEFAULT_ACTOR_CHANNEL);
    }

    #[test]
    fn parse_actor_mcp_context_rejects_unknown_flag() {
        let args = vec!["--unknown".to_string()];
        let err = parse_actor_mcp_context_with_env(&args, |_| None).expect_err("unknown flag");
        assert!(
            err.to_string().contains("unknown flag for actor-mcp"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn actor_tools_exposes_expected_tool_names() {
        let tools = actor_tools();
        let names = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["actor_inbox", "actor_ack", "actor_send"]);
    }

    #[test]
    fn resolve_idempotency_key_rejects_conflicting_options() {
        let err = resolve_idempotency_key(ResolveIdempotencyKeyInput {
            run_id: "run-1",
            from_actor_id: "leader",
            to_actor_id: "worker",
            channel: "default",
            transport: &ActorMessageTransport::Local,
            route: None,
            payload: &json!({"task":"x"}),
            explicit: Some("k-1".to_string()),
            allow_duplicate: true,
        })
        .expect_err("allow_duplicate and explicit key should conflict");
        assert!(err.contains("allow_duplicate"));
    }

    #[test]
    fn resolve_idempotency_key_supports_allow_duplicate_without_explicit_key() {
        let key = resolve_idempotency_key(ResolveIdempotencyKeyInput {
            run_id: "run-1",
            from_actor_id: "leader",
            to_actor_id: "worker",
            channel: "default",
            transport: &ActorMessageTransport::Local,
            route: None,
            payload: &json!({"task":"x"}),
            explicit: None,
            allow_duplicate: true,
        })
        .expect("allow duplicate should be accepted");
        assert!(key.is_none());
    }

    #[test]
    fn resolve_idempotency_key_rejects_blank_and_too_long_explicit_key() {
        let blank_err = resolve_idempotency_key(ResolveIdempotencyKeyInput {
            run_id: "run-1",
            from_actor_id: "leader",
            to_actor_id: "worker",
            channel: "default",
            transport: &ActorMessageTransport::Local,
            route: None,
            payload: &json!({"task":"x"}),
            explicit: Some("   ".to_string()),
            allow_duplicate: false,
        })
        .expect_err("blank key should fail");
        assert!(blank_err.contains("non-empty"));

        let long_err = resolve_idempotency_key(ResolveIdempotencyKeyInput {
            run_id: "run-1",
            from_actor_id: "leader",
            to_actor_id: "worker",
            channel: "default",
            transport: &ActorMessageTransport::Local,
            route: None,
            payload: &json!({"task":"x"}),
            explicit: Some("x".repeat(129)),
            allow_duplicate: false,
        })
        .expect_err("too long key should fail");
        assert!(long_err.contains("at most 128"));
    }

    #[test]
    fn resolve_idempotency_key_generates_stable_default_key() {
        let route = json!({"endpoint":"https://node-a"});
        let payload = json!({"task":"x"});
        let first = resolve_idempotency_key(ResolveIdempotencyKeyInput {
            run_id: "run-1",
            from_actor_id: "leader",
            to_actor_id: "worker",
            channel: "default",
            transport: &ActorMessageTransport::Remote,
            route: Some(&route),
            payload: &payload,
            explicit: None,
            allow_duplicate: false,
        })
        .expect("default key should be generated")
        .expect("idempotency key");

        let second = resolve_idempotency_key(ResolveIdempotencyKeyInput {
            run_id: "run-1",
            from_actor_id: "leader",
            to_actor_id: "worker",
            channel: "default",
            transport: &ActorMessageTransport::Remote,
            route: Some(&route),
            payload: &payload,
            explicit: None,
            allow_duplicate: false,
        })
        .expect("default key should be generated")
        .expect("idempotency key");

        assert_eq!(first, second);
        assert!(!first.is_empty());
    }

    #[tokio::test]
    async fn jsonrpc_rejects_tools_before_initialize() {
        let state = build_test_state().await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: format!("actor-mcp-pre-init-{}", Uuid::new_v4()),
                description: Some("actor mcp pre initialize guard".to_string()),
                spec: json!({
                    "entrypoint":"planner",
                    "members":[{"member_id":"planner"}]
                }),
            })
            .await
            .expect("create team");
        let run = state
            .teams
            .create_run(
                &team.id,
                Some("ctx-actor-mcp-pre-init"),
                json!({"prompt":"go"}),
            )
            .await
            .expect("create run");
        let service = state.teams.actor_mailbox_service();
        let context = ActorMcpContext {
            run_id: run.id,
            actor_id: "planner".to_string(),
            default_channel: "default".to_string(),
        };

        let mut initialized = false;
        let response = handle_jsonrpc_request(
            &service,
            &context,
            &mut initialized,
            "tools/list",
            json!(1),
            None,
        )
        .await;

        assert_eq!(response["error"]["code"], JSONRPC_INVALID_REQUEST);
        assert!(
            response["error"]["message"]
                .as_str()
                .is_some_and(|msg| msg.contains("initialize must be called")),
            "unexpected error response: {response}"
        );
    }

    #[tokio::test]
    async fn jsonrpc_tools_list_and_call_drive_local_mailbox_flow() {
        let state = build_test_state().await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: format!("actor-mcp-jsonrpc-{}", Uuid::new_v4()),
                description: Some("actor mcp jsonrpc mailbox flow".to_string()),
                spec: json!({
                    "entrypoint":"planner",
                    "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
                }),
            })
            .await
            .expect("create team");
        let run = state
            .teams
            .create_run(
                &team.id,
                Some("ctx-actor-mcp-jsonrpc"),
                json!({"prompt":"go"}),
            )
            .await
            .expect("create run");
        let service = state.teams.actor_mailbox_service();

        let mut planner_initialized = false;
        let planner_context = ActorMcpContext {
            run_id: run.id.clone(),
            actor_id: "planner".to_string(),
            default_channel: "coordination".to_string(),
        };
        let init_resp = handle_jsonrpc_request(
            &service,
            &planner_context,
            &mut planner_initialized,
            "initialize",
            json!(1),
            Some(&json!({"protocolVersion":"2025-03-26"})),
        )
        .await;
        assert_eq!(
            init_resp["result"]["serverInfo"]["name"],
            "agenthub-actor-mailbox"
        );
        assert!(planner_initialized);

        let list_resp = handle_jsonrpc_request(
            &service,
            &planner_context,
            &mut planner_initialized,
            "tools/list",
            json!(2),
            None,
        )
        .await;
        let tool_names = list_resp["result"]["tools"]
            .as_array()
            .expect("tool array")
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(tool_names, vec!["actor_inbox", "actor_ack", "actor_send"]);

        let send_resp = handle_jsonrpc_request(
            &service,
            &planner_context,
            &mut planner_initialized,
            "tools/call",
            json!(3),
            Some(&json!({
                "name":"actor_send",
                "arguments":{
                    "to_actor_id":"reviewer",
                    "payload":{"task":"review patch"}
                }
            })),
        )
        .await;
        assert_eq!(send_resp["result"]["isError"], false);
        let message_id = send_resp["result"]["structuredContent"]["message_id"]
            .as_i64()
            .expect("message id");
        assert!(message_id > 0);

        let reviewer_context = ActorMcpContext {
            run_id: run.id,
            actor_id: "reviewer".to_string(),
            default_channel: "coordination".to_string(),
        };
        let mut reviewer_initialized = false;
        let _ = handle_jsonrpc_request(
            &service,
            &reviewer_context,
            &mut reviewer_initialized,
            "initialize",
            json!(4),
            Some(&json!({"protocolVersion":"2025-03-26"})),
        )
        .await;
        assert!(reviewer_initialized);

        let inbox_resp = handle_jsonrpc_request(
            &service,
            &reviewer_context,
            &mut reviewer_initialized,
            "tools/call",
            json!(5),
            Some(&json!({
                "name":"actor_inbox",
                "arguments":{"limit":20}
            })),
        )
        .await;
        assert_eq!(inbox_resp["result"]["isError"], false);
        let inbox_messages = inbox_resp["result"]["structuredContent"]["messages"]
            .as_array()
            .expect("inbox messages");
        assert_eq!(inbox_messages.len(), 1);
        assert_eq!(inbox_messages[0]["message_id"].as_i64(), Some(message_id));
        assert_eq!(inbox_messages[0]["status"], "pending");

        let ack_resp = handle_jsonrpc_request(
            &service,
            &reviewer_context,
            &mut reviewer_initialized,
            "tools/call",
            json!(6),
            Some(&json!({
                "name":"actor_ack",
                "arguments":{"message_id":message_id}
            })),
        )
        .await;
        assert_eq!(ack_resp["result"]["isError"], false);
        assert_eq!(
            ack_resp["result"]["structuredContent"]["state"],
            "delivered"
        );

        let delivered_resp = handle_jsonrpc_request(
            &service,
            &reviewer_context,
            &mut reviewer_initialized,
            "tools/call",
            json!(7),
            Some(&json!({
                "name":"actor_inbox",
                "arguments":{"limit":20,"include_delivered":true}
            })),
        )
        .await;
        assert_eq!(delivered_resp["result"]["isError"], false);
        let delivered_messages = delivered_resp["result"]["structuredContent"]["messages"]
            .as_array()
            .expect("delivered messages");
        assert_eq!(delivered_messages.len(), 1);
        assert_eq!(delivered_messages[0]["status"], "delivered");
    }
}
