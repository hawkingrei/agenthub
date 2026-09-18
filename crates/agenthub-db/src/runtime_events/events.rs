use agenthub_agent_domain::OutputStream;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use super::{RuntimeEventError, RuntimeEventStore, sequence, validate_id};

#[derive(Clone)]
pub struct RuntimeEventStream {
    pub(super) owner: RuntimeEventStore,
    pub(super) native_session_id: String,
}

/// The adapter computes a canonical event digest; provider payloads are not receipt metadata.
pub struct RuntimeEventIdentity<'a> {
    pub event_id: &'a str,
    pub sequence: u64,
    pub fingerprint: &'a [u8; 32],
}

/// Already normalized and encoded by the existing history codec.
pub struct RuntimeHistoryEntry<'a> {
    pub seq: &'a str,
    pub ts: i64,
    pub stream: OutputStream,
    pub message: &'a [u8],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimePersistResult {
    Persisted { history_ids: Vec<i64> },
    Duplicate,
    Gap { expected: u64, received: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeReplayGap {
    pub requested_after: u64,
    pub oldest_available: u64,
    pub latest: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCursor {
    pub sequence: u64,
    pub gap: Option<RuntimeReplayGap>,
}

impl RuntimeEventStream {
    pub fn native_session_id(&self) -> &str {
        &self.native_session_id
    }

    pub async fn cursor(&self) -> anyhow::Result<RuntimeCursor> {
        let row = sqlx::query(
            "SELECT last_sequence, gap_after, gap_oldest, gap_latest \
             FROM runtime_event_streams WHERE runtime_id = ? AND native_session_id = ?",
        )
        .bind(&self.owner.runtime_id)
        .bind(&self.native_session_id)
        .fetch_one(&self.owner.pool)
        .await?;
        Ok(RuntimeCursor {
            sequence: row.try_get::<i64, _>("last_sequence")? as u64,
            gap: row
                .try_get::<Option<i64>, _>("gap_after")?
                .map(|after| -> anyhow::Result<_> {
                    Ok(RuntimeReplayGap {
                        requested_after: after as u64,
                        oldest_available: row.try_get::<i64, _>("gap_oldest")? as u64,
                        latest: row.try_get::<i64, _>("gap_latest")? as u64,
                    })
                })
                .transpose()?,
        })
    }

    pub async fn persist(
        &self,
        event: RuntimeEventIdentity<'_>,
        history: &[RuntimeHistoryEntry<'_>],
    ) -> anyhow::Result<RuntimePersistResult> {
        validate_id(event.event_id)?;
        let event_sequence = sequence(event.sequence)?;
        anyhow::ensure!(event_sequence > 0, RuntimeEventError::InvalidSequence);
        anyhow::ensure!(
            history.len() <= 16
                && history
                    .iter()
                    .map(|entry| entry.message.len())
                    .sum::<usize>()
                    <= 2 * 1024 * 1024,
            RuntimeEventError::ProjectionLimit
        );
        for entry in history {
            validate_id(entry.seq)?;
        }
        let mut tx = self.owner.pool.begin_with("BEGIN IMMEDIATE").await?;
        self.owner.require_open(&mut tx).await?;
        let existing = sqlx::query(
            "SELECT event_id, sequence, fingerprint FROM runtime_event_receipts \
             WHERE runtime_id = ? AND native_session_id = ? AND (event_id = ? OR sequence = ?)",
        )
        .bind(&self.owner.runtime_id)
        .bind(&self.native_session_id)
        .bind(event.event_id)
        .bind(event_sequence)
        .fetch_all(&mut *tx)
        .await?;
        if !existing.is_empty() {
            let same = existing.len() == 1
                && existing[0].try_get::<&str, _>("event_id")? == event.event_id
                && existing[0].try_get::<i64, _>("sequence")? == event_sequence
                && existing[0].try_get::<&[u8], _>("fingerprint")? == event.fingerprint;
            anyhow::ensure!(same, RuntimeEventError::EventConflict);
            tx.commit().await?;
            return Ok(RuntimePersistResult::Duplicate);
        }
        let row = sqlx::query(
            "SELECT last_sequence, gap_after FROM runtime_event_streams \
             WHERE runtime_id = ? AND native_session_id = ?",
        )
        .bind(&self.owner.runtime_id)
        .bind(&self.native_session_id)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(
            row.try_get::<Option<i64>, _>("gap_after")?.is_none(),
            RuntimeEventError::ReplayGap
        );
        let last = row.try_get::<i64, _>("last_sequence")?;
        anyhow::ensure!(event_sequence > last, RuntimeEventError::EventConflict);
        if event_sequence - last != 1 {
            tx.commit().await?;
            return Ok(RuntimePersistResult::Gap {
                expected: last as u64 + 1,
                received: event.sequence,
            });
        }
        sqlx::query(
            "INSERT INTO runtime_event_receipts \
             (runtime_id, native_session_id, event_id, sequence, fingerprint) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&self.owner.runtime_id)
        .bind(&self.native_session_id)
        .bind(event.event_id)
        .bind(event_sequence)
        .bind(event.fingerprint.as_slice())
        .execute(&mut *tx)
        .await?;
        let mut history_ids = Vec::with_capacity(history.len());
        for entry in history {
            let stream = match entry.stream {
                OutputStream::Stdout => "stdout",
                OutputStream::Stderr => "stderr",
                OutputStream::System => "system",
                OutputStream::Acp => "acp",
            };
            let inserted = sqlx::query(
                "INSERT INTO agent_events(session_id, seq, ts, stream, message) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&self.owner.local_session_id)
            .bind(entry.seq)
            .bind(entry.ts)
            .bind(stream)
            .bind(entry.message)
            .execute(&mut *tx)
            .await?;
            let history_id = inserted.last_insert_rowid();
            sqlx::query(
                "INSERT INTO runtime_event_history(history_id, runtime_id, native_session_id, event_id) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(history_id)
            .bind(&self.owner.runtime_id)
            .bind(&self.native_session_id)
            .bind(event.event_id)
            .execute(&mut *tx)
            .await?;
            history_ids.push(history_id);
        }
        sqlx::query(
            "UPDATE runtime_event_streams SET last_sequence = ? \
             WHERE runtime_id = ? AND native_session_id = ?",
        )
        .bind(event_sequence)
        .bind(&self.owner.runtime_id)
        .bind(&self.native_session_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(RuntimePersistResult::Persisted { history_ids })
    }

    /// An unavailable prefix is terminal for this stream. Never skip to the oldest event.
    pub async fn record_replay_gap(&self, gap: RuntimeReplayGap) -> anyhow::Result<()> {
        let after = sequence(gap.requested_after)?;
        let oldest = sequence(gap.oldest_available)?;
        let latest = sequence(gap.latest)?;
        anyhow::ensure!(
            oldest > 0 && (after < oldest - 1 || after > latest) && oldest - 1 <= latest,
            RuntimeEventError::InvalidGap
        );
        let mut tx = self.owner.pool.begin_with("BEGIN IMMEDIATE").await?;
        self.owner.require_open(&mut tx).await?;
        let changed = sqlx::query(
            "UPDATE runtime_event_streams SET gap_after = ?, gap_oldest = ?, gap_latest = ? \
             WHERE runtime_id = ? AND native_session_id = ? AND last_sequence = ? \
             AND (gap_after IS NULL OR (gap_after = ? AND gap_oldest = ? AND gap_latest = ?))",
        )
        .bind(after)
        .bind(oldest)
        .bind(latest)
        .bind(&self.owner.runtime_id)
        .bind(&self.native_session_id)
        .bind(after)
        .bind(after)
        .bind(oldest)
        .bind(latest)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        anyhow::ensure!(changed == 1, RuntimeEventError::InvalidGap);
        tx.commit().await?;
        Ok(())
    }
}
