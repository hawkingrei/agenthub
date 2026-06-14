use serde::{Deserialize, Serialize};
use url::Url;

pub const AGENT_NODE_MAIN_ID: &str = "main";
pub const AGENT_NODE_MAIN_NAME: &str = "Main Node";

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
    pub codex_acp_default_mode: Option<String>,
    /// Operator-set runtime profile overrides (Agent Runtime Profiles). Provider-neutral; the effective
    /// provider is derived from `command`/`args`, so it is not stored here. `None` means use the
    /// adapter/provider default for that field.
    pub runtime_model: Option<String>,
    pub thinking_level: Option<String>,
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
    pub codex_acp_default_mode: Option<String>,
    /// Operator-set runtime profile overrides (Agent Runtime Profiles). Provider-neutral; the effective
    /// provider is derived from `command`/`args`, so it is not stored here. `None` means use the
    /// adapter/provider default for that field.
    pub runtime_model: Option<String>,
    pub thinking_level: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    pub group_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNodeUpdate {
    pub name: String,
    pub grpc_target: String,
    pub tls_server_name: Option<String>,
    pub default_worktree_root: Option<String>,
    pub group_id: Option<String>,
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
    pub group_id: Option<String>,
    pub last_seen_at: Option<i64>,
    pub is_main: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug)]
pub struct ValidatedAgentNodeConfig {
    pub id: String,
    pub name: String,
    pub grpc_target: String,
    pub tls_server_name: Option<String>,
    pub default_worktree_root: Option<String>,
    pub group_id: Option<String>,
}

#[derive(Debug)]
pub struct ValidatedAgentNodeUpdate {
    pub name: String,
    pub grpc_target: String,
    pub tls_server_name: Option<String>,
    pub default_worktree_root: Option<String>,
    pub group_id: Option<String>,
}

pub fn build_main_agent_node_record() -> AgentNodeRecord {
    AgentNodeRecord {
        id: AGENT_NODE_MAIN_ID.to_string(),
        name: AGENT_NODE_MAIN_NAME.to_string(),
        grpc_target: None,
        tls_server_name: None,
        default_worktree_root: None,
        group_id: None,
        last_seen_at: None,
        is_main: true,
        created_at: 0,
        updated_at: 0,
    }
}

pub fn normalize_target_node_id(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty() && *value != AGENT_NODE_MAIN_ID)
        .map(str::to_string)
}

pub fn validate_agent_node_config_input(
    config: &AgentNodeConfig,
) -> anyhow::Result<ValidatedAgentNodeConfig> {
    let id = validate_agent_node_id(&config.id)?;
    let name = validate_agent_node_name(&config.name)?;
    let grpc_target = validate_agent_node_grpc_target(&config.grpc_target)?;
    let tls_server_name = config
        .tls_server_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let default_worktree_root =
        validate_agent_node_default_worktree_root(config.default_worktree_root.as_deref())?;
    let group_id = validate_agent_node_group_id(config.group_id.as_deref())?;
    Ok(ValidatedAgentNodeConfig {
        id,
        name,
        grpc_target,
        tls_server_name,
        default_worktree_root,
        group_id,
    })
}

pub fn validate_agent_node_update_input(
    config: &AgentNodeUpdate,
) -> anyhow::Result<ValidatedAgentNodeUpdate> {
    let name = validate_agent_node_name(&config.name)?;
    let grpc_target = validate_agent_node_grpc_target(&config.grpc_target)?;
    let tls_server_name = config
        .tls_server_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let default_worktree_root =
        validate_agent_node_default_worktree_root(config.default_worktree_root.as_deref())?;
    let group_id = validate_agent_node_group_id(config.group_id.as_deref())?;
    Ok(ValidatedAgentNodeUpdate {
        name,
        grpc_target,
        tls_server_name,
        default_worktree_root,
        group_id,
    })
}

fn validate_agent_node_id(raw: &str) -> anyhow::Result<String> {
    let id = raw.trim();
    if id.is_empty() {
        anyhow::bail!("agent node id is required");
    }
    if id == AGENT_NODE_MAIN_ID {
        anyhow::bail!("agent node id '{}' is reserved", AGENT_NODE_MAIN_ID);
    }
    if id.len() > 128 {
        anyhow::bail!("agent node id must be at most 128 characters");
    }
    if !id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ':'))
    {
        anyhow::bail!(
            "agent node id must contain only ASCII letters, numbers, '.', '_', '-', or ':'"
        );
    }
    Ok(id.to_string())
}

fn validate_agent_node_name(raw: &str) -> anyhow::Result<String> {
    let name = raw.trim();
    if name.is_empty() {
        anyhow::bail!("agent node name is required");
    }
    if name.len() > 128 {
        anyhow::bail!("agent node name must be at most 128 characters");
    }
    Ok(name.to_string())
}

