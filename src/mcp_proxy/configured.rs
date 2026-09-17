//! Resolve existing Mem profiles inside the daemon. No secret-bearing type implements Debug.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use agenthub_config::{AppConfig, ResolvedNowledgeMemBinding};
use agenthub_mcp::{
    bridge::McpProxyBinding,
    http::McpHttpTransport,
    policy::{McpBinding, McpPolicyError},
};
use reqwest::{
    Url,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde_json::json;
use sha2::{Digest, Sha256};

mod authorization;

pub(crate) struct ConfiguredMcpBinding {
    pub binding: Arc<McpProxyBinding>,
    pub fingerprint: String,
}

pub(crate) fn shim_executable() -> anyhow::Result<std::path::PathBuf> {
    #[cfg(test)]
    {
        crate::agenthub_binary::resolve_agenthub_binary_path()
            .ok_or_else(|| anyhow::anyhow!("MCP shim executable is unavailable"))
    }
    #[cfg(not(test))]
    {
        std::env::current_exe().map_err(|_| anyhow::anyhow!("MCP shim executable is unavailable"))
    }
}

pub(crate) fn has_mem_binding(config: &AppConfig, team_id: &str) -> bool {
    config
        .nowledge_mem
        .as_ref()
        .and_then(|mem| mem.team_bindings.as_ref())
        .is_some_and(|bindings| bindings.contains_key(team_id))
}

struct MemConnection {
    endpoint: Url,
    headers: HeaderMap,
    resolved: ResolvedNowledgeMemBinding,
}

fn resolve_connection(
    config: &AppConfig,
    team_id: &str,
    actor_id: &str,
    secret: impl FnOnce(&str) -> Option<String>,
) -> anyhow::Result<MemConnection> {
    let resolved = config
        .resolve_nowledge_mem_binding(team_id, actor_id)
        .map_err(|_| anyhow::anyhow!("Mem binding configuration is invalid"))?;
    connect_profile(resolved, secret)
}

fn connect_profile(
    resolved: ResolvedNowledgeMemBinding,
    secret: impl FnOnce(&str) -> Option<String>,
) -> anyhow::Result<MemConnection> {
    let reference = &resolved.profile.credential_env;
    anyhow::ensure!(
        valid_credential_reference(reference),
        "Mem credential reference is invalid"
    );
    let key = secret(reference).ok_or_else(|| anyhow::anyhow!("Mem credential is unavailable"))?;
    anyhow::ensure!(
        !key.is_empty() && key.len() <= 8192 && key.bytes().all(|b| b.is_ascii_graphic()),
        "Mem credential is invalid"
    );
    let mut endpoint = Url::parse(&resolved.profile.endpoint)
        .map_err(|_| anyhow::anyhow!("Mem endpoint is invalid"))?;
    let local = endpoint.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    anyhow::ensure!(
        endpoint.host_str().is_some()
            && endpoint.username().is_empty()
            && endpoint.password().is_none()
            && endpoint.query().is_none()
            && endpoint.fragment().is_none()
            && (endpoint.scheme() == "https" || endpoint.scheme() == "http" && local),
        "Mem endpoint is invalid"
    );
    // Mem's canonical endpoint has no trailing slash. Credentials belong in headers, never URLs.
    let path = endpoint.path().trim_end_matches('/').to_owned();
    anyhow::ensure!(path.ends_with("/mcp"), "Mem endpoint must end with /mcp");
    endpoint.set_path(&path);
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {key}"))
            .map_err(|_| anyhow::anyhow!("Mem credential is invalid"))?,
    );
    if let Some(tool_set) = &resolved.profile.tool_set {
        anyhow::ensure!(
            !tool_set.is_empty()
                && tool_set.len() <= 128
                && tool_set
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b)),
            "Mem tool set is invalid"
        );
        headers.insert(
            "x-nmem-tool-set",
            HeaderValue::from_str(tool_set)
                .map_err(|_| anyhow::anyhow!("Mem tool set is invalid"))?,
        );
    }
    for value in headers.values_mut() {
        value.set_sensitive(true);
    }
    Ok(MemConnection {
        endpoint,
        headers,
        resolved,
    })
}

fn resolve_profile_connections(
    config: &AppConfig,
    team_id: &str,
    actor_id: &str,
    mut secret: impl FnMut(&str) -> Option<String>,
) -> anyhow::Result<(MemConnection, Option<MemConnection>)> {
    let actor = resolve_connection(config, team_id, actor_id, &mut secret)?;
    let team_profile = config
        .nowledge_mem
        .as_ref()
        .and_then(|mem| mem.team_bindings.as_ref())
        .and_then(|bindings| bindings.get(team_id.trim()))
        .ok_or_else(|| anyhow::anyhow!("Mem Team binding is unavailable"))?
        .profile
        .trim();
    let team = if team_profile == actor.resolved.profile_name {
        None
    } else {
        Some(connect_profile(
            ResolvedNowledgeMemBinding {
                profile_name: team_profile.to_owned(),
                profile: config
                    .nowledge_mem_profile(team_profile)
                    .map_err(|_| anyhow::anyhow!("Mem Team profile is invalid"))?,
                space_id: actor.resolved.space_id.clone(),
            },
            secret,
        )?)
    };
    Ok((actor, team))
}

