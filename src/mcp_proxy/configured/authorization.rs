//! Verify existing Mem key narrowing without treating a routing header as authorization.

use std::time::Duration;

use reqwest::{Client, Url, header::HeaderMap};
use serde_json::Value;

const MAX_AUTHORIZATION_BYTES: usize = 65_536;

/// The configured endpoint and this sibling route must share the same trusted Mem deployment.
/// Use exactly the credential retained for MCP; never expose the membership response to a provider.
pub(super) async fn verify(
    endpoint: &Url,
    headers: &HeaderMap,
    space: &str,
) -> anyhow::Result<String> {
    let mut address = endpoint.clone();
    let prefix = address
        .path()
        .strip_suffix("/mcp")
        .ok_or_else(|| anyhow::anyhow!("Mem authorization endpoint is invalid"))?;
    address.set_path(&format!("{prefix}/members/me"));
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .referer(false)
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| anyhow::anyhow!("Mem authorization client is unavailable"))?;
    let mut response = client
        .get(address)
        .headers(headers.clone())
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("Mem authorization check is unavailable"))?;
    anyhow::ensure!(
        response.status().is_success(),
        "Mem credential scope could not be verified"
    );
    anyhow::ensure!(
        response
            .content_length()
            .is_none_or(|length| length <= MAX_AUTHORIZATION_BYTES as u64),
        "Mem authorization response exceeds its limit"
    );
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| anyhow::anyhow!("Mem authorization response is incomplete"))?
    {
        anyhow::ensure!(
            chunk.len() <= MAX_AUTHORIZATION_BYTES - body.len(),
            "Mem authorization response exceeds its limit"
        );
        body.extend_from_slice(&chunk);
    }
    let response: Value = serde_json::from_slice(&body)
        .map_err(|_| anyhow::anyhow!("Mem authorization response is invalid"))?;
    validate(&response, space)
}

fn validate(response: &Value, space: &str) -> anyhow::Result<String> {
    let workspace = response["workspace_id"]
        .as_str()
        .and_then(|id| uuid::Uuid::parse_str(id).ok())
        .filter(|id| !id.is_nil())
        .ok_or_else(|| anyhow::anyhow!("Mem authorization workspace is invalid"))?;
    let scope = &response["key_scope"];
    anyhow::ensure!(
        scope["scope_mode"] == "narrowed"
            && scope["grants"]
                .as_array()
                .is_some_and(|grants| grants.len() == 1 && grants[0].as_str() == Some(space)),
        "Mem credential must grant only the bound Team space"
    );
    // Narrowed-key placement is immutable upstream. A null mint-time value is valid when
    // the effective target is the member's personal space, so inspect the resolved target too.
    anyhow::ensure!(
        (scope["write_space"].is_null() || scope["write_space"].as_str() == Some(space))
            && response["key_write_target"]["write_space"].as_str() == Some(space)
            && response["key_write_target"]["write_space_live"] == true,
        "Mem credential has no active write target in the bound Team space"
    );
    Ok(workspace.to_string())
}

#[cfg(test)]
mod tests;
