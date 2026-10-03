use super::*;

#[tokio::test]
async fn native_transport_loss_releases_reservation_after_descendant_cleanup() {
    let fixture = fixture("transport-loss").await;
    let manager = &fixture.state.agents;
    let store = LoopStore::new(fixture.state.db.clone());
    let reservation = fixture.admit("transport-loss").await;
    let activation_id = reservation.activation_id.clone().unwrap();
    let mailbox = fixture
        .state
        .teams
        .ensure_loop_mailbox_partition(&fixture.team_id)
        .await
        .unwrap();
    store
        .bind_mailbox(&reservation, &mailbox.id, Utc::now().timestamp())
        .await
        .unwrap();
    let mut context = crate::team::build_team_member_actor_context_for_role(
        &fixture.team_id,
        Some(&mailbox.id),
        "worker",
        "worker",
    );
    context.contract_version = Some(LOOP_ACTIVATION_CONTRACT_VERSION.into());
    let session = manager
        .start_loop_agent(&reservation, context)
        .await
        .unwrap();
    let reservation = store
        .reservation(&fixture.team_id, "worker")
        .await
        .unwrap()
        .unwrap();
    store
        .mark_running(&reservation, Utc::now().timestamp())
        .await
        .unwrap();
    let runtime = {
        let handles = manager.inner.read().await;
        let AgentInput::Rara(runtime) = &handles["worker"].input else {
            panic!("native runtime");
        };
        runtime.clone()
    };
    let descendant = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(pid) = std::fs::read_to_string(fixture.directory.join("native-descendant")) {
                break std::path::PathBuf::from(format!(
                    "/proc/{}",
                    pid.trim().parse::<u32>().unwrap()
                ));
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(descendant.exists());
    // No activation monitor runs here: transport cleanup must retire its own reservation.
    runtime.abort();
    tokio::time::timeout(Duration::from_secs(10), async {
        while manager.inner.read().await.contains_key("worker") {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let remaining = store.reservation(&fixture.team_id, "worker").await.unwrap();
    let activation = store
        .activation(&fixture.team_id, &activation_id)
        .await
        .unwrap()
        .unwrap();
    let descendant_gone = !descendant.exists();
    let process_gone = !manager.process_supervisor.has_actor_process("worker").await;
    let memory_released = !manager.has_loop_activation("worker").await;
    let credentials_released = !manager.loop_credentials.lock().await.contains_key("worker");
    let ended: bool =
        sqlx::query_scalar("SELECT ended_at IS NOT NULL FROM agent_sessions WHERE id = ?")
            .bind(&session)
            .fetch_one(&fixture.state.db)
            .await
            .unwrap();
    // Retire the pre-fix leak too, so a regression never leaves the fixture's lease running.
    manager.fence_loop_reservation(&reservation).await.unwrap();
    let replacement = if remaining.is_none() {
        let next = fixture.admit("after-transport-loss").await;
        manager.fence_loop_reservation(&next).await.unwrap();
        Some(next.generation)
    } else {
        None
    };
    fixture.close().await;
    assert!(
        remaining.is_none(),
        "transport loss must retire the durable reservation"
    );
    assert_eq!(activation.state, LoopActivationState::Interrupted);
    assert!(activation.outcome.is_none());
    assert!(descendant_gone && process_gone && ended);
    assert!(memory_released && credentials_released);
    assert!(replacement.is_some_and(|generation| generation > reservation.generation));
}
