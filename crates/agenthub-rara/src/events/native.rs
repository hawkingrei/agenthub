use serde::{Deserialize, Serialize};
use serde_json::Value;

// Payload-bearing types intentionally do not implement Debug. They are attributed
// conversation data, never safe diagnostics or control-plane authority.
#[derive(Clone, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub(super) enum NativeEvent {
    Session(SessionEvent),
    Input(InputEvent),
    Assistant(AssistantEvent),
    Tool(ToolEvent),
    Approval(ApprovalEvent),
    Plan(PlanEvent),
    Todo(TodoEvent),
    Warning(WarningEvent),
    Error(ErrorEvent),
    PromptSource(Value),
    Skill(Value),
    Mcp(Value),
    Memory(Value),
    Hook(Value),
    Context(Value),
    Extension(Value),
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub(super) enum SessionEvent {
    RuntimeState {
        snapshot: SessionSnapshot,
    },
    Created {
        session_id: String,
    },
    Resumed {
        session_id: String,
    },
    Status {
        message: String,
    },
    TurnStarted,
    TurnCancelled,
    TurnInterrupted,
    TurnFinished {
        reason: Option<String>,
    },
    TurnFailed {
        reason: String,
    },
    ModelRequest {
        model: String,
        input_tokens: u32,
    },
    ModelResponse {
        model: String,
        output_tokens: u32,
        finish_reason: Option<String>,
    },
    Compacted {
        count: usize,
        before_tokens: usize,
        after_tokens: usize,
        summary: String,
        recent_files: Vec<String>,
    },
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub phase: SessionPhase,
    pub generation: u64,
    pub last_sequence: u64,
    #[serde(default)]
    pub pending_input: Option<PendingInput>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum SessionPhase {
    Idle,
    AwaitingInput { turn_id: String },
    Running { turn_id: String },
    Cancelling { turn_id: String },
    Closing,
    Closed,
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct PendingInput {
    pub turn_id: String,
    pub kind: PendingInputKind,
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum PendingInputKind {
    User {
        question: String,
        options: Vec<(String, String)>,
        note: Option<String>,
    },
    Plan {
        approval_id: String,
        plan: String,
    },
    Shell {
        approval_id: String,
        request: Value,
    },
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub(super) enum InputEvent {
    Requested {
        pending: PendingInput,
    },
    Discarded {
        waiting_turn: String,
        reason: DiscardReason,
    },
    Answered {
        waiting_turn: String,
    },
    UserPromptSubmitted,
    FollowUpQueued {
        queue_len: u32,
    },
    PendingInputAnswered,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DiscardReason {
    Cancelled,
    Interrupted,
    Shutdown,
    Superseded,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub(super) enum AssistantEvent {
    Text(String),
    TextDelta(String),
    ThinkingDelta(String),
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub(super) enum ToolEvent {
    Use {
        call_id: Option<String>,
        name: String,
        input: Value,
    },
    Result {
        call_id: Option<String>,
        name: String,
        content: String,
        is_error: bool,
    },
    Progress {
        call_id: Option<String>,
        name: String,
        stream: ToolStream,
        chunk: String,
    },
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ToolStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub(super) enum ApprovalEvent {
    Requested { approval_id: String, kind: String },
    Answered { approval_id: String, approved: bool },
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub(super) enum PlanEvent {
    Updated {
        steps: Vec<PlanStep>,
        explanation: Option<String>,
    },
    Approved,
    Continued,
}

#[derive(Clone, Deserialize)]
pub(super) struct PlanStep {
    pub step: String,
    pub status: StepStatus,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StepStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub(super) enum TodoEvent {
    Updated { state: TodoState },
}

#[derive(Clone, Deserialize)]
pub(super) struct TodoState {
    pub version: u32,
    pub items: Vec<TodoItem>,
    pub updated_at: i64,
}

#[derive(Clone, Deserialize)]
pub(super) struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub(super) enum WarningEvent {
    RuntimeWarning { message: String },
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub(super) enum ErrorEvent {
    RuntimeError { message: String, recoverable: bool },
}
