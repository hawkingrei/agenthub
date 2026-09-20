use agenthub_agent_domain::mcp_operations::McpDigest;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt::Write;

use crate::{MAX_MESSAGE_BYTES, McpTransportError};

/// Sort recursively even when another workspace crate enables serde_json/preserve_order.
pub(crate) fn digest(tag: &str, value: &Value) -> Result<McpDigest, McpTransportError> {
    let mut canonical = value.clone();
    normalize_numbers(&mut canonical);
    canonical.sort_all_objects();
    let bytes = serde_json::to_vec(&canonical).map_err(|_| McpTransportError::InvalidMessage)?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(McpTransportError::MessageTooLarge);
    }
    let mut hash = Sha256::new();
    hash.update(tag.as_bytes());
    hash.update([0]);
    hash.update(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in hash.finalize() {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
        .try_into()
        .map_err(|_| McpTransportError::InvalidMessage)
}

fn normalize_numbers(value: &mut Value) {
    match value {
        Value::Object(object) => object.values_mut().for_each(normalize_numbers),
        Value::Array(array) => array.iter_mut().for_each(normalize_numbers),
        Value::Number(number) if number.is_f64() => {
            let number = number.as_f64().expect("JSON number is finite");
            // JSON integer values can be spelled as 1, 1.0 or 1e0. Those spellings must not
            // give an unresolved write a fresh semantic identity. Preserve exact u64/i64 values
            // instead of converting all integers through a lossy f64 representation.
            if number.fract() == 0.0 {
                if (0.0..18_446_744_073_709_551_616.0).contains(&number) {
                    *value = Value::from(number as u64);
                } else if (-9_223_372_036_854_775_808.0..0.0).contains(&number) {
                    *value = Value::from(number as i64);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_identity_ignores_object_order_and_equivalent_number_spellings() {
        let left: Value = serde_json::from_str(r#"{"b":[1,1000000,0],"a":{"x":2}}"#).unwrap();
        let right: Value = serde_json::from_str(r#"{"a":{"x":2.0},"b":[1e0,1e6,-0.0]}"#).unwrap();
        assert_eq!(
            digest("arguments", &left).unwrap(),
            digest("arguments", &right).unwrap()
        );
        assert_ne!(
            digest("arguments", &left).unwrap(),
            digest("schema", &left).unwrap()
        );
        let exact: Value = serde_json::from_str("9007199254740993").unwrap();
        let rounded: Value = serde_json::from_str("9007199254740992.0").unwrap();
        assert_ne!(
            digest("arguments", &exact).unwrap(),
            digest("arguments", &rounded).unwrap()
        );
    }
}
