//! Trusted integration permissions. Discovery and protocol metadata cannot expand these grants.

use std::collections::BTreeSet;

use serde_json::Value;

#[cfg(test)]
mod tests;

use crate::{
    McpTransportError,
    policy::McpPolicyError,
    protocol::{MessageKind, message_kind},
};

#[derive(Clone, Debug, Default)]
pub enum McpSelection {
    #[default]
    None,
    All,
    Names(BTreeSet<String>),
}

impl McpSelection {
    pub fn allows(&self, name: &str) -> bool {
        match self {
            Self::None => false,
            Self::All => true,
            Self::Names(names) => names.contains(name),
        }
    }

    fn any(&self) -> bool {
        match self {
            Self::None => false,
            Self::All => true,
            Self::Names(names) => !names.is_empty(),
        }
    }
}

/// Constructed by the integration, never deserialized from provider messages. Exact resource
/// and template identifiers are independent grants; a template is not a URI-prefix wildcard.
#[derive(Clone, Debug, Default)]
pub struct McpAccessPolicy {
    pub tools: McpSelection,
    pub resources: McpSelection,
    pub resource_templates: McpSelection,
    pub prompts: McpSelection,
    pub callbacks: McpSelection,
    pub logging: bool,
}

impl McpAccessPolicy {
    /// Use only when the integration has independently established authority for the entire
    /// upstream surface. An upstream capability declaration does not establish that authority.
    pub fn unrestricted() -> Self {
        Self {
            tools: McpSelection::All,
            resources: McpSelection::All,
            resource_templates: McpSelection::All,
            prompts: McpSelection::All,
            callbacks: McpSelection::All,
            logging: true,
        }
    }

