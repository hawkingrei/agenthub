mod digest;
mod native;
mod telemetry;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::protocol::{validate_id, validate_label};
use crate::{EventFrame, ProtocolError};
use native::*;

pub use native::{PendingInput, PendingInputKind, SessionPhase, SessionSnapshot};

pub enum ProjectedHistory {
    Conversation(Value),
    System(String),
}

pub enum TurnEnd {
    Finished { reason: Option<String> },
    Cancelled,
    Interrupted,
    Failed,
}

/// Effects become visible only after the matching event transaction commits.
pub enum EventEffect {
    None,
    Created,
    Snapshot(SessionSnapshot),
    TurnStarted {
        turn_id: String,
    },
    TurnEnded {
        turn_id: String,
        outcome: TurnEnd,
    },
    InputRequested {
        pending: PendingInput,
        tool_call_id: String,
    },
    InputCleared {
        waiting_turn: String,
        terminal: bool,
    },
    ApprovalAnswered {
        approval_id: String,
        approved: bool,
    },
}

pub struct EventProjection {
    pub history: Vec<ProjectedHistory>,
    pub effect: EventEffect,
    /// Separate from conversation payloads, including their presentation metadata.
    pub safe_metadata: Value,
}

/// Clone before projection and install the clone only after successful persistence.
/// Duplicates and failed transactions must not advance presentation state.
#[derive(Clone)]
pub struct EventProjector {
    runtime_id: String,
    session_id: String,
    chunk: Option<Chunk>,
    question_turn: Option<String>,
    pending_turn: Option<String>,
    active_turn: Option<String>,
    tools: BTreeMap<String, OpenTool>,
}

#[derive(Clone)]
struct OpenTool {
    name: String,
    id: String,
    approval: Option<ToolApproval>,
    resume_turn: Option<String>,
}

#[derive(Clone)]
enum ToolApproval {
    Plan { waiting_turn: String },
    Shell,
}

#[derive(Clone)]
struct Chunk {
    kind: &'static str,
    turn_id: Option<String>,
    message_id: String,
    index: u64,
}

impl EventProjector {
    pub fn new(runtime_id: &str, session_id: &str) -> Result<Self, ProtocolError> {
        validate_id(runtime_id)?;
        validate_id(session_id)?;
        Ok(Self {
            runtime_id: runtime_id.into(),
            session_id: session_id.into(),
            chunk: None,
            question_turn: None,
            pending_turn: None,
            active_turn: None,
            tools: BTreeMap::new(),
        })
    }

