use super::*;
use crate::team::{TeamTaskCreateInput, TeamTaskPriority};

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
async fn teamspace_invite_is_single_use_and_task_claim_is_single_owner() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);
    let team = manager
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "teamspace-control".to_string(),
                description: None,
                spec: json!({"members":[
                    {"member_id":"worker-1","role":"worker"},
                    {"member_id":"worker-2","role":"worker"}
                ]}),
            },
            Some("owner-user"),
        )
        .await
        .expect("create Teamspace");
    let (invite, token) = manager
        .create_teamspace_invite(
            &team.id,
            "contributor",
            "owner-user",
            chrono::Utc::now().timestamp() + 60,
        )
        .await
        .expect("create invite");
    assert_eq!(invite.role, "contributor");

    let member = manager
        .accept_teamspace_invite(&token, "invited-user")
        .await
        .expect("accept invite");
    assert_eq!(member.user_id, "invited-user");
    assert!(
        manager
            .is_teamspace_member(&team.id, "invited-user")
            .await
            .expect("check member")
    );
    assert!(
        manager
            .accept_teamspace_invite(&token, "another-user")
            .await
            .is_err()
    );
    manager
        .revoke_teamspace_member(&team.id, "invited-user", "owner-user")
        .await
        .expect("revoke member");
    assert!(
        !manager
            .is_teamspace_member(&team.id, "invited-user")
            .await
            .expect("revoked member loses access")
    );
    assert!(
        manager
            .revoke_teamspace_member(&team.id, "owner-user", "owner-user")
            .await
            .is_err()
    );

    let (task, _) = manager
        .create_task_with_metadata(TeamTaskCreateInput {
            team_id: &team.id,
            title: "owned work",
            created_by_actor_id: "coordinator",
            priority: TeamTaskPriority::Medium,
            assigned_member_id: Some("worker-1"),
            context: json!({}),
            conversation_mode: "group_chat",
            topic: None,
        })
        .await
        .expect("create assigned task");
    manager
        .update_task_status(&task.id, TeamTaskStatus::InProgress)
        .await
        .expect("start task");
    let claim = manager
        .claim_task_execution(&task.id, "worker-1", 60)
        .await
        .expect("claim task");
    assert_eq!(claim.lease_generation, 1);
    let active_goal = manager
        .get_task_goal_lease(&task.id)
        .await
        .expect("load active goal")
        .expect("goal is created with the task claim");
    assert_eq!(active_goal.owner_member_id, "worker-1");
    assert!(active_goal.released_at.is_none());
    assert!(
        manager
            .claim_task_execution(&task.id, "worker-1", 60)
            .await
            .is_err()
    );
    assert!(
        manager
            .claim_task_execution(&task.id, "worker-2", 60)
            .await
            .is_err()
    );

    let handed_off = manager
        .handoff_task_execution(
            &task.id,
            "worker-2",
            "owner-user",
            "worker-1 is unavailable",
        )
        .await
        .expect("handoff task");
    assert_eq!(handed_off.assigned_member_id.as_deref(), Some("worker-2"));
    let released_goal = manager
        .get_task_goal_lease(&task.id)
        .await
        .expect("load released goal")
        .expect("goal record is retained for audit");
    assert_eq!(released_goal.release_reason.as_deref(), Some("handoff"));
    assert!(released_goal.released_at.is_some());
    let replacement_claim = manager
        .claim_task_execution(&task.id, "worker-2", 60)
        .await
        .expect("replacement owner can claim task");
    assert_eq!(replacement_claim.lease_generation, 2);
    let replacement_goal = manager
        .get_task_goal_lease(&task.id)
        .await
        .expect("load replacement goal")
        .expect("replacement goal exists");
    assert_eq!(
        replacement_goal.lease_generation,
        replacement_claim.lease_generation
    );
    assert!(
        manager
            .claim_task_execution(&task.id, "worker-1", 60)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn task_goal_capacity_is_reserved_per_member_and_released_at_terminal_status() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "goal-capacity".to_string(),
            description: None,
            spec: json!({"members":[
                {"member_id":"worker-1","role":"worker"},
                {"member_id":"worker-2","role":"worker"},
                {"member_id":"worker-3","role":"worker"},
                {"member_id":"worker-4","role":"worker"}
            ]}),
        })
        .await
        .expect("create team");

    let (first, _) = manager
        .create_task_with_metadata(TeamTaskCreateInput {
            team_id: &team.id,
            title: "first goal",
            created_by_actor_id: "coordinator",
            priority: TeamTaskPriority::Medium,
            assigned_member_id: Some("worker-1"),
            context: json!({}),
            conversation_mode: "group_chat",
            topic: None,
        })
        .await
        .expect("create first task");
    manager
        .update_task_status(&first.id, TeamTaskStatus::InProgress)
        .await
        .expect("start first task");
    manager
        .claim_task_execution(&first.id, "worker-1", 60)
        .await
        .expect("claim first goal");

    let other_team = manager
        .create_team(TeamDefinitionConfig {
            name: "goal-capacity-other-team".to_string(),
            description: None,
            spec: json!({"members":[{"member_id":"worker-1","role":"worker"}]}),
        })
        .await
        .expect("create other team");
    let (other_team_task, _) = manager
        .create_task_with_metadata(TeamTaskCreateInput {
            team_id: &other_team.id,
            title: "independent Team goal",
            created_by_actor_id: "coordinator",
            priority: TeamTaskPriority::Medium,
            assigned_member_id: Some("worker-1"),
            context: json!({}),
            conversation_mode: "group_chat",
            topic: None,
        })
        .await
        .expect("create other Team task");
    manager
        .update_task_status(&other_team_task.id, TeamTaskStatus::InProgress)
        .await
        .expect("start other Team task");
    manager
        .claim_task_execution(&other_team_task.id, "worker-1", 60)
        .await
        .expect("same member id in another Team has independent capacity");

    for member_id in ["worker-2", "worker-3"] {
        let title = format!("goal for {member_id}");
        let (parallel, _) = manager
            .create_task_with_metadata(TeamTaskCreateInput {
                team_id: &team.id,
                title: &title,
                created_by_actor_id: "coordinator",
                priority: TeamTaskPriority::Medium,
                assigned_member_id: Some(member_id),
                context: json!({}),
                conversation_mode: "group_chat",
                topic: None,
            })
            .await
            .expect("create parallel task");
        manager
            .update_task_status(&parallel.id, TeamTaskStatus::InProgress)
            .await
            .expect("start parallel task");
        manager
            .claim_task_execution(&parallel.id, member_id, 60)
            .await
            .expect("reserve Team capacity");
    }
    let (over_capacity, _) = manager
        .create_task_with_metadata(TeamTaskCreateInput {
            team_id: &team.id,
            title: "fourth concurrent Team goal",
            created_by_actor_id: "coordinator",
            priority: TeamTaskPriority::Medium,
            assigned_member_id: Some("worker-4"),
            context: json!({}),
            conversation_mode: "group_chat",
            topic: None,
        })
        .await
        .expect("create over-capacity task");
    manager
        .update_task_status(&over_capacity.id, TeamTaskStatus::InProgress)
        .await
        .expect("start over-capacity task");
    assert!(
        manager
            .claim_task_execution(&over_capacity.id, "worker-4", 60)
            .await
            .is_err()
    );

    let (second, _) = manager
        .create_task_with_metadata(TeamTaskCreateInput {
            team_id: &team.id,
            title: "second goal",
            created_by_actor_id: "coordinator",
            priority: TeamTaskPriority::Medium,
            assigned_member_id: Some("worker-1"),
            context: json!({}),
            conversation_mode: "group_chat",
            topic: None,
        })
        .await
        .expect("create second task");
    manager
        .update_task_status(&second.id, TeamTaskStatus::InProgress)
        .await
        .expect("start second task");
    assert!(
        manager
            .claim_task_execution(&second.id, "worker-1", 60)
            .await
            .is_err()
    );

    manager
        .update_task_status(&first.id, TeamTaskStatus::Completed)
        .await
        .expect("complete first task");
    let released = manager
        .get_task_goal_lease(&first.id)
        .await
        .expect("load completed goal")
        .expect("completed goal is retained");
    assert_eq!(released.release_reason.as_deref(), Some("completed"));
    assert!(released.released_at.is_some());

    manager
        .claim_task_execution(&second.id, "worker-1", 60)
        .await
        .expect("terminal release makes capacity available");
}

