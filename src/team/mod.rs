mod manager;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use manager::TeamManager;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRunEventRecord {
    pub event_id: i64,
    pub run_id: String,
    pub step_id: Option<String>,
    pub event_type: String,
    pub ts: i64,
    pub payload: Value,
}