    pub fn project(&mut self, frame: &EventFrame) -> Result<EventProjection, ProtocolError> {
        if frame.runtime_id != self.runtime_id || frame.session_id != self.session_id {
            return Err(ProtocolError::InvalidTarget);
        }
        crate::ServerFrame::Event(frame.clone()).validate()?;
        let event: NativeEvent = serde_json::from_value(frame.event.event.clone())
            .map_err(|_| ProtocolError::MalformedFrame)?;
        let family = frame.event.event["type"]
            .as_str()
            .ok_or(ProtocolError::MalformedFrame)?;
        let kind = frame.event.event["payload"]["type"]
            .as_str()
            .ok_or(ProtocolError::MalformedFrame)?;
        validate_label(kind)?;
        let metadata = json!({
            "provider": "rara", "runtime_id": frame.runtime_id,
            "native_session_id": frame.session_id, "event_id": frame.event.event_id,
            "sequence": frame.event.sequence, "turn_id": frame.event.turn_id,
            "family": family, "kind": kind,
        });
        let mut history = Vec::new();
        if !matches!(event, NativeEvent::Assistant(_)) {
            self.chunk = None;
        }
        let mut effect = EventEffect::None;
        match event {
            NativeEvent::Assistant(event) => {
                history.push(ProjectedHistory::Conversation(self.assistant(frame, event)))
            }
            NativeEvent::Tool(event) => {
                history.push(ProjectedHistory::Conversation(self.tool(frame, event)?))
            }
            NativeEvent::Session(event) => self.session(frame, event, &mut history, &mut effect)?,
            NativeEvent::Input(event) => self.input(frame, event, &mut history, &mut effect)?,
            NativeEvent::Approval(ApprovalEvent::Requested { approval_id, kind }) => {
                validate_id(&approval_id)?;
                validate_label(&kind)?;
                // Input::Requested carries the owned turn and concrete interaction. A bare
                // approval notice must never allocate a second live callback.
                history.push(update(
                    json!({"event": "approval_requested", "approval_id": approval_id}),
                ));
            }
            NativeEvent::Approval(ApprovalEvent::Answered {
                approval_id,
                approved,
            }) => {
                validate_id(&approval_id)?;
                if let Some(tool) = self.tools.get_mut(&approval_id)
                    && matches!(tool.approval.take(), Some(ToolApproval::Shell))
                    && approved
                {
                    tool.resume_turn.clone_from(&frame.event.turn_id);
                }
                history.push(update(json!({"event": "approval_answered", "approval_id": approval_id, "approved": approved})));
                effect = EventEffect::ApprovalAnswered {
                    approval_id,
                    approved,
                };
            }
            NativeEvent::Plan(PlanEvent::Updated { steps, explanation }) => {
                if steps.len() > 512 {
                    return Err(ProtocolError::FrameTooLarge);
                }
                history.push(ProjectedHistory::Conversation(
                    json!({"type": "plan", "plan": {
                        "entries": steps.into_iter().map(|step| json!({"content": step.step,
                            "status": step.status, "priority": "medium"})).collect::<Vec<_>>(),
                        "explanation": explanation,
                    }}),
                ));
            }
            NativeEvent::Plan(PlanEvent::Approved | PlanEvent::Continued) => {
                history.push(update(json!({"event": kind})))
            }
            NativeEvent::Todo(TodoEvent::Updated { state }) => {
                if state.items.len() > 512 {
                    return Err(ProtocolError::FrameTooLarge);
                }
                let items: Vec<_> = state.items.into_iter().map(|item|
                    json!({"id": item.id, "content": item.content, "status": item.status})).collect();
                history.push(update(
                    json!({"event": "todo_updated", "version": state.version,
                    "updated_at": state.updated_at, "items": items}),
                ));
            }
            NativeEvent::Warning(WarningEvent::RuntimeWarning { message }) => {
                drop(message);
                history.push(ProjectedHistory::System(
                    "Runtime reported a warning.".into(),
                ));
            }
            NativeEvent::Error(ErrorEvent::RuntimeError {
                message,
                recoverable,
            }) => {
                drop(message);
                history.push(ProjectedHistory::System(
                    if recoverable {
                        "Runtime reported a recoverable error."
                    } else {
                        "Runtime reported an error."
                    }
                    .into(),
                ));
                history.push(update(
                    json!({"event": "runtime_error", "recoverable": recoverable}),
                ));
            }
            NativeEvent::PromptSource(payload)
            | NativeEvent::Skill(payload)
            | NativeEvent::Mcp(payload)
            | NativeEvent::Memory(payload)
            | NativeEvent::Hook(payload)
            | NativeEvent::Context(payload)
            | NativeEvent::Extension(payload) => {
                history.push(update(telemetry::project(family, kind, &payload)?));
            }
        }
        for entry in &mut history {
            if let ProjectedHistory::Conversation(value) = entry {
                if value.get("meta").is_none() {
                    value["meta"] = json!({});
                }
                value["meta"]["provider_runtime"] = metadata.clone();
            }
        }
        Ok(EventProjection {
            history,
            effect,
            safe_metadata: metadata,
        })
    }

