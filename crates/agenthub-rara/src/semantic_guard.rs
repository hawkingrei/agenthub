use serde::{Deserialize, Serialize};

use crate::{Handshake, ProtocolError};

/// Explicit routing context. It contains task data and is never safe diagnostics.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticGuardContext {
    pub role: String,
    pub card: String,
    pub work: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuardedPrompt {
    pub prompt: String,
    pub context: SemanticGuardContext,
}

impl GuardedPrompt {
    pub fn require_capability(handshake: &Handshake) -> Result<(), ProtocolError> {
        handshake.require_methods(&["input.submit_guarded_prompt"])?;
        if !handshake
            .event_families
            .iter()
            .any(|family| family == "semantic_guard")
        {
            return Err(ProtocolError::UnsupportedHandshake);
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        for (text, limit) in [
            (&self.prompt, 64 * 1024),
            (&self.context.role, 4 * 1024),
            (&self.context.card, 8 * 1024),
            (&self.context.work, 64 * 1024),
        ] {
            if text.len() > limit {
                return Err(ProtocolError::FrameTooLarge);
            }
            if text.trim().is_empty() || text.contains('\0') {
                return Err(ProtocolError::MalformedFrame);
            }
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum SemanticGuardDecision {
    Compatible {},
    Mismatch { reason: String },
    NeedsClarification { reason: String, question: String },
}

impl SemanticGuardDecision {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        let bounded = |text: &str| {
            if text.len() > 1024 {
                return Err(ProtocolError::FrameTooLarge);
            }
            if text.trim().is_empty() || text.chars().any(char::is_control) {
                return Err(ProtocolError::MalformedFrame);
            }
            Ok(())
        };
        match self {
            Self::Compatible {} => Ok(()),
            Self::Mismatch { reason } => bounded(reason),
            Self::NeedsClarification { reason, question } => {
                bounded(reason)?;
                bounded(question)
            }
        }
    }

    pub fn is_decline(&self) -> bool {
        !matches!(self, Self::Compatible {})
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticGuardFailure {
    Provider,
    Timeout,
    InvalidResponse,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SemanticGuardEvent {
    Decided { decision: SemanticGuardDecision },
    Unavailable { reason: SemanticGuardFailure },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClientFrame, ControlRequest};

    fn guarded() -> GuardedPrompt {
        GuardedPrompt {
            prompt: "Run the activation".into(),
            context: SemanticGuardContext {
                role: "worker".into(),
                card: "Database reviewer".into(),
                work: "Review the query".into(),
            },
        }
    }

    #[test]
    fn guarded_prompt_enforces_text_bounds_before_encoding() {
        for field in ["prompt", "role", "card", "work"] {
            for value in ["".into(), " \n".into(), "x\0x".into(), "x".repeat(65537)] {
                let mut input = guarded();
                *match field {
                    "prompt" => &mut input.prompt,
                    "role" => &mut input.context.role,
                    "card" => &mut input.context.card,
                    _ => &mut input.context.work,
                } = value;
                assert!(
                    ControlRequest::GuardedPrompt(input)
                        .frame("runtime", "request", Some("session"))
                        .is_err()
                );
            }
        }
        let mut input = guarded();
        input.prompt = "x".repeat(65536);
        input.context.role = "x".repeat(4096);
        input.context.card = "x".repeat(8192);
        input.context.work = "x".repeat(65536);
        let ClientFrame::Control {
            envelope,
            expected_turn_id,
            ..
        } = ControlRequest::GuardedPrompt(input.clone())
            .frame("runtime", "request", Some("session"))
            .unwrap()
        else {
            panic!("expected control")
        };
        assert!(expected_turn_id.is_none());
        assert_eq!(envelope.request["payload"]["type"], "submit_guarded_prompt");
        assert_eq!(
            envelope.request["payload"]["payload"],
            serde_json::to_value(input).unwrap()
        );
    }

    #[test]
    fn guarded_prompt_requires_both_method_and_event_family() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/stdio-v1.json")).unwrap();
        let hello: Handshake =
            serde_json::from_value(fixture["frames"][0]["payload"].clone()).unwrap();
        GuardedPrompt::require_capability(&hello).unwrap();
        let mut missing = hello.clone();
        missing
            .request_methods
            .retain(|method| method != "input.submit_guarded_prompt");
        assert_eq!(
            GuardedPrompt::require_capability(&missing),
            Err(ProtocolError::UnsupportedHandshake)
        );
        let mut missing = hello;
        missing
            .event_families
            .retain(|family| family != "semantic_guard");
        assert_eq!(
            GuardedPrompt::require_capability(&missing),
            Err(ProtocolError::UnsupportedHandshake)
        );
    }
}
