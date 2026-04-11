mod mailbox_hint;
mod manager;
mod permission_review;
mod role_skills;
mod runtime;

use std::collections::HashSet;

use crate::acp::{AcpActorSkillContext, DEFAULT_ACTOR_CHANNEL};
pub use agenthub_team_actor::{
    ActorMessageRecord as TeamActorMessageRecord, ActorMessageStatus as TeamActorMessageStatus,
    ActorMessageTransport as TeamActorMessageTransport,
};
pub use agenthub_team_domain::{
    TEAM_RUN_CONTINUITY_MODE_VALUES, TEAM_RUN_STATUS_VALUES, TEAM_STEP_STATUS_VALUES,
    TEAM_TASK_STATUS_VALUES, TeamConversationMessageRecord, TeamConversationRecord,
    TeamDefinitionConfig, TeamDefinitionRecord, TeamMemberContinuityStateRecord,
    TeamRunEventRecord, TeamRunRecord, TeamRunResumeError, TeamRunStatus, TeamStepRecord,
    TeamStepStatus, TeamTaskDetailRecord, TeamTaskRecord, TeamTaskStatus,
};
pub(crate) use mailbox_hint::{
    ActorMailboxImmediateHintReason, TeamMailboxUnreadHintWorker,
    TeamMailboxUnreadHintWorkerSettings, build_actor_mailbox_immediate_hint_prompt,
    plan_actor_mailbox_immediate_hint,
};
pub(crate) use manager::TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX;
#[allow(unused_imports)]
pub use manager::{
    SendActorMessageInput, TeamContextLookupError, TeamContextRecord, TeamConversationStreamEvent,
    TeamManager, TeamMemoryFlushRequest, TeamRemoteRelayWorkerSettings, TeamRuntimeRecord,
    TeamRuntimeStatus, TeamTaskAssignmentUpdate, TeamTaskContextPatch, TeamTaskListQuery,
};
pub(crate) use permission_review::resolve_team_permission_review_target;
pub use permission_review::{
    TeamPermissionReviewDispatcher, TeamPermissionReviewDispatcherSettings,
};
pub use role_skills::effective_team_member_skills;
pub use runtime::{
    TeamRuntimeControlRecord, TeamRuntimeStartError, ensure_team_runtime_started,
    force_team_member_new_session, stop_team_runtime,
};
use serde_json::Value;

pub(crate) fn collect_team_member_ids(spec: &Value) -> Vec<String> {
    let Some(members) = spec.get("members").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut member_ids = Vec::with_capacity(members.len());
    let mut seen = HashSet::with_capacity(members.len());
    for member in members {
        let Some(member_id) = member
            .get("member_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if seen.insert(member_id) {
            member_ids.push(member_id.to_string());
        }
    }
    member_ids
}

pub(crate) fn team_member_role_from_spec(spec: &Value, member_id: &str) -> Option<String> {
    let members = spec.get("members")?.as_array()?;
    members
        .iter()
        .find(|member| {
            member
                .get("member_id")
                .and_then(Value::as_str)
                .map(str::trim)
                == Some(member_id)
        })
        .and_then(|member| member.get("role"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|role| !role.is_empty())
        .map(str::to_string)
}

pub(crate) fn build_team_member_actor_context_for_role(
    team_id: &str,
    run_id: Option<&str>,
    member_id: &str,
    member_role: &str,
) -> AcpActorSkillContext {
    AcpActorSkillContext {
        team_id: Some(team_id.to_string()),
        current_run_id: run_id.map(str::to_string),
        actor_id: member_id.to_string(),
        default_channel: DEFAULT_ACTOR_CHANNEL.to_string(),
        member_role: Some(member_role.to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    }
}

pub(crate) fn normalize_optional_idempotency_key_input(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