    fn assistant(&mut self, frame: &EventFrame, event: AssistantEvent) -> Value {
        let (kind, text, delta) = match event {
            AssistantEvent::Text(text) => ("agent_message", text, false),
            AssistantEvent::TextDelta(text) => ("agent_message", text, true),
            AssistantEvent::ThinkingDelta(text) => ("agent_thought", text, true),
        };
        if !delta {
            self.chunk = None;
            return json!({"type": kind, "text": text, "chunk": false,
                "message_id": format!("direct:{}:{}", self.session_id, frame.event.event_id)});
        }
        let same = self
            .chunk
            .as_ref()
            .is_some_and(|chunk| chunk.kind == kind && chunk.turn_id == frame.event.turn_id);
        if !same {
            self.chunk = Some(Chunk {
                kind,
                turn_id: frame.event.turn_id.clone(),
                message_id: format!("direct:{}:{}", self.session_id, frame.event.event_id),
                index: 0,
            });
        }
        let chunk = self.chunk.as_mut().expect("chunk was initialized");
        let value = json!({"type": kind, "text": text, "chunk": true,
            "message_id": chunk.message_id, "chunk_index": chunk.index});
        chunk.index += 1;
        value
    }

    fn tool(&mut self, frame: &EventFrame, event: ToolEvent) -> Result<Value, ProtocolError> {
        let (call_id, name) = match &event {
            ToolEvent::Use { call_id, name, .. }
            | ToolEvent::Result { call_id, name, .. }
            | ToolEvent::Progress { call_id, name, .. } => (call_id, name),
        };
        validate_label(name)?;
        if let Some(id) = call_id {
            validate_id(id)?;
        }
        // Approval answers start a new turn while the original call is still open.
        // Bind output to that call, and give a reused call ID a fresh history item.
        let mut id = format!("direct:{}:tool:{}", self.session_id, frame.event.event_id);
        let mut resumed = false;
        if let Some(call_id) = call_id {
            match &event {
                ToolEvent::Use { .. } => {
                    if self.active_turn.is_none() {
                        self.active_turn.clone_from(&frame.event.turn_id);
                    }
                    if let Some(tool) = self.tools.get_mut(call_id) {
                        if tool.name != *name
                            || tool.resume_turn.is_none()
                            || tool.resume_turn != frame.event.turn_id
                        {
                            return Err(ProtocolError::InvalidTarget);
                        }
                        tool.resume_turn = None;
                        id.clone_from(&tool.id);
                        resumed = true;
                    } else {
                        if self.tools.len() >= 512 {
                            return Err(ProtocolError::FrameTooLarge);
                        }
                        self.tools.insert(
                            call_id.clone(),
                            OpenTool {
                                name: name.clone(),
                                id: id.clone(),
                                approval: None,
                                resume_turn: None,
                            },
                        );
                    }
                }
                ToolEvent::Progress { .. } | ToolEvent::Result { .. } => {
                    if let Some(tool) = self.tools.get(call_id) {
                        if tool.name != *name {
                            return Err(ProtocolError::InvalidTarget);
                        }
                        id.clone_from(&tool.id);
                    }
                    if matches!(event, ToolEvent::Result { .. }) {
                        self.tools.remove(call_id);
                    }
                }
            }
        }
        Ok(match event {
            ToolEvent::Use { name, input, .. } => {
                json!({"type": if resumed {"tool_call_update"} else {"tool_call"}, "id": id,
                "title": name, "kind": "other", "status": "in_progress", "raw_input": input})
            }
            ToolEvent::Result {
                name,
                content,
                is_error,
                ..
            } => json!({"type": "tool_call_update", "id": id,
                "title": name, "status": if is_error { "failed" } else { "completed" }, "raw_output": content,
                "content": [{"type": "content", "content": {"type": "text", "text": content}}]}),
            ToolEvent::Progress {
                name,
                stream,
                chunk,
                ..
            } => json!({"type": "tool_call_update", "id": id,
                "title": name, "status": "in_progress", "meta": {"terminal_output": {"data": chunk, "stream": stream}}}),
        })
    }

