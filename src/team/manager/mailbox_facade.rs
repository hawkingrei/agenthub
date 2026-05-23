use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, AckActorMessageResult, ActorMailbox, ActorMessageHandlingDisposition,
    ActorMessageKind, ActorMessageTaskRelation, LinkActorMessageTaskResult,
    RelayRemotePendingResult, SendActorMessageCommand, TriageActorMessageCommand,
    infer_actor_message_kind, normalize_actor_message_envelope_payload,
};
use chrono::Utc;
use sqlx::{QueryBuilder, Sqlite};

use super::mailbox::{SendActorMessageInput, SqlActorMailboxStore, map_actor_mailbox_store_error};
use super::mailbox_payloads::should_persist_human_visible_chat_reply_for_payload;
use super::{TeamConversationStreamEvent, TeamManager};
use crate::team::TeamActorMessageRecord;

impl TeamManager {
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn send_actor_message(
        &self,
        request: SendActorMessageInput<'_>,
    ) -> anyhow::Result<TeamActorMessageRecord> {
        let (message, _created) = self.send_actor_message_with_created(request).await?;
        Ok(message)
    }

    pub async fn send_actor_message_with_created(
        &self,
        request: SendActorMessageInput<'_>,
    ) -> anyhow::Result<(TeamActorMessageRecord, bool)> {
        self.send_actor_message_with_created_kind(request, None)
            .await
    }

    pub(super) async fn send_actor_message_with_created_kind(
        &self,
        request: SendActorMessageInput<'_>,
        explicit_message_kind: Option<ActorMessageKind>,
    ) -> anyhow::Result<(TeamActorMessageRecord, bool)> {
        let SendActorMessageInput {
            run_id,
            from_actor_id,
            from_peer_id,
            to_actor_id,
            to_peer_id,
            channel,
            transport,
            route,
            payload,
            message_kind,
            idempotency_key,
        } = request;
        let message_kind = infer_actor_message_kind(
            from_actor_id,
            &payload,
            explicit_message_kind.or(message_kind),
        );
        let normalized_payload = normalize_actor_message_envelope_payload(
            from_actor_id,
            to_actor_id,
            &message_kind,
            payload,
        );
        let should_emit_human_visible_reply = should_persist_human_visible_chat_reply_for_payload(
            &transport,
            to_actor_id,
            to_peer_id,
            from_actor_id,
            &normalized_payload,
        );
        let now = Utc::now().timestamp();
        let mailbox = self.actor_mailbox();
        let result = mailbox
            .send_with_result(SendActorMessageCommand {
                run_id: run_id.to_string(),
                from_actor_id: from_actor_id.to_string(),
                from_peer_id: from_peer_id.to_string(),
                to_actor_id: to_actor_id.to_string(),
                to_peer_id: to_peer_id.to_string(),
                channel: channel.to_string(),
                transport,
                message_kind,
                route,
                payload: normalized_payload,
                idempotency_key: idempotency_key.map(str::to_string),
                created_at: now,
            })
            .await
            .map_err(map_actor_mailbox_store_error)?;
        if result.created {
            self.spawn_archive_team_actor_message(&result.message);
        }
        if result.created
            && should_emit_human_visible_reply
            && let Some((team_id, task_id, conversation_id)) =
                self.shared_thread_target_for_run(run_id).await?
        {
            self.emit_conversation_event(TeamConversationStreamEvent {
                team_id,
                task_id,
                conversation_id,
                message_id: None,
                source: "canonical_chat_reply".to_string(),
            });
        }
        Ok((result.message, result.created))
    }

    #[cfg(test)]
    pub async fn list_actor_inbox(
        &self,
        run_id: &str,
        actor_id: &str,
        limit: i64,
        after_id: Option<i64>,
        include_delivered: bool,
    ) -> anyhow::Result<Vec<TeamActorMessageRecord>> {
        let mailbox = self.actor_mailbox();
        let messages = mailbox
            .list_inbox(agenthub_team_actor::ListActorInboxQuery {
                run_id: run_id.to_string(),
                actor_id: actor_id.to_string(),
                peer_id: ACTOR_MAIN_PEER_ID.to_string(),
                limit,
                after_id,
                include_delivered,
            })
            .await
            .map_err(map_actor_mailbox_store_error)?;
        Ok(messages)
    }

