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
///
/// The lookup order is store → outbox → store. The second store check is what makes this race-free
/// against a concurrent drainer: the drainer writes the body into the store and only *then* deletes its
/// outbox row, so if the body is missing from the store on the first check and also missing from the
/// outbox, the drainer must have moved it between the two checks — and an absent outbox row means the
/// store write already completed. Without the re-check, a read interleaved with a drain could see the
/// body in neither place and spuriously fail.
async fn load_moved_conversation_body<'a, E>(
    body_store: Option<&dyn MessageBodyStore>,
    outbox_executor: E,
    key: &AuthorityMessageId,
) -> anyhow::Result<Option<Vec<u8>>>
where
    E: sqlx::Executor<'a, Database = Sqlite>,
{
    let get_from_store = |store: &dyn MessageBodyStore| {
        store
            .get_body(key)
            .map_err(|err| anyhow::anyhow!(err.to_string()))
    };

    // Steady state: the body has been drained into the store. Check it first so a read does not pay for
    // an outbox query once the body has settled.
    if let Some(store) = body_store
        && let Some(body) = get_from_store(store)?
    {
        return Ok(Some(body));
    }

    // Not (yet) in the store: it should still be staged in the durable outbox.
    if let Some(body) = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT body FROM message_body_outbox WHERE authority_message_id = ?1",
    )
    .bind(key.as_str())
    .fetch_optional(outbox_executor)
    .await?
    {
        return Ok(Some(body));
    }

    // Missing from both the first store check and the outbox: a concurrent drainer may have moved the
    // body into the store after that first check and before the outbox check. Re-check the store to
    // close that window (the drainer writes the store before deleting the outbox row).
    if let Some(store) = body_store
        && let Some(body) = get_from_store(store)?
    {
        return Ok(Some(body));
    }

    Ok(None)
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
///
/// `inter_batch_delay` paces the loop by sleeping between batches. SQLite allows only one writer, so a
/// tight loop of write transactions can starve concurrent writers; a non-zero delay yields the write
/// lock so online traffic makes progress. Pass `Duration::ZERO` for the foreground `agenthub migrate`
/// command (run it flat-out), and a small delay for the background startup backfill.
pub(crate) async fn migrate_conversation_bodies_into_store(
    pool: &SqlitePool,
    store: &dyn MessageBodyStore,
    batch_size: usize,
    staged_at: i64,
    inter_batch_delay: std::time::Duration,
) -> anyhow::Result<ConversationBodyMigrationReport> {
    use sqlx::Row as _;

    let batch_size = batch_size.max(1) as i64;
    // The drain window is deliberately decoupled from the SQLite-tx batch size: `batch_size` bounds how
    // many rows we rewrite per transaction, but draining must always look at a window wide enough to
    // step over a sparse body that persistently fails to write. Otherwise, with a tiny batch size, the
    // oldest stuck body would sit at the head of every drain and block everything behind it.
    let drain_batch = batch_size.max(256) as usize;
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

        // Keep the outbox (which holds a duplicate of each body) bounded as we go.
        report.drained +=
            agenthub_db::message_body_outbox::drain_into(pool, store, drain_batch).await? as u64;

        // Yield the single SQLite writer between batches so concurrent writes are not starved.
        if !inter_batch_delay.is_zero() {
            tokio::time::sleep(inter_batch_delay).await;
        }
    }

    // Clear anything left staged (e.g. by an earlier interrupted run). Drive the loop off the outbox
    // count rather than the per-pass drained count: if a body persistently fails to write, a full pass
    // returns fewer than the window size, which must not stop the loop while other bodies are still
    // pending (head-of-line blocking). Stop when the outbox is empty, or when a whole pass makes no
    // progress at all — only genuinely stuck bodies remain, which a later retry / the drainer can pick
    // up.
    loop {
        if agenthub_db::message_body_outbox::pending_count(pool).await? == 0 {
            break;
        }
        let drained =
            agenthub_db::message_body_outbox::drain_into(pool, store, drain_batch).await?;
        report.drained += drained as u64;
        if drained == 0 {
            break;
        }
        if !inter_batch_delay.is_zero() {
            tokio::time::sleep(inter_batch_delay).await;
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
