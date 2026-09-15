use agenthub_agent_domain::loop_runtime::LoopAdmission;

use super::*;

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
