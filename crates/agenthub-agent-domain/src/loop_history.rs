//! Payload-free projections for authorized activation history, including absent executors.

use serde::{Deserialize, Serialize};

use crate::loop_runtime::{
    LoopActivation, LoopEvent, LoopSourceReferences, LoopToolStatus, LoopToolSurface,
    LoopTriggerKind,
};

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

/// A boundary observation is not a task outcome or evidence of an external effect.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopToolSummary {
    pub id: i64,
    pub activation_id: String,
    pub generation: i64,
    pub surface: LoopToolSurface,
    pub tool_name: String,
    pub target_ref: Option<String>,
    pub operation_id: Option<String>,
    pub attempt_number: Option<u32>,
    pub status: LoopToolStatus,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    /// Measured with a process-local monotonic clock; absent if no completion was observed.
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopToolHistoryPage {
    pub tools: Vec<LoopToolSummary>,
    pub next_cursor: Option<i64>,
}
