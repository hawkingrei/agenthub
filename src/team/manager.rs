mod archive_documents;
mod archive_migration;
mod channels;
mod codec;
mod context_artifacts;
mod conversation;
mod mailbox;
mod mailbox_channels;
mod mailbox_errors;
mod mailbox_facade;
mod mailbox_mentions;
mod mailbox_payloads;
mod mailbox_queries;
mod mailbox_reply_obligations;
mod mailbox_service;
mod mailbox_service_channels;
mod mailbox_shared_thread;
mod mailbox_sqlite;
mod mailbox_store;
mod mailbox_store_delivery;
mod mailbox_store_inbox;
mod mailbox_store_mutations;
mod mailbox_store_relay;
#[cfg(test)]
mod mailbox_tests;
mod mailbox_threads;
mod mailbox_worker;
mod memory_flush;
mod message_archive;
mod payload_utils;
mod remote_relay;
mod run_admin;
mod run_lifecycle;
mod run_queries;
mod run_status_sync;
mod runtime_views;
mod session_views;
mod shared_thread;
mod state_mutations;
mod step_lifecycle;
mod step_queries;
mod support;
mod task_catalog;
mod task_updates;
mod team_catalog;
mod team_message_archive;

#[cfg(test)]
mod tests;

use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use agenthub_team_domain::{continuity_note_relative_path, extract_context_artifact_path};
use serde_json::Value;

fn hex_encode(data: &[u8]) -> String {
    const HEX_CHARS: &[u8] = b"0123456789abcdef";
    let mut result = String::with_capacity(data.len() * 2);
    for byte in data {
        result.push(HEX_CHARS[(byte >> 4) as usize] as char);
        result.push(HEX_CHARS[(byte & 0xf) as usize] as char);
    }
    result
}
use sqlx::{Error as SqlxError, SqlitePool};
use tokio::sync::broadcast;

#[cfg(test)]
pub(super) use self::archive_documents::message_archive_body_text;
use self::archive_documents::{
    AgentEventArchiveSnapshot, MessageArchiveScopeFallback, message_archive_payload_string,
    message_archive_scope_for_payload_db, team_actor_message_archive_document,
    team_conversation_message_archive_document, team_run_event_archive_document,
    team_run_event_archive_document_for_db, team_run_event_archive_document_for_db_cached,
};
use self::context_artifacts::ContextArtifactPointer;
use self::payload_utils::{
    filter_visible_team_runs, merge_json_value, redact_sensitive_json, resolve_task_context_patch,
};
use self::run_status_sync::load_linked_task_ids_for_runs;
use self::runtime_views::TeamMemberSpecView;
use self::runtime_views::parse_team_member_specs;
use self::step_lifecycle::build_continuity_snapshot;
use self::support::maybe_attach_context_artifact_pointer;
pub use mailbox::{SendActorMessageInput, TeamRemoteRelayWorkerSettings};

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

use self::codec::{
    parse_run_event_row, parse_team_actor_message_row, parse_team_conversation_message_row,
    parse_team_definition_row, parse_team_member_continuity_state_row, parse_team_run_row,
    parse_team_step_row, parse_team_task_note_row, parse_team_task_row, team_run_status_to_str,
    team_step_status_to_str, team_task_priority_to_str, team_task_status_to_str,
};
#[cfg(test)]
pub(super) use self::conversation::task_conversation_payload_correlation_id;
use self::remote_relay::{GrpcRelayTlsDefaults, TeamRemoteRelayAdapter};
pub(super) use self::shared_thread::fetch_canonical_shared_thread_target;
use super::{
    TEAM_RUN_CONTINUITY_MODE_VALUES, TeamActorMessageRecord, TeamConversationRecord,
    TeamDefinitionConfig, TeamDefinitionRecord, TeamMemberContinuityStateRecord,
    TeamRunEventRecord, TeamRunRecord, TeamRunStatus, TeamStepRecord, TeamStepStatus,
    TeamTaskNoteRecord, TeamTaskPriority, TeamTaskRecord, TeamTaskStatus,
    TeamTaskStepExecutionSpec, build_team_member_actor_context_for_role, collect_team_member_ids,
    normalize_optional_idempotency_key_input, parse_task_execution_plan,
    team_member_role_from_spec, validate_task_execution_plan, validate_task_execution_steps,
};
use crate::internal::client::InternalGrpcPeerClientConfig;
use crate::internal::tls::InternalGrpcSecurityMode;
use agenthub_db::AgentEventDbRouter;
use agenthub_message_archive::MessageArchiveStoreRef;
use agenthub_team_actor::ACTOR_MAIN_PEER_ID;

