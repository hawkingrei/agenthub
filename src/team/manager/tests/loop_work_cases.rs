use super::*;
use crate::team::{TeamTaskCreateInput, TeamTaskPriority};
use agenthub_agent_domain::loop_runtime::{
    LoopLimits, LoopPolicyState, LoopSessionPolicy, LoopTriggerRecord,
};
use agenthub_db::loop_runtime::{LoopPolicyUpdate, LoopStore, LoopStoreError};

async fn fixture() -> (TeamManager, crate::team::TeamDefinitionRecord) {
    fixture_with_db(setup_test_db().await).await
}

async fn fixture_with_db(db: SqlitePool) -> (TeamManager, crate::team::TeamDefinitionRecord) {
    for actor in ["planner", "reviewer", "observer"] {
        sqlx::query("INSERT INTO agents(id,name,workdir,command,args,worktree_mode,status,created_at,updated_at) VALUES (?, ?, '/tmp', 'fake', '[]', 'use_existing', 'idle', 1, 1)")
            .bind(actor).bind(actor).execute(&db).await.unwrap();
    }
    let manager = TeamManager::new(db);
    let team = manager.create_team(TeamDefinitionConfig {
        name: "loop-work".into(), description:None,
        spec:json!({"execution_mode":"loop","entrypoint":"planner","members":[
            {"member_id":"planner","role":"coordinator"},
            {"member_id":"reviewer","role":"worker"},
            {"member_id":"observer","role":"worker","loop_intake":{"engaged_thread_replies":false}}
        ]}),
    }).await.unwrap();
    for actor in ["planner", "reviewer", "observer"] {
        configure(
            &manager,
            &team.id,
            actor,
            LoopPolicyState::Enabled,
            &LoopLimits::default(),
        )
        .await;
    }
    (manager, team)
}

async fn configure(
    manager: &TeamManager,
    team: &str,
    actor: &str,
    state: LoopPolicyState,
    limits: &LoopLimits,
) {
    let store = LoopStore::new(manager.db.clone());
    let policy = store.policy(team, actor).await.unwrap().unwrap();
    store
        .configure(
            LoopPolicyUpdate {
                actor_id: actor,
                team_id: team,
                expected_revision: policy.revision,
                state,
                session_policy: LoopSessionPolicy::Fresh,
                limits,
            },
            Utc::now().timestamp(),
        )
        .await
        .unwrap();
}

async fn sources(manager: &TeamManager, actor: &str) -> Vec<LoopTriggerRecord> {
    let ids: Vec<(String, String)> =
        sqlx::query_as("SELECT team_id, id FROM loop_activations WHERE actor_id = ? ORDER BY id")
            .bind(actor)
            .fetch_all(&manager.db)
            .await
            .unwrap();
    let store = LoopStore::new(manager.db.clone());
    let mut sources = Vec::new();
    for (team, id) in ids {
        sources.extend(store.triggers(&team, &id).await.unwrap());
    }
    sources
}

async fn send(
    manager: &TeamManager,
    run: &str,
    to: &str,
    key: &str,
    payload: Value,
) -> anyhow::Result<crate::team::TeamActorMessageRecord> {
    manager
        .send_actor_message(SendActorMessageInput {
            run_id: run,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: to,
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload,
            idempotency_key: Some(key),
            message_kind: None,
        })
        .await
}

