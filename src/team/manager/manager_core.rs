use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use agenthub_db::AgentEventDbRouter;
use agenthub_message_archive::MessageArchiveStoreRef;
use sqlx::{Error as SqlxError, SqlitePool};
use tokio::sync::broadcast;

use super::remote_relay_types::{GrpcRelayTlsDefaults, TeamRemoteRelayAdapter};
use super::{TEAM_CONVERSATION_STREAM_BUFFER_CAPACITY, TeamConversationStreamEvent, TeamManager};
use crate::internal::client::InternalGrpcPeerClientConfig;
use crate::internal::tls::InternalGrpcSecurityMode;

pub(super) fn hex_encode(data: &[u8]) -> String {
    const HEX_CHARS: &[u8] = b"0123456789abcdef";
    let mut result = String::with_capacity(data.len() * 2);
    for byte in data {
        result.push(HEX_CHARS[(byte >> 4) as usize] as char);
        result.push(HEX_CHARS[(byte & 0xf) as usize] as char);
    }
    result
}

pub(super) fn is_row_not_found(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<SqlxError>(),
        Some(SqlxError::RowNotFound)
    )
}

#[derive(Debug, thiserror::Error)]
pub(super) enum TaskConversationMessageStoreError {
    #[error("idempotency_key conflicts with an existing task conversation message payload")]
    IdempotencyConflict,
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
            body_store: None,
            message_index: None,
            read_repair: None,
            conversation_events,
            remote_relay_adapter,
            agents_target_node_id_column,
        }
    }

    /// Attach the tiered message body store. The read path uses it to rehydrate bodies that have been
    /// moved out of `payload_json`; `None` keeps bodies inline.
    pub fn with_body_store(
        mut self,
        body_store: Option<crate::message_body_store::SharedBodyStore>,
    ) -> Self {
        self.body_store = body_store;
        self
    }

    /// Attach the rebuildable message index store. Reads use it only when the relevant stream is
    /// marked fresh through SQLite authority and fall back to SQLite otherwise.
    pub fn with_message_index(
        mut self,
        message_index: Option<crate::message_body_store::SharedIndexStore>,
    ) -> Self {
        self.message_index = message_index;
        self
    }

    /// Attach a scheduler for lagging index projections discovered by guarded reads.
    pub fn with_read_repair_scheduler(
        mut self,
        read_repair: Option<crate::message_body_store::SharedReadRepairScheduler>,
    ) -> Self {
        self.read_repair = read_repair;
        self
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

    pub(super) fn schedule_index_read_repair(
        &self,
        stream_id: impl Into<String>,
        authority_max: u64,
        freshness: agenthub_message_store::IndexFreshness,
    ) {
        let agenthub_message_store::IndexFreshness::Lagging {
            indexed_through, ..
        } = freshness
        else {
            return;
        };
        let Some(scheduler) = self.read_repair.as_deref() else {
            return;
        };
        let stream_id = stream_id.into();
        if let Err(error) =
            scheduler.schedule_read_repair(agenthub_message_store::IndexReadRepairRequest {
                stream_id: stream_id.clone(),
                authority_max,
                reason: agenthub_message_store::IndexReadRepairReason::Lagging { indexed_through },
            })
        {
            tracing::warn!(
                ?error,
                stream_id,
                authority_max,
                "failed to schedule message index read repair"
            );
        }
    }
}
