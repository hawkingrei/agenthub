use agent_client_protocol::{RequestPermissionOutcome, SelectedPermissionOutcome};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, ActorAckRequest, ActorInboxRequest,
    ActorMailboxService, ActorMessageStatus, ActorMessageTransport, ActorSendRequest,
    ActorServiceError, actor_inbox_with_auto_ack, build_default_actor_message_idempotency_key,
    parse_actor_transport,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::acp::DEFAULT_ACTOR_CHANNEL;
use crate::acp::{AcpPermissionRespondResult, AcpPermissionService};
use crate::agent::{AgentTimeTriggerCreateInput, AgentTimeTriggerManager};
use crate::team::TeamManager;

const ACTOR_RUNTIME_TEAM_ID_ENV: &str = "AGENTHUB_ACTOR_TEAM_ID";
const ACTOR_RUNTIME_CURRENT_RUN_ID_ENV: &str = "AGENTHUB_ACTOR_CURRENT_RUN_ID";
const ACTOR_RUNTIME_ACTOR_ID_ENV: &str = "AGENTHUB_ACTOR_ID";
const ACTOR_RUNTIME_AGENT_ID_ENV: &str = "AGENTHUB_ACTOR_AGENT_ID";
const ACTOR_RUNTIME_CHANNEL_ENV: &str = "AGENTHUB_ACTOR_CHANNEL";

const JSONRPC_PARSE_ERROR: i32 = -32700;
const JSONRPC_INVALID_REQUEST: i32 = -32600;
const JSONRPC_METHOD_NOT_FOUND: i32 = -32601;
const JSONRPC_INVALID_PARAMS: i32 = -32602;

