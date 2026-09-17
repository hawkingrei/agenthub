//! A bounded projection of client support, independent of integration access grants.

use serde_json::Value;

use crate::{
    McpTransportError,
    protocol::{MessageKind, ProtocolVersion, message_kind},
};

#[derive(Clone, Copy, Default)]
pub(crate) struct ClientCapabilities {
    roots: bool,
    sampling: bool,
    sampling_tools: bool,
    elicitation_form: bool,
    elicitation_url: bool,
    log_level: Option<u8>,
}

impl ClientCapabilities {
    pub(crate) fn from_value(value: &Value) -> Self {
        Self {
            roots: value["roots"].is_object(),
            sampling: value["sampling"].is_object(),
            sampling_tools: value["sampling"]["tools"].is_object(),
            // Empty elicitation capabilities retain the specified form-only compatibility.
            elicitation_form: value["elicitation"].as_object().is_some_and(|modes| {
                modes.is_empty() || modes.get("form").is_some_and(Value::is_object)
            }),
            elicitation_url: value["elicitation"]["url"].is_object(),
            // Legacy logging has no per-request opt-in. Its server controls the level.
            log_level: Some(0),
        }
    }

    pub(crate) fn from_request(request: &Value) -> Self {
        Self {
            log_level: log_level(&request["params"]["_meta"]["io.modelcontextprotocol/logLevel"]),
            ..Self::from_value(
                &request["params"]["_meta"]["io.modelcontextprotocol/clientCapabilities"],
            )
        }
    }

    pub(crate) fn authorize_message(self, message: &Value) -> Result<(), McpTransportError> {
        let kind = message_kind(message)?;
        if kind == MessageKind::Request {
            self.authorize_input(message)?;
        }
        let result = &message["result"];
        if kind == MessageKind::Response
            && (result["resultType"] == "input_required"
                || result["status"] == "input_required" && result["taskId"].is_string())
        {
            self.authorize_inputs(result)?;
        }
        if message["method"] == "notifications/tasks"
            && message["params"]["status"] == "input_required"
        {
            self.authorize_inputs(&message["params"])?;
        }
        if message["method"] == "notifications/message"
            && self
                .log_level
                .zip(log_level(&message["params"]["level"]))
                .is_none_or(|(minimum, observed)| observed < minimum)
        {
            return Err(McpTransportError::InvalidResponse);
        }
        Ok(())
    }

    fn authorize_inputs(self, envelope: &Value) -> Result<(), McpTransportError> {
        if let Some(inputs) = envelope.get("inputRequests") {
            let inputs = inputs
                .as_object()
                .ok_or(McpTransportError::InvalidResponse)?;
            for input in inputs.values() {
                self.authorize_input(input)?;
            }
        }
        Ok(())
    }

    fn authorize_input(self, request: &Value) -> Result<(), McpTransportError> {
        let supported = match request["method"].as_str() {
            Some("roots/list") => self.roots,
            Some("sampling/createMessage") => {
                self.sampling
                    && (request["params"].get("tools").is_none()
                        && request["params"].get("toolChoice").is_none()
                        || self.sampling_tools)
            }
            Some("elicitation/create") => match request["params"].get("mode") {
                None => self.elicitation_form,
                Some(mode) if mode == "form" => self.elicitation_form,
                Some(mode) if mode == "url" => self.elicitation_url,
                _ => false,
            },
            // Ping needs no capability. Unknown extension methods remain subject to integration
            // grants; do not infer their semantics from a vendor method or capability name.
            _ => true,
        };
        if supported {
            Ok(())
        } else {
            Err(McpTransportError::InvalidResponse)
        }
    }
}

fn log_level(value: &Value) -> Option<u8> {
    [
        "debug",
        "info",
        "notice",
        "warning",
        "error",
        "critical",
        "alert",
        "emergency",
    ]
    .iter()
    .position(|level| value == *level)
    .map(|index| index as u8)
}

pub(crate) fn validate_client_method(
    message: &Value,
    version: ProtocolVersion,
) -> Result<(), McpTransportError> {
    let method = message["method"].as_str().unwrap_or("");
    let unsupported = if version == ProtocolVersion::July2026 {
        matches!(
            method,
            "ping"
                | "initialize"
                | "notifications/initialized"
                | "notifications/progress"
                | "notifications/roots/list_changed"
                | "logging/setLevel"
                | "resources/subscribe"
                | "resources/unsubscribe"
                | "tasks/result"
        )
    } else {
        matches!(
            method,
            "server/discover" | "subscriptions/listen" | "tasks/update"
        )
    };
    if unsupported {
        return Err(McpTransportError::InvalidMessage);
    }
    if version == ProtocolVersion::July2026
        && let Some(level) = message.pointer("/params/_meta/io.modelcontextprotocol~1logLevel")
        && log_level(level).is_none()
    {
        return Err(McpTransportError::InvalidMetadata);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