#[tokio::test]
async fn goal_fork_requires_an_active_goal_and_returns_an_immutable_result() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "goal-fork".to_string(),
            description: None,
            spec: json!({"members":[{"member_id":"worker-1","role":"worker"}]}),
        })
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task_with_metadata(TeamTaskCreateInput {
            team_id: &team.id,
            title: "research parent",
            created_by_actor_id: "coordinator",
            priority: TeamTaskPriority::Medium,
            assigned_member_id: Some("worker-1"),
            context: json!({}),
            conversation_mode: "group_chat",
            topic: None,
        })
        .await
        .expect("create task");
    assert!(
        manager
            .create_goal_fork(
                &task.id,
                "check",
                "cite evidence",
                json!({"type":"object"}),
                chrono::Utc::now().timestamp() + 60,
                None,
            )
            .await
            .is_err()
    );
    manager
        .update_task_status(&task.id, TeamTaskStatus::InProgress)
        .await
        .expect("start");
    manager
        .claim_task_execution(&task.id, "worker-1", 300)
        .await
        .expect("claim");
    let fork = manager
        .create_goal_fork(
            &task.id,
            "check",
            "cite evidence",
            json!({"type":"object"}),
            chrono::Utc::now().timestamp() + 60,
            None,
        )
        .await
        .expect("create read-only fork");
    assert_eq!(fork.profile, "read_only");
    assert_eq!(fork.result_schema, json!({"type":"object"}));
    let second = manager
        .create_goal_fork(
            &task.id,
            "check another source",
            "cite independent evidence",
            json!({"type":"object"}),
            chrono::Utc::now().timestamp() + 60,
            None,
        )
        .await
        .expect("create second read-only fork");
    assert!(
        manager
            .create_goal_fork(
                &task.id,
                "exceed capacity",
                "must not start",
                json!({"type":"object"}),
                chrono::Utc::now().timestamp() + 60,
                None,
            )
            .await
            .is_err()
    );
    let completed = manager
        .complete_goal_fork(
            &fork.id,
            json!({"summary":"evidence","api_key":"secret"}),
            None,
        )
        .await
        .expect("complete");
    assert_eq!(
        completed.result,
        Some(json!({"summary":"evidence","api_key":"[redacted]"}))
    );
    let notes = manager
        .list_task_notes(&task.id, 10)
        .await
        .expect("list parent evidence");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].kind.as_str(), "result");
    assert_eq!(notes[0].text, "evidence");
    manager
        .create_goal_fork(
            &task.id,
            "reuse released capacity",
            "start after completion",
            json!({"type":"object"}),
            chrono::Utc::now().timestamp() + 60,
            None,
        )
        .await
        .expect("completed fork releases Team fork capacity");
    assert!(
        manager
            .complete_goal_fork(&fork.id, json!({"summary":"rewrite"}), None)
            .await
            .is_err()
    );
    assert!(manager.get_goal_fork(&second.id).await.is_ok());
}

