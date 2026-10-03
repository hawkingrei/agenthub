use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::{Instant, timeout};

use crate::{
    Acknowledgement, ClientFrame, EventFrame, FrameReader, Handshake, ProtocolError, ReplayGap,
    RequestResult, ServerFrame, encode_request,
};

mod driver;
#[cfg(test)]
mod process_tests;
#[cfg(test)]
mod tests;

const REQUEST_CAPACITY: usize = 8;
const OUTPUT_CAPACITY: usize = 32;

#[derive(Clone, Copy, Debug)]
pub struct ConnectionOptions {
    pub startup_timeout: Duration,
    pub request_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub output_stall_timeout: Duration,
}

impl Default for ConnectionOptions {
    fn default() -> Self {
        Self {
            startup_timeout: Duration::from_secs(120),
            request_timeout: Duration::from_secs(30),
            shutdown_timeout: Duration::from_secs(30),
            output_stall_timeout: Duration::from_secs(5),
        }
    }
}

impl ConnectionOptions {
    fn validate(self) -> Result<(), ConnectionError> {
        for duration in [
            self.startup_timeout,
            self.request_timeout,
            self.shutdown_timeout,
            self.output_stall_timeout,
        ] {
            if duration.is_zero() || duration > Duration::from_secs(600) {
                return Err(ConnectionError::InvalidOptions);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConnectionError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error("runtime-control connection options are invalid")]
    InvalidOptions,
    #[error("runtime-control startup timed out")]
    StartupTimeout,
    #[error("runtime-control request timed out; its outcome is unknown")]
    RequestTimeout,
    #[error("runtime-control shutdown did not complete before its deadline")]
    ShutdownTimeout,
    #[error("runtime-control request queue is full")]
    QueueFull,
    #[error("runtime-control receipt capacity is exhausted")]
    ReceiptCapacity,
    #[error("runtime-control request identity was already used on this connection")]
    RequestIdReused,
    #[error("runtime-control request belongs to another runtime")]
    WrongRuntime,
    #[error("runtime-control method is not supported by this connection")]
    UnsupportedMethod,
    #[error("runtime-control received an unexpected or uncorrelated frame")]
    UnexpectedFrame,
    #[error("runtime-control connection is closing")]
    Closing,
    #[error("runtime-control connection was aborted")]
    Aborted,
    #[error("runtime-control event consumer closed")]
    ConsumerClosed,
    #[error("runtime-control event consumer stalled; delivery is incomplete")]
    OutputStalled,
    #[error("runtime-control shutdown was rejected")]
    ShutdownRejected,
}

/// Confirms the correlated completion frame followed by clean stdout EOF only.
/// The process supervisor must independently verify exit and descendant cleanup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShutdownReceipt {
    pub runtime_id: String,
    pub request_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionStatus {
    Running,
    Closing,
    Closed(Result<ShutdownReceipt, ConnectionError>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum OutputFrame {
    Event(EventFrame),
    ReplayGap(ReplayGap),
}

/// Owns streams only. Dropping all clients or the output receiver closes them.
pub struct Connection {
    pub client: Client,
    pub output: mpsc::Receiver<OutputFrame>,
}

impl Connection {
    pub async fn open<R, W>(
        input: R,
        output: W,
        options: ConnectionOptions,
    ) -> Result<Self, ConnectionError>
    where
        R: AsyncBufRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        options.validate()?;
        let mut reader = FrameReader::new(input);
        let first = timeout(options.startup_timeout, reader.next())
            .await
            .map_err(|_| ConnectionError::StartupTimeout)??;
        let ServerFrame::Handshake(handshake) = first else {
            return Err(ConnectionError::UnexpectedFrame);
        };
        let handshake = Arc::new(handshake);
        let (commands, incoming) = mpsc::channel(REQUEST_CAPACITY);
        let (events, output_events) = mpsc::channel(OUTPUT_CAPACITY);
        let (status, observed) = watch::channel(ConnectionStatus::Running);
        let (abort, cancelled) = watch::channel(false);
        tokio::spawn(
            driver::Driver::new(handshake.clone(), options, status)
                .run(reader, output, incoming, events, cancelled),
        );
        Ok(Self {
            client: Client {
                handshake,
                commands,
                status: observed,
                abort,
                options,
            },
            output: output_events,
        })
    }
}

#[derive(Clone)]
pub struct Client {
    handshake: Arc<Handshake>,
    commands: mpsc::Sender<Command>,
    status: watch::Receiver<ConnectionStatus>,
    abort: watch::Sender<bool>,
    options: ConnectionOptions,
}

impl Client {
    pub fn handshake(&self) -> &Handshake {
        &self.handshake
    }

    pub fn status(&self) -> ConnectionStatus {
        self.status.borrow().clone()
    }

    /// No implicit retry. Once queued, cancellation does not retract an operation.
    /// An ACK is admission, not completion of its resulting work or event delivery.
    pub async fn request(&self, frame: ClientFrame) -> Result<Acknowledgement, ConnectionError> {
        match self.status() {
            ConnectionStatus::Running => {}
            ConnectionStatus::Closing | ConnectionStatus::Closed(Ok(_)) => {
                return Err(ConnectionError::Closing);
            }
            ConnectionStatus::Closed(Err(error)) => return Err(error),
        }
        if frame.runtime_id() != self.handshake.runtime_id {
            return Err(ConnectionError::WrongRuntime);
        }
        if !self.handshake.supports(method(&frame)?) {
            return Err(ConnectionError::UnsupportedMethod);
        }
        let permit = self.commands.try_reserve().map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => ConnectionError::QueueFull,
            mpsc::error::TrySendError::Closed(_) => ConnectionError::Closing,
        })?;
        let bytes = encode_request(&frame)?;
        let shutdown = matches!(frame, ClientFrame::Shutdown { .. });
        let duration = if shutdown {
            self.options.shutdown_timeout
        } else {
            self.options.request_timeout
        };
        let (reply, response) = oneshot::channel();
        let deadline = Instant::now() + duration;
        permit.send(Command {
            request_id: frame.request_id().into(),
            bytes,
            shutdown,
            deadline,
            reply,
        });
        // The driver owns the deadline even if this awaiting caller is cancelled.
        match tokio::time::timeout_at(deadline, response).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(self
                .closed()
                .await
                .err()
                .unwrap_or(ConnectionError::Closing)),
            Err(_) => {
                self.abort();
                Err(if shutdown {
                    ConnectionError::ShutdownTimeout
                } else {
                    ConnectionError::RequestTimeout
                })
            }
        }
    }

