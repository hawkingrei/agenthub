use std::str::FromStr;

use agenthub_message_archive::{
    MessageArchiveBackend, MessageArchiveConfig, MessageArchiveStoreRef, open_message_archive_store,
};

use super::AppState;

impl AppState {
    pub(super) async fn initialize_message_archive(
        config: &agenthub_config::AppConfig,
    ) -> anyhow::Result<Option<MessageArchiveStoreRef>> {
        let backend_config = config.message_archive_backend();
        let backend_config = backend_config.trim();
        if backend_config.is_empty() {
            tracing::warn!("message archive disabled because backend config is empty");
            return Ok(None);
        }
        let backend = match MessageArchiveBackend::from_str(backend_config) {
            Ok(backend) => backend,
            Err(error) => {
                tracing::warn!(
                    error = ?error,
                    "message archive disabled because backend config could not be parsed"
                );
                return Ok(None);
            }
        };
        let archive_config = MessageArchiveConfig {
            backend,
            uri: config.message_archive_uri(),
            message_table: config.message_archive_table(),
        };
        let archive = match open_message_archive_store(archive_config).await {
            Ok(archive) => archive,
            Err(error) => {
                tracing::warn!(
                    error = ?error,
                    "message archive disabled because backend initialization failed"
                );
                return Ok(None);
            }
        };
        if let Err(error) = archive.ensure_ready().await {
            tracing::warn!(
                error = ?error,
                "message archive disabled because backend readiness check failed"
            );
            return Ok(None);
        }
        Ok(Some(archive))
    }
}
