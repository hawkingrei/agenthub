//! Protocol lifecycle only. Execution and tool-binding authority remain daemon responsibilities.

use serde_json::Value;
use std::sync::Arc;

use crate::{
    McpTransportError,
    budget::{Budgeted, ByteBudget, json_bytes},
    http::{HttpContext, HttpSessionId},
    protocol::{
        MessageKind, ProtocolVersion, correlation_id, message_kind, validate_versioned_message,
    },
};

#[derive(Clone)]
pub struct McpProtocolSession {
    state: State,
    retained: ByteBudget,
}

impl Default for McpProtocolSession {
    fn default() -> Self {
        Self::new(ByteBudget::new(crate::MAX_MESSAGE_BYTES))
    }
}

#[derive(Default, Clone)]
enum State {
    #[default]
    New,
    Initializing {
        request_id: [u8; 32],
        version: ProtocolVersion,
    },
    AwaitingInitialized {
        context: HttpContext,
        capabilities: Arc<Budgeted<Value>>,
    },
    Ready {
        context: HttpContext,
        capabilities: Arc<Budgeted<Value>>,
    },
}

impl McpProtocolSession {
    pub(crate) fn new(retained: ByteBudget) -> Self {
        Self {
            state: State::New,
            retained,
        }
    }
    /// Resolve transport metadata without inventing capabilities or modifying the message.
    pub fn begin(&mut self, message: &Value) -> Result<HttpContext, McpTransportError> {
        let kind = message_kind(message)?;
        if kind == MessageKind::Batch {
            let version = match &self.state {
                State::Initializing { version, .. } => *version,
                State::AwaitingInitialized { context, .. } | State::Ready { context, .. } => {
                    context.version
                }
                State::New => return Err(McpTransportError::InvalidMessage),
            };
            validate_versioned_message(message, version)?;
            let mut next = self.clone();
            let mut context = None;
            for member in message.as_array().unwrap() {
                context = Some(next.begin(member)?);
            }
            self.state = next.state;
            return context.ok_or(McpTransportError::InvalidMessage);
        }
        let method = message.get("method").and_then(Value::as_str);
        if method == Some("initialize") {
            if kind != MessageKind::Request || !matches!(self.state, State::New) {
                return Err(McpTransportError::InvalidMessage);
            }
            let version: ProtocolVersion = message
                .pointer("/params/protocolVersion")
                .and_then(Value::as_str)
                .ok_or(McpTransportError::InvalidMessage)?
                .parse()?;
            if !version.uses_initialization() {
                return Err(McpTransportError::UnsupportedVersion);
            }
            if !message
                .pointer("/params/capabilities")
                .is_some_and(Value::is_object)
                || !valid_implementation_info(message.pointer("/params/clientInfo"))
            {
                return Err(McpTransportError::InvalidMessage);
            }
            self.state = State::Initializing {
                request_id: correlation_id(&message["id"]),
                version,
            };
            return Ok(HttpContext {
                version,
                session_id: None,
            });
        }
        let declared = message
            .get("params")
            .and_then(|params| params.get("_meta"))
            .and_then(|metadata| metadata.get("io.modelcontextprotocol/protocolVersion"))
            .and_then(Value::as_str);
        match &self.state {
            State::New => {
                let version: ProtocolVersion = declared
                    .ok_or(McpTransportError::InvalidMetadata)?
                    .parse()?;
                if version.uses_initialization() {
                    return Err(McpTransportError::InvalidMetadata);
                }
                validate_versioned_message(message, version)?;
                Ok(HttpContext {
                    version,
                    session_id: None,
                })
            }
            State::Initializing { version, .. }
                if method == Some("ping") || kind == MessageKind::Response =>
            {
                Ok(HttpContext {
                    version: *version,
                    session_id: None,
                })
            }
            State::AwaitingInitialized { context, .. }
                if method == Some("ping") || kind == MessageKind::Response =>
            {
                Ok(context.clone())
            }
            State::AwaitingInitialized {
                context,
                capabilities,
            } if method == Some("notifications/initialized")
                && kind == MessageKind::Notification =>
            {
                let context = context.clone();
                self.state = State::Ready {
                    context: context.clone(),
                    capabilities: capabilities.clone(),
                };
                Ok(context)
            }
            State::Ready { context, .. } => {
                if declared.is_some_and(|version| version != context.version.as_str()) {
                    return Err(McpTransportError::InvalidMetadata);
                }
                validate_versioned_message(message, context.version)?;
                Ok(context.clone())
            }
            _ => Err(McpTransportError::InvalidMessage),
        }
    }

