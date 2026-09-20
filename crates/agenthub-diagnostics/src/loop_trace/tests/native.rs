use agenthub_db::runtime_events::{
    RuntimeEventIdentity, RuntimeEventStore, RuntimeReplayGap, RuntimeRequestAck,
    RuntimeRequestIntent, RuntimeRequestKind, RuntimeRequestStatus,
};

use super::*;

#[tokio::test]
async fn activation_trace_reopens_native_receipts_for_finished_and_interrupted_executions() {
    for finished in [true, false] {
        let fixture = Fixture::new().await;
        let id = fixture.pending().await;
        let reservation = fixture.running(&id).await;
        let router = agenthub_db::AgentEventDbRouter::new(fixture.directory.clone());
        let pool = router.pool_for_agent("actor").await.unwrap();
        let owner = RuntimeEventStore::bind(pool.clone(), "session", "runtime")
            .await
            .unwrap();
        let stream = owner.bind_stream("native").await.unwrap();
        stream
            .persist(
                RuntimeEventIdentity {
                    event_id: "private-event",
                    sequence: 1,
                    fingerprint: &[1; 32],
                },
                &[],
            )
            .await
            .unwrap();
        stream
            .record_replay_gap(RuntimeReplayGap {
                requested_after: 1,
                oldest_available: 4,
                latest: 5,
            })
            .await
            .unwrap();
        for request_id in ["entry", "uncertain"] {
            owner
                .prepare_request(
                    RuntimeRequestIntent {
                        request_id,
                        kind: RuntimeRequestKind::Prompt,
                        target_session_id: Some("native"),
                        expected_turn_id: None,
                    },
                    fixture.now,
                )
                .await
                .unwrap();
            let permit = owner
                .mark_request_sent(request_id, fixture.now)
                .await
                .unwrap();
            if request_id == "entry" {
                owner
                    .record_request_ack(
                        &permit,
                        RuntimeRequestAck::Accepted {
                            session_id: "native".into(),
                            turn_id: Some("turn".into()),
                            last_sequence: Some(5),
                        },
                        fixture.now,
                    )
                    .await
                    .unwrap();
            }
        }
        owner.close(fixture.now + 1).await.unwrap();
        RuntimeEventStore::bind(pool.clone(), "other-session", "foreign-runtime")
            .await
            .unwrap()
            .bind_stream("foreign-native")
            .await
            .unwrap();
        if finished {
            fixture
                .store
                .finish(
                    &reservation,
                    &LoopOutcome {
                        kind: LoopOutcomeKind::NoActionableWork,
                        wait_reason: None,
                        task_note_id: None,
                        continuation: None,
                    },
                    fixture.now + 1,
                )
                .await
                .unwrap();
        }
        fixture
            .store
            .cleanup_verified(
                &reservation,
                LoopCleanupDisposition::Exited,
                fixture.now + 2,
            )
            .await
            .unwrap();
        pool.close().await;

        let report = collect_from_pool(
            &fixture.pool,
            fixture.directory.clone(),
            AgentTraceRequest {
                activation_id: Some(id),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let trace = report.activation.as_ref().unwrap();
        assert_eq!(
            trace.activation.state,
            if finished {
                LoopActivationState::Finished
            } else {
                LoopActivationState::Interrupted
            }
        );
        let history = trace.runtime.as_ref().unwrap();
        assert!(history.closed);
        assert_eq!(history.local_session_id, "session");
        assert_eq!(history.streams[0].cursor.sequence, 1);
        assert_eq!(history.streams[0].cursor.gap.unwrap().oldest_available, 4);
        assert_eq!(
            history.receipts[0].status,
            RuntimeRequestStatus::OutcomeUnknown
        );
        assert_eq!(history.receipts[1].status, RuntimeRequestStatus::Accepted);
        let rendered = render_human(&report);
        assert!(rendered.contains("loop.runtime.stream: native_session=native cursor=1"));
        assert!(rendered.contains("status=OutcomeUnknown"));
        for private in [
            "private-event",
            "fingerprint",
            "foreign-runtime",
            "foreign-native",
        ] {
            assert!(!rendered.contains(private));
            assert!(!serde_json::to_string(&report).unwrap().contains(private));
        }
        let decoded: crate::agent_trace::AgentTraceReport =
            serde_json::from_value(serde_json::to_value(&report).unwrap()).unwrap();
        assert_eq!(decoded, report);
        fixture.close().await;
    }
}