    fn session(
        &mut self,
        frame: &EventFrame,
        event: SessionEvent,
        history: &mut Vec<ProjectedHistory>,
        effect: &mut EventEffect,
    ) -> Result<(), ProtocolError> {
        match event {
            SessionEvent::Created { session_id } | SessionEvent::Resumed { session_id } => {
                if session_id != self.session_id {
                    return Err(ProtocolError::InvalidTarget);
                }
                *effect = EventEffect::Created;
            }
            SessionEvent::RuntimeState { snapshot } => {
                snapshot.validate(&self.session_id, frame.event.sequence)?;
                history.push(update(
                    json!({"event": "runtime_state", "generation": snapshot.generation,
                    "last_sequence": snapshot.last_sequence, "phase": snapshot.phase.name()}),
                ));
                history.push(run_status(match snapshot.phase {
                    SessionPhase::AwaitingInput { .. } => "waiting_permission",
                    SessionPhase::Closed => "stopped",
                    _ => snapshot.phase.name(),
                }));
                *effect = EventEffect::Snapshot(snapshot);
            }
            SessionEvent::TurnStarted => {
                self.active_turn = Some(required_turn(frame)?);
                *effect = EventEffect::TurnStarted {
                    turn_id: required_turn(frame)?,
                };
                history.push(run_status("running"));
            }
            SessionEvent::TurnFinished { reason } => {
                let status = if reason.as_deref() == Some("awaiting_input") {
                    "waiting_permission"
                } else {
                    self.retire_tools(&required_turn(frame)?, history);
                    "idle"
                };
                *effect = EventEffect::TurnEnded {
                    turn_id: required_turn(frame)?,
                    outcome: TurnEnd::Finished { reason },
                };
                history.push(run_status(status));
            }
            SessionEvent::TurnCancelled | SessionEvent::TurnInterrupted => {
                let interrupted = matches!(event, SessionEvent::TurnInterrupted);
                self.retire_tools(&required_turn(frame)?, history);
                *effect = EventEffect::TurnEnded {
                    turn_id: required_turn(frame)?,
                    outcome: if interrupted {
                        TurnEnd::Interrupted
                    } else {
                        TurnEnd::Cancelled
                    },
                };
                history.push(run_status("cancelled"));
            }
            SessionEvent::TurnFailed { reason } => {
                drop(reason);
                self.retire_tools(&required_turn(frame)?, history);
                *effect = EventEffect::TurnEnded {
                    turn_id: required_turn(frame)?,
                    outcome: TurnEnd::Failed,
                };
                history.push(run_status("error"));
                history.push(ProjectedHistory::System("Runtime turn failed.".into()));
            }
            SessionEvent::Status { message } => {
                drop(message);
                history.push(update(json!({"event": "status"})));
            }
            SessionEvent::ModelRequest {
                model,
                input_tokens,
            } => {
                drop(model);
                history.push(update(
                    json!({"event": "model_request", "input_tokens": input_tokens}),
                ));
            }
            SessionEvent::ModelResponse {
                model,
                output_tokens,
                finish_reason,
            } => {
                drop((model, finish_reason));
                history.push(update(
                    json!({"event": "model_response", "output_tokens": output_tokens}),
                ));
            }
            SessionEvent::Compacted {
                count,
                before_tokens,
                after_tokens,
                summary,
                recent_files,
            } => {
                drop((summary, recent_files));
                history.push(update(json!({"event": "compacted", "count": count,
                    "before_tokens": before_tokens, "after_tokens": after_tokens})));
            }
        }
        Ok(())
    }

