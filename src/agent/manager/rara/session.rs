use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use agenthub_db::runtime_events::{
    RuntimeEventStore, RuntimeEventStream, RuntimeReplayGap, RuntimeRequestAck, RuntimeRequestKind,
};
use agenthub_rara::{
    Client, ClientFrame, ConnectionError, ControlRequest, EventEffect, OutputFrame, PendingInput,
    SessionPhase, ShutdownReceipt,
};
use tokio::sync::{Mutex, RwLock, broadcast, mpsc, watch};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

mod input;

use super::{AgentManager, events::DurableEvents, receipts};
use crate::agent::AgentOutput;
use crate::daemon_tasks::DaemonTaskGroup;

#[derive(Clone)]
pub struct RaraHandle {
    client: Client,
    store: RuntimeEventStore,
    stream: RuntimeEventStream,
    agent_id: String,
    tasks: DaemonTaskGroup,
    output_tx: broadcast::Sender<AgentOutput>,
    idle_gc: Option<agenthub_db::AgentEventIdleGc>,
    event_dbs: agenthub_db::AgentEventDbRouter,
    input_gate: Arc<Mutex<()>>,
    state: Arc<RwLock<LiveState>>,
    delivery: watch::Sender<Option<bool>>,
}

struct LiveState {
    phase: SessionPhase,
    pending: Option<PendingInput>,
    sequence: u64,
}

struct Replay {
    id: String,
    after: u64,
    target: Option<u64>,
    deadline: Instant,
}

impl Deref for RaraHandle {
    type Target = Client;
    fn deref(&self) -> &Client {
        &self.client
    }
}

impl RaraHandle {
    pub(super) async fn create(
        manager: &AgentManager,
        client: Client,
        store: RuntimeEventStore,
        agent_id: &str,
        output_tx: broadcast::Sender<AgentOutput>,
    ) -> anyhow::Result<Self> {
        let ack = receipts::control(
            &manager.daemon_tasks,
            &client,
            &store,
            None,
            ControlRequest::CreateSession,
        )
        .await?;
        let RuntimeRequestAck::Accepted { session_id, .. } = ack else {
            anyhow::bail!("direct runtime rejected session creation");
        };
        let stream = store
            .stream(&session_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("direct runtime stream ownership is missing"))?;
        let (delivery, _) = watch::channel(None);
        Ok(Self {
            client,
            store,
            stream,
            agent_id: agent_id.into(),
            tasks: manager.daemon_tasks.clone(),
            output_tx,
            idle_gc: manager.idle_gc.clone(),
            event_dbs: manager.event_dbs.clone(),
            input_gate: Arc::new(Mutex::new(())),
            state: Arc::new(RwLock::new(LiveState {
                phase: SessionPhase::Idle,
                pending: None,
                sequence: 0,
            })),
            delivery,
        })
    }

    /// Semantic shutdown also needs proof that received history finished committing.
    pub async fn closed(&self) -> Result<ShutdownReceipt, ConnectionError> {
        let receipt = self.client.closed().await?;
        let mut delivery = self.delivery.subscribe();
        loop {
            match *delivery.borrow_and_update() {
                Some(true) => return Ok(receipt),
                Some(false) => return Err(ConnectionError::ConsumerClosed),
                None => {}
            }
            delivery
                .changed()
                .await
                .map_err(|_| ConnectionError::ConsumerClosed)?;
        }
    }

    pub(super) fn finish_delivery(&self, success: bool) {
        self.delivery.send_replace(Some(success));
    }

