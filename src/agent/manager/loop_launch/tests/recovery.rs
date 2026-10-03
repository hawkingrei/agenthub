use super::*;

#[tokio::test]
async fn loop_recovery_replacement_manager_waits_for_guardian_cleanup() {
    use crate::executor_guardian::{CleanupWitness, prepare};
    use tokio::io::{AsyncBufReadExt, BufReader};

    let fixture = Fixture::new("outcome").await;
    let store = LoopStore::new(fixture.state.db.clone());
    let now = Utc::now().timestamp();
    // The replacement manager has no old process handle or in-memory reservation.
    let trigger = store
        .accept_trigger(
            &LoopTriggerInput {
                actor_id: "worker".into(),
                team_id: fixture.team_id.clone(),
                kind: LoopTriggerKind::Operator,
                source_key: "crashed-native-session".into(),
                due_at: None,
                references: LoopSourceReferences::default(),
            },
            now,
        )
        .await
        .unwrap();
    let LoopAdmission::Admitted(reservation) = store
        .admit(
            &fixture.team_id,
            &trigger.activation_id,
            "previous-daemon",
            now,
        )
        .await
        .unwrap()
    else {
        panic!("admit old activation");
    };
    sqlx::query("INSERT INTO agent_sessions (id, agent_id, status, started_at) VALUES ('old-native-local', 'worker', 'running', ?)")
        .bind(now).execute(&fixture.state.db).await.unwrap();
    let reservation = store
        .bind_session(&reservation, "old-native-local", now)
        .await
        .unwrap();
    store.mark_running(&reservation, now).await.unwrap();
    let event_pool = fixture
        .state
        .agents
        .event_dbs
        .pool_for_agent("worker")
        .await
        .unwrap();
    use agenthub_db::runtime_events::{
        RuntimeEventStore, RuntimeRequestIntent, RuntimeRequestKind, RuntimeRequestStatus,
    };
    let transport = RuntimeEventStore::bind(event_pool, "old-native-local", "old-runtime")
        .await
        .unwrap();
    transport.bind_stream("old-native").await.unwrap();
    for id in ["prepared", "sent"] {
        transport
            .prepare_request(
                RuntimeRequestIntent {
                    request_id: id,
                    kind: RuntimeRequestKind::Prompt,
                    target_session_id: Some("old-native"),
                    expected_turn_id: None,
                },
                now,
            )
            .await
            .unwrap();
        if id == "sent" {
            transport.mark_request_sent(id, now).await.unwrap();
        }
    }
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
    let mut daemon = crate::daemon_instance::DaemonInstanceGuard::acquire(
        &fixture.directory.join("replacement.db"),
        "main",
    )
    .unwrap();
    daemon.claim_generation(&fixture.state.db).await.unwrap();
    fixture.state.agents.mark_exited_on_startup().await.unwrap();
    let before = store
        .activation(&fixture.team_id, &trigger.activation_id)
        .await
        .unwrap();
    for _ in 0..2 {
        fixture
            .state
            .agents
            .recover_runtime_receipts_on_startup(&daemon)
            .await
            .unwrap();
    }
    let history = transport.history(100, None).await.unwrap();
    let session: (String, Option<i64>) =
        sqlx::query_as("SELECT status, ended_at FROM agent_sessions WHERE id = 'old-native-local'")
            .fetch_one(&fixture.state.db)
            .await
            .unwrap();
    let retained = store.reservation(&fixture.team_id, "worker").await.unwrap();
    let after = store
        .activation(&fixture.team_id, &trigger.activation_id)
        .await
        .unwrap();
    let replacement_blocked = store
        .reserve_manual(&fixture.team_id, "worker", "replacement", now)
        .await
        .is_err();
    let mut raw = child.into_inner();
    tokio::time::timeout(Duration::from_secs(5), raw.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(!descendant.exists());
    assert!(
        history.closed,
        "reserved sessions must retire their old transport"
    );
    assert_eq!(
        history.receipts[0].status,
        RuntimeRequestStatus::OutcomeUnknown
    );
    assert_eq!(history.receipts[1].status, RuntimeRequestStatus::NotSent);
    assert_eq!(session, ("running".into(), None));
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
    assert_eq!(retained.unwrap().generation, reservation.generation);
    assert!(replacement_blocked);
    drop(daemon);
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
