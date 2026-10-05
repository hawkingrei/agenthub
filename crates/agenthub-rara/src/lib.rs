//! Direct, bounded runtime-control transport. Process ownership stays with the supervisor.

mod connection;
mod control;
mod events;
mod framing;
mod handshake;
mod launch;
mod protocol;
mod semantic_guard;
mod source;

pub use connection::{
    Client, Connection, ConnectionError, ConnectionOptions, ConnectionStatus, OutputFrame,
    ShutdownReceipt,
};
pub use control::{ControlKind, ControlRequest, InputTarget, PlanDecision, ShellDecision};
pub use events::{
    EventEffect, EventProjection, EventProjector, PendingInput, PendingInputKind, ProjectedHistory,
    SessionPhase, SessionSnapshot, TurnEnd,
};
pub use framing::{FrameReader, encode_request};
pub use handshake::{Capabilities, Handshake, ReceiptCapability, ReplayCapability};
pub use launch::LaunchCommand;
pub use protocol::{
    Acknowledgement, ClientFrame, ControlEnvelope, EventFrame, ProtocolError, Provenance,
    RejectionCode, ReplayGap, RequestResult, RuntimeEvent, ServerFrame,
};
pub use semantic_guard::{
    GuardedPrompt, SemanticGuardContext, SemanticGuardDecision, SemanticGuardEvent,
    SemanticGuardFailure,
};
pub use source::{McpSource, SourceRegistration};

pub const PROTOCOL_VERSION: u32 = 1;
pub const TRANSPORT: &str = "stdio-jsonl";
pub const MAX_FRAME_BYTES: usize = 1_048_576;
pub const PINNED_UPSTREAM_REVISION: &str = "5585583a6674cb0d6ac36d5be98744dd75589871";

#[cfg(test)]
mod tests;
