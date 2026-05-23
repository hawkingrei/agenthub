use super::*;
use std::collections::HashMap;
use std::sync::Arc;

use crate::api::team_tests::build_test_state;
use crate::team::mailbox_hint::RunningActorRuntime;
use crate::team::permission_review::tests_support::{
    PermissionReviewRequestFixture, TestMailboxHintAgentNudger, create_permission_review_team,
    insert_pending_permission_request, insert_running_agent,
};
use agenthub_acp::acp_permission_review_timeout;
use agenthub_acp::{
    AcpPermissionReviewDispatcher, AcpPermissionReviewRequest, AcpPermissionService,
};
use agenthub_team_actor::{ActorInboxRequest, ActorMailboxService, ActorMessageStatus};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::payload::build_permission_review_summary;
use super::selection::{PermissionReviewCandidate, collect_team_permission_review_candidates};

#[test]
fn permission_review_dispatcher_default_human_fallback_is_forty_seconds() {
    assert_eq!(
        TeamPermissionReviewDispatcherSettings::default().human_fallback_delay,
        DEFAULT_TEAM_PERMISSION_REVIEW_HUMAN_FALLBACK_DELAY
    );
}

#[test]
fn permission_review_human_fallback_stays_below_acp_timeout() {
    assert!(
        TeamPermissionReviewDispatcherSettings::default().human_fallback_delay
            < acp_permission_review_timeout(),
        "human fallback delay must remain below ACP permission timeout"
    );
}

#[test]
fn builds_permission_review_summary_from_tool_name() {
    let request = AcpPermissionReviewRequest {
        request_id: "perm-1".to_string(),
        agent_id: "worker-agent".to_string(),
        agent_session_id: "session-1".to_string(),
        acp_session_id: "acp-1".to_string(),
        tool_call_id: Some("tool-1".to_string()),
        options: Vec::new(),
        tool_call: Some(json!({"tool":{"name":"mcp__fs__read"}})),
        current_run_id: None,
        routing: agenthub_acp::AcpPermissionRoutingMetadata {
            team_id: Some("team-1".to_string()),
            requester_actor_id: Some("worker-1".to_string()),
            requester_role: Some("worker".to_string()),
        },
    };
    assert_eq!(
        build_permission_review_summary(&request),
        "worker-1 requests permission to execute mcp__fs__read."
    );
}

#[test]
fn worker_request_skips_coordinator_even_if_coordinator_member_role_is_misconfigured_as_worker() {
    let spec = json!({
        "entrypoint":"coordinator",
        "coordinator_member_id":"coordinator",
        "members":[
            {"member_id":"coordinator","role":"worker"},
            {"member_id":"reviewer","role":"worker"},
            {"member_id":"worker","role":"worker"}
        ]
    });

    let (reviewer, dispatch_status) =
        resolve_team_permission_review_target(&spec, "worker", "worker").expect("resolve reviewer");

    assert_eq!(reviewer, "reviewer");
    assert_eq!(dispatch_status, "worker_dispatched");
}

#[test]
fn requester_role_is_trimmed_before_review_target_resolution() {
    let spec = json!({
        "entrypoint":"planner",
        "coordinator_member_id":"planner",
        "members":[
            {"member_id":"planner","role":"coordinator"},
            {"member_id":"reviewer","role":"worker"}
        ]
    });

    let (reviewer, dispatch_status) =
        resolve_team_permission_review_target(&spec, "planner", " coordinator ")
            .expect("resolve reviewer");

    assert_eq!(reviewer, "reviewer");
    assert_eq!(dispatch_status, "worker_dispatched");
}

