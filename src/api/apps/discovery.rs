use agenthub_db::app_registry::AppRegistry;
use serde::Serialize;

use super::{ApiError, store_error};

#[derive(Debug, Serialize)]
pub struct AppCapability {
    pub app_id: String,
    pub name: String,
    pub version: i64,
    pub tools: Vec<String>,
}

/// The caller must authorize Team inspection. This describes configuration, not runtime authority.
pub(in crate::api) async fn member_capabilities(
    registry: &AppRegistry,
    team_id: &str,
    actor_id: &str,
) -> Result<Vec<AppCapability>, ApiError> {
    let mut capabilities = Vec::new();
    for binding in registry
        .active_member_bindings(team_id, actor_id)
        .await
        .map_err(store_error)?
    {
        let Some(app) = registry.app(&binding.app_id).await.map_err(store_error)? else {
            continue;
        };
        if app.revoked_at.is_some() {
            continue;
        }
        let version = registry
            .version(&binding.app_id, binding.version)
            .await
            .map_err(store_error)?
            .ok_or_else(|| ApiError::conflict("app manifest is unavailable"))?;
        let tools = version
            .manifest
            .compile()
            .and_then(|manifest| manifest.allowed_tools(&binding.scopes))
            .map_err(store_error)?;
        capabilities.push(AppCapability {
            app_id: app.id,
            name: app.name,
            version: binding.version,
            tools: tools.into_iter().collect(),
        });
    }
    Ok(capabilities)
}
