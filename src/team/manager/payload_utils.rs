use serde_json::Value;

use super::{TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND, TeamRunRecord, TeamTaskContextPatch};

pub(super) fn redact_sensitive_json(value: &Value) -> Value {
    const REDACTED: &str = "[redacted]";
    match value {
        Value::Object(map) => {
            let mut redacted = serde_json::Map::with_capacity(map.len());
            for (key, child) in map {
                if is_sensitive_key(key) {
                    redacted.insert(key.clone(), Value::String(REDACTED.to_string()));
                } else {
                    redacted.insert(key.clone(), redact_sensitive_json(child));
                }
            }
            Value::Object(redacted)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_sensitive_json).collect()),
        _ => value.clone(),
    }
}

pub(super) fn resolve_task_context_patch(
    current: &Value,
    patch: TeamTaskContextPatch,
) -> Option<Value> {
    let next = match patch {
        TeamTaskContextPatch::Replace(value) => redact_sensitive_json(&value),
        TeamTaskContextPatch::Merge(value) => {
            let mut merged = current.clone();
            merge_json_value(&mut merged, &value);
            redact_sensitive_json(&merged)
        }
    };
    (next != *current).then_some(next)
}

#[allow(dead_code)]
pub(super) fn filter_visible_team_runs(runs: Vec<TeamRunRecord>) -> Vec<TeamRunRecord> {
    runs.into_iter()
        .filter(|run| !is_shared_thread_mailbox_run_input(&run.input))
        .collect()
}

pub(super) fn merge_json_value(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(target_obj), Value::Object(patch_obj)) => {
            for (key, patch_value) in patch_obj {
                match target_obj.get_mut(key) {
                    Some(target_value) => merge_json_value(target_value, patch_value),
                    None => {
                        target_obj.insert(key.clone(), patch_value.clone());
                    }
                }
            }
        }
        (target, patch) => *target = patch.clone(),
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    normalized.contains("token")
        || normalized.contains("secret")
        || normalized.contains("password")
        || normalized.contains("authorization")
        || normalized.contains("api_key")
        || normalized.contains("apikey")
}

#[allow(dead_code)]
fn is_shared_thread_mailbox_run_input(input: &Value) -> bool {
    input
        .as_object()
        .and_then(|obj| obj.get("bootstrap_kind"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| value == TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND)
}
