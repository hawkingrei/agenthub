use agenthub_db::runtime_events::{
    RuntimeEventStore, RuntimeRejectionCode, RuntimeRequestAck, RuntimeRequestIntent,
    RuntimeRequestKind, RuntimeSendPermit, RuntimeSubmissionFailure,
};
use agenthub_rara::{
    Client, ClientFrame, ConnectionError, ControlKind, ControlRequest, RequestResult,
};
use chrono::Utc;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::daemon_tasks::DaemonTaskGroup;

/// Own the response through caller disconnects, including the durable ACK write.
pub(super) async fn control(
    tasks: &DaemonTaskGroup,
    client: &Client,
    store: &RuntimeEventStore,
    session: Option<&str>,
    request: ControlRequest,
) -> anyhow::Result<RuntimeRequestAck> {
    let request_id = Uuid::now_v7().to_string();
    let frame = request.frame(store.runtime_id(), &request_id, session)?;
    let kind = kind(request.kind());
    let session = session.map(str::to_owned);
    let turn = request.expected_turn_id().map(str::to_owned);
    let client = client.clone();
    let store = store.clone();
    let (reply, response) = oneshot::channel();
    tasks.spawn_runtime_task(format!("direct-control:{request_id}"), async move {
        let result = submit(
            &client,
            &store,
            frame,
            kind,
            session.as_deref(),
            turn.as_deref(),
        )
        .await;
        let _ = reply.send(result);
        Ok(())
    })?;
    response
        .await
        .map_err(|_| anyhow::anyhow!("direct control receipt task stopped"))?
}

pub(super) fn kind(kind: ControlKind) -> RuntimeRequestKind {
    match kind {
        ControlKind::CreateSession => RuntimeRequestKind::CreateSession,
        ControlKind::Query => RuntimeRequestKind::Query,
        ControlKind::Prompt => RuntimeRequestKind::Prompt,
        ControlKind::FollowUp => RuntimeRequestKind::FollowUp,
        ControlKind::Cancel => RuntimeRequestKind::Cancel,
        ControlKind::Interrupt => RuntimeRequestKind::Interrupt,
        ControlKind::UserAnswer => RuntimeRequestKind::UserAnswer,
        ControlKind::PlanAnswer => RuntimeRequestKind::PlanAnswer,
        ControlKind::ShellAnswer => RuntimeRequestKind::ShellAnswer,
    }
}

pub(super) async fn submit(
    client: &Client,
    store: &RuntimeEventStore,
    frame: ClientFrame,
    kind: RuntimeRequestKind,
    session: Option<&str>,
    turn: Option<&str>,
) -> anyhow::Result<RuntimeRequestAck> {
    agenthub_rara::encode_request(&frame)?;
    store
        .prepare_request(
            RuntimeRequestIntent {
                request_id: frame.request_id(),
                kind,
                target_session_id: session,
                expected_turn_id: turn,
            },
            Utc::now().timestamp(),
        )
        .await?;
    let permit = store
        .mark_request_sent(frame.request_id(), Utc::now().timestamp())
        .await?;
    send_prepared(client, store, frame, permit).await
}

pub(super) async fn send_prepared(
    client: &Client,
    store: &RuntimeEventStore,
    frame: ClientFrame,
    permit: RuntimeSendPermit,
) -> anyhow::Result<RuntimeRequestAck> {
    anyhow::ensure!(
        frame.request_id() == permit.request_id(),
        "direct request permit identity mismatch"
    );
    match client.request(frame).await {
        Ok(ack) => {
            let result = async {
                let ack = safe_ack(ack.result)?;
                store
                    .record_request_ack(&permit, ack.clone(), Utc::now().timestamp())
                    .await?;
                Ok(ack)
            }
            .await;
            if result.is_err() {
                // A received but uncommitted response cannot authorize a replacement send.
                let _ = store
                    .record_submission_failure(
                        &permit,
                        RuntimeSubmissionFailure::OutcomeUnknown,
                        Utc::now().timestamp(),
                    )
                    .await;
                client.abort();
            }
            result
        }
        Err(error) => {
            let failure = match error {
                ConnectionError::QueueFull
                | ConnectionError::ReceiptCapacity
                | ConnectionError::RequestIdReused
                | ConnectionError::WrongRuntime
                | ConnectionError::UnsupportedMethod => RuntimeSubmissionFailure::NotSent,
                _ => RuntimeSubmissionFailure::OutcomeUnknown,
            };
            store
                .record_submission_failure(&permit, failure, Utc::now().timestamp())
                .await?;
            Err(error.into())
        }
    }
}

fn safe_ack(result: RequestResult) -> anyhow::Result<RuntimeRequestAck> {
    Ok(match result {
        RequestResult::Accepted {
            session_id,
            turn_id,
            last_sequence,
        } => RuntimeRequestAck::Accepted {
            session_id: session_id
                .ok_or_else(|| anyhow::anyhow!("direct control ACK lacks its session"))?,
            turn_id,
            last_sequence,
        },
        RequestResult::Queued { session_id } => RuntimeRequestAck::Queued { session_id },
        RequestResult::Rejected { code, .. } => RuntimeRequestAck::Rejected {
            code: match code {
                agenthub_rara::RejectionCode::InvalidRequest => {
                    RuntimeRejectionCode::InvalidRequest
                }
                agenthub_rara::RejectionCode::StaleRuntime => RuntimeRejectionCode::StaleRuntime,
                agenthub_rara::RejectionCode::UnknownSession => {
                    RuntimeRejectionCode::UnknownSession
                }
                agenthub_rara::RejectionCode::Unsupported => RuntimeRejectionCode::Unsupported,
                agenthub_rara::RejectionCode::Busy => RuntimeRejectionCode::Busy,
                agenthub_rara::RejectionCode::NotRunning => RuntimeRejectionCode::NotRunning,
                agenthub_rara::RejectionCode::Overloaded => RuntimeRejectionCode::Overloaded,
                agenthub_rara::RejectionCode::Closed => RuntimeRejectionCode::Closed,
                agenthub_rara::RejectionCode::RequestConflict => {
                    RuntimeRejectionCode::RequestConflict
                }
                agenthub_rara::RejectionCode::Internal => RuntimeRejectionCode::Internal,
            },
        },
    })
}
