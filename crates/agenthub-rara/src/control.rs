use serde::Serialize;
use serde_json::{Value, json};

use crate::{ClientFrame, ControlEnvelope, MAX_FRAME_BYTES, ProtocolError, Provenance};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlKind {
    CreateSession,
    Query,
    Prompt,
    FollowUp,
    Cancel,
    Interrupt,
    UserAnswer,
    PlanAnswer,
    ShellAnswer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanDecision {
    Approve,
    ContinuePlanning,
    Reject,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellDecision {
    Once,
    Prefix,
    Always,
    // The pinned native implementation records rejection and does not run the command.
    #[serde(rename = "suggestion")]
    Deny,
}

/// Deliberately lacks Debug: user content must not become request diagnostics.
#[derive(Clone)]
pub enum ControlRequest {
    CreateSession,
    Query,
    Prompt {
        prompt: String,
    },
    FollowUp {
        prompt: String,
    },
    Cancel {
        turn_id: String,
    },
    Interrupt {
        turn_id: String,
    },
    UserAnswer {
        turn_id: String,
        answer: String,
    },
    PlanAnswer {
        turn_id: String,
        decision: PlanDecision,
        feedback: Option<String>,
    },
    ShellAnswer {
        turn_id: String,
        decision: ShellDecision,
    },
}

impl ControlRequest {
    pub fn kind(&self) -> ControlKind {
        match self {
            Self::CreateSession => ControlKind::CreateSession,
            Self::Query => ControlKind::Query,
            Self::Prompt { .. } => ControlKind::Prompt,
            Self::FollowUp { .. } => ControlKind::FollowUp,
            Self::Cancel { .. } => ControlKind::Cancel,
            Self::Interrupt { .. } => ControlKind::Interrupt,
            Self::UserAnswer { .. } => ControlKind::UserAnswer,
            Self::PlanAnswer { .. } => ControlKind::PlanAnswer,
            Self::ShellAnswer { .. } => ControlKind::ShellAnswer,
        }
    }

    pub fn expected_turn_id(&self) -> Option<&str> {
        match self {
            Self::Cancel { turn_id }
            | Self::Interrupt { turn_id }
            | Self::UserAnswer { turn_id, .. }
            | Self::PlanAnswer { turn_id, .. }
            | Self::ShellAnswer { turn_id, .. } => Some(turn_id),
            _ => None,
        }
    }

    pub fn frame(
        &self,
        runtime_id: &str,
        request_id: &str,
        session_id: Option<&str>,
    ) -> Result<ClientFrame, ProtocolError> {
        let (family, method, body): (&str, &str, Option<Value>) = match self {
            Self::CreateSession => ("session", "create_session", None),
            Self::Query => ("session", "query_runtime_state", None),
            Self::Cancel { .. } => ("session", "cancel_current_turn", None),
            Self::Interrupt { .. } => ("session", "interrupt_current_turn", None),
            Self::Prompt { prompt } | Self::FollowUp { prompt } => {
                text(prompt, true)?;
                let method = if matches!(self, Self::Prompt { .. }) {
                    "submit_user_prompt"
                } else {
                    "submit_follow_up"
                };
                ("input", method, Some(json!({"prompt": prompt})))
            }
            Self::UserAnswer { answer, .. } => {
                text(answer, true)?;
                (
                    "input",
                    "answer_pending_input",
                    Some(json!({"answer": answer})),
                )
            }
            Self::PlanAnswer {
                decision, feedback, ..
            } => {
                if let Some(feedback) = feedback {
                    text(feedback, false)?;
                }
                (
                    "input",
                    "answer_plan_approval",
                    Some(json!({"decision": decision, "feedback": feedback})),
                )
            }
            Self::ShellAnswer { decision, .. } => (
                "input",
                "answer_shell_approval",
                Some(json!({"decision": decision})),
            ),
        };
        let mut operation = json!({"type": method});
        if let Some(body) = body {
            operation["payload"] = body;
        }
        let frame = ClientFrame::Control {
            runtime_id: runtime_id.to_owned(),
            envelope: ControlEnvelope {
                request_id: request_id.to_owned(),
                provenance: Provenance::new(session_id.map(str::to_owned)),
                request: json!({"type": family, "payload": operation}),
            },
            expected_turn_id: self.expected_turn_id().map(str::to_owned),
        };
        frame.validate()?;
        // Includes JSON escaping and envelope overhead; reject before durable send intent.
        crate::encode_request(&frame)?;
        Ok(frame)
    }
}

fn text(text: &str, required: bool) -> Result<(), ProtocolError> {
    if required && text.trim().is_empty() {
        return Err(ProtocolError::MalformedFrame);
    }
    if text.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_controls_preserve_targets_and_exact_wire_methods() {
        let cases = [
            (
                ControlRequest::CreateSession,
                None,
                "session",
                "create_session",
                None,
            ),
            (
                ControlRequest::Query,
                Some("native"),
                "session",
                "query_runtime_state",
                None,
            ),
            (
                ControlRequest::Prompt {
                    prompt: "hello".into(),
                },
                Some("native"),
                "input",
                "submit_user_prompt",
                None,
            ),
            (
                ControlRequest::FollowUp {
                    prompt: "next".into(),
                },
                Some("native"),
                "input",
                "submit_follow_up",
                None,
            ),
            (
                ControlRequest::Cancel {
                    turn_id: "waiting".into(),
                },
                Some("native"),
                "session",
                "cancel_current_turn",
                Some("waiting"),
            ),
            (
                ControlRequest::Interrupt {
                    turn_id: "waiting".into(),
                },
                Some("native"),
                "session",
                "interrupt_current_turn",
                Some("waiting"),
            ),
            (
                ControlRequest::UserAnswer {
                    turn_id: "waiting".into(),
                    answer: "yes".into(),
                },
                Some("native"),
                "input",
                "answer_pending_input",
                Some("waiting"),
            ),
            (
                ControlRequest::PlanAnswer {
                    turn_id: "waiting".into(),
                    decision: PlanDecision::ContinuePlanning,
                    feedback: Some("refine".into()),
                },
                Some("native"),
                "input",
                "answer_plan_approval",
                Some("waiting"),
            ),
            (
                ControlRequest::ShellAnswer {
                    turn_id: "waiting".into(),
                    decision: ShellDecision::Deny,
                },
                Some("native"),
                "input",
                "answer_shell_approval",
                Some("waiting"),
            ),
        ];
        for (request, target, family, method, turn) in cases {
            let wire =
                serde_json::to_value(request.frame("runtime", "request", target).unwrap()).unwrap();
            let payload = &wire["payload"];
            assert_eq!(payload["runtime_id"], "runtime");
            assert_eq!(payload["envelope"]["request_id"], "request");
            assert_eq!(payload["envelope"]["request"]["type"], family);
            assert_eq!(payload["envelope"]["request"]["payload"]["type"], method);
            assert_eq!(
                payload["envelope"]["provenance"]["session_id"].as_str(),
                target
            );
            assert_eq!(payload["expected_turn_id"].as_str(), turn);
            assert_eq!(payload["envelope"]["provenance"]["trust"], "untrusted");
        }
    }

    #[test]
    fn permission_choices_are_explicit_and_deny_never_serializes_as_approval() {
        for (decision, expected) in [
            (ShellDecision::Once, "once"),
            (ShellDecision::Prefix, "prefix"),
            (ShellDecision::Always, "always"),
            (ShellDecision::Deny, "suggestion"),
        ] {
            let frame = ControlRequest::ShellAnswer {
                turn_id: "waiting".into(),
                decision,
            }
            .frame("runtime", "request", Some("native"))
            .unwrap();
            let wire = serde_json::to_value(frame).unwrap();
            assert_eq!(
                wire["payload"]["envelope"]["request"]["payload"]["payload"]["decision"],
                expected
            );
        }
        assert_eq!(
            serde_json::to_value(PlanDecision::Reject).unwrap(),
            "reject"
        );
        assert_eq!(
            serde_json::to_value(PlanDecision::Approve).unwrap(),
            "approve"
        );
    }

    #[test]
    fn invalid_control_scope_and_oversized_content_fail_before_dispatch() {
        assert!(
            ControlRequest::CreateSession
                .frame("runtime", "request", Some("native"))
                .is_err()
        );
        assert!(
            ControlRequest::Query
                .frame("runtime", "request", None)
                .is_err()
        );
        assert!(
            ControlRequest::Cancel {
                turn_id: String::new()
            }
            .frame("runtime", "request", Some("native"))
            .is_err()
        );
        for prompt in [
            " ".into(),
            "x".repeat(MAX_FRAME_BYTES + 1),
            "\u{0000}".repeat(MAX_FRAME_BYTES / 2),
        ] {
            assert!(
                ControlRequest::Prompt { prompt }
                    .frame("runtime", "request", Some("native"))
                    .is_err()
            );
        }
        let prompt = "quotes: \"line\"\nnext";
        let frame = ControlRequest::Prompt {
            prompt: prompt.into(),
        }
        .frame("runtime", "request", Some("native"))
        .unwrap();
        let wire = serde_json::to_value(frame).unwrap();
        assert_eq!(
            wire["payload"]["envelope"]["request"]["payload"]["payload"]["prompt"],
            prompt
        );
    }
}
