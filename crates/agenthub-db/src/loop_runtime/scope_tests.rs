use super::*;

async fn suspend(fixture: &Fixture) {
    fixture
        .store
        .configure(
            LoopPolicyUpdate {
                actor_id: "worker",
                team_id: "team",
                expected_revision: 1,
                state: LoopPolicyState::Suspended,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits::default(),
            },
            101,
        )
        .await
        .unwrap();
}

async fn check(
    fixture: &Fixture,
) -> anyhow::Result<Option<agenthub_agent_domain::loop_runtime::LoopPolicy>> {
    let mut tx = fixture.store.pool.begin_with("BEGIN IMMEDIATE").await?;
    LoopStore::require_scope_quiescent_tx(&mut tx, "worker").await
}

#[tokio::test]
async fn loop_scope_mutation_requires_suspension_and_retains_pending_work() {
    let fixture = Fixture::new().await;
    assert!(check(&fixture).await.unwrap().is_none());
    fixture.enable("worker", &LoopLimits::default()).await;
    assert!(check(&fixture).await.is_err());
    suspend(&fixture).await;
    assert!(check(&fixture).await.unwrap().is_some());
    let accepted = fixture
        .store
        .accept_trigger(&trigger("pending"), 102)
        .await
        .unwrap();
    assert!(check(&fixture).await.is_err());
    fixture
        .store
        .cancel("team", &accepted.activation_id, 103)
        .await
        .unwrap();
    assert!(check(&fixture).await.is_ok());
    let mut tx = fixture
        .store
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap();
    assert!(
        LoopStore::require_no_retained_history_tx(&mut tx, "worker")
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn loop_scope_mutation_rejects_an_expired_reservation_until_verified_cleanup() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    suspend(&fixture).await;
    let reservation = fixture
        .store
        .reserve_manual("team", "worker", "daemon", 1)
        .await
        .unwrap();
    assert!(check(&fixture).await.is_err());
    fixture
        .store
        .cleanup_verified(
            &reservation,
            agenthub_agent_domain::loop_runtime::LoopCleanupDisposition::Exited,
            102,
        )
        .await
        .unwrap();
    assert!(check(&fixture).await.is_ok());
    fixture.close().await;
}

#[tokio::test]
async fn loop_scope_mutation_requires_explicit_release_of_expired_task_claims() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    suspend(&fixture).await;
    sqlx::query("INSERT INTO team_execution_claims(entity_kind, entity_id, team_id, owner_member_id, lease_generation, claimed_at, expires_at) VALUES ('task', 'task', 'team', 'worker', 1, 1, 2)")
        .execute(&fixture.store.pool).await.unwrap();
    assert!(check(&fixture).await.is_err());
    sqlx::query("UPDATE team_execution_claims SET released_at = 102 WHERE entity_id = 'task'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    assert!(check(&fixture).await.is_ok());
    fixture.close().await;
}

#[tokio::test]
async fn loop_scope_launch_policy_change_is_atomic_with_execution_reservation() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let reservation = fixture
        .store
        .reserve_manual("team", "worker", "daemon", 101)
        .await
        .unwrap();
    let limits = LoopLimits::default();
    let update = |revision, session_policy| LoopPolicyUpdate {
        actor_id: "worker",
        team_id: "team",
        expected_revision: revision,
        state: LoopPolicyState::Suspended,
        session_policy,
        limits: &limits,
    };
    assert!(
        fixture
            .store
            .configure(update(1, LoopSessionPolicy::Resume), 102)
            .await
            .is_err()
    );
    fixture
        .store
        .configure(update(1, LoopSessionPolicy::Fresh), 102)
        .await
        .unwrap();
    fixture
        .store
        .cleanup_verified(
            &reservation,
            agenthub_agent_domain::loop_runtime::LoopCleanupDisposition::Exited,
            103,
        )
        .await
        .unwrap();
    fixture
        .store
        .configure(update(2, LoopSessionPolicy::Resume), 104)
        .await
        .unwrap();
    fixture.close().await;
}
