use std::collections::HashSet;

use serde_json::Value;

pub(crate) fn extract_task_message_mention_actor_ids(
    payload: &Value,
    member_ids: &HashSet<String>,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for key in ["mention_actor_ids", "mentioned_actor_ids"] {
        if let Some(explicit_mentions) = payload.get(key).and_then(Value::as_array) {
            for value in explicit_mentions {
                if let Some(candidate) = value.as_str() {
                    push_member_mention(candidate, member_ids, &mut seen, &mut out);
                }
            }
        }
    }
    if let Some(text) = payload.get("text").and_then(Value::as_str) {
        for candidate in extract_mentions_from_text(text) {
            push_member_mention(candidate.as_str(), member_ids, &mut seen, &mut out);
        }
    }
    out
}

pub(crate) fn push_member_mention(
    raw_candidate: &str,
    member_ids: &HashSet<String>,
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
) {
    let candidate = raw_candidate.trim();
    if candidate.is_empty()
        || !member_ids.contains(candidate)
        || !seen.insert(candidate.to_string())
    {
        return;
    }
    out.push(candidate.to_string());
}

pub(crate) fn extract_mentions_from_text(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut cursor = 0usize;
    while let Some(open_index) = text[cursor..].find("<at>") {
        let mention_start = cursor + open_index + 4;
        let Some(close_index) = text[mention_start..].find("</at>") else {
            break;
        };
        let mention_end = mention_start + close_index;
        let candidate = text[mention_start..mention_end].trim();
        if !candidate.is_empty()
            && candidate
                .as_bytes()
                .iter()
                .all(|raw| is_valid_mention_char(*raw))
        {
            out.push(candidate.to_string());
        }
        cursor = mention_end + 5;
    }
    let mut raw_cursor = 0usize;
    while raw_cursor < bytes.len() {
        if bytes[raw_cursor] != b'@'
            || (raw_cursor > 0 && is_email_local_char(bytes[raw_cursor - 1]))
        {
            raw_cursor += 1;
            continue;
        }
        let start = raw_cursor + 1;
        let mut end = start;
        while end < bytes.len() && is_valid_mention_char(bytes[end]) {
            end += 1;
        }
        if end > start {
            out.push(text[start..end].to_string());
        }
        raw_cursor = end;
    }
    out
}

fn is_valid_mention_char(raw: u8) -> bool {
    matches!(raw, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b':' | b'-')
}

fn is_email_local_char(raw: u8) -> bool {
    matches!(
        raw,
        b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'%' | b'+' | b'-'
    )
}
