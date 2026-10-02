use super::*;

#[tokio::test]
async fn loop_recovery_replacement_manager_waits_for_guardian_cleanup() {
    use crate::executor_guardian::{CleanupWitness, prepare};
    use tokio::io::{AsyncBufReadExt, BufReader};

    let fixture = Fixture::new("outcome").await;
    let store = LoopStore::new(fixture.state.db.clone());
    let now = Utc::now().timestamp();
    // The replacement manager has no old process handle or in-memory reservation.
    let reservation = store
        .reserve_manual(&fixture.team_id, "worker", "previous-daemon", now)
        .await
        .unwrap();
    let base = fixture.state.agents.event_dbs.base_dir();
    let witness = Arc::new(CleanupWitness::prepare(base, &reservation).unwrap());
    store
        .authorize_guarded_spawn(&reservation, now)
        .await
        .unwrap();
    let (mut command, mut channel) = prepare(
        "/bin/sh",
        &[
            "-c".into(),
            "setsid /bin/sh -c 'echo $$; exec sleep 60' & wait".into(),
        ],
    )
    .unwrap();
    command
        .stdout(std::process::Stdio::piped())
        .stdin(std::process::Stdio::piped());
    channel.attach_witness(&mut command, witness);
    let mut child = channel.spawn(command).unwrap();
    let mut reader = BufReader::new(child.stdout().take().unwrap());
    let mut pid = String::new();
    tokio::time::timeout(Duration::from_secs(5), reader.read_line(&mut pid))
        .await
        .unwrap()
        .unwrap();
    let descendant =
        std::path::PathBuf::from(format!("/proc/{}", pid.trim().parse::<u32>().unwrap()));
    store.revoke_execution(&reservation, now).await.unwrap();
    assert_eq!(
        fixture
            .state
            .agents
            .recover_expired_loop_executors()
            .await
            .unwrap(),
        0
    );
    assert!(
        store
            .reserve_manual(&fixture.team_id, "worker", "replacement", now)
            .await
            .is_err()
    );
    let mut raw = child.into_inner();
    tokio::time::timeout(Duration::from_secs(5), raw.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(!descendant.exists());
    assert_eq!(
        fixture
            .state
            .agents
            .recover_expired_loop_executors()
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        fixture
            .state
            .agents
            .recover_expired_loop_executors()
            .await
            .unwrap(),
        0
    );
    let next = store
        .reserve_manual(
            &fixture.team_id,
            "worker",
            fixture.state.agents.loop_owner_id(),
            now,
        )
        .await
        .unwrap();
    assert!(next.generation > reservation.generation);
    assert!(
        store
            .authorize_guarded_spawn(&reservation, now)
            .await
            .is_err()
    );
    store
        .cleanup_verified(
            &next,
            agenthub_agent_domain::loop_runtime::LoopCleanupDisposition::Exited,
            now,
        )
        .await
        .unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn loop_recovery_replacement_manager_distinguishes_unstarted_and_unknown() {
    let fixture = Fixture::new("outcome").await;
    let store = LoopStore::new(fixture.state.db.clone());
    let now = Utc::now().timestamp();
    let unstarted = store
        .reserve_manual(&fixture.team_id, "worker", "old", now)
        .await
        .unwrap();
    store.revoke_execution(&unstarted, now).await.unwrap();
    assert_eq!(
        fixture
            .state
            .agents
            .recover_expired_loop_executors()
            .await
            .unwrap(),
        1
    );
    let missing = store
        .reserve_manual(&fixture.team_id, "worker", "old", now)
        .await
        .unwrap();
    store.authorize_guarded_spawn(&missing, now).await.unwrap();
    store.revoke_execution(&missing, now).await.unwrap();
    assert_eq!(
        fixture
            .state
            .agents
            .recover_expired_loop_executors()
            .await
            .unwrap(),
        0
    );
    assert!(
        store
            .reservation(&fixture.team_id, "worker")
            .await
            .unwrap()
            .is_some()
    );
    fixture.close().await;
}
