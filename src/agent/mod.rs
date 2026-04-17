pub(crate) mod event_message_codec;
mod manager;
mod node;
mod triggers;

use serde::{Deserialize, Serialize};

pub(crate) use manager::derive_team_runtime_workdir;
pub use manager::{AgentManager, AgentSendInputError};
pub(crate) use node::{
    AGENT_NODE_MAIN_ID, build_main_agent_node_record, normalize_target_node_id,
    validate_agent_node_config_input, validate_agent_node_update_input,
};
pub use triggers::{
    AgentTimeTriggerCreateInput, AgentTimeTriggerManager, AgentTimeTriggerRecord,
    AgentTimeTriggerWorker, AgentTimeTriggerWorkerSettings,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub workdir: String,
    pub command: String,
    pub args: Vec<String>,
    pub target_node_id: Option<String>,
    pub worktree_mode: WorktreeMode,
    pub worktree_repo: Option<String>,
    pub worktree_ref: Option<String>,
    pub code_mode: bool,
    pub agent_loop_enabled: bool,
    pub agent_loop_idle_seconds: Option<i64>,
    pub agent_loop_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRecord {
    pub id: String,
    pub name: String,
    pub workdir: String,
    pub command: String,
    pub args: Vec<String>,
    pub target_node_id: Option<String>,
    pub worktree_mode: WorktreeMode,
    pub worktree_repo: Option<String>,
    pub worktree_ref: Option<String>,
    pub code_mode: bool,
    pub agent_loop_enabled: bool,
    pub agent_loop_idle_seconds: Option<i64>,
    pub agent_loop_prompt: Option<String>,
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
    pub event_id: i64,
    pub agent_id: String,
    pub session_id: String,
    pub seq: String,
    pub ts: i64,
    pub stream: OutputStream,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub event_id: i64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNodeConfig {
    pub id: String,
    pub name: String,
    pub grpc_target: String,
    pub tls_server_name: Option<String>,
    pub default_worktree_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNodeUpdate {
    pub name: String,
    pub grpc_target: String,
    pub tls_server_name: Option<String>,
    pub default_worktree_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentNodeJoinBootstrapInfo {
    pub enabled: bool,
    pub bootstrap_token: Option<String>,
    pub grpc_listen_addr: Option<String>,
    pub security_mode: Option<String>,
    pub cert_dir: Option<String>,
    pub issuer: Option<String>,
    pub audience: Option<String>,
}

impl AgentNodeJoinBootstrapInfo {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            bootstrap_token: None,
            grpc_listen_addr: None,
            security_mode: None,
            cert_dir: None,
            issuer: None,
            audience: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNodeRecord {
    pub id: String,
    pub name: String,
    pub grpc_target: Option<String>,
    pub tls_server_name: Option<String>,
    pub default_worktree_root: Option<String>,
    pub is_main: bool,
    pub created_at: i64,
    pub updated_at: i64,
}