    fn input(
        &mut self,
        frame: &EventFrame,
        event: InputEvent,
        history: &mut Vec<ProjectedHistory>,
        effect: &mut EventEffect,
    ) -> Result<(), ProtocolError> {
        match event {
            InputEvent::Requested { pending } => {
                pending.validate()?;
                if frame
                    .event
                    .turn_id
                    .as_ref()
                    .is_some_and(|turn| turn != &pending.turn_id)
                {
                    return Err(ProtocolError::InvalidTarget);
                }
                self.active_turn = Some(pending.turn_id.clone());
                self.pending_turn = Some(pending.turn_id.clone());
                let tool_call_id = match &pending.kind {
                    PendingInputKind::User { .. } => self.question_id(&pending.turn_id),
                    PendingInputKind::Plan { approval_id, .. }
                    | PendingInputKind::Shell { approval_id, .. } => {
                        let is_plan = matches!(pending.kind, PendingInputKind::Plan { .. });
                        let approval = if is_plan {
                            ToolApproval::Plan {
                                waiting_turn: pending.turn_id.clone(),
                            }
                        } else {
                            ToolApproval::Shell
                        };
                        if let Some(tool) = self.tools.get_mut(approval_id) {
                            tool.approval = Some(approval);
                            tool.id.clone()
                        } else {
                            let id =
                                format!("direct:{}:approval:{}", self.session_id, pending.turn_id);
                            let (title, raw_input) = match &pending.kind {
                                PendingInputKind::Plan { plan, .. } => {
                                    ("Plan approval", json!({"plan":plan}))
                                }
                                PendingInputKind::Shell { request, .. } => {
                                    ("Shell approval", request.clone())
                                }
                                _ => unreachable!("approval kind"),
                            };
                            history.push(ProjectedHistory::Conversation(
                                json!({"type":"tool_call", "id":id,
                                "title":title,"status":"pending","raw_input":raw_input}),
                            ));
                            if self.tools.len() >= 512 {
                                return Err(ProtocolError::FrameTooLarge);
                            }
                            self.tools.insert(
                                approval_id.clone(),
                                OpenTool {
                                    name: if is_plan { "exit_plan_mode" } else { "bash" }.into(),
                                    id: id.clone(),
                                    approval: Some(approval),
                                    resume_turn: None,
                                },
                            );
                            id
                        }
                    }
                };
                if let PendingInputKind::User {
                    question,
                    options,
                    note,
                } = &pending.kind
                {
                    self.question_turn = Some(pending.turn_id.clone());
                    history.push(ProjectedHistory::Conversation(json!({"type": "tool_call",
                        "id": self.question_id(&pending.turn_id), "title": "Question", "kind": "other", "status": "pending",
                        "raw_input": [{"id": pending.turn_id, "header": "Question", "question": question,
                            "isOther": true, "isSecret": false,
                            "options": options.iter().map(|(label, description)| json!({"label":label, "description": description})).collect::<Vec<_>>() }],
                        "content": [{"type": "content", "content": {"type": "text", "text": note.as_deref().unwrap_or("Input required before continuing.")}}],
                        "meta": {"native_input": {"runtime_id": self.runtime_id, "session_id": self.session_id, "turn_id": pending.turn_id}}
                    })));
                } else {
                    self.question_turn = None;
                }
                history.push(run_status("waiting_permission"));
                *effect = EventEffect::InputRequested {
                    pending,
                    tool_call_id,
                };
            }
            InputEvent::Discarded {
                waiting_turn,
                reason,
            } => {
                validate_id(&waiting_turn)?;
                if self.pending_turn.as_deref() == Some(&waiting_turn) {
                    self.retire_tools(&waiting_turn, history);
                    self.pending_turn = None;
                }
                if self.question_turn.as_deref() == Some(&waiting_turn) {
                    history.push(ProjectedHistory::Conversation(
                        json!({"type":"tool_call_update", "id":self.question_id(&waiting_turn),
                        "status":"failed", "raw_output":"Input request expired."}),
                    ));
                    self.question_turn = None;
                }
                *effect = EventEffect::InputCleared {
                    waiting_turn,
                    terminal: !matches!(reason, DiscardReason::Superseded),
                };
            }
            InputEvent::Answered { waiting_turn } => {
                validate_id(&waiting_turn)?;
                // Native plan responses settle the interaction without a Tool::Result event.
                // Shell approvals instead re-emit Tool::Use before executing the original call.
                self.tools.retain(|_, tool| {
                    let plan_answered = matches!(tool.approval.as_ref(), Some(ToolApproval::Plan { waiting_turn: turn }) if turn == &waiting_turn);
                    if plan_answered {
                        history.push(ProjectedHistory::Conversation(json!({
                            "type":"tool_call_update", "id":tool.id, "status":"completed",
                            "raw_output":"Plan response received."
                        })));
                    }
                    !plan_answered
                });
                if self.pending_turn.as_deref() == Some(&waiting_turn) {
                    self.pending_turn = None;
                }
                if self.question_turn.as_deref() == Some(&waiting_turn) {
                    history.push(ProjectedHistory::Conversation(
                        json!({"type":"tool_call_update", "id":self.question_id(&waiting_turn),
                        "status":"completed", "raw_output":"Input received."}),
                    ));
                    self.question_turn = None;
                }
                *effect = EventEffect::InputCleared {
                    waiting_turn,
                    terminal: false,
                };
            }
            InputEvent::FollowUpQueued { queue_len } => history.push(update(
                json!({"event":"follow_up_queued", "queue_len":queue_len}),
            )),
            InputEvent::UserPromptSubmitted | InputEvent::PendingInputAnswered => {}
        }
        Ok(())
    }

