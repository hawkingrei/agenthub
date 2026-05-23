use super::*;

#[tokio::test]
async fn task_status_updates_are_persisted() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-status-team".to_string(),
            description: Some("team for task status updates".to_string()),
            spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Ship kanban",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("status"),
        )
        .await
        .expect("create task");
    assert_eq!(task.status, TeamTaskStatus::Open);

    let updated = manager
        .update_task_status(&task.id, TeamTaskStatus::InProgress)
        .await
        .expect("update task status");
    assert_eq!(updated.id, task.id);
    assert_eq!(updated.status, TeamTaskStatus::InProgress);

    let reloaded = manager
        .get_task(&task.id)
        .await
        .expect("reload updated task");
    assert_eq!(reloaded.status, TeamTaskStatus::InProgress);
    assert_eq!(reloaded.assigned_member_id, None);
}

#[tokio::test]
async fn task_assignment_updates_are_persisted() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-assignment-team".to_string(),
            description: Some("team for task assignment updates".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Assign a worker",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("assignment"),
        )
        .await
        .expect("create task");
    assert_eq!(task.assigned_member_id, None);

    let assigned = manager
        .update_task(
            &task.id,
            None,
            TeamTaskAssignmentUpdate::Assigned("worker-1".to_string()),
        )
        .await
        .expect("assign task");
    assert_eq!(assigned.status, TeamTaskStatus::Open);
    assert_eq!(assigned.assigned_member_id.as_deref(), Some("worker-1"));

    let unassigned = manager
        .update_task(&task.id, None, TeamTaskAssignmentUpdate::Unassigned)
        .await
        .expect("unassign task");
    assert_eq!(unassigned.assigned_member_id, None);
}

#[tokio::test]
async fn task_partial_updates_preserve_unpatched_fields() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-partial-update-team".to_string(),
            description: Some("team for task patch semantics".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Keep unrelated task fields intact",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("patch"),
        )
        .await
        .expect("create task");

    let assigned = manager
        .update_task(
            &task.id,
            None,
            TeamTaskAssignmentUpdate::Assigned("worker-1".to_string()),
        )
        .await
        .expect("assign task");
    assert_eq!(assigned.assigned_member_id.as_deref(), Some("worker-1"));
    assert_eq!(assigned.status, TeamTaskStatus::Open);

    let status_updated = manager
        .update_task_status(&task.id, TeamTaskStatus::InProgress)
        .await
        .expect("update task status");
    assert_eq!(status_updated.status, TeamTaskStatus::InProgress);
    assert_eq!(
        status_updated.assigned_member_id.as_deref(),
        Some("worker-1")
    );

    let unassigned = manager
        .update_task(&task.id, None, TeamTaskAssignmentUpdate::Unassigned)
        .await
        .expect("unassign task");
    assert_eq!(unassigned.status, TeamTaskStatus::InProgress);
    assert_eq!(unassigned.assigned_member_id, None);
}

