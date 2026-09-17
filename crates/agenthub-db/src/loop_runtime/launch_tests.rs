use agenthub_agent_domain::loop_runtime::{LoopAdmission, LoopLaunchSnapshot, LoopReservation};

use super::*;

async fn starting(fixture: &Fixture) -> LoopReservation {
    fixture.enable("worker", &LoopLimits::default()).await;
    let receipt = fixture
        .store
        .accept_trigger(&trigger("launch"), 100)
        .await
        .unwrap();
    let LoopAdmission::Admitted(reservation) = fixture
        .store
        .admit("team", &receipt.activation_id, "daemon", 100)
        .await
        .unwrap()
    else {
        panic!("not admitted");
    };
    reservation
}

#[tokio::test]
async fn loop_mem_context_evidence_is_fenced_immutable_and_survives_reopen() {
    use agenthub_agent_domain::loop_runtime::LoopEventKind;
    let mut fixture = Fixture::new().await;
    let reservation = starting(&fixture).await;
    partition(&fixture, "mailbox", "team").await;
    fixture
        .store
        .bind_mailbox(&reservation, "mailbox", 101)
        .await
        .unwrap();
    fixture
        .store
        .record_launch(&reservation, &snapshot(), 101)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES ('local-session', 'worker', 'running', 101)")
        .execute(&fixture.store.pool).await.unwrap();
    let reservation = fixture
        .store
        .bind_session(&reservation, "local-session", 101)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .record_mem_context(&reservation, LoopEventKind::MemContextReady, 101)
            .await
            .is_err()
    );
    fixture.store.mark_running(&reservation, 102).await.unwrap();
    for mutate in [0, 1, 2] {
        let mut invalid = reservation.clone();
        match mutate {
            0 => invalid.generation += 1,
            1 => invalid.owner_id = "foreign".into(),
            _ => invalid.team_id = "other".into(),
        }
        assert!(
            fixture
                .store
                .record_mem_context(&invalid, LoopEventKind::MemContextReady, 102)
                .await
                .is_err()
        );
    }
    fixture
        .store
        .record_mem_context(&reservation, LoopEventKind::MemContextUnavailable, 102)
        .await
        .unwrap();
    fixture
        .store
        .record_mem_context(&reservation, LoopEventKind::MemContextUnavailable, 103)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .record_mem_context(&reservation, LoopEventKind::MemContextReady, 103)
            .await
            .is_err()
    );
    assert!(
        fixture
            .store
            .record_mem_context(&reservation, LoopEventKind::Running, 103)
            .await
            .is_err()
    );
    fixture.store.pool.close().await;
    fixture.store = LoopStore::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    let events = fixture
        .store
        .events(
            "team",
            reservation.activation_id.as_deref().unwrap(),
            0,
            100,
        )
        .await
        .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == LoopEventKind::MemContextUnavailable)
            .count(),
        1
    );
    assert!(
        events
            .iter()
            .all(|event| event.kind != LoopEventKind::MemContextReady)
    );
    assert!(
        fixture
            .store
            .record_mem_context(&reservation, LoopEventKind::MemContextUnavailable, 161)
            .await
            .is_err()
    );
    fixture.close().await;
}

async fn partition(fixture: &Fixture, run: &str, team: &str) {
    sqlx::query("INSERT INTO team_runs(id, team_id, context_id, status, input_json, created_at) VALUES (?, ?, 'loop', 'submitted', '{}', 100)")
        .bind(run).bind(team).execute(&fixture.store.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO loop_mailbox_partitions(run_id, team_id, created_at) VALUES (?, ?, 100)",
    )
    .bind(run)
    .bind(team)
    .execute(&fixture.store.pool)
    .await
    .unwrap();
}

fn snapshot() -> LoopLaunchSnapshot {
    LoopLaunchSnapshot {
        version: 1,
        provider_id: "fake-acp".into(),
        configuration_digest: "a".repeat(64),
        entry_prompt_version: "loop-v1".into(),
        session_policy: LoopSessionPolicy::Fresh,
        workspace: "/workspace".into(),
        model: Some("fixture".into()),
        thinking_level: None,
    }
}

#[tokio::test]
async fn loop_launch_is_immutable_and_survives_reopen() {
    let mut fixture = Fixture::new().await;
    let reservation = starting(&fixture).await;
    assert!(
        fixture
            .store
            .record_launch(&reservation, &snapshot(), 101)
            .await
            .is_err()
    );
    partition(&fixture, "mailbox", "team").await;
    fixture
        .store
        .bind_mailbox(&reservation, "mailbox", 101)
        .await
        .unwrap();
    fixture
        .store
        .bind_mailbox(&reservation, "mailbox", 101)
        .await
        .unwrap();
    fixture
        .store
        .record_launch(&reservation, &snapshot(), 101)
        .await
        .unwrap();
    fixture
        .store
        .record_launch(&reservation, &snapshot(), 102)
        .await
        .unwrap();
    let mut changed = snapshot();
    changed.model = Some("changed".into());
    assert!(
        fixture
            .store
            .record_launch(&reservation, &changed, 102)
            .await
            .is_err()
    );
    fixture.store.pool.close().await;
    fixture.store = LoopStore::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    let activation = fixture
        .store
        .activation("team", reservation.activation_id.as_deref().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(activation.launch, Some(snapshot()));
    assert_eq!(activation.mailbox_run_id.as_deref(), Some("mailbox"));
    let events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM loop_activation_events WHERE kind = 'launch_resolved'",
    )
    .fetch_one(&fixture.store.pool)
    .await
    .unwrap();
    assert_eq!(events, 1);
    fixture.close().await;
}

