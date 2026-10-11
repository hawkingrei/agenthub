use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{GuardedPrompt, ProtocolError, SemanticGuardEvent, protocol::validate_id};

#[cfg(test)]
mod tests;

// Recovery data is conversation content. It deliberately does not implement Debug.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionOrigin {
    pub runtime_id: String,
    pub request_id: String,
}

impl DecisionOrigin {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_id(&self.runtime_id)?;
        validate_id(&self.request_id)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionState {
    Accepted,
    Completed,
    Uncertain,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputDecisionReceipt {
    pub waiting_turn_id: String,
    pub answer_turn_id: String,
    pub origin: Option<DecisionOrigin>,
    pub answer_fingerprint: String,
    pub state: DecisionState,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryReason {
    ProcessLost,
    ExecutionInterrupted,
    PendingCancelled,
    CleanupIncomplete,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryBlock {
    pub recovery_id: String,
    pub turn_id: Option<String>,
    pub reason: RecoveryReason,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryResolution {
    pub recovery_id: String,
    pub note: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryTarget {
    pub runtime_id: String,
    pub session_id: String,
    pub recovery_id: String,
}

impl RecoveryTarget {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_id(&self.runtime_id)?;
        validate_id(&self.session_id)?;
        validate_recovery_id(&self.recovery_id)
    }
}

impl RecoveryResolution {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_recovery_id(&self.recovery_id)?;
        if self.note.len() > 4096 {
            return Err(ProtocolError::FrameTooLarge);
        }
        if self.note.trim().is_empty() || self.note.chars().any(char::is_control) {
            return Err(ProtocolError::MalformedFrame);
        }
        Ok(())
    }
}

pub(crate) fn validate_recovery_id(id: &str) -> Result<(), ProtocolError> {
    validate_id(id)?;
    if !id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(ProtocolError::InvalidIdentity);
    }
    Ok(())
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryStatus {
    pub waiting_turn_id: Option<String>,
    pub blocked: Option<RecoveryBlock>,
    pub decisions: Vec<InputDecisionReceipt>,
    pub last_resolution: Option<RecoveryResolution>,
}

impl RecoveryStatus {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.decisions.len() > 256 {
            return Err(ProtocolError::FrameTooLarge);
        }
        if let Some(turn) = &self.waiting_turn_id {
            validate_id(turn)?;
            if self.blocked.is_some() {
                return Err(ProtocolError::InvalidTarget);
            }
        }
        if let Some(block) = &self.blocked {
            validate_recovery_id(&block.recovery_id)?;
            if let Some(turn) = &block.turn_id {
                validate_id(turn)?;
            }
        }
        if let Some(resolution) = &self.last_resolution {
            resolution.validate()?;
        }
        let mut waiting = BTreeSet::new();
        let mut answers = BTreeSet::new();
        let mut origins = BTreeSet::new();
        for decision in &self.decisions {
            validate_id(&decision.waiting_turn_id)?;
            validate_id(&decision.answer_turn_id)?;
            if !waiting.insert(&decision.waiting_turn_id)
                || !answers.insert(&decision.answer_turn_id)
                || self.waiting_turn_id.as_ref() == Some(&decision.waiting_turn_id)
                || decision.waiting_turn_id == decision.answer_turn_id
            {
                return Err(ProtocolError::InvalidTarget);
            }
            if decision.answer_fingerprint.len() != 64
                || !decision
                    .answer_fingerprint
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(ProtocolError::MalformedFrame);
            }
            if let Some(origin) = &decision.origin {
                origin.validate()?;
                if !origins.insert((&origin.runtime_id, &origin.request_id)) {
                    return Err(ProtocolError::InvalidTarget);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReentryTarget {
    Waiting { turn_id: String },
    Recovery { recovery_id: String },
}

impl ReentryTarget {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::Waiting { turn_id } => validate_id(turn_id),
            Self::Recovery { recovery_id } => validate_recovery_id(recovery_id),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReentryGuard {
    pub target: ReentryTarget,
    pub guard: GuardedPrompt,
}

impl ReentryGuard {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.target.validate()?;
        self.guard.validate()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReentryEvaluation {
    pub origin: DecisionOrigin,
    pub target: ReentryTarget,
    pub result: SemanticGuardEvent,
}

impl ReentryEvaluation {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.origin.validate()?;
        self.target.validate()?;
        if let SemanticGuardEvent::Decided { decision } = &self.result {
            decision.validate()?;
        }
        Ok(())
    }
}
