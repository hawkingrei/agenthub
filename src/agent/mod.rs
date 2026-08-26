pub(crate) mod event_message_codec;
mod manager;
mod triggers;

pub(crate) use agenthub_agent_domain::{
    AGENT_NODE_MAIN_ID, build_main_agent_node_record, normalize_target_node_id,
    validate_agent_node_config_input, validate_agent_node_update_input,
};
pub use agenthub_agent_domain::{
    AgentConfig, AgentEvent, AgentNodeConfig, AgentNodeJoinBootstrapInfo, AgentNodeRecord,
    AgentNodeUpdate, AgentOutput, AgentRecord, AgentStatus, OutputStream, WorktreeMode,
};
pub(crate) use manager::derive_team_runtime_workdir;
pub use manager::{AgentInputImage, AgentManager, AgentSendInputError};
pub use triggers::{
    AgentTimeTriggerCreateInput, AgentTimeTriggerManager, AgentTimeTriggerRecord,
    AgentTimeTriggerWorker, AgentTimeTriggerWorkerSettings,
};
