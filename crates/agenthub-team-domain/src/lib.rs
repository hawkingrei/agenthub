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

pub const TEAM_MAIN_TASK_STATUS_VALUES: [&str; 4] =
    ["open", "in_progress", "completed", "canceled"];

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamMainTaskStatus {
    Open,
    InProgress,
    Completed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMainTaskRecord {
    pub id: String,
    pub team_id: String,
    pub title: String,
    pub status: TeamMainTaskStatus,
    pub created_by_actor_id: String,
    pub context: Value,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamConversationRecord {
    pub id: String,
    pub team_id: String,
    pub main_task_id: String,
    pub mode: String,
    pub topic: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamConversationMessageRecord {
    pub message_id: i64,
    pub conversation_id: String,
    pub main_task_id: String,
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
        TEAM_MAIN_TASK_STATUS_VALUES, TEAM_RUN_STATUS_VALUES, TEAM_STEP_STATUS_VALUES,
        TeamRunResumeError,
    };

    #[test]
    fn status_values_keep_expected_length() {
        assert_eq!(TEAM_RUN_STATUS_VALUES.len(), 6);
        assert_eq!(TEAM_STEP_STATUS_VALUES.len(), 6);
        assert_eq!(TEAM_MAIN_TASK_STATUS_VALUES.len(), 4);
    }

    #[test]
    fn resume_error_message_is_stable() {
        assert_eq!(
            TeamRunResumeError::CompletedRun.to_string(),
            "completed run cannot be resumed"
        );
    }
}