#[tokio::test]
async fn task_context_patches_support_merge_and_replace() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-context-patch-team".to_string(),
            description: Some("team for task context patching".to_string()),
            spec: json!({"entrypoint":"coordinator","members":[{"member_id":"coordinator","role":"coordinator"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Patch task context",
            "coordinator",
            json!({"repo":"agenthub","nested":{"issue":128}}),
            "group_chat",
            Some("patch"),
        )
        .await
        .expect("create task");

    let merged = manager
        .update_task_with_context(
            &task.id,
            None,
            TeamTaskAssignmentUpdate::Unchanged,
            Some(TeamTaskContextPatch::Merge(json!({
                "nested":{"pr":227},
                "result":"done"
            }))),
        )
        .await
        .expect("merge task context");
    assert_eq!(merged.context["repo"], json!("agenthub"));
    assert_eq!(merged.context["nested"]["issue"], json!(128));
    assert_eq!(merged.context["nested"]["pr"], json!(227));
    assert_eq!(merged.context["result"], json!("done"));

    let replaced = manager
        .update_task_with_context(
            &task.id,
            Some(TeamTaskStatus::InReview),
            TeamTaskAssignmentUpdate::Unchanged,
            Some(TeamTaskContextPatch::Replace(
                json!({"owner":"coordinator"}),
            )),
        )
        .await
        .expect("replace task context");
    assert_eq!(replaced.status, TeamTaskStatus::InReview);
    assert_eq!(replaced.context, json!({"owner":"coordinator"}));
}

#[tokio::test]
async fn create_task_rejects_invalid_reconcile_loop_execution_plan() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "invalid-execution-plan-team".to_string(),
            description: Some("team for invalid execution plan coverage".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let err = manager
        .create_task(
            &team.id,
            "Invalid execution plan",
            "coordinator",
            json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"worker-1",
                        "execution":{"mode":"reconcile_loop","max_rounds":0},
                        "acceptance":["tests pass"]
                    }]
                }
            }),
            "group_chat",
            Some("invalid"),
        )
        .await
        .expect_err("invalid reconcile loop plan should fail");
    assert!(
        err.to_string()
            .contains("reconcile_loop steps require a non-empty goal")
            || err
                .to_string()
                .contains("execution_plan.steps[].execution.max_rounds"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn update_task_context_rejects_execution_plan_with_unknown_member() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "unknown-member-execution-plan-team".to_string(),
            description: Some("team for execution plan member validation".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Execution plan patch",
            "coordinator",
            json!({"repo":"agenthub"}),
            "group_chat",
            Some("patch"),
        )
        .await
        .expect("create task");

    let err = manager
        .update_task_with_context(
            &task.id,
            None,
            TeamTaskAssignmentUpdate::Unchanged,
            Some(TeamTaskContextPatch::Merge(json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"missing-worker",
                        "execution":{"mode":"single_pass"}
                    }]
                }
            }))),
        )
        .await
        .expect_err("unknown member should fail validation");
    assert!(
        err.to_string()
            .contains("task context execution_plan.steps[].member_id must reference"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn list_tasks_with_query_filters_by_run_topic_and_owner() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-query-team".to_string(),
            description: Some("team for task query filtering".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
                    {"member_id":"worker-1","role":"worker"},
                    {"member_id":"worker-2","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (task_a, _) = manager
        .create_task(
            &team.id,
            "Coordinator task",
            "coordinator",
            json!({"source":"ui"}),
            "group_chat",
            Some("kanban"),
        )
        .await
        .expect("create first task");
    let (task_b, _) = manager
        .create_task(
            &team.id,
            "Worker task",
            "coordinator",
            json!({"source":"ui"}),
            "group_chat",
            Some("runtime"),
        )
        .await
        .expect("create second task");
    manager
        .update_task(
            &task_b.id,
            Some(TeamTaskStatus::InReview),
            TeamTaskAssignmentUpdate::Assigned("worker-2".to_string()),
        )
        .await
        .expect("assign task");
    let updated_task = manager
        .get_task(&task_b.id)
        .await
        .expect("reload updated task");
    assert_eq!(updated_task.status, TeamTaskStatus::InReview);
    assert_eq!(updated_task.assigned_member_id.as_deref(), Some("worker-2"));
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-task-query"),
            json!({"source":"query-scope"}),
        )
        .await
        .expect("create scope run");

    let scoped = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: None,
            run_id: Some(run.id.clone()),
            limit: 20,
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list run-scoped tasks");
    assert_eq!(scoped.len(), 2);

    let filtered_by_id = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: None,
            run_id: Some(run.id.clone()),
            limit: 20,
            task_id: Some(task_b.id.clone()),
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list task-id filtered tasks");
    assert_eq!(filtered_by_id.len(), 1);
    assert_eq!(filtered_by_id[0].id, task_b.id);

    let conversation = manager
        .get_task_conversation(&task_b.id)
        .await
        .expect("load task conversation");
    assert_eq!(conversation.topic.as_deref(), Some("runtime"));

    let filtered_by_status = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: None,
            run_id: Some(run.id.clone()),
            limit: 20,
            status: Some(TeamTaskStatus::InReview),
            task_id: Some(task_b.id.clone()),
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list status filtered tasks");
    assert_eq!(filtered_by_status.len(), 1);

    let filtered_by_owner = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: None,
            run_id: Some(run.id.clone()),
            limit: 20,
            task_id: Some(task_b.id.clone()),
            assigned_member_id: Some("worker-2".to_string()),
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list owner filtered tasks");
    assert_eq!(filtered_by_owner.len(), 1);

    let filtered_by_topic = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: None,
            run_id: Some(run.id.clone()),
            limit: 20,
            task_id: Some(task_b.id.clone()),
            topic: Some("runtime".to_string()),
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list topic filtered tasks");
    assert_eq!(filtered_by_topic.len(), 1);

    let listed = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: None,
            run_id: Some(run.id),
            limit: 20,
            status: Some(TeamTaskStatus::InReview),
            priority: None,
            task_id: Some(task_b.id.clone()),
            assigned_member_id: Some("worker-2".to_string()),
            topic: Some("runtime".to_string()),
            include_shared_thread: false,
        })
        .await
        .expect("list filtered tasks");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, task_b.id);
    assert_ne!(listed[0].id, task_a.id);
}

#[tokio::test]
async fn list_tasks_with_query_hides_shared_thread_bootstrap_kind_case_insensitively() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-shared-thread-casefold".to_string(),
            description: Some(
                "verify shared thread filtering remains case-insensitive".to_string(),
            ),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (visible_task, _) = manager
        .create_task(
            &team.id,
            "Normal task",
            "coordinator",
            json!({"topic":"visible"}),
            "group_chat",
            Some("visible"),
        )
        .await
        .expect("create visible task");
    manager
        .create_task(
            &team.id,
            "Shared thread",
            "coordinator",
            json!({"bootstrap_kind":"Shared_Thread"}),
            "group_chat",
            Some("shared"),
        )
        .await
        .expect("create shared thread task");

    let listed = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: Some(team.id.clone()),
            limit: 20,
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list visible tasks");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, visible_task.id);
}

#[tokio::test]
async fn list_tasks_with_query_keeps_tasks_without_conversation_rows() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-query-left-join".to_string(),
            description: Some("list tasks should not require conversation rows".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (orphan_task, _) = manager
        .create_task(
            &team.id,
            "Legacy orphan task",
            "coordinator",
            json!({"source":"legacy"}),
            "group_chat",
            Some("legacy"),
        )
        .await
        .expect("create orphan task");
    sqlx::query("DELETE FROM team_conversations WHERE task_id = ?1")
        .bind(&orphan_task.id)
        .execute(&db)
        .await
        .expect("delete orphan task conversation");

    let (topic_task, _) = manager
        .create_task(
            &team.id,
            "Topic task",
            "coordinator",
            json!({"source":"ui"}),
            "group_chat",
            Some("topic-a"),
        )
        .await
        .expect("create topic task");

    let listed = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: Some(team.id.clone()),
            limit: 20,
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list tasks with orphan row");
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().any(|task| task.id == orphan_task.id));
    assert!(listed.iter().any(|task| task.id == topic_task.id));

    let topic_filtered = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: Some(team.id),
            limit: 20,
            topic: Some("topic-a".to_string()),
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list topic filtered tasks");
    assert_eq!(topic_filtered.len(), 1);
    assert_eq!(topic_filtered[0].id, topic_task.id);
}
