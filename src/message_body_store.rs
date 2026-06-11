//! Wiring for the tiered message body store (see `docs/features/message-storage-tiering.md`).
//!
//! The durable SQLite outbox lives in `agenthub-db` (`message_body_outbox`). The RocksDB-backed body
//! store that staged bodies drain into is only compiled when the binary is built with the `rocksdb`
//! feature, which is enabled per release target (x86_64) so the native `librocksdb-sys` build does not
//! affect the default, Bazel, or aarch64 builds. The background drainer and the body/metadata split on
//! the write path are added in a follow-up.

pub use agenthub_message_store::AuthorityMessageId;

/// Open the RocksDB-backed message body store at `path`.
#[cfg(feature = "rocksdb")]
pub fn open_rocksdb_body_store(
    path: impl AsRef<std::path::Path>,
) -> Result<agenthub_message_store::RocksdbBodyStore, agenthub_message_store::BodyStoreError> {
    agenthub_message_store::RocksdbBodyStore::open(path)
}
