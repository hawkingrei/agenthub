use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MessageArchiveBackend {
    Sqlite,
    #[default]
    #[serde(rename = "lancedb", alias = "lance_db")]
    LanceDb,
}

impl FromStr for MessageArchiveBackend {
    type Err = MessageArchiveBackendParseError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "sqlite" => Ok(Self::Sqlite),
            "lancedb" | "lance_db" => Ok(Self::LanceDb),
            other => Err(MessageArchiveBackendParseError {
                value: other.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageArchiveBackendParseError {
    value: String,
}

impl fmt::Display for MessageArchiveBackendParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unsupported message archive backend: {}", self.value)
    }
}

impl std::error::Error for MessageArchiveBackendParseError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageArchiveConfig {
    pub backend: MessageArchiveBackend,
    pub uri: String,
    pub message_table: String,
}

impl MessageArchiveConfig {
    pub fn new(backend: MessageArchiveBackend, uri: impl Into<String>) -> Self {
        Self {
            backend,
            uri: uri.into(),
            message_table: "messages".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageDocumentKind {
    AgentEvent,
    TeamConversationMessage,
    TeamRunEvent,
    TeamActorMessage,
    AggregatedAcpMessage,
}

impl MessageDocumentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AgentEvent => "agent_event",
            Self::TeamConversationMessage => "team_conversation_message",
            Self::TeamRunEvent => "team_run_event",
            Self::TeamActorMessage => "team_actor_message",
            Self::AggregatedAcpMessage => "aggregated_acp_message",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageDocument {
    pub document_id: String,
    pub source_kind: MessageDocumentKind,
    pub source_id: String,
    pub logical_message_id: Option<String>,
    pub authority_message_id: Option<i64>,
    pub correlation_id: Option<String>,
    pub group_id: Option<String>,
    pub team_id: Option<String>,
    pub run_id: Option<String>,
    pub conversation_id: Option<String>,
    pub task_id: Option<String>,
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub body_text: String,
    pub payload_json: Option<String>,
    pub created_at: i64,
    pub event_id_from: Option<i64>,
    pub event_id_to: Option<i64>,
    pub chunk_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MessageSearchQuery {
    pub query_text: String,
    pub limit: usize,
    pub authority_message_id: Option<i64>,
    pub correlation_id: Option<String>,
    pub group_id: Option<String>,
    pub team_id: Option<String>,
    pub run_id: Option<String>,
    pub conversation_id: Option<String>,
    pub task_id: Option<String>,
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub source_kind: Option<MessageDocumentKind>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MessageSearchHit {
    pub document_id: String,
    pub source_kind: MessageDocumentKind,
    pub body_text: String,
    pub score: Option<f32>,
    pub authority_message_id: Option<i64>,
    pub correlation_id: Option<String>,
    pub group_id: Option<String>,
    pub team_id: Option<String>,
    pub run_id: Option<String>,
    pub conversation_id: Option<String>,
    pub task_id: Option<String>,
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingArchiveDocument {
    pub document_id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArchiveDocumentIntegrityReport {
    pub checked_documents: usize,
    pub missing_documents: Vec<MissingArchiveDocument>,
}

impl ArchiveDocumentIntegrityReport {
    pub fn is_clean(&self) -> bool {
        self.missing_documents.is_empty()
    }
}

#[async_trait]
pub trait MessageArchiveStore: Send + Sync {
    async fn ensure_ready(&self) -> Result<()>;
    async fn append_documents(&self, documents: &[MessageDocument]) -> Result<()>;
    async fn contains_document(&self, document_id: &str) -> Result<bool>;
    async fn search(&self, query: &MessageSearchQuery) -> Result<Vec<MessageSearchHit>>;
}

pub async fn check_archive_documents_exist<S, D>(
    archive: &S,
    document_ids: impl IntoIterator<Item = D>,
) -> Result<ArchiveDocumentIntegrityReport>
where
    S: MessageArchiveStore + ?Sized,
    D: AsRef<str>,
{
    let mut report = ArchiveDocumentIntegrityReport::default();
    for document_id in document_ids {
        let document_id = document_id.as_ref().trim();
        if document_id.is_empty() {
            continue;
        }
        report.checked_documents += 1;
        if archive.contains_document(document_id).await? {
            continue;
        }
        report.missing_documents.push(MissingArchiveDocument {
            document_id: document_id.to_string(),
        });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::{
        MessageArchiveBackend, MessageArchiveStore, MessageDocument, MessageSearchHit,
        MessageSearchQuery, check_archive_documents_exist,
    };
    use async_trait::async_trait;
    use std::str::FromStr;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingArchive {
        documents: Mutex<Vec<MessageDocument>>,
    }

    #[async_trait]
    impl MessageArchiveStore for RecordingArchive {
        async fn ensure_ready(&self) -> anyhow::Result<()> {
            Ok(())
        }

        async fn append_documents(&self, documents: &[MessageDocument]) -> anyhow::Result<()> {
            self.documents
                .lock()
                .expect("archive mutex poisoned")
                .extend_from_slice(documents);
            Ok(())
        }

        async fn contains_document(&self, document_id: &str) -> anyhow::Result<bool> {
            Ok(self
                .documents
                .lock()
                .expect("archive mutex poisoned")
                .iter()
                .any(|document| document.document_id == document_id))
        }

        async fn search(
            &self,
            _query: &MessageSearchQuery,
        ) -> anyhow::Result<Vec<MessageSearchHit>> {
            Ok(Vec::new())
        }
    }

    fn document(document_id: &str) -> MessageDocument {
        MessageDocument {
            document_id: document_id.to_string(),
            source_kind: super::MessageDocumentKind::TeamConversationMessage,
            source_id: "1".to_string(),
            logical_message_id: None,
            authority_message_id: None,
            correlation_id: None,
            group_id: None,
            team_id: None,
            run_id: None,
            conversation_id: None,
            task_id: None,
            agent_id: None,
            session_id: None,
            body_text: "body".to_string(),
            payload_json: None,
            created_at: 1,
            event_id_from: None,
            event_id_to: None,
            chunk_count: None,
        }
    }

    #[test]
    fn lancedb_backend_uses_canonical_config_string() {
        let encoded =
            serde_json::to_string(&MessageArchiveBackend::LanceDb).expect("backend serializes");
        assert_eq!(encoded, "\"lancedb\"");

        let canonical: MessageArchiveBackend =
            serde_json::from_str("\"lancedb\"").expect("canonical string parses");
        assert_eq!(canonical, MessageArchiveBackend::LanceDb);

        let legacy_alias: MessageArchiveBackend =
            serde_json::from_str("\"lance_db\"").expect("legacy alias parses");
        assert_eq!(legacy_alias, MessageArchiveBackend::LanceDb);
    }

    #[test]
    fn message_archive_backend_parses_config_values() {
        assert_eq!(
            MessageArchiveBackend::from_str(" lancedb ").expect("parse canonical backend"),
            MessageArchiveBackend::LanceDb
        );
        assert_eq!(
            MessageArchiveBackend::from_str("lance_db").expect("parse legacy backend alias"),
            MessageArchiveBackend::LanceDb
        );
        assert_eq!(
            MessageArchiveBackend::from_str("sqlite").expect("parse sqlite backend"),
            MessageArchiveBackend::Sqlite
        );
        assert!(
            MessageArchiveBackend::from_str("tantivy").is_err(),
            "unsupported backends should fail fast"
        );
    }

    #[tokio::test]
    async fn archive_document_integrity_reports_missing_documents() {
        let archive = RecordingArchive::default();
        archive
            .append_documents(&[document("doc-present")])
            .await
            .expect("append document");

        let report = check_archive_documents_exist(
            &archive,
            ["doc-present", "doc-missing", " ", "doc-missing-2"],
        )
        .await
        .expect("check archive documents");

        assert_eq!(report.checked_documents, 3);
        assert!(!report.is_clean());
        assert_eq!(
            report
                .missing_documents
                .iter()
                .map(|missing| missing.document_id.as_str())
                .collect::<Vec<_>>(),
            vec!["doc-missing", "doc-missing-2"]
        );
    }
}
