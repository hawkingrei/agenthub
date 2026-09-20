//! Bounded metric categories for an authorized actor scope. IDs are not metric labels.

use serde::{Deserialize, Serialize};

use crate::loop_runtime::{
    LoopEventKind, LoopExitReason, LoopOutcomeKind, LoopPolicyState, LoopWaitReason,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopMetricCount<T> {
    pub kind: T,
    pub count: i64,
}

/// Cross-process lifecycle intervals are wall-clock estimates, not monotonic measurements.
/// Regressing clock samples are reported separately and omitted from the duration aggregate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopWallDurationMetrics {
    pub samples: i64,
    pub total_seconds: i64,
    pub maximum_seconds: Option<i64>,
    pub clock_regressions: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopPendingMetrics {
    pub count: i64,
    pub oldest_age_seconds: Option<i64>,
    pub due_count: i64,
    pub oldest_due_age_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopDuplicateMetrics {
    /// Cumulative observed retries, including sources older than the selected event window.
    pub suppressed_total: i64,
    /// Pre-instrumentation sources have an unknown baseline; their total is a lower bound.
    pub sources_with_unknown_baseline: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopProgressMetrics {
    pub finalized_activations: i64,
    /// Divide by finalized_activations for the observed no-progress rate. Zero samples is unknown.
    pub no_progress_activations: i64,
    pub current_no_progress_streak: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoopWaitMetricKind {
    Due,
    Recurring,
    TaskStatus,
    ThreadReply,
    AppEvent,
}

impl LoopWaitMetricKind {
    pub const ALL: [Self; 5] = [
        Self::Due,
        Self::Recurring,
        Self::TaskStatus,
        Self::ThreadReply,
        Self::AppEvent,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Due => "due",
            Self::Recurring => "recurring",
            Self::TaskStatus => "task_status",
            Self::ThreadReply => "thread_reply",
            Self::AppEvent => "app_event",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopWaitMetrics {
    pub kind: LoopWaitMetricKind,
    pub count: i64,
    pub oldest_age_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopBusinessWaitMetrics {
    pub reason: LoopWaitReason,
    pub recorded_at: i64,
    pub age_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopMemObservation {
    pub kind: LoopEventKind,
    pub observed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopMemMetrics {
    /// None means there is no availability observation, rather than an unavailable service.
    pub latest: Option<LoopMemObservation>,
    pub observations: Vec<LoopMetricCount<LoopEventKind>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopMetricsSnapshot {
    pub observed_at: i64,
    pub window_start: i64,
    pub policy_state: Option<LoopPolicyState>,
    pub pending: LoopPendingMetrics,
    pub admission_latency: LoopWallDurationMetrics,
    pub running_duration: LoopWallDurationMetrics,
    /// A retained reservation after running is not proof of a live provider process.
    pub unsettled_run_count: i64,
    pub oldest_unsettled_run_age_seconds: Option<i64>,
    pub outcomes: Vec<LoopMetricCount<LoopOutcomeKind>>,
    pub exits: Vec<LoopMetricCount<LoopExitReason>>,
    pub exits_without_reason: i64,
    pub startup_failures: i64,
    pub retries: i64,
    pub duplicates: LoopDuplicateMetrics,
    pub progress: LoopProgressMetrics,
    pub waits: Vec<LoopWaitMetrics>,
    pub current_wait: Option<LoopBusinessWaitMetrics>,
    pub mem: LoopMemMetrics,
}
