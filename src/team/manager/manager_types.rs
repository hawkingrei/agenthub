use std::collections::HashMap;

use serde_json::Value;

use crate::team::{TeamStepStatus, TeamTaskPriority, TeamTaskRecord, TeamTaskStatus};

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct TeamMessageArchiveMigrationReport {
    pub team_conversation_messages: usize,
    pub team_run_events: usize,
    pub team_actor_messages: usize,
    pub agent_events: usize,
    pub aggregated_acp_messages: usize,
}

impl TeamMessageArchiveMigrationReport {
    pub fn total_documents(&self) -> usize {
        self.team_conversation_messages
            + self.team_run_events
            + self.team_actor_messages
            + self.agent_events
            + self.aggregated_acp_messages
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TeamConversationStreamEvent {
    pub team_id: String,
    pub task_id: String,
    pub conversation_id: String,
    pub message_id: Option<i64>,
    pub source: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TeamRunContextStreamEvent {
    pub team_id: String,
    pub run_id: String,
    pub refresh_run: bool,
    pub refresh_events: bool,
    pub refresh_snapshot: bool,
    pub refresh_mailbox: bool,
    pub latest_event_id: Option<i64>,
    pub latest_mailbox_message_id: Option<i64>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamRunContextFingerprint {
    pub team_id: String,
    pub run_id: String,
    pub run_status: String,
    pub latest_event_id: i64,
    pub latest_mailbox_message_id: i64,
    pub mailbox_pending: i64,
    pub mailbox_delivered: i64,
    pub mailbox_dead_letter: i64,
}

#[derive(Debug, Clone)]
pub struct TeamMemoryFlushRequest {
    pub member_id: String,
    pub session_id: Option<String>,
    pub trigger: String,
    pub max_events: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct TeamMemoryFlushResult {
    pub status: String,
    pub run_id: String,
    pub team_id: String,
    pub member_id: String,
    pub session_id: Option<String>,
    pub trigger: String,
    pub reason: Option<String>,
    pub artifact_pointer: Option<Value>,
    pub event_id_from: Option<i64>,
    pub event_id_to: Option<i64>,
    pub flushed_events: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamPendingActorUnreadRecord {
    pub run_id: String,
    pub actor_id: String,
    pub unread_count: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamRunMembersRecord {
    pub team_id: String,
    pub team_name: String,
    pub run_id: String,
    pub members: Vec<TeamRunMemberRecord>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamContextRunOverlayRecord {
    pub run_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TeamContextLookupError {
    #[error("team_id or run_id is required")]
    MissingSelector,
    #[error("run_id {run_id} belongs to team {actual_team_id}, not {requested_team_id}")]
    RunTeamMismatch {
        run_id: String,
        actual_team_id: String,
        requested_team_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamRuntimeStatus {
    Running,
    Stopped,
    Degraded,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamContextRecord {
    pub team_id: String,
    pub team_name: String,
    pub runtime: TeamRuntimeSummaryRecord,
    pub members: Vec<TeamRunMemberRecord>,
    pub run: Option<TeamContextRunOverlayRecord>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamRuntimeRecord {
    pub team_id: String,
    pub team_name: String,
    pub status: TeamRuntimeStatus,
    pub members: Vec<TeamRuntimeMemberRecord>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TeamReplyObligationRecord {
    pub message_id: i64,
    pub agent_actor_id: String,
    pub human_actor_id: String,
    pub source_surface: String,
    pub reply_target: Option<Value>,
    pub conversation_id: Option<String>,
    pub thread_root_message_id: Option<i64>,
    pub text_excerpt: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TeamReplyObligationSummary {
    pub open_total: i64,
    pub open_by_actor: HashMap<String, i64>,
    pub open_items: Vec<TeamReplyObligationRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamTaskAssignmentUpdate {
    Unchanged,
    Unassigned,
    Assigned(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TeamspaceMemberRecord {
    pub team_id: String,
    pub user_id: String,
    pub role: String,
    pub created_by_user_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TeamspaceInviteRecord {
    pub id: String,
    pub team_id: String,
    pub role: String,
    pub created_by_user_id: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub accepted_at: Option<i64>,
    pub revoked_at: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TeamExecutionClaimRecord {
    pub entity_kind: String,
    pub entity_id: String,
    pub team_id: String,
    pub owner_member_id: String,
    pub lease_generation: i64,
    pub claimed_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Default)]
pub struct TeamTaskListQuery {
    pub team_id: Option<String>,
    pub run_id: Option<String>,
    pub limit: i64,
    pub status: Option<TeamTaskStatus>,
    pub priority: Option<TeamTaskPriority>,
    pub task_id: Option<String>,
    pub assigned_member_id: Option<String>,
    pub topic: Option<String>,
    pub include_shared_thread: bool,
}

#[derive(Debug, Clone)]
pub enum TeamTaskContextPatch {
    Replace(Value),
    Merge(Value),
}

#[derive(Debug, Clone)]
pub struct TeamTaskCreateInput<'a> {
    pub team_id: &'a str,
    pub title: &'a str,
    pub created_by_actor_id: &'a str,
    pub priority: TeamTaskPriority,
    pub assigned_member_id: Option<&'a str>,
    pub context: Value,
    pub conversation_mode: &'a str,
    pub topic: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct TeamTaskNoteCreateInput<'a> {
    pub from_actor_id: &'a str,
    pub to_actor_id: Option<&'a str>,
    pub route: &'a str,
    pub payload: Value,
    pub idempotency_key: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct TeamTaskUpdateWithNoteInput<'a> {
    pub task_id: &'a str,
    pub status: Option<TeamTaskStatus>,
    pub assignment: TeamTaskAssignmentUpdate,
    pub priority: Option<TeamTaskPriority>,
    pub context_patch: Option<TeamTaskContextPatch>,
    pub note: Option<TeamTaskNoteCreateInput<'a>>,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedTeamTaskUpdate {
    pub(super) current: TeamTaskRecord,
    pub(super) status_patch: Option<TeamTaskStatus>,
    pub(super) priority_patch: Option<TeamTaskPriority>,
    pub(super) assignment_patch: Option<Option<String>>,
    pub(super) context_patch: Option<Value>,
}

impl PreparedTeamTaskUpdate {
    pub(super) fn has_changes(&self) -> bool {
        self.status_patch.is_some()
            || self.priority_patch.is_some()
            || self.assignment_patch.is_some()
            || self.context_patch.is_some()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamRuntimeSummaryRecord {
    pub status: TeamRuntimeStatus,
    pub online_count: usize,
    pub member_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamRuntimeMemberRecord {
    pub member_id: String,
    pub display_name: String,
    pub role: String,
    pub description: Option<String>,
    pub pending_inbox_count: i64,
    pub agent_status: Option<String>,
    pub session_id: Option<String>,
    pub session_status: Option<String>,
    pub card: TeamMemberCardRecord,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamRunMemberRecord {
    pub member_id: String,
    pub display_name: String,
    pub role: String,
    pub description: Option<String>,
    pub pending_inbox_count: i64,
    pub agent_status: Option<String>,
    pub session_id: Option<String>,
    pub session_status: Option<String>,
    pub card: TeamMemberCardRecord,
    pub steps: Vec<TeamRunMemberStepRecord>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamMemberCardRecord {
    pub card_id: String,
    pub schema_version: String,
    pub description: String,
    pub role: String,
    pub skills: Vec<String>,
    pub capability_tags: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamRunMemberStepRecord {
    pub step_id: String,
    pub step_key: String,
    pub status: TeamStepStatus,
    pub attempt: i64,
    pub session_id: Option<String>,
    pub session_status: Option<String>,
}