#[derive(Debug, Clone)]
struct ActorMcpContext {
    team_id: Option<String>,
    current_run_id: Option<String>,
    actor_id: String,
    default_channel: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ActorInboxToolArgs {
    run_id: Option<String>,
    limit: Option<i64>,
    cursor: Option<i64>,
    include_delivered: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ActorAckToolArgs {
    #[serde(default)]
    run_id: Option<String>,
    message_id: i64,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ActorSendToolArgs {
    run_id: Option<String>,
    to_actor_id: Option<String>,
    payload: Option<Value>,
    channel: Option<String>,
    transport: Option<String>,
    route: Option<Value>,
    idempotency_key: Option<String>,
    allow_duplicate: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct TeamMembersToolArgs {
    team_id: Option<String>,
    run_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AgentTimeTriggerSetToolArgs {
    delay_seconds: i64,
    message: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct AgentTimeTriggerListToolArgs {
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct AgentTimeTriggerCancelToolArgs {
    trigger_id: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct AcpPermissionReviewRespondToolArgs {
    permission_id: String,
    option_id: Option<String>,
    outcome: Option<String>,
}

fn actor_mcp_usage() -> &'static str {
    r#"Usage:
  agenthub actor-mcp [--team-id <team_id>] [--run-id <run_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--channel <name>]

Environment fallback:
  AGENTHUB_ACTOR_TEAM_ID
  AGENTHUB_ACTOR_CURRENT_RUN_ID
  AGENTHUB_ACTOR_ID
  AGENTHUB_ACTOR_AGENT_ID
  AGENTHUB_ACTOR_CHANNEL
"#
}

fn normalized_env_var(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn take_required(value: Option<String>, env_keys: &[&str], field: &str) -> anyhow::Result<String> {
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

fn parse_actor_mcp_context_with_env<F>(
    args: &[String],
    mut env_lookup: F,
) -> anyhow::Result<ActorMcpContext>
where
    F: FnMut(&str) -> Option<String>,
{
    let mut team_id = None;
    let mut run_id = None;
    let mut actor_id = None;
    let mut channel = None;

    let mut idx = 0;
    while idx < args.len() {
        match args[idx].as_str() {
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
            "--actor-id" | "--agent-id" => {
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

    let team_id = take_optional(team_id).or_else(|| env_lookup(ACTOR_RUNTIME_TEAM_ID_ENV));
    let current_run_id =
        take_optional(run_id).or_else(|| env_lookup(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV));
    let actor_id = take_required(
        actor_id
            .or_else(|| env_lookup(ACTOR_RUNTIME_ACTOR_ID_ENV))
            .or_else(|| env_lookup(ACTOR_RUNTIME_AGENT_ID_ENV)),
        &[],
        "actor_id",
    )?;
    let default_channel = take_optional(channel)
        .or_else(|| env_lookup(ACTOR_RUNTIME_CHANNEL_ENV))
        .unwrap_or_else(|| DEFAULT_ACTOR_CHANNEL.to_string());

    Ok(ActorMcpContext {
        team_id,
        current_run_id,
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
                    "run_id": { "type": "string", "minLength": 1 },
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
                    "run_id": { "type": "string", "minLength": 1 },
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
                    "run_id": { "type": "string", "minLength": 1 },
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
        json!({
            "name": "team_members",
            "description": "Return current team runtime summary, member roster/card details, and optional run step overlay.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "team_id": { "type": "string", "minLength": 1 },
                    "run_id": { "type": "string", "minLength": 1 }
                }
            }
        }),
        json!({
            "name": "agent_time_trigger_set",
            "description": "Create a one-shot time trigger that will inject a future ACP prompt back into the current agent.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["delay_seconds", "message"],
                "properties": {
                    "delay_seconds": { "type": "integer", "minimum": 1, "maximum": 2592000 },
                    "message": { "type": "string", "minLength": 1 }
                }
            }
        }),
        json!({
            "name": "agent_time_trigger_list",
            "description": "List recent time triggers for the current agent.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 20 }
                }
            }
        }),
        json!({
            "name": "agent_time_trigger_cancel",
            "description": "Cancel a pending time trigger for the current agent.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["trigger_id"],
                "properties": {
                    "trigger_id": { "type": "string", "minLength": 1 }
                }
            }
        }),
        json!({
            "name": "acp_permission_review_respond",
            "description": "Approve or cancel a pending ACP permission request for your Team.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["permission_id"],
                "properties": {
                    "permission_id": { "type": "string", "minLength": 1 },
                    "option_id": { "type": "string", "minLength": 1 },
                    "outcome": { "type": "string", "enum": ["cancelled"] }
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

fn resolve_tool_run_id(explicit: Option<String>, current: Option<&str>) -> Result<String, String> {
    if let Some(run_id) = take_optional(explicit) {
        return Ok(run_id);
    }
    if let Some(run_id) = current.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }) {
        return Ok(run_id);
    }
    Err("run_id is required for this tool call".to_string())
}

struct ResolveIdempotencyKeyInput<'a> {
    run_id: &'a str,
    from_actor_id: &'a str,
    from_peer_id: &'a str,
    to_actor_id: &'a str,
    to_peer_id: &'a str,
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
            input.from_peer_id,
            input.to_actor_id,
            input.to_peer_id,
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
    let run_id = match resolve_tool_run_id(args.run_id, context.current_run_id.as_deref()) {
        Ok(run_id) => run_id,
        Err(err) => return tool_result_error(err, None),
    };
    let states = if args.include_delivered.unwrap_or(false) {
        Some(vec![
            ActorMessageStatus::Pending,
            ActorMessageStatus::Delivered,
        ])
    } else {
        Some(vec![ActorMessageStatus::Pending])
    };
    let response = actor_inbox_with_auto_ack(
        service,
        ActorInboxRequest {
            run_id,
            actor_id: context.actor_id.clone(),
            cursor: args.cursor,
            limit: Some(limit),
            states,
        },
    )
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
    let run_id = match resolve_tool_run_id(args.run_id, context.current_run_id.as_deref()) {
        Ok(run_id) => run_id,
        Err(err) => return tool_result_error(err, None),
    };
    let response = service
        .actor_ack(ActorAckRequest {
            run_id,
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
    manager: &TeamManager,
    permissions: &AcpPermissionService,
    context: &ActorMcpContext,
    arguments: Option<&Map<String, Value>>,
) -> Value {
    let args = match parse_tool_args::<ActorSendToolArgs>(arguments) {
        Ok(args) => args,
        Err(err) => return tool_result_error(err, None),
    };
    let to_actor_id = match take_required(args.to_actor_id, &[], "to_actor_id") {
        Ok(value) => value,
        Err(err) => return tool_result_error(err.to_string(), None),
    };
    let mut payload = match args.payload {
        Some(payload) => payload,
        None => return tool_result_error("payload is required", None),
    };
    let transport = match parse_actor_transport(args.transport.as_deref()) {
        Ok(transport) => transport,
        Err(err) => return tool_result_error(err.to_string(), None),
    };
    let route = args.route;
    let run_id = match resolve_tool_run_id(args.run_id, context.current_run_id.as_deref()) {
        Ok(run_id) => run_id,
        Err(err) => return tool_result_error(err, None),
    };
    if transport == ActorMessageTransport::Remote && route.is_none() {
        return tool_result_error("route is required for remote transport", None);
    }
    if transport == ActorMessageTransport::Local && route.is_some() {
        return tool_result_error("route is not supported for local transport", None);
    }
    let channel =
        take_optional(args.channel).unwrap_or_else(|| context.default_channel.to_string());
    let allow_duplicate = args.allow_duplicate.unwrap_or(false);
    let permission_review_request_id = match maybe_prepare_permission_review_delegation(
        manager,
        permissions,
        context,
        &to_actor_id,
        &mut payload,
    )
    .await
    {
        Ok(request_id) => request_id,
        Err(err) => return tool_result_error(format!("actor_send failed: {err}"), None),
    };
    let to_peer_id = if transport == ActorMessageTransport::Remote {
        ACTOR_NODE_PEER_ID
    } else {
        ACTOR_MAIN_PEER_ID
    };
    let idempotency_key = match resolve_idempotency_key(ResolveIdempotencyKeyInput {
        run_id: &run_id,
        from_actor_id: &context.actor_id,
        from_peer_id: ACTOR_MAIN_PEER_ID,
        to_actor_id: &to_actor_id,
        to_peer_id,
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
            run_id: run_id.clone(),
            from_actor_id: context.actor_id.clone(),
            from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            to_actor_id: to_actor_id.clone(),
            to_peer_id: Some(to_peer_id.to_string()),
            channel: Some(channel),
            transport: Some(transport),
            route,
            payload,
            idempotency_key,
        })
        .await;
    match response {
        Ok(response) => {
            if let Some(permission_id) = permission_review_request_id.as_deref() {
                if let Err(err) = permissions
                    .record_review_dispatch(
                        permission_id,
                        Some(to_actor_id.as_str()),
                        "leader_delegated",
                        Some(run_id.as_str()),
                        Some(response.message_id),
                    )
                    .await
                {
                    return tool_result_error(
                        format!("actor_send failed: {err}"),
                        Some(json!({
                            "permission_id": permission_id,
                            "message_id": response.message_id,
                        })),
                    );
                }
            }
            tool_result_success(json!({
                "message_id": response.message_id,
                "state": response.state,
                "deduped": response.deduped,
                "created_at": response.created_at,
                "message": response.message,
            }))
        }
        Err(err) => tool_result_actor_service_error(err),
    }
}

async fn maybe_prepare_permission_review_delegation(
    manager: &TeamManager,
    permissions: &AcpPermissionService,
    context: &ActorMcpContext,
    to_actor_id: &str,
    payload: &mut Value,
) -> Result<Option<String>, String> {
    let Some(payload_obj) = payload.as_object_mut() else {
        return Ok(None);
    };
    if payload_obj.get("type").and_then(Value::as_str) != Some("permission_review_request") {
        return Ok(None);
    }
    let permission_id = payload_obj
        .get("permission_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "permission_review_request payload requires permission_id".to_string())?;
    let Some(team_id) = context.team_id.as_deref() else {
        return Err("team_id is required to delegate permission review".to_string());
    };
    let team = manager
        .get_team(team_id)
        .await
        .map_err(|err| format!("load team failed: {err}"))?;
    let leader_member_id = team
        .spec
        .get("leader_member_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "team has no leader configured".to_string())?;
    if context.actor_id != leader_member_id {
        return Err("only leader may delegate permission review requests".to_string());
    }
    let record = permissions
        .get(&permission_id)
        .await
        .map_err(|err| format!("load permission request failed: {err}"))?
        .ok_or_else(|| "permission request not found".to_string())?;
    if record.team_id.as_deref() != Some(team_id) {
        return Err("permission request does not belong to this team".to_string());
    }
    if record.status != "pending" {
        return Err("permission request is already resolved".to_string());
    }
    if !manager
        .team_has_member(team_id, to_actor_id)
        .await
        .map_err(|err| format!("load team members failed: {err}"))?
    {
        return Err("delegated reviewer is not a member of this team".to_string());
    }
    if record.requester_actor_id.as_deref() == Some(to_actor_id) {
        return Err("requester cannot review its own permission request".to_string());
    }
    payload_obj.insert(
        "review_target_actor_id".to_string(),
        Value::String(to_actor_id.to_string()),
    );
    Ok(Some(permission_id))
}

async fn tool_team_members(
    manager: &TeamManager,
    context: &ActorMcpContext,
    arguments: Option<&Map<String, Value>>,
) -> Value {
    let args = match parse_tool_args::<TeamMembersToolArgs>(arguments) {
        Ok(args) => args,
        Err(err) => return tool_result_error(err, None),
    };
    let team_id = take_optional(args.team_id).or_else(|| context.team_id.clone());
    let run_id = take_optional(args.run_id).or_else(|| context.current_run_id.clone());
    if team_id.is_none() && run_id.is_none() {
        return tool_result_error("team_id or run_id is required for this tool call", None);
    }
    match manager
        .describe_team_context(team_id.as_deref(), run_id.as_deref())
        .await
    {
        Ok(team_context) => tool_result_success(json!(team_context)),
        Err(err) => tool_result_error(format!("team_members failed: {err}"), None),
    }
}

async fn tool_agent_time_trigger_set(
    trigger_manager: &AgentTimeTriggerManager,
    context: &ActorMcpContext,
    arguments: Option<&Map<String, Value>>,
) -> Value {
    let args = match parse_tool_args::<AgentTimeTriggerSetToolArgs>(arguments) {
        Ok(args) => args,
        Err(err) => return tool_result_error(err, None),
    };
    if !(1..=60 * 60 * 24 * 30).contains(&args.delay_seconds) {
        return tool_result_error("delay_seconds must be between 1 and 2592000", None);
    }
    let message = args.message.trim();
    if message.is_empty() {
        return tool_result_error("message must be a non-empty string", None);
    }
    let fire_at = chrono::Utc::now().timestamp() + args.delay_seconds;
    match trigger_manager
        .create_time_trigger(AgentTimeTriggerCreateInput {
            agent_id: context.actor_id.clone(),
            created_by_actor_id: context.actor_id.clone(),
            message_text: message.to_string(),
            fire_at,
        })
        .await
    {
        Ok(record) => tool_result_success(json!(record)),
        Err(err) => tool_result_error(format!("agent_time_trigger_set failed: {err}"), None),
    }
}

async fn tool_agent_time_trigger_list(
    trigger_manager: &AgentTimeTriggerManager,
    context: &ActorMcpContext,
    arguments: Option<&Map<String, Value>>,
) -> Value {
    let args = match parse_tool_args::<AgentTimeTriggerListToolArgs>(arguments) {
        Ok(args) => args,
        Err(err) => return tool_result_error(err, None),
    };
    match trigger_manager
        .list_triggers_for_agent(context.actor_id.as_str(), args.limit.unwrap_or(20))
        .await
    {
        Ok(records) => tool_result_success(json!(records)),
        Err(err) => tool_result_error(format!("agent_time_trigger_list failed: {err}"), None),
    }
}

async fn tool_agent_time_trigger_cancel(
    trigger_manager: &AgentTimeTriggerManager,
    context: &ActorMcpContext,
    arguments: Option<&Map<String, Value>>,
) -> Value {
    let args = match parse_tool_args::<AgentTimeTriggerCancelToolArgs>(arguments) {
        Ok(args) => args,
        Err(err) => return tool_result_error(err, None),
    };
    let trigger_id = args.trigger_id.trim();
    if trigger_id.is_empty() {
        return tool_result_error("trigger_id must be a non-empty string", None);
    }
    match trigger_manager
        .cancel_trigger(context.actor_id.as_str(), trigger_id)
        .await
    {
        Ok(true) => tool_result_success(json!({ "status": "ok", "trigger_id": trigger_id })),
        Ok(false) => tool_result_error("agent_time_trigger_cancel failed: trigger not found", None),
        Err(err) => tool_result_error(format!("agent_time_trigger_cancel failed: {err}"), None),
    }
}

async fn tool_acp_permission_review_respond(
    permissions: &AcpPermissionService,
    manager: &TeamManager,
    context: &ActorMcpContext,
    arguments: Option<&Map<String, Value>>,
) -> Value {
    let args = match parse_tool_args::<AcpPermissionReviewRespondToolArgs>(arguments) {
        Ok(args) => args,
        Err(err) => return tool_result_error(err, None),
    };
    let permission_id = args.permission_id.trim();
    if permission_id.is_empty() {
        return tool_result_error("permission_id must be a non-empty string", None);
    }
    let Some(record) = (match permissions.get(permission_id).await {
        Ok(record) => record,
        Err(err) => {
            return tool_result_error(format!("acp_permission_review_respond failed: {err}"), None);
        }
    }) else {
        return tool_result_error("permission request not found", None);
    };
    let Some(team_id) = context.team_id.as_deref() else {
        return tool_result_error("team_id is required for permission review", None);
    };
    if record.team_id.as_deref() != Some(team_id) {
        return tool_result_error("permission request does not belong to this team", None);
    }
    if !manager
        .team_has_member(team_id, context.actor_id.as_str())
        .await
        .unwrap_or(false)
    {
        return tool_result_error("current actor is not a member of this team", None);
    }
    let team = match manager.get_team(team_id).await {
        Ok(team) => team,
        Err(err) => {
            return tool_result_error(
                format!("acp_permission_review_respond failed: load team failed: {err}"),
                None,
            );
        }
    };
    let leader_member_id = team
        .spec
        .get("leader_member_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if record.requester_actor_id.as_deref() == Some(context.actor_id.as_str()) {
        return tool_result_error("requester cannot review its own permission request", None);
    }
    let authorized_actor = leader_member_id == Some(context.actor_id.as_str())
        || record.review_target_actor_id.as_deref() == Some(context.actor_id.as_str());
    if !authorized_actor {
        return tool_result_error(
            "current actor is not the active reviewer for this permission request",
            None,
        );
    }
    if record.status != "pending" {
        return tool_result_success(json!({
            "status": "already_resolved",
            "permission_id": permission_id,
            "request_status": record.status,
        }));
    }

    let outcome = if let Some(option_id) = args.option_id.as_ref() {
        RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option_id.clone()))
    } else {
        match args.outcome.as_deref() {
            Some("cancelled") | None => RequestPermissionOutcome::Cancelled,
            Some(other) => {
                return tool_result_error(
                    format!("unsupported outcome '{other}', expected 'cancelled'"),
                    None,
                );
            }
        }
    };

    let respond_result = match permissions
        .respond(
            permission_id,
            outcome,
            args.option_id.clone(),
            Some(context.actor_id.clone()),
        )
        .await
    {
        Ok(result) => result,
        Err(err) => {
            return tool_result_error(format!("acp_permission_review_respond failed: {err}"), None);
        }
    };
    if matches!(respond_result, AcpPermissionRespondResult::AlreadyResolved) {
        let request_status = permissions
            .get(permission_id)
            .await
            .ok()
            .flatten()
            .map(|current| current.status)
            .unwrap_or_else(|| "resolved".to_string());
        return tool_result_success(json!({
            "status": "already_resolved",
            "permission_id": permission_id,
            "request_status": request_status,
        }));
    }
    tool_result_success(json!({
        "status": "ok",
        "permission_id": permission_id,
        "reviewed_by_actor_id": context.actor_id,
    }))
}

async fn handle_tool_call<S: ActorMailboxService>(
    tool_context: &ActorToolContext<'_, S>,
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
        "actor_inbox" => tool_actor_inbox(tool_context.service, context, arguments).await,
        "actor_ack" => tool_actor_ack(tool_context.service, context, arguments).await,
        "actor_send" => {
            tool_actor_send(
                tool_context.service,
                tool_context.manager,
                tool_context.permissions,
                context,
                arguments,
            )
            .await
        }
        "team_members" => tool_team_members(tool_context.manager, context, arguments).await,
        "agent_time_trigger_set" => {
            tool_agent_time_trigger_set(tool_context.trigger_manager, context, arguments).await
        }
        "agent_time_trigger_list" => {
            tool_agent_time_trigger_list(tool_context.trigger_manager, context, arguments).await
        }
        "agent_time_trigger_cancel" => {
            tool_agent_time_trigger_cancel(tool_context.trigger_manager, context, arguments).await
        }
        "acp_permission_review_respond" => {
            tool_acp_permission_review_respond(
                tool_context.permissions,
                tool_context.manager,
                context,
                arguments,
            )
            .await
        }
        other => tool_result_error(format!("unknown tool: {}", other), None),
    };
    Ok(result)
}

struct ActorToolContext<'a, S: ActorMailboxService> {
    service: &'a S,
    manager: &'a TeamManager,
    trigger_manager: &'a AgentTimeTriggerManager,
    permissions: &'a AcpPermissionService,
}

async fn handle_jsonrpc_request<S: ActorMailboxService>(
    tool_context: &ActorToolContext<'_, S>,
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
                    "instructions": "Use actor_inbox / actor_ack / actor_send for Team mailbox coordination, team_members for Team runtime context, and acp_permission_review_respond when a Team permission review request arrives."
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
        "tools/call" => match handle_tool_call(tool_context, context, params).await {
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
    let db = agenthub_db::init_db().await?;
    let trigger_manager = AgentTimeTriggerManager::new(db.clone());
    let permissions = AcpPermissionService::new(db.clone());
    let manager = TeamManager::new(db);
    let service = manager.actor_mailbox_service();

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();
    let mut initialized = false;
    let tool_context = ActorToolContext {
        service: &service,
        manager: &manager,
        trigger_manager: &trigger_manager,
        permissions: &permissions,
    };

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
                &tool_context,
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
    use sqlx::Row;
    use uuid::Uuid;

    #[test]
    fn parse_actor_mcp_context_uses_env_fallback() {
        let env = [
            (ACTOR_RUNTIME_TEAM_ID_ENV.to_string(), "team-x".to_string()),
            (
                ACTOR_RUNTIME_CURRENT_RUN_ID_ENV.to_string(),
                "run-x".to_string(),
            ),
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
        assert_eq!(context.team_id.as_deref(), Some("team-x"));
        assert_eq!(context.current_run_id.as_deref(), Some("run-x"));
        assert_eq!(context.actor_id, "planner");
        assert_eq!(context.default_channel, "coord");
    }

    #[test]
    fn parse_actor_mcp_context_uses_agent_id_env_alias() {
        let env = [
            (
                ACTOR_RUNTIME_CURRENT_RUN_ID_ENV.to_string(),
                "run-x".to_string(),
            ),
            (
                ACTOR_RUNTIME_AGENT_ID_ENV.to_string(),
                "planner-agent".to_string(),
            ),
        ]
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>();
        let context = parse_actor_mcp_context_with_env(&[], |key| env.get(key).cloned())
            .expect("parse actor mcp context");
        assert_eq!(context.actor_id, "planner-agent");
    }

    #[test]
    fn parse_actor_mcp_context_accepts_agent_id_flag() {
        let args = vec![
            "--team-id".to_string(),
            "team-flag".to_string(),
            "--run-id".to_string(),
            "run-flag".to_string(),
            "--agent-id".to_string(),
            "planner-agent".to_string(),
        ];
        let context =
            parse_actor_mcp_context_with_env(&args, |_| None).expect("parse actor mcp context");
        assert_eq!(context.team_id.as_deref(), Some("team-flag"));
        assert_eq!(context.current_run_id.as_deref(), Some("run-flag"));
        assert_eq!(context.actor_id, "planner-agent");
    }

    #[test]
    fn parse_actor_mcp_context_requires_actor_id_without_env_fallback() {
        let err =
            parse_actor_mcp_context_with_env(&[], |_| None).expect_err("actor_id is required");
        assert!(
            err.to_string().contains("actor_id is required"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_actor_mcp_context_defaults_channel_when_missing() {
        let env = [
            (
                ACTOR_RUNTIME_CURRENT_RUN_ID_ENV.to_string(),
                "run-x".to_string(),
            ),
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
    fn parse_actor_mcp_context_ignores_legacy_run_env_alias() {
        let env = [
            (
                "AGENTHUB_ACTOR_RUN_ID".to_string(),
                "run-legacy-only".to_string(),
            ),
            (
                ACTOR_RUNTIME_ACTOR_ID_ENV.to_string(),
                "planner".to_string(),
            ),
        ]
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>();
        let context = parse_actor_mcp_context_with_env(&[], |key| env.get(key).cloned())
            .expect("parse actor mcp context");
        assert!(context.current_run_id.is_none());
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
        assert_eq!(
            names,
            vec![
                "actor_inbox",
                "actor_ack",
                "actor_send",
                "team_members",
                "agent_time_trigger_set",
                "agent_time_trigger_list",
                "agent_time_trigger_cancel",
                "acp_permission_review_respond",
            ]
        );
    }

    #[tokio::test]
    async fn agent_time_trigger_tools_roundtrip() {
        let state = build_test_state().await;
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS agent_time_triggers (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                created_by_actor_id TEXT NOT NULL,
                message_text TEXT NOT NULL,
                fire_at INTEGER NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                fired_at INTEGER,
                last_error TEXT
            )
            "#,
        )
        .execute(&state.db)
        .await
        .expect("create agent_time_triggers");
        let now = chrono::Utc::now().timestamp();
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
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'created', ?7, ?8)
            "#,
        )
        .bind("trigger-agent")
        .bind("trigger-agent")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert trigger agent");
        let manager = AgentTimeTriggerManager::new(state.db.clone());
        let context = ActorMcpContext {
            team_id: None,
            current_run_id: None,
            actor_id: "trigger-agent".to_string(),
            default_channel: DEFAULT_ACTOR_CHANNEL.to_string(),
        };

        let created = tool_agent_time_trigger_set(
            &manager,
            &context,
            Some(
                json!({
                    "delay_seconds": 30,
                    "message": "Check the queue again."
                })
                .as_object()
                .expect("args object"),
            ),
        )
        .await;
        assert_eq!(created["isError"], Value::Bool(false));
        let trigger_id = created["structuredContent"]["id"]
            .as_str()
            .expect("trigger id")
            .to_string();

        let listed = tool_agent_time_trigger_list(&manager, &context, None).await;
        let listed_records = listed["structuredContent"]
            .as_array()
            .expect("listed records");
        assert_eq!(listed_records.len(), 1);
        assert_eq!(listed_records[0]["id"], Value::from(trigger_id.clone()));

        let canceled = tool_agent_time_trigger_cancel(
            &manager,
            &context,
            Some(
                json!({
                    "trigger_id": trigger_id
                })
                .as_object()
                .expect("cancel args"),
            ),
        )
        .await;
        assert_eq!(canceled["isError"], Value::Bool(false));
    }

    #[tokio::test]
    async fn acp_permission_review_respond_updates_pending_team_request() {
        let state = build_test_state().await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: format!("actor-mcp-permission-review-{}", Uuid::new_v4()),
                description: Some("actor mcp permission review".to_string()),
                spec: json!({
                    "entrypoint":"leader",
                    "leader_member_id":"leader",
                    "members":[
                        {"member_id":"leader","role":"leader"},
                        {"member_id":"worker","role":"worker"}
                    ]
                }),
            })
            .await
            .expect("create team");
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'running', ?7, ?8)
            "#,
        )
        .bind("worker-agent")
        .bind("worker-agent")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker agent");
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind("worker-session")
        .bind("worker-agent")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker session");
        sqlx::query(
            r#"
            INSERT INTO acp_permission_requests (
                id,
                agent_id,
                session_id,
                acp_session_id,
                team_id,
                requester_actor_id,
                requester_role,
                tool_call_id,
                options_json,
                tool_call_json,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11)
            "#,
        )
        .bind("perm-review-1")
        .bind("worker-agent")
        .bind("worker-session")
        .bind("acp-session-1")
        .bind(&team.id)
        .bind("worker")
        .bind("worker")
        .bind("tool-call-1")
        .bind(
            json!([
                {
                    "option_id": "allow",
                    "name": "Allow once",
                    "kind": "allow_once"
                }
            ])
            .to_string(),
        )
        .bind(json!({"tool":{"name":"mcp__fs__read"}}).to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert permission request");

        let permissions = AcpPermissionService::new(state.db.clone());
        let context = ActorMcpContext {
            team_id: Some(team.id.clone()),
            current_run_id: None,
            actor_id: "leader".to_string(),
            default_channel: DEFAULT_ACTOR_CHANNEL.to_string(),
        };

        let response = tool_acp_permission_review_respond(
            &permissions,
            &state.teams,
            &context,
            Some(
                json!({
                    "permission_id": "perm-review-1",
                    "option_id": "allow"
                })
                .as_object()
                .expect("review args"),
            ),
        )
        .await;
        assert_eq!(response["isError"], Value::Bool(false), "{response}");

        let row = sqlx::query(
            "SELECT status, selected_option_id, reviewed_by_actor_id FROM acp_permission_requests WHERE id = ?1",
        )
        .bind("perm-review-1")
        .fetch_one(&state.db)
        .await
        .expect("load permission request");
        assert_eq!(row.get::<String, _>("status"), "responded");
        assert_eq!(row.get::<String, _>("selected_option_id"), "allow");
        assert_eq!(row.get::<String, _>("reviewed_by_actor_id"), "leader");
    }

    #[tokio::test]
    async fn acp_permission_review_respond_reports_already_resolved_for_second_reviewer() {
        let state = build_test_state().await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: format!("actor-mcp-permission-race-{}", Uuid::new_v4()),
                description: Some("actor mcp permission race".to_string()),
                spec: json!({
                    "entrypoint":"leader",
                    "leader_member_id":"leader",
                    "members":[
                        {"member_id":"leader","role":"leader"},
                        {"member_id":"worker","role":"worker"}
                    ]
                }),
            })
            .await
            .expect("create team");
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'running', ?7, ?8)
            "#,
        )
        .bind("worker-agent-race")
        .bind("worker-agent-race")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker agent");
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind("worker-session-race")
        .bind("worker-agent-race")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker session");
        sqlx::query(
            r#"
            INSERT INTO acp_permission_requests (
                id,
                agent_id,
                session_id,
                acp_session_id,
                team_id,
                requester_actor_id,
                requester_role,
                tool_call_id,
                options_json,
                tool_call_json,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11)
            "#,
        )
        .bind("perm-review-race")
        .bind("worker-agent-race")
        .bind("worker-session-race")
        .bind("acp-session-race")
        .bind(&team.id)
        .bind("worker")
        .bind("worker")
        .bind("tool-call-race")
        .bind(
            json!([
                {
                    "option_id": "allow",
                    "name": "Allow once",
                    "kind": "allow_once"
                }
            ])
            .to_string(),
        )
        .bind(json!({"tool":{"name":"mcp__fs__read"}}).to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert permission request");

        let permissions = AcpPermissionService::new(state.db.clone());
        let leader = ActorMcpContext {
            team_id: Some(team.id.clone()),
            current_run_id: None,
            actor_id: "leader".to_string(),
            default_channel: DEFAULT_ACTOR_CHANNEL.to_string(),
        };

        let first = tool_acp_permission_review_respond(
            &permissions,
            &state.teams,
            &leader,
            Some(
                json!({
                    "permission_id": "perm-review-race",
                    "option_id": "allow"
                })
                .as_object()
                .expect("review args"),
            ),
        )
        .await;
        assert_eq!(first["isError"], Value::Bool(false), "{first}");
        let first_payload = serde_json::from_str::<Value>(
            first["content"][0]["text"]
                .as_str()
                .expect("first payload text"),
        )
        .expect("parse first payload");
        assert_eq!(first_payload["status"], "ok");
        assert_eq!(first_payload["permission_id"], "perm-review-race");
        assert_eq!(first_payload["reviewed_by_actor_id"], "leader");

        let second = tool_acp_permission_review_respond(
            &permissions,
            &state.teams,
            &leader,
            Some(
                json!({
                    "permission_id": "perm-review-race",
                    "outcome": "cancelled"
                })
                .as_object()
                .expect("second review args"),
            ),
        )
        .await;
        assert_eq!(second["isError"], Value::Bool(false), "{second}");
        let second_payload = second["content"][0]["text"]
            .as_str()
            .expect("second payload text");
        assert!(
            second_payload.contains("\"status\":\"already_resolved\""),
            "{second_payload}"
        );
        assert!(
            second_payload.contains("\"request_status\":\"responded\""),
            "{second_payload}"
        );
    }

    #[tokio::test]
    async fn leader_delegation_updates_active_reviewer_and_blocks_other_members() {
        let state = build_test_state().await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: format!("actor-mcp-permission-delegate-{}", Uuid::new_v4()),
                description: Some("actor mcp permission delegation".to_string()),
                spec: json!({
                    "entrypoint":"leader",
                    "leader_member_id":"leader",
                    "members":[
                        {"member_id":"leader","role":"leader"},
                        {"member_id":"worker","role":"worker"},
                        {"member_id":"reviewer","role":"worker"},
                        {"member_id":"observer","role":"worker"}
                    ]
                }),
            })
            .await
            .expect("create team");
        let run = state
            .teams
            .create_run(&team.id, Some("ctx-permission-delegate"), json!({"goal":"review"}))
            .await
            .expect("create run");
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'running', ?7, ?8)
            "#,
        )
        .bind("worker-agent-delegate")
        .bind("worker-agent-delegate")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker agent");
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind("worker-session-delegate")
        .bind("worker-agent-delegate")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker session");
        sqlx::query(
            r#"
            INSERT INTO acp_permission_requests (
                id,
                agent_id,
                session_id,
                acp_session_id,
                team_id,
                requester_actor_id,
                requester_role,
                review_target_actor_id,
                review_dispatch_status,
                review_delivery_run_id,
                tool_call_id,
                options_json,
                tool_call_json,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'pending', ?14)
            "#,
        )
        .bind("perm-review-delegate")
        .bind("worker-agent-delegate")
        .bind("worker-session-delegate")
        .bind("acp-session-delegate")
        .bind(&team.id)
        .bind("worker")
        .bind("worker")
        .bind("leader")
        .bind("leader_dispatched")
        .bind(&run.id)
        .bind("tool-call-delegate")
        .bind(
            json!([
                {
                    "option_id": "allow",
                    "name": "Allow once",
                    "kind": "allow_once"
                }
            ])
            .to_string(),
        )
        .bind(json!({"tool":{"name":"mcp__fs__read"}}).to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert permission request");

        let service = state.teams.actor_mailbox_service();
        let permissions = AcpPermissionService::new(state.db.clone());
        let leader = ActorMcpContext {
            team_id: Some(team.id.clone()),
            current_run_id: Some(run.id.clone()),
            actor_id: "leader".to_string(),
            default_channel: DEFAULT_ACTOR_CHANNEL.to_string(),
        };
        let reviewer = ActorMcpContext {
            team_id: Some(team.id.clone()),
            current_run_id: Some(run.id.clone()),
            actor_id: "reviewer".to_string(),
            default_channel: DEFAULT_ACTOR_CHANNEL.to_string(),
        };
        let observer = ActorMcpContext {
            team_id: Some(team.id.clone()),
            current_run_id: Some(run.id.clone()),
            actor_id: "observer".to_string(),
            default_channel: DEFAULT_ACTOR_CHANNEL.to_string(),
        };

        let delegated = tool_actor_send(
            &service,
            &state.teams,
            &permissions,
            &leader,
            Some(
                json!({
                    "to_actor_id":"reviewer",
                    "payload":{
                        "type":"permission_review_request",
                        "permission_id":"perm-review-delegate",
                        "review_target_actor_id":"leader"
                    }
                })
                .as_object()
                .expect("delegate args"),
            ),
        )
        .await;
        assert_eq!(delegated["isError"], Value::Bool(false), "{delegated}");

        let delegated_row = sqlx::query(
            "SELECT review_target_actor_id, review_dispatch_status FROM acp_permission_requests WHERE id = ?1",
        )
        .bind("perm-review-delegate")
        .fetch_one(&state.db)
        .await
        .expect("load delegated permission");
        assert_eq!(
            delegated_row.get::<String, _>("review_target_actor_id"),
            "reviewer"
        );
        assert_eq!(
            delegated_row.get::<String, _>("review_dispatch_status"),
            "leader_delegated"
        );

        let blocked = tool_acp_permission_review_respond(
            &permissions,
            &state.teams,
            &observer,
            Some(
                json!({
                    "permission_id": "perm-review-delegate",
                    "option_id": "allow"
                })
                .as_object()
                .expect("observer args"),
            ),
        )
        .await;
        assert_eq!(blocked["isError"], Value::Bool(true), "{blocked}");
        assert_eq!(
            blocked["content"][0]["text"],
            "current actor is not the active reviewer for this permission request"
        );

        let approved = tool_acp_permission_review_respond(
            &permissions,
            &state.teams,
            &reviewer,
            Some(
                json!({
                    "permission_id": "perm-review-delegate",
                    "option_id": "allow"
                })
                .as_object()
                .expect("reviewer args"),
            ),
        )
        .await;
        assert_eq!(approved["isError"], Value::Bool(false), "{approved}");
    }

    #[test]
    fn resolve_idempotency_key_rejects_conflicting_options() {
        let err = resolve_idempotency_key(ResolveIdempotencyKeyInput {
            run_id: "run-1",
            from_actor_id: "leader",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "worker",
            to_peer_id: ACTOR_MAIN_PEER_ID,
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
    fn resolve_tool_run_id_prefers_explicit_then_current_context() {
        assert_eq!(
            resolve_tool_run_id(Some("run-explicit".to_string()), Some("run-current"))
                .expect("resolve explicit"),
            "run-explicit"
        );
        assert_eq!(
            resolve_tool_run_id(None, Some("run-current")).expect("resolve current"),
            "run-current"
        );
        let err = resolve_tool_run_id(Some("   ".to_string()), Some("   "))
            .expect_err("run_id should be required");
        assert!(err.contains("run_id is required"));
    }

    #[test]
    fn resolve_idempotency_key_supports_allow_duplicate_without_explicit_key() {
        let key = resolve_idempotency_key(ResolveIdempotencyKeyInput {
            run_id: "run-1",
            from_actor_id: "leader",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "worker",
            to_peer_id: ACTOR_MAIN_PEER_ID,
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
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "worker",
            to_peer_id: ACTOR_MAIN_PEER_ID,
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
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "worker",
            to_peer_id: ACTOR_MAIN_PEER_ID,
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
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "worker",
            to_peer_id: ACTOR_NODE_PEER_ID,
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
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "worker",
            to_peer_id: ACTOR_NODE_PEER_ID,
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
        let trigger_manager = AgentTimeTriggerManager::new(state.db.clone());
        let permissions = AcpPermissionService::new(state.db.clone());
        let tool_context = ActorToolContext {
            service: &service,
            manager: &state.teams,
            trigger_manager: &trigger_manager,
            permissions: &permissions,
        };
        let context = ActorMcpContext {
            team_id: Some(team.id),
            current_run_id: Some(run.id),
            actor_id: "planner".to_string(),
            default_channel: "default".to_string(),
        };

        let mut initialized = false;
        let response = handle_jsonrpc_request(
            &tool_context,
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
        let trigger_manager = AgentTimeTriggerManager::new(state.db.clone());
        let permissions = AcpPermissionService::new(state.db.clone());
        let tool_context = ActorToolContext {
            service: &service,
            manager: &state.teams,
            trigger_manager: &trigger_manager,
            permissions: &permissions,
        };

        let mut planner_initialized = false;
        let planner_context = ActorMcpContext {
            team_id: Some(team.id.clone()),
            current_run_id: Some(run.id.clone()),
            actor_id: "planner".to_string(),
            default_channel: "coordination".to_string(),
        };
        let init_resp = handle_jsonrpc_request(
            &tool_context,
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
            &tool_context,
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
        assert_eq!(
            tool_names,
            vec![
                "actor_inbox",
                "actor_ack",
                "actor_send",
                "team_members",
                "agent_time_trigger_set",
                "agent_time_trigger_list",
                "agent_time_trigger_cancel",
                "acp_permission_review_respond",
            ]
        );

        let send_resp = handle_jsonrpc_request(
            &tool_context,
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
        assert_eq!(
            send_resp["result"]["isError"], false,
            "actor_send failed response: {send_resp}"
        );
        let message_id = send_resp["result"]["structuredContent"]["message_id"]
            .as_i64()
            .expect("message id");
        assert!(message_id > 0);

        let reviewer_context = ActorMcpContext {
            team_id: Some(team.id),
            current_run_id: Some(run.id),
            actor_id: "reviewer".to_string(),
            default_channel: "coordination".to_string(),
        };
        let mut reviewer_initialized = false;
        let _ = handle_jsonrpc_request(
            &tool_context,
            &reviewer_context,
            &mut reviewer_initialized,
            "initialize",
            json!(4),
            Some(&json!({"protocolVersion":"2025-03-26"})),
        )
        .await;
        assert!(reviewer_initialized);

        let inbox_resp = handle_jsonrpc_request(
            &tool_context,
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
        assert_eq!(inbox_messages[0]["status"], "delivered");

        let ack_resp = handle_jsonrpc_request(
            &tool_context,
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
            &tool_context,
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

    #[tokio::test]
    async fn jsonrpc_team_members_returns_live_roster_view() {
        let state = build_test_state().await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: format!("actor-mcp-team-members-{}", Uuid::new_v4()),
                description: Some("actor mcp team members".to_string()),
                spec: json!({
                    "entrypoint":"leader",
                    "members":[
                        {"member_id":"leader","role":"leader","description":"Lead planner"},
                        {"member_id":"worker","role":"worker","description":"Implements patches"}
                    ]
                }),
            })
            .await
            .expect("create team");
        let run = state
            .teams
            .create_run(
                &team.id,
                Some("ctx-actor-mcp-team-members"),
                json!({"prompt":"go"}),
            )
            .await
            .expect("create run");
        let leader_step = state
            .teams
            .submit_step(&run.id, "leader_plan", "leader", Vec::new(), None)
            .await
            .expect("submit leader step");
        state
            .teams
            .submit_step(
                &run.id,
                "worker_exec",
                "worker",
                vec!["leader_plan".to_string()],
                None,
            )
            .await
            .expect("submit worker step");

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9)
            "#,
        )
        .bind("leader")
        .bind("Leader Agent")
        .bind("/tmp/leader")
        .bind("codex")
        .bind("[]")
        .bind("use_existing")
        .bind("running")
        .bind(1_i64)
        .bind(1_i64)
        .execute(&state.db)
        .await
        .expect("insert leader agent");

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, ?7, ?8, ?9)
            "#,
        )
        .bind("worker")
        .bind("Worker Agent")
        .bind("/tmp/worker")
        .bind("codex")
        .bind("[]")
        .bind("create_worktree")
        .bind("idle")
        .bind(1_i64)
        .bind(1_i64)
        .execute(&state.db)
        .await
        .expect("insert worker agent");

        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, ?3, ?4, NULL)
            "#,
        )
        .bind("session-leader")
        .bind("leader")
        .bind("running")
        .bind(1_i64)
        .execute(&state.db)
        .await
        .expect("insert leader session");

        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, ?3, ?4, NULL)
            "#,
        )
        .bind("session-worker")
        .bind("worker")
        .bind("running")
        .bind(1_i64)
        .execute(&state.db)
        .await
        .expect("insert worker session");

        state
            .teams
            .start_step(&leader_step.id, Some("session-leader"))
            .await
            .expect("start leader step");

        let team_id = team.id.clone();
        let run_id = run.id.clone();
        let service = state.teams.actor_mailbox_service();
        let trigger_manager = AgentTimeTriggerManager::new(state.db.clone());
        let permissions = AcpPermissionService::new(state.db.clone());
        let tool_context = ActorToolContext {
            service: &service,
            manager: &state.teams,
            trigger_manager: &trigger_manager,
            permissions: &permissions,
        };
        let context = ActorMcpContext {
            team_id: Some(team_id.clone()),
            current_run_id: Some(run_id),
            actor_id: "leader".to_string(),
            default_channel: "coordination".to_string(),
        };
        let mut initialized = false;
        let _ = handle_jsonrpc_request(
            &tool_context,
            &context,
            &mut initialized,
            "initialize",
            json!(1),
            Some(&json!({"protocolVersion":"2025-03-26"})),
        )
        .await;

        let response = handle_jsonrpc_request(
            &tool_context,
            &context,
            &mut initialized,
            "tools/call",
            json!(2),
            Some(&json!({
                "name":"team_members",
                "arguments":{}
            })),
        )
        .await;

        assert_eq!(response["result"]["isError"], false, "{response}");
        assert_eq!(response["result"]["structuredContent"]["team_id"], team_id);
        assert_eq!(
            response["result"]["structuredContent"]["runtime"]["status"],
            "running"
        );
        assert_eq!(
            response["result"]["structuredContent"]["runtime"]["online_count"],
            2
        );
        assert_eq!(
            response["result"]["structuredContent"]["runtime"]["member_count"],
            2
        );
        assert_eq!(
            response["result"]["structuredContent"]["run"]["run_id"],
            run.id
        );
        let members = response["result"]["structuredContent"]["members"]
            .as_array()
            .expect("members array");
        assert_eq!(members.len(), 2);
        assert_eq!(members[0]["display_name"], "Leader Agent");
        assert_eq!(members[0]["session_id"], "session-leader");
        assert_eq!(members[0]["session_status"], "running");
        assert_eq!(members[0]["steps"][0]["session_id"], "session-leader");
        assert_eq!(members[0]["steps"][0]["session_status"], "running");
        assert_eq!(members[1]["display_name"], "Worker Agent");
        assert_eq!(members[1]["description"], "Implements patches");
        assert_eq!(members[1]["session_id"], "session-worker");
        assert_eq!(members[1]["session_status"], "running");
        assert_eq!(members[1]["steps"][0]["status"], "submitted");
    }

    #[tokio::test]
    async fn jsonrpc_team_members_supports_runtime_only_context_without_run_overlay() {
        let state = build_test_state().await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: format!("actor-mcp-team-runtime-{}", Uuid::new_v4()),
                description: Some("actor mcp team runtime".to_string()),
                spec: json!({
                    "entrypoint":"leader",
                    "members":[
                        {"member_id":"leader","role":"leader","description":"Lead planner"},
                        {"member_id":"worker","role":"worker","description":"Implements patches"}
                    ]
                }),
            })
            .await
            .expect("create team");

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9)
            "#,
        )
        .bind("leader")
        .bind("Leader Agent")
        .bind("/tmp/leader-runtime-only")
        .bind("codex")
        .bind("[]")
        .bind("use_existing")
        .bind("running")
        .bind(1_i64)
        .bind(1_i64)
        .execute(&state.db)
        .await
        .expect("insert leader agent");

        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, ?3, ?4, NULL)
            "#,
        )
        .bind("session-leader-runtime-only")
        .bind("leader")
        .bind("running")
        .bind(1_i64)
        .execute(&state.db)
        .await
        .expect("insert leader session");

