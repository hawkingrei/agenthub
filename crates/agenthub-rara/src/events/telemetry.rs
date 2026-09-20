use serde_json::{Value, json};

use crate::{ProtocolError, protocol::validate_id};

/// Source/catalogue/diagnostic events are observations, not commands or authority.
/// Only declared identifiers, counts and statuses reach provider update metadata.
pub(super) fn project(family: &str, kind: &str, payload: &Value) -> Result<Value, ProtocolError> {
    let allowed = match family {
        "prompt_source" => matches!(kind, "registered" | "injected" | "unregistered" | "dropped"),
        "skill" => matches!(
            kind,
            "catalogue"
                | "disabled"
                | "registered"
                | "unregistered"
                | "injected"
                | "shadowed"
                | "failed"
        ),
        "mcp" => matches!(
            kind,
            "status_updated"
                | "status_load_failed"
                | "server_state_changed"
                | "server_reconnecting"
                | "configuration_refreshed"
        ),
        "memory" => matches!(
            kind,
            "record_added"
                | "record_updated"
                | "record_deleted"
                | "labels_listed"
                | "metadata_queried"
                | "records_queried"
                | "action_observed"
                | "session_shard_promotion_observed"
                | "selection_updated"
        ),
        "hook" => matches!(kind, "declared" | "injected" | "ignored" | "command_output"),
        "context" => matches!(
            kind,
            "snapshot_updated" | "retrieval_orchestration_updated" | "observability_updated"
        ),
        "extension" => kind == "readiness_updated",
        _ => false,
    };
    if !allowed {
        return Err(ProtocolError::MalformedFrame);
    }
    let body = &payload["payload"];
    let mut projected = json!({"family": family, "event": kind});
    let identifier = match family {
        "prompt_source" => Some("source_id"),
        "skill" if !matches!(kind, "catalogue" | "shadowed") => Some("source_id"),
        "memory" if matches!(kind, "record_added" | "record_updated" | "record_deleted") => {
            Some("memory_id")
        }
        "hook" if kind != "command_output" => Some("hook_id"),
        _ => None,
    };
    if let Some(key) = identifier {
        let id = body[key].as_str().ok_or(ProtocolError::MalformedFrame)?;
        validate_id(id)?;
        projected[key] = json!(id);
    }
    match (family, kind) {
        ("extension", "readiness_updated") => {
            for key in [
                "plugin_count",
                "hook_count",
                "skill_count",
                "command_count",
                "agent_count",
                "mcp_server_count",
            ] {
                let count = body["snapshot"][key]
                    .as_u64()
                    .ok_or(ProtocolError::MalformedFrame)?;
                projected[key] = json!(count);
            }
        }
        ("skill", "catalogue") => {
            projected["skill_count"] = json!(
                body["skills"]
                    .as_array()
                    .ok_or(ProtocolError::MalformedFrame)?
                    .len()
            );
        }
        ("memory", "records_queried") => {
            projected["record_count"] = json!(
                body["records"]
                    .as_array()
                    .ok_or(ProtocolError::MalformedFrame)?
                    .len()
            );
        }
        ("memory", "metadata_queried") => {
            projected["record_count"] = json!(
                body["record_count"]
                    .as_u64()
                    .ok_or(ProtocolError::MalformedFrame)?
            );
        }
        ("memory", "labels_listed") => {
            projected["label_count"] = json!(
                body["labels"]
                    .as_array()
                    .ok_or(ProtocolError::MalformedFrame)?
                    .len()
            );
        }
        ("mcp", "server_reconnecting") => {
            for key in ["attempt", "backoff_ms"] {
                projected[key] = json!(body[key].as_u64().ok_or(ProtocolError::MalformedFrame)?);
            }
        }
        ("hook", "command_output") => {
            for key in ["timed_out", "ok"] {
                projected[key] = json!(body[key].as_bool().ok_or(ProtocolError::MalformedFrame)?);
            }
            if !body["exit_code"].is_null() {
                projected["exit_code"] = json!(
                    body["exit_code"]
                        .as_i64()
                        .ok_or(ProtocolError::MalformedFrame)?
                );
            }
        }
        _ => {}
    }
    Ok(projected)
}
