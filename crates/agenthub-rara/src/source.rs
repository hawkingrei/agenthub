use serde_json::{Value, json};

use crate::protocol::validate_id;
use crate::{ControlKind, Handshake, ProtocolError};

/// Explicit session inputs. No filesystem discovery, system-layer authority, or
/// persistence across native sessions is implied by registering these sources.
#[derive(Clone)]
pub enum SourceRegistration {
    Prompt {
        source_id: String,
        content: String,
    },
    Skill {
        source_id: String,
        name: String,
        content: String,
    },
}

impl SourceRegistration {
    /// Validate the entire immutable bootstrap before sending any source request.
    pub fn validate_batch(sources: &[Self], handshake: &Handshake) -> Result<(), ProtocolError> {
        let mut prompt_count = 0;
        let mut skill_count = 0;
        let mut prompt_bytes = 0;
        let mut skill_bytes = 0;
        let mut identities = std::collections::BTreeSet::new();
        for source in sources {
            source.require_capability(handshake)?;
            source.operation()?;
            let identity = match source {
                Self::Prompt { source_id, content } => {
                    prompt_count += 1;
                    prompt_bytes += content.len();
                    ("prompt", source_id.as_str(), "")
                }
                Self::Skill {
                    source_id,
                    name,
                    content,
                } => {
                    skill_count += 1;
                    skill_bytes += content.len();
                    ("skill", source_id.as_str(), name.as_str())
                }
            };
            if !identities.insert(identity) {
                return Err(ProtocolError::InvalidIdentity);
            }
            if prompt_count > 32
                || skill_count > 32
                || prompt_bytes > 256 * 1024
                || skill_bytes > 256 * 1024
            {
                return Err(ProtocolError::FrameTooLarge);
            }
        }
        Ok(())
    }

    pub fn source_id(&self) -> &str {
        match self {
            Self::Prompt { source_id, .. } | Self::Skill { source_id, .. } => source_id,
        }
    }

    pub(crate) fn kind(&self) -> ControlKind {
        match self {
            Self::Prompt { .. } => ControlKind::PromptSource,
            Self::Skill { .. } => ControlKind::SkillSource,
        }
    }

    pub fn require_capability(&self, handshake: &Handshake) -> Result<(), ProtocolError> {
        handshake.require_methods(&[match self {
            Self::Prompt { .. } => "prompt_source.register",
            Self::Skill { .. } => "skill_source.register",
        }])
    }