#[test]
fn collect_permission_review_candidates_keeps_coordinator_as_fallback_after_workers() {
    let spec = json!({
        "entrypoint":"coordinator",
        "coordinator_member_id":"coordinator",
        "members":[
            {"member_id":"coordinator","role":"coordinator"},
            {"member_id":"busy","role":"worker"},
            {"member_id":"idle","role":"worker"},
            {"member_id":"worker","role":"worker"}
        ]
    });

    let candidates = collect_team_permission_review_candidates(&spec, "worker", "worker")
        .expect("collect reviewer candidates");

    assert_eq!(
        candidates,
        vec![
            PermissionReviewCandidate {
                actor_id: "busy".to_string(),
                dispatch_status: "worker_dispatched",
                idle_dispatch_status: "worker_idle_dispatched",
            },
            PermissionReviewCandidate {
                actor_id: "idle".to_string(),
                dispatch_status: "worker_dispatched",
                idle_dispatch_status: "worker_idle_dispatched",
            },
            PermissionReviewCandidate {
                actor_id: "coordinator".to_string(),
                dispatch_status: "coordinator_dispatched",
                idle_dispatch_status: "coordinator_idle_dispatched",
            },
        ]
    );
}

#[test]
fn worker_request_uses_coordinator_when_no_peer_worker_exists() {
    let spec = json!({
        "entrypoint":"coordinator",
        "coordinator_member_id":"coordinator",
        "members":[
            {"member_id":"coordinator","role":"coordinator"},
            {"member_id":"worker","role":"worker"}
        ]
    });

    let (reviewer, dispatch_status) =
        resolve_team_permission_review_target(&spec, "worker", "worker")
            .expect("resolve coordinator fallback reviewer");

    assert_eq!(reviewer, "coordinator");
    assert_eq!(dispatch_status, "coordinator_dispatched");
}

