//! Trusted App policy and HTTP metadata. No connection or credential enters provider discovery.

use std::{path::Path, sync::Arc, time::Duration};

use agenthub_agent_domain::app_tools::{
    AppConnection, AppReplayPolicy, CompiledAppManifest, validate_app_payload,
};
use agenthub_db::app_registry::{AppActivationPin, AppRegistry};
use agenthub_mcp::{
    McpTransportError,
    access::{McpAccessPolicy, McpSelection},
    bridge::McpProxyBinding,
    http::McpHttpTransport,
    policy::{McpBinding, McpPolicyError, TrustedReplayPolicy},
};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) struct AppLaunchBinding {
    pub pin: AppActivationPin,
    pub binding: Arc<McpProxyBinding>,
    pub fingerprint: String,
}

/// Offline checks use a retained activation selection when present, including an empty one.
pub(crate) async fn validate_configuration(
    registry: &AppRegistry,
    team_id: &str,
    actor_id: &str,
    activation_id: Option<&str>,
    mut secret: impl FnMut(&str) -> Option<String>,
) -> anyhow::Result<bool> {
    let pinned = if let Some(activation) = activation_id {
        registry.activation_selection(team_id, activation).await?
    } else {
        None
    };
    let selected: Vec<_> = if let Some(pins) = pinned {
        pins.into_iter()
            .map(|pin| (pin.app_id, pin.version, pin.scopes))
            .collect()
    } else {
        registry
            .active_member_bindings(team_id, actor_id)
            .await?
            .into_iter()
            .map(|binding| (binding.app_id, binding.version, binding.scopes))
            .collect()
    };
    for (app_id, version, scopes) in &selected {
        let connection = registry.connection(app_id).await?;
        credential_headers(&connection, &mut secret)?;
        let manifest = registry
            .version(app_id, *version)
            .await?
            .ok_or_else(|| anyhow::anyhow!("pinned app manifest is unavailable"))?;
        manifest.manifest.compile()?.allowed_tools(scopes)?;
    }
    Ok(!selected.is_empty())
}

pub(crate) async fn resolve_pinned(
    registry: &AppRegistry,
    pin: AppActivationPin,
    workdir: &Path,
    secret: impl FnOnce(&str) -> Option<String>,
) -> anyhow::Result<AppLaunchBinding> {
    let connection = registry.connection(&pin.app_id).await?;
    let version = registry
        .version(&pin.app_id, pin.version)
        .await?
        .ok_or_else(|| anyhow::anyhow!("pinned app manifest is unavailable"))?;
    let manifest = Arc::new(version.manifest.compile()?);
    let allowed = manifest.allowed_tools(&pin.scopes)?;
    let workspace = std::fs::canonicalize(workdir)
        .map_err(|_| anyhow::anyhow!("app workspace is unavailable"))?;
    let workspace = digest(workspace.as_os_str().as_encoded_bytes());
    let headers = connection_headers(&connection, &pin, &workspace, secret)?;
    let revision = json!({
        "contract":1, "manifest":manifest.manifest(), "app_id":pin.app_id,
        "version":pin.version, "scopes":pin.scopes,
        "grant_epoch":pin.grant_epoch, "binding_epoch":pin.binding_epoch,
        "workspace":workspace,
    });
    let fingerprint = digest(&serde_json::to_vec(&revision)?);
    let transport = McpHttpTransport::new(&connection.endpoint, headers, Duration::from_secs(120))?;
    let replay = manifest
        .manifest()
        .tools
        .iter()
        .filter(|tool| allowed.contains(&tool.name))
        .map(|tool| {
            (
                tool.name.clone(),
                match &tool.replay {
                    AppReplayPolicy::ReadOnly => TrustedReplayPolicy::ReadOnly,
                    AppReplayPolicy::NonIdempotent => TrustedReplayPolicy::NonIdempotent,
                    AppReplayPolicy::StableIdentity { property_path } => {
                        TrustedReplayPolicy::StableIdentity {
                            property_path: property_path.clone(),
                        }
                    }
                },
            )
        })
        .collect();
    // Version and permission changes cannot disguise an earlier uncertain external effect.
    let policy = McpBinding::new(
        format!("app-{}", pin.app_id),
        &json!({"service":"registered-app", "authority":connection.authority, "namespace":connection.namespace}),
        &revision, transport, replay,
    )?.with_verified_authority();
    let arguments_manifest = manifest.clone();
    let arguments_scopes = pin.scopes.clone();
    let declaration_manifest = manifest.clone();
    let declaration_scopes = pin.scopes.clone();
    let binding = McpProxyBinding::new(
        policy,
        McpAccessPolicy {
            tools: McpSelection::Names(allowed),
            ..McpAccessPolicy::default()
        },
        Arc::new(move |name, schema, arguments| {
            arguments_manifest
                .validate_arguments(name, schema, &arguments, &arguments_scopes)
                .map_err(|_| McpPolicyError::Call)?;
            Ok(arguments)
        }),
    )
    .with_tool_declaration_validator(Arc::new(move |declaration| {
        declaration_manifest
            .validate_declaration(declaration, &declaration_scopes)
            .map_err(|_| McpPolicyError::Catalog)
    }))
    .with_tool_result_validator(Arc::new(move |name, result| {
        validate_result(&manifest, name, result).map_err(|_| McpTransportError::InvalidResponse)
    }));
    Ok(AppLaunchBinding {
        pin,
        binding: Arc::new(binding),
        fingerprint,
    })
}

