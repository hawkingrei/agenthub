use agenthub_team_actor::{
    AckActorMessageCommand, AckActorMessageResult, CreatePendingMessageResult,
    SendActorMessageCommand, normalize_actor_message_envelope_payload,
};

use super::codec_status::{team_actor_message_status_to_str, team_actor_message_transport_to_str};
use super::mailbox::{
    fetch_enriched_message_by_id, fetch_message_by_idempotency, fetch_message_for_actor,
    maybe_persist_human_visible_chat_reply,
};
use super::mailbox_store::{SqlActorMailboxStore, SqlActorMailboxStoreError};
use super::mailbox_threads::ensure_idempotency_compatible;
use crate::team::TeamActorMessageStatus;

impl SqlActorMailboxStore {
    pub(super) async fn create_pending_message_impl(
        &self,
        cmd: &SendActorMessageCommand,
    ) -> Result<CreatePendingMessageResult, SqlActorMailboxStoreError> {
        let normalized_payload = normalize_actor_message_envelope_payload(
            &cmd.from_actor_id,
            &cmd.to_actor_id,
            &cmd.message_kind,
            cmd.payload.clone(),
        );
        let normalized_cmd = SendActorMessageCommand {
            payload: normalized_payload,
            ..cmd.clone()
        };
        let route_json = cmd
            .route
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|err| sqlx::Error::Protocol(err.to_string()))?;
        let payload_json = serde_json::to_string(&normalized_cmd.payload)
            .map_err(|err| sqlx::Error::Protocol(err.to_string()))?;
        let transport_raw = team_actor_message_transport_to_str(&cmd.transport);
        let status_raw = team_actor_message_status_to_str(&TeamActorMessageStatus::Pending);

        let mut tx = self.db.begin().await?;
        let inserted = sqlx::query(
            r#"
            INSERT OR IGNORE INTO team_actor_messages (
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
                group_id,
                status,
                created_at,
                idempotency_key
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, (SELECT group_id FROM team_runs WHERE id = ?1), ?11, ?12, ?13)
            "#,
        )
        .bind(&cmd.run_id)
        .bind(&cmd.from_actor_id)
        .bind(&cmd.from_peer_id)
        .bind(&cmd.to_actor_id)
        .bind(&cmd.to_peer_id)
        .bind(&cmd.channel)
        .bind(transport_raw)
        .bind(route_json)
        .bind(payload_json)
        .bind(cmd.message_kind.as_str())
        .bind(status_raw)
        .bind(cmd.created_at)
        .bind(&cmd.idempotency_key)
        .execute(&mut *tx)
        .await?;

        let (message_id, created) = if inserted.rows_affected() == 1 {
            let message_id = inserted.last_insert_rowid();
            maybe_persist_human_visible_chat_reply(&mut tx, &normalized_cmd).await?;
            (message_id, true)
        } else if let Some(idempotency_key) = cmd.idempotency_key.as_deref() {
            let message = fetch_message_by_idempotency(
                &mut tx,
                &cmd.run_id,
                &cmd.from_actor_id,
                &cmd.from_peer_id,
                idempotency_key,
            )
            .await?;
            ensure_idempotency_compatible(&normalized_cmd, &message)?;
            (message.message_id, false)
        } else {
            return Err(sqlx::Error::Protocol(
                "insert was ignored without idempotency_key".to_string(),
            )
            .into());
        };
        tx.commit().await?;
        let message = fetch_enriched_message_by_id(&self.db, message_id).await?;
        Ok(CreatePendingMessageResult { message, created })
    }

    pub(super) async fn ack_message_impl(
        &self,
        cmd: &AckActorMessageCommand,
    ) -> Result<AckActorMessageResult, SqlActorMailboxStoreError> {
        let mut tx = self.db.begin().await?;
        let current = fetch_message_for_actor(
            &mut tx,
            &cmd.run_id,
            &cmd.actor_id,
            &cmd.peer_id,
            cmd.message_id,
        )
        .await?;
        let status_changed = if current.status == TeamActorMessageStatus::Pending {
            sqlx::query(
                r#"
                UPDATE team_actor_messages
                SET status = 'delivered', delivered_at = COALESCE(delivered_at, ?1)
                WHERE id = ?2 AND run_id = ?3 AND to_actor_id = ?4 AND to_peer_id = ?5 AND status = 'pending'
                "#,
            )
            .bind(cmd.delivered_at)
            .bind(cmd.message_id)
            .bind(&cmd.run_id)
            .bind(&cmd.actor_id)
            .bind(&cmd.peer_id)
            .execute(&mut *tx)
            .await?
            .rows_affected()
                > 0
        } else {
            false
        };

        tx.commit().await?;
        let message = fetch_enriched_message_by_id(&self.db, cmd.message_id).await?;
        Ok(AckActorMessageResult {
            message,
            status_changed,
        })
    }
}
