use crate::{EventFrame, ProtocolError, SemanticGuardDecision, SemanticGuardEvent};

use super::native::{InputEvent, NativeEvent, SessionEvent};
use super::required_turn;

#[derive(Clone, Default)]
pub(super) struct GuardTracker {
    turn: Option<GuardTurn>,
}

#[derive(Clone)]
struct GuardTurn {
    id: String,
    seen: bool,
    worker_started: bool,
    decision: Option<SemanticGuardDecision>,
}

impl GuardTracker {
    /// Only a unique decision followed by its normal terminal event can escape as an outcome.
    pub(super) fn observe(
        &mut self,
        frame: &EventFrame,
        event: &NativeEvent,
    ) -> Result<Option<SemanticGuardDecision>, ProtocolError> {
        match event {
            NativeEvent::Session(SessionEvent::TurnStarted) => {
                if self.turn.is_some() {
                    return Err(ProtocolError::InvalidTarget);
                }
                self.turn = Some(GuardTurn {
                    id: required_turn(frame)?,
                    seen: false,
                    worker_started: false,
                    decision: None,
                });
            }
            NativeEvent::SemanticGuard(guard) => {
                let turn = self.turn.as_mut().ok_or(ProtocolError::InvalidTarget)?;
                if turn.id != required_turn(frame)?
                    || turn.seen
                    || turn.worker_started
                    || frame.event.provenance["session_id"].as_str()
                        != Some(frame.session_id.as_str())
                    || frame.event.provenance["controller"] != "runtime"
                    || frame.event.provenance["trust"] != "trusted"
                    || frame.event.provenance["authorship"] != "runtime"
                {
                    return Err(ProtocolError::InvalidTarget);
                }
                turn.seen = true;
                match guard {
                    SemanticGuardEvent::Decided { decision } => {
                        decision.validate()?;
                        turn.decision = Some(decision.clone());
                    }
                    SemanticGuardEvent::Unavailable { .. } => {}
                }
            }
            NativeEvent::Session(SessionEvent::TurnFinished { reason }) => {
                if let Some(turn) = &self.turn {
                    if turn.id != required_turn(frame)? {
                        if turn.seen {
                            return Err(ProtocolError::InvalidTarget);
                        }
                        return Ok(None);
                    }
                    if reason.as_deref() == Some("awaiting_input")
                        && turn
                            .decision
                            .as_ref()
                            .is_some_and(SemanticGuardDecision::is_decline)
                    {
                        return Err(ProtocolError::InvalidTarget);
                    }
                }
                return Ok(self
                    .turn
                    .take()
                    .and_then(|turn| turn.decision)
                    .filter(SemanticGuardDecision::is_decline));
            }
            NativeEvent::Session(
                SessionEvent::TurnCancelled
                | SessionEvent::TurnInterrupted
                | SessionEvent::TurnFailed { .. },
            ) => {
                if let Some(turn) = &self.turn
                    && turn.seen
                    && turn.id != required_turn(frame)?
                {
                    return Err(ProtocolError::InvalidTarget);
                }
                self.turn = None;
            }
            NativeEvent::Assistant(_)
            | NativeEvent::Tool(_)
            | NativeEvent::Plan(_)
            | NativeEvent::Todo(_)
            | NativeEvent::Approval(_)
            | NativeEvent::Input(InputEvent::Requested { .. })
            | NativeEvent::Session(
                SessionEvent::ModelRequest { .. } | SessionEvent::ModelResponse { .. },
            ) => {
                if let Some(turn) = &mut self.turn {
                    if turn
                        .decision
                        .as_ref()
                        .is_some_and(SemanticGuardDecision::is_decline)
                    {
                        return Err(ProtocolError::InvalidTarget);
                    }
                    turn.worker_started = true;
                }
            }
            _ => {}
        }
        Ok(None)
    }
}