#[derive(Clone)]
pub struct TeamManager {
    db: SqlitePool,
    event_dbs: AgentEventDbRouter,
    message_archive: Option<MessageArchiveStoreRef>,
    conversation_events: broadcast::Sender<TeamConversationStreamEvent>,
    remote_relay_adapter: Arc<TeamRemoteRelayAdapter>,
    agents_target_node_id_column: Arc<Mutex<Option<bool>>>,
}

const CONTINUITY_MODE_DEFAULT: &str = "inherit_recent";
const CONTINUITY_MODE_RESET: &str = "reset";
const CONTINUITY_MAX_SUMMARY_CHARS: usize = 2048;
const CONTINUITY_MAX_HISTORY_CHARS: usize = 4096;
const CONTINUITY_ARTIFACT_KIND_OUTPUT: &str = "continuity_output";
const RECONCILE_ROUND_ARTIFACT_KIND: &str = "reconcile_round_result";
const MEMORY_FLUSH_MAX_EVENTS_DEFAULT: i64 = 200;
const MEMORY_FLUSH_MAX_EVENTS_MAX: i64 = 1000;
const MEMORY_FLUSH_MAX_SUMMARY_CHARS: usize = 2048;
const MEMORY_FLUSH_MAX_EXCERPT_CHARS: usize = 700;
const MEMORY_FLUSH_ARTIFACT_KIND: &str = "memory_flush";
const TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND: &str = "shared_thread_mailbox";
const TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_SOURCE: &str = "teams_all";
const TEAM_CONVERSATION_STREAM_BUFFER_CAPACITY: usize = 256;
pub(crate) const TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX: i64 = 500;
pub(crate) const TEAM_SHARED_THREAD_TITLE: &str = "all";
pub(crate) const TEAM_SHARED_THREAD_BOOTSTRAP_KIND: &str = "shared_thread";
pub(crate) const TEAM_CHANNEL_BOOTSTRAP_KIND: &str = "team_channel";
const SQLITE_CONSTRAINT_UNIQUE_CODE: &str = "2067";
const MESSAGE_ARCHIVE_APPEND_TIMEOUT: Duration = Duration::from_secs(2);
fn is_row_not_found(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<SqlxError>(),
        Some(SqlxError::RowNotFound)
    )
}

#[derive(Debug, thiserror::Error)]
enum TaskConversationMessageStoreError {
    #[error("idempotency_key conflicts with an existing task conversation message payload")]
    IdempotencyConflict,
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
    current: TeamTaskRecord,
    status_patch: Option<TeamTaskStatus>,
    priority_patch: Option<TeamTaskPriority>,
    assignment_patch: Option<Option<String>>,
    context_patch: Option<Value>,
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

impl TeamManager {
    #[cfg(test)]
    pub(crate) fn task_message_idempotency_conflict_error() -> anyhow::Error {
        TaskConversationMessageStoreError::IdempotencyConflict.into()
    }

    pub fn is_task_message_idempotency_conflict(err: &anyhow::Error) -> bool {
        err.downcast_ref::<TaskConversationMessageStoreError>()
            .is_some_and(|cause| {
                matches!(
                    cause,
                    TaskConversationMessageStoreError::IdempotencyConflict
                )
            })
    }

    #[cfg(test)]
    pub fn new(db: SqlitePool) -> Self {
        Self::new_with_event_dbs(db, AgentEventDbRouter::with_default_base_dir())
    }

    #[cfg(test)]
    pub fn new_with_event_dbs(db: SqlitePool, event_dbs: AgentEventDbRouter) -> Self {
        Self::new_with_event_dbs_and_message_archive(db, event_dbs, None)
    }

    pub fn new_with_event_dbs_and_message_archive(
        db: SqlitePool,
        event_dbs: AgentEventDbRouter,
        message_archive: Option<MessageArchiveStoreRef>,
    ) -> Self {
        let (conversation_events, _) = broadcast::channel(TEAM_CONVERSATION_STREAM_BUFFER_CAPACITY);
        let remote_relay_adapter = Arc::new(TeamRemoteRelayAdapter::new(db.clone()));
        let agents_target_node_id_column = Arc::new(Mutex::new(None));
        Self {
            db,
            event_dbs,
            message_archive,
            conversation_events,
            remote_relay_adapter,
            agents_target_node_id_column,
        }
    }

    pub fn subscribe_conversation_events(
        &self,
    ) -> broadcast::Receiver<TeamConversationStreamEvent> {
        self.conversation_events.subscribe()
    }

    pub fn configure_internal_grpc_relay(&self, cert_dir: &Path, mode: InternalGrpcSecurityMode) {
        self.remote_relay_adapter
            .configure_grpc_tls_defaults(Some(GrpcRelayTlsDefaults::from_cert_dir(cert_dir, mode)));
    }

    pub fn configure_internal_grpc_peer_client(
        &self,
        config: Option<InternalGrpcPeerClientConfig>,
    ) {
        self.remote_relay_adapter.configure_grpc_peer_client(config);
    }
}
