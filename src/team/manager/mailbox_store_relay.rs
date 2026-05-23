use serde_json::Value;
use sqlx::Row;

use agenthub_team_actor::PendingRemoteRelayRecord;

use super::archive_scope::team_run_event_archive_document_for_db;
use super::codec_rows::parse_team_actor_message_row;
use super::mailbox::mailbox_run_event_archive_semaphore;
use super::mailbox_store::{SqlActorMailboxStore, SqlActorMailboxStoreError};
use crate::team::TeamRunEventRecord;

impl SqlActorMailboxStore {
    pub(super) async fn list_remote_pending_messages_impl(
        &self,
        limit: i64,
        now: i64,
    ) -> Result<Vec<PendingRemoteRelayRecord>, SqlActorMailboxStoreError> {
        let rows = sqlx::query(
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
                message_kind,
                handling_disposition,
                handled_by_actor_id,
                handled_at,
                status,
                created_at,
                delivered_at,
                relay_attempt
            FROM team_actor_messages
            WHERE transport = 'remote'
                AND status = 'pending'
                AND (
                    relay_next_retry_at IS NULL
                    OR relay_next_retry_at <= ?1
                )
            ORDER BY id ASC
            LIMIT ?2
            "#,
        )
        .bind(now)
        .bind(limit.max(1))
        .fetch_all(&self.db)
        .await?;

        let mut messages = Vec::with_capacity(rows.len());
        for row in rows {
            let message = parse_team_actor_message_row(&row)
                .map_err(|err| sqlx::Error::Protocol(err.to_string()))?;
            let attempt: i64 = row.try_get("relay_attempt").unwrap_or(0);
            messages.push(PendingRemoteRelayRecord { message, attempt });
        }
        Ok(messages)
    }

    pub(super) async fn mark_remote_retry_impl(
        &self,
        run_id: &str,
        message_id: i64,
        ts: i64,
        attempt: i64,
        next_retry_at: i64,
        error: &str,
    ) -> Result<(), SqlActorMailboxStoreError> {
        sqlx::query(
            r#"
            UPDATE team_actor_messages
            SET
                relay_attempt = ?1,
                relay_next_retry_at = ?2,
                relay_last_error = ?3
            WHERE id = ?4 AND run_id = ?5 AND transport = 'remote' AND status = 'pending'
            "#,
        )
        .bind(attempt)
        .bind(next_retry_at.max(ts))
        .bind(error)
        .bind(message_id)
        .bind(run_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub(super) async fn mark_remote_dead_letter_impl(
        &self,
        run_id: &str,
        message_id: i64,
        ts: i64,
        attempt: i64,
        error: &str,
    ) -> Result<(), SqlActorMailboxStoreError> {
        sqlx::query(
            r#"
            UPDATE team_actor_messages
            SET
                status = 'dead_letter',
                relay_attempt = ?1,
                dead_letter_at = ?2,
                relay_last_error = ?3
            WHERE id = ?4 AND run_id = ?5 AND transport = 'remote' AND status = 'pending'
            "#,
        )
        .bind(attempt)
        .bind(ts)
        .bind(error)
        .bind(message_id)
        .bind(run_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub(super) async fn append_run_event_impl(
        &self,
        run_id: &str,
        event_type: &str,
        ts: i64,
        payload: Value,
    ) -> Result<(), SqlActorMailboxStoreError> {
        let result = sqlx::query(
            r#"
            INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
            VALUES (?1, NULL, ?2, ?3, ?4)
            "#,
        )
        .bind(run_id)
        .bind(event_type)
        .bind(ts)
        .bind(payload.to_string())
        .execute(&self.db)
        .await?;
        let event = TeamRunEventRecord {
            event_id: result.last_insert_rowid(),
            run_id: run_id.to_string(),
            step_id: None,
            event_type: event_type.to_string(),
            ts,
            payload,
        };
        if let Some(archive) = self.message_archive.as_ref().cloned() {
            let db = self.db.clone();
            let permit = mailbox_run_event_archive_semaphore()
                .acquire_owned()
                .await
                .expect("mailbox run event archive semaphore stays open");
            let run_id = event.run_id.clone();
            let event_id = event.event_id;
            tokio::spawn(async move {
                let _permit = permit;
                match team_run_event_archive_document_for_db(&db, &event).await {
                    Ok(Some(document)) => {
                        match tokio::time::timeout(
                            super::MESSAGE_ARCHIVE_APPEND_TIMEOUT,
                            archive.append_documents(&[document]),
                        )
                        .await
                        {
                            Ok(Ok(())) => {}
                            Ok(Err(error)) => {
                                tracing::warn!(
                                    error = ?error,
                                    run_id = %run_id,
                                    event_id,
                                    "failed to dual-write team actor mailbox run event to archive"
                                );
                            }
                            Err(_) => {
                                tracing::warn!(
                                    run_id = %run_id,
                                    event_id,
                                    timeout_ms = super::MESSAGE_ARCHIVE_APPEND_TIMEOUT.as_millis(),
                                    "timed out dual-writing team actor mailbox run event to archive"
                                );
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        tracing::warn!(
                            error = ?error,
                            run_id = %run_id,
                            event_id,
                            "failed to build team actor mailbox run event archive document"
                        );
                    }
                }
            });
        }
        Ok(())
    }
}
