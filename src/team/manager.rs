mod archive_documents;
mod archive_migration;
mod archive_migration_scope;
mod archive_payloads;
mod archive_scope;
mod channel_mutations;
mod channels;
mod codec_rows;
mod codec_status;
mod context_artifact_persistence;
mod context_artifact_support;
mod context_artifacts;
mod conversation;
mod conversation_append;
mod conversation_body;
mod conversation_idempotency;
mod conversation_insert_common;
mod conversation_side_effects;
mod conversation_tx_insert;
mod mailbox;
mod mailbox_channels;
mod mailbox_errors;
mod mailbox_facade;
mod mailbox_mentions;
mod mailbox_payloads;
mod mailbox_queries;
mod mailbox_reply_obligation_payloads;
mod mailbox_reply_obligation_snapshot_conversion;
mod mailbox_reply_obligation_snapshots;
mod mailbox_reply_obligation_summary;
mod mailbox_reply_obligations;
mod mailbox_service;
mod mailbox_service_channels;
mod mailbox_service_escalation;
mod mailbox_service_rpc;
mod mailbox_service_rpc_inbox;
mod mailbox_service_rpc_mutations;
mod mailbox_service_rpc_send;
mod mailbox_shared_thread;
mod mailbox_sqlite;
mod mailbox_store;
mod mailbox_store_delivery;
mod mailbox_store_inbox;
mod mailbox_store_inbox_enrichment;
mod mailbox_store_inbox_queries;
mod mailbox_store_mutations;
mod mailbox_store_relay;
#[cfg(test)]
mod mailbox_tests;
mod mailbox_threads;
mod mailbox_worker;
mod manager_consts;
mod manager_core;
mod manager_types;
mod memory_flush_event_loader;
mod memory_flush_execution;
mod memory_flush_finalize;
mod memory_flush_helpers;
mod memory_flush_request;
mod message_archive;
mod message_archive_batches;
mod message_archive_support;
#[cfg(any(test, feature = "rocksdb"))]
mod message_index_projection;
mod payload_utils;
mod remote_relay;
mod remote_relay_delivery;
mod remote_relay_grpc;
mod remote_relay_route;
mod remote_relay_types;
mod run_admin;
mod run_cancel;
mod run_create;
mod run_history;
mod run_queries;
mod run_summary;
mod run_task_status_sync;
mod runtime_context_lookup;
mod runtime_run_members;
mod runtime_team_runtime;
mod runtime_view_builders;
mod runtime_view_loaders;
mod runtime_views;
mod session_views;
mod shared_thread_mailbox_run;
mod shared_thread_targets;
mod state_mutations;
mod step_completion;
mod step_completion_continuity;
mod step_completion_run_finish;
mod step_continue;
mod step_continuity;
mod step_failure;
mod step_helpers;
mod step_input_required;
mod step_materialization;
mod step_progress;
mod step_queries;
mod step_reconcile;
mod step_resume;
mod step_template_builders;
mod support;
mod task_catalog;
mod task_updates;
mod team_catalog;
mod team_message_archive;
mod teamspace;

#[cfg(test)]
mod tests;

use std::sync::{Arc, Mutex};

use agenthub_team_domain::{continuity_note_relative_path, extract_context_artifact_path};
use sqlx::SqlitePool;
use tokio::sync::broadcast;

