use std::collections::HashMap;

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AggregateKey {
    agent_id: String,
    session_id: String,
    message_kind: String,
    message_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpenAggregate {
    chunks: Vec<ParsedChunkEvent>,
    agent_id: String,
    session_id: String,
    message_kind: String,
    message_id: String,
    first_event_id: i64,
    last_event_id: i64,
    created_at: i64,
}

pub fn aggregate_acp_chunk_rows(rows: &[AcpEventRow]) -> Vec<AcpAggregatedMessage> {
    let mut sorted = rows.to_vec();
    sorted.sort_by_key(|row| row.event_id);

    let mut open = HashMap::<AggregateKey, OpenAggregate>::new();

    for row in sorted {
        let Some(parsed) = parse_chunk_event(&row.message) else {
            continue;
        };

        let key = AggregateKey {
            agent_id: row.agent_id.clone(),
            session_id: row.session_id.clone(),
            message_kind: parsed.message_kind.clone(),
            message_id: parsed.message_id.clone(),
        };

        match open.get_mut(&key) {
            Some(active) => {
                active.first_event_id = active.first_event_id.min(row.event_id);
                active.last_event_id = active.last_event_id.max(row.event_id);
                active.created_at = active.created_at.min(row.ts);
                active.chunks.push(parsed);
            }
            None => {
                open.insert(
                    key,
                    OpenAggregate {
                        agent_id: row.agent_id,
                        session_id: row.session_id,
                        message_kind: parsed.message_kind.clone(),
                        message_id: parsed.message_id.clone(),
                        chunks: vec![parsed],
                        first_event_id: row.event_id,
                        last_event_id: row.event_id,
                        created_at: row.ts,
                    },
                );
            }
        }
    }

    let mut out: Vec<_> = open
        .into_values()
        .map(|mut aggregate| {
            aggregate.chunks.sort_by_key(|chunk| chunk.chunk_index);
            let text = aggregate
                .chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<String>();
            AcpAggregatedMessage {
                logical_message_id: aggregate.message_id,
                message_kind: aggregate.message_kind,
                agent_id: aggregate.agent_id,
                session_id: aggregate.session_id,
                text,
                created_at: aggregate.created_at,
                first_event_id: aggregate.first_event_id,
                last_event_id: aggregate.last_event_id,
                chunk_count: aggregate.chunks.len().try_into().unwrap_or(u32::MAX),
            }
        })
        .collect();
    out.sort_by_key(|message| message.first_event_id);
    out
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
    fn keeps_non_consecutive_chunks_in_one_logical_message() {
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
        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].logical_message_id, "m1");
        assert_eq!(aggregated[0].text, "hello");
        assert_eq!(aggregated[0].first_event_id, 21);
        assert_eq!(aggregated[0].last_event_id, 22);
        assert_eq!(aggregated[0].chunk_count, 2);
    }

    #[test]
    fn orders_chunks_by_chunk_index_before_event_id() {
        let rows = vec![
            AcpEventRow {
                event_id: 51,
                agent_id: "agent-a".to_string(),
                session_id: "session-a".to_string(),
                ts: 500,
                message: r#"{"type":"agent_message","text":"lo","chunk":true,"message_id":"m1","chunk_index":1}"#.to_string(),
            },
            AcpEventRow {
                event_id: 52,
                agent_id: "agent-a".to_string(),
                session_id: "session-a".to_string(),
                ts: 501,
                message: r#"{"type":"agent_message","text":"hel","chunk":true,"message_id":"m1","chunk_index":0}"#.to_string(),
            },
        ];

        let aggregated = aggregate_acp_chunk_rows(&rows);
        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].text, "hello");
        assert_eq!(aggregated[0].created_at, 500);
        assert_eq!(aggregated[0].first_event_id, 51);
        assert_eq!(aggregated[0].last_event_id, 52);
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

    #[test]
    fn aggregates_interleaved_messages_by_message_id() {
        let rows = vec![
            AcpEventRow {
                event_id: 41,
                agent_id: "agent-a".to_string(),
                session_id: "session-a".to_string(),
                ts: 400,
                message: r#"{"type":"agent_message","text":"he","chunk":true,"message_id":"m1","chunk_index":0}"#.to_string(),
            },
            AcpEventRow {
                event_id: 42,
                agent_id: "agent-a".to_string(),
                session_id: "session-a".to_string(),
                ts: 401,
                message: r#"{"type":"agent_message","text":"wo","chunk":true,"message_id":"m2","chunk_index":0}"#.to_string(),
            },
            AcpEventRow {
                event_id: 43,
                agent_id: "agent-a".to_string(),
                session_id: "session-a".to_string(),
                ts: 402,
                message: r#"{"type":"agent_message","text":"llo","chunk":true,"message_id":"m1","chunk_index":1}"#.to_string(),
            },
            AcpEventRow {
                event_id: 44,
                agent_id: "agent-a".to_string(),
                session_id: "session-a".to_string(),
                ts: 403,
                message: r#"{"type":"agent_message","text":"rld","chunk":true,"message_id":"m2","chunk_index":1}"#.to_string(),
            },
        ];

        let aggregated = aggregate_acp_chunk_rows(&rows);
        assert_eq!(aggregated.len(), 2);
        assert_eq!(aggregated[0].logical_message_id, "m1");
        assert_eq!(aggregated[0].text, "hello");
        assert_eq!(aggregated[1].logical_message_id, "m2");
        assert_eq!(aggregated[1].text, "world");
    }
}
