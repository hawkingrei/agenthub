use url::Url;

use super::{AgentNodeConfig, AgentNodeRecord};

pub(crate) const AGENT_NODE_MAIN_ID: &str = "main";
pub(crate) const AGENT_NODE_MAIN_NAME: &str = "Main Node";

pub(crate) fn build_main_agent_node_record() -> AgentNodeRecord {
    AgentNodeRecord {
        id: AGENT_NODE_MAIN_ID.to_string(),
        name: AGENT_NODE_MAIN_NAME.to_string(),
        grpc_target: None,
        tls_server_name: None,
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
) -> anyhow::Result<(String, String, String, Option<String>)> {
    let id = validate_agent_node_id(&config.id)?;
    let name = validate_agent_node_name(&config.name)?;
    let grpc_target = validate_agent_node_grpc_target(&config.grpc_target)?;
    let tls_server_name = config
        .tls_server_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Ok((id, name, grpc_target, tls_server_name))
}

#[cfg(test)]
mod tests {
    use super::{AGENT_NODE_MAIN_ID, normalize_target_node_id, validate_agent_node_config_input};
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
        })
        .expect_err("plain http should fail");
        assert!(err.to_string().contains("must use https://"));

        let ok = validate_agent_node_config_input(&AgentNodeConfig {
            id: "node-local".to_string(),
            name: "Node Local".to_string(),
            grpc_target: "https://node-local.internal:50051".to_string(),
            tls_server_name: Some("node-local.internal".to_string()),
        })
        .expect("https target should pass");
        assert_eq!(ok.0, "node-local");
        assert_eq!(ok.2, "https://node-local.internal:50051");
        assert_eq!(ok.3.as_deref(), Some("node-local.internal"));
    }

    #[test]
    fn build_remote_agent_node_route_emits_grpc_metadata() {
        let route = serde_json::json!({
            "kind": "grpc",
            "grpc_target": "https://node.example.com:50051",
            "tls_server_name": "node.example.com",
        });
        assert_eq!(route["kind"], "grpc");
        assert_eq!(route["grpc_target"], "https://node.example.com:50051");
        assert_eq!(route["tls_server_name"], "node.example.com");
    }
}