use self::archive_documents::{
    AgentEventArchiveSnapshot, MessageArchiveScopeFallback, team_actor_message_archive_document,
    team_conversation_message_archive_document, team_run_event_archive_document,
};
#[cfg(test)]
pub(super) use self::archive_payloads::message_archive_body_text;
use self::manager_consts::{
    CONTINUITY_ARTIFACT_KIND_OUTPUT, CONTINUITY_MAX_HISTORY_CHARS, CONTINUITY_MAX_SUMMARY_CHARS,
    CONTINUITY_MODE_DEFAULT, CONTINUITY_MODE_RESET, MEMORY_FLUSH_ARTIFACT_KIND,
    MEMORY_FLUSH_MAX_EVENTS_DEFAULT, MEMORY_FLUSH_MAX_EVENTS_MAX, MEMORY_FLUSH_MAX_EXCERPT_CHARS,
    MEMORY_FLUSH_MAX_SUMMARY_CHARS, MESSAGE_ARCHIVE_APPEND_TIMEOUT, RECONCILE_ROUND_ARTIFACT_KIND,
    SQLITE_CONSTRAINT_UNIQUE_CODE, TEAM_CONVERSATION_STREAM_BUFFER_CAPACITY,
    TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND, TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_SOURCE,
};
pub(crate) use self::manager_consts::{
    TEAM_CHANNEL_BOOTSTRAP_KIND, TEAM_SHARED_THREAD_BOOTSTRAP_KIND, TEAM_SHARED_THREAD_TITLE,
    TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX,
};
use self::manager_core::{TaskConversationMessageStoreError, hex_encode, is_row_not_found};
use self::manager_types::PreparedTeamTaskUpdate;
pub use self::manager_types::{
    TeamContextLookupError, TeamContextRecord, TeamContextRunOverlayRecord,
    TeamConversationStreamEvent, TeamExecutionClaimRecord, TeamGoalForkRecord, TeamGoalLeaseRecord,
    TeamMemberCardRecord, TeamMemoryFlushRequest, TeamMemoryFlushResult,
    TeamMessageArchiveMigrationReport, TeamPendingActorUnreadRecord, TeamReplyObligationRecord,
    TeamReplyObligationSummary, TeamRunContextFingerprint, TeamRunContextStreamEvent,
    TeamRunMemberRecord, TeamRunMemberStepRecord, TeamRunMembersRecord, TeamRuntimeMemberRecord,
    TeamRuntimeRecord, TeamRuntimeStatus, TeamRuntimeSummaryRecord, TeamTaskAssignmentUpdate,
    TeamTaskContextPatch, TeamTaskCreateInput, TeamTaskListQuery, TeamTaskNoteCreateInput,
    TeamTaskUpdateWithNoteInput, TeamspaceInviteRecord, TeamspaceMemberRecord,
};
use self::payload_utils::{
    filter_visible_team_runs, merge_json_value, redact_sensitive_json, resolve_task_context_patch,
};
use self::run_task_status_sync::load_linked_task_ids_for_runs;
use self::runtime_views::TeamMemberSpecView;
use self::runtime_views::parse_team_member_specs;
use self::support::maybe_attach_context_artifact_pointer;
pub use mailbox::{SendActorMessageInput, TeamRemoteRelayWorkerSettings};

use self::codec_rows::{
    parse_run_event_row, parse_team_actor_message_row, parse_team_conversation_message_row,
    parse_team_definition_row, parse_team_member_continuity_state_row, parse_team_run_row,
    parse_team_step_row, parse_team_task_note_row, parse_team_task_row,
};
use self::codec_status::{
    team_run_status_to_str, team_step_status_to_str, team_task_priority_to_str,
    team_task_status_to_str,
};
pub(crate) use self::conversation_body::{
    count_pending_conversation_body_migration, migrate_conversation_bodies_into_store,
};
#[cfg(test)]
pub(super) use self::conversation_idempotency::task_conversation_payload_correlation_id;
use self::remote_relay_types::TeamRemoteRelayAdapter;
pub(super) use self::shared_thread_targets::fetch_canonical_shared_thread_target;
use super::{
    TEAM_RUN_CONTINUITY_MODE_VALUES, TeamActorMessageRecord, TeamConversationRecord,
    TeamDefinitionConfig, TeamDefinitionRecord, TeamMemberContinuityStateRecord,
    TeamRunEventRecord, TeamRunRecord, TeamRunStatus, TeamStepRecord, TeamStepStatus,
    TeamTaskNoteRecord, TeamTaskPriority, TeamTaskRecord, TeamTaskStatus,
    TeamTaskStepExecutionSpec, build_team_member_actor_context_for_role, collect_team_member_ids,
    normalize_optional_idempotency_key_input, parse_task_execution_plan,
    team_member_role_from_spec, validate_task_execution_plan, validate_task_execution_steps,
};
use agenthub_db::AgentEventDbRouter;
use agenthub_message_archive::MessageArchiveStoreRef;
use agenthub_team_actor::ACTOR_MAIN_PEER_ID;

#[derive(Clone)]
pub struct TeamManager {
    db: SqlitePool,
    event_dbs: AgentEventDbRouter,
    message_archive: Option<MessageArchiveStoreRef>,
    /// Tiered message body store. When set, conversation bodies are dual-written through the durable
    /// outbox while SQLite remains the Phase 1 compatibility source of truth. Legacy sentinel rows
    /// are still rehydrated until the migration restores them.
    body_store: Option<crate::message_body_store::SharedBodyStore>,
    /// Rebuildable ordered message index. It is used only behind freshness guards and falls back to
    /// SQLite whenever the projection is missing, lagging, or incomplete.
    message_index: Option<crate::message_body_store::SharedIndexStore>,
    /// Read-repair scheduler for lagging ordered-index projections. Reads still serve SQLite.
    read_repair: Option<crate::message_body_store::SharedReadRepairScheduler>,
    conversation_events: broadcast::Sender<TeamConversationStreamEvent>,
    remote_relay_adapter: Arc<TeamRemoteRelayAdapter>,
    agents_target_node_id_column: Arc<Mutex<Option<bool>>>,
}