    /// Tool argument binding and upstream namespace enforcement remain integration obligations.
    pub fn tools_only() -> Self {
        Self {
            tools: McpSelection::All,
            callbacks: McpSelection::Names(
                ["roots/list", "sampling/createMessage", "elicitation/create"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            ),
            ..Self::default()
        }
    }

    pub(crate) fn authorize_request(&self, message: &Value) -> Result<(), McpPolicyError> {
        if message_kind(message)? == MessageKind::Response {
            // The session must additionally match a previously authorized callback ID.
            return Ok(());
        }
        let params = &message["params"];
        let allowed = match message["method"].as_str() {
            Some(
                "initialize"
                | "server/discover"
                | "ping"
                | "notifications/initialized"
                | "notifications/cancelled"
                | "notifications/progress"
                | "notifications/roots/list_changed",
            ) => true,
            Some("tools/list" | "tasks/get" | "tasks/result" | "tasks/cancel" | "tasks/update") => {
                self.tools.any()
            }
            Some("tools/call") => allows_field(&self.tools, params, "name"),
            Some("resources/list") => self.resources.any(),
            Some("resources/templates/list") => self.resource_templates.any(),
            Some("resources/read" | "resources/subscribe" | "resources/unsubscribe") => {
                allows_field(&self.resources, params, "uri")
            }
            Some("prompts/list") => self.prompts.any(),
            Some("prompts/get") => allows_field(&self.prompts, params, "name"),
            Some("completion/complete") => match params["ref"]["type"].as_str() {
                Some("ref/prompt") => allows_field(&self.prompts, &params["ref"], "name"),
                Some("ref/resource") => {
                    allows_field(&self.resource_templates, &params["ref"], "uri")
                }
                _ => false,
            },
            Some("logging/setLevel") => self.logging,
            Some("subscriptions/listen") => self.authorize_filters(&params["notifications"]),
            _ => false,
        };
        if allowed {
            Ok(())
        } else {
            Err(McpPolicyError::Scope)
        }
    }

    fn authorize_filters(&self, filters: &Value) -> bool {
        filters.as_object().is_some_and(|filters| {
            filters.iter().all(|(name, value)| match name.as_str() {
                "toolsListChanged" => value == false || value == true && self.tools.any(),
                "resourcesListChanged" => {
                    value == false
                        || value == true && (self.resources.any() || self.resource_templates.any())
                }
                "promptsListChanged" => value == false || value == true && self.prompts.any(),
                "resourceSubscriptions" => value.as_array().is_some_and(|uris| {
                    uris.iter()
                        .all(|uri| uri.as_str().is_some_and(|uri| self.resources.allows(uri)))
                }),
                "taskIds" => value
                    .as_array()
                    .is_some_and(|ids| ids.is_empty() || self.tools.any()),
                _ => false,
            })
        })
    }

    pub(crate) fn authorize_server_message(
        &self,
        message: &Value,
    ) -> Result<(), McpTransportError> {
        let kind = message_kind(message)?;
        let result = &message["result"];
        if kind == MessageKind::Response
            && (result["resultType"] == "input_required"
                || result["status"] == "input_required" && result["taskId"].is_string())
        {
            self.authorize_input_requests(result)?;
        }
        if message["method"] == "notifications/tasks"
            && message["params"]["status"] == "input_required"
        {
            self.authorize_input_requests(&message["params"])?;
        }
        let allowed = if kind == MessageKind::Request {
            message["method"] == "ping"
                || message["method"]
                    .as_str()
                    .is_some_and(|method| self.callbacks.allows(method))
        } else {
            match message["method"].as_str() {
                Some("notifications/resources/updated") => {
                    allows_field(&self.resources, &message["params"], "uri")
                }
                Some("notifications/resources/list_changed") => {
                    self.resources.any() || self.resource_templates.any()
                }
                Some("notifications/prompts/list_changed") => self.prompts.any(),
                Some("notifications/tools/list_changed") => self.tools.any(),
                _ => true,
            }
        };
        if allowed {
            Ok(())
        } else {
            Err(McpTransportError::InvalidResponse)
        }
    }

    fn authorize_input_requests(&self, envelope: &Value) -> Result<(), McpTransportError> {
        let Some(requests) = envelope.get("inputRequests") else {
            return Ok(());
        };
        let requests = requests
            .as_object()
            .ok_or(McpTransportError::InvalidResponse)?;
        if requests.len() > 64
            || requests.values().any(|request| {
                !crate::continuation::valid_input_request(request)
                    || !allows_field(&self.callbacks, request, "method")
            })
        {
            return Err(McpTransportError::InvalidResponse);
        }
        Ok(())
    }

    pub(crate) fn filter_tools(&self, tools: &mut Value) -> Result<(), McpTransportError> {
        filter_items(tools, "name", &self.tools)
    }

    /// Preserve errors and every field of permitted items. Only discovery visibility changes.
    pub(crate) fn project_response(
        &self,
        method: &str,
        response: &mut Value,
    ) -> Result<(), McpTransportError> {
        if response.get("error").is_some() {
            return Ok(());
        }
        let result = &mut response["result"];
        let deferred = matches!(
            result["resultType"].as_str(),
            Some("input_required" | "task")
        ) || result.get("task").is_some_and(Value::is_object);
        match method {
            "initialize" | "server/discover" => {
                if let Some(capabilities) = result
                    .get_mut("capabilities")
                    .and_then(Value::as_object_mut)
                {
                    for (name, permitted) in [
                        ("tools", self.tools.any()),
                        (
                            "resources",
                            self.resources.any() || self.resource_templates.any(),
                        ),
                        ("prompts", self.prompts.any()),
                        ("logging", self.logging),
                        (
                            "completions",
                            self.prompts.any() || self.resource_templates.any(),
                        ),
                        ("tasks", self.tools.any()),
                    ] {
                        if !permitted {
                            capabilities.remove(name);
                        }
                    }
                }
            }
            "resources/list" if !deferred || result.get("resources").is_some() => {
                filter_items(&mut result["resources"], "uri", &self.resources)?
            }
            "resources/templates/list"
                if !deferred || result.get("resourceTemplates").is_some() =>
            {
                filter_items(
                    &mut result["resourceTemplates"],
                    "uriTemplate",
                    &self.resource_templates,
                )?
            }
            "prompts/list" if !deferred || result.get("prompts").is_some() => {
                filter_items(&mut result["prompts"], "name", &self.prompts)?
            }
            "resources/read" if !deferred || result.get("contents").is_some() => {
                let contents = result["contents"]
                    .as_array()
                    .ok_or(McpTransportError::InvalidResponse)?;
                if !contents
                    .iter()
                    .all(|item| allows_field(&self.resources, item, "uri"))
                {
                    return Err(McpTransportError::InvalidResponse);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn allows_field(selection: &McpSelection, value: &Value, field: &str) -> bool {
    value[field]
        .as_str()
        .is_some_and(|name| selection.allows(name))
}

fn filter_items(
    value: &mut Value,
    key: &str,
    selection: &McpSelection,
) -> Result<(), McpTransportError> {
    let items = value
        .as_array_mut()
        .ok_or(McpTransportError::InvalidResponse)?;
    if items.iter().any(|item| item[key].as_str().is_none()) {
        return Err(McpTransportError::InvalidResponse);
    }
    items.retain(|item| allows_field(selection, item, key));
    Ok(())
}
