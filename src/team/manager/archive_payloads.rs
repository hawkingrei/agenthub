use serde_json::Value;

pub(super) fn message_archive_payload_string_any(payload: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| message_archive_payload_string(payload, key))
}

pub(super) fn message_archive_payload_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn message_archive_payload_i64(payload: &Value, key: &str) -> Option<i64> {
    match payload.get(key)? {
        Value::Number(value) => value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|raw| i64::try_from(raw).ok())),
        Value::String(value) => value.trim().parse::<i64>().ok(),
        _ => None,
    }
}

pub(crate) fn message_archive_body_text(payload: &Value) -> String {
    payload
        .as_str()
        .or_else(|| payload.get("text").and_then(Value::as_str))
        .or_else(|| payload.get("summary").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_default()
}
