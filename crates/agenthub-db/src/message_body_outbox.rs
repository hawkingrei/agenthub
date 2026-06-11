//! SQLite-backed durable outbox for message bodies being moved into the tiered body store.
//!
//! A body is staged into `message_body_outbox` inside the authority transaction (so it is committed
//! atomically with the authority metadata) and is removed only after the body store durably
//! acknowledges it. A body store write failure or a crash between commit and ack therefore never loses
//! a body: it survives in the outbox until a later drain succeeds. This is the persistent counterpart
//! of `agenthub_message_store::BodyOutbox`. See `docs/features/message-storage-tiering.md`.

use agenthub_message_store::{AuthorityMessageId, MessageBodyStore};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

/// Stage a body for `id` inside an authority transaction. Idempotent per logical message: re-staging
/// the same `authority_message_id` replaces the row rather than creating a second copy.
pub async fn stage_body(
    tx: &mut Transaction<'_, Sqlite>,
    id: &AuthorityMessageId,
    body: &[u8],
    staged_at: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO message_body_outbox (authority_message_id, body, staged_at) \
         VALUES (?, ?, ?)",
    )
    .bind(id.as_str())
    .bind(body)
    .bind(staged_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Number of bodies awaiting confirmation in the body store.
pub async fn pending_count(pool: &SqlitePool) -> anyhow::Result<i64> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message_body_outbox")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

/// Drain up to `batch` staged bodies into `store`, oldest first.
///
/// Each body is written to the store and its outbox row is deleted only after the store write
/// succeeds, so a failed write leaves the row for a later retry and no body is lost. The store write
/// is idempotent per logical message, so a crash between the store write and the row delete is safe:
/// the next drain simply re-writes and deletes. Returns the number of bodies confirmed in this drain.
pub async fn drain_into<S: MessageBodyStore + ?Sized>(
    pool: &SqlitePool,
    store: &S,
    batch: usize,
) -> anyhow::Result<usize> {
    let rows = sqlx::query(
        "SELECT authority_message_id, body FROM message_body_outbox ORDER BY staged_at, \
         authority_message_id LIMIT ?",
    )
    .bind(batch as i64)
    .fetch_all(pool)
    .await?;

    // Do the (potentially slow) store writes without holding a SQLite transaction, collecting the
    // ids that were durably accepted. A failed write leaves its row for a later retry.
    let mut confirmed_ids = Vec::new();
    for row in rows {
        let id: String = row.get("authority_message_id");
        let body: Vec<u8> = row.get("body");
        if store
            .put_body(&AuthorityMessageId::new(id.clone()), &body)
            .is_ok()
        {
            confirmed_ids.push(id);
        }
    }

    // Clear all confirmed rows in one transaction, so the batch costs a single fsync rather than one
    // fsync per row.
    if !confirmed_ids.is_empty() {
        let mut tx = pool.begin().await?;
        for id in &confirmed_ids {
            sqlx::query("DELETE FROM message_body_outbox WHERE authority_message_id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
    }
    Ok(confirmed_ids.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agenthub_message_store::{BodyStoreError, InMemoryBodyStore};
    use uuid::Uuid;

    /// Body store that fails the first `fail_next` writes, to exercise the retry path.
    struct FlakyStore {
        inner: InMemoryBodyStore,
        fail_next: std::sync::atomic::AtomicUsize,
    }

    impl MessageBodyStore for FlakyStore {
        fn put_body(&self, id: &AuthorityMessageId, body: &[u8]) -> Result<(), BodyStoreError> {
            use std::sync::atomic::Ordering;
            // Atomically check-and-decrement so concurrent callers can't underflow the counter.
            if self
                .fail_next
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
                    (v > 0).then(|| v - 1)
                })
                .is_ok()
            {
                return Err(BodyStoreError::Backend("injected".into()));
            }
            self.inner.put_body(id, body)
        }
        fn get_body(&self, id: &AuthorityMessageId) -> Result<Option<Vec<u8>>, BodyStoreError> {
            self.inner.get_body(id)
        }
        fn contains(&self, id: &AuthorityMessageId) -> bool {
            self.inner.contains(id)
        }
        fn len(&self) -> usize {
            self.inner.len()
        }
    }

    async fn test_pool() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!("agenthub-outbox-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        crate::init_db_at_path(&dir.join("agenthub.db"))
            .await
            .expect("init db")
    }

    #[tokio::test]
    async fn stage_then_drain_moves_body_to_store_and_clears_outbox() {
        let pool = test_pool().await;
        let id = AuthorityMessageId::new("auth-1");

        let mut tx = pool.begin().await.unwrap();
        stage_body(&mut tx, &id, b"hello body", 100).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(pending_count(&pool).await.unwrap(), 1);

        let store = InMemoryBodyStore::new();
        let confirmed = drain_into(&pool, &store, 64).await.unwrap();
        assert_eq!(confirmed, 1);
        assert_eq!(pending_count(&pool).await.unwrap(), 0);
        assert_eq!(
            store.get_body(&id).unwrap().as_deref(),
            Some(&b"hello body"[..])
        );
    }

    #[tokio::test]
    async fn restaging_same_message_keeps_one_row() {
        let pool = test_pool().await;
        let id = AuthorityMessageId::new("auth-dup");

        let mut tx = pool.begin().await.unwrap();
        stage_body(&mut tx, &id, b"v1", 100).await.unwrap();
        stage_body(&mut tx, &id, b"v1", 101).await.unwrap();
        tx.commit().await.unwrap();

        assert_eq!(pending_count(&pool).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn body_survives_store_failure_and_is_recovered_by_replay() {
        let pool = test_pool().await;
        let id = AuthorityMessageId::new("auth-replay");

        let mut tx = pool.begin().await.unwrap();
        stage_body(&mut tx, &id, b"must not be lost", 100)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let store = FlakyStore {
            inner: InMemoryBodyStore::new(),
            fail_next: std::sync::atomic::AtomicUsize::new(1),
        };
        // First drain hits the injected failure: nothing confirmed, body stays staged.
        assert_eq!(drain_into(&pool, &store, 64).await.unwrap(), 0);
        assert_eq!(pending_count(&pool).await.unwrap(), 1);

        // Retry (e.g. background drainer after a crash) succeeds and clears the outbox.
        assert_eq!(drain_into(&pool, &store, 64).await.unwrap(), 1);
        assert_eq!(pending_count(&pool).await.unwrap(), 0);
        assert_eq!(
            store.get_body(&id).unwrap().as_deref(),
            Some(&b"must not be lost"[..])
        );
    }
}
