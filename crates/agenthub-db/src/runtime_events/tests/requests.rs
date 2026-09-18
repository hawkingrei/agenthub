use super::*;

fn intent(id: &str, kind: RuntimeRequestKind) -> RuntimeRequestIntent<'_> {
    RuntimeRequestIntent {
        request_id: id,
        kind,
        target_session_id: (kind != RuntimeRequestKind::CreateSession).then_some("native"),
        expected_turn_id: matches!(
            kind,
            RuntimeRequestKind::Cancel
                | RuntimeRequestKind::Interrupt
                | RuntimeRequestKind::UserAnswer
                | RuntimeRequestKind::PlanAnswer
                | RuntimeRequestKind::ShellAnswer
        )
        .then_some("waiting-turn"),
    }
}

fn accepted(session: &str) -> RuntimeRequestAck {
    RuntimeRequestAck::Accepted {
        session_id: session.into(),
        turn_id: Some("new-turn".into()),
        last_sequence: Some(2),
    }
}

#[tokio::test]
async fn send_permit_is_issued_once_and_ack_is_admission_only() {
    let fixture = Fixture::new().await;
    fixture
        .owner
        .prepare_request(intent("request", RuntimeRequestKind::Prompt), 1)
        .await
        .unwrap();
    let permit = fixture.owner.mark_request_sent("request", 2).await.unwrap();
    assert_eq!(permit.request_id(), "request");
    assert!(fixture.owner.mark_request_sent("request", 3).await.is_err());
    fixture
        .owner
        .record_request_ack(&permit, accepted("native"), 4)
        .await
        .unwrap();
    fixture
        .owner
        .record_request_ack(&permit, accepted("native"), 5)
        .await
        .unwrap();
    let receipt = fixture
        .owner
        .request_receipt("request")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.status, RuntimeRequestStatus::Accepted);
    assert_eq!(receipt.created_at, 1);
    assert_eq!(receipt.updated_at, 4);
    assert_eq!(fixture.stream.cursor().await.unwrap().sequence, 0);
    assert_eq!(fixture.history_count().await, 0);
    assert!(
        fixture
            .owner
            .prepare_request(intent("request", RuntimeRequestKind::Prompt), 6)
            .await
            .is_err()
    );
    assert!(
        fixture
            .owner
            .record_request_ack(
                &permit,
                RuntimeRequestAck::Rejected {
                    code: RuntimeRejectionCode::Busy,
                },
                6
            )
            .await
            .is_err()
    );
    assert!(
        fixture
            .owner
            .record_submission_failure(&permit, RuntimeSubmissionFailure::OutcomeUnknown, 6)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn close_preserves_acks_and_marks_unresolved_sends_unknown() {
    let fixture = Fixture::new().await;
    for id in ["prepared", "sent", "acknowledged"] {
        fixture
            .owner
            .prepare_request(intent(id, RuntimeRequestKind::Prompt), 1)
            .await
            .unwrap();
    }
    let unresolved = fixture.owner.mark_request_sent("sent", 2).await.unwrap();
    let acked = fixture
        .owner
        .mark_request_sent("acknowledged", 2)
        .await
        .unwrap();
    fixture
        .owner
        .record_request_ack(&acked, accepted("native"), 3)
        .await
        .unwrap();
    fixture.owner.close(4).await.unwrap();
    for (id, expected) in [
        ("prepared", RuntimeRequestStatus::NotSent),
        ("sent", RuntimeRequestStatus::OutcomeUnknown),
        ("acknowledged", RuntimeRequestStatus::Accepted),
    ] {
        assert_eq!(
            fixture
                .owner
                .request_receipt(id)
                .await
                .unwrap()
                .unwrap()
                .status,
            expected
        );
        assert!(fixture.owner.mark_request_sent(id, 5).await.is_err());
    }
    // A correlated late ACK can settle uncertainty without issuing another send permit.
    fixture
        .owner
        .record_request_ack(&unresolved, accepted("native"), 6)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .owner
            .request_receipt("sent")
            .await
            .unwrap()
            .unwrap()
            .status,
        RuntimeRequestStatus::Accepted
    );
    assert!(
        fixture
            .owner
            .prepare_request(intent("fresh-retry", RuntimeRequestKind::Prompt), 7)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn crash_after_send_intent_does_not_allow_replay_on_reopen() {
    let fixture = Fixture::new().await;
    fixture
        .owner
        .prepare_request(intent("request", RuntimeRequestKind::Prompt), 1)
        .await
        .unwrap();
    let permit = fixture.owner.mark_request_sent("request", 2).await.unwrap();
    drop(permit);
    fixture.pool.close().await;
    let pool = crate::AgentEventDbRouter::new(fixture.directory.clone())
        .pool_for_agent("actor")
        .await
        .unwrap();
    let owner = RuntimeEventStore::bind(pool.clone(), "local", "runtime")
        .await
        .unwrap();
    assert_eq!(
        owner
            .request_receipt("request")
            .await
            .unwrap()
            .unwrap()
            .status,
        RuntimeRequestStatus::Sent
    );
    assert!(owner.mark_request_sent("request", 3).await.is_err());
    owner.close(3).await.unwrap();
    assert_eq!(
        owner
            .request_receipt("request")
            .await
            .unwrap()
            .unwrap()
            .status,
        RuntimeRequestStatus::OutcomeUnknown
    );
    assert!(
        owner
            .prepare_request(intent("request", RuntimeRequestKind::Prompt), 4)
            .await
            .is_err()
    );
    pool.close().await;
}

#[tokio::test]
async fn creation_ack_binds_owned_stream_without_advancing_event_cursor() {
    let fixture = Fixture::new().await;
    fixture
        .owner
        .prepare_request(intent("create", RuntimeRequestKind::CreateSession), 1)
        .await
        .unwrap();
    let permit = fixture.owner.mark_request_sent("create", 2).await.unwrap();
    assert!(fixture.owner.stream("created").await.unwrap().is_none());
    fixture
        .owner
        .record_request_ack(
            &permit,
            RuntimeRequestAck::Accepted {
                session_id: "created".into(),
                turn_id: None,
                last_sequence: Some(10),
            },
            3,
        )
        .await
        .unwrap();
    let stream = fixture.owner.stream("created").await.unwrap().unwrap();
    assert_eq!(stream.cursor().await.unwrap().sequence, 0);
    assert_eq!(
        fixture
            .owner
            .request_receipt("create")
            .await
            .unwrap()
            .unwrap()
            .status,
        RuntimeRequestStatus::Accepted
    );
}

#[tokio::test]
async fn explicit_failures_and_rejections_remain_distinct_and_safe() {
    let fixture = Fixture::new().await;
    for (id, failure, status) in [
        (
            "unsent",
            RuntimeSubmissionFailure::NotSent,
            RuntimeRequestStatus::NotSent,
        ),
        (
            "unknown",
            RuntimeSubmissionFailure::OutcomeUnknown,
            RuntimeRequestStatus::OutcomeUnknown,
        ),
    ] {
        fixture
            .owner
            .prepare_request(intent(id, RuntimeRequestKind::Prompt), 1)
            .await
            .unwrap();
        let permit = fixture.owner.mark_request_sent(id, 2).await.unwrap();
        fixture
            .owner
            .record_submission_failure(&permit, failure, 3)
            .await
            .unwrap();
        fixture
            .owner
            .record_submission_failure(&permit, failure, 3)
            .await
            .unwrap();
        assert_eq!(
            fixture
                .owner
                .request_receipt(id)
                .await
                .unwrap()
                .unwrap()
                .status,
            status
        );
        assert!(fixture.owner.mark_request_sent(id, 4).await.is_err());
    }
    fixture
        .owner
        .prepare_request(intent("rejected", RuntimeRequestKind::Prompt), 1)
        .await
        .unwrap();
    let permit = fixture
        .owner
        .mark_request_sent("rejected", 2)
        .await
        .unwrap();
    fixture
        .owner
        .record_request_ack(
            &permit,
            RuntimeRequestAck::Rejected {
                code: RuntimeRejectionCode::Busy,
            },
            3,
        )
        .await
        .unwrap();
    let receipt = fixture
        .owner
        .request_receipt("rejected")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.status, RuntimeRequestStatus::Rejected);
    let json = serde_json::to_value(receipt).unwrap();
    assert_eq!(
        json["ack"],
        serde_json::json!({"status": "rejected", "code": "busy"})
    );
    fixture
        .owner
        .prepare_request(intent("queued", RuntimeRequestKind::FollowUp), 1)
        .await
        .unwrap();
    let permit = fixture.owner.mark_request_sent("queued", 2).await.unwrap();
    fixture
        .owner
        .record_request_ack(
            &permit,
            RuntimeRequestAck::Queued {
                session_id: "native".into(),
            },
            3,
        )
        .await
        .unwrap();
    assert_eq!(
        fixture
            .owner
            .request_receipt("queued")
            .await
            .unwrap()
            .unwrap()
            .status,
        RuntimeRequestStatus::Queued
    );
}

#[tokio::test]
async fn request_scope_and_pending_turn_are_required_before_send() {
    let fixture = Fixture::new().await;
    for kind in [
        RuntimeRequestKind::Cancel,
        RuntimeRequestKind::Interrupt,
        RuntimeRequestKind::UserAnswer,
        RuntimeRequestKind::PlanAnswer,
        RuntimeRequestKind::ShellAnswer,
    ] {
        assert!(
            fixture
                .owner
                .prepare_request(
                    RuntimeRequestIntent {
                        expected_turn_id: None,
                        ..intent("bad", kind)
                    },
                    1
                )
                .await
                .is_err()
        );
    }
    assert!(
        fixture
            .owner
            .prepare_request(
                RuntimeRequestIntent {
                    target_session_id: Some("not-owned"),
                    ..intent("bad", RuntimeRequestKind::Prompt)
                },
                1
            )
            .await
            .is_err()
    );
    assert!(
        fixture
            .owner
            .prepare_request(
                RuntimeRequestIntent {
                    expected_turn_id: Some("turn"),
                    ..intent("bad", RuntimeRequestKind::Prompt)
                },
                1
            )
            .await
            .is_err()
    );
    fixture
        .owner
        .prepare_request(intent("answer", RuntimeRequestKind::UserAnswer), 1)
        .await
        .unwrap();
    let permit = fixture.owner.mark_request_sent("answer", 2).await.unwrap();
    assert!(
        fixture
            .owner
            .record_request_ack(&permit, accepted("another-native"), 3)
            .await
            .is_err()
    );
    // Answer admission starts a new turn; it need not equal the original waiting turn.
    fixture
        .owner
        .record_request_ack(&permit, accepted("native"), 3)
        .await
        .unwrap();
    let receipt = fixture
        .owner
        .request_receipt("answer")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.expected_turn_id.as_deref(), Some("waiting-turn"));
    let other = RuntimeEventStore::bind(fixture.pool.clone(), "another-local", "another-runtime")
        .await
        .unwrap();
    assert!(
        other
            .record_request_ack(&permit, accepted("native"), 4)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn concurrent_send_claims_issue_one_permit() {
    let fixture = Fixture::new().await;
    fixture
        .owner
        .prepare_request(intent("request", RuntimeRequestKind::Prompt), 1)
        .await
        .unwrap();
    let mut pending = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let owner = fixture.owner.clone();
        pending.spawn(async move { owner.mark_request_sent("request", 2).await.is_ok() });
    }
    let mut permits = 0;
    while let Some(result) = pending.join_next().await {
        permits += usize::from(result.unwrap());
    }
    assert_eq!(permits, 1);
}

#[tokio::test]
async fn cancellation_ack_must_name_the_fenced_turn() {
    let fixture = Fixture::new().await;
    fixture
        .owner
        .prepare_request(intent("cancel", RuntimeRequestKind::Cancel), 1)
        .await
        .unwrap();
    let permit = fixture.owner.mark_request_sent("cancel", 2).await.unwrap();
    assert!(
        fixture
            .owner
            .record_request_ack(&permit, accepted("native"), 3)
            .await
            .is_err()
    );
    fixture
        .owner
        .record_request_ack(
            &permit,
            RuntimeRequestAck::Accepted {
                session_id: "native".into(),
                turn_id: Some("waiting-turn".into()),
                last_sequence: Some(1),
            },
            3,
        )
        .await
        .unwrap();
    assert_eq!(
        fixture
            .owner
            .request_receipt("cancel")
            .await
            .unwrap()
            .unwrap()
            .status,
        RuntimeRequestStatus::Accepted
    );
}

#[tokio::test]
async fn request_receipt_limit_prevents_unbounded_preparation() {
    let fixture = Fixture::new().await;
    sqlx::query("WITH RECURSIVE numbers(n) AS (VALUES (1) UNION ALL SELECT n + 1 FROM numbers WHERE n < 4096) \
        INSERT INTO runtime_control_receipts (runtime_id, request_id, kind, status, created_at, updated_at) \
        SELECT 'runtime', 'request-' || n, 'create_session', 'prepared', 1, 1 FROM numbers")
        .execute(&fixture.pool).await.unwrap();
    assert!(matches!(
        fixture
            .owner
            .prepare_request(intent("overflow", RuntimeRequestKind::CreateSession), 2)
            .await
            .unwrap_err()
            .downcast_ref(),
        Some(RuntimeEventError::ReceiptCapacity)
    ));
    assert!(
        fixture
            .owner
            .request_receipt("overflow")
            .await
            .unwrap()
            .is_none()
    );
}