    pub(super) async fn consume(
        &self,
        mut output: mpsc::Receiver<OutputFrame>,
        cancellation: CancellationToken,
    ) -> anyhow::Result<()> {
        let mut events = DurableEvents::new(
            self.stream.clone(),
            self.store.runtime_id(),
            &self.agent_id,
            self.store.local_session_id(),
        )
        .await?;
        let (replies, mut replay_replies) = mpsc::channel(1);
        let mut replay: Option<Replay> = None;
        loop {
            let deadline = replay
                .as_ref()
                .map(|r| r.deadline)
                .unwrap_or_else(|| Instant::now() + Duration::from_secs(30));
            tokio::select! {
                _ = cancellation.cancelled() => anyhow::bail!("direct runtime event delivery was interrupted"),
                _ = tokio::time::sleep_until(deadline), if replay.is_some() => anyhow::bail!("direct runtime replay did not complete"),
                response = replay_replies.recv() => {
                    let (id, response): (String, anyhow::Result<RuntimeRequestAck>) = response.ok_or_else(|| anyhow::anyhow!("direct replay receipt task stopped"))?;
                    if replay.as_ref().is_none_or(|active| active.id != id) { continue; }
                    match response? {
                        RuntimeRequestAck::Accepted { last_sequence: Some(target), .. } => {
                            let active = replay.as_mut().expect("active replay");
                            anyhow::ensure!(target > active.after, "direct replay did not contain missing events");
                            active.target = Some(target);
                        }
                        // The pinned peer sends its explicit gap immediately after rejection.
                        RuntimeRequestAck::Rejected { .. } => {}
                        _ => anyhow::bail!("direct replay returned an invalid receipt"),
                    }
                }
                frame = output.recv() => {
                    let Some(frame) = frame else {
                        anyhow::ensure!(!events.missing_prefix(), "direct runtime closed with incomplete history");
                        return Ok(());
                    };
                    match frame {
                        OutputFrame::Event(frame) => events.enqueue(frame).await?,
                        OutputFrame::ReplayGap(gap) => {
                            anyhow::ensure!(gap.runtime_id == self.store.runtime_id() && gap.session_id == self.stream.native_session_id(), "direct replay gap belongs to another session");
                            let receipt = self.store.request_receipt(&gap.request_id).await?.ok_or_else(|| anyhow::anyhow!("direct replay gap has no control receipt"))?;
                            anyhow::ensure!(receipt.kind == RuntimeRequestKind::CreateSession || replay.as_ref().is_some_and(|r| r.id == gap.request_id && r.after == gap.requested_after), "direct replay gap has an invalid request");
                            let after = events.sequence();
                            if after + 1 < gap.oldest_available || after > gap.latest {
                                self.stream.record_replay_gap(RuntimeReplayGap { requested_after: after, oldest_available: gap.oldest_available, latest: gap.latest }).await?;
                                anyhow::bail!("direct runtime replay history is unavailable");
                            }
                            // Live delivery may already have committed the requested prefix.
                            replay = None;
                        }
                    }
                    while let Some(committed) = events.commit_next().await? {
                        self.apply_effect(committed.effect, committed.sequence).await;
                        if let Some(idle_gc) = &self.idle_gc { idle_gc.record_activity(&self.agent_id).await; }
                        for entry in committed.output { let _ = self.output_tx.send(entry); }
                    }
                }
            }
            if replay
                .as_ref()
                .and_then(|r| r.target)
                .is_some_and(|target| events.sequence() >= target)
            {
                replay = None;
            }
            if replay.is_none() && events.missing_prefix() {
                anyhow::ensure!(
                    self.client.handshake().supports("output.replay"),
                    "direct runtime cannot replay missing history"
                );
                let id = Uuid::now_v7().to_string();
                let after = events.sequence();
                let frame = ClientFrame::Replay {
                    runtime_id: self.store.runtime_id().into(),
                    request_id: id.clone(),
                    session_id: self.stream.native_session_id().into(),
                    after_sequence: after,
                };
                let client = self.client.clone();
                let store = self.store.clone();
                let session = self.stream.native_session_id().to_owned();
                let reply = replies.clone();
                let reply_id = id.clone();
                self.tasks
                    .spawn_runtime_task(format!("direct-replay:{id}"), async move {
                        let result = receipts::submit(
                            &client,
                            &store,
                            frame,
                            RuntimeRequestKind::Replay,
                            Some(&session),
                            None,
                        )
                        .await;
                        let _ = reply.send((reply_id, result)).await;
                        Ok(())
                    })?;
                replay = Some(Replay {
                    id,
                    after,
                    target: None,
                    deadline: Instant::now() + Duration::from_secs(30),
                });
            }
        }
    }

    async fn apply_effect(&self, effect: EventEffect, sequence: u64) {
        let mut state = self.state.write().await;
        state.sequence = sequence;
        match effect {
            EventEffect::Snapshot(snapshot) => {
                state.phase = snapshot.phase;
                state.pending = snapshot.pending_input;
            }
            EventEffect::TurnStarted { turn_id } => state.phase = SessionPhase::Running { turn_id },
            EventEffect::TurnEnded { turn_id, .. } => {
                if let Some(pending) = &state.pending {
                    state.phase = SessionPhase::AwaitingInput {
                        turn_id: pending.turn_id.clone(),
                    };
                } else if matches!(&state.phase, SessionPhase::Running { turn_id: active } | SessionPhase::Cancelling { turn_id: active } if active == &turn_id)
                {
                    state.phase = SessionPhase::Idle;
                }
            }
            EventEffect::InputRequested(pending) => {
                state.phase = SessionPhase::AwaitingInput {
                    turn_id: pending.turn_id.clone(),
                };
                state.pending = Some(pending);
            }
            EventEffect::InputCleared { waiting_turn }
                if state
                    .pending
                    .as_ref()
                    .is_some_and(|p| p.turn_id == waiting_turn) =>
            {
                state.pending = None;
                if matches!(&state.phase, SessionPhase::AwaitingInput { turn_id } if turn_id == &waiting_turn)
                {
                    state.phase = SessionPhase::Idle;
                }
            }
            _ => {}
        }
    }
}