    pub async fn shutdown(&self, request_id: String) -> Result<ShutdownReceipt, ConnectionError> {
        let ack = self
            .request(ClientFrame::Shutdown {
                runtime_id: self.handshake.runtime_id.clone(),
                request_id: request_id.clone(),
            })
            .await?;
        if !matches!(ack.result, RequestResult::Accepted { .. }) {
            return Err(ConnectionError::ShutdownRejected);
        }
        let receipt = self.closed().await?;
        if receipt.request_id != request_id {
            return Err(ConnectionError::UnexpectedFrame);
        }
        Ok(receipt)
    }

    /// Cancels stream I/O. The owner must still stop the supervised child.
    pub fn abort(&self) {
        self.abort.send_replace(true);
    }

    pub async fn closed(&self) -> Result<ShutdownReceipt, ConnectionError> {
        let mut status = self.status.clone();
        loop {
            if let ConnectionStatus::Closed(result) = status.borrow_and_update().clone() {
                return result;
            }
            if status.changed().await.is_err() {
                return Err(ProtocolError::TransportLost.into());
            }
        }
    }
}

struct Command {
    request_id: String,
    bytes: Vec<u8>,
    shutdown: bool,
    deadline: Instant,
    reply: oneshot::Sender<Result<Acknowledgement, ConnectionError>>,
}

fn method(frame: &ClientFrame) -> Result<&'static str, ConnectionError> {
    let ClientFrame::Control { envelope, .. } = frame else {
        return Ok(match frame {
            ClientFrame::Replay { .. } => "output.replay",
            ClientFrame::Shutdown { .. } => "server.shutdown",
            ClientFrame::Control { .. } => unreachable!(),
        });
    };
    let family = envelope.request["type"].as_str();
    let operation = envelope.request["payload"]["type"].as_str();
    match (family, operation) {
        (Some("session"), Some("create_session")) => Ok("session.create"),
        (Some("session"), Some("query_runtime_state")) => Ok("session.query_state"),
        (Some("session"), Some("cancel_current_turn")) => Ok("session.cancel"),
        (Some("session"), Some("interrupt_current_turn")) => Ok("session.interrupt"),
        (Some("input"), Some("submit_user_prompt")) => Ok("input.submit_prompt"),
        (Some("input"), Some("submit_follow_up")) => Ok("input.submit_follow_up"),
        (Some("input"), Some("answer_pending_input")) => Ok("input.answer_user"),
        (Some("input"), Some("answer_plan_approval")) => Ok("input.answer_plan"),
        (Some("input"), Some("answer_shell_approval")) => Ok("input.answer_shell"),
        (Some("prompt_source"), Some("register")) => Ok("prompt_source.register"),
        (Some("prompt_source"), Some("unregister")) => Ok("prompt_source.unregister"),
        (Some("prompt_source"), Some("query_sources")) => Ok("prompt_source.query"),
        (Some("skill_source"), Some("register_skill")) => Ok("skill_source.register"),
        (Some("skill_source"), Some("disable_skill")) => Ok("skill_source.disable"),
        (Some("skill_source"), Some("query_skills")) => Ok("skill_source.query"),
        _ => Err(ConnectionError::UnsupportedMethod),
    }
}
