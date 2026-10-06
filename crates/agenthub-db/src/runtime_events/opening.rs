use sqlx::Row;

use super::{
    RuntimeEventError, RuntimeEventStore, RuntimeRequestAck, RuntimeRequestKind,
    RuntimeRequestStatus,
};

/// Exact accepted opening from a closed event owner. This proves conversation
/// identity only; the loop store must independently prove executor retirement.
pub struct RuntimeOpeningEvidence {
    pub(crate) local_session_id: String,
    pub(crate) native_session_id: String,
    pub(crate) kind: RuntimeRequestKind,
}

impl RuntimeEventStore {
    pub async fn accepted_closed_opening(&self) -> anyhow::Result<Option<RuntimeOpeningEvidence>> {
        let mut tx = self.pool.begin().await?;
        let closed: bool = sqlx::query_scalar(
            "SELECT closed FROM runtime_event_owners WHERE runtime_id = ? AND local_session_id = ?",
        )
        .bind(&self.runtime_id)
        .bind(&self.local_session_id)
        .fetch_one(&mut *tx)
        .await?;
        if !closed {
            return Ok(None);
        }
        let rows = sqlx::query(
            "SELECT * FROM runtime_control_receipts WHERE runtime_id = ? \
             AND kind IN ('create_session', 'resume_session') LIMIT 2",
        )
        .bind(&self.runtime_id)
        .fetch_all(&mut *tx)
        .await?;
        anyhow::ensure!(rows.len() <= 1, RuntimeEventError::ReceiptConflict);
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        let receipt = super::requests::receipt_from_row(row)?;
        if receipt.status != RuntimeRequestStatus::Accepted {
            return Ok(None);
        }
        let Some(RuntimeRequestAck::Accepted {
            session_id,
            turn_id: None,
            ..
        }) = receipt.ack
        else {
            anyhow::bail!(RuntimeEventError::ReceiptConflict);
        };
        let valid_target = match receipt.kind {
            RuntimeRequestKind::CreateSession => receipt.target_session_id.is_none(),
            RuntimeRequestKind::ResumeSession => {
                receipt.target_session_id.as_ref() == Some(&session_id)
            }
            _ => false,
        };
        anyhow::ensure!(valid_target, RuntimeEventError::InvalidTarget);
        super::validate_id(&session_id)?;
        // The accepted ACK and this stream were committed in the same transaction.
        let stream = sqlx::query(
            "SELECT native_session_id FROM runtime_event_streams WHERE runtime_id = ? LIMIT 2",
        )
        .bind(&self.runtime_id)
        .fetch_all(&mut *tx)
        .await?;
        anyhow::ensure!(
            stream.len() == 1 && stream[0].try_get::<&str, _>("native_session_id")? == session_id,
            RuntimeEventError::OwnershipConflict
        );
        tx.commit().await?;
        Ok(Some(RuntimeOpeningEvidence {
            local_session_id: self.local_session_id.clone(),
            native_session_id: session_id,
            kind: receipt.kind,
        }))
    }
}
