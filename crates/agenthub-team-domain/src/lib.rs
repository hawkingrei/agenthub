use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};
use serde_json::Value;
use std::collections::BTreeMap;

pub const TEAM_RUNTIME_STATE_SCHEMA_FAMILY: &str = "team_runtime_state";
pub const TEAM_RUNTIME_STATE_SCHEMA_VERSION: i64 = 1;
pub const TEAM_CONTINUITY_NOTE_SCHEMA_FAMILY: &str = "team_continuity_note";
pub const TEAM_CONTINUITY_NOTE_SCHEMA_VERSION: i64 = 1;
pub const TEAM_RUNTIME_STATE_RELATIVE_PATH: &str = ".cache/context/state.md";

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

pub const TEAM_TASK_STATUS_VALUES: [&str; 6] = [
    "open",
    "in_progress",
    "waiting",
    "in_review",
    "completed",
    "canceled",
];
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamChannelRecord {
    pub team_id: String,
    pub channel_id: String,
    pub task_id: String,
    pub conversation_id: String,
    pub description: Option<String>,
    pub created_by_actor_id: String,
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
    Waiting,
    InReview,
    Completed,
    Canceled,
}

impl TeamTaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::InProgress => "in_progress",
            Self::Waiting => "waiting",
            Self::InReview => "in_review",
            Self::Completed => "completed",
            Self::Canceled => "canceled",
        }
    }
}

impl std::str::FromStr for TeamTaskStatus {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw.trim() {
            "open" => Ok(Self::Open),
            "in_progress" => Ok(Self::InProgress),
            "waiting" => Ok(Self::Waiting),
            "in_review" => Ok(Self::InReview),
            "completed" => Ok(Self::Completed),
            "canceled" => Ok(Self::Canceled),
            other => Err(other.to_string()),
        }
    }
}

pub fn continuity_note_relative_path(source_run_id: &str) -> Option<String> {
    let source_run_id = source_run_id.trim();
    if source_run_id.is_empty() {
        return None;
    }
    Some(format!(".cache/context/run/{source_run_id}/continuity.md"))
}

