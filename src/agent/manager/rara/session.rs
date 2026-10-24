use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
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

mod controls;
mod input;
mod permissions;
mod reconciliation;
mod recovery;
mod recovery_query;
mod semantic_guard;
mod sources;

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
    ack_cursor: Arc<AtomicU64>,
    progress: watch::Sender<u64>,
    permissions: Arc<agenthub_acp::AcpPermissionService>,
    permission: Arc<Mutex<Option<permissions::LivePermission>>>,
    state: Arc<RwLock<LiveState>>,
    delivery: watch::Sender<Option<bool>>,
    loop_owner: Option<recovery::LoopOwner>,
    standalone_owner: Option<super::standalone::StandaloneOwner>,
}

struct LiveState {
    phase: SessionPhase,
    pending: Option<PendingInput>,
    answered_user_turn: Option<String>,
    sequence: u64,
    sources_registered: bool,
    terminal_turn: bool,
    input_attempted: bool,
    guard: Option<semantic_guard::GuardedActivation>,
    recovery: Option<agenthub_rara::RecoveryStatus>,
    recovery_sequence: u64,
    recovery_reconciled: bool,
    pending_tool_call: Option<String>,
    entry_ready: bool,
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
    pub(super) async fn open(
        manager: &AgentManager,
        client: Client,
        store: RuntimeEventStore,
        agent_id: &str,
        output_tx: broadcast::Sender<AgentOutput>,
        config: &agenthub_config::RaraLaunchConfig,
        workspace: &std::path::Path,
    ) -> anyhow::Result<Self> {
        let opening = manager
            .begin_native_conversation(
                agent_id,
                store.local_session_id(),
                config,
                client.handshake(),
                workspace,
            )
            .await?;
        let ack = receipts::control(
            &manager.daemon_tasks,
            &client,
            &store,
            None,
            opening.request,
        )
        .await?;
        let RuntimeRequestAck::Accepted {
            session_id,
            last_sequence,
            ..
        } = ack
        else {
            anyhow::bail!("direct runtime rejected conversation opening");
        };
        let stream = store
            .stream(&session_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("direct runtime stream ownership is missing"))?;
        if let Some(reservation) = &opening.loop_owner {
            agenthub_db::loop_runtime::LoopStore::new(manager.db.clone())
                .bind_native_session(reservation, &session_id, chrono::Utc::now().timestamp())
                .await?;
        }
        if let Some(owner) = &opening.standalone_owner {
            agenthub_db::native_sessions::NativeSessionStore::new(manager.db.clone())
                .bind_conversation(owner, &session_id, chrono::Utc::now().timestamp())
                .await?;
        }
        let (delivery, _) = watch::channel(None);
        let (progress, _) = watch::channel(0);
        let loop_owner = if let Some(reservation) = opening.loop_owner {
            Some(recovery::LoopOwner {
                store: agenthub_db::loop_runtime::LoopStore::new(manager.db.clone()),
                reservation,
                operations: manager.loop_operation_gate(agent_id).await,
            })
        } else {
            None
        };
        let standalone_owner = if let Some(reservation) = opening.standalone_owner {
            Some(super::standalone::StandaloneOwner {
                store: agenthub_db::native_sessions::NativeSessionStore::new(manager.db.clone()),
                reservation,
                operations: manager.loop_operation_gate(agent_id).await,
            })
        } else {
            None
        };
        let entry_ready = loop_owner.is_none();
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
            ack_cursor: Arc::new(AtomicU64::new(last_sequence.unwrap_or(0))),
            progress,
            permissions: manager.permissions.clone(),
            permission: Arc::new(Mutex::new(None)),
            state: Arc::new(RwLock::new(LiveState {
                phase: SessionPhase::Idle,
                pending: None,
                answered_user_turn: None,
                sequence: 0,
                sources_registered: false,
                terminal_turn: false,
                input_attempted: false,
                guard: None,
                recovery: None,
                recovery_reconciled: false,
                recovery_sequence: 0,
                pending_tool_call: None,
                entry_ready,
            })),
            delivery,
            loop_owner,
            standalone_owner,
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
                            anyhow::ensure!(matches!(receipt.kind, RuntimeRequestKind::CreateSession | RuntimeRequestKind::ResumeSession) || replay.as_ref().is_some_and(|r| r.id == gap.request_id && r.after == gap.requested_after), "direct replay gap has an invalid request");
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
                        self.apply_effect(committed.effect, committed.sequence).await?;
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

    async fn apply_effect(&self, effect: EventEffect, sequence: u64) -> anyhow::Result<()> {
        let mut interaction = None;
        let mut state = self.state.write().await;
        state.sequence = sequence;
        match effect {
            EventEffect::Snapshot(snapshot) => {
                if state.pending != snapshot.pending_input {
                    state.pending_tool_call = None;
                }
                state.phase = snapshot.phase;
                state.pending = snapshot.pending_input;
            }
            EventEffect::Recovery(recovery) => {
                if let Some(blocked) = &recovery.blocked {
                    anyhow::ensure!(
                        state.pending.is_none(),
                        "native recovery conflicts with pending input"
                    );
                    state.phase = SessionPhase::RecoveryRequired {
                        recovery_id: blocked.recovery_id.clone(),
                    };
                } else if matches!(state.phase, SessionPhase::RecoveryRequired { .. }) {
                    state.phase = SessionPhase::Idle;
                }
                anyhow::ensure!(
                    recovery.waiting_turn_id.as_deref()
                        == state
                            .pending
                            .as_ref()
                            .map(|pending| pending.turn_id.as_str()),
                    "native recovery waiting identity changed"
                );
                state.recovery = Some(recovery);
                state.recovery_sequence = sequence;
            }
            EventEffect::Reentry(evaluation) => {
                anyhow::ensure!(
                    recovery::matches_target(&evaluation.target, &state),
                    "native reentry result targets a changed session phase"
                );
                let guard = state.guard.as_mut().ok_or_else(|| {
                    anyhow::anyhow!("native reentry has no configured activation")
                })?;
                anyhow::ensure!(
                    evaluation.origin.request_id == guard.request_id,
                    "native reentry request changed"
                );
                let check = guard
                    .reentry
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("native reentry was not requested"))?;
                anyhow::ensure!(
                    check.target == evaluation.target && check.evaluation.is_none(),
                    "native reentry target or result changed"
                );
                check.evaluation = Some((sequence, evaluation.result));
            }
            EventEffect::TurnStarted { turn_id } => {
                if let Some(guard) = &mut state.guard {
                    guard.completed = None;
                }
                state.terminal_turn = false;
                state.phase = SessionPhase::Running { turn_id };
            }
            EventEffect::TurnEnded {
                turn_id,
                outcome,
                semantic,
            } => {
                if let Some(guard) = &mut state.guard {
                    guard.completed = semantic.map(|decision| (turn_id.clone(), decision));
                }
                let awaiting_input = matches!(outcome, agenthub_rara::TurnEnd::Finished { reason: Some(ref reason) } if reason == "awaiting_input");
                if !awaiting_input {
                    state.terminal_turn = true;
                }
                if !awaiting_input
                    && state
                        .pending
                        .as_ref()
                        .is_some_and(|pending| pending.turn_id == turn_id)
                {
                    state.pending = None;
                }
                if let Some(pending) = &state.pending {
                    state.phase = SessionPhase::AwaitingInput {
                        turn_id: pending.turn_id.clone(),
                    };
                } else if matches!(&state.phase, SessionPhase::Running { turn_id: active } | SessionPhase::Cancelling { turn_id: active } | SessionPhase::AwaitingInput { turn_id: active } if active == &turn_id)
                {
                    state.phase = SessionPhase::Idle;
                }
            }
            EventEffect::InputRequested {
                pending,
                tool_call_id,
            } => {
                state.pending_tool_call = Some(tool_call_id.clone());
                if state.entry_ready {
                    interaction = Some((pending.clone(), tool_call_id));
                }
                state.terminal_turn = false;
                state.phase = SessionPhase::AwaitingInput {
                    turn_id: pending.turn_id.clone(),
                };
                state.pending = Some(pending);
            }
            EventEffect::InputCleared {
                waiting_turn,
                terminal,
            } if state
                .pending
                .as_ref()
                .is_some_and(|p| p.turn_id == waiting_turn) =>
            {
                state.pending = None;
                state.pending_tool_call = None;
                state.terminal_turn = terminal;
                if matches!(&state.phase, SessionPhase::AwaitingInput { turn_id } if turn_id == &waiting_turn)
                {
                    state.phase = SessionPhase::Idle;
                }
            }
            _ => {}
        }
        let pending = state.pending.clone();
        drop(state);
        self.expire_obsolete_permission(pending.as_ref()).await?;
        if let Some((pending, tool_call_id)) = interaction {
            self.request_permission(pending, tool_call_id).await?;
        }
        self.progress.send_replace(sequence);
        Ok(())
    }

    fn record_ack_cursor(&self, ack: &RuntimeRequestAck) {
        if let RuntimeRequestAck::Accepted {
            last_sequence: Some(sequence),
            ..
        } = ack
        {
            self.ack_cursor.fetch_max(*sequence, Ordering::AcqRel);
        }
    }

    /// ACKs may arrive before their events. Route the next input/control only after
    /// those events commit, without treating an ACK cursor as persisted history.
    async fn await_admitted_events(&self) -> anyhow::Result<()> {
        let cursor = self.ack_cursor.load(Ordering::Acquire);
        let mut progress = self.progress.subscribe();
        let result = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                if *progress.borrow_and_update() >= cursor { return Ok(()); }
                tokio::select! {
                    _ = self.client.closed() => anyhow::bail!("direct runtime closed before admitted events committed"),
                    changed = progress.changed() => changed?,
                }
            }
        }).await;
        match result {
            Ok(result) => result,
            Err(_) => {
                self.client.abort();
                anyhow::bail!("direct runtime admitted events did not commit")
            }
        }
    }
}