#[tokio::test]
async fn goal_fork_completion_is_fenced_by_the_parent_lease_generation() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "goal-fork-fencing".to_string(),
            description: None,
            spec: json!({"members":[
                {"member_id":"worker-1","role":"worker"},
                {"member_id":"worker-2","role":"worker"}
            ]}),
        })
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task_with_metadata(TeamTaskCreateInput {
            team_id: &team.id,
            title: "fenced parent",
            created_by_actor_id: "coordinator",
            priority: TeamTaskPriority::Medium,
            assigned_member_id: Some("worker-1"),
            context: json!({}),
            conversation_mode: "group_chat",
            topic: None,
        })
        .await
        .expect("create task");
    manager
        .update_task_status(&task.id, TeamTaskStatus::InProgress)
        .await
        .expect("start task");
    manager
        .claim_task_execution(&task.id, "worker-1", 300)
        .await
        .expect("claim task");
    let fork = manager
        .create_goal_fork(
            &task.id,
            "check before handoff",
            "return to the original generation",
            json!({"type":"object"}),
            chrono::Utc::now().timestamp() + 60,
            None,
        )
        .await
        .expect("create fork");
    manager
        .handoff_task_execution(&task.id, "worker-2", "owner-user", "ownership changed")
        .await
        .expect("handoff task");
    manager
        .claim_task_execution(&task.id, "worker-2", 300)
        .await
        .expect("replacement owner claims task");
    for question in [
        "first current-generation fork",
        "second current-generation fork",
    ] {
        manager
            .create_goal_fork(
                &task.id,
                question,
                "return current-generation evidence",
                json!({"type":"object"}),
                chrono::Utc::now().timestamp() + 60,
                None,
            )
            .await
            .expect("stale fork does not consume current Team capacity");
    }

    assert!(
        manager
            .complete_goal_fork(&fork.id, json!({"summary":"stale"}), None)
            .await
            .is_err()
    );
    assert!(
        manager
            .list_task_notes(&task.id, 10)
            .await
            .expect("list parent evidence")
            .is_empty()
    );
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

