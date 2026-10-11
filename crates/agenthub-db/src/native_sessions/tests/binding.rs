use super::*;

async fn evidence(
    fixture: &Fixture,
    local: &str,
    kind: RuntimeRequestKind,
) -> crate::runtime_events::RuntimeOpeningEvidence {
    let router = crate::AgentEventDbRouter::new(fixture.directory.join("events"));
    let pool = router.pool_for_agent("agent").await.unwrap();
    let events = RuntimeEventStore::bind(pool.clone(), local, &format!("runtime-{local}"))
        .await
        .unwrap();
    events
        .prepare_request(
            RuntimeRequestIntent {
                request_id: "open",
                kind,
                target_session_id: (kind == RuntimeRequestKind::ResumeSession)
                    .then_some("conversation"),
                expected_turn_id: None,
            },
            100,
        )
        .await
        .unwrap();
    let permit = events.mark_request_sent("open", 101).await.unwrap();
    events
        .record_request_ack(
            &permit,
            RuntimeRequestAck::Accepted {
                session_id: "conversation".into(),
                turn_id: None,
                last_sequence: Some(1),
            },
            102,
        )
        .await
        .unwrap();
    assert!(events.accepted_closed_opening().await.unwrap().is_none());
    events.close(103).await.unwrap();
    let evidence = events.accepted_closed_opening().await.unwrap().unwrap();
    pool.close().await;
    evidence
}

