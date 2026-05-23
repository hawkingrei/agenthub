use std::collections::HashMap;

use chrono::Utc;
use serde_json::Value;
use sqlx::Row;

#[cfg(test)]
use super::team_run_event_archive_document_for_db;
use super::{
    MESSAGE_ARCHIVE_APPEND_TIMEOUT, MessageArchiveScopeFallback,
    TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND, TeamActorMessageRecord, TeamManager,
    TeamRunEventRecord, message_archive_payload_string, message_archive_scope_for_payload_db,
    redact_sensitive_json, team_actor_message_archive_document,
    team_run_event_archive_document_for_db_cached,
};
use agenthub_message_archive::MessageDocument;

impl TeamManager {
    pub(super) fn spawn_archive_team_actor_message(&self, message: &TeamActorMessageRecord) {
        if self.message_archive.is_none() {
            return;
        }
        let manager = self.clone();
        let message = message.clone();
        let run_id = message.run_id.clone();
        let message_id = message.message_id;

        tokio::spawn(async move {
            match manager.team_actor_message_archive_document(&message).await {
                Ok(Some(document)) => {
                    let Some(archive) = manager.message_archive.as_ref().cloned() else {
                        return;
                    };
                    match tokio::time::timeout(
                        MESSAGE_ARCHIVE_APPEND_TIMEOUT,
                        archive.append_documents(&[document]),
                    )
                    .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => {
                            tracing::warn!(
                                error = ?error,
                                run_id = %run_id,
                                message_id,
                                "failed to dual-write team actor message to archive"
                            );
                        }
                        Err(_) => {
                            tracing::warn!(
                                run_id = %run_id,
                                message_id,
                                timeout_ms = MESSAGE_ARCHIVE_APPEND_TIMEOUT.as_millis(),
                                "timed out dual-writing team actor message to archive"
                            );
                        }
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(
                        error = ?error,
                        run_id = %run_id,
                        message_id,
                        "failed to build team actor message archive document"
                    );
                }
            }
        });
    }

    async fn team_actor_message_archive_document(
        &self,
        message: &TeamActorMessageRecord,
    ) -> anyhow::Result<Option<MessageDocument>> {
        let row = sqlx::query(
            r#"
            SELECT r.team_id, m.group_id, r.input_json
            FROM team_runs AS r
            LEFT JOIN team_actor_messages AS m ON m.id = ?2
            WHERE r.id = ?1
            "#,
        )
        .bind(&message.run_id)
        .bind(message.message_id)
        .fetch_one(&self.db)
        .await?;
        let team_id: String = row.get("team_id");
        let group_id: Option<String> = row.try_get("group_id")?;
        let input_json: String = row.get("input_json");
        let run_input: Value = serde_json::from_str(&input_json)?;
        if message_archive_payload_string(&run_input, "bootstrap_kind").as_deref()
            == Some(TEAM_SHARED_THREAD_MAILBOX_RUN_BOOTSTRAP_KIND)
        {
            return Ok(None);
        }
        let base_scope = MessageArchiveScopeFallback::from_run_input(&run_input);
        let scope = message_archive_scope_for_payload_db(
            &self.db,
            &team_id,
            &message.payload,
            &base_scope,
            &mut HashMap::new(),
        )
        .await?;
        Ok(Some(team_actor_message_archive_document(
            &team_id, message, &scope, group_id,
        )))
    }

    pub(super) fn spawn_archive_team_run_event(&self, event: &TeamRunEventRecord) {
        self.spawn_archive_team_run_events(vec![event.clone()]);
    }

    #[cfg(test)]
    pub(super) async fn team_run_event_archive_document(
        &self,
        event: &TeamRunEventRecord,
    ) -> anyhow::Result<Option<MessageDocument>> {
        team_run_event_archive_document_for_db(&self.db, event).await
    }

    pub(super) fn spawn_archive_team_run_events(&self, events: Vec<TeamRunEventRecord>) {
        let Some(archive) = self.message_archive.as_ref().cloned() else {
            return;
        };
        if events.is_empty() {
            return;
        }
        let manager = self.clone();

        tokio::spawn(async move {
            let mut documents = Vec::with_capacity(events.len());
            let mut run_scope_cache = HashMap::new();
            let mut task_conversation_cache = HashMap::new();
            for event in events {
                match team_run_event_archive_document_for_db_cached(
                    &manager.db,
                    &event,
                    &mut run_scope_cache,
                    &mut task_conversation_cache,
                )
                .await
                {
                    Ok(Some(document)) => documents.push(document),
                    Ok(None) => {}
                    Err(error) => {
                        tracing::warn!(
                            error = ?error,
                            run_id = %event.run_id,
                            event_id = event.event_id,
                            "failed to build team run event archive document"
                        );
                    }
                }
            }
            if documents.is_empty() {
                return;
            }

            let document_count = documents.len();
            match tokio::time::timeout(
                MESSAGE_ARCHIVE_APPEND_TIMEOUT,
                archive.append_documents(&documents),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::warn!(
                        error = ?error,
                        document_count,
                        "failed to dual-write team run events to archive"
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        document_count,
                        timeout_ms = MESSAGE_ARCHIVE_APPEND_TIMEOUT.as_millis(),
                        "timed out dual-writing team run events to archive"
                    );
                }
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn append_channel_replica_message(
        &self,
        authority_message_id: i64,
        correlation_id: &str,
        run_id: &str,
        team_id: &str,
        conversation_id: &str,
        task_id: &str,
        channel_id: &str,
        from_actor_id: &str,
        source_node_id: &str,
        payload: &Value,
    ) -> anyhow::Result<bool> {
        let stored_at = Utc::now().timestamp();
        let payload_json = redact_sensitive_json(payload).to_string();
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO team_channel_message_replicas (
                authority_message_id,
                correlation_id,
                run_id,
                team_id,
                conversation_id,
                task_id,
                channel_id,
                group_id,
                from_actor_id,
                source_node_id,
                payload_json,
                stored_at
            )
            VALUES (
                ?1,
                ?2,
                ?3,
                ?4,
                ?5,
                ?6,
                ?7,
                COALESCE(
                    (SELECT group_id FROM team_conversation_messages WHERE id = ?1),
                    (SELECT group_id FROM team_runs WHERE id = ?3)
                ),
                ?8,
                ?9,
                ?10,
                ?11
            )
            "#,
        )
        .bind(authority_message_id)
        .bind(correlation_id)
        .bind(run_id)
        .bind(team_id)
        .bind(conversation_id)
        .bind(task_id)
        .bind(channel_id)
        .bind(from_actor_id)
        .bind(source_node_id)
        .bind(payload_json)
        .bind(stored_at)
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}
