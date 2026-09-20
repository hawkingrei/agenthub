use std::collections::BTreeMap;

use agenthub_agent_event_codec::encode_message_for_storage;
use agenthub_db::runtime_events::{
    RuntimeEventIdentity, RuntimeEventStream, RuntimeHistoryEntry, RuntimePersistResult,
};
use agenthub_rara::{EventEffect, EventFrame, EventProjector, ProjectedHistory};
use chrono::Utc;
use uuid::Uuid;

use crate::agent::{AgentOutput, OutputStream};

const MAX_REORDER_EVENTS: usize = 256;
const MAX_REORDER_BYTES: usize = 8 * 1024 * 1024;

#[cfg(test)]
mod tests;

pub(super) struct DurableEvents {
    stream: RuntimeEventStream,
    runtime_id: String,
    agent_id: String,
    local_session_id: String,
    projector: EventProjector,
    sequence: u64,
    waiting: BTreeMap<u64, (EventFrame, usize)>,
    waiting_bytes: usize,
}

pub(super) struct CommittedEvent {
    pub effect: EventEffect,
    pub output: Vec<AgentOutput>,
    pub sequence: u64,
}

impl DurableEvents {
    pub(super) async fn new(
        stream: RuntimeEventStream,
        runtime_id: &str,
        agent_id: &str,
        local_session_id: &str,
    ) -> anyhow::Result<Self> {
        let cursor = stream.cursor().await?;
        anyhow::ensure!(
            cursor.gap.is_none(),
            "direct history has an unrecoverable replay gap"
        );
        // A new process cannot reconstruct live presentation state from a cursor alone.
        // Reattach to this same consumer for a live transport; cross-process resume is gated.
        anyhow::ensure!(
            cursor.sequence == 0,
            "direct projection cannot resume across process lifetimes"
        );
        Ok(Self {
            projector: EventProjector::new(runtime_id, stream.native_session_id())?,
            stream,
            runtime_id: runtime_id.into(),
            agent_id: agent_id.into(),
            local_session_id: local_session_id.into(),
            sequence: 0,
            waiting: BTreeMap::new(),
            waiting_bytes: 0,
        })
    }

    pub(super) fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(super) fn missing_prefix(&self) -> bool {
        self.waiting
            .first_key_value()
            .is_some_and(|(sequence, _)| *sequence > self.sequence + 1)
    }

    pub(super) async fn enqueue(&mut self, frame: EventFrame) -> anyhow::Result<()> {
        anyhow::ensure!(
            frame.runtime_id == self.runtime_id
                && frame.session_id == self.stream.native_session_id(),
            "direct event belongs to another runtime session"
        );
        let fingerprint = frame.fingerprint()?;
        let sequence = frame.event.sequence;
        if sequence <= self.sequence {
            let result = self
                .stream
                .persist(
                    RuntimeEventIdentity {
                        event_id: &frame.event.event_id,
                        sequence,
                        fingerprint: &fingerprint,
                    },
                    &[],
                )
                .await?;
            anyhow::ensure!(
                matches!(result, RuntimePersistResult::Duplicate),
                "direct replay did not match committed history"
            );
            return Ok(());
        }
        if let Some((previous, _)) = self.waiting.get(&sequence) {
            anyhow::ensure!(
                previous.fingerprint()? == fingerprint,
                "direct buffered event identity conflicts"
            );
            return Ok(());
        }
        let size = serde_json::to_vec(&frame)?.len();
        anyhow::ensure!(
            size <= agenthub_rara::MAX_FRAME_BYTES
                && self.waiting.len() < MAX_REORDER_EVENTS
                && self.waiting_bytes + size <= MAX_REORDER_BYTES,
            "direct event reorder buffer is full"
        );
        self.waiting_bytes += size;
        self.waiting.insert(sequence, (frame, size));
        Ok(())
    }

    pub(super) async fn commit_next(&mut self) -> anyhow::Result<Option<CommittedEvent>> {
        let Some((frame, _)) = self.waiting.get(&(self.sequence + 1)) else {
            return Ok(None);
        };
        let mut projector = self.projector.clone();
        let projection = projector.project(frame)?;
        let ts = Utc::now().timestamp();
        let mut output = Vec::with_capacity(projection.history.len());
        let mut encoded = Vec::with_capacity(projection.history.len());
        for entry in projection.history {
            let (stream, message) = match entry {
                ProjectedHistory::Conversation(value) => (OutputStream::Acp, value.to_string()),
                ProjectedHistory::System(message) => (OutputStream::System, message),
            };
            encoded.push(encode_message_for_storage(&stream, &message));
            output.push(AgentOutput {
                event_id: 0,
                agent_id: self.agent_id.clone(),
                session_id: self.local_session_id.clone(),
                seq: Uuid::now_v7().to_string(),
                ts,
                stream,
                message,
            });
        }
        let history: Vec<_> = output
            .iter()
            .zip(&encoded)
            .map(|(entry, bytes)| RuntimeHistoryEntry {
                seq: &entry.seq,
                ts,
                stream: entry.stream.clone(),
                message: bytes,
            })
            .collect();
        let fingerprint = frame.fingerprint()?;
        let result = self
            .stream
            .persist(
                RuntimeEventIdentity {
                    event_id: &frame.event.event_id,
                    sequence: frame.event.sequence,
                    fingerprint: &fingerprint,
                },
                &history,
            )
            .await?;
        let RuntimePersistResult::Persisted { history_ids } = result else {
            anyhow::bail!("direct history consumer lost exclusive stream ownership");
        };
        for (entry, id) in output.iter_mut().zip(history_ids) {
            entry.event_id = id;
        }
        self.projector = projector;
        self.sequence += 1;
        let (_, size) = self
            .waiting
            .remove(&self.sequence)
            .expect("committed buffered event");
        self.waiting_bytes -= size;
        Ok(Some(CommittedEvent {
            effect: projection.effect,
            output,
            sequence: self.sequence,
        }))
    }
}