    pub async fn ack_actor_message(
        &self,
        run_id: &str,
        actor_id: &str,
        message_id: i64,
    ) -> anyhow::Result<AckActorMessageResult> {
        let now = Utc::now().timestamp();
        let mailbox = self.actor_mailbox();
        let result = mailbox
            .ack(agenthub_team_actor::AckActorMessageCommand {
                run_id: run_id.to_string(),
                actor_id: actor_id.to_string(),
                peer_id: ACTOR_MAIN_PEER_ID.to_string(),
                message_id,
                delivered_at: now,
            })
            .await
            .map_err(map_actor_mailbox_store_error)?;
        Ok(result)
    }

    pub async fn triage_actor_message(
        &self,
        run_id: &str,
        actor_id: &str,
        message_id: i64,
        disposition: ActorMessageHandlingDisposition,
    ) -> anyhow::Result<agenthub_team_actor::TriageActorMessageResult> {
        let now = Utc::now().timestamp();
        let mailbox = self.actor_mailbox();
        let result = mailbox
            .triage(TriageActorMessageCommand {
                run_id: run_id.to_string(),
                actor_id: actor_id.to_string(),
                peer_id: ACTOR_MAIN_PEER_ID.to_string(),
                message_id,
                disposition,
                handled_at: now,
            })
            .await
            .map_err(map_actor_mailbox_store_error)?;
        Ok(result)
    }

    pub async fn link_actor_message_task(
        &self,
        run_id: &str,
        actor_id: &str,
        message_id: i64,
        task_id: &str,
        relation: ActorMessageTaskRelation,
    ) -> anyhow::Result<LinkActorMessageTaskResult> {
        let now = Utc::now().timestamp();
        let mailbox = self.actor_mailbox();
        let result = mailbox
            .link_task(agenthub_team_actor::LinkActorMessageTaskCommand {
                run_id: run_id.to_string(),
                actor_id: actor_id.to_string(),
                peer_id: ACTOR_MAIN_PEER_ID.to_string(),
                message_id,
                task_id: task_id.to_string(),
                relation,
                linked_at: now,
            })
            .await
            .map_err(map_actor_mailbox_store_error)?;
        Ok(result)
    }

    #[allow(dead_code)]
    pub async fn has_pending_actor_message_payload_type(
        &self,
        run_id: &str,
        actor_id: &str,
        payload_type: &str,
        current_message_id: Option<i64>,
    ) -> anyhow::Result<bool> {
        let payload_type = payload_type.trim();
        if payload_type.is_empty() {
            return Ok(false);
        }

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT id
            FROM team_actor_messages
            WHERE run_id = "#,
        );
        builder.push_bind(run_id);
        builder.push(" AND to_actor_id = ");
        builder.push_bind(actor_id);
        builder.push(" AND to_peer_id = ");
        builder.push_bind(ACTOR_MAIN_PEER_ID);
        builder.push(" AND status = 'pending'");
        builder.push(" AND trim(json_extract(payload_json, '$.type')) = ");
        builder.push_bind(payload_type);
        if let Some(message_id) = current_message_id {
            builder.push(" AND id < ");
            builder.push_bind(message_id);
        }
        builder.push(" LIMIT 1");

        let row = builder.build().fetch_optional(&self.db).await?;
        Ok(row.is_some())
    }

    pub async fn relay_remote_messages_once(
        &self,
        limit: i64,
        max_attempts: i64,
        retry_delay_secs: i64,
    ) -> anyhow::Result<RelayRemotePendingResult> {
        let now = Utc::now().timestamp();
        let mailbox = self.actor_mailbox();
        let relay = self.remote_relay_adapter.as_ref();
        let result = mailbox
            .relay_remote_pending(
                relay,
                agenthub_team_actor::RelayRemotePendingCommand {
                    limit,
                    now,
                    max_attempts,
                    retry_delay_secs,
                },
            )
            .await
            .map_err(map_actor_mailbox_store_error)?;
        Ok(result)
    }

    fn actor_mailbox(&self) -> ActorMailbox<SqlActorMailboxStore> {
        ActorMailbox::new(SqlActorMailboxStore {
            db: self.db.clone(),
            message_archive: self.message_archive.clone(),
        })
    }
}
