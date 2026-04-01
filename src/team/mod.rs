mod mailbox_hint;
mod manager;
mod permission_review;
mod role_skills;
mod runtime;

use std::collections::HashSet;

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