/// A stale terminal-status write racing a concurrent handoff must not silently apply: whichever
/// transaction's guarded `UPDATE team_tasks` actually commits first wins, and the other must fail
/// cleanly instead of releasing a goal lease it never observed. Uses a real WAL-backed multi-connection
/// pool (`setup_concurrent_teamspace_db`) because the shared `:memory:` pool used elsewhere is
/// single-connection and cannot exercise genuine interleaving between two transactions.
///
/// Whether any single attempt actually lands the two operations' internal reads/writes in the
/// vulnerable order is scheduler-dependent -- empirically only about 1 in 20 single attempts do, even
/// with a real multi-connection pool, since `update_task_status` reaches its write in fewer awaits than
/// `handoff_task_execution` and so usually wins outright rather than losing to a stale read. Looping
/// many fresh task/lease pairs turns that low per-attempt probability into a reliable regression check
/// (a single-attempt version of this test reliably passed even with the CAS guard reverted).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_terminal_status_update_and_handoff_do_not_both_apply() {
    const ATTEMPTS: usize = 60;

    let (db, dir) = setup_concurrent_teamspace_db().await;

    struct CleanupGuard(std::path::PathBuf);
    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            let path = std::mem::take(&mut self.0);
            tokio::spawn(async move {
                let _ = tokio::fs::remove_dir_all(path).await;
            });
        }
    }
    let _cleanup = CleanupGuard(dir);

    let manager = Arc::new(TeamManager::new(db.clone()));

    for attempt in 0..ATTEMPTS {
        let now = Utc::now().timestamp();
        let team_id = format!("team-{}", Uuid::new_v4());
        sqlx::query(
            "INSERT INTO team_definitions (id, name, spec_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        )
        .bind(&team_id)
        .bind(format!("race-team-{}", Uuid::new_v4()))
        .bind(json!({"entrypoint":"member-a","members":[{"member_id":"member-a"},{"member_id":"member-b"}]}).to_string())
        .bind(now)
        .execute(&db)
        .await
        .expect("insert team");

        let task_id = format!("task-{}", Uuid::new_v4());
        sqlx::query(
            "INSERT INTO team_tasks (id, team_id, title, status, priority, created_by_actor_id, assigned_member_id, context_json, created_at, updated_at) \
             VALUES (?1, ?2, 'race task', 'in_progress', 'medium', 'user', 'member-a', '{}', ?3, ?3)",
        )
        .bind(&task_id)
        .bind(&team_id)
        .bind(now)
        .execute(&db)
        .await
        .expect("insert task");

        manager
            .claim_task_execution(&task_id, "member-a", 300)
            .await
            .expect("claim initial goal lease");

        let complete_manager = manager.clone();
        let complete_task_id = task_id.clone();
        let complete = tokio::spawn(async move {
            complete_manager
                .update_task_status(&complete_task_id, TeamTaskStatus::Completed)
                .await
        });
        let handoff_manager = manager.clone();
        let handoff_task_id = task_id.clone();
        let handoff = tokio::spawn(async move {
            handoff_manager
                .handoff_task_execution(&handoff_task_id, "member-b", "owner-1", "reassign")
                .await
        });
        let (complete_result, handoff_result) = tokio::join!(complete, handoff);
        let complete_result = complete_result.expect("complete task did not panic");
        let handoff_result = handoff_result.expect("handoff task did not panic");

        let final_task = manager.get_task(&task_id).await.expect("reload task");
        let lease_row = sqlx::query(
            "SELECT released_at, release_reason FROM team_goal_leases WHERE task_id = ?1 AND lease_generation = 1",
        )
        .bind(&task_id)
        .fetch_one(&db)
        .await
        .expect("reload goal lease");
        let released_at: Option<i64> = lease_row.get("released_at");
        let release_reason: Option<String> = lease_row.get("release_reason");

        match (complete_result.is_ok(), handoff_result.is_ok()) {
            (true, false) => {
                assert_eq!(
                    final_task.status,
                    TeamTaskStatus::Completed,
                    "attempt {attempt}"
                );
                assert_eq!(
                    final_task.assigned_member_id.as_deref(),
                    Some("member-a"),
                    "attempt {attempt}"
                );
                assert_eq!(
                    release_reason.as_deref(),
                    Some("completed"),
                    "attempt {attempt}"
                );
            }
            (false, true) => {
                assert_eq!(
                    final_task.status,
                    TeamTaskStatus::InProgress,
                    "attempt {attempt}"
                );
                assert_eq!(
                    final_task.assigned_member_id.as_deref(),
                    Some("member-b"),
                    "attempt {attempt}"
                );
                assert_eq!(
                    release_reason.as_deref(),
                    Some("handoff"),
                    "attempt {attempt}"
                );
            }
            other => panic!(
                "attempt {attempt}: expected exactly one of the two concurrent operations to win, got {:?} (complete={:?}, handoff={:?})",
                other, complete_result, handoff_result
            ),
        }
        assert!(
            released_at.is_some(),
            "attempt {attempt}: the generation-1 goal lease must be released by whichever operation won"
        );
    }
}
