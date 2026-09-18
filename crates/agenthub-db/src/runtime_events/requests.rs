use serde::{Deserialize, Serialize};
use sqlx::{Connection, Row};

use super::{RuntimeEventError, RuntimeEventStore, sequence, validate_id};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRequestKind {
    CreateSession,
    Prompt,
    FollowUp,
    Cancel,
    Interrupt,
    UserAnswer,
    PlanAnswer,
    ShellAnswer,
    Query,
    Replay,
}

impl RuntimeRequestKind {
    fn needs_turn(self) -> bool {
        matches!(
            self,
            Self::Cancel
                | Self::Interrupt
                | Self::UserAnswer
                | Self::PlanAnswer
                | Self::ShellAnswer
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRequestStatus {
    Prepared,
    Sent,
    Accepted,
    Queued,
    Rejected,
    OutcomeUnknown,
    NotSent,
}

/// Rejection codes are an allowlist; provider rejection prose is never stored here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeRequestAck {
    Accepted {
        session_id: String,
        turn_id: Option<String>,
        last_sequence: Option<u64>,
    },
    Queued {
        session_id: String,
    },
    Rejected {
        code: RuntimeRejectionCode,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRejectionCode {
    InvalidRequest,
    StaleRuntime,
    UnknownSession,
    Unsupported,
    Busy,
    NotRunning,
    Overloaded,
    Closed,
    RequestConflict,
    Internal,
}

pub struct RuntimeRequestIntent<'a> {
    pub request_id: &'a str,
    pub kind: RuntimeRequestKind,
    pub target_session_id: Option<&'a str>,
    pub expected_turn_id: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRequestReceipt {
    pub request_id: String,
    pub kind: RuntimeRequestKind,
    pub target_session_id: Option<String>,
    pub expected_turn_id: Option<String>,
    pub status: RuntimeRequestStatus,
    pub ack: Option<RuntimeRequestAck>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Issued once after durable send intent. Dropping it never permits another submission.
pub struct RuntimeSendPermit {
    runtime_id: String,
    local_session_id: String,
    request_id: String,
}

impl RuntimeSendPermit {
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
}

#[derive(Clone, Copy)]
pub enum RuntimeSubmissionFailure {
    /// Only use when the transport proves the request was never queued or written.
    NotSent,
    OutcomeUnknown,
}

impl RuntimeEventStore {
    pub async fn prepare_request(
        &self,
        intent: RuntimeRequestIntent<'_>,
        now: i64,
    ) -> anyhow::Result<()> {
        self.prepare_with_history(intent, now, None)
            .await
            .map(|_| ())
    }

    /// Atomically attribute a visible input attempt to its single-use request identity.
    pub async fn prepare_input_request(
        &self,
        intent: RuntimeRequestIntent<'_>,
        now: i64,
        history: super::RuntimeHistoryEntry<'_>,
    ) -> anyhow::Result<i64> {
        anyhow::ensure!(
            matches!(
                intent.kind,
                RuntimeRequestKind::Prompt
                    | RuntimeRequestKind::FollowUp
                    | RuntimeRequestKind::UserAnswer
            ),
            RuntimeEventError::InvalidTarget
        );
        validate_id(history.seq)?;
        anyhow::ensure!(
            matches!(history.stream, agenthub_agent_domain::OutputStream::Acp)
                && history.message.len() <= 2 * 1024 * 1024,
            RuntimeEventError::ProjectionLimit
        );
        self.prepare_with_history(intent, now, Some(history))
            .await?
            .ok_or_else(|| RuntimeEventError::ReceiptConflict.into())
    }

    async fn prepare_with_history(
        &self,
        intent: RuntimeRequestIntent<'_>,
        now: i64,
        history: Option<super::RuntimeHistoryEntry<'_>>,
    ) -> anyhow::Result<Option<i64>> {
        validate_id(intent.request_id)?;
        for id in [intent.target_session_id, intent.expected_turn_id]
            .into_iter()
            .flatten()
        {
            validate_id(id)?;
        }
        anyhow::ensure!(
            (intent.kind == RuntimeRequestKind::CreateSession)
                == intent.target_session_id.is_none()
                && intent.kind.needs_turn() == intent.expected_turn_id.is_some(),
            RuntimeEventError::InvalidTarget
        );
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        self.require_open(&mut tx).await?;
        if let Some(target) = intent.target_session_id {
            let owns: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM runtime_event_streams \
                WHERE runtime_id = ? AND native_session_id = ?)",
            )
            .bind(&self.runtime_id)
            .bind(target)
            .fetch_one(&mut *tx)
            .await?;
            anyhow::ensure!(owns, RuntimeEventError::InvalidTarget);
        }
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM runtime_control_receipts WHERE runtime_id = ?",
        )
        .bind(&self.runtime_id)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(count < 4096, RuntimeEventError::ReceiptCapacity);
        let changed = sqlx::query("INSERT INTO runtime_control_receipts \
            (runtime_id, request_id, kind, target_session_id, expected_turn_id, status, created_at, updated_at) \
            VALUES (?, ?, ?, ?, ?, 'prepared', ?, ?) ON CONFLICT DO NOTHING")
            .bind(&self.runtime_id).bind(intent.request_id).bind(enum_name(intent.kind)?)
            .bind(intent.target_session_id).bind(intent.expected_turn_id).bind(now).bind(now)
            .execute(&mut *tx).await?.rows_affected();
        anyhow::ensure!(changed == 1, RuntimeEventError::RequestReused);
        let history_id = if let Some(entry) = history {
            Some(sqlx::query("INSERT INTO agent_events(session_id, seq, ts, stream, message) VALUES (?, ?, ?, 'acp', ?)")
                .bind(&self.local_session_id).bind(entry.seq).bind(entry.ts).bind(entry.message)
                .execute(&mut *tx).await?.last_insert_rowid())
        } else {
            None
        };
        tx.commit().await?;
        Ok(history_id)
    }

    pub async fn mark_request_sent(
        &self,
        request_id: &str,
        now: i64,
    ) -> anyhow::Result<RuntimeSendPermit> {
        validate_id(request_id)?;
        let mut connection = self.pool.acquire().await?;
        // Flush send intent before an external effect, including after power loss. Retain
        // FULL on this pooled connection so cancellation cannot restore an unsafe setting.
        sqlx::query("PRAGMA synchronous = FULL")
            .execute(&mut *connection)
            .await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        self.require_open(&mut tx).await?;
        let changed = sqlx::query(
            "UPDATE runtime_control_receipts SET status = 'sent', updated_at = ? \
            WHERE runtime_id = ? AND request_id = ? AND status = 'prepared'",
        )
        .bind(now)
        .bind(&self.runtime_id)
        .bind(request_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        anyhow::ensure!(changed == 1, RuntimeEventError::ReceiptConflict);
        tx.commit().await?;
        Ok(RuntimeSendPermit {
            runtime_id: self.runtime_id.clone(),
            local_session_id: self.local_session_id.clone(),
            request_id: request_id.to_owned(),
        })
    }

    pub async fn request_receipt(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Option<RuntimeRequestReceipt>> {
        validate_id(request_id)?;
        sqlx::query(
            "SELECT * FROM runtime_control_receipts WHERE runtime_id = ? AND request_id = ?",
        )
        .bind(&self.runtime_id)
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await?
        .as_ref()
        .map(receipt_from_row)
        .transpose()
    }

    pub async fn record_request_ack(
        &self,
        permit: &RuntimeSendPermit,
        ack: RuntimeRequestAck,
        now: i64,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            permit.runtime_id == self.runtime_id
                && permit.local_session_id == self.local_session_id,
            RuntimeEventError::OwnershipConflict
        );
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let row = sqlx::query(
            "SELECT * FROM runtime_control_receipts WHERE runtime_id = ? AND request_id = ?",
        )
        .bind(&self.runtime_id)
        .bind(&permit.request_id)
        .fetch_one(&mut *tx)
        .await?;
        let receipt = receipt_from_row(&row)?;
        let (status, returned_session) = match &ack {
            RuntimeRequestAck::Accepted {
                session_id,
                turn_id,
                last_sequence,
            } => {
                validate_id(session_id)?;
                if let Some(turn) = turn_id {
                    validate_id(turn)?;
                }
                if let Some(last) = last_sequence {
                    sequence(*last)?;
                }
                let valid_turn = match receipt.kind {
                    RuntimeRequestKind::CreateSession
                    | RuntimeRequestKind::Query
                    | RuntimeRequestKind::Replay => turn_id.is_none(),
                    RuntimeRequestKind::Cancel | RuntimeRequestKind::Interrupt => {
                        *turn_id == receipt.expected_turn_id
                    }
                    _ => turn_id.is_some(),
                };
                anyhow::ensure!(valid_turn, RuntimeEventError::InvalidTarget);
                (RuntimeRequestStatus::Accepted, Some(session_id))
            }
            RuntimeRequestAck::Queued { session_id } => {
                validate_id(session_id)?;
                anyhow::ensure!(
                    receipt.kind == RuntimeRequestKind::FollowUp,
                    RuntimeEventError::ReceiptConflict
                );
                (RuntimeRequestStatus::Queued, Some(session_id))
            }
            RuntimeRequestAck::Rejected { .. } => (RuntimeRequestStatus::Rejected, None),
        };
        if let (Some(expected), Some(returned)) =
            (receipt.target_session_id.as_ref(), returned_session)
        {
            anyhow::ensure!(expected == returned, RuntimeEventError::InvalidTarget);
        }
        if let Some(previous) = receipt.ack {
            anyhow::ensure!(previous == ack, RuntimeEventError::ReceiptConflict);
            tx.commit().await?;
            return Ok(());
        }
        anyhow::ensure!(
            matches!(
                receipt.status,
                RuntimeRequestStatus::Sent | RuntimeRequestStatus::OutcomeUnknown
            ),
            RuntimeEventError::ReceiptConflict
        );
        if receipt.kind == RuntimeRequestKind::CreateSession
            && let Some(session_id) = returned_session
        {
            // Initial events can follow the ACK immediately. Expose stream ownership and
            // its admission receipt in one commit before the consumer accepts those events.
            sqlx::query(
                "INSERT INTO runtime_event_streams(runtime_id, native_session_id) VALUES (?, ?) \
                ON CONFLICT DO NOTHING",
            )
            .bind(&self.runtime_id)
            .bind(session_id)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query(
            "UPDATE runtime_control_receipts SET status = ?, ack_json = ?, updated_at = ? \
            WHERE runtime_id = ? AND request_id = ?",
        )
        .bind(enum_name(status)?)
        .bind(serde_json::to_string(&ack)?)
        .bind(now)
        .bind(&self.runtime_id)
        .bind(&permit.request_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn record_submission_failure(
        &self,
        permit: &RuntimeSendPermit,
        failure: RuntimeSubmissionFailure,
        now: i64,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            permit.runtime_id == self.runtime_id
                && permit.local_session_id == self.local_session_id,
            RuntimeEventError::OwnershipConflict
        );
        let status = match failure {
            RuntimeSubmissionFailure::NotSent => "not_sent",
            RuntimeSubmissionFailure::OutcomeUnknown => "outcome_unknown",
        };
        let changed = sqlx::query(
            "UPDATE runtime_control_receipts SET status = ?, updated_at = ? \
            WHERE runtime_id = ? AND request_id = ? AND status IN ('sent', ?)",
        )
        .bind(status)
        .bind(now)
        .bind(&self.runtime_id)
        .bind(&permit.request_id)
        .bind(status)
        .execute(&self.pool)
        .await?
        .rows_affected();
        anyhow::ensure!(changed == 1, RuntimeEventError::ReceiptConflict);
        Ok(())
    }
}

fn enum_name(value: impl Serialize) -> anyhow::Result<String> {
    match serde_json::to_value(value)? {
        serde_json::Value::String(name) => Ok(name),
        _ => anyhow::bail!(RuntimeEventError::ReceiptConflict),
    }
}

fn receipt_from_row(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<RuntimeRequestReceipt> {
    Ok(RuntimeRequestReceipt {
        request_id: row.try_get("request_id")?,
        kind: serde_json::from_value(serde_json::Value::String(row.try_get("kind")?))?,
        target_session_id: row.try_get("target_session_id")?,
        expected_turn_id: row.try_get("expected_turn_id")?,
        status: serde_json::from_value(serde_json::Value::String(row.try_get("status")?))?,
        ack: row
            .try_get::<Option<&str>, _>("ack_json")?
            .map(serde_json::from_str)
            .transpose()?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
