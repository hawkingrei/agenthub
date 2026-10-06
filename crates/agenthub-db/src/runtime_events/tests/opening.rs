use super::*;

async fn accept(owner: &RuntimeEventStore, id: &str, kind: RuntimeRequestKind) {
    owner
        .prepare_request(
            RuntimeRequestIntent {
                request_id: id,
                kind,
                target_session_id: (kind == RuntimeRequestKind::ResumeSession).then_some("native"),
                expected_turn_id: None,
            },
            1,
        )
        .await
        .unwrap();
    let permit = owner.mark_request_sent(id, 2).await.unwrap();
    owner
        .record_request_ack(
            &permit,
            RuntimeRequestAck::Accepted {
                session_id: "native".into(),
                turn_id: None,
                last_sequence: Some(1),
            },
            3,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn closed_opening_requires_one_accepted_exact_conversation_and_survives_reload() {
    for kind in [
        RuntimeRequestKind::CreateSession,
        RuntimeRequestKind::ResumeSession,
    ] {
        let fixture = Fixture::new().await;
        accept(&fixture.owner, "open", kind).await;
        assert!(
            fixture
                .owner
                .accepted_closed_opening()
                .await
                .unwrap()
                .is_none()
        );
        fixture.owner.close(4).await.unwrap();
        let loaded = RuntimeEventStore::load(fixture.pool.clone(), "local")
            .await
            .unwrap()
            .unwrap();
        let evidence = loaded.accepted_closed_opening().await.unwrap().unwrap();
        assert_eq!(evidence.local_session_id, "local");
        assert_eq!(evidence.native_session_id, "native");
        assert_eq!(evidence.kind, kind);
        assert_eq!(fixture.stream.cursor().await.unwrap().sequence, 0);
    }
}

#[tokio::test]
async fn closed_opening_never_recovers_missing_rejected_or_uncertain_results() {
    for status in ["missing", "prepared", "sent", "rejected"] {
        let fixture = Fixture::new().await;
        if status != "missing" {
            fixture
                .owner
                .prepare_request(
                    RuntimeRequestIntent {
                        request_id: "open",
                        kind: RuntimeRequestKind::CreateSession,
                        target_session_id: None,
                        expected_turn_id: None,
                    },
                    1,
                )
                .await
                .unwrap();
        }
        if matches!(status, "sent" | "rejected") {
            let permit = fixture.owner.mark_request_sent("open", 2).await.unwrap();
            if status == "rejected" {
                fixture
                    .owner
                    .record_request_ack(
                        &permit,
                        RuntimeRequestAck::Rejected {
                            code: RuntimeRejectionCode::Internal,
                        },
                        3,
                    )
                    .await
                    .unwrap();
            }
        }
        fixture.owner.close(4).await.unwrap();
        assert!(
            fixture
                .owner
                .accepted_closed_opening()
                .await
                .unwrap()
                .is_none(),
            "{status}"
        );
    }
}

#[tokio::test]
async fn closed_opening_rejects_ambiguous_receipts_and_inconsistent_streams() {
    for change in [
        "multiple",
        "missing-stream",
        "foreign-stream",
        "bad-target",
        "non-accepted-ack",
        "bad-turn",
    ] {
        let fixture = Fixture::new().await;
        accept(&fixture.owner, "open", RuntimeRequestKind::CreateSession).await;
        if change == "multiple" {
            accept(&fixture.owner, "other", RuntimeRequestKind::ResumeSession).await;
        }
        let mutation = match change {
            "missing-stream" => Some("DELETE FROM runtime_event_streams"),
            "foreign-stream" => {
                Some("UPDATE runtime_event_streams SET native_session_id = 'foreign'")
            }
            "bad-target" => {
                Some("UPDATE runtime_control_receipts SET target_session_id = 'foreign'")
            }
            "non-accepted-ack" => Some(
                "UPDATE runtime_control_receipts SET ack_json = '{\"status\":\"rejected\",\"code\":\"internal\"}'",
            ),
            "bad-turn" => Some(
                "UPDATE runtime_control_receipts SET ack_json = json_set(ack_json, '$.turn_id', 'unexpected')",
            ),
            _ => None,
        };
        if let Some(query) = mutation {
            sqlx::query(query).execute(&fixture.pool).await.unwrap();
        }
        fixture.owner.close(4).await.unwrap();
        assert!(
            fixture.owner.accepted_closed_opening().await.is_err(),
            "{change}"
        );
    }
}
