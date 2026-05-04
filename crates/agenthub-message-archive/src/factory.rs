use std::sync::Arc;

use anyhow::{Result, bail};

use crate::lance::LanceDbMessageArchive;
use crate::model::{MessageArchiveBackend, MessageArchiveConfig, MessageArchiveStore};

pub type MessageArchiveStoreRef = Arc<dyn MessageArchiveStore>;

pub async fn open_message_archive_store(
    config: MessageArchiveConfig,
) -> Result<MessageArchiveStoreRef> {
    match config.backend {
        MessageArchiveBackend::LanceDb => {
            let store = LanceDbMessageArchive::connect(config).await?;
            Ok(Arc::new(store))
        }
        MessageArchiveBackend::Sqlite => {
            bail!("sqlite message archive backend is not implemented in phase 1")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::open_message_archive_store;
    use crate::model::{MessageArchiveBackend, MessageArchiveConfig};

    #[tokio::test]
    async fn opens_lancedb_backend_via_factory() {
        let store = open_message_archive_store(MessageArchiveConfig {
            backend: MessageArchiveBackend::LanceDb,
            uri: "memory://factory-message-archive".to_string(),
            message_table: "messages".to_string(),
        })
        .await
        .expect("lancedb store opens");

        store.ensure_ready().await.expect("store initializes");
    }

    #[tokio::test]
    async fn sqlite_backend_is_rejected_in_phase_one() {
        let result = open_message_archive_store(MessageArchiveConfig {
            backend: MessageArchiveBackend::Sqlite,
            uri: ":memory:".to_string(),
            message_table: "messages".to_string(),
        })
        .await;

        match result {
            Ok(_) => panic!("sqlite backend should not open yet"),
            Err(error) => assert!(
                error
                    .to_string()
                    .contains("sqlite message archive backend is not implemented in phase 1")
            ),
        }
    }
}
