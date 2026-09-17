//! Payload-free projections for authorized activation history, including absent executors.

use serde::{Deserialize, Serialize};

use crate::loop_runtime::{LoopActivation, LoopEvent, LoopSourceReferences, LoopTriggerKind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopHistoryPage {
    pub activations: Vec<LoopActivation>,
    pub next_cursor: Option<String>,
}

/// A trigger's private source key and original input never enter the history surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopSourceSummary {
    pub id: String,
    pub kind: LoopTriggerKind,
    pub references: LoopSourceReferences,
    pub due_at: Option<i64>,
    pub created_at: i64,
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopSourceHistoryPage {
    pub sources: Vec<LoopSourceSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopEventHistoryPage {
    pub events: Vec<LoopEvent>,
    pub next_cursor: Option<i64>,
}