    fn retire_tools(&mut self, turn: &str, history: &mut Vec<ProjectedHistory>) {
        if self.active_turn.as_deref() != Some(turn) {
            return;
        }
        for (_, tool) in std::mem::take(&mut self.tools) {
            history.push(ProjectedHistory::Conversation(
                json!({"type":"tool_call_update", "id":tool.id,
                "status":"failed", "raw_output":"The turn ended before this tool completed."}),
            ));
        }
        self.active_turn = None;
    }

    fn question_id(&self, turn: &str) -> String {
        format!("request-user-input:direct:{}:{turn}", self.session_id)
    }
}

impl PendingInput {
    fn validate(&self) -> Result<(), ProtocolError> {
        validate_id(&self.turn_id)?;
        match &self.kind {
            PendingInputKind::User {
                question, options, ..
            } => {
                if question.trim().is_empty() || options.len() > 64 {
                    return Err(ProtocolError::MalformedFrame);
                }
                for (label, _) in options {
                    validate_label(label)?;
                }
            }
            PendingInputKind::Plan { approval_id, .. } => validate_id(approval_id)?,
            PendingInputKind::Shell {
                approval_id,
                request,
            } => {
                validate_id(approval_id)?;
                if !request.is_object() {
                    return Err(ProtocolError::MalformedFrame);
                }
            }
        }
        Ok(())
    }
}

impl SessionSnapshot {
    fn validate(&self, session: &str, sequence: u64) -> Result<(), ProtocolError> {
        if self.session_id != session || self.last_sequence > sequence {
            return Err(ProtocolError::InvalidTarget);
        }
        if let Some(pending) = &self.pending_input {
            pending.validate()?;
        }
        match &self.phase {
            SessionPhase::AwaitingInput { turn_id } => {
                validate_id(turn_id)?;
                if self
                    .pending_input
                    .as_ref()
                    .is_none_or(|pending| &pending.turn_id != turn_id)
                {
                    return Err(ProtocolError::InvalidTarget);
                }
            }
            SessionPhase::Running { turn_id } | SessionPhase::Cancelling { turn_id } => {
                validate_id(turn_id)?
            }
            _ => {}
        }
        Ok(())
    }
}

impl SessionPhase {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::AwaitingInput { .. } => "awaiting_input",
            Self::Running { .. } => "running",
            Self::Cancelling { .. } => "cancelling",
            Self::Closing => "closing",
            Self::Closed => "closed",
        }
    }
}

fn required_turn(frame: &EventFrame) -> Result<String, ProtocolError> {
    frame
        .event
        .turn_id
        .clone()
        .ok_or(ProtocolError::InvalidTarget)
}

fn update(payload: Value) -> ProjectedHistory {
    ProjectedHistory::Conversation(json!({"type": "session_update", "payload": payload}))
}

fn run_status(status: &str) -> ProjectedHistory {
    ProjectedHistory::Conversation(json!({"type": "run_status", "status": status}))
}