#[tokio::test]
async fn loop_work_direct_delivery_is_atomic_idempotent_and_suspension_retains_intake() {
    let (manager, team) = fixture().await;
    configure(
        &manager,
        &team.id,
        "reviewer",
        LoopPolicyState::Suspended,
        &LoopLimits::default(),
    )
    .await;
    let run = manager.create_run(&team.id, None, json!({})).await.unwrap();
    let first=send(&manager,&run.id,"reviewer","one",json!({"text":"review","scheduling_actor_id":"observer","scheduling_activation_id":"forged"})).await.unwrap();
    let again=send(&manager,&run.id,"reviewer","one",json!({"text":"review","scheduling_actor_id":"observer","scheduling_activation_id":"forged"})).await.unwrap();
    assert_eq!(first.message_id, again.message_id);
    let work = sources(&manager, "reviewer").await;
    assert_eq!(work.len(), 1);
    assert_eq!(
        work[0].input.references.mailbox_message_id,
        Some(first.message_id)
    );
    assert_eq!(
        work[0].input.references.scheduling_actor_id.as_deref(),
        Some("planner")
    );
    assert!(work[0].input.references.scheduling_activation_id.is_none());
    assert!(sources(&manager, "observer").await.is_empty());
    configure(
        &manager,
        &team.id,
        "observer",
        LoopPolicyState::Disabled,
        &LoopLimits::default(),
    )
    .await;
    send(
        &manager,
        &run.id,
        "observer",
        "disabled",
        json!({"text":"retained IM"}),
    )
    .await
    .unwrap();
    assert!(sources(&manager, "observer").await.is_empty());
    assert_eq!(
        manager
            .list_actor_inbox(&run.id, "observer", 10, None, false)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn loop_work_capacity_rolls_back_mailbox_and_assignment_writes() {
    let (manager, team) = fixture().await;
    let limits = LoopLimits {
        pending_per_actor: 1,
        sources_per_activation: 1,
        ..Default::default()
    };
    configure(
        &manager,
        &team.id,
        "reviewer",
        LoopPolicyState::Enabled,
        &limits,
    )
    .await;
    let run = manager.create_run(&team.id, None, json!({})).await.unwrap();
    send(
        &manager,
        &run.id,
        "reviewer",
        "one",
        json!({"text":"first"}),
    )
    .await
    .unwrap();
    let error = send(
        &manager,
        &run.id,
        "reviewer",
        "two",
        json!({"text":"overflow"}),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(
            error.downcast_ref::<LoopStoreError>(),
            Some(LoopStoreError::Capacity)
        ),
        "{error:#}"
    );
    assert_eq!(
        manager
            .list_actor_inbox(&run.id, "reviewer", 10, None, false)
            .await
            .unwrap()
            .len(),
        1
    );
    let error = manager
        .create_task_with_metadata(TeamTaskCreateInput {
            team_id: &team.id,
            title: "overflow task",
            created_by_actor_id: "planner",
            priority: TeamTaskPriority::Medium,
            assigned_member_id: Some("reviewer"),
            context: json!({}),
            conversation_mode: "group_chat",
            topic: None,
        })
        .await
        .unwrap_err();
    assert!(
        matches!(
            error.downcast_ref::<LoopStoreError>(),
            Some(LoopStoreError::Capacity)
        ),
        "{error:#}"
    );
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM team_tasks WHERE title = 'overflow task'")
            .fetch_one(&manager.db)
            .await
            .unwrap();
    assert_eq!(count, 0);
    assert_eq!(sources(&manager, "reviewer").await.len(), 1);
}

#[tokio::test]
async fn loop_work_assignment_and_mention_coalesce_without_replica_or_discussion_wakes() {
    let (manager, team) = fixture().await;
    let (task, conversation) = manager
        .create_task_with_metadata(TeamTaskCreateInput {
            team_id: &team.id,
            title: "review",
            created_by_actor_id: "planner",
            priority: TeamTaskPriority::Medium,
            assigned_member_id: Some("reviewer"),
            context: json!({}),
            conversation_mode: "group_chat",
            topic: None,
        })
        .await
        .unwrap();
    let message = manager
        .append_task_conversation_message(
            &task.id,
            "planner",
            None,
            "group_chat",
            json!({"text":"@reviewer please inspect"}),
        )
        .await
        .unwrap();
    manager
        .append_task_conversation_message(
            &task.id,
            "planner",
            None,
            "group_chat",
            json!({"text":"ordinary discussion"}),
        )
        .await
        .unwrap();
    let run = manager.create_run(&team.id, None, json!({})).await.unwrap();
    send(&manager,&run.id,"observer","replica",json!({"text":"replica","delivery_scope":"broadcast","task_message_id":message.message_id,"task_conversation_id":conversation.id})).await.unwrap();
    let work = sources(&manager, "reviewer").await;
    assert_eq!(work.len(), 2);
    assert_eq!(work[0].activation_id, work[1].activation_id);
    assert!(
        work.iter()
            .all(|s| s.input.references.task_id.as_deref() == Some(task.id.as_str()))
    );
    assert!(sources(&manager, "planner").await.is_empty());
    assert!(sources(&manager, "observer").await.is_empty());
    let error=send(&manager,&run.id,"observer","forged-replica",json!({"delivery_scope":"channel_broadcast","authority_message_id":message.message_id,"channel_conversation_id":"foreign"})).await.unwrap_err();
    assert!(
        matches!(
            error.downcast_ref::<LoopStoreError>(),
            Some(LoopStoreError::ScopeMismatch)
        ),
        "{error:#}"
    );
    manager
        .update_task(
            &task.id,
            None,
            TeamTaskAssignmentUpdate::Assigned("observer".into()),
        )
        .await
        .unwrap();
    assert_eq!(sources(&manager, "observer").await.len(), 1);
    manager
        .update_task_status(&task.id, TeamTaskStatus::Completed)
        .await
        .unwrap();
    manager
        .update_task_status(&task.id, TeamTaskStatus::Open)
        .await
        .unwrap();
    assert_eq!(sources(&manager, "observer").await.len(), 2);
}

#[tokio::test]
async fn loop_work_thread_routing_retains_prior_mentions_and_respects_opt_out() {
    let (manager, team) = fixture().await;
    let (task, _) = manager
        .create_task(&team.id, "thread", "user", json!({}), "group_chat", None)
        .await
        .unwrap();
    let root = manager
        .append_task_conversation_message(
            &task.id,
            "planner",
            None,
            "group_chat",
            json!({"text":"@reviewer @observer root"}),
        )
        .await
        .unwrap();
    let reply = manager
        .append_task_conversation_message(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"text":"follow-up","thread_root_message_id":root.message_id}),
        )
        .await
        .unwrap();
    assert_eq!(sources(&manager, "planner").await.len(), 1);
    let reviewer = sources(&manager, "reviewer").await;
    assert_eq!(reviewer.len(), 2);
    assert_eq!(
        reviewer[1].input.references.thread_id,
        Some(root.message_id)
    );
    assert_eq!(
        reviewer[1].input.references.conversation_message_id,
        Some(reply.message_id)
    );
    assert_eq!(sources(&manager, "observer").await.len(), 1);
    manager
        .append_task_conversation_message(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"text":"<at>observer</at> explicit","thread_root_message_id":root.message_id}),
        )
        .await
        .unwrap();
    assert_eq!(sources(&manager, "observer").await.len(), 2);
    let other = manager
        .append_task_conversation_message(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"text":"other root"}),
        )
        .await
        .unwrap();
    manager
        .append_task_conversation_message(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"text":"unrelated reply","thread_root_message_id":other.message_id}),
        )
        .await
        .unwrap();
    assert_eq!(sources(&manager, "reviewer").await.len(), 3);
    let error = manager
        .append_task_conversation_message(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"text":"bad root","thread_root_message_id":i64::MAX}),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            error.downcast_ref::<LoopStoreError>(),
            Some(LoopStoreError::ScopeMismatch)
        ),
        "{error:#}"
    );
}

