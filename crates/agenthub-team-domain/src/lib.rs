use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};
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
    pub summary: Option<String>,
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

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct TeamStepRecord {
    pub id: String,
    pub run_id: String,
    pub step_key: String,
    pub member_id: String,
    /// Runtime executor handle for this step.
    ///
    /// In the current ACP-backed implementation this stores the member agent
    /// session id. Serialization keeps the legacy `remote_task_id` wire field
    /// alongside `runtime_handle_id` for compatibility.
    #[serde(default, alias = "remote_task_id")]
    pub runtime_handle_id: Option<String>,
    pub status: TeamStepStatus,
    pub attempt: i64,
    pub depends_on: Vec<String>,
    pub input: Option<Value>,
    pub output: Option<Value>,
    pub error_text: Option<String>,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
}

impl Serialize for TeamStepRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("TeamStepRecord", 14)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("run_id", &self.run_id)?;
        state.serialize_field("step_key", &self.step_key)?;
        state.serialize_field("member_id", &self.member_id)?;
        state.serialize_field("runtime_handle_id", &self.runtime_handle_id)?;
        state.serialize_field("remote_task_id", &self.runtime_handle_id)?;
        state.serialize_field("status", &self.status)?;
        state.serialize_field("attempt", &self.attempt)?;
        state.serialize_field("depends_on", &self.depends_on)?;
        state.serialize_field("input", &self.input)?;
        state.serialize_field("output", &self.output)?;
        state.serialize_field("error_text", &self.error_text)?;
        state.serialize_field("started_at", &self.started_at)?;
        state.serialize_field("ended_at", &self.ended_at)?;
        state.end()
    }
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
        TEAM_TASK_STATUS_VALUES, TeamRunResumeError, TeamStepRecord, TeamStepStatus,
    };
    use serde_json::json;

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

    #[test]
    fn step_record_serializes_runtime_handle_with_legacy_alias() {
        let step = TeamStepRecord {
            id: "step-1".to_string(),
            run_id: "run-1".to_string(),
            step_key: "leader_plan".to_string(),
            member_id: "leader".to_string(),
            runtime_handle_id: Some("session-1".to_string()),
            status: TeamStepStatus::Working,
            attempt: 1,
            depends_on: vec!["seed".to_string()],
            input: Some(json!({"goal":"plan"})),
            output: None,
            error_text: None,
            started_at: Some(1),
            ended_at: None,
        };

        let value = serde_json::to_value(&step).expect("serialize step");
        assert_eq!(value["runtime_handle_id"], "session-1");
        assert_eq!(value["remote_task_id"], "session-1");
    }

    #[test]
    fn step_record_deserializes_legacy_remote_task_id_alias() {
        let value = json!({
            "id": "step-1",
            "run_id": "run-1",
            "step_key": "leader_plan",
            "member_id": "leader",
            "remote_task_id": "session-legacy",
            "status": "working",
            "attempt": 1,
            "depends_on": [],
            "input": null,
            "output": null,
            "error_text": null,
            "started_at": null,
            "ended_at": null
        });

        let step: TeamStepRecord = serde_json::from_value(value).expect("deserialize step");
        assert_eq!(step.runtime_handle_id.as_deref(), Some("session-legacy"));
    }
}
