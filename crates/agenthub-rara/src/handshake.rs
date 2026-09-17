use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::protocol::{ProtocolError, validate_id, validate_label};
use crate::{PROTOCOL_VERSION, TRANSPORT};

const REQUIRED_METHODS: &[&str] = &[
    "session.create",
    "session.query_state",
    "session.cancel",
    "session.interrupt",
    "input.submit_prompt",
    "input.submit_follow_up",
    "input.answer_user",
    "input.answer_plan",
    "input.answer_shell",
    "server.shutdown",
];
const REQUIRED_EVENTS: &[&str] = &[
    "session",
    "input",
    "assistant",
    "tool",
    "approval",
    "plan",
    "warning",
    "error",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handshake {
    pub protocol_version: u32,
    pub runtime_version: String,
    pub runtime_id: String,
    pub transport: String,
    pub request_families: Vec<String>,
    pub request_methods: Vec<String>,
    pub event_families: Vec<String>,
    pub capabilities: Capabilities,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub graceful_shutdown: bool,
    pub approval_persistence: bool,
    pub replay: ReplayCapability,
    pub request_receipts: ReceiptCapability,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "lifetime", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReplayCapability {
    Unavailable,
    Runtime { max_events_per_session: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "lifetime", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReceiptCapability {
    Runtime { max_requests: u32 },
}

impl Handshake {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.protocol_version != PROTOCOL_VERSION || self.transport != TRANSPORT {
            return Err(ProtocolError::UnsupportedHandshake);
        }
        validate_id(&self.runtime_id)?;
        validate_label(&self.runtime_version)?;
        for label in [self.provider.as_ref(), self.model.as_ref()]
            .into_iter()
            .flatten()
        {
            validate_label(label)?;
        }
        for entries in [
            &self.request_families,
            &self.request_methods,
            &self.event_families,
        ] {
            if entries.is_empty()
                || entries.len() > 64
                || entries.iter().collect::<BTreeSet<_>>().len() != entries.len()
            {
                return Err(ProtocolError::InvalidCapabilities);
            }
            for entry in entries {
                validate_id(entry).map_err(|_| ProtocolError::InvalidCapabilities)?;
            }
        }
        let mut families = BTreeSet::new();
        for method in &self.request_methods {
            let Some((family, operation)) = method.split_once('.') else {
                return Err(ProtocolError::InvalidCapabilities);
            };
            if family.is_empty() || operation.is_empty() {
                return Err(ProtocolError::InvalidCapabilities);
            }
            families.insert(family);
        }
        if families != self.request_families.iter().map(String::as_str).collect() {
            return Err(ProtocolError::InvalidCapabilities);
        }
        self.require_methods(REQUIRED_METHODS)?;
        if !self.capabilities.graceful_shutdown
            || REQUIRED_EVENTS
                .iter()
                .any(|family| !self.event_families.iter().any(|e| e == family))
        {
            return Err(ProtocolError::UnsupportedHandshake);
        }
        match self.capabilities.replay {
            ReplayCapability::Unavailable if self.supports("output.replay") => {
                return Err(ProtocolError::InvalidCapabilities);
            }
            ReplayCapability::Runtime {
                max_events_per_session,
            } if max_events_per_session == 0 || !self.supports("output.replay") => {
                return Err(ProtocolError::InvalidCapabilities);
            }
            ReplayCapability::Unavailable | ReplayCapability::Runtime { .. } => {}
        }
        if matches!(
            self.capabilities.request_receipts,
            ReceiptCapability::Runtime { max_requests: 0 }
        ) {
            return Err(ProtocolError::InvalidCapabilities);
        }
        Ok(())
    }

    pub fn supports(&self, method: &str) -> bool {
        self.request_methods.iter().any(|entry| entry == method)
    }

    /// Callers must require their concrete operations before relying on a family.
    pub fn require_methods(&self, methods: &[&str]) -> Result<(), ProtocolError> {
        if methods.iter().all(|method| self.supports(method)) {
            Ok(())
        } else {
            Err(ProtocolError::UnsupportedHandshake)
        }
    }
}
