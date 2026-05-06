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

#[async_trait]
pub trait MessageArchiveStore: Send + Sync {
    async fn ensure_ready(&self) -> Result<()>;
    async fn append_documents(&self, documents: &[MessageDocument]) -> Result<()>;
    async fn search(&self, query: &MessageSearchQuery) -> Result<Vec<MessageSearchHit>>;
}

#[cfg(test)]
mod tests {
    use super::MessageArchiveBackend;
    use std::str::FromStr;

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
}
