use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const TEAM_RUN_STATUS_VALUES: [&str; 6] = [
    "submitted",
    "working",
    "input_required",
    "completed",
    "failed",
    "canceled",
];

pub const TEAM_STEP_STATUS_VALUES: [&str; 6] = [
    "submitted",
    "working",
    "input_required",
    "completed",
    "failed",
    "canceled",
];

pub const TEAM_TASK_STATUS_VALUES: [&str; 4] = ["open", "in_progress", "completed", "canceled"];
pub const TEAM_RUN_CONTINUITY_MODE_VALUES: [&str; 2] = ["inherit_recent", "reset"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamDefinitionConfig {
    pub name: String,
    pub description: Option<String>,
    pub spec: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamDefinitionRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub spec: Value,
    #[serde(skip_serializing, skip_deserializing, default)]
    pub owner_user_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamRunStatus {
    Submitted,
    Working,
    InputRequired,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRunRecord {
    pub id: String,
    pub team_id: String,
    pub context_id: String,
    pub status: TeamRunStatus,
    pub input: Value,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMemberContinuityStateRecord {
    pub team_id: String,
    pub member_id: String,
    pub source_run_id: String,
    pub source_session_id: Option<String>,
    pub summary_text: String,
    pub history_window: Value,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamTaskStatus {
    Open,
    InProgress,
    Completed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamTaskRecord {
    pub id: String,
    pub team_id: String,
    pub title: String,
    pub status: TeamTaskStatus,
    pub created_by_actor_id: String,
    pub context: Value,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamConversationRecord {
    pub id: String,
    pub team_id: String,
    pub task_id: String,
    pub mode: String,
    pub topic: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamConversationMessageRecord {
    pub message_id: i64,
    pub conversation_id: String,
    pub task_id: String,
    pub from_actor_id: String,
    pub to_actor_id: Option<String>,
    pub route: String,
    pub payload: Value,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRunEventRecord {
    pub event_id: i64,
    pub run_id: String,
    pub step_id: Option<String>,
    pub event_type: String,
    pub ts: i64,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum TeamStepStatus {
    Submitted,
    Working,
    InputRequired,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct TeamStepRecord {
    pub id: String,
    pub run_id: String,
    pub step_key: String,
    pub member_id: String,
    /// Runtime executor handle for this step.
    ///
    /// In the current ACP-backed implementation this stores the member agent
    /// session id. The field name is legacy and is kept for compatibility with
    /// existing DB/API/proto payloads until a broader rename can be rolled out.
    pub remote_task_id: Option<String>,
    pub status: TeamStepStatus,
    pub attempt: i64,
    pub depends_on: Vec<String>,
    pub input: Option<Value>,
    pub output: Option<Value>,
    pub error_text: Option<String>,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TeamRunResumeError {
    #[error("completed run cannot be resumed")]
    CompletedRun,
}

#[cfg(test)]
mod tests {
    use super::{
        TEAM_RUN_CONTINUITY_MODE_VALUES, TEAM_RUN_STATUS_VALUES, TEAM_STEP_STATUS_VALUES,
        TEAM_TASK_STATUS_VALUES, TeamRunResumeError,
    };

    #[test]
    fn status_values_keep_expected_length() {
        assert_eq!(TEAM_RUN_STATUS_VALUES.len(), 6);
        assert_eq!(TEAM_STEP_STATUS_VALUES.len(), 6);
        assert_eq!(TEAM_TASK_STATUS_VALUES.len(), 4);
        assert_eq!(TEAM_RUN_CONTINUITY_MODE_VALUES.len(), 2);
    }

    #[test]
    fn resume_error_message_is_stable() {
        assert_eq!(
            TeamRunResumeError::CompletedRun.to_string(),
            "completed run cannot be resumed"
        );
    }
}
