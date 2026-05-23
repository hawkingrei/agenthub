use chrono::Utc;

use super::mailbox::{
    MAILBOX_RESOLUTION_ESCALATED, apply_thread_claim_transition, fetch_enriched_message_by_id,
    fetch_message_for_actor, map_actor_service_error,
};
use super::mailbox_service::TeamActorMailboxService;
use crate::team::{TeamActorMessageRecord, TeamActorMessageStatus};
use agenthub_team_actor::{
    ActorMessageHandlingDisposition, TriageActorMessageCommand,
    normalize_actor_message_envelope_payload,
};

impl TeamActorMailboxService {
    pub async fn escalate_reply_required_message_to_coordinator(
        &self,
        run_id: &str,
        actor_id: &str,
        peer_id: &str,
        message_id: i64,
    ) -> Result<TeamActorMessageRecord, agenthub_team_actor::ActorServiceError> {
        let member_specs = self
            .load_member_specs_for_run(run_id)
            .await
            .map_err(map_actor_service_error)?;
        let coordinator_actor_id = member_specs
            .iter()
            .find(|member| member.role.eq_ignore_ascii_case("coordinator"))
            .map(|member| member.member_id.clone())
            .ok_or_else(|| {
                map_actor_service_error(anyhow::Error::new(
                    super::mailbox::SqlActorMailboxStoreError::ReplyRequiredEscalationTargetUnavailable,
                ))
            })?;
        if coordinator_actor_id == actor_id {
            return Err(map_actor_service_error(anyhow::Error::new(
                super::mailbox::SqlActorMailboxStoreError::ReplyRequiredEscalationAlreadyAtCoordinator,
            )));
        }

        let delivery = self
            .manager
            .resolve_channel_recipient_deliveries(std::slice::from_ref(&coordinator_actor_id))
            .await
            .map_err(map_actor_service_error)?
            .into_iter()
            .next()
            .ok_or_else(|| {
                map_actor_service_error(anyhow::Error::new(
                    super::mailbox::SqlActorMailboxStoreError::ReplyRequiredEscalationTargetUnavailable,
                ))
            })?;

        let now = Utc::now().timestamp();
        let route_json = delivery
            .route
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|err| {
                agenthub_team_actor::ActorServiceError::new(
                    agenthub_team_actor::ActorServiceErrorCode::Internal,
                    err.to_string(),
                )
            })?;
        let transport_raw =
            super::codec_status::team_actor_message_transport_to_str(&delivery.transport);
        let status_raw =
            super::codec_status::team_actor_message_status_to_str(&TeamActorMessageStatus::Pending);

        let mut tx = self
            .manager
            .db
            .begin()
            .await
            .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?;
        let message = fetch_message_for_actor(&mut tx, run_id, actor_id, peer_id, message_id)
            .await
            .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?;
        if super::mailbox_reply_obligations::reply_actor_pair_for_inbound_obligation(&message)
            .is_none()
        {
            return Err(map_actor_service_error(anyhow::Error::new(
                super::mailbox::SqlActorMailboxStoreError::ReplyRequiredEscalationUnsupported,
            )));
        }
        if super::mailbox_reply_obligations::reply_obligation_is_terminal(&message) {
            return Err(agenthub_team_actor::ActorServiceError::new(
                agenthub_team_actor::ActorServiceErrorCode::BadRequest,
                "mailbox work is already in a terminal state",
            ));
        }

        let release_payload = super::mailbox_reply_obligations::build_mailbox_resolution_payload(
            &message.payload,
            MAILBOX_RESOLUTION_ESCALATED,
            actor_id,
            &coordinator_actor_id,
            now,
        );
        let released_payload_json = serde_json::to_string(&release_payload).map_err(|err| {
            agenthub_team_actor::ActorServiceError::new(
                agenthub_team_actor::ActorServiceErrorCode::Internal,
                err.to_string(),
            )
        })?;
        let escalated_payload = normalize_actor_message_envelope_payload(
            &message.from_actor_id,
            &coordinator_actor_id,
            &message.message_kind,
            super::mailbox_reply_obligations::build_escalated_mailbox_payload(
                &message.payload,
                message.message_id,
                actor_id,
                &coordinator_actor_id,
                actor_id,
                now,
            ),
        );
        let escalated_payload_json = serde_json::to_string(&escalated_payload).map_err(|err| {
            agenthub_team_actor::ActorServiceError::new(
                agenthub_team_actor::ActorServiceErrorCode::Internal,
                err.to_string(),
            )
        })?;

        if message.handling_disposition == ActorMessageHandlingDisposition::Claimed {
            apply_thread_claim_transition(
                &mut tx,
                &message,
                &TriageActorMessageCommand {
                    run_id: run_id.to_string(),
                    actor_id: actor_id.to_string(),
                    peer_id: peer_id.to_string(),
                    message_id,
                    disposition: ActorMessageHandlingDisposition::Released,
                    handled_at: now,
                },
            )
            .await
            .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?;
        }

        sqlx::query(
            r#"
            UPDATE team_actor_messages
            SET
                handling_disposition = 'released',
                handled_by_actor_id = ?1,
                handled_at = ?2,
                payload_json = ?3
            WHERE id = ?4
              AND run_id = ?5
              AND to_actor_id = ?6
              AND to_peer_id = ?7
            "#,
        )
        .bind(actor_id)
        .bind(now)
        .bind(&released_payload_json)
        .bind(message_id)
        .bind(run_id)
        .bind(actor_id)
        .bind(peer_id)
        .execute(&mut *tx)
        .await
        .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?;

        let inserted = sqlx::query(
            r#"
            INSERT INTO team_actor_messages (
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
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, (SELECT group_id FROM team_runs WHERE id = ?1), ?11, ?12, NULL)
            "#,
        )
        .bind(run_id)
        .bind(&message.from_actor_id)
        .bind(&message.from_peer_id)
        .bind(&coordinator_actor_id)
        .bind(&delivery.to_peer_id)
        .bind(&message.channel)
        .bind(transport_raw)
        .bind(route_json)
        .bind(escalated_payload_json)
        .bind(message.message_kind.as_str())
        .bind(status_raw)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?;
        let escalated_message_id = inserted.last_insert_rowid();

        tx.commit()
            .await
            .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?;
        fetch_enriched_message_by_id(&self.manager.db, escalated_message_id)
            .await
            .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))
    }
}
