use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpEventRow {
    pub event_id: i64,
    pub agent_id: String,
    pub session_id: String,
    pub ts: i64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpAggregatedMessage {
    pub logical_message_id: String,
    pub message_kind: String,
    pub agent_id: String,
    pub session_id: String,
    pub text: String,
    pub created_at: i64,
    pub first_event_id: i64,
    pub last_event_id: i64,
    pub chunk_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedChunkEvent {
    message_kind: String,
    message_id: String,
    chunk_index: u64,
    text: String,
}

pub fn aggregate_acp_chunk_rows(rows: &[AcpEventRow]) -> Vec<AcpAggregatedMessage> {
    let mut sorted = rows.to_vec();
    sorted.sort_by_key(|row| row.event_id);

    let mut out = Vec::new();
    let mut current: Option<AcpAggregatedMessage> = None;
    let mut expected_chunk_index: Option<u64> = None;

    for row in sorted {
        let Some(parsed) = parse_chunk_event(&row.message) else {
            finish_current(&mut current, &mut out);
            expected_chunk_index = None;
            continue;
        };

        let should_extend = current.as_ref().is_some_and(|active| {
            active.agent_id == row.agent_id
                && active.session_id == row.session_id
                && active.message_kind == parsed.message_kind
                && active.logical_message_id == parsed.message_id
                && expected_chunk_index.is_none_or(|expected| expected == parsed.chunk_index)
        });

        if should_extend {
            let active = current.as_mut().expect("current aggregate exists");
            active.text.push_str(&parsed.text);
            active.last_event_id = row.event_id;
            active.chunk_count = active.chunk_count.saturating_add(1);
            expected_chunk_index = parsed.chunk_index.checked_add(1);
            continue;
        }

        finish_current(&mut current, &mut out);
        expected_chunk_index = parsed.chunk_index.checked_add(1);
        current = Some(AcpAggregatedMessage {
            logical_message_id: parsed.message_id,
            message_kind: parsed.message_kind,
            agent_id: row.agent_id,
            session_id: row.session_id,
            text: parsed.text,
            created_at: row.ts,
            first_event_id: row.event_id,
            last_event_id: row.event_id,
            chunk_count: 1,
        });
    }

    finish_current(&mut current, &mut out);
    out
}

fn finish_current(current: &mut Option<AcpAggregatedMessage>, out: &mut Vec<AcpAggregatedMessage>) {
    if let Some(active) = current.take() {
        out.push(active);
    }
}

fn parse_chunk_event(raw: &str) -> Option<ParsedChunkEvent> {
    let value: Value = serde_json::from_str(raw).ok()?;
    let obj = value.as_object()?;
    let kind = obj.get("type")?.as_str()?;
    if !matches!(kind, "user_message" | "agent_message" | "agent_thought") {
        return None;
    }
    if obj.get("chunk").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let message_id = obj.get("message_id")?.as_str()?.trim();
    if message_id.is_empty() {
        return None;
    }
    let text = obj.get("text")?.as_str()?.to_string();
    let chunk_index = parse_chunk_index(obj.get("chunk_index"))?;
    Some(ParsedChunkEvent {
        message_kind: kind.to_string(),
        message_id: message_id.to_string(),
        chunk_index,
        text,
    })
}

fn parse_chunk_index(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::Number(number) => number.as_u64(),
        Value::String(raw) => raw.parse::<u64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{AcpEventRow, aggregate_acp_chunk_rows};

    #[test]
    fn aggregates_consecutive_agent_message_chunks() {
        let rows = vec![
            AcpEventRow {
                event_id: 11,
                agent_id: "agent-a".to_string(),
                session_id: "session-a".to_string(),
                ts: 100,
                message: r#"{"type":"agent_message","text":"hel","chunk":true,"message_id":"m1","chunk_index":0}"#.to_string(),
            },
            AcpEventRow {
                event_id: 12,
                agent_id: "agent-a".to_string(),
                session_id: "session-a".to_string(),
                ts: 101,
                message: r#"{"type":"agent_message","text":"lo","chunk":true,"message_id":"m1","chunk_index":1}"#.to_string(),
            },
        ];

        let aggregated = aggregate_acp_chunk_rows(&rows);
        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].logical_message_id, "m1");
        assert_eq!(aggregated[0].text, "hello");
        assert_eq!(aggregated[0].first_event_id, 11);
        assert_eq!(aggregated[0].last_event_id, 12);
        assert_eq!(aggregated[0].chunk_count, 2);
    }

    #[test]
    fn splits_on_non_consecutive_chunk_index() {
        let rows = vec![
            AcpEventRow {
                event_id: 21,
                agent_id: "agent-a".to_string(),
                session_id: "session-a".to_string(),
                ts: 200,
                message: r#"{"type":"agent_message","text":"hel","chunk":true,"message_id":"m1","chunk_index":0}"#.to_string(),
            },
            AcpEventRow {
                event_id: 22,
                agent_id: "agent-a".to_string(),
                session_id: "session-a".to_string(),
                ts: 201,
                message: r#"{"type":"agent_message","text":"lo","chunk":true,"message_id":"m1","chunk_index":3}"#.to_string(),
            },
        ];

        let aggregated = aggregate_acp_chunk_rows(&rows);
        assert_eq!(aggregated.len(), 2);
        assert_eq!(aggregated[0].text, "hel");
        assert_eq!(aggregated[1].text, "lo");
    }

    #[test]
    fn ignores_non_chunk_events() {
        let rows = vec![AcpEventRow {
            event_id: 31,
            agent_id: "agent-a".to_string(),
            session_id: "session-a".to_string(),
            ts: 300,
            message: r#"{"type":"tool_call","id":"tool-1"}"#.to_string(),
        }];

        let aggregated = aggregate_acp_chunk_rows(&rows);
        assert!(aggregated.is_empty());
    }
}
