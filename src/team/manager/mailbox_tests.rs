use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ActorMessageHandlingDisposition, ActorServiceErrorCode,
    infer_actor_message_kind,
};
use serde_json::{Value, json};

use super::TeamMemberSpecView;
use super::mailbox::TEAM_SPECIAL_USER_ACTOR_ALIAS;
use super::mailbox_errors::{
    map_actor_service_error, validate_direct_mailbox_target_for_member_specs,
};
use super::mailbox_payloads::{
    channel_payload_correlation_id, ensure_channel_message_correlation_id,
    normalize_channel_message_payload, resolve_canonical_chat_reply,
    should_persist_human_visible_chat_reply_for_payload,
};
use super::mailbox_reply_obligations::summarize_open_reply_obligations_from_messages;
use crate::team::{TeamActorMessageRecord, TeamActorMessageStatus, TeamActorMessageTransport};

fn mock_member_specs(member_specs: &[(&str, &str)]) -> Vec<TeamMemberSpecView> {
    member_specs
        .iter()
        .map(|(member_id, role)| TeamMemberSpecView {
            member_id: (*member_id).to_string(),
            role: (*role).to_string(),
            description: None,
        })
        .collect()
}

fn build_mailbox_record(
    message_id: i64,
    from_actor_id: &str,
    to_actor_id: &str,
    payload: Value,
) -> TeamActorMessageRecord {
    TeamActorMessageRecord {
        message_id,
        run_id: "run-1".to_string(),
        from_actor_id: from_actor_id.to_string(),
        from_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
        from_actor_kind: agenthub_team_actor::infer_actor_identity_kind(from_actor_id),
        to_actor_id: to_actor_id.to_string(),
        to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
        to_actor_kind: agenthub_team_actor::infer_actor_identity_kind(to_actor_id),
        channel: "default".to_string(),
        transport: TeamActorMessageTransport::Local,
        route: None,
        payload: payload.clone(),
        idempotency_key: None,
        message_kind: infer_actor_message_kind(from_actor_id, &payload, None),
        status: TeamActorMessageStatus::Pending,
        handling_disposition: ActorMessageHandlingDisposition::Untriaged,
        handled_by_actor_id: None,
        thread_topic_key: None,
        thread_claim_status: None,
        thread_owner_actor_id: None,
        thread_lease_expires_at: None,
        linked_task_id: None,
        linked_task_relation: None,
        created_at: 1_700_000_000 + message_id,
        delivered_at: None,
        handled_at: None,
    }
}

#[test]
fn normalize_channel_message_payload_wraps_non_object_inputs() {
    let text = normalize_channel_message_payload(Value::String("hello".to_string()));
    let number = normalize_channel_message_payload(json!(42));

    assert_eq!(
        text,
        json!({
            "type": "chat_message",
            "text": "hello"
        })
    );
    assert_eq!(
        number,
        json!({
            "type": "chat_message",
            "text": "42"
        })
    );
}

#[test]
fn ensure_channel_message_correlation_id_preserves_existing_or_uses_fallback() {
    let existing = ensure_channel_message_correlation_id(
        json!({
            "type": "chat_message",
            "text": "hello",
            "correlation_id": "corr-existing"
        }),
        Some("corr-fallback"),
    );
    let fallback = ensure_channel_message_correlation_id(
        json!({
            "type": "chat_message",
            "text": "hello"
        }),
        Some("corr-fallback"),
    );

    assert_eq!(
        channel_payload_correlation_id(&existing),
        Some("corr-existing")
    );
    assert_eq!(
        channel_payload_correlation_id(&fallback),
        Some("corr-fallback")
    );
}

#[test]
fn map_actor_service_error_surfaces_readonly_database_failures() {
    let mapped = map_actor_service_error(anyhow::anyhow!("attempt to write a readonly database"));

    assert_eq!(mapped.code, ActorServiceErrorCode::Internal);
    assert_eq!(
        mapped.message,
        "mailbox write failed: attempt to write a readonly database"
    );
}

#[test]
fn human_visible_chat_reply_requires_local_non_human_to_human_chat_message() {
    let payload = json!({
        "type": "chat_message",
        "text": "final answer"
    });

    assert!(should_persist_human_visible_chat_reply_for_payload(
        &agenthub_team_actor::ActorMessageTransport::Local,
        TEAM_SPECIAL_USER_ACTOR_ALIAS,
        ACTOR_MAIN_PEER_ID,
        "worker",
        &payload,
    ));
    assert!(!should_persist_human_visible_chat_reply_for_payload(
        &agenthub_team_actor::ActorMessageTransport::Remote,
        TEAM_SPECIAL_USER_ACTOR_ALIAS,
        ACTOR_MAIN_PEER_ID,
        "worker",
        &payload,
    ));
    assert!(!should_persist_human_visible_chat_reply_for_payload(
        &agenthub_team_actor::ActorMessageTransport::Local,
        "reviewer",
        ACTOR_MAIN_PEER_ID,
        "worker",
        &payload,
    ));
    assert!(!should_persist_human_visible_chat_reply_for_payload(
        &agenthub_team_actor::ActorMessageTransport::Local,
        "user:123",
        ACTOR_MAIN_PEER_ID,
        TEAM_SPECIAL_USER_ACTOR_ALIAS,
        &payload,
    ));
    assert!(!should_persist_human_visible_chat_reply_for_payload(
        &agenthub_team_actor::ActorMessageTransport::Local,
        TEAM_SPECIAL_USER_ACTOR_ALIAS,
        ACTOR_MAIN_PEER_ID,
        "worker",
        &json!({
            "type": "tool_result",
            "text": "not a chat"
        }),
    ));
}

