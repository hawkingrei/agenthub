mod manager;
mod orchestrator;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use agenthub_team_actor::{
    ActorMessageRecord as TeamActorMessageRecord, ActorMessageStatus as TeamActorMessageStatus,
    ActorMessageTransport as TeamActorMessageTransport,
};
pub use manager::{TeamManager, TeamRemoteRelayWorkerSettings};
pub use orchestrator::{TeamOrchestratorWorker, TeamOrchestratorWorkerSettings};

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