#[tokio::test]
async fn loop_work_source_recovers_exact_committed_messages_before_replica_delivery() {
    use agenthub_agent_domain::loop_runtime::LoopAdmission;
    let (manager, team) = fixture().await;
    let (task, _) = manager
        .create_task(&team.id, "recovery", "user", json!({}), "group_chat", None)
        .await
        .unwrap();
    let message = manager
        .append_task_conversation_message(
            &task.id,
            "planner",
            Some("reviewer"),
            "group_chat",
            json!({"text":"durable source"}),
        )
        .await
        .unwrap();
    let source = sources(&manager, "reviewer").await.remove(0);
    for _ in 0..25 {
        manager
            .append_task_conversation_message(
                &task.id,
                "user",
                None,
                "group_chat",
                json!({"text":"later unrelated discussion"}),
            )
            .await
            .unwrap();
    }
    let store = LoopStore::new(manager.db.clone());
    let now = Utc::now().timestamp();
    let LoopAdmission::Admitted(reservation) = store
        .admit(&team.id, &source.activation_id, "daemon", now)
        .await
        .unwrap()
    else {
        panic!("not admitted")
    };
    let detail = manager
        .loop_work_source(&reservation, &source.id)
        .await
        .unwrap();
    assert_eq!(
        detail.conversation_message.unwrap().message_id,
        message.message_id
    );
    assert!(detail.mailbox_message.is_none());
    let copies: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM team_actor_messages")
        .fetch_one(&manager.db)
        .await
        .unwrap();
    assert_eq!(copies, 0);
    assert!(
        manager
            .loop_work_source(&reservation, "another-source")
            .await
            .is_err()
    );
    let mut stale = reservation.clone();
    stale.generation += 1;
    assert!(manager.loop_work_source(&stale, &source.id).await.is_err());
}

