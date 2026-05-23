use agenthub_team_actor::ListActorInboxQuery;

use super::mailbox_store::{SqlActorMailboxStore, SqlActorMailboxStoreError};
use super::mailbox_store_inbox_enrichment::enrich_actor_messages;
use super::mailbox_store_inbox_queries::{
    count_pending_inbox_on_executor, inbox_rows_include_pending, list_inbox_rows_on_executor,
    parse_inbox_rows,
};
use crate::team::TeamActorMessageRecord;

#[derive(Debug)]
pub(super) struct ActorInboxSnapshot {
    pub(super) messages: Vec<TeamActorMessageRecord>,
    pub(super) pending_count: i64,
}

impl SqlActorMailboxStore {
    pub(super) async fn read_inbox_snapshot(
        &self,
        query: &ListActorInboxQuery,
    ) -> Result<ActorInboxSnapshot, SqlActorMailboxStoreError> {
        let mut tx = self.db.begin().await?;
        let pending_count = count_pending_inbox_on_executor(
            &mut *tx,
            &query.run_id,
            &query.actor_id,
            &query.peer_id,
        )
        .await?;
        let rows = if query.include_delivered && query.after_id.is_none() {
            let pending_only_query = ListActorInboxQuery {
                include_delivered: false,
                ..query.clone()
            };
            let pending_rows = list_inbox_rows_on_executor(&mut *tx, &pending_only_query).await?;
            if pending_rows.is_empty() {
                list_inbox_rows_on_executor(&mut *tx, query).await?
            } else {
                let requested_rows = list_inbox_rows_on_executor(&mut *tx, query).await?;
                if inbox_rows_include_pending(&requested_rows) {
                    requested_rows
                } else {
                    pending_rows
                }
            }
        } else {
            list_inbox_rows_on_executor(&mut *tx, query).await?
        };
        let mut messages = parse_inbox_rows(rows)?;
        tx.commit().await?;
        enrich_actor_messages(&self.db, &mut messages).await?;
        Ok(ActorInboxSnapshot {
            messages,
            pending_count,
        })
    }

    pub(super) async fn list_inbox_messages(
        &self,
        query: &ListActorInboxQuery,
    ) -> Result<Vec<TeamActorMessageRecord>, SqlActorMailboxStoreError> {
        let rows = if query.include_delivered && query.after_id.is_none() {
            let pending_only_query = ListActorInboxQuery {
                include_delivered: false,
                ..query.clone()
            };
            let pending_rows = list_inbox_rows_on_executor(&self.db, &pending_only_query).await?;
            if pending_rows.is_empty() {
                list_inbox_rows_on_executor(&self.db, query).await?
            } else {
                let requested_rows = list_inbox_rows_on_executor(&self.db, query).await?;
                if inbox_rows_include_pending(&requested_rows) {
                    requested_rows
                } else {
                    pending_rows
                }
            }
        } else {
            list_inbox_rows_on_executor(&self.db, query).await?
        };
        let mut messages = parse_inbox_rows(rows)?;
        enrich_actor_messages(&self.db, &mut messages).await?;
        Ok(messages)
    }
}
