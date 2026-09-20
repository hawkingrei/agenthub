//! Provider cursors and receipts live beside history, never in the task or mailbox ledger.

mod events;
mod history;
mod requests;
mod schema;
#[cfg(test)]
mod tests;

use sqlx::{Row, SqlitePool};

pub use events::{
    RuntimeCursor, RuntimeEventIdentity, RuntimeEventStream, RuntimeHistoryEntry,
    RuntimePersistResult, RuntimeReplayGap,
};
pub use history::{RuntimeHistory, RuntimeStreamSummary};
pub use requests::{
    RuntimeRejectionCode, RuntimeRequestAck, RuntimeRequestIntent, RuntimeRequestKind,
    RuntimeRequestReceipt, RuntimeRequestStatus, RuntimeSendPermit, RuntimeSubmissionFailure,
};
pub(crate) use schema::migrate;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeEventError {
    #[error("runtime event identity is invalid")]
    InvalidIdentity,
    #[error("runtime event sequence is outside the supported range")]
    InvalidSequence,
    #[error("runtime event projection exceeds the storage bound")]
    ProjectionLimit,
    #[error("runtime belongs to another local launch")]
    OwnershipConflict,
    #[error("runtime event identity or sequence was reused with different content")]
    EventConflict,
    #[error("runtime event stream has an unrecoverable replay gap")]
    ReplayGap,
    #[error("runtime replay gap does not match the persisted cursor")]
    InvalidGap,
    #[error("runtime event owner is closed")]
    Closed,
    #[error("runtime control request identity has already been prepared")]
    RequestReused,
    #[error("runtime control receipt capacity is exhausted")]
    ReceiptCapacity,
    #[error("runtime control request target is invalid")]
    InvalidTarget,
    #[error("runtime control receipt transition conflicts with recorded state")]
    ReceiptConflict,
}

/// Construct this only from the agent's routed event database and owned local launch.
#[derive(Clone)]
pub struct RuntimeEventStore {
    pool: SqlitePool,
    local_session_id: String,
    runtime_id: String,
}

impl RuntimeEventStore {
    pub async fn bind(
        pool: SqlitePool,
        local_session_id: &str,
        runtime_id: &str,
    ) -> anyhow::Result<Self> {
        validate_id(local_session_id)?;
        validate_id(runtime_id)?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query(
            "INSERT INTO runtime_event_owners(runtime_id, local_session_id) VALUES (?, ?) \
             ON CONFLICT DO NOTHING",
        )
        .bind(runtime_id)
        .bind(local_session_id)
        .execute(&mut *tx)
        .await?;
        let matches: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM runtime_event_owners \
             WHERE runtime_id = ? AND local_session_id = ?)",
        )
        .bind(runtime_id)
        .bind(local_session_id)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(matches, RuntimeEventError::OwnershipConflict);
        tx.commit().await?;
        Ok(Self {
            pool,
            local_session_id: local_session_id.to_owned(),
            runtime_id: runtime_id.to_owned(),
        })
    }

    pub fn runtime_id(&self) -> &str {
        &self.runtime_id
    }

    pub fn local_session_id(&self) -> &str {
        &self.local_session_id
    }

    /// Binding is explicit: an unsolicited event cannot allocate its own authority.
    pub async fn bind_stream(&self, native_session_id: &str) -> anyhow::Result<RuntimeEventStream> {
        validate_id(native_session_id)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        self.require_open(&mut tx).await?;
        sqlx::query(
            "INSERT INTO runtime_event_streams(runtime_id, native_session_id) VALUES (?, ?) \
             ON CONFLICT DO NOTHING",
        )
        .bind(&self.runtime_id)
        .bind(native_session_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(RuntimeEventStream {
            owner: self.clone(),
            native_session_id: native_session_id.to_owned(),
        })
    }

    pub async fn stream(
        &self,
        native_session_id: &str,
    ) -> anyhow::Result<Option<RuntimeEventStream>> {
        validate_id(native_session_id)?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM runtime_event_streams \
             WHERE runtime_id = ? AND native_session_id = ?)",
        )
        .bind(&self.runtime_id)
        .bind(native_session_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists.then(|| RuntimeEventStream {
            owner: self.clone(),
            native_session_id: native_session_id.to_owned(),
        }))
    }

    /// Call after draining received events. Closing cannot later reopen a runtime.
    pub async fn close(&self, now: i64) -> anyhow::Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query(
            "UPDATE runtime_control_receipts SET status = CASE status \
             WHEN 'prepared' THEN 'not_sent' ELSE 'outcome_unknown' END, updated_at = ? \
             WHERE runtime_id = ? AND status IN ('prepared', 'sent')",
        )
        .bind(now)
        .bind(&self.runtime_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE runtime_event_owners SET closed = 1 WHERE runtime_id = ?")
            .bind(&self.runtime_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn require_open(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ) -> anyhow::Result<()> {
        let row = sqlx::query(
            "SELECT local_session_id, closed FROM runtime_event_owners WHERE runtime_id = ?",
        )
        .bind(&self.runtime_id)
        .fetch_one(&mut **tx)
        .await?;
        anyhow::ensure!(
            row.try_get::<&str, _>("local_session_id")? == self.local_session_id,
            RuntimeEventError::OwnershipConflict
        );
        anyhow::ensure!(
            !row.try_get::<bool, _>("closed")?,
            RuntimeEventError::Closed
        );
        Ok(())
    }
}

fn validate_id(id: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !id.is_empty()
            && id.len() <= 128
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte)),
        RuntimeEventError::InvalidIdentity
    );
    Ok(())
}

fn sequence(value: u64) -> anyhow::Result<i64> {
    i64::try_from(value).map_err(|_| RuntimeEventError::InvalidSequence.into())
}
