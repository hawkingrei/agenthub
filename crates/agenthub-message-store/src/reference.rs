//! The body-free delivery index row.

use serde::{Deserialize, Serialize};

use crate::ids::{AuthorityMessageId, DeliveryMessageId, MessageKind};

/// A compact, body-free row stored in the delivery index.
///
/// It carries enough metadata to render an ordered list without fetching the body, plus the
/// [`AuthorityMessageId`] needed to locate the body in the body store. By construction it has no body
/// field: full bodies live only in the body store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageRef {
    /// Per-delivery id for this index row.
    pub message_id: DeliveryMessageId,
    /// Canonical logical-message identity; the body-store key.
    pub authority_message_id: AuthorityMessageId,
    /// Optional pointer to the LanceDB archive document.
    pub archive_document_id: Option<String>,
    /// Authority-assigned creation time (epoch).
    pub created_at: i64,
    /// Origin of the row, e.g. `team_conversation_messages`.
    pub source_kind: String,
    /// Presentation hint for list rendering.
    pub message_kind: MessageKind,
    pub correlation_id: Option<String>,
    pub group_id: Option<String>,
    pub run_id: Option<String>,
    pub conversation_id: Option<String>,
    pub agent_id: Option<String>,
}

impl MessageRef {
    /// Serialize for storage. The encoding is deliberately behind this method so the physical format
    /// can be made more compact later without changing call sites.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize a stored row.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
