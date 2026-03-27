use serde_json::Value;
use sha2::{Digest, Sha256};

fn hex_encode(data: &[u8]) -> String {
    const HEX_CHARS: &[u8] = b"0123456789abcdef";
    let mut result = String::with_capacity(data.len() * 2);
    for byte in data {
        result.push(HEX_CHARS[(byte >> 4) as usize] as char);
        result.push(HEX_CHARS[(byte & 0xf) as usize] as char);
    }
    result
}

#[allow(clippy::too_many_arguments)]
pub fn actor_message_fingerprint(
    run_id: &str,
    from_actor_id: &str,
    from_peer_id: &str,
    to_actor_id: &str,
    to_peer_id: &str,
    channel: &str,
    transport: &str,
    route: Option<&Value>,
    payload: &Value,
) -> String {
    let route_canonical = route
        .map(canonical_json)
        .unwrap_or_else(|| "null".to_string());
    let payload_canonical = canonical_json(payload);

    let mut hasher = Sha256::new();
    hasher.update("actor-message-fingerprint:v1");
    push_component(&mut hasher, run_id);
    push_component(&mut hasher, from_actor_id);
    push_component(&mut hasher, from_peer_id);
    push_component(&mut hasher, to_actor_id);
    push_component(&mut hasher, to_peer_id);
    push_component(&mut hasher, channel);
    push_component(&mut hasher, transport);
    push_component(&mut hasher, &route_canonical);
    push_component(&mut hasher, &payload_canonical);
    hex_encode(&hasher.finalize())
}

