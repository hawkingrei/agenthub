use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio::io::{AsyncBufRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::{Instant, sleep_until, timeout_at};

use super::{
    Command, ConnectionError, ConnectionOptions, ConnectionStatus, OutputFrame, REQUEST_CAPACITY,
    ShutdownReceipt,
};
use crate::{
    Acknowledgement, FrameReader, Handshake, ProtocolError, ReceiptCapability, RequestResult,
    ServerFrame,
};

const MAX_PENDING: usize = 32;
const MAX_RECEIPTS: usize = 4096;

pub(super) struct Driver {
    handshake: Arc<Handshake>,
    options: ConnectionOptions,
    status: watch::Sender<ConnectionStatus>,
    pending: HashMap<String, Pending>,
    issued: HashSet<String>,
    shutdown: Option<Shutdown>,
}

struct Pending {
    deadline: Instant,
    reply: oneshot::Sender<Result<Acknowledgement, ConnectionError>>,
}

struct Shutdown {
    request_id: String,
    deadline: Instant,
    accepted: bool,
    complete: bool,
}

struct Packet {
    bytes: Vec<u8>,
    deadline: Instant,
    timeout_error: ConnectionError,
}

impl Driver {
    pub(super) fn new(
        handshake: Arc<Handshake>,
        options: ConnectionOptions,
        status: watch::Sender<ConnectionStatus>,
    ) -> Self {
        Self {
            handshake,
            options,
            status,
            pending: HashMap::new(),
            issued: HashSet::new(),
            shutdown: None,
        }
    }

    pub(super) async fn run<R, W>(
        mut self,
        mut reader: FrameReader<R>,
        mut writer: W,
        mut commands: mpsc::Receiver<Command>,
        events: mpsc::Sender<OutputFrame>,
        mut cancelled: watch::Receiver<bool>,
    ) where
        R: AsyncBufRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let (outgoing, packets) = mpsc::channel(REQUEST_CAPACITY);
        // Independent futures keep a blocked stdin write from blocking stdout reads.
        // Either failure cancels both streams; a partially written frame is never retried.
        let result = tokio::select! {
            result = self.read_loop(&mut reader, &mut commands, &events, &outgoing, &mut cancelled) => result,
            error = write_loop(&mut writer, packets) => Err(error),
        };
        drop(reader);
        drop(writer);
        self.status
            .send_replace(ConnectionStatus::Closed(result.clone()));
        let failure = result.err().unwrap_or(ConnectionError::Closing);
        for (_, pending) in self.pending.drain() {
            let _ = pending.reply.send(Err(failure));
        }
        commands.close();
        while let Ok(command) = commands.try_recv() {
            let _ = command.reply.send(Err(failure));
        }
    }

    async fn read_loop<R: AsyncBufRead + Unpin>(
        &mut self,
        reader: &mut FrameReader<R>,
        commands: &mut mpsc::Receiver<Command>,
        events: &mpsc::Sender<OutputFrame>,
        outgoing: &mpsc::Sender<Packet>,
        cancelled: &mut watch::Receiver<bool>,
    ) -> Result<ShutdownReceipt, ConnectionError> {
        loop {
            let deadline = self.deadline();
            tokio::select! {
                _ = cancelled.changed() => return Err(ConnectionError::Aborted),
                _ = events.closed() => return Err(ConnectionError::ConsumerClosed),
                error = wait_deadline(deadline) => return Err(error),
                command = commands.recv() => match command {
                    Some(command) => self.admit(command, outgoing),
                    None => return Err(ConnectionError::Aborted),
                },
                frame = reader.read_frame() => match frame? {
                    Some(frame) => {
                        if frame.runtime_id() != self.handshake.runtime_id {
                            return Err(ConnectionError::WrongRuntime);
                        }
                        if self.shutdown.as_ref().is_some_and(|s| s.complete) {
                            return Err(ConnectionError::UnexpectedFrame);
                        }
                        match frame {
                            ServerFrame::Ack(ack) => self.acknowledge(ack)?,
                            ServerFrame::Event(frame) => {
                                self.deliver(OutputFrame::Event(frame), events, cancelled).await?;
                            }
                            ServerFrame::ReplayGap(gap) => {
                                if !self.issued.contains(&gap.request_id) {
                                    return Err(ConnectionError::UnexpectedFrame);
                                }
                                self.deliver(OutputFrame::ReplayGap(gap), events, cancelled).await?;
                            }
                            ServerFrame::ShutdownComplete { request_id, .. } => {
                                let Some(shutdown) = self.shutdown.as_mut() else {
                                    return Err(ConnectionError::UnexpectedFrame);
                                };
                                if shutdown.request_id != request_id || !shutdown.accepted {
                                    return Err(ConnectionError::UnexpectedFrame);
                                }
                                shutdown.complete = true;
                            }
                            ServerFrame::Handshake(_) => return Err(ConnectionError::UnexpectedFrame),
                        }
                    }
                    None => {
                        let Some(shutdown) = &self.shutdown else {
                            return Err(ProtocolError::TransportLost.into());
                        };
                        if !shutdown.complete || !self.pending.is_empty() {
                            return Err(ProtocolError::TransportLost.into());
                        }
                        return Ok(ShutdownReceipt {
                            runtime_id: self.handshake.runtime_id.clone(),
                            request_id: shutdown.request_id.clone(),
                        });
                    }
                },
            }
        }
    }

    fn admit(&mut self, command: Command, outgoing: &mpsc::Sender<Packet>) {
        if let Err(error) = self.check_admission(&command) {
            let _ = command.reply.send(Err(error));
            return;
        }
        let packet = Packet {
            bytes: command.bytes,
            deadline: command.deadline,
            timeout_error: if command.shutdown {
                ConnectionError::ShutdownTimeout
            } else {
                ConnectionError::RequestTimeout
            },
        };
        if outgoing.try_send(packet).is_err() {
            let _ = command.reply.send(Err(ConnectionError::QueueFull));
            return;
        }
        self.issued.insert(command.request_id.clone());
        if command.shutdown {
            self.shutdown = Some(Shutdown {
                request_id: command.request_id.clone(),
                deadline: command.deadline,
                accepted: false,
                complete: false,
            });
            self.status.send_replace(ConnectionStatus::Closing);
        }
        self.pending.insert(
            command.request_id,
            Pending {
                deadline: command.deadline,
                reply: command.reply,
            },
        );
    }

    fn check_admission(&self, command: &Command) -> Result<(), ConnectionError> {
        if self.shutdown.is_some() {
            return Err(ConnectionError::Closing);
        }
        if self.issued.contains(&command.request_id) {
            return Err(ConnectionError::RequestIdReused);
        }
        if self.pending.len() >= MAX_PENDING {
            return Err(ConnectionError::QueueFull);
        }
        let ReceiptCapability::Runtime { max_requests } =
            self.handshake.capabilities.request_receipts;
        let capacity = (max_requests as usize).min(MAX_RECEIPTS);
        // Keep one peer receipt available for semantic shutdown, even at capacity.
        let allowance = if command.shutdown {
            capacity
        } else {
            capacity.saturating_sub(1)
        };
        if self.issued.len() >= allowance {
            return Err(ConnectionError::ReceiptCapacity);
        }
        if Instant::now() >= command.deadline {
            return Err(if command.shutdown {
                ConnectionError::ShutdownTimeout
            } else {
                ConnectionError::RequestTimeout
            });
        }
        Ok(())
    }

    fn acknowledge(&mut self, ack: Acknowledgement) -> Result<(), ConnectionError> {
        if !self.pending.contains_key(&ack.request_id) {
            return Err(ConnectionError::UnexpectedFrame);
        }
        if let Some(shutdown) = self.shutdown.as_mut()
            && shutdown.request_id == ack.request_id
        {
            match ack.result {
                RequestResult::Accepted {
                    session_id: None,
                    turn_id: None,
                    last_sequence: None,
                } => {
                    shutdown.accepted = true;
                }
                RequestResult::Rejected { .. } => return Err(ConnectionError::ShutdownRejected),
                _ => return Err(ConnectionError::UnexpectedFrame),
            }
        }
        let pending = self
            .pending
            .remove(&ack.request_id)
            .expect("checked pending request");
        let _ = pending.reply.send(Ok(ack));
        Ok(())
    }

    fn deadline(&self) -> Option<(Instant, ConnectionError)> {
        self.pending
            .iter()
            .map(|(id, request)| {
                let error = if self.shutdown.as_ref().is_some_and(|s| &s.request_id == id) {
                    ConnectionError::ShutdownTimeout
                } else {
                    ConnectionError::RequestTimeout
                };
                (request.deadline, error)
            })
            .chain(
                self.shutdown
                    .as_ref()
                    .map(|s| (s.deadline, ConnectionError::ShutdownTimeout)),
            )
            .min_by_key(|(deadline, _)| *deadline)
    }

    async fn deliver(
        &self,
        frame: OutputFrame,
        events: &mpsc::Sender<OutputFrame>,
        cancelled: &mut watch::Receiver<bool>,
    ) -> Result<(), ConnectionError> {
        let stall = Instant::now() + self.options.output_stall_timeout;
        let (deadline, error) = self
            .deadline()
            .filter(|(deadline, _)| *deadline <= stall)
            .unwrap_or((stall, ConnectionError::OutputStalled));
        tokio::select! {
            _ = cancelled.changed() => Err(ConnectionError::Aborted),
            result = timeout_at(deadline, events.send(frame)) => {
                result.map_err(|_| error)?.map_err(|_| ConnectionError::ConsumerClosed)
            }
        }
    }
}

async fn wait_deadline(deadline: Option<(Instant, ConnectionError)>) -> ConnectionError {
    match deadline {
        Some((deadline, error)) => {
            sleep_until(deadline).await;
            error
        }
        None => std::future::pending().await,
    }
}

async fn write_loop<W: AsyncWrite + Unpin>(
    writer: &mut W,
    mut packets: mpsc::Receiver<Packet>,
) -> ConnectionError {
    while let Some(packet) = packets.recv().await {
        let write = async {
            writer.write_all(&packet.bytes).await?;
            writer.flush().await
        };
        match timeout_at(packet.deadline, write).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return ProtocolError::TransportLost.into(),
            Err(_) => return packet.timeout_error,
        }
    }
    ConnectionError::Aborted
}
