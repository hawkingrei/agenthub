use serde::Serialize;
use sqlx::{Row, SqlitePool};

use super::{
    RuntimeCursor, RuntimeEventStore, RuntimeReplayGap, RuntimeRequestReceipt, validate_id,
};

/// Read-only delivery evidence. It grants neither execution nor session-resume authority.
#[derive(Debug, Serialize)]
pub struct RuntimeHistory {
    pub local_session_id: String,
    pub runtime_id: String,
    pub closed: bool,
    pub streams: Vec<RuntimeStreamSummary>,
    pub streams_truncated: bool,
    pub receipts: Vec<RuntimeRequestReceipt>,
    pub next_before_request_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RuntimeStreamSummary {
    pub native_session_id: String,
    pub cursor: RuntimeCursor,
}

impl RuntimeEventStore {
    /// Look up existing ownership without creating or reopening it.
    pub async fn load(pool: SqlitePool, local_session_id: &str) -> anyhow::Result<Option<Self>> {
        validate_id(local_session_id)?;
        let runtime_id: Option<String> = sqlx::query_scalar(
            "SELECT runtime_id FROM runtime_event_owners WHERE local_session_id = ?",
        )
        .bind(local_session_id)
        .fetch_optional(&pool)
        .await?;
        Ok(runtime_id.map(|runtime_id| Self {
            pool,
            local_session_id: local_session_id.to_owned(),
            runtime_id,
        }))
    }

    /// Receipts are paged in descending request-ID order, independent of ACK updates.
    pub async fn history(
        &self,
        limit: i64,
        before_request_id: Option<&str>,
    ) -> anyhow::Result<RuntimeHistory> {
        if let Some(before) = before_request_id {
            validate_id(before)?;
        }
        let limit = limit.clamp(1, 100);
        let mut tx = self.pool.begin().await?;
        let closed = sqlx::query_scalar(
            "SELECT closed FROM runtime_event_owners WHERE runtime_id = ? AND local_session_id = ?",
        )
        .bind(&self.runtime_id)
        .bind(&self.local_session_id)
        .fetch_one(&mut *tx)
        .await?;
        let mut rows = sqlx::query(
            "SELECT native_session_id, last_sequence, gap_after, gap_oldest, gap_latest \
             FROM runtime_event_streams WHERE runtime_id = ? ORDER BY native_session_id LIMIT 101",
        )
        .bind(&self.runtime_id)
        .fetch_all(&mut *tx)
        .await?;
        let streams_truncated = rows.len() > 100;
        rows.truncate(100);
        let streams = rows
            .into_iter()
            .map(|row| -> anyhow::Result<_> {
                Ok(RuntimeStreamSummary {
                    native_session_id: row.try_get("native_session_id")?,
                    cursor: RuntimeCursor {
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
                    },
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let rows = sqlx::query(
            "SELECT * FROM runtime_control_receipts WHERE runtime_id = ? \
             AND (? IS NULL OR request_id < ?) ORDER BY request_id DESC LIMIT ?",
        )
        .bind(&self.runtime_id)
        .bind(before_request_id)
        .bind(before_request_id)
        .bind(limit + 1)
        .fetch_all(&mut *tx)
        .await?;
        let has_more = rows.len() > limit as usize;
        let receipts = rows
            .iter()
            .take(limit as usize)
            .map(super::requests::receipt_from_row)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let next_before_request_id = has_more
            .then(|| receipts.last().map(|receipt| receipt.request_id.clone()))
            .flatten();
        tx.commit().await?;
        Ok(RuntimeHistory {
            local_session_id: self.local_session_id.clone(),
            runtime_id: self.runtime_id.clone(),
            closed,
            streams,
            streams_truncated,
            receipts,
            next_before_request_id,
        })
    }
}
