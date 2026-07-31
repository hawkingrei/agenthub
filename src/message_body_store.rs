//! Wiring for the tiered message body store (see `docs/features/message-storage-tiering.md`).
//!
//! The durable SQLite outbox lives in `agenthub-db` (`message_body_outbox`). The RocksDB-backed body
//! store that staged bodies drain into is only compiled when the binary is built with the `rocksdb`
//! feature, which is enabled per release target so the native `librocksdb-sys` build does not affect
//! the default or Bazel builds. Phase 1 keeps SQLite as the compatibility copy and dual-writes bodies
//! through the durable outbox.

use std::sync::Arc;

pub use agenthub_message_store::{
    AuthorityMessageId, IndexReadRepairScheduler, MessageBodyStore, MessageIndexStore,
};

/// A shared handle to the message body store, when one is active.
pub type SharedBodyStore = Arc<dyn MessageBodyStore>;
/// A shared handle to the rebuildable message index store, when one is active.
pub type SharedIndexStore = Arc<dyn MessageIndexStore>;
/// A shared handle to the read-repair scheduler, when one is active.
pub type SharedReadRepairScheduler = Arc<dyn IndexReadRepairScheduler>;

#[derive(Clone, Default)]
pub struct MessageStores {
    pub body: Option<SharedBodyStore>,
    pub index: Option<SharedIndexStore>,
    pub read_repair: Option<SharedReadRepairScheduler>,
}

/// Construct the tiered message stores from config, if enabled and compiled in.
pub fn init_message_stores(config: &agenthub_config::AppConfig) -> MessageStores {
    if !config.message_body_store_enabled() {
        tracing::info!("message body store disabled by config");
        return MessageStores::default();
    }

    #[cfg(feature = "rocksdb")]
    {
        let path = config.message_body_store_path();
        match agenthub_message_store::RocksdbBodyStore::open(&path) {
            Ok(store) => {
                let store = Arc::new(store);
                tracing::info!(path = %path, "message store (rocksdb) enabled");
                MessageStores {
                    body: Some(store.clone() as SharedBodyStore),
                    index: Some(store as SharedIndexStore),
                    read_repair: Some(Arc::new(
                        agenthub_message_store::InMemoryIndexReadRepairScheduler::new(),
                    ) as SharedReadRepairScheduler),
                }
            }
            Err(error) => {
                tracing::error!(error = %error, path = %path, "failed to open message store; running without it");
                MessageStores::default()
            }
        }
    }

    #[cfg(not(feature = "rocksdb"))]
    {
        tracing::info!(
            "message store is enabled in config but this binary was built without the `rocksdb` feature; running without it"
        );
        MessageStores::default()
    }
}

/// Construct the message body store from config, if it is enabled and compiled in.
///
/// The store is opt-in through `message_body_store.enabled = true`. It can only actually be
/// constructed when the binary is built with the `rocksdb` feature; otherwise this logs and returns
/// `None` so the binary still runs (without compression).
pub fn init_body_store(config: &agenthub_config::AppConfig) -> Option<SharedBodyStore> {
    init_message_stores(config).body
}