#[test]
fn coordinator_request_errors_without_subordinate_reviewer() {
    let spec = json!({
        "entrypoint":"coordinator",
        "coordinator_member_id":"coordinator",
        "members":[
            {"member_id":"coordinator","role":"coordinator"}
        ]
    });

    let err = resolve_team_permission_review_target(&spec, "coordinator", "coordinator")
        .expect_err("coordinator should need a subordinate reviewer");

    assert!(
        err.to_string()
            .contains("team has no subordinate reviewer configured"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn dispatches_worker_permission_to_peer_worker_before_human_review() {
    let state = build_test_state().await;
    let team = create_permission_review_team(
        &state,
        "permission-review",
        "team permission review dispatch",
        json!({
            "entrypoint":"coordinator",
            "coordinator_member_id":"coordinator",
            "members":[
                {"member_id":"coordinator","role":"coordinator"},
                {"member_id":"reviewer","role":"worker"},
                {"member_id":"worker","role":"worker"}
            ]
        }),
    )
    .await;
    let now = chrono::Utc::now().timestamp();
    let fixture = PermissionReviewRequestFixture {
        request_id: "perm-review-timeout",
        agent_id: "worker-agent",
        session_id: "worker-session",
        acp_session_id: "acp-session-1",
        requester_actor_id: "worker",
        requester_role: "worker",
        tool_call_id: "tool-call-1",
        tool_name: "mcp__fs__read",
    };
    insert_running_agent(&state, fixture.agent_id, fixture.session_id, now).await;
    insert_pending_permission_request(&state, &team.id, &fixture, now).await;

    let dispatcher = TeamPermissionReviewDispatcher::new(
        state.teams.clone(),
        state.agents.clone(),
        Arc::new(AcpPermissionService::new(state.db.clone())),
        TeamPermissionReviewDispatcherSettings {
            human_fallback_delay: Duration::from_millis(10),
        },
    );
    let request = fixture.request(&team.id);

    dispatcher
        .dispatch_review(request.clone())
        .await
        .expect("dispatch permission review");

    let record = state
        .acp_permissions
        .get("perm-review-timeout")
        .await
        .expect("load permission record")
        .expect("permission record");
    assert_eq!(record.review_target_actor_id.as_deref(), Some("reviewer"));
    assert_eq!(
        record.review_dispatch_status.as_deref(),
        Some("worker_dispatched")
    );
    let run_id = record
        .review_delivery_run_id
        .as_deref()
        .expect("review run id")
        .to_string();

    let inbox = state
        .teams
        .actor_mailbox_service()
        .actor_inbox(ActorInboxRequest {
            run_id: run_id.clone(),
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: None,
        })
        .await
        .expect("load reviewer inbox");
    assert_eq!(inbox.messages.len(), 1);
    assert_eq!(
        inbox.messages[0].payload["type"],
        Value::from(TEAM_PERMISSION_REVIEW_PAYLOAD_TYPE)
    );

    let (task_id, _) = state
        .teams
        .ensure_shared_thread_target_for_team(&team.id, "worker")
        .await
        .expect("ensure shared thread");
    dispatcher
        .notify_human_review_if_pending(&request, "review_timeout")
        .await
        .expect("fallback to human review");

    let conversation_messages = state
        .teams
        .list_task_conversation_messages(&task_id, 50, None)
        .await
        .expect("list shared-thread messages");
    let fallback = conversation_messages.iter().find(|message| {
        message.payload.get("type").and_then(Value::as_str)
            == Some(TEAM_HUMAN_PERMISSION_CARD_PAYLOAD_TYPE)
            && message.payload.get("permission_id").and_then(Value::as_str)
                == Some("perm-review-timeout")
    });
    let record_after_timeout = state
        .acp_permissions
        .get("perm-review-timeout")
        .await
        .expect("reload permission record")
        .expect("permission record after timeout");
    let fallback = fallback.unwrap_or_else(|| {
        panic!(
            "human-review fallback message missing; record={record_after_timeout:?} messages={conversation_messages:?}"
        )
    });
    assert_eq!(fallback.from_actor_id, "worker");
    assert_eq!(
        fallback.payload["reason_text"],
        json!("Agent review timed out")
    );
    assert_eq!(fallback.payload["status"], json!("pending"));

    assert_eq!(
        record_after_timeout.review_dispatch_status.as_deref(),
        Some("review_timeout")
    );
    assert!(record_after_timeout.human_review_notified_at.is_some());

    let reviewer_pending_inbox = state
        .teams
        .actor_mailbox_service()
        .actor_inbox(ActorInboxRequest {
            run_id: run_id.clone(),
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: None,
        })
        .await
        .expect("reload reviewer pending inbox");
    assert_eq!(
        reviewer_pending_inbox.messages.len(),
        0,
        "stale agent-review mailbox request should be acked after human fallback"
    );
    assert_eq!(reviewer_pending_inbox.pending_count, 0);

    let reviewer_delivered_inbox = state
        .teams
        .actor_mailbox_service()
        .actor_inbox(ActorInboxRequest {
            run_id,
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![ActorMessageStatus::Delivered]),
        })
        .await
        .expect("reload reviewer delivered inbox");
    assert_eq!(reviewer_delivered_inbox.messages.len(), 1);
    assert_eq!(
        reviewer_delivered_inbox.messages[0].payload["type"],
        Value::from(TEAM_PERMISSION_REVIEW_PAYLOAD_TYPE)
    );
}

#[tokio::test]
async fn dispatches_worker_permission_to_idle_peer_worker_before_busy_peer() {
    let state = build_test_state().await;
    let team = create_permission_review_team(
        &state,
        "permission-review-idle-first",
        "idle-first permission review dispatch",
        json!({
            "entrypoint":"coordinator",
            "coordinator_member_id":"coordinator",
            "members":[
                {"member_id":"coordinator","role":"coordinator"},
                {"member_id":"busy","role":"worker"},
                {"member_id":"idle","role":"worker"},
                {"member_id":"worker","role":"worker"}
            ]
        }),
    )
    .await;
    let now = chrono::Utc::now().timestamp();
    let fixture = PermissionReviewRequestFixture {
        request_id: "perm-review-idle-first",
        agent_id: "worker-agent",
        session_id: "worker-session",
        acp_session_id: "acp-session-idle-first",
        requester_actor_id: "worker",
        requester_role: "worker",
        tool_call_id: "tool-call-idle-first",
        tool_name: "mcp__fs__read",
    };
    insert_running_agent(&state, fixture.agent_id, fixture.session_id, now).await;
    insert_pending_permission_request(&state, &team.id, &fixture, now).await;

    let nudger = Arc::new(TestMailboxHintAgentNudger {
        runtimes: HashMap::from([
            (
                "busy".to_string(),
                RunningActorRuntime {
                    session_id: "session-busy".to_string(),
                    current_run_id: Some("run-busy".to_string()),
                },
            ),
            (
                "idle".to_string(),
                RunningActorRuntime {
                    session_id: "session-idle".to_string(),
                    current_run_id: Some("run-idle".to_string()),
                },
            ),
        ]),
        idle_anchor_by_actor: HashMap::from([
            ("busy".to_string(), Some(now - 10)),
            ("idle".to_string(), Some(now - 600)),
        ]),
        prompts: Mutex::new(Vec::new()),
    });
    let dispatcher = TeamPermissionReviewDispatcher::with_agent_nudger(
        state.teams.clone(),
        nudger.clone(),
        Arc::new(AcpPermissionService::new(state.db.clone())),
        TeamPermissionReviewDispatcherSettings {
            human_fallback_delay: Duration::from_millis(10),
        },
    );
    let request = fixture.request(&team.id);

    dispatcher
        .dispatch_review(request)
        .await
        .expect("dispatch permission review");

    let record = state
        .acp_permissions
        .get("perm-review-idle-first")
        .await
        .expect("load permission record")
        .expect("permission record");
    assert_eq!(record.review_target_actor_id.as_deref(), Some("idle"));
    assert_eq!(
        record.review_dispatch_status.as_deref(),
        Some("worker_idle_dispatched")
    );
    let run_id = record
        .review_delivery_run_id
        .as_deref()
        .expect("review run id")
        .to_string();

    let idle_inbox = state
        .teams
        .actor_mailbox_service()
        .actor_inbox(ActorInboxRequest {
            run_id: run_id.clone(),
            actor_id: "idle".to_string(),
            cursor: None,
            limit: Some(20),
            states: None,
        })
        .await
        .expect("load idle reviewer inbox");
    assert_eq!(idle_inbox.messages.len(), 1);

    let busy_inbox = state
        .teams
        .actor_mailbox_service()
        .actor_inbox(ActorInboxRequest {
            run_id,
            actor_id: "busy".to_string(),
            cursor: None,
            limit: Some(20),
            states: None,
        })
        .await
        .expect("load busy reviewer inbox");
    assert!(busy_inbox.messages.is_empty());

    let prompts = nudger.prompts.lock().await;
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].0, "idle");
    assert!(prompts[0].2.contains(TEAM_PERMISSION_REVIEW_PAYLOAD_TYPE));
}

