//! Wiring for the tiered message body store (see `docs/features/message-storage-tiering.md`).
//!
//! The durable SQLite outbox lives in `agenthub-db` (`message_body_outbox`). The RocksDB-backed body
//! store that staged bodies drain into is only compiled when the binary is built with the `rocksdb`
//! feature, which is enabled per release target so the native `librocksdb-sys` build does not affect
//! the default or Bazel builds. The write-path body/metadata split is added in a follow-up.

use std::sync::Arc;

pub use agenthub_message_store::{AuthorityMessageId, MessageBodyStore};

/// A shared handle to the message body store, when one is active.
pub type SharedBodyStore = Arc<dyn MessageBodyStore>;

/// Construct the message body store from config, if it is enabled and compiled in.
///
/// The store is a default capability: it is on unless `message_body_store.enabled = false`. It can
/// only actually be constructed when the binary is built with the `rocksdb` feature; otherwise this
/// logs and returns `None` so the binary still runs (without compression).
pub fn init_body_store(config: &agenthub_config::AppConfig) -> Option<SharedBodyStore> {
    if !config.message_body_store_enabled() {
        tracing::info!("message body store disabled by config");
        return None;
    }

    #[cfg(feature = "rocksdb")]
    {
        let path = config.message_body_store_path();
        match agenthub_message_store::RocksdbBodyStore::open(&path) {
            Ok(store) => {
                tracing::info!(path = %path, "message body store (rocksdb) enabled");
                Some(Arc::new(store) as SharedBodyStore)
            }
            Err(error) => {
                tracing::error!(error = %error, path = %path, "failed to open message body store; running without it");
                None
            }
        }
    }

    #[cfg(not(feature = "rocksdb"))]
    {
        tracing::info!(
            "message body store is enabled in config but this binary was built without the `rocksdb` feature; running without it"
        );
        None
    }
}