/// Offline preflight checks configuration and credential availability without contacting Mem.
pub(crate) fn validate_mem_configuration(
    config: &AppConfig,
    team_id: &str,
    actor_id: &str,
    secret: impl FnMut(&str) -> Option<String>,
) -> anyhow::Result<()> {
    resolve_profile_connections(config, team_id, actor_id, secret).map(|_| ())
}

pub(crate) async fn resolve_mem(
    config: &AppConfig,
    team_id: &str,
    actor_id: &str,
    secret: impl FnMut(&str) -> Option<String>,
) -> anyhow::Result<ConfiguredMcpBinding> {
    let (
        MemConnection {
            endpoint,
            headers,
            resolved,
        },
        team,
    ) = resolve_profile_connections(config, team_id, actor_id, secret)?;
    let workspace = authorization::verify(&endpoint, &headers, &resolved.space_id).await?;
    if let Some(team) = &team {
        let team_workspace =
            authorization::verify(&team.endpoint, &team.headers, &resolved.space_id).await?;
        anyhow::ensure!(
            workspace == team_workspace,
            "Mem actor profile does not match the Team workspace"
        );
    }
    let team_reference = team.as_ref().map(|team| {
        json!({"profile":team.resolved.profile_name,
        "endpoint":team.endpoint.as_str(),"credential_ref":team.resolved.profile.credential_env})
    });
    let revision = json!({"version":5, "access_policy":"scoped-key-v1", "endpoint":endpoint.as_str(), "profile":resolved.profile_name,
        "credential_ref":resolved.profile.credential_env, "space_id":resolved.space_id,
        "workspace_id":workspace, "team_reference":team_reference, "tool_set":resolved.profile.tool_set});
    let fingerprint = Sha256::digest(serde_json::to_vec(&revision)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let transport = McpHttpTransport::new(endpoint.as_str(), headers, Duration::from_secs(120))?;
    let policy = McpBinding::new(
        "nowledge-mem".into(),
        &json!({"service":"nowledge-mem", "workspace_id":workspace, "space_id":resolved.space_id}),
        &revision,
        transport,
        BTreeMap::new(),
    )?
    .with_verified_authority();
    let scope = agenthub_acp_core::nowledge_mem::MemScopeBinding::new(
        resolved.profile_name,
        resolved.space_id,
    )?;
    let binding = Arc::new(McpProxyBinding::new(
        policy,
        agenthub_mcp::access::McpAccessPolicy {
            resources: agenthub_mcp::access::McpSelection::All,
            resource_templates: agenthub_mcp::access::McpSelection::All,
            prompts: agenthub_mcp::access::McpSelection::All,
            logging: true,
            ..agenthub_mcp::access::McpAccessPolicy::tools_only()
        },
        Arc::new(move |_, schema, arguments| {
            agenthub_acp_core::nowledge_mem::bind_declared_space_id(schema, arguments, &scope)
                .map_err(|_| McpPolicyError::Scope)
        }),
    ));
    Ok(ConfiguredMcpBinding {
        binding,
        fingerprint,
    })
}

fn valid_credential_reference(reference: &str) -> bool {
    !reference.is_empty()
        && reference.len() <= 128
        && reference
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || i > 0 && b.is_ascii_digit())
        && !reference.starts_with("AGENTHUB_")
        && !matches!(
            reference,
            "HOME" | "PATH" | "USER" | "SHELL" | "TMPDIR" | "TMP" | "TEMP"
        )
}

pub(crate) fn private_environment(config: &AppConfig) -> Vec<String> {
    let mut names: Vec<_> = config
        .nowledge_mem
        .as_ref()
        .and_then(|mem| mem.profiles.as_ref())
        .into_iter()
        .flat_map(|profiles| profiles.values())
        .map(|profile| profile.credential_env.clone())
        .filter(|name| valid_credential_reference(name))
        .collect();
    names.sort();
    names.dedup();
    names
}

pub(crate) fn is_private_environment(name: &str, configured: &[String]) -> bool {
    let upper = name.to_ascii_uppercase();
    configured.iter().any(|key| key == name)
        || upper.starts_with("NMEM_")
        || upper.starts_with("NOWLEDGE_MEM_")
        || matches!(
            upper.as_str(),
            "MCP_HEADERS" | "MCP_HTTP_HEADERS" | "MCP_SERVER_HEADERS"
        )
}

#[cfg(test)]
mod tests;