#[tokio::test]
async fn dispatches_coordinator_permission_to_subordinate_worker() {
    let state = build_test_state().await;
    let team = create_permission_review_team(
        &state,
        "permission-review-coordinator",
        "coordinator permission review dispatch",
        json!({
            "entrypoint":"coordinator",
            "coordinator_member_id":"coordinator",
            "members":[
                {"member_id":"coordinator","role":"coordinator"},
                {"member_id":"reviewer","role":"worker"},
                {"member_id":"worker","role":"worker"}
            ]
        }),
    )
    .await;
    let now = chrono::Utc::now().timestamp();
    let fixture = PermissionReviewRequestFixture {
        request_id: "perm-review-coordinator",
        agent_id: "coordinator-agent",
        session_id: "coordinator-session",
        acp_session_id: "acp-session-coordinator",
        requester_actor_id: "coordinator",
        requester_role: "coordinator",
        tool_call_id: "tool-call-coordinator",
        tool_name: "mcp__fs__write",
    };
    insert_running_agent(&state, fixture.agent_id, fixture.session_id, now).await;
    insert_pending_permission_request(&state, &team.id, &fixture, now).await;

    let dispatcher = TeamPermissionReviewDispatcher::new(
        state.teams.clone(),
        state.agents.clone(),
        Arc::new(AcpPermissionService::new(state.db.clone())),
        TeamPermissionReviewDispatcherSettings {
            human_fallback_delay: Duration::from_millis(10),
        },
    );
    let request = fixture.request(&team.id);

    dispatcher
        .dispatch_review(request)
        .await
        .expect("dispatch coordinator permission review");

    let record = state
        .acp_permissions
        .get("perm-review-coordinator")
        .await
        .expect("load permission record")
        .expect("permission record");
    assert_eq!(record.review_target_actor_id.as_deref(), Some("reviewer"));
    assert_eq!(
        record.review_dispatch_status.as_deref(),
        Some("worker_dispatched")
    );
    let run_id = record
        .review_delivery_run_id
        .as_deref()
        .expect("review run id")
        .to_string();

    let inbox = state
        .teams
        .actor_mailbox_service()
        .actor_inbox(ActorInboxRequest {
            run_id,
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: None,
        })
        .await
        .expect("load reviewer inbox");
    assert_eq!(inbox.messages.len(), 1);
    assert_eq!(
        inbox.messages[0].payload["review_target_actor_id"],
        Value::from("reviewer")
    );
}