fn validate_result(
    manifest: &CompiledAppManifest,
    name: &str,
    result: &Value,
) -> anyhow::Result<()> {
    validate_app_payload(result)?;
    anyhow::ensure!(result.is_object(), "invalid app tool result");
    if result.get("isError") != Some(&Value::Bool(true)) {
        manifest.validate_output(
            name,
            result.get("structuredContent").unwrap_or(&Value::Null),
        )?;
    }
    Ok(())
}

fn connection_headers(
    connection: &AppConnection,
    pin: &AppActivationPin,
    workspace: &str,
    secret: impl FnOnce(&str) -> Option<String>,
) -> anyhow::Result<HeaderMap> {
    let mut headers = credential_headers(connection, secret)?;
    for (name, value) in [
        ("x-agenthub-app-id", pin.app_id.as_str()),
        ("x-agenthub-app-version", &pin.version.to_string()),
        ("x-agenthub-team-id", pin.team_id.as_str()),
        ("x-agenthub-actor-id", pin.actor_id.as_str()),
        ("x-agenthub-activation-id", pin.activation_id.as_str()),
        ("x-agenthub-workspace", workspace),
    ] {
        headers.insert(
            name,
            HeaderValue::from_str(value)
                .map_err(|_| anyhow::anyhow!("invalid app execution metadata"))?,
        );
    }
    for value in headers.values_mut() {
        value.set_sensitive(true);
    }
    Ok(headers)
}

fn credential_headers(
    connection: &AppConnection,
    secret: impl FnOnce(&str) -> Option<String>,
) -> anyhow::Result<HeaderMap> {
    connection.validate()?;
    let mut headers = HeaderMap::new();
    if let Some(reference) = &connection.credential_env {
        let key =
            secret(reference).ok_or_else(|| anyhow::anyhow!("app credential is unavailable"))?;
        anyhow::ensure!(
            !key.is_empty() && key.len() <= 8192 && key.bytes().all(|byte| byte.is_ascii_graphic()),
            "invalid app credential"
        );
        let mut value = HeaderValue::from_str(&format!("Bearer {key}"))
            .map_err(|_| anyhow::anyhow!("invalid app credential"))?;
        value.set_sensitive(true);
        headers.insert(AUTHORIZATION, value);
    }
    Ok(headers)
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests;
