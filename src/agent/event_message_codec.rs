use crate::agent::OutputStream;
use crate::db::{AgentEventDbRouter, AgentEventIdleGc};

// Marker for binary-compressed ACP rows: prefix + zstd(payload).
// Using a binary prefix keeps detection cheap and avoids text re-encoding.
const ACP_ZSTD_PREFIX: &[u8] = b"\x00ahzstd\x01";
const ACP_ZSTD_LEVEL: i32 = 3;
const ACP_ZSTD_MIN_MESSAGE_BYTES: usize = 256;
const ACP_ZSTD_MAX_DECOMPRESSED_BYTES: usize = 64 * 1024 * 1024;

pub(crate) fn encode_message_for_storage(stream: &OutputStream, message: &str) -> Vec<u8> {
    // Only ACP payloads are high-volume structured JSON. Keep other streams raw for simplicity.
    if !matches!(stream, OutputStream::Acp) || message.len() < ACP_ZSTD_MIN_MESSAGE_BYTES {
        return message.as_bytes().to_vec();
    }

    let compressed = match zstd::bulk::compress(message.as_bytes(), ACP_ZSTD_LEVEL) {
        Ok(payload) => payload,
        Err(err) => {
            tracing::warn!(
                error = %err,
                "failed to compress acp event message with zstd; storing raw message"
            );
            return message.as_bytes().to_vec();
        }
    };

    let candidate_len = ACP_ZSTD_PREFIX.len() + compressed.len();
    if candidate_len >= message.len() {
        return message.as_bytes().to_vec();
    }

    let mut payload = Vec::with_capacity(candidate_len);
    payload.extend_from_slice(ACP_ZSTD_PREFIX);
    payload.extend_from_slice(&compressed);
    payload
}

pub(crate) fn decode_message_from_storage(message: &[u8]) -> String {
    // SQLite can return either legacy text rows or new binary rows through the same column.
    let decoded_bytes = if let Some(compressed) = message.strip_prefix(ACP_ZSTD_PREFIX) {
        match zstd::bulk::decompress(compressed, ACP_ZSTD_MAX_DECOMPRESSED_BYTES) {
            Ok(payload) => payload,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "failed to decompress zstd acp event message; returning original payload"
                );
                message.to_vec()
            }
        }
    } else {
        message.to_vec()
    };

    match String::from_utf8(decoded_bytes) {
        Ok(decoded) => decoded,
        Err(utf8_err) => {
            tracing::warn!(
                error = %utf8_err,
                "failed to decode utf-8 agent event message; returning lossy decoded payload"
            );
            String::from_utf8_lossy(utf8_err.as_bytes()).to_string()
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn persist_agent_event(
    event_dbs: &AgentEventDbRouter,
    idle_gc: Option<&AgentEventIdleGc>,
    agent_id: &str,
    session_id: &str,
    seq: &str,
    ts: i64,
    stream: &OutputStream,
    message: &str,
) -> anyhow::Result<i64> {
    let event_db = event_dbs.pool_for_agent(agent_id).await?;
    let result = sqlx::query(
        r#"
        INSERT INTO agent_events (session_id, seq, ts, stream, message)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind(session_id)
    .bind(seq)
    .bind(ts)
    .bind(stream_to_str(stream))
    .bind(encode_message_for_storage(stream, message))
    .execute(&event_db)
    .await?;
    if let Some(idle_gc) = idle_gc {
        idle_gc.record_activity(agent_id).await;
    }
    Ok(result.last_insert_rowid())
}

fn stream_to_str(stream: &OutputStream) -> &'static str {
    match stream {
        OutputStream::Stdout => "stdout",
        OutputStream::Stderr => "stderr",
        OutputStream::System => "system",
        OutputStream::Acp => "acp",
    }
}

#[cfg(test)]
mod tests {
    use super::{ACP_ZSTD_PREFIX, decode_message_from_storage, encode_message_for_storage};
    use crate::agent::OutputStream;

    #[test]
    fn encode_and_decode_large_acp_message_roundtrip() {
        let payload = format!(
            "{{\"type\":\"agent_message\",\"text\":\"{}\"}}",
            "x".repeat(4096)
        );

        let encoded = encode_message_for_storage(&OutputStream::Acp, &payload);
        assert!(
            encoded.starts_with(ACP_ZSTD_PREFIX),
            "expected acp payload to be zstd encoded"
        );

        let decoded = decode_message_from_storage(encoded.as_slice());
        assert_eq!(decoded, payload);
    }

    #[test]
    fn encode_skips_non_acp_streams() {
        let payload = "plain output";
        let encoded = encode_message_for_storage(&OutputStream::Stdout, payload);
        assert_eq!(encoded, payload.as_bytes());
    }

    #[test]
    fn decode_returns_original_on_invalid_compressed_payload() {
        let mut corrupted = ACP_ZSTD_PREFIX.to_vec();
        corrupted.extend_from_slice(b"not-zstd");
        let decoded = decode_message_from_storage(corrupted.as_slice());
        assert_eq!(decoded, String::from_utf8_lossy(&corrupted));
    }
}
