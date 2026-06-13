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
use sqlx::{Sqlite, SqlitePool};

use super::TeamManager;
use crate::team::TeamConversationMessageRecord;

/// Sentinel stored in `payload_json` when a conversation body has been moved into the body store.
///
/// It is intentionally not valid JSON, so it can never collide with a real serialized payload (which
/// is always valid JSON). Moved-ness is therefore decided by an exact string comparison against this
/// constant, not by inspecting the parsed payload value, which keeps user-supplied payloads from ever
/// being mistaken for a moved body. It is kept ASCII (no control characters) so it stays legible in
/// SQLite browsers / CLIs, which can silently truncate strings containing a NUL byte.
pub(super) const CONVERSATION_BODY_MOVED_SENTINEL: &str = "::agenthub:tcm-body-moved::";

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

/// Outcome of [`migrate_conversation_bodies_into_store`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConversationBodyMigrationReport {
    /// Rows whose inline body was moved out of SQLite and staged into the outbox.
    pub(crate) migrated: u64,
    /// Bodies durably confirmed in the body store (drained from the outbox).
    pub(crate) drained: u64,
}

/// Number of conversation rows whose body is still stored inline (i.e. not yet moved into the store).
pub(crate) async fn count_inline_conversation_bodies(pool: &SqlitePool) -> anyhow::Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM team_conversation_messages WHERE payload_json != ?1",
    )
    .bind(CONVERSATION_BODY_MOVED_SENTINEL)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Backfill existing inline conversation bodies into the body store.
///
/// Each batch moves rows out of SQLite (staging the body into the durable outbox and replacing the
/// row's `payload_json` with the moved sentinel, atomically per transaction) and then drains that
/// batch into the body store. A final drain pass clears any bodies left staged by an earlier
/// interrupted run. The pass is resumable and idempotent: already-moved rows are skipped, so re-running
/// after an interruption only processes what remains.
pub(crate) async fn migrate_conversation_bodies_into_store(
    pool: &SqlitePool,
    store: &dyn MessageBodyStore,
    batch_size: usize,
    staged_at: i64,
) -> anyhow::Result<ConversationBodyMigrationReport> {
    use sqlx::Row as _;

    let batch_size = batch_size.max(1) as i64;
    // Only touch rows that already exist; rows written concurrently are handled by the write path.
    let max_id: Option<i64> = sqlx::query_scalar("SELECT MAX(id) FROM team_conversation_messages")
        .fetch_one(pool)
        .await?;
    let Some(max_id) = max_id else {
        return Ok(ConversationBodyMigrationReport::default());
    };

    let mut report = ConversationBodyMigrationReport::default();
    let mut cursor = 0_i64;
    loop {
        let rows = sqlx::query(
            "SELECT id, payload_json FROM team_conversation_messages \
             WHERE id > ?1 AND id <= ?2 AND payload_json != ?3 \
             ORDER BY id ASC LIMIT ?4",
        )
        .bind(cursor)
        .bind(max_id)
        .bind(CONVERSATION_BODY_MOVED_SENTINEL)
        .bind(batch_size)
        .fetch_all(pool)
        .await?;
        if rows.is_empty() {
            break;
        }

        let mut tx = pool.begin().await?;
        for row in &rows {
            let id: i64 = row.get("id");
            let payload_json: String = row.get("payload_json");
            agenthub_db::message_body_outbox::stage_body(
                &mut tx,
                &conversation_body_key(id),
                payload_json.as_bytes(),
                staged_at,
            )
            .await?;
            sqlx::query("UPDATE team_conversation_messages SET payload_json = ?1 WHERE id = ?2")
                .bind(CONVERSATION_BODY_MOVED_SENTINEL)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            cursor = id;
            report.migrated += 1;
        }
        tx.commit().await?;

        // Drain this batch so the outbox (which holds a duplicate of each body) stays bounded.
        report.drained +=
            agenthub_db::message_body_outbox::drain_into(pool, store, rows.len()).await? as u64;
    }

    // Clear anything left staged by an earlier interrupted run.
    loop {
        let drained =
            agenthub_db::message_body_outbox::drain_into(pool, store, batch_size as usize).await?;
        report.drained += drained as u64;
        if (drained as i64) < batch_size {
            break;
        }
    }

    Ok(report)
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
            "{\"text\":\"::agenthub:tcm-body-moved::\"}"
        ));
    }

    #[test]
    fn body_key_is_namespaced_by_row_id() {
        assert_eq!(conversation_body_key(42).as_str(), "tcm:42");
    }
}