fn validate_agent_node_grpc_target(raw: &str) -> anyhow::Result<String> {
    let target = raw.trim();
    if target.is_empty() {
        anyhow::bail!("agent node gRPC target is required");
    }
    let url = Url::parse(target)
        .map_err(|err| anyhow::anyhow!("invalid agent node gRPC target: {}", err))?;
    if url.scheme() != "https" {
        anyhow::bail!("agent node gRPC target must use https://");
    }
    if url.host_str().unwrap_or_default().is_empty() {
        anyhow::bail!("agent node gRPC target must include a host");
    }
    Ok(target.to_string())
}

fn validate_agent_node_default_worktree_root(raw: Option<&str>) -> anyhow::Result<Option<String>> {
    let Some(trimmed) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if trimmed.len() > 1024 {
        anyhow::bail!("agent node default worktree root must be at most 1024 characters");
    }
    Ok(Some(trimmed.to_string()))
}

fn validate_agent_node_group_id(raw: Option<&str>) -> anyhow::Result<Option<String>> {
    let Some(trimmed) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if trimmed.len() > 256 {
        anyhow::bail!("agent node group id must be at most 256 characters");
    }
    Ok(Some(trimmed.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{
        AGENT_NODE_MAIN_ID, AGENT_NODE_MAIN_NAME, AgentNodeConfig, AgentNodeUpdate,
        build_main_agent_node_record, normalize_target_node_id, validate_agent_node_config_input,
        validate_agent_node_update_input,
    };

    #[test]
    fn normalize_target_node_id_treats_main_as_local() {
        assert_eq!(normalize_target_node_id(None), None);
        assert_eq!(normalize_target_node_id(Some("")), None);
        assert_eq!(normalize_target_node_id(Some(AGENT_NODE_MAIN_ID)), None);
        assert_eq!(
            normalize_target_node_id(Some("node-east")),
            Some("node-east".to_string())
        );
    }

    #[test]
    fn validate_agent_node_requires_encrypted_endpoint() {
        let err = validate_agent_node_config_input(&AgentNodeConfig {
            id: "node-east".to_string(),
            name: "Node East".to_string(),
            grpc_target: "http://node-east.internal:50051".to_string(),
            tls_server_name: None,
            default_worktree_root: None,
            group_id: None,
        })
        .expect_err("plain http should fail");
        assert!(err.to_string().contains("must use https://"));

        let ok = validate_agent_node_config_input(&AgentNodeConfig {
            id: "node-local".to_string(),
            name: "Node Local".to_string(),
            grpc_target: "https://node-local.internal:50051".to_string(),
            tls_server_name: Some("node-local.internal".to_string()),
            default_worktree_root: Some(" ~/.agenthub/worktrees ".to_string()),
            group_id: Some(" group-a ".to_string()),
        })
        .expect("https target should pass");
        assert_eq!(ok.id, "node-local");
        assert_eq!(ok.grpc_target, "https://node-local.internal:50051");
        assert_eq!(ok.tls_server_name.as_deref(), Some("node-local.internal"));
        assert_eq!(
            ok.default_worktree_root.as_deref(),
            Some("~/.agenthub/worktrees")
        );
        assert_eq!(ok.group_id.as_deref(), Some("group-a"));
    }

    #[test]
    fn build_main_agent_node_record_returns_reserved_main_metadata() {
        let record = build_main_agent_node_record();
        assert_eq!(record.id, AGENT_NODE_MAIN_ID);
        assert_eq!(record.name, AGENT_NODE_MAIN_NAME);
        assert!(record.grpc_target.is_none());
        assert!(record.tls_server_name.is_none());
        assert!(record.default_worktree_root.is_none());
        assert!(record.group_id.is_none());
        assert!(record.last_seen_at.is_none());
        assert!(record.is_main);
    }

    #[test]
    fn validate_agent_node_update_trims_blank_default_worktree_root() {
        let ok = validate_agent_node_update_input(&AgentNodeUpdate {
            name: "Node East".to_string(),
            grpc_target: "https://node-east.internal:50051".to_string(),
            tls_server_name: Some("  ".to_string()),
            default_worktree_root: Some("  ".to_string()),
            group_id: Some(" group-east ".to_string()),
        })
        .expect("blank optional fields should normalize");
        assert_eq!(ok.name, "Node East");
        assert_eq!(ok.grpc_target, "https://node-east.internal:50051");
        assert!(ok.tls_server_name.is_none());
        assert!(ok.default_worktree_root.is_none());
        assert_eq!(ok.group_id.as_deref(), Some("group-east"));
    }
}