#[tokio::test]
async fn dispatch_failure_falls_back_to_human_permission_card() {
    let state = build_test_state().await;
    let team = create_permission_review_team(
        &state,
        "permission-review-dispatch-failure",
        "permission review dispatch failure",
        json!({
            "entrypoint":"coordinator",
            "coordinator_member_id":"coordinator",
            "members":[
                {"member_id":"coordinator","role":"coordinator"}
            ]
        }),
    )
    .await;
    let now = chrono::Utc::now().timestamp();
    let fixture = PermissionReviewRequestFixture {
        request_id: "perm-review-dispatch-failure",
        agent_id: "coordinator-agent-fallback",
        session_id: "coordinator-session-fallback",
        acp_session_id: "acp-session-dispatch-failure",
        requester_actor_id: "coordinator",
        requester_role: "coordinator",
        tool_call_id: "tool-call-dispatch-failure",
        tool_name: "mcp__fs__write",
    };
    insert_running_agent(&state, fixture.agent_id, fixture.session_id, now).await;
    insert_pending_permission_request(&state, &team.id, &fixture, now).await;

    let dispatcher = TeamPermissionReviewDispatcher::new(
        state.teams.clone(),
        state.agents.clone(),
        Arc::new(AcpPermissionService::new(state.db.clone())),
        TeamPermissionReviewDispatcherSettings {
            human_fallback_delay: Duration::from_secs(60),
        },
    );
    let request = fixture.request(&team.id);

    dispatcher
        .dispatch_review(request)
        .await
        .expect("dispatch failure should be handled as human fallback");

    let record = state
        .acp_permissions
        .get("perm-review-dispatch-failure")
        .await
        .expect("load permission record")
        .expect("permission record");
    assert_eq!(
        record.review_dispatch_status.as_deref(),
        Some("review_dispatch_failed")
    );
    assert!(record.review_target_actor_id.is_none());
    assert!(record.human_review_notified_at.is_some());

    let (task_id, _) = state
        .teams
        .ensure_shared_thread_target_for_team(&team.id, "coordinator")
        .await
        .expect("ensure shared thread");
    let conversation_messages = state
        .teams
        .list_task_conversation_messages(&task_id, 50, None)
        .await
        .expect("list shared-thread messages");
    let fallback = conversation_messages
        .iter()
        .find(|message| {
            message.payload.get("type").and_then(Value::as_str)
                == Some(TEAM_HUMAN_PERMISSION_CARD_PAYLOAD_TYPE)
                && message.payload.get("permission_id").and_then(Value::as_str)
                    == Some("perm-review-dispatch-failure")
        })
        .expect("human permission card");
    assert_eq!(fallback.from_actor_id, "coordinator");
    assert_eq!(
        fallback.payload["reason_text"],
        json!("Agent review dispatch failed")
    );
    assert_eq!(fallback.payload["status"], json!("pending"));
}