#[tokio::test]
async fn standalone_resume_preserves_native_identity_and_rejects_changed_configuration() {
    let mut fixture = Fixture::new().await;
    let digest = "a".repeat(64);
    let first = fixture.guarded("agent").await;
    assert!(
        fixture
            .store
            .begin_conversation(&first, &digest, LoopSessionPolicy::Resume, 102)
            .await
            .unwrap()
            .is_none()
    );
    fixture
        .store
        .bind_conversation(&first, "conversation", 103)
        .await
        .unwrap();
    fixture
        .store
        .bind_conversation(&first, "conversation", 104)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .bind_conversation(&first, "replacement", 104)
            .await
            .is_err()
    );
    assert!(
        fixture
            .store
            .begin_conversation(&first, &digest, LoopSessionPolicy::Fresh, 104)
            .await
            .is_err()
    );
    fixture.store.cleanup_verified(&first, 105).await.unwrap();
    fixture.reopen().await;
    let second = fixture.guarded("agent").await;
    assert!(
        fixture
            .store
            .bind_conversation(&first, "conversation", 106)
            .await
            .is_err()
    );
    assert!(matches!(
        fixture
            .store
            .begin_conversation(&second, &"b".repeat(64), LoopSessionPolicy::Resume, 106)
            .await
            .unwrap_err()
            .downcast_ref(),
        Some(NativeSessionError::ConfigurationChanged)
    ));
    assert_eq!(
        fixture
            .store
            .begin_conversation(&second, &digest, LoopSessionPolicy::Resume, 106)
            .await
            .unwrap()
            .as_deref(),
        Some("conversation")
    );
    assert!(
        fixture
            .store
            .bind_conversation(&second, "different", 107)
            .await
            .is_err()
    );
    fixture
        .store
        .bind_conversation(&second, "conversation", 107)
        .await
        .unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn standalone_unknown_opening_needs_explicit_fresh_after_cleanup() {
    let fixture = Fixture::new().await;
    let digest = "a".repeat(64);
    let first = fixture.guarded("agent").await;
    fixture
        .store
        .begin_conversation(&first, &digest, LoopSessionPolicy::Resume, 102)
        .await
        .unwrap();
    assert!(fixture.store.clear_conversation("agent").await.is_err());
    fixture.store.cleanup_verified(&first, 103).await.unwrap();
    let second = fixture.guarded("agent").await;
    assert!(matches!(
        fixture
            .store
            .begin_conversation(&second, &digest, LoopSessionPolicy::Resume, 104)
            .await
            .unwrap_err()
            .downcast_ref(),
        Some(NativeSessionError::OpeningUncertain)
    ));
    assert_eq!(
        fixture
            .store
            .opening_to_reconcile(&second, &digest)
            .await
            .unwrap()
            .as_deref(),
        Some(first.local_session_id.as_str())
    );
    assert!(
        fixture
            .store
            .begin_conversation(&second, &"b".repeat(64), LoopSessionPolicy::Fresh, 105)
            .await
            .unwrap()
            .is_none()
    );
    fixture
        .store
        .bind_conversation(&second, "fresh", 106)
        .await
        .unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn standalone_explicit_reset_preserves_generation_and_old_owner_fences() {
    let fixture = Fixture::new().await;
    let digest = "a".repeat(64);
    let first = fixture.guarded("agent").await;
    fixture
        .store
        .begin_conversation(&first, &digest, LoopSessionPolicy::Fresh, 102)
        .await
        .unwrap();
    fixture
        .store
        .bind_conversation(&first, "conversation", 103)
        .await
        .unwrap();
    assert!(fixture.store.clear_conversation("agent").await.is_err());
    fixture.store.cleanup_verified(&first, 104).await.unwrap();
    fixture.store.clear_conversation("agent").await.unwrap();
    fixture.store.clear_conversation("agent").await.unwrap();
    let second = fixture.guarded("agent").await;
    assert_eq!(second.generation, first.generation + 1);
    assert!(
        fixture
            .store
            .begin_conversation(&second, &digest, LoopSessionPolicy::Resume, 105)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        fixture
            .store
            .bind_conversation(&first, "old", 106)
            .await
            .is_err()
    );
    fixture.close().await;
}

#[tokio::test]
async fn standalone_closed_opening_receipt_repairs_exact_retired_launch_only() {
    let mut fixture = Fixture::new().await;
    let digest = "a".repeat(64);
    let mut first = fixture.guarded("agent").await;
    for kind in [
        RuntimeRequestKind::CreateSession,
        RuntimeRequestKind::ResumeSession,
    ] {
        fixture
            .store
            .begin_conversation(&first, &digest, LoopSessionPolicy::Resume, 102)
            .await
            .unwrap();
        let accepted = evidence(&fixture, &first.local_session_id, kind).await;
        assert!(
            fixture
                .store
                .reconcile_opening(&first, &digest, &accepted, 104)
                .await
                .is_err(),
            "closed receipts do not prove executor retirement"
        );
        fixture.store.cleanup_verified(&first, 105).await.unwrap();
        fixture.reopen().await;
        let next = fixture.guarded("agent").await;
        assert!(
            fixture
                .store
                .reconcile_opening(&next, &"b".repeat(64), &accepted, 106)
                .await
                .is_err()
        );
        let foreign = crate::runtime_events::RuntimeOpeningEvidence {
            local_session_id: "foreign".into(),
            native_session_id: "conversation".into(),
            kind,
        };
        assert!(
            fixture
                .store
                .reconcile_opening(&next, &digest, &foreign, 106)
                .await
                .is_err()
        );
        fixture
            .store
            .reconcile_opening(&next, &digest, &accepted, 106)
            .await
            .unwrap();
        fixture
            .store
            .reconcile_opening(&next, &digest, &accepted, 107)
            .await
            .unwrap();
        assert!(
            fixture
                .store
                .opening_to_reconcile(&next, &digest)
                .await
                .unwrap()
                .is_none()
        );
        first = next;
    }
    assert_eq!(
        fixture
            .store
            .begin_conversation(&first, &digest, LoopSessionPolicy::Resume, 108)
            .await
            .unwrap()
            .as_deref(),
        Some("conversation")
    );
    fixture.close().await;
}

#[tokio::test]
async fn standalone_binding_rejects_unstarted_ended_and_foreign_launches() {
    let fixture = Fixture::new().await;
    let owner = fixture.reserve("agent").await;
    let digest = "a".repeat(64);
    assert!(
        fixture
            .store
            .begin_conversation(&owner, &digest, LoopSessionPolicy::Resume, 101)
            .await
            .is_err()
    );
    fixture.store.authorize_spawn(&owner, 102).await.unwrap();
    assert!(fixture.store.verify_live(&owner).await.is_err());
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES (?, 'other', 'running', 103)")
        .bind(&owner.local_session_id).execute(&fixture.store.pool).await.unwrap();
    assert!(
        fixture
            .store
            .begin_conversation(&owner, &digest, LoopSessionPolicy::Fresh, 104)
            .await
            .is_err()
    );
    sqlx::query("UPDATE agent_sessions SET agent_id = 'agent', ended_at = 105 WHERE id = ?")
        .bind(&owner.local_session_id)
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .bind_conversation(&owner, "conversation", 106)
            .await
            .is_err()
    );
    assert!(
        fixture
            .store
            .begin_conversation(&owner, "bad-digest", LoopSessionPolicy::Fresh, 106)
            .await
            .is_err()
    );
    fixture.close().await;
}