    /// False means the upstream rejected initialization; its unchanged error should reach the caller.
    pub fn accept_initialize_response(
        &mut self,
        response: &Value,
        session_id: Option<HttpSessionId>,
    ) -> Result<bool, McpTransportError> {
        let State::Initializing { request_id, .. } = &self.state else {
            return Err(McpTransportError::InvalidResponse);
        };
        if message_kind(response)? != MessageKind::Response
            || correlation_id(&response["id"]) != *request_id
        {
            return Err(McpTransportError::InvalidResponse);
        }
        if response.get("error").is_some() {
            self.state = State::New;
            return Ok(false);
        }
        let result = response
            .get("result")
            .ok_or(McpTransportError::InvalidResponse)?;
        let version: ProtocolVersion = result
            .get("protocolVersion")
            .and_then(Value::as_str)
            .ok_or(McpTransportError::InvalidResponse)?
            .parse()?;
        if !version.uses_initialization() {
            return Err(McpTransportError::UnsupportedVersion);
        }
        let capabilities = result
            .get("capabilities")
            .filter(|value| value.is_object())
            .ok_or(McpTransportError::InvalidResponse)?;
        let capability_bytes = json_bytes(capabilities)?;
        let capabilities = Arc::new(
            self.retained
                .retain(capabilities.clone(), capability_bytes)?,
        );
        if !valid_implementation_info(result.get("serverInfo")) {
            return Err(McpTransportError::InvalidResponse);
        }
        self.state = State::AwaitingInitialized {
            context: HttpContext {
                version,
                session_id,
            },
            capabilities,
        };
        Ok(true)
    }

    /// Call after a failed initialize exchange or initialized notification. No tool call has begun.
    pub fn initialization_failed(&mut self) {
        self.state = State::New;
    }

    pub fn server_capabilities(&self) -> Option<&Value> {
        match &self.state {
            State::AwaitingInitialized { capabilities, .. } | State::Ready { capabilities, .. } => {
                Some(&capabilities.value)
            }
            _ => None,
        }
    }

    pub fn http_context(&self) -> Option<HttpContext> {
        match &self.state {
            State::AwaitingInitialized { context, .. } | State::Ready { context, .. } => {
                Some(context.clone())
            }
            _ => None,
        }
    }
}

fn valid_implementation_info(value: Option<&Value>) -> bool {
    value.is_some_and(|value| {
        ["name", "version"].iter().all(|field| {
            value
                .get(*field)
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty())
        })
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn initialize() -> Value {
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{"roots":{}},"clientInfo":{"name":"fixture","version":"1"}}})
    }

    #[test]
    fn legacy_handshake_accepts_upstream_version_and_capabilities_before_tools() {
        let mut session = McpProtocolSession::default();
        let tool = json!({"jsonrpc":"2.0","id":2,"method":"tools/list"});
        assert!(session.begin(&tool).is_err());
        assert_eq!(
            session.begin(&initialize()).unwrap().version,
            ProtocolVersion::November2025
        );
        assert!(session.begin(&tool).is_err());
        let capabilities =
            json!({"tools":{"listChanged":true},"experimental":{"vendor":{"unmodified":true}}});
        let response = json!({"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":capabilities,"serverInfo":{"name":"upstream","version":"7"}}});
        assert!(session.accept_initialize_response(&response, None).unwrap());
        assert_eq!(session.server_capabilities(), Some(&capabilities));
        assert!(
            session
                .begin(&json!({"jsonrpc":"2.0","id":"ping","method":"ping"}))
                .is_ok()
        );
        assert!(session.begin(&tool).is_err());
        let context = session
            .begin(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .unwrap();
        assert_eq!(context.version, ProtocolVersion::June2025);
        assert_eq!(
            session.begin(&tool).unwrap().version,
            ProtocolVersion::June2025
        );
        assert!(session.begin(&initialize()).is_err());
    }

    #[test]
    fn failed_initialization_can_restart_without_faking_an_upstream_capability() {
        let mut session = McpProtocolSession::default();
        session.begin(&initialize()).unwrap();
        assert!(!session.accept_initialize_response(&json!({"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"unsupported"}}),None).unwrap());
        assert!(session.server_capabilities().is_none());
        session.begin(&initialize()).unwrap();
        assert!(
            session
                .accept_initialize_response(
                    &json!({"jsonrpc":"2.0","id":"wrong","result":{}}),
                    None
                )
                .is_err()
        );
        session.initialization_failed();
        assert!(session.begin(&initialize()).is_ok());
    }

    #[test]
    fn modern_requests_choose_their_declared_version_without_a_fake_initialize() {
        let mut session = McpProtocolSession::default();
        let request = json!({"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}});
        assert_eq!(
            session.begin(&request).unwrap().version,
            ProtocolVersion::July2026
        );
        assert!(session.server_capabilities().is_none());
        let mut invalid = initialize();
        invalid["params"]["protocolVersion"] = "2026-07-28".into();
        assert!(session.begin(&invalid).is_err());
    }

    #[test]
    fn capability_charge_survives_initialization_and_is_shared_between_sessions() {
        let capabilities = json!({"tools":{"listChanged":true}});
        let bytes = json_bytes(&capabilities).unwrap();
        let budget = ByteBudget::new(bytes);
        let mut first = McpProtocolSession::new(budget.clone());
        let mut second = McpProtocolSession::new(budget.clone());
        let response = json!({"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25",
            "capabilities":capabilities,"serverInfo":{"name":"fixture","version":"1"}}});
        first.begin(&initialize()).unwrap();
        second.begin(&initialize()).unwrap();
        first.accept_initialize_response(&response, None).unwrap();
        first
            .begin(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .unwrap();
        assert_eq!(budget.used(), bytes);
        assert_eq!(
            second.accept_initialize_response(&response, None),
            Err(McpTransportError::Capacity)
        );
        assert_eq!(first.server_capabilities(), Some(&capabilities));
        drop(first);
        assert_eq!(budget.used(), 0);
        second.accept_initialize_response(&response, None).unwrap();
    }
}
