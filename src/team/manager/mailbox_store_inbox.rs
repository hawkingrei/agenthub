use agenthub_team_actor::{
    ActorMessageHandlingDisposition, ActorMessageStatus, ListActorInboxQuery,
};
use sqlx::{QueryBuilder, Sqlite};

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
        match self.try_list_inbox_messages_from_index(query).await {
            Ok(Some(mut messages)) => {
                enrich_actor_messages(&self.db, &mut messages).await?;
                return Ok(messages);
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    ?error,
                    run_id = %query.run_id,
                    actor_id = %query.actor_id,
                    peer_id = %query.peer_id,
                    "falling back to SQLite after actor inbox index read failed"
                );
            }
        }
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

    async fn try_list_inbox_messages_from_index(
        &self,
        query: &ListActorInboxQuery,
    ) -> Result<Option<Vec<TeamActorMessageRecord>>, SqlActorMailboxStoreError> {
        if !query.include_delivered {
            return Ok(None);
        }
        let Some(index) = self.message_index.as_deref() else {
            return Ok(None);
        };

        let authority_max = self.max_actor_inbox_message_id_for_page(query).await?;
        if authority_max == 0 {
            return Ok(Some(Vec::new()));
        }
        let freshness = agenthub_message_store::check_index_freshness(
            index,
            "team_actor_messages",
            authority_max as u64,
        )
        .map_err(|err| sqlx::Error::Protocol(err.to_string()))?;
        if !matches!(
            freshness,
            agenthub_message_store::IndexFreshness::Fresh { .. }
        ) {
            self.schedule_index_read_repair("team_actor_messages", authority_max as u64, freshness);
            return Ok(None);
        }

        let inbox_id = format!("{}:{}", query.peer_id, query.actor_id);
        let refs = index
            .scan_prefix(&agenthub_message_store::keys::inbox_prefix(&inbox_id))
            .map_err(|err| sqlx::Error::Protocol(err.to_string()))?;
        let mut ids = Vec::new();
        let after_id = query.after_id.unwrap_or_default();
        let limit = query.limit.max(1) as usize;
        for message_ref in refs {
            if message_ref.source_kind != "team_actor_messages"
                || message_ref.run_id.as_deref() != Some(query.run_id.as_str())
            {
                continue;
            }
            let Some(message_id) =
                actor_message_id_from_delivery_id(&query.run_id, message_ref.message_id.as_str())
            else {
                return Ok(None);
            };
            if message_id <= after_id {
                continue;
            }
            ids.push(message_id);
            if ids.len() >= limit {
                break;
            }
        }
        if ids != self.expected_actor_inbox_message_ids(query, limit).await? {
            return Ok(None);
        }

        let messages = self.load_actor_messages_by_ids(query, &ids).await?;
        if query.after_id.is_none()
            && self
                .first_page_index_would_hide_pending(query, &messages)
                .await?
        {
            return Ok(None);
        }

        Ok(Some(messages))
    }

    async fn expected_actor_inbox_message_ids(
        &self,
        query: &ListActorInboxQuery,
        limit: usize,
    ) -> Result<Vec<i64>, SqlActorMailboxStoreError> {
        Ok(sqlx::query_scalar(
            r#"
            SELECT id
            FROM team_actor_messages
            WHERE run_id = ?1
              AND to_actor_id = ?2
              AND to_peer_id = ?3
              AND id > ?4
            ORDER BY id ASC
            LIMIT ?5
            "#,
        )
        .bind(&query.run_id)
        .bind(&query.actor_id)
        .bind(&query.peer_id)
        .bind(query.after_id.unwrap_or_default())
        .bind(limit as i64)
        .fetch_all(&self.db)
        .await?)
    }

    async fn first_page_index_would_hide_pending(
        &self,
        query: &ListActorInboxQuery,
        messages: &[TeamActorMessageRecord],
    ) -> Result<bool, SqlActorMailboxStoreError> {
        if messages.iter().any(is_untriaged_pending_actor_message) {
            return Ok(false);
        }

        let pending_count = count_pending_inbox_on_executor(
            &self.db,
            &query.run_id,
            &query.actor_id,
            &query.peer_id,
        )
        .await?;
        Ok(pending_count > 0)
    }

    async fn max_actor_inbox_message_id_for_page(
        &self,
        query: &ListActorInboxQuery,
    ) -> Result<i64, SqlActorMailboxStoreError> {
        Ok(sqlx::query_scalar(
            r#"
            SELECT COALESCE(MAX(id), 0)
            FROM team_actor_messages
            WHERE run_id = ?1
              AND to_actor_id = ?2
              AND to_peer_id = ?3
              AND id > ?4
            "#,
        )
        .bind(&query.run_id)
        .bind(&query.actor_id)
        .bind(&query.peer_id)
        .bind(query.after_id.unwrap_or_default())
        .fetch_one(&self.db)
        .await?)
    }

    async fn load_actor_messages_by_ids(
        &self,
        query: &ListActorInboxQuery,
        ids: &[i64],
    ) -> Result<Vec<TeamActorMessageRecord>, SqlActorMailboxStoreError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                id,
                run_id,
                from_actor_id,
                from_peer_id,
                to_actor_id,
                to_peer_id,
                channel,
                transport,
                route_json,
                payload_json,
                idempotency_key,
                message_kind,
                handling_disposition,
                handled_by_actor_id,
                handled_at,
                status,
                created_at,
                delivered_at
            FROM team_actor_messages
            WHERE run_id =
            "#,
        );
        builder.push_bind(&query.run_id);
        builder.push(" AND to_actor_id = ");
        builder.push_bind(&query.actor_id);
        builder.push(" AND to_peer_id = ");
        builder.push_bind(&query.peer_id);
        builder.push(" AND id IN (");
        let mut separated = builder.separated(", ");
        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");

        let rows = builder.build().fetch_all(&self.db).await?;
        if rows.len() != ids.len() {
            return Err(sqlx::Error::Protocol(format!(
                "actor inbox index referenced {} rows for run {}, but SQLite hydrated {}",
                ids.len(),
                query.run_id,
                rows.len()
            ))
            .into());
        }

        let mut by_id = std::collections::HashMap::with_capacity(rows.len());
        for row in rows {
            let message = super::codec_rows::parse_team_actor_message_row(&row)
                .map_err(|err| sqlx::Error::Protocol(err.to_string()))?;
            by_id.insert(message.message_id, message);
        }

        let mut messages = Vec::with_capacity(ids.len());
        for id in ids {
            let Some(message) = by_id.remove(id) else {
                return Err(sqlx::Error::Protocol(format!(
                    "actor inbox index referenced row {id} that SQLite did not return"
                ))
                .into());
            };
            messages.push(message);
        }
        Ok(messages)
    }
}

fn is_untriaged_pending_actor_message(message: &TeamActorMessageRecord) -> bool {
    message.status == ActorMessageStatus::Pending
        && message.handling_disposition == ActorMessageHandlingDisposition::Untriaged
}

fn actor_message_id_from_delivery_id(run_id: &str, delivery_id: &str) -> Option<i64> {
    delivery_id
        .strip_prefix(&format!("team_actor_message:{run_id}:"))?
        .parse()
        .ok()
}