#[test]
fn resolve_canonical_chat_reply_accepts_stringified_json_chat_message() {
    let reply = resolve_canonical_chat_reply(&Value::String(
        r#"{"type":"chat_message","text":"hello","correlation_id":"corr-9"}"#.to_string(),
    ))
    .expect("resolve chat reply");

    assert_eq!(reply.text, "hello");
    assert_eq!(reply.correlation_id.as_deref(), Some("corr-9"));
}

#[test]
fn summarize_open_reply_obligations_counts_unanswered_human_requests() {
    let messages = vec![build_mailbox_record(
        1,
        "user",
        "worker",
        json!({
            "type": "chat_message",
            "text": "Need update",
            "requires_user_visible_reply": true
        }),
    )];

    let summary = summarize_open_reply_obligations_from_messages(messages.as_slice());

    assert_eq!(summary.open_total, 1);
    assert_eq!(summary.open_by_actor.get("worker").copied(), Some(1));
    assert_eq!(summary.open_items.len(), 1);
    let obligation = &summary.open_items[0];
    assert_eq!(obligation.agent_actor_id, "worker");
    assert_eq!(obligation.human_actor_id, "user");
    assert_eq!(obligation.source_surface, "mailbox");
    assert_eq!(obligation.text_excerpt.as_deref(), Some("Need update"));
}

#[test]
fn summarize_open_reply_obligations_consumes_visible_reply_credit() {
    let mut inbound = build_mailbox_record(
        1,
        "user",
        "worker",
        json!({
            "type": "chat_message",
            "text": "Need update",
            "requires_user_visible_reply": true
        }),
    );
    inbound.status = TeamActorMessageStatus::Delivered;
    let outbound = build_mailbox_record(
        2,
        "worker",
        "user",
        json!({
            "type": "chat_message",
            "text": "Here is the update"
        }),
    );

    let summary = summarize_open_reply_obligations_from_messages(&[inbound, outbound]);

    assert_eq!(summary.open_total, 0);
    assert!(summary.open_by_actor.is_empty());
}

#[test]
fn summarize_open_reply_obligations_skips_ignored_and_completed_items() {
    let mut ignored = build_mailbox_record(
        1,
        "user",
        "worker",
        json!({
            "type": "chat_message",
            "text": "Need update",
            "requires_user_visible_reply": true
        }),
    );
    ignored.handling_disposition = ActorMessageHandlingDisposition::Ignored;
    let mut completed = build_mailbox_record(
        2,
        "user",
        "reviewer",
        json!({
            "type": "chat_message",
            "text": "Please review",
            "requires_user_visible_reply": true
        }),
    );
    completed.handling_disposition = ActorMessageHandlingDisposition::Completed;

    let summary = summarize_open_reply_obligations_from_messages(&[ignored, completed]);

    assert_eq!(summary.open_total, 0);
    assert!(summary.open_by_actor.is_empty());
}

#[test]
fn validate_direct_mailbox_target_rejects_role_alias() {
    let member_specs = mock_member_specs(&[
        ("595d1ae8-fcbd-4111-b5c7-d446a12c044b", "coordinator"),
        ("c319f933-1358-4418-a111-872304052422", "worker"),
    ]);
    let err = validate_direct_mailbox_target_for_member_specs(&member_specs, "coordinator")
        .expect_err("role alias should be rejected");
    assert_eq!(err.code, ActorServiceErrorCode::BadRequest);
    assert!(err.message.contains("not a canonical team member_id"));
    assert!(err.message.contains("595d1ae8-fcbd-4111-b5c7-d446a12c044b"));
}

#[test]
fn validate_direct_mailbox_target_allows_member_id_and_human_mailbox() {
    let member_specs = mock_member_specs(&[
        ("595d1ae8-fcbd-4111-b5c7-d446a12c044b", "coordinator"),
        ("c319f933-1358-4418-a111-872304052422", "worker"),
    ]);
    validate_direct_mailbox_target_for_member_specs(
        &member_specs,
        "595d1ae8-fcbd-4111-b5c7-d446a12c044b",
    )
    .expect("member id should be accepted");
    validate_direct_mailbox_target_for_member_specs(&member_specs, "user")
        .expect("human alias should be accepted");
    validate_direct_mailbox_target_for_member_specs(&member_specs, "user:test")
        .expect("human actor target should be accepted");
}

#[test]
fn validate_direct_mailbox_target_rejects_empty_human_mailbox_suffix() {
    let member_specs = mock_member_specs(&[
        ("595d1ae8-fcbd-4111-b5c7-d446a12c044b", "coordinator"),
        ("c319f933-1358-4418-a111-872304052422", "worker"),
    ]);
    let err = validate_direct_mailbox_target_for_member_specs(&member_specs, "user:")
        .expect_err("human mailbox target with empty suffix should be rejected");
    assert_eq!(err.code, ActorServiceErrorCode::BadRequest);
    assert!(
        err.message
            .contains("must reference spec.members[].member_id")
    );
}

#[test]
fn validate_direct_mailbox_target_rejects_ambiguous_role_alias() {
    let member_specs = mock_member_specs(&[
        ("planner", "coordinator"),
        ("worker-a", "worker"),
        ("worker-b", "worker"),
    ]);
    let err = validate_direct_mailbox_target_for_member_specs(&member_specs, "worker")
        .expect_err("ambiguous role alias should be rejected");
    assert_eq!(err.code, ActorServiceErrorCode::BadRequest);
    assert!(err.message.contains("matches multiple team members"));
    assert!(err.message.contains("worker-a"));
    assert!(err.message.contains("worker-b"));
}
