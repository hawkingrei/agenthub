use agenthub_acp::AcpPermissionReviewRequest;
use serde_json::{Value, json};

use super::TEAM_PERMISSION_REVIEW_PAYLOAD_TYPE;

pub(super) fn build_permission_review_payload(
    request: &AcpPermissionReviewRequest,
    review_target_actor_id: &str,
) -> Value {
    json!({
        "type": TEAM_PERMISSION_REVIEW_PAYLOAD_TYPE,
        "source_kind": "system",
        "source_surface": "system",
        "requires_user_visible_reply": false,
        "permission_id": request.request_id,
        "requester_actor_id": request.routing.requester_actor_id,
        "requester_role": request.routing.requester_role,
        "review_target_actor_id": review_target_actor_id,
        "tool_call_id": request.tool_call_id,
        "tool_call": request.tool_call,
        "options": request.options,
        "summary": build_permission_review_summary(request),
        "instruction": "Review the request through the ACP permission approval flow. The reviewer is assigned automatically by the Team runtime.",
    })
}

pub(super) fn build_permission_review_summary(request: &AcpPermissionReviewRequest) -> String {
    let requester = request
        .routing
        .requester_actor_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("worker");
    let tool_name = request
        .tool_call
        .as_ref()
        .and_then(extract_tool_name)
        .unwrap_or("an ACP tool call");
    format!("{requester} requests permission to execute {tool_name}.")
}

pub(super) fn human_review_reason_text(reason: &str) -> &str {
    match reason {
        "review_timeout" => "Agent review timed out",
        "review_dispatch_failed" | "coordinator_dispatch_failed" => "Agent review dispatch failed",
        other => other,
    }
}

pub(super) fn extract_tool_name(value: &Value) -> Option<&str> {
    let obj = value.as_object()?;
    obj.get("title")
        .and_then(Value::as_str)
        .or_else(|| obj.get("name").and_then(Value::as_str))
        .or_else(|| {
            obj.get("tool")
                .and_then(Value::as_object)
                .and_then(|tool| tool.get("name"))
                .and_then(Value::as_str)
        })
}
