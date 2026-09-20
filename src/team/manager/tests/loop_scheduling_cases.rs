use super::*;
use agenthub_agent_domain::loop_runtime::LoopSourceReferences;
use agenthub_agent_domain::loop_scheduling::{LoopRegistrationInput, LoopSchedule};
use agenthub_db::loop_runtime::LoopStore;

#[tokio::test]
async fn loop_schedule_canonical_task_run_and_thread_writers_preserve_transient_observations() {
    let (manager, team) = super::loop_work_cases::fixture().await;
    let (task, _) = manager
        .create_task(
            &team.id,
            "dependency",
            "planner",
            json!({}),
            "group_chat",
            None,
        )
        .await
        .unwrap();
    let store = LoopStore::new(manager.db.clone());
    let now = Utc::now().timestamp();
    let registration = store
        .register_schedule(
            &LoopRegistrationInput {
                actor_id: "observer".into(),
                team_id: team.id.clone(),
                source_key: "canonical-task".into(),
                schedule: LoopSchedule::TaskStatus {
                    task_id: task.id.clone(),
                    statuses: vec![TeamTaskStatus::Completed],
                    repeat: true,
                },
                work_task_id: None,
                references: LoopSourceReferences::default(),
            },
            now,
        )
        .await
        .unwrap();
    manager
        .update_task_status(&task.id, TeamTaskStatus::Completed)
        .await
        .unwrap();
    manager
        .update_task_status(&task.id, TeamTaskStatus::Open)
        .await
        .unwrap();
    assert_eq!(
        store
            .reconcile_schedules(Utc::now().timestamp())
            .await
            .unwrap()
            .len(),
        1
    );
    let mut tx = manager.db.begin_with("BEGIN IMMEDIATE").await.unwrap();
    super::super::run_task_status_sync::sync_linked_task_status_tx(
        &mut tx,
        &team.id,
        &json!({"task_id":task.id}),
        TeamTaskStatus::Completed,
        now,
        false,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    manager
        .update_task_status(&task.id, TeamTaskStatus::Open)
        .await
        .unwrap();
    assert_eq!(
        store
            .reconcile_schedules(Utc::now().timestamp())
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        store
            .registration_firings(&team.id, &registration.registration.id, None, 10)
            .await
            .unwrap()
            .len(),
        2
    );
    let root = manager
        .append_task_conversation_message(
            &task.id,
            "planner",
            None,
            "group_chat",
            json!({"text":"thread root"}),
        )
        .await
        .unwrap();
    let registration = store
        .register_schedule(
            &LoopRegistrationInput {
                actor_id: "observer".into(),
                team_id: team.id.clone(),
                source_key: "canonical-thread".into(),
                schedule: LoopSchedule::ThreadReply {
                    root_message_id: root.message_id,
                    after_message_id: root.message_id,
                    repeat: false,
                },
                work_task_id: None,
                references: LoopSourceReferences::default(),
            },
            now,
        )
        .await
        .unwrap();
    let reply = manager
        .append_task_conversation_message(
            &task.id,
            "reviewer",
            None,
            "group_chat",
            json!({"text":"reply", "thread_root_message_id":root.message_id}),
        )
        .await
        .unwrap();
    let firings = store
        .reconcile_schedules(Utc::now().timestamp())
        .await
        .unwrap();
    assert_eq!(firings.len(), 1);
    assert_eq!(firings[0].registration_id, registration.registration.id);
    assert_eq!(firings[0].first_cursor, reply.message_id);
}
