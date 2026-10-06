use super::*;

#[tokio::test]
async fn native_opening_recovery_uses_only_the_retired_actors_committed_receipt() {
    for accepted in [true, false] {
        let fixture = fixture("finish").await;
        let first = fixture.execute("receipt-before-binding").await;
        let store = LoopStore::new(fixture.state.db.clone());
        let (native, digest, local): (String, String, String) = sqlx::query_as(
            "SELECT native_session_id, configuration_digest, local_session_id FROM loop_native_sessions WHERE actor_id = 'worker'",
        ).fetch_one(&fixture.state.db).await.unwrap();
        let event_pool = fixture
            .state
            .agents
            .event_dbs
            .pool_for_agent("worker")
            .await
            .unwrap();
        let owner =
            agenthub_db::runtime_events::RuntimeEventStore::load(event_pool.clone(), &local)
                .await
                .unwrap()
                .unwrap();
        assert!(owner.history(100, None).await.unwrap().closed);
        // Reconstruct the crash window after ACK persistence but before binding.
        sqlx::query("UPDATE loop_native_sessions SET state = 'opening', native_session_id = NULL WHERE actor_id = 'worker'")
            .execute(&fixture.state.db).await.unwrap();
        if !accepted {
            sqlx::query("UPDATE runtime_control_receipts SET status = 'outcome_unknown', ack_json = NULL WHERE kind = 'create_session'")
                .execute(&event_pool).await.unwrap();
        }
        let now = Utc::now().timestamp();
        let next = fixture.admit("repair-opening").await;
        store
            .bind_mailbox(&next, first.mailbox_run_id.as_deref().unwrap(), now)
            .await
            .unwrap();
        let mut launch = first.launch.unwrap();
        launch.session_policy = LoopSessionPolicy::Resume;
        store.record_launch(&next, &launch, now).await.unwrap();
        store.authorize_guarded_spawn(&next, now).await.unwrap();
        let next_local = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES (?, 'worker', 'running', ?)")
            .bind(&next_local).bind(now).execute(&fixture.state.db).await.unwrap();
        let next = store.bind_session(&next, &next_local, now).await.unwrap();
        fixture
            .state
            .agents
            .loop_reservations
            .lock()
            .await
            .insert("worker".into(), next.clone());
        let before = std::fs::read(fixture.directory.join("native-requests.jsonl")).unwrap();
        fixture
            .state
            .agents
            .reconcile_retired_native_opening(&store, &next, &digest)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(fixture.directory.join("native-requests.jsonl")).unwrap(),
            before,
            "binding reconciliation must not contact the old process or replay input"
        );
        let resumed = store.begin_native_session(&next, &digest, now).await;
        if accepted {
            assert_eq!(resumed.unwrap().as_deref(), Some(native.as_str()));
        } else {
            assert!(matches!(
                resumed.unwrap_err().downcast_ref(),
                Some(agenthub_db::loop_runtime::LoopStoreError::NativeOpeningUncertain)
            ));
        }
        fixture
            .state
            .agents
            .fence_loop_reservation(&next)
            .await
            .unwrap();
        fixture.close().await;
    }
}