pub fn extract_context_artifact_path(value: &Value) -> Option<&str> {
    value
        .get("path")
        .and_then(Value::as_str)
        .or_else(|| value.as_str())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamRuntimeStateIndex {
    pub schema_family: Option<String>,
    pub schema_version: Option<i64>,
    pub updated_at: Option<String>,
    pub team_id: Option<String>,
    pub member_id: Option<String>,
    pub current_execution_run_id: Option<String>,
    pub continuity_mode: Option<String>,
    pub continuity_source_execution_run_id: Option<String>,
    pub continuity_note_path: Option<String>,
    pub continuity_artifact_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamContinuityNoteHeader {
    pub schema_family: Option<String>,
    pub schema_version: Option<i64>,
    pub updated_at: Option<String>,
    pub team_id: Option<String>,
    pub member_id: Option<String>,
    pub current_execution_run_id: Option<String>,
    pub continuity_source_execution_run_id: Option<String>,
    pub continuity_mode: Option<String>,
    pub continuity_artifact_path: Option<String>,
}

pub fn parse_team_runtime_state_index(contents: &str) -> Option<TeamRuntimeStateIndex> {
    let metadata = parse_machine_read_markdown_metadata(contents, "# Team Runtime State")?;
    Some(TeamRuntimeStateIndex {
        schema_family: metadata.get("schema_family").cloned(),
        schema_version: metadata
            .get("schema_version")
            .and_then(|value| value.parse::<i64>().ok()),
        updated_at: metadata.get("updated_at").cloned(),
        team_id: metadata.get("team_id").cloned(),
        member_id: metadata.get("member_id").cloned(),
        current_execution_run_id: metadata.get("current_execution_run_id").cloned(),
        continuity_mode: metadata.get("continuity_mode").cloned(),
        continuity_source_execution_run_id: metadata
            .get("continuity_source_execution_run_id")
            .cloned(),
        continuity_note_path: metadata.get("continuity_note_path").cloned(),
        continuity_artifact_path: metadata.get("continuity_artifact_path").cloned(),
    })
}

pub fn parse_team_continuity_note_header(contents: &str) -> Option<TeamContinuityNoteHeader> {
    let metadata = parse_machine_read_markdown_metadata(contents, "# Team Continuity Note")?;
    Some(TeamContinuityNoteHeader {
        schema_family: metadata.get("schema_family").cloned(),
        schema_version: metadata
            .get("schema_version")
            .and_then(|value| value.parse::<i64>().ok()),
        updated_at: metadata.get("updated_at").cloned(),
        team_id: metadata.get("team_id").cloned(),
        member_id: metadata.get("member_id").cloned(),
        current_execution_run_id: metadata.get("current_execution_run_id").cloned(),
        continuity_source_execution_run_id: metadata
            .get("continuity_source_execution_run_id")
            .cloned(),
        continuity_mode: metadata.get("continuity_mode").cloned(),
        continuity_artifact_path: metadata.get("continuity_artifact_path").cloned(),
    })
}

fn parse_machine_read_markdown_metadata(
    contents: &str,
    expected_title: &str,
) -> Option<BTreeMap<String, String>> {
    let mut lines = contents.lines().skip_while(|line| line.trim().is_empty());
    let title = lines.next()?.trim();
    if title != expected_title {
        return None;
    }

    let mut metadata = BTreeMap::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("## ") {
            break;
        }
        let Some(rest) = trimmed.strip_prefix("- ") else {
            continue;
        };
        let Some((key, value)) = rest.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        metadata.insert(key.to_string(), value.trim().to_string());
    }
    Some(metadata)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamTaskRecord {
    pub id: String,
    pub team_id: String,
    pub title: String,
    pub status: TeamTaskStatus,
    pub created_by_actor_id: String,
    pub assigned_member_id: Option<String>,
    pub context: Value,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TeamTaskStepExecutionMode {
    #[default]
    SinglePass,
    ReconcileLoop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TeamTaskStepExecutionSpec {
    pub mode: TeamTaskStepExecutionMode,
    #[serde(default)]
    pub max_rounds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamTaskExecutionStepSpec {
    pub step_key: String,
    pub member_id: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default)]
    pub acceptance: Vec<String>,
    #[serde(default)]
    pub execution: TeamTaskStepExecutionSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamTaskExecutionPlan {
    pub steps: Vec<TeamTaskExecutionStepSpec>,
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
pub struct TeamTaskDetailRecord {
    pub task: TeamTaskRecord,
    pub conversation: TeamConversationRecord,
    pub latest_run: Option<TeamRunRecord>,
    pub recent_messages: Vec<TeamConversationMessageRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamThreadOpenRecord {
    pub team_id: String,
    pub channel_id: String,
    pub task_id: String,
    pub conversation_id: String,
    pub root_message_id: i64,
    pub thread_id: String,
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
        TEAM_CONTINUITY_NOTE_SCHEMA_FAMILY, TEAM_CONTINUITY_NOTE_SCHEMA_VERSION,
        TEAM_RUN_CONTINUITY_MODE_VALUES, TEAM_RUN_STATUS_VALUES, TEAM_RUNTIME_STATE_SCHEMA_FAMILY,
        TEAM_RUNTIME_STATE_SCHEMA_VERSION, TEAM_STEP_STATUS_VALUES, TEAM_TASK_STATUS_VALUES,
        TeamRunResumeError, TeamStepRecord, TeamStepStatus, TeamTaskStatus,
        continuity_note_relative_path, extract_context_artifact_path,
        parse_team_continuity_note_header, parse_team_runtime_state_index,
    };
    use serde_json::json;

    #[test]
    fn status_values_keep_expected_length() {
        assert_eq!(TEAM_RUN_STATUS_VALUES.len(), 6);
        assert_eq!(TEAM_STEP_STATUS_VALUES.len(), 6);
        assert_eq!(TEAM_TASK_STATUS_VALUES.len(), 6);
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
    fn team_task_status_string_roundtrip_is_stable() {
        for (status, label) in [
            (TeamTaskStatus::Open, "open"),
            (TeamTaskStatus::InProgress, "in_progress"),
            (TeamTaskStatus::Waiting, "waiting"),
            (TeamTaskStatus::InReview, "in_review"),
            (TeamTaskStatus::Completed, "completed"),
            (TeamTaskStatus::Canceled, "canceled"),
        ] {
            assert_eq!(status.as_str(), label);
            assert_eq!(
                label.parse::<TeamTaskStatus>().expect("parse status"),
                status
            );
        }
        assert_eq!(
            "invalid"
                .parse::<TeamTaskStatus>()
                .expect_err("invalid status"),
            "invalid"
        );
    }

    #[test]
    fn continuity_note_relative_path_uses_source_run_id() {
        assert_eq!(
            continuity_note_relative_path("run-42").as_deref(),
            Some(".cache/context/run/run-42/continuity.md")
        );
        assert_eq!(continuity_note_relative_path("   ").as_deref(), None);
    }

    #[test]
    fn extract_context_artifact_path_accepts_object_and_legacy_string() {
        assert_eq!(
            extract_context_artifact_path(&json!({
                "path": ".cache/context/run/run-42/artifact-0001.json"
            })),
            Some(".cache/context/run/run-42/artifact-0001.json")
        );
        assert_eq!(
            extract_context_artifact_path(&json!(".cache/context/run/run-42/artifact-0001.json")),
            Some(".cache/context/run/run-42/artifact-0001.json")
        );
        assert_eq!(
            extract_context_artifact_path(&json!({ "other": "value" })),
            None
        );
    }

    #[test]
    fn parse_team_runtime_state_index_reads_current_schema_metadata() {
        let parsed = parse_team_runtime_state_index(
            format!(
                "# Team Runtime State\n\n- schema_family: {TEAM_RUNTIME_STATE_SCHEMA_FAMILY}\n- schema_version: {TEAM_RUNTIME_STATE_SCHEMA_VERSION}\n- updated_at: 123\n- team_id: team-1\n- member_id: worker\n- current_execution_run_id: run-7\n- continuity_mode: inherit_recent\n- continuity_source_execution_run_id: run-6\n- continuity_note_path: .cache/context/run/run-6/continuity.md\n- continuity_artifact_path: .cache/context/run/run-6/artifact-0001.json\n"
            )
            .as_str(),
        )
        .expect("parse runtime state");
        assert_eq!(
            parsed.schema_family.as_deref(),
            Some(TEAM_RUNTIME_STATE_SCHEMA_FAMILY)
        );
        assert_eq!(
            parsed.schema_version,
            Some(TEAM_RUNTIME_STATE_SCHEMA_VERSION)
        );
        assert_eq!(parsed.current_execution_run_id.as_deref(), Some("run-7"));
        assert_eq!(
            parsed.continuity_note_path.as_deref(),
            Some(".cache/context/run/run-6/continuity.md")
        );
    }

    #[test]
    fn parse_team_runtime_state_index_accepts_legacy_shape_without_schema() {
        let parsed = parse_team_runtime_state_index(
            "# Team Runtime State\n\n- updated_at: 123\n- team_id: team-1\n- member_id: worker\n- current_execution_run_id: run-7\n- continuity_mode: inherit_recent\n",
        )
        .expect("parse runtime state");
        assert_eq!(parsed.schema_family, None);
        assert_eq!(parsed.schema_version, None);
        assert_eq!(parsed.team_id.as_deref(), Some("team-1"));
        assert_eq!(parsed.member_id.as_deref(), Some("worker"));
    }

    #[test]
    fn parse_team_runtime_state_index_skips_leading_blank_lines() {
        let parsed = parse_team_runtime_state_index(
            "\n\n  \n# Team Runtime State\n- team_id: team-1\n- member_id: worker-1\n",
        )
        .expect("parse runtime state after leading blank lines");
        assert_eq!(parsed.team_id.as_deref(), Some("team-1"));
        assert_eq!(parsed.member_id.as_deref(), Some("worker-1"));
    }

    #[test]
    fn parse_team_continuity_note_header_stops_before_summary_body() {
        let parsed = parse_team_continuity_note_header(
            format!(
                "# Team Continuity Note\n\n- schema_family: {TEAM_CONTINUITY_NOTE_SCHEMA_FAMILY}\n- schema_version: {TEAM_CONTINUITY_NOTE_SCHEMA_VERSION}\n- updated_at: 123\n- team_id: team-1\n- member_id: worker\n- current_execution_run_id: run-7\n- continuity_source_execution_run_id: run-6\n- continuity_mode: inherit_recent\n- continuity_artifact_path: .cache/context/run/run-6/artifact-0001.json\n\n## Summary\nlarge payload\n\n## History Window\n````json\n{{\"schema_version\":1}}\n````\n"
            )
            .as_str(),
        )
        .expect("parse continuity note");
        assert_eq!(
            parsed.schema_family.as_deref(),
            Some(TEAM_CONTINUITY_NOTE_SCHEMA_FAMILY)
        );
        assert_eq!(
            parsed.schema_version,
            Some(TEAM_CONTINUITY_NOTE_SCHEMA_VERSION)
        );
        assert_eq!(
            parsed.continuity_artifact_path.as_deref(),
            Some(".cache/context/run/run-6/artifact-0001.json")
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
