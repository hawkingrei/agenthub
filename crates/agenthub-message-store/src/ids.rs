//! Message identity newtypes.
//!
//! The storage-tiering contract distinguishes two ids (see
//! `docs/features/message-storage-tiering.md` and
//! `docs/features/logical-message-metadata-contract.md`):
//!
//! - [`AuthorityMessageId`] is the canonical logical-message identity. There is exactly one body per
//!   `AuthorityMessageId`, so it is the body-store key.
//! - [`DeliveryMessageId`] identifies one delivery / fan-out attempt. It labels a delivery index row
//!   and must never be used as the body key, otherwise fan-out would duplicate a body.

use serde::{Deserialize, Serialize};

/// Canonical logical-message identity. One body per id.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AuthorityMessageId(String);

/// Per-delivery / fan-out id. Identifies a delivery index row, never the body.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeliveryMessageId(String);

impl AuthorityMessageId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl DeliveryMessageId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Compact presentation hint stored in a delivery index row so ordered list views can render an icon
/// or summary without fetching the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Text,
    ToolCall,
    Event,
    Thought,
    System,
}