#[allow(clippy::too_many_arguments)]
pub fn build_default_actor_message_idempotency_key(
    run_id: &str,
    from_actor_id: &str,
    from_peer_id: &str,
    to_actor_id: &str,
    to_peer_id: &str,
    channel: &str,
    transport: &str,
    route: Option<&Value>,
    payload: &Value,
) -> String {
    format!(
        "auto:v1:{}",
        actor_message_fingerprint(
            run_id,
            from_actor_id,
            from_peer_id,
            to_actor_id,
            to_peer_id,
            channel,
            transport,
            route,
            payload
        )
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_default_actor_channel_idempotency_key(
    run_id: &str,
    from_actor_id: &str,
    from_peer_id: &str,
    channel_id: &str,
    channel: &str,
    transport: &str,
    route: Option<&Value>,
    payload: &Value,
) -> String {
    let route_canonical = route
        .map(canonical_json)
        .unwrap_or_else(|| "null".to_string());
    let payload_canonical = canonical_json(payload);

    let mut hasher = Sha256::new();
    hasher.update("actor-channel-idempotency:v1");
    push_component(&mut hasher, run_id);
    push_component(&mut hasher, from_actor_id);
    push_component(&mut hasher, from_peer_id);
    push_component(&mut hasher, channel_id);
    push_component(&mut hasher, channel);
    push_component(&mut hasher, transport);
    push_component(&mut hasher, &route_canonical);
    push_component(&mut hasher, &payload_canonical);
    format!("auto:channel:v1:{}", hex_encode(&hasher.finalize()))
}

pub fn build_actor_channel_fanout_idempotency_key(
    channel_idempotency_key: &str,
    to_actor_id: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update("actor-channel-fanout:v1");
    push_component(&mut hasher, channel_idempotency_key);
    push_component(&mut hasher, to_actor_id);
    format!("fanout:v1:{}", hex_encode(&hasher.finalize()))
}

pub fn canonical_json(value: &Value) -> String {
    let mut out = String::new();
    write_canonical_json(value, &mut out);
    out
}

fn push_component(hasher: &mut Sha256, value: &str) {
    hasher.update(value.as_bytes());
    hasher.update([0_u8]);
}

fn write_canonical_json(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(v) => {
            if *v {
                out.push_str("true");
            } else {
                out.push_str("false");
            }
        }
        Value::Number(number) => out.push_str(&number.to_string()),
        Value::String(s) => {
            out.push_str(&serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string()))
        }
        Value::Array(items) => {
            out.push('[');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                write_canonical_json(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
            for (idx, (key, value)) in entries.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_string()));
                out.push(':');
                write_canonical_json(value, out);
            }
            out.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::message::ACTOR_MAIN_PEER_ID;

    #[test]
    fn canonical_json_sorts_object_keys_recursively() {
        let value = json!({
            "b": 2,
            "a": {
                "y": [3, 1],
                "x": true
            }
        });
        assert_eq!(
            canonical_json(&value),
            r#"{"a":{"x":true,"y":[3,1]},"b":2}"#
        );
    }

    #[test]
    fn default_idempotency_key_is_stable_for_equivalent_payloads() {
        let route_a = json!({"endpoint":"https://a.example","meta":{"b":2,"a":1}});
        let route_b = json!({"meta":{"a":1,"b":2},"endpoint":"https://a.example"});
        let payload_a = json!({"text":"hello","data":{"k2":"v2","k1":"v1"}});
        let payload_b = json!({"data":{"k1":"v1","k2":"v2"},"text":"hello"});

        let key_a = build_default_actor_message_idempotency_key(
            "run-1",
            "planner",
            ACTOR_MAIN_PEER_ID,
            "reviewer",
            ACTOR_MAIN_PEER_ID,
            "coordination",
            "local",
            Some(&route_a),
            &payload_a,
        );
        let key_b = build_default_actor_message_idempotency_key(
            "run-1",
            "planner",
            ACTOR_MAIN_PEER_ID,
            "reviewer",
            ACTOR_MAIN_PEER_ID,
            "coordination",
            "local",
            Some(&route_b),
            &payload_b,
        );
        let key_c = build_default_actor_message_idempotency_key(
            "run-1",
            "planner",
            "peer-a",
            "reviewer",
            ACTOR_MAIN_PEER_ID,
            "coordination",
            "local",
            Some(&route_b),
            &payload_b,
        );
        assert_eq!(key_a, key_b);
        assert_ne!(key_a, key_c);
        assert!(key_a.starts_with("auto:v1:"));
    }

    #[test]
    fn default_channel_idempotency_key_is_stable_for_equivalent_payloads() {
        let route_a = json!({"endpoint":"https://a.example","meta":{"b":2,"a":1}});
        let route_b = json!({"meta":{"a":1,"b":2},"endpoint":"https://a.example"});
        let payload_a = json!({"text":"hello","data":{"k2":"v2","k1":"v1"}});
        let payload_b = json!({"data":{"k1":"v1","k2":"v2"},"text":"hello"});

        let key_a = build_default_actor_channel_idempotency_key(
            "run-1",
            "planner",
            ACTOR_MAIN_PEER_ID,
            "all",
            "coordination",
            "local",
            Some(&route_a),
            &payload_a,
        );
        let key_b = build_default_actor_channel_idempotency_key(
            "run-1",
            "planner",
            ACTOR_MAIN_PEER_ID,
            "all",
            "coordination",
            "local",
            Some(&route_b),
            &payload_b,
        );
        assert_eq!(key_a, key_b);
        assert!(key_a.starts_with("auto:channel:v1:"));
    }

    #[test]
    fn channel_fanout_idempotency_key_is_stable_per_recipient() {
        let base = "auto:channel:v1:abc";
        let reviewer_a = build_actor_channel_fanout_idempotency_key(base, "reviewer-a");
        let reviewer_a_retry = build_actor_channel_fanout_idempotency_key(base, "reviewer-a");
        let reviewer_b = build_actor_channel_fanout_idempotency_key(base, "reviewer-b");

        assert_eq!(reviewer_a, reviewer_a_retry);
        assert_ne!(reviewer_a, reviewer_b);
        assert!(reviewer_a.starts_with("fanout:v1:"));
    }

    #[test]
    fn default_channel_idempotency_key_changes_when_channel_target_changes() {
        let payload = json!({"text":"hello"});
        let all_key = build_default_actor_channel_idempotency_key(
            "run-1",
            "planner",
            ACTOR_MAIN_PEER_ID,
            "all",
            "coordination",
            "local",
            None,
            &payload,
        );
        let reviewers_key = build_default_actor_channel_idempotency_key(
            "run-1",
            "planner",
            ACTOR_MAIN_PEER_ID,
            "reviewers",
            "coordination",
            "local",
            None,
            &payload,
        );

        assert_ne!(all_key, reviewers_key);
    }
}