        let service = state.teams.actor_mailbox_service();
        let trigger_manager = AgentTimeTriggerManager::new(state.db.clone());
        let permissions = AcpPermissionService::new(state.db.clone());
        let tool_context = ActorToolContext {
            service: &service,
            manager: &state.teams,
            trigger_manager: &trigger_manager,
            permissions: &permissions,
        };
        let context = ActorMcpContext {
            team_id: Some(team.id.clone()),
            current_run_id: None,
            actor_id: "leader".to_string(),
            default_channel: "coordination".to_string(),
        };
        let mut initialized = false;
        let _ = handle_jsonrpc_request(
            &tool_context,
            &context,
            &mut initialized,
            "initialize",
            json!(1),
            Some(&json!({"protocolVersion":"2025-03-26"})),
        )
        .await;

        let response = handle_jsonrpc_request(
            &tool_context,
            &context,
            &mut initialized,
            "tools/call",
            json!(2),
            Some(&json!({
                "name":"team_members",
                "arguments":{}
            })),
        )
        .await;

        assert_eq!(response["result"]["isError"], false, "{response}");
        assert_eq!(response["result"]["structuredContent"]["team_id"], team.id);
        assert_eq!(
            response["result"]["structuredContent"]["runtime"]["status"],
            "degraded"
        );
        assert_eq!(
            response["result"]["structuredContent"]["runtime"]["online_count"],
            1
        );
        assert!(response["result"]["structuredContent"]["run"].is_null());
        let members = response["result"]["structuredContent"]["members"]
            .as_array()
            .expect("members array");
        assert_eq!(members.len(), 2);
        assert!(
            members[0]["steps"]
                .as_array()
                .expect("leader steps array")
                .is_empty()
        );
    }
}
