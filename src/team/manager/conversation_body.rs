//! Tiered-body support for `team_conversation_messages`.
//!
//! When the message body store is active, a conversation message's `payload_json` is moved out of the
//! SQLite row into the body store (keyed by the row's authority id) so the larger chat body lives in
//! the compressed RocksDB store rather than inline in SQLite. The SQLite row keeps the queried
//! metadata columns (see `init_db`) and stores [`CONVERSATION_BODY_MOVED_SENTINEL`] in place of the
//! body. The read path rehydrates the real payload from the body store, falling back to the durable
//! outbox for bodies that have been staged but not yet drained.
//!
//! Rows written without an active store (older rows, or platforms built without the `rocksdb` feature)
//! keep their full inline `payload_json` and are never treated as moved, so both shapes coexist.

use agenthub_message_store::{AuthorityMessageId, MessageBodyStore};
use sqlx::Sqlite;

use super::TeamManager;
use crate::team::TeamConversationMessageRecord;

/// Sentinel stored in `payload_json` when a conversation body has been moved into the body store.
///
/// It is intentionally not valid JSON, so it can never collide with a real serialized payload (which
/// is always valid JSON). Moved-ness is therefore decided by an exact string comparison against this
/// constant, not by inspecting the parsed payload value, which keeps user-supplied payloads from ever
/// being mistaken for a moved body.
pub(super) const CONVERSATION_BODY_MOVED_SENTINEL: &str = "\u{0}agenthub:tcm-body-moved\u{1}";

/// Body-store key for a conversation message's moved body. One copy per logical message (row id).
pub(super) fn conversation_body_key(message_id: i64) -> AuthorityMessageId {
    AuthorityMessageId::new(format!("tcm:{message_id}"))
}

/// Whether a stored `payload_json` indicates the body was moved into the body store.
pub(super) fn conversation_payload_was_moved(payload_json: &str) -> bool {
    payload_json == CONVERSATION_BODY_MOVED_SENTINEL
}

/// Load a moved conversation body, preferring the drained body store and falling back to the durable
/// outbox (for bodies staged but not yet drained). Returns `None` only if the body is in neither.
async fn load_moved_conversation_body<'a, E>(
    body_store: Option<&dyn MessageBodyStore>,
    outbox_executor: E,
    key: &AuthorityMessageId,
) -> anyhow::Result<Option<Vec<u8>>>
where
    E: sqlx::Executor<'a, Database = Sqlite>,
{
    if let Some(store) = body_store
        && let Some(body) = store
            .get_body(key)
            .map_err(|err| anyhow::anyhow!(err.to_string()))?
    {
        return Ok(Some(body));
    }
    let body: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT body FROM message_body_outbox WHERE authority_message_id = ?1")
            .bind(key.as_str())
            .fetch_optional(outbox_executor)
            .await?;
    Ok(body)
}

/// Rehydrate a moved conversation body in place, using `tx` for the outbox fallback. For the insert
/// idempotency-conflict path, where the existing row may have been staged within an in-flight write.
pub(super) async fn rehydrate_conversation_body_in_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    body_store: Option<&dyn MessageBodyStore>,
    record: &mut TeamConversationMessageRecord,
) -> anyhow::Result<()> {
    let key = conversation_body_key(record.message_id);
    let body = load_moved_conversation_body(body_store, &mut **tx, &key)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "conversation message {} is marked moved but its body is missing from the store and outbox",
                record.message_id
            )
        })?;
    record.payload = serde_json::from_slice(&body)?;
    Ok(())
}

impl TeamManager {
    /// Rehydrate a moved conversation body in place from the body store (falling back to the outbox).
    /// Callers must only invoke this for rows whose `payload_json` was the moved sentinel.
    pub(super) async fn rehydrate_moved_conversation_payload(
        &self,
        record: &mut TeamConversationMessageRecord,
    ) -> anyhow::Result<()> {
        let key = conversation_body_key(record.message_id);
        let body = load_moved_conversation_body(self.body_store.as_deref(), &self.db, &key)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "conversation message {} is marked moved but its body is missing from the store and outbox",
                    record.message_id
                )
            })?;
        record.payload = serde_json::from_slice(&body)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CONVERSATION_BODY_MOVED_SENTINEL, conversation_body_key, conversation_payload_was_moved,
    };

    #[test]
    fn sentinel_is_not_valid_json() {
        // The moved sentinel must never collide with a real serialized payload (always valid JSON).
        assert!(
            serde_json::from_str::<serde_json::Value>(CONVERSATION_BODY_MOVED_SENTINEL).is_err()
        );
    }

    #[test]
    fn moved_detection_matches_only_sentinel() {
        assert!(conversation_payload_was_moved(
            CONVERSATION_BODY_MOVED_SENTINEL
        ));
        assert!(!conversation_payload_was_moved("{\"text\":\"hi\"}"));
        // A payload that merely mentions the sentinel as data is not treated as moved.
        assert!(!conversation_payload_was_moved(
            "{\"text\":\"\\u0000agenthub:tcm-body-moved\\u0001\"}"
        ));
    }

    #[test]
    fn body_key_is_namespaced_by_row_id() {
        assert_eq!(conversation_body_key(42).as_str(), "tcm:42");
    }
}
