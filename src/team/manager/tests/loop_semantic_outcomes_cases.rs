use super::*;
use agenthub_agent_domain::loop_runtime::{
    LoopActivationState, LoopAdmission, LoopCleanupDisposition, LoopOutcome, LoopOutcomeKind,
    LoopReservation, LoopSourceReferences, LoopTriggerInput, LoopTriggerKind, LoopWaitReason,
};
use agenthub_db::loop_runtime::LoopStore;
use agenthub_rara::SemanticGuardDecision;

async fn running(manager: &TeamManager, team: &str, task: Option<&str>) -> LoopReservation {
    let store = LoopStore::new(manager.db.clone());
    let now = Utc::now().timestamp();
    let trigger = store
        .accept_trigger(
            &LoopTriggerInput {
                actor_id: "observer".into(),
                team_id: team.into(),
                kind: LoopTriggerKind::Operator,
                source_key: Uuid::new_v4().to_string(),
                due_at: None,
                references: LoopSourceReferences {
                    task_id: task.map(str::to_owned),
                    ..Default::default()
                },
            },
            now,
        )
        .await
        .unwrap();
    let LoopAdmission::Admitted(reservation) = store
        .admit(team, &trigger.activation_id, "daemon", now)
        .await
        .unwrap()
    else {
        panic!("not admitted")
    };
    store
        .authorize_guarded_spawn(&reservation, now)
        .await
        .unwrap();
    let session = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES (?, 'observer', 'running', ?)")
        .bind(&session).bind(now).execute(&manager.db).await.unwrap();
    let reservation = store
        .bind_session(&reservation, &session, now)
        .await
        .unwrap();
    store.mark_running(&reservation, now).await.unwrap();
    reservation
}

fn clarification() -> SemanticGuardDecision {
    SemanticGuardDecision::NeedsClarification {
        reason: "Missing target".into(),
        question: "Which target?".into(),
    }
}

async fn counts(manager: &TeamManager) -> (i64, i64, i64) {
    sqlx::query_as("SELECT (SELECT COUNT(*) FROM loop_finish_receipts), (SELECT COUNT(*) FROM team_conversation_messages), (SELECT COUNT(*) FROM loop_registrations)")
        .fetch_one(&manager.db).await.unwrap()
}

