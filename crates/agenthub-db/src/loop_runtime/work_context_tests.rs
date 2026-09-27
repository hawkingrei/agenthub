use agenthub_agent_domain::loop_runtime::LoopAdmission;
use agenthub_agent_domain::loop_scheduling::{LoopRegistrationInput, LoopSchedule};

use super::*;

#[tokio::test]
async fn loop_work_context_excludes_revoked_sources_and_preserves_pagination_and_history() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    for index in 0..5 {
        fixture
            .store
            .register_schedule(
                &LoopRegistrationInput {
                    actor_id: "worker".into(),
                    team_id: "team".into(),
                    source_key: format!("scheduled:{index}"),
                    schedule: LoopSchedule::Due { due_at: 101 },
                    work_task_id: None,
                    references: LoopSourceReferences::default(),
                },
                100,
            )
            .await
            .unwrap();
    }
    let mut firings = fixture.store.reconcile_schedules(101).await.unwrap();
    firings.sort_by(|left, right| left.receipt.trigger_id.cmp(&right.receipt.trigger_id));
    assert_eq!(firings.len(), 5);
    let activation_id = &firings[0].receipt.activation_id;
    assert!(
        firings
            .iter()
            .all(|firing| &firing.receipt.activation_id == activation_id)
    );
    let LoopAdmission::Admitted(reservation) = fixture
        .store
        .admit("team", activation_id, "daemon", 102)
        .await
        .unwrap()
    else {
        panic!("not admitted");
    };
    let original = fixture
        .store
        .work_context(&reservation, None, 1, 102)
        .await
        .unwrap();
    assert_eq!(original.sources[0].id, firings[0].receipt.trigger_id);
    for index in [0, 2, 4] {
        fixture
            .store
            .revoke_schedule("team", &firings[index].registration_id, 103)
            .await
            .unwrap();
    }

    // Revoked rows before, between and after live sources must not consume a page slot.
    let first = fixture
        .store
        .work_context(&reservation, None, 1, 103)
        .await
        .unwrap();
    assert_eq!(first.sources.len(), 1);
    assert_eq!(first.sources[0].id, firings[1].receipt.trigger_id);
    assert!(!first.sources[0].revoked);
    assert_eq!(
        first.next_cursor.as_deref(),
        Some(firings[1].receipt.trigger_id.as_str())
    );
    assert!(
        fixture
            .store
            .work_source(&reservation, &firings[1].receipt.trigger_id, 103)
            .await
            .is_ok()
    );
    let revoked = fixture
        .store
        .work_source(&reservation, &original.sources[0].id, 103)
        .await
        .unwrap_err();
    assert!(matches!(
        revoked.downcast_ref::<LoopStoreError>(),
        Some(LoopStoreError::ScopeMismatch)
    ));
    let after_revoked = fixture
        .store
        .work_context(&reservation, original.next_cursor.as_deref(), 1, 103)
        .await
        .unwrap();
    assert_eq!(after_revoked.sources, first.sources);

    // A cursor remains usable if its source is revoked after the previous page was read.
    fixture
        .store
        .revoke_schedule("team", &firings[1].registration_id, 104)
        .await
        .unwrap();
    let last = fixture
        .store
        .work_context(&reservation, first.next_cursor.as_deref(), 1, 104)
        .await
        .unwrap();
    assert_eq!(last.sources.len(), 1);
    assert_eq!(last.sources[0].id, firings[3].receipt.trigger_id);
    assert!(!last.sources[0].revoked);
    assert!(last.next_cursor.is_none());
    let history = fixture
        .store
        .activation_source_history("team", "worker", activation_id, None, 5)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(history.sources.len(), 5);
    for (index, source) in history.sources.iter().enumerate() {
        assert_eq!(source.id, firings[index].receipt.trigger_id);
        assert_eq!(source.revoked, index != 3);
    }
    assert!(
        fixture
            .store
            .reservation("team", "worker")
            .await
            .unwrap()
            .is_some()
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_work_context_pages_canonical_sources_and_rejects_a_stale_executor() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let mut ids = Vec::new();
    let mut activation = String::new();
    for key in ["first", "second", "third"] {
        let receipt = fixture
            .store
            .accept_trigger(&trigger(key), 100)
            .await
            .unwrap();
        ids.push(receipt.trigger_id);
        activation = receipt.activation_id;
    }
    let LoopAdmission::Admitted(reservation) = fixture
        .store
        .admit("team", &activation, "daemon", 101)
        .await
        .unwrap()
    else {
        panic!("not admitted");
    };
    let first = fixture
        .store
        .work_context(&reservation, None, 2, 102)
        .await
        .unwrap();
    assert_eq!(first.activation.id, activation);
    assert_eq!(first.sources.len(), 2);
    let second = fixture
        .store
        .work_context(&reservation, first.next_cursor.as_deref(), 2, 102)
        .await
        .unwrap();
    assert_eq!(second.sources.len(), 1);
    assert!(second.next_cursor.is_none());
    let mut actual: Vec<_> = first
        .sources
        .into_iter()
        .chain(second.sources)
        .map(|source| source.id)
        .collect();
    actual.sort();
    ids.sort();
    assert_eq!(actual, ids);
    let mut wrong = reservation.clone();
    wrong.generation += 1;
    assert!(
        fixture
            .store
            .work_context(&wrong, None, 2, 102)
            .await
            .is_err()
    );
    wrong = reservation.clone();
    wrong.actor_id = "other".into();
    assert!(
        fixture
            .store
            .work_context(&wrong, None, 2, 102)
            .await
            .is_err()
    );
    assert!(
        fixture
            .store
            .work_context(&reservation, None, 2, 162)
            .await
            .is_err()
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_work_user_attribution_requires_an_existing_identity() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let mut input = trigger("operator");
    input.references.scheduling_user_id = Some("human".into());
    assert!(fixture.store.accept_trigger(&input, 100).await.is_err());
    sqlx::query("INSERT INTO users(id, username, display_name, role, created_at) VALUES ('human', 'human', 'Human', 'root', 1)").execute(&fixture.store.pool).await.unwrap();
    let accepted = fixture.store.accept_trigger(&input, 100).await.unwrap();
    let sources = fixture
        .store
        .triggers("team", &accepted.activation_id)
        .await
        .unwrap();
    assert_eq!(
        sources[0].input.references.scheduling_user_id.as_deref(),
        Some("human")
    );
    assert!(
        fixture
            .store
            .accept_trigger(&input, 101)
            .await
            .unwrap()
            .duplicate
    );
    fixture.close().await;
}
