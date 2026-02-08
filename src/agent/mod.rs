mod manager;

use serde::{Deserialize, Serialize};

pub use manager::AgentManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub workdir: String,
    pub command: String,
    pub args: Vec<String>,
    pub worktree_mode: WorktreeMode,
    pub worktree_repo: Option<String>,
    pub worktree_ref: Option<String>,
    pub code_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRecord {
    pub id: String,
    pub name: String,
    pub workdir: String,
    pub command: String,
    pub args: Vec<String>,
    pub worktree_mode: WorktreeMode,
    pub worktree_repo: Option<String>,
    pub worktree_ref: Option<String>,
    pub code_mode: bool,
    pub status: AgentStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Created,
    Running,
    Stopped,
    Exited,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    pub agent_id: String,
    pub session_id: String,
    pub seq: String,
    pub ts: i64,
    pub stream: OutputStream,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub agent_id: String,
    pub session_id: String,
    pub seq: String,
    pub ts: i64,
    pub stream: OutputStream,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputStream {
    Stdout,
    Stderr,
    System,
    Acp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeMode {
    UseExisting,
    CreateWorktree,
    ReuseWorktree,
}