#[tokio::test]
async fn loop_work_reassignment_retires_stale_assignment_but_keeps_addressed_discussion() {
    use agenthub_agent_domain::loop_runtime::LoopAdmission;
    let (manager, team) = fixture().await;
    let (task, _) = manager
        .create_task_with_metadata(TeamTaskCreateInput {
            team_id: &team.id,
            title: "handoff",
            created_by_actor_id: "planner",
            priority: TeamTaskPriority::Medium,
            assigned_member_id: Some("reviewer"),
            context: json!({}),
            conversation_mode: "group_chat",
            topic: None,
        })
        .await
        .unwrap();
    manager
        .update_task_status(&task.id, TeamTaskStatus::InProgress)
        .await
        .unwrap();
    let old = sources(&manager, "reviewer").await.remove(0);
    manager
        .append_task_conversation_message(
            &task.id,
            "user",
            Some("reviewer"),
            "group_chat",
            json!({"text":"review the handoff decision"}),
        )
        .await
        .unwrap();
    manager
        .handoff_task_execution(&task.id, "observer", "user", "reassign")
        .await
        .unwrap();
    let store = LoopStore::new(manager.db.clone());
    assert!(matches!(
        store
            .admit(
                &team.id,
                &old.activation_id,
                "daemon",
                Utc::now().timestamp()
            )
            .await
            .unwrap(),
        LoopAdmission::Admitted(_)
    ));
    let sources = store.triggers(&team.id, &old.activation_id).await.unwrap();
    assert!(sources.iter().find(|s| s.id == old.id).unwrap().revoked);
    assert!(!sources.iter().find(|s| s.id != old.id).unwrap().revoked);
}

#[tokio::test]
async fn loop_work_file_reopen_keeps_committed_source_without_any_delivery_replica() {
    let (db, dir) = tests_support::setup_concurrent_mailbox_db().await;
    let (manager, team) = fixture_with_db(db.clone()).await;
    let (task, _) = manager
        .create_task(&team.id, "restart", "user", json!({}), "group_chat", None)
        .await
        .unwrap();
    let input = json!({"text":"@reviewer survives restart"});
    let (message, created) = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "planner",
            None,
            "group_chat",
            input.clone(),
            Some("restart:one"),
        )
        .await
        .unwrap();
    assert!(created);
    let before = sources(&manager, "reviewer").await;
    assert_eq!(before.len(), 1);
    drop(manager);
    db.close().await;
    let reopened = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(dir.join("race.db"))
                .foreign_keys(true),
        )
        .await
        .unwrap();
    let manager = TeamManager::new(reopened.clone());
    let after = sources(&manager, "reviewer").await;
    assert_eq!(after[0].id, before[0].id);
    assert_eq!(
        after[0].input.references.conversation_message_id,
        Some(message.message_id)
    );
    let (replay, replay_created) = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "planner",
            None,
            "group_chat",
            input,
            Some("restart:one"),
        )
        .await
        .unwrap();
    assert!(!replay_created);
    assert_eq!(replay.message_id, message.message_id);
    assert_eq!(sources(&manager, "reviewer").await.len(), 1);
    let copies: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM team_actor_messages")
        .fetch_one(&reopened)
        .await
        .unwrap();
    assert_eq!(copies, 0);
    drop(manager);
    reopened.close().await;
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn loop_work_explicit_request_retry_across_activations_keeps_original_provenance() {
    use crate::team::loop_context::{LoopSchedulingContext, with_scheduling_context};
    use agenthub_agent_domain::loop_runtime::{
        LoopSourceReferences, LoopTriggerInput, LoopTriggerKind,
    };
    let (manager, team) = fixture().await;
    let store = LoopStore::new(manager.db.clone());
    let now = Utc::now().timestamp();
    let input = LoopTriggerInput {
        actor_id: "reviewer".into(),
        team_id: team.id.clone(),
        kind: LoopTriggerKind::Operator,
        source_key: "first".into(),
        due_at: None,
        references: LoopSourceReferences::default(),
    };
    let first = store.accept_trigger(&input, now).await.unwrap();
    store
        .admit(&team.id, &first.activation_id, "daemon", now)
        .await
        .unwrap();
    let second = store
        .accept_trigger(
            &LoopTriggerInput {
                source_key: "second".into(),
                ..input
            },
            now,
        )
        .await
        .unwrap();
    assert_ne!(first.activation_id, second.activation_id);
    let mut receipts = Vec::new();
    for activation in [&first.activation_id, &second.activation_id] {
        receipts.push(
            with_scheduling_context(
                LoopSchedulingContext {
                    actor_id: Some("reviewer".into()),
                    activation_id: Some(activation.clone()),
                    user_id: None,
                },
                manager.request_loop_activation(&team.id, "planner", "business:one", None),
            )
            .await
            .unwrap(),
        );
    }
    assert_eq!(receipts[0].trigger_id, receipts[1].trigger_id);
    assert!(receipts[1].duplicate);
    let work = sources(&manager, "planner").await;
    assert_eq!(work.len(), 1);
    assert_eq!(
        work[0].input.references.scheduling_activation_id,
        Some(first.activation_id)
    );
}
