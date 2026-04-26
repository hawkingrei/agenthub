use url::Url;

use super::{AgentNodeConfig, AgentNodeRecord, AgentNodeUpdate};

pub(crate) const AGENT_NODE_MAIN_ID: &str = "main";
pub(crate) const AGENT_NODE_MAIN_NAME: &str = "Main Node";

type ValidatedAgentNodeConfig = (String, String, String, Option<String>, Option<String>);

type ValidatedAgentNodeUpdate = (String, String, Option<String>, Option<String>);

pub(crate) fn build_main_agent_node_record() -> AgentNodeRecord {
    AgentNodeRecord {
        id: AGENT_NODE_MAIN_ID.to_string(),
        name: AGENT_NODE_MAIN_NAME.to_string(),
        grpc_target: None,
        tls_server_name: None,
        default_worktree_root: None,
        last_seen_at: None,
        is_main: true,
        created_at: 0,
        updated_at: 0,
    }
}

pub(crate) fn normalize_target_node_id(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty() && *value != AGENT_NODE_MAIN_ID)
        .map(str::to_string)
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

pub(crate) fn validate_agent_node_config_input(
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
    Ok((
        id,
        name,
        grpc_target,
        tls_server_name,
        default_worktree_root,
    ))
}

pub(crate) fn validate_agent_node_update_input(
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
    Ok((name, grpc_target, tls_server_name, default_worktree_root))
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

#[cfg(test)]
mod tests {
    use super::{
        AGENT_NODE_MAIN_ID, AGENT_NODE_MAIN_NAME, build_main_agent_node_record,
        normalize_target_node_id, validate_agent_node_config_input,
        validate_agent_node_update_input,
    };
    use crate::agent::AgentNodeConfig;

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
        })
        .expect_err("plain http should fail");
        assert!(err.to_string().contains("must use https://"));

        let ok = validate_agent_node_config_input(&AgentNodeConfig {
            id: "node-local".to_string(),
            name: "Node Local".to_string(),
            grpc_target: "https://node-local.internal:50051".to_string(),
            tls_server_name: Some("node-local.internal".to_string()),
            default_worktree_root: Some(" ~/.agenthub/worktrees ".to_string()),
        })
        .expect("https target should pass");
        assert_eq!(ok.0, "node-local");
        assert_eq!(ok.2, "https://node-local.internal:50051");
        assert_eq!(ok.3.as_deref(), Some("node-local.internal"));
        assert_eq!(ok.4.as_deref(), Some("~/.agenthub/worktrees"));
    }

    #[test]
    fn build_main_agent_node_record_returns_reserved_main_metadata() {
        let record = build_main_agent_node_record();
        assert_eq!(record.id, AGENT_NODE_MAIN_ID);
        assert_eq!(record.name, AGENT_NODE_MAIN_NAME);
        assert!(record.grpc_target.is_none());
        assert!(record.tls_server_name.is_none());
        assert!(record.default_worktree_root.is_none());
        assert!(record.last_seen_at.is_none());
        assert!(record.is_main);
    }

    #[test]
    fn validate_agent_node_update_trims_blank_default_worktree_root() {
        let ok = validate_agent_node_update_input(&crate::agent::AgentNodeUpdate {
            name: "Node East".to_string(),
            grpc_target: "https://node-east.internal:50051".to_string(),
            tls_server_name: Some("  ".to_string()),
            default_worktree_root: Some("  ".to_string()),
        })
        .expect("blank optional fields should normalize");
        assert_eq!(ok.0, "Node East");
        assert_eq!(ok.1, "https://node-east.internal:50051");
        assert!(ok.2.is_none());
        assert!(ok.3.is_none());
    }
}
