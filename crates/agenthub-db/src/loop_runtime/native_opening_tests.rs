use crate::runtime_events::{
    RuntimeEventStore, RuntimeRequestAck, RuntimeRequestIntent, RuntimeRequestKind,
};

use super::*;

async fn evidence(
    fixture: &Fixture,
    local: &str,
    kind: RuntimeRequestKind,
) -> crate::runtime_events::RuntimeOpeningEvidence {
    let router = crate::AgentEventDbRouter::new(fixture.path.parent().unwrap().join("events"));
    let pool = router.pool_for_agent("worker").await.unwrap();
    let store = RuntimeEventStore::bind(pool.clone(), local, &format!("runtime-{local}"))
        .await
        .unwrap();
    store
        .prepare_request(
            RuntimeRequestIntent {
                request_id: "open",
                kind,
                target_session_id: (kind == RuntimeRequestKind::ResumeSession)
                    .then_some("conversation"),
                expected_turn_id: None,
            },
            1,
        )
        .await
        .unwrap();
    let permit = store.mark_request_sent("open", 2).await.unwrap();
    store
        .record_request_ack(
            &permit,
            RuntimeRequestAck::Accepted {
                session_id: "conversation".into(),
                turn_id: None,
                last_sequence: Some(1),
            },
            3,
        )
        .await
        .unwrap();
    store.close(4).await.unwrap();
    let evidence = store.accepted_closed_opening().await.unwrap().unwrap();
    pool.close().await;
    evidence
}

#[tokio::test]
async fn native_opening_reconciliation_repairs_committed_create_and_resume_once() {
    let mut fixture = fixture().await;
    let digest = "a".repeat(64);
    let first = starting(&fixture, LoopSessionPolicy::Resume, 101).await;
    fixture
        .store
        .begin_native_session(&first, &digest, 102)
        .await
        .unwrap();
    let created = evidence(
        &fixture,
        first.session_id.as_deref().unwrap(),
        RuntimeRequestKind::CreateSession,
    )
    .await;
    fixture
        .store
        .cleanup_verified(&first, LoopCleanupDisposition::Exited, 103)
        .await
        .unwrap();
    fixture.store.pool.close().await;
    fixture.store = LoopStore::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    let second = starting(&fixture, LoopSessionPolicy::Resume, 104).await;
    assert_eq!(
        fixture
            .store
            .native_opening_to_reconcile(&second, &digest, 105)
            .await
            .unwrap(),
        first.session_id
    );
    fixture
        .store
        .reconcile_native_opening(&second, &created, 105)
        .await
        .unwrap();
    fixture
        .store
        .reconcile_native_opening(&second, &created, 105)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .native_opening_to_reconcile(&second, &digest, 105)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        fixture
            .store
            .begin_native_session(&second, &digest, 105)
            .await
            .unwrap()
            .as_deref(),
        Some("conversation")
    );
    let resumed = evidence(
        &fixture,
        second.session_id.as_deref().unwrap(),
        RuntimeRequestKind::ResumeSession,
    )
    .await;
    fixture
        .store
        .cleanup_verified(&second, LoopCleanupDisposition::Exited, 106)
        .await
        .unwrap();
    let third = starting(&fixture, LoopSessionPolicy::Resume, 107).await;
    assert!(
        fixture
            .store
            .reconcile_native_opening(&third, &created, 108)
            .await
            .is_err(),
        "old proof must not repair a different launch"
    );
    fixture
        .store
        .reconcile_native_opening(&third, &resumed, 108)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .begin_native_session(&third, &digest, 108)
            .await
            .unwrap()
            .as_deref(),
        Some("conversation")
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM loop_activation_events WHERE kind = 'native_opening_reconciled'",
    )
    .fetch_one(&fixture.store.pool)
    .await
    .unwrap();
    assert_eq!(count, 2, "one audit event per repaired opening");
    fixture.close().await;
}

#[tokio::test]
async fn native_opening_reconciliation_requires_current_authority_and_exact_retirement() {
    let fixture = fixture().await;
    let digest = "a".repeat(64);
    let first = starting(&fixture, LoopSessionPolicy::Resume, 101).await;
    fixture
        .store
        .begin_native_session(&first, &digest, 102)
        .await
        .unwrap();
    let accepted = evidence(
        &fixture,
        first.session_id.as_deref().unwrap(),
        RuntimeRequestKind::CreateSession,
    )
    .await;
    assert!(
        fixture
            .store
            .reconcile_native_opening(&first, &accepted, 102)
            .await
            .is_err()
    );
    fixture
        .store
        .cleanup_verified(&first, LoopCleanupDisposition::Exited, 103)
        .await
        .unwrap();
    let next = starting(&fixture, LoopSessionPolicy::Resume, 104).await;
    for change in [
        "generation",
        "owner",
        "local",
        "membership",
        "retirement",
        "foreign-binding",
        "conflicting-native",
    ] {
        let mut current = next.clone();
        match change {
            "generation" => current.generation += 1,
            "owner" => current.owner_id = "other".into(),
            "local" => current.session_id = Some("other".into()),
            "membership" => {
                sqlx::query("UPDATE team_definitions SET spec_json = '{}' WHERE id = 'team'")
                    .execute(&fixture.store.pool)
                    .await
                    .unwrap();
            }
            "retirement" => {
                sqlx::query("UPDATE loop_activation_events SET generation = generation + 10 WHERE kind = 'cleanup_verified'").execute(&fixture.store.pool).await.unwrap();
            }
            "foreign-binding" => current.team_id = "elsewhere".into(),
            "conflicting-native" => {
                sqlx::query("UPDATE loop_native_sessions SET native_session_id = 'unexpected'")
                    .execute(&fixture.store.pool)
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            fixture
                .store
                .reconcile_native_opening(&current, &accepted, 105)
                .await
                .is_err(),
            "{change}"
        );
        let restore = match change {
            "membership" => Some(
                "UPDATE team_definitions SET spec_json = '{\"members\":[{\"member_id\":\"worker\"},{\"member_id\":\"other\"}]}' WHERE id = 'team'",
            ),
            "retirement" => Some(
                "UPDATE loop_activation_events SET generation = generation - 10 WHERE kind = 'cleanup_verified'",
            ),
            "conflicting-native" => {
                Some("UPDATE loop_native_sessions SET native_session_id = NULL")
            }
            _ => None,
        };
        if let Some(query) = restore {
            sqlx::query(query)
                .execute(&fixture.store.pool)
                .await
                .unwrap();
        }
    }
    assert!(
        fixture
            .store
            .native_opening_to_reconcile(&next, &"b".repeat(64), 105)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        fixture
            .store
            .reconcile_native_opening(&next, &accepted, 200)
            .await
            .is_err()
    );
    fixture
        .store
        .reconcile_native_opening(&next, &accepted, 105)
        .await
        .unwrap();
    fixture.close().await;
}
