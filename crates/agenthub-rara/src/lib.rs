//! Direct, bounded runtime-control transport. Process ownership stays with the supervisor.

mod connection;
mod framing;
mod handshake;
mod launch;
mod protocol;

pub use connection::{
    Client, Connection, ConnectionError, ConnectionOptions, ConnectionStatus, OutputFrame,
    ShutdownReceipt,
};
pub use framing::{FrameReader, encode_request};
pub use handshake::{Capabilities, Handshake, ReceiptCapability, ReplayCapability};
pub use launch::LaunchCommand;
pub use protocol::{
    Acknowledgement, ClientFrame, ControlEnvelope, EventFrame, ProtocolError, Provenance,
    RejectionCode, ReplayGap, RequestResult, RuntimeEvent, ServerFrame,
};

pub const PROTOCOL_VERSION: u32 = 1;
pub const TRANSPORT: &str = "stdio-jsonl";
pub const MAX_FRAME_BYTES: usize = 1_048_576;
pub const PINNED_UPSTREAM_REVISION: &str = "6f489462251b73e1695bb22a59d2ece59ba26a21";

#[cfg(test)]
mod tests;
