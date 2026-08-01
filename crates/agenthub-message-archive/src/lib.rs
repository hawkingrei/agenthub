mod acp;
mod factory;
mod lance;
mod model;

pub use acp::{
    AcpAggregatedMessage, AcpChunkAccumulator, AcpEventRow, aggregate_acp_chunk_rows,
    is_aggregatable_acp_chunk,
};
pub use factory::{MessageArchiveStoreRef, open_message_archive_store};
pub use lance::LanceDbMessageArchive;
pub use model::{
    ArchiveDocumentIntegrityReport, MessageArchiveBackend, MessageArchiveConfig,
    MessageArchiveStore, MessageDocument, MessageDocumentKind, MessageSearchHit,
    MessageSearchQuery, MissingArchiveDocument, check_archive_documents_exist,
};
