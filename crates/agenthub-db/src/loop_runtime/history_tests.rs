use agenthub_agent_domain::loop_runtime::{
    LoopCleanupDisposition, LoopEventKind, LoopOutcome, LoopOutcomeKind,
};

use super::*;

#[tokio::test]
async fn loop_history_keyset_is_scoped_and_does_not_repeat_when_new_activations_arrive() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    fixture.enable("other", &LoopLimits::default()).await;
    let mut expected = Vec::new();
    for (index, now) in [100, 102, 102].into_iter().enumerate() {
        let mut input = trigger(&format!("history:{index}"));
        input.due_at = Some(200 + index as i64);
        expected.push(
            fixture
                .store
                .accept_trigger(&input, now)
                .await
                .unwrap()
                .activation_id,
        );
    }
    let first = fixture
        .store
        .activation_history("team", "worker", None, 1)
        .await
        .unwrap();
    assert_eq!(first.activations.len(), 1);
    assert_eq!(first.activations[0].created_at, 102);
    let mut actual = vec![first.activations[0].id.clone()];
    let mut cursor = first.next_cursor;
    let newer = fixture
        .store
        .accept_trigger(&trigger("newer"), 300)
        .await
        .unwrap();
    while cursor.is_some() {
        let page = fixture
            .store
            .activation_history("team", "worker", cursor.as_deref(), 1)
            .await
            .unwrap();
        actual.extend(page.activations.into_iter().map(|activation| activation.id));
        cursor = page.next_cursor;
    }
    assert!(!actual.contains(&newer.activation_id));
    expected.sort();
    actual.sort();
    assert_eq!(actual, expected);
    let mut foreign = trigger("other-actor");
    foreign.actor_id = "other".into();
    let foreign = fixture.store.accept_trigger(&foreign, 300).await.unwrap();
    assert!(
        fixture
            .store
            .activation_history("team", "worker", Some(&foreign.activation_id), 1)
            .await
            .is_err()
    );
    assert!(
        fixture
            .store
            .activation_history("elsewhere", "worker", None, 1)
            .await
            .unwrap()
            .activations
            .is_empty()
    );
    for limit in [0, 101] {
        assert!(
            fixture
                .store
                .activation_history("team", "worker", None, limit)
                .await
                .is_err()
        );
    }
    fixture.close().await;
}

#[tokio::test]
async fn loop_history_recovers_finished_trace_after_reopen_without_source_payloads() {
    let mut fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    for key in ["private-source-key:alpha", "private-source-key:beta"] {
        fixture
            .store
            .accept_trigger(&trigger(key), 100)
            .await
            .unwrap();
    }
    let current = lifecycle_tests::running(&fixture, "private-source-key:gamma", 100).await;
    let id = current.activation_id.as_deref().unwrap();
    fixture
        .store
        .finish(
            &current,
            &LoopOutcome {
                kind: LoopOutcomeKind::NoActionableWork,
                wait_reason: None,
                task_note_id: None,
                continuation: None,
            },
            101,
        )
        .await
        .unwrap();
    fixture
        .store
        .cleanup_verified(&current, LoopCleanupDisposition::Exited, 102)
        .await
        .unwrap();
    fixture.store.pool.close().await;
    fixture.store = LoopStore::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    let history = fixture
        .store
        .activation_history("team", "worker", None, 1)
        .await
        .unwrap();
    assert_eq!(history.activations[0].id, id);
    assert_eq!(history.activations[0].state, LoopActivationState::Finished);
    assert_eq!(
        history.activations[0].outcome.as_ref().unwrap().kind,
        LoopOutcomeKind::NoActionableWork
    );
    let mut sources = Vec::new();
    let mut cursor = None;
    loop {
        let page = fixture
            .store
            .activation_source_history("team", "worker", id, cursor.as_deref(), 1)
            .await
            .unwrap()
            .unwrap();
        sources.extend(page.sources);
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(sources.len(), 3);
    let serialized = serde_json::to_string(&sources).unwrap();
    assert!(!serialized.contains("source_key"));
    assert!(!serialized.contains("private-source-key"));
    let mut events = Vec::new();
    let mut cursor = None;
    loop {
        let page = fixture
            .store
            .activation_event_history("team", "worker", id, cursor, 2)
            .await
            .unwrap()
            .unwrap();
        events.extend(page.events);
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert!(events.windows(2).all(|pair| pair[0].id < pair[1].id));
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == LoopEventKind::TriggerAccepted)
            .count(),
        3
    );
    assert!(
        events
            .iter()
            .any(|event| event.kind == LoopEventKind::OutcomeRecorded)
    );
    assert_eq!(events.last().unwrap().kind, LoopEventKind::CleanupVerified);
    for (team, actor) in [("elsewhere", "worker"), ("team", "other")] {
        assert!(
            fixture
                .store
                .activation_source_history(team, actor, id, None, 1)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            fixture
                .store
                .activation_event_history(team, actor, id, None, 1)
                .await
                .unwrap()
                .is_none()
        );
    }
    assert!(
        fixture
            .store
            .activation_source_history("team", "worker", id, Some("unknown"), 1)
            .await
            .is_err()
    );
    assert!(
        fixture
            .store
            .activation_event_history("team", "worker", id, Some(i64::MAX), 1)
            .await
            .is_err()
    );
    fixture.close().await;
}