#[tokio::test]
async fn native_semantic_clarification_commits_once_and_exact_reply_wait_survives_reopen() {
    for bound_task in [false, true] {
        let (db, directory) = tests_support::setup_concurrent_mailbox_db().await;
        let (manager, team) = super::loop_work_cases::fixture_with_db(db.clone()).await;
        let task = if bound_task {
            Some(
                manager
                    .create_task(&team.id, "Target", "user", json!({}), "group_chat", None)
                    .await
                    .unwrap()
                    .0
                    .id,
            )
        } else {
            None
        };
        let reservation = running(&manager, &team.id, task.as_deref()).await;
        let before = counts(&manager).await;
        assert!(
            manager
                .finish_native_semantic_guard(&reservation, &clarification(), task.as_deref())
                .await
                .unwrap()
        );
        assert_eq!(
            counts(&manager).await,
            (before.0 + 1, before.1 + 1, before.2 + 1)
        );
        assert!(
            !manager
                .finish_native_semantic_guard(&reservation, &clarification(), task.as_deref())
                .await
                .unwrap()
        );
        assert_eq!(
            counts(&manager).await,
            (before.0 + 1, before.1 + 1, before.2 + 1)
        );
        let store = LoopStore::new(manager.db.clone());
        let activation = store
            .activation(&team.id, reservation.activation_id.as_ref().unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(activation.state, LoopActivationState::Finalizing);
        let outcome = activation.outcome.unwrap();
        assert_eq!(outcome.kind, LoopOutcomeKind::Waiting);
        assert_eq!(outcome.wait_reason, Some(LoopWaitReason::Input));
        let (message_id, actual_task, payload): (i64, String, String) = sqlx::query_as("SELECT id, task_id, payload_json FROM team_conversation_messages WHERE from_actor_id = 'observer'")
            .fetch_one(&manager.db).await.unwrap();
        if let Some(task) = task {
            assert_eq!(actual_task, task);
        }
        let payload: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(payload["text"], "Which target?");
        assert!(payload.get("thread_root_message_id").is_none());
        assert!(payload.get("reason").is_none());
        // Reopen SQLite before the reply; no live callback or pool owns the wait.
        drop(store);
        drop(manager);
        db.close().await;
        let db = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(directory.join("race.db"))
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        let reopened = TeamManager::new(db.clone());
        let store = LoopStore::new(db.clone());
        reopened
            .append_task_conversation_message(
                &actual_task,
                "user",
                None,
                "group_chat",
                json!({"text":"unrelated root"}),
            )
            .await
            .unwrap();
        assert!(
            store
                .reconcile_schedules(Utc::now().timestamp())
                .await
                .unwrap()
                .is_empty()
        );
        let reply = reopened
            .append_task_conversation_message(
                &actual_task,
                "user",
                None,
                "group_chat",
                json!({"text":"the database","thread_root_message_id":message_id}),
            )
            .await
            .unwrap();
        let firings = store
            .reconcile_schedules(Utc::now().timestamp())
            .await
            .unwrap();
        assert_eq!(firings.len(), 1);
        assert_eq!(firings[0].first_cursor, reply.message_id);
        assert!(
            store
                .reconcile_schedules(Utc::now().timestamp())
                .await
                .unwrap()
                .is_empty()
        );
        let next: String = sqlx::query_scalar(
            "SELECT id FROM loop_activations WHERE actor_id = 'observer' AND state = 'pending'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert!(matches!(
            store
                .admit(&team.id, &next, "daemon", Utc::now().timestamp())
                .await
                .unwrap(),
            LoopAdmission::Deferred(_)
        ));
        store
            .cleanup_verified(
                &reservation,
                LoopCleanupDisposition::Exited,
                Utc::now().timestamp(),
            )
            .await
            .unwrap();
        assert!(matches!(
            store
                .admit(&team.id, &next, "replacement", Utc::now().timestamp() + 61)
                .await
                .unwrap(),
            LoopAdmission::Admitted(_)
        ));
        drop(store);
        drop(reopened);
        db.close().await;
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[tokio::test]
async fn native_semantic_finish_rolls_back_question_and_outcome_when_wait_fails() {
    let (manager, team) = super::loop_work_cases::fixture().await;
    let reservation = running(&manager, &team.id, None).await;
    let before = counts(&manager).await;
    sqlx::raw_sql("CREATE TRIGGER fail_semantic_wait BEFORE INSERT ON loop_registrations BEGIN SELECT RAISE(ABORT, 'fixture schedule failure'); END;")
        .execute(&manager.db).await.unwrap();
    assert!(
        manager
            .finish_native_semantic_guard(&reservation, &clarification(), None)
            .await
            .is_err()
    );
    assert_eq!(counts(&manager).await, before);
    let activation = LoopStore::new(manager.db.clone())
        .activation(&team.id, reservation.activation_id.as_ref().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(activation.state, LoopActivationState::Running);
    assert!(activation.outcome.is_none());
    sqlx::query("DROP TRIGGER fail_semantic_wait")
        .execute(&manager.db)
        .await
        .unwrap();
    assert!(
        manager
            .finish_native_semantic_guard(&reservation, &clarification(), None)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn native_semantic_finish_preserves_actor_authority_and_rejects_stale_or_foreign_context() {
    let (manager, team) = super::loop_work_cases::fixture().await;
    let reservation = running(&manager, &team.id, None).await;
    let before = counts(&manager).await;
    for field in ["generation", "owner", "session", "actor", "team"] {
        let mut stale = reservation.clone();
        match field {
            "generation" => stale.generation += 1,
            "owner" => stale.owner_id = "another".into(),
            "session" => stale.session_id = Some("another".into()),
            "actor" => stale.actor_id = "planner".into(),
            _ => stale.team_id = "another".into(),
        }
        assert!(
            manager
                .finish_native_semantic_guard(&stale, &clarification(), None)
                .await
                .is_err()
        );
        assert_eq!(counts(&manager).await, before);
    }
    let (unrelated, _) = manager
        .create_task(
            &team.id,
            "Unaddressed",
            "user",
            json!({}),
            "group_chat",
            None,
        )
        .await
        .unwrap();
    assert!(
        manager
            .finish_native_semantic_guard(&reservation, &clarification(), Some(&unrelated.id))
            .await
            .is_err()
    );
    assert_eq!(counts(&manager).await, before);
    let authoritative = LoopOutcome {
        kind: LoopOutcomeKind::Waiting,
        wait_reason: Some(LoopWaitReason::Input),
        task_note_id: None,
        continuation: None,
    };
    LoopStore::new(manager.db.clone())
        .finish(&reservation, &authoritative, Utc::now().timestamp())
        .await
        .unwrap();
    assert!(
        !manager
            .finish_native_semantic_guard(&reservation, &clarification(), None)
            .await
            .unwrap()
    );
    assert_eq!(counts(&manager).await, (before.0 + 1, before.1, before.2));
    let mut wrong_owner = reservation.clone();
    wrong_owner.owner_id = "another".into();
    assert!(
        manager
            .finish_native_semantic_guard(&wrong_owner, &clarification(), None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn native_semantic_compatible_is_noop_and_mismatch_records_no_action_without_a_wait() {
    let (manager, team) = super::loop_work_cases::fixture().await;
    let reservation = running(&manager, &team.id, None).await;
    let before = counts(&manager).await;
    assert!(
        !manager
            .finish_native_semantic_guard(&reservation, &SemanticGuardDecision::Compatible {}, None)
            .await
            .unwrap()
    );
    assert_eq!(counts(&manager).await, before);
    assert!(
        manager
            .finish_native_semantic_guard(
                &reservation,
                &SemanticGuardDecision::Mismatch {
                    reason: "Outside role".into()
                },
                None
            )
            .await
            .unwrap()
    );
    assert_eq!(counts(&manager).await, (before.0 + 1, before.1, before.2));
}
