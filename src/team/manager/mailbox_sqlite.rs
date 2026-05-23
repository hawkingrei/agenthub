use serde_json::Value;

pub(super) fn normalize_optional_sqlite_string(raw: Option<String>) -> Option<String> {
    raw.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "null")
}

pub(super) fn parse_optional_sqlite_json_value(
    raw: Option<String>,
) -> Result<Option<Value>, sqlx::Error> {
    let Some(raw) = normalize_optional_sqlite_string(raw) else {
        return Ok(None);
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(value) => Ok(Some(value)),
        Err(_) => Ok(Some(Value::String(raw))),
    }
}