#[tokio::test]
async fn loop_launch_rejects_wrong_mailbox_and_stale_generation() {
    let fixture = Fixture::new().await;
    let reservation = starting(&fixture).await;
    partition(&fixture, "foreign", "elsewhere").await;
    assert!(
        fixture
            .store
            .bind_mailbox(&reservation, "foreign", 101)
            .await
            .is_err()
    );
    partition(&fixture, "mailbox", "team").await;
    let mut stale = reservation.clone();
    stale.generation += 1;
    assert!(
        fixture
            .store
            .bind_mailbox(&stale, "mailbox", 101)
            .await
            .is_err()
    );
    fixture
        .store
        .bind_mailbox(&reservation, "mailbox", 101)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .record_launch(&stale, &snapshot(), 101)
            .await
            .is_err()
    );
    assert!(
        fixture
            .store
            .record_launch(&reservation, &snapshot(), 161)
            .await
            .is_err()
    );
    fixture
        .store
        .record_launch(&reservation, &snapshot(), 101)
        .await
        .unwrap();
    sqlx::query("UPDATE loop_mailbox_partitions SET active = 0 WHERE run_id = 'mailbox'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .bind_mailbox(&reservation, "mailbox", 102)
            .await
            .is_err()
    );
    fixture.close().await;
}

#[test]
fn loop_launch_snapshot_is_bounded_and_excludes_credentials() {
    let value = serde_json::to_value(snapshot()).unwrap();
    let mut injected = value.clone();
    injected["access_token"] = "secret".into();
    assert!(serde_json::from_value::<LoopLaunchSnapshot>(injected).is_err());
    let mut invalid = snapshot();
    invalid.configuration_digest = "secret".into();
    assert!(invalid.validate().is_err());
    invalid = snapshot();
    invalid.workspace = "\n/workspace".into();
    assert!(invalid.validate().is_err());
    snapshot().validate().unwrap();
}

#[tokio::test]
async fn loop_bootstrap_requires_bound_launch_and_session_without_granting_execution() {
    let fixture = Fixture::new().await;
    let reservation = starting(&fixture).await;
    partition(&fixture, "mailbox", "team").await;
    fixture
        .store
        .bind_mailbox(&reservation, "mailbox", 101)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .verify_executor_bootstrap_live(&reservation, 101)
            .await
            .is_err()
    );
    fixture
        .store
        .record_launch(&reservation, &snapshot(), 101)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .verify_executor_bootstrap_live(&reservation, 101)
            .await
            .is_err()
    );
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES ('local-session', 'worker', 'running', 101)")
        .execute(&fixture.store.pool).await.unwrap();
    let reservation = fixture
        .store
        .bind_session(&reservation, "local-session", 101)
        .await
        .unwrap();
    fixture
        .store
        .verify_executor_bootstrap_live(&reservation, 101)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .verify_executor_live(&reservation, 101)
            .await
            .is_err()
    );
    for change in ["generation", "owner", "team", "actor"] {
        let mut invalid = reservation.clone();
        match change {
            "generation" => invalid.generation += 1,
            "owner" => invalid.owner_id = "foreign".into(),
            "team" => invalid.team_id = "elsewhere".into(),
            "actor" => invalid.actor_id = "other".into(),
            _ => unreachable!(),
        }
        assert!(
            fixture
                .store
                .verify_executor_bootstrap_live(&invalid, 101)
                .await
                .is_err(),
            "{change}"
        );
    }
    assert!(
        fixture
            .store
            .verify_executor_bootstrap_live(&reservation, 161)
            .await
            .is_err()
    );
    fixture.store.mark_running(&reservation, 102).await.unwrap();
    fixture
        .store
        .verify_executor_live(&reservation, 102)
        .await
        .unwrap();
    fixture
        .store
        .verify_executor_bootstrap_live(&reservation, 102)
        .await
        .unwrap();
    sqlx::query("UPDATE loop_mailbox_partitions SET active = 0 WHERE run_id = 'mailbox'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .verify_executor_bootstrap_live(&reservation, 102)
            .await
            .is_err()
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_bootstrap_is_fenced_by_member_removal_and_executor_revocation() {
    let fixture = Fixture::new().await;
    let reservation = starting(&fixture).await;
    partition(&fixture, "mailbox", "team").await;
    fixture
        .store
        .bind_mailbox(&reservation, "mailbox", 101)
        .await
        .unwrap();
    fixture
        .store
        .record_launch(&reservation, &snapshot(), 101)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES ('local-session', 'worker', 'running', 101)")
        .execute(&fixture.store.pool).await.unwrap();
    let reservation = fixture
        .store
        .bind_session(&reservation, "local-session", 101)
        .await
        .unwrap();
    fixture
        .store
        .verify_executor_bootstrap_live(&reservation, 101)
        .await
        .unwrap();
    let spec: String =
        sqlx::query_scalar("SELECT spec_json FROM team_definitions WHERE id = 'team'")
            .fetch_one(&fixture.store.pool)
            .await
            .unwrap();
    sqlx::query("UPDATE team_definitions SET spec_json = '{\"members\":[]}' WHERE id = 'team'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .verify_executor_bootstrap_live(&reservation, 101)
            .await
            .is_err()
    );
    sqlx::query("UPDATE team_definitions SET spec_json = ? WHERE id = 'team'")
        .bind(spec)
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    fixture
        .store
        .verify_executor_bootstrap_live(&reservation, 101)
        .await
        .unwrap();
    fixture
        .store
        .revoke_execution(&reservation, 101)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .verify_executor_bootstrap_live(&reservation, 101)
            .await
            .is_err()
    );
    fixture.close().await;
}