    pub(crate) fn operation(&self) -> Result<(&'static str, &'static str, Value), ProtocolError> {
        validate_id(self.source_id())?;
        let content = match self {
            Self::Prompt { content, .. } | Self::Skill { content, .. } => content,
        };
        if content.trim().is_empty() {
            return Err(ProtocolError::MalformedFrame);
        }
        if content.len() > 64 * 1024 {
            return Err(ProtocolError::FrameTooLarge);
        }
        Ok(match self {
            Self::Prompt { source_id, content } => (
                "prompt_source",
                "register",
                json!({
                    "source_id": source_id,
                    "scope": "session",
                    "layer": "user",
                    "budget_hint_tokens": null,
                    "lifetime": {"type": "session"},
                    "content": content,
                }),
            ),
            Self::Skill {
                source_id,
                name,
                content,
            } => {
                validate_id(name)?;
                (
                    "skill_source",
                    "register_skill",
                    json!({"source_id": source_id, "name": name, "content": content, "precedence_hint": null}),
                )
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ControlRequest;

    #[test]
    fn source_registration_preserves_scope_content_and_untrusted_provenance() {
        let content = "---\ndescription: Review the assigned work.\n---\nUse the canonical task.";
        for source in [
            SourceRegistration::Prompt {
                source_id: "activation-context".into(),
                content: content.into(),
            },
            SourceRegistration::Skill {
                source_id: "loop-skill".into(),
                name: "team-loop-runtime".into(),
                content: content.into(),
            },
        ] {
            let wire = serde_json::to_value(
                ControlRequest::RegisterSource(source.clone())
                    .frame("runtime", "request", Some("native"))
                    .unwrap(),
            )
            .unwrap();
            let envelope = &wire["payload"]["envelope"];
            assert_eq!(envelope["provenance"]["session_id"], "native");
            assert_eq!(envelope["provenance"]["source_id"], source.source_id());
            assert_eq!(envelope["provenance"]["trust"], "untrusted");
            let registration = &envelope["request"]["payload"]["payload"];
            assert_eq!(registration["content"], content);
            assert_eq!(registration["source_id"], source.source_id());
            assert!(wire["payload"].get("expected_turn_id").is_none());
            match source {
                SourceRegistration::Prompt { .. } => {
                    assert_eq!(envelope["request"]["type"], "prompt_source");
                    assert_eq!(registration["scope"], "session");
                    assert_eq!(registration["layer"], "user");
                    assert_eq!(registration["lifetime"], json!({"type":"session"}));
                }
                SourceRegistration::Skill { .. } => {
                    assert_eq!(envelope["request"]["type"], "skill_source");
                    assert_eq!(envelope["request"]["payload"]["type"], "register_skill");
                    assert_eq!(registration["name"], "team-loop-runtime");
                }
            }
            assert!(
                ControlRequest::RegisterSource(source)
                    .frame("runtime", "request", None)
                    .is_err()
            );
        }
    }

    #[test]
    fn source_registration_rejects_invalid_ids_and_native_content_limits() {
        for (source_id, content) in [
            ("invalid id", "body".into()),
            ("context", " ".into()),
            ("context", "x".repeat(64 * 1024 + 1)),
        ] {
            let request = ControlRequest::RegisterSource(SourceRegistration::Prompt {
                source_id: source_id.into(),
                content,
            });
            assert!(request.frame("runtime", "request", Some("native")).is_err());
        }
        let request = ControlRequest::RegisterSource(SourceRegistration::Skill {
            source_id: "skill".into(),
            name: "invalid name".into(),
            content: "body".into(),
        });
        assert!(request.frame("runtime", "request", Some("native")).is_err());
        let request = ControlRequest::RegisterSource(SourceRegistration::Prompt {
            source_id: "context".into(),
            content: "x".repeat(64 * 1024),
        });
        assert!(request.frame("runtime", "request", Some("native")).is_ok());
    }

    #[test]
    fn source_registration_requires_its_method_even_when_family_is_advertised() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/stdio-v1.json")).unwrap();
        let hello: Handshake =
            serde_json::from_value(fixture["frames"][0]["payload"].clone()).unwrap();
        for source in [
            SourceRegistration::Prompt {
                source_id: "context".into(),
                content: "context".into(),
            },
            SourceRegistration::Skill {
                source_id: "skill".into(),
                name: "review".into(),
                content: "review".into(),
            },
        ] {
            source.require_capability(&hello).unwrap();
            let mut missing = hello.clone();
            missing.request_methods.retain(|method| {
                !matches!(
                    method.as_str(),
                    "prompt_source.register" | "skill_source.register"
                )
            });
            missing.validate().unwrap();
            assert_eq!(
                source.require_capability(&missing),
                Err(ProtocolError::UnsupportedHandshake)
            );
        }
    }

    #[test]
    fn source_batch_rejects_duplicate_identity_and_aggregate_capacity_before_send() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/stdio-v1.json")).unwrap();
        let hello: Handshake =
            serde_json::from_value(fixture["frames"][0]["payload"].clone()).unwrap();
        for skill in [false, true] {
            let source = |index, bytes| {
                let source_id = format!("source-{index}");
                let content = "x".repeat(bytes);
                if skill {
                    SourceRegistration::Skill {
                        source_id,
                        name: "review".into(),
                        content,
                    }
                } else {
                    SourceRegistration::Prompt { source_id, content }
                }
            };
            let mut count: Vec<_> = (0..32).map(|index| source(index, 1)).collect();
            SourceRegistration::validate_batch(&count, &hello).unwrap();
            count.push(source(32, 1));
            assert_eq!(
                SourceRegistration::validate_batch(&count, &hello),
                Err(ProtocolError::FrameTooLarge)
            );
            let mut bytes: Vec<_> = (0..4).map(|index| source(index, 64 * 1024)).collect();
            SourceRegistration::validate_batch(&bytes, &hello).unwrap();
            bytes.push(source(4, 1));
            assert_eq!(
                SourceRegistration::validate_batch(&bytes, &hello),
                Err(ProtocolError::FrameTooLarge)
            );
            assert_eq!(
                SourceRegistration::validate_batch(&[source(0, 1), source(0, 2)], &hello),
                Err(ProtocolError::InvalidIdentity)
            );
        }
    }
}
