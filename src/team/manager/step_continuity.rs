use serde_json::Value;

use super::context_artifacts::ContinuitySnapshot;
use super::{
    CONTINUITY_MAX_HISTORY_CHARS, CONTINUITY_MAX_SUMMARY_CHARS, CONTINUITY_MODE_DEFAULT,
    CONTINUITY_MODE_RESET, TEAM_RUN_CONTINUITY_MODE_VALUES, redact_sensitive_json,
};
use agenthub_text::truncate_chars;

pub(super) fn normalize_run_input_continuity(mut input: Value) -> Value {
    let Some(input_obj) = input.as_object_mut() else {
        return input;
    };
    let continuity_value = input_obj
        .entry("continuity".to_string())
        .or_insert_with(|| serde_json::json!({ "mode": CONTINUITY_MODE_DEFAULT }));
    if !continuity_value.is_object() {
        *continuity_value = serde_json::json!({ "mode": CONTINUITY_MODE_DEFAULT });
        return input;
    }
    let continuity_obj = continuity_value
        .as_object_mut()
        .expect("continuity object must be object");
    let mode = continuity_obj
        .get("mode")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(CONTINUITY_MODE_DEFAULT);
    let normalized_mode = if TEAM_RUN_CONTINUITY_MODE_VALUES.contains(&mode) {
        mode
    } else {
        CONTINUITY_MODE_DEFAULT
    };
    continuity_obj.insert(
        "mode".to_string(),
        Value::String(normalized_mode.to_string()),
    );

    if let Some(raw) = continuity_obj
        .get("max_history_items")
        .and_then(Value::as_i64)
    {
        if !(1..=200).contains(&raw) {
            continuity_obj.remove("max_history_items");
        }
    } else {
        continuity_obj.remove("max_history_items");
    }

    if let Some(raw) = continuity_obj.get("max_chars").and_then(Value::as_i64) {
        if !(256..=20000).contains(&raw) {
            continuity_obj.remove("max_chars");
        }
    } else {
        continuity_obj.remove("max_chars");
    }

    input
}

pub(super) fn extract_continuity_mode_from_input(input: &Value) -> String {
    let Some(input_obj) = input.as_object() else {
        return CONTINUITY_MODE_DEFAULT.to_string();
    };
    let mode = input_obj
        .get("continuity")
        .and_then(Value::as_object)
        .and_then(|continuity| continuity.get("mode"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(CONTINUITY_MODE_DEFAULT);
    if mode == CONTINUITY_MODE_RESET {
        CONTINUITY_MODE_RESET.to_string()
    } else if TEAM_RUN_CONTINUITY_MODE_VALUES.contains(&mode) {
        mode.to_string()
    } else {
        CONTINUITY_MODE_DEFAULT.to_string()
    }
}

pub(super) fn build_continuity_snapshot(output: Option<&Value>) -> ContinuitySnapshot {
    let redacted_output = output
        .map(redact_sensitive_json)
        .unwrap_or_else(|| serde_json::json!({}));

    let summary_seed = redacted_output
        .as_object()
        .and_then(|obj| obj.get("summary"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| redacted_output.as_str().map(str::to_string))
        .unwrap_or_else(|| redacted_output.to_string());
    let summary_text = truncate_chars(summary_seed.as_str(), CONTINUITY_MAX_SUMMARY_CHARS);

    let output_excerpt_seed = redacted_output.to_string();
    let history_window = serde_json::json!({
        "schema_version": 1,
        "output_excerpt": truncate_chars(
            output_excerpt_seed.as_str(),
            CONTINUITY_MAX_HISTORY_CHARS
        ),
    });
    ContinuitySnapshot {
        summary_text,
        history_window,
        redacted_output,
        redacted_output_text: output_excerpt_seed,
    }
}

pub(super) fn should_offload_continuity_output(raw_output: &str) -> bool {
    raw_output.chars().count() > CONTINUITY_MAX_HISTORY_CHARS
}
