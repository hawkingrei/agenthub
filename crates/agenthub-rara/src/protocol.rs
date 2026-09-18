use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Handshake;

/// Errors contain categories only; raw wire bytes and provider diagnostics stay private.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    #[error("rara_app_server_unsupported: incompatible handshake or missing required capability")]
    UnsupportedHandshake,
    #[error("runtime-control frame is malformed or truncated")]
    MalformedFrame,
    #[error("runtime-control frame exceeds the byte limit")]
    FrameTooLarge,
    #[error("runtime-control identity is invalid")]
    InvalidIdentity,
    #[error("runtime-control capabilities are invalid")]
    InvalidCapabilities,
    #[error("runtime-control request target is invalid")]
    InvalidTarget,
    #[error("runtime-control transport closed or failed")]
    TransportLost,
    #[error("runtime-control frame serialization failed")]
    Serialization,
}

#[derive(Clone, Debug, Serialize)]
pub struct Provenance {
    controller: &'static str,
    adapter: &'static str,
    pub session_id: Option<String>,
    pub source_id: Option<String>,
    trust: &'static str,
    authorship: &'static str,
}

impl Provenance {
    pub fn new(session_id: Option<String>) -> Self {
        Self {
            controller: "app_server",
            adapter: "agenthub",
            session_id,
            source_id: None,
            trust: "untrusted",
            authorship: "user_provided",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ControlEnvelope {
    pub request_id: String,
    pub provenance: Provenance,
    pub request: Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ClientFrame {
    Control {
        runtime_id: String,
        envelope: ControlEnvelope,
        #[serde(skip_serializing_if = "Option::is_none")]
        expected_turn_id: Option<String>,
    },
    Replay {
        runtime_id: String,
        request_id: String,
        session_id: String,
        after_sequence: u64,
    },
    Shutdown {
        runtime_id: String,
        request_id: String,
    },
}

impl ClientFrame {
    pub fn runtime_id(&self) -> &str {
        match self {
            Self::Control { runtime_id, .. }
            | Self::Replay { runtime_id, .. }
            | Self::Shutdown { runtime_id, .. } => runtime_id,
        }
    }

    pub fn request_id(&self) -> &str {
        match self {
            Self::Control { envelope, .. } => &envelope.request_id,
            Self::Replay { request_id, .. } | Self::Shutdown { request_id, .. } => request_id,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), ProtocolError> {
        validate_id(self.runtime_id())?;
        validate_id(self.request_id())?;
        match self {
            Self::Control {
                envelope,
                expected_turn_id,
                ..
            } => {
                for id in [
                    envelope.provenance.session_id.as_ref(),
                    envelope.provenance.source_id.as_ref(),
                    expected_turn_id.as_ref(),
                ]
                .into_iter()
                .flatten()
                {
                    validate_id(id)?;
                }
                let family = envelope
                    .request
                    .get("type")
                    .and_then(Value::as_str)
                    .ok_or(ProtocolError::MalformedFrame)?;
                let method = envelope
                    .request
                    .get("payload")
                    .and_then(|p| p.get("type"))
                    .and_then(Value::as_str)
                    .ok_or(ProtocolError::MalformedFrame)?;
                let create = family == "session" && method == "create_session";
                if create == envelope.provenance.session_id.is_some() {
                    return Err(ProtocolError::InvalidTarget);
                }
                let requires_turn = matches!(
                    (family, method),
                    ("session", "cancel_current_turn" | "interrupt_current_turn")
                        | (
                            "input",
                            "answer_pending_input"
                                | "answer_plan_approval"
                                | "answer_shell_approval"
                        )
                        | ("approval", "answer_pending_approval")
                );
                if requires_turn != expected_turn_id.is_some() {
                    return Err(ProtocolError::InvalidTarget);
                }
            }
            Self::Replay { session_id, .. } => validate_id(session_id)?,
            Self::Shutdown { .. } => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ServerFrame {
    Handshake(Handshake),
    Ack(Acknowledgement),
    Event(EventFrame),
    ReplayGap(ReplayGap),
    ShutdownComplete {
        runtime_id: String,
        request_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Acknowledgement {
    pub runtime_id: String,
    pub request_id: String,
    pub result: RequestResult,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum RequestResult {
    Accepted {
        session_id: Option<String>,
        turn_id: Option<String>,
        last_sequence: Option<u64>,
    },
    Queued {
        session_id: String,
    },
    Rejected {
        code: RejectionCode,
        message: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionCode {
    InvalidRequest,
    StaleRuntime,
    UnknownSession,
    Unsupported,
    Busy,
    NotRunning,
    Overloaded,
    Closed,
    RequestConflict,
    Internal,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventFrame {
    pub runtime_id: String,
    pub session_id: String,
    pub event: RuntimeEvent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeEvent {
    pub event_id: String,
    pub provenance: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    pub sequence: u64,
    pub event: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayGap {
    pub runtime_id: String,
    pub request_id: String,
    pub session_id: String,
    pub requested_after: u64,
    pub oldest_available: u64,
    pub latest: u64,
}

impl ServerFrame {
    pub fn runtime_id(&self) -> &str {
        match self {
            Self::Handshake(frame) => &frame.runtime_id,
            Self::Ack(frame) => &frame.runtime_id,
            Self::Event(frame) => &frame.runtime_id,
            Self::ReplayGap(frame) => &frame.runtime_id,
            Self::ShutdownComplete { runtime_id, .. } => runtime_id,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), ProtocolError> {
        validate_id(self.runtime_id())?;
        match self {
            Self::Handshake(hello) => hello.validate()?,
            Self::Ack(ack) => {
                validate_id(&ack.request_id)?;
                match &ack.result {
                    RequestResult::Accepted {
                        session_id,
                        turn_id,
                        ..
                    } => {
                        for id in [session_id, turn_id].into_iter().flatten() {
                            validate_id(id)?;
                        }
                    }
                    RequestResult::Queued { session_id } => validate_id(session_id)?,
                    RequestResult::Rejected { message, .. } => validate_label(message)?,
                }
            }
            Self::Event(frame) => {
                validate_id(&frame.session_id)?;
                validate_id(&frame.event.event_id)?;
                if let Some(turn) = &frame.event.turn_id {
                    validate_id(turn)?;
                }
                if frame.event.sequence == 0
                    || !frame.event.provenance.is_object()
                    || !frame.event.event.is_object()
                {
                    return Err(ProtocolError::MalformedFrame);
                }
            }
            Self::ReplayGap(gap) => {
                validate_id(&gap.request_id)?;
                validate_id(&gap.session_id)?;
                if gap.oldest_available == 0
                    || gap.oldest_available > gap.latest.saturating_add(1)
                    || (gap.requested_after <= gap.latest
                        && gap.requested_after.saturating_add(1) >= gap.oldest_available)
                {
                    return Err(ProtocolError::MalformedFrame);
                }
            }
            Self::ShutdownComplete { request_id, .. } => validate_id(request_id)?,
        }
        Ok(())
    }
}

pub(crate) fn validate_id(id: &str) -> Result<(), ProtocolError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:/-".contains(&b))
    {
        Err(ProtocolError::InvalidIdentity)
    } else {
        Ok(())
    }
}

pub(crate) fn validate_label(label: &str) -> Result<(), ProtocolError> {
    if label.trim().is_empty() || label.len() > 256 || label.chars().any(char::is_control) {
        Err(ProtocolError::InvalidIdentity)
    } else {
        Ok(())
    }
}
