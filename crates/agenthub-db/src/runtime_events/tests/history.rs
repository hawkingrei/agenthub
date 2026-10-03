use super::*;

#[tokio::test]
async fn history_pages_receipts_without_allocating_ownership_or_exposing_content() {
    let fixture = Fixture::new().await;
    assert!(
        RuntimeEventStore::load(fixture.pool.clone(), "missing")
            .await
            .unwrap()
            .is_none()
    );
    for index in 0..105 {
        fixture
            .owner
            .prepare_request(
                RuntimeRequestIntent {
                    request_id: &format!("request-{index:03}"),
                    kind: RuntimeRequestKind::Prompt,
                    target_session_id: Some("native"),
                    expected_turn_id: None,
                },
                1,
            )
            .await
            .unwrap();
    }
    fixture
        .stream
        .persist(event("secret-event", 1), &[history("secret-history")])
        .await
        .unwrap();
    fixture
        .stream
        .record_replay_gap(RuntimeReplayGap {
            requested_after: 1,
            oldest_available: 4,
            latest: 5,
        })
        .await
        .unwrap();
    let other = RuntimeEventStore::bind(fixture.pool.clone(), "other-local", "other-runtime")
        .await
        .unwrap();
    other
        .prepare_request(
            RuntimeRequestIntent {
                request_id: "foreign-request",
                kind: RuntimeRequestKind::CreateSession,
                target_session_id: None,
                expected_turn_id: None,
            },
            1,
        )
        .await
        .unwrap();

    let first = fixture.owner.history(1000, None).await.unwrap();
    assert_eq!(first.receipts.len(), 100);
    assert_eq!(first.next_before_request_id.as_deref(), Some("request-005"));
    assert_eq!(first.streams[0].cursor.sequence, 1);
    assert_eq!(first.streams[0].cursor.gap.unwrap().oldest_available, 4);
    assert!(!first.closed);
    let encoded = serde_json::to_string(&first).unwrap();
    for forbidden in [
        "visible conversation",
        "secret-event",
        "secret-history",
        "foreign-request",
        "fingerprint",
    ] {
        assert!(!encoded.contains(forbidden));
    }

    // Receipt status updates do not change page membership or advance event progress.
    let permit = fixture
        .owner
        .mark_request_sent("request-002", 2)
        .await
        .unwrap();
    fixture
        .owner
        .record_request_ack(
            &permit,
            RuntimeRequestAck::Accepted {
                session_id: "native".into(),
                turn_id: Some("turn".into()),
                last_sequence: Some(99),
            },
            3,
        )
        .await
        .unwrap();
    fixture.owner.close(4).await.unwrap();
    let loaded = RuntimeEventStore::load(fixture.pool.clone(), "local")
        .await
        .unwrap()
        .unwrap();
    let next = loaded
        .history(100, first.next_before_request_id.as_deref())
        .await
        .unwrap();
    assert!(next.closed);
    assert!(next.next_before_request_id.is_none());
    assert_eq!(next.receipts.len(), 5);
    assert_eq!(next.receipts[2].request_id, "request-002");
    assert_eq!(next.receipts[2].status, RuntimeRequestStatus::Accepted);
    assert_eq!(next.receipts[0].status, RuntimeRequestStatus::NotSent);
    assert_eq!(next.streams[0].cursor.sequence, 1);
    assert_eq!(loaded.history(0, None).await.unwrap().receipts.len(), 1);
    assert!(loaded.history(1, Some("invalid cursor")).await.is_err());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_event_owners")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn history_reports_bounded_stream_metadata_explicitly() {
    let fixture = Fixture::new().await;
    for index in 0..100 {
        fixture
            .owner
            .bind_stream(&format!("native-{index:03}"))
            .await
            .unwrap();
    }
    let history = fixture.owner.history(1, None).await.unwrap();
    assert!(history.streams_truncated);
    assert_eq!(history.streams.len(), 100);
    assert!(history.receipts.is_empty());
    assert!(history.next_before_request_id.is_none());
}

#[tokio::test]
async fn open_owner_recovery_migrates_an_index_and_pages_only_unclosed_launches() {
    let fixture = Fixture::new().await;
    fixture.owner.close(1).await.unwrap();
    // Model an event database created before the open-owner recovery index existed.
    sqlx::query("DROP INDEX idx_runtime_event_owners_open")
        .execute(&fixture.pool)
        .await
        .unwrap();
    sqlx::raw_sql(
        "WITH RECURSIVE numbers(n) AS (VALUES(0) UNION ALL SELECT n + 1 FROM numbers WHERE n < 304) \
         INSERT INTO runtime_event_owners(runtime_id, local_session_id, closed) \
         SELECT printf('runtime-%03d', n), printf('local-%03d', n), n < 100 FROM numbers",
    ).execute(&fixture.pool).await.unwrap();
    migrate(&fixture.pool).await.unwrap();
    migrate(&fixture.pool).await.unwrap();
    let plan = sqlx::query(
        "EXPLAIN QUERY PLAN SELECT local_session_id, runtime_id FROM runtime_event_owners \
         WHERE closed = 0 AND local_session_id > ? ORDER BY local_session_id LIMIT 100",
    )
    .bind("")
    .fetch_all(&fixture.pool)
    .await
    .unwrap();
    assert!(plan.iter().any(|row| {
        row.get::<String, _>("detail")
            .contains("idx_runtime_event_owners_open")
    }));
    let mut after = None;
    let mut sizes = Vec::new();
    loop {
        let page = RuntimeEventStore::load_open_page(fixture.pool.clone(), after.as_deref())
            .await
            .unwrap();
        if page.is_empty() {
            break;
        }
        sizes.push(page.len());
        for owner in page {
            assert!(owner.local_session_id() >= "local-100");
            after = Some(owner.local_session_id().to_owned());
            owner.close(2).await.unwrap();
        }
    }
    assert_eq!(sizes, [100, 100, 5]);
    assert!(
        RuntimeEventStore::load_open_page(fixture.pool.clone(), None)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        RuntimeEventStore::load_open_page(fixture.pool.clone(), Some("invalid cursor"))
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_event_owners")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(count, 306, "recovery preserves retained ownership evidence");
}
