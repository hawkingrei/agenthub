use chrono::Utc;

use super::mailbox::{
    MAILBOX_RESOLUTION_ESCALATED, MAILBOX_RESOLUTION_TAKEN_OVER, MAILBOX_RESOLUTION_TRANSFERRED,
    apply_thread_claim_takeover, apply_thread_claim_transition, fetch_enriched_message_by_id,
    fetch_message_for_actor, map_actor_service_error,
};
use super::mailbox_service::TeamActorMailboxService;
use crate::team::{TeamActorMessageRecord, TeamActorMessageStatus};
use agenthub_team_actor::{
    ActorMessageHandlingDisposition, TriageActorMessageCommand,
    normalize_actor_message_envelope_payload,
};

impl TeamActorMailboxService {
    pub async fn transfer_reply_required_message(
        &self,
        run_id: &str,
        actor_id: &str,
        peer_id: &str,
        message_id: i64,
        target_actor_id: &str,
    ) -> Result<TeamActorMessageRecord, agenthub_team_actor::ActorServiceError> {
        let member_specs = self
            .load_member_specs_for_run(run_id)
            .await
            .map_err(map_actor_service_error)?;
        let target_actor_id = target_actor_id.trim();
        if target_actor_id.is_empty() {
            return Err(agenthub_team_actor::ActorServiceError::new(
                agenthub_team_actor::ActorServiceErrorCode::BadRequest,
                "target_actor_id is required",
            ));
        }
        if target_actor_id == actor_id {
            return Err(agenthub_team_actor::ActorServiceError::new(
                agenthub_team_actor::ActorServiceErrorCode::BadRequest,
                "reply-required mailbox work cannot be transferred to the acting actor",
            ));
        }
        if !member_specs
            .iter()
            .any(|member| member.member_id == target_actor_id)
        {
            let valid_member_ids = member_specs
                .iter()
                .map(|member| member.member_id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(agenthub_team_actor::ActorServiceError::new(
                agenthub_team_actor::ActorServiceErrorCode::BadRequest,
                format!(
                    "target_actor_id `{}` must reference a canonical team member_id; valid member_ids: {}",
                    target_actor_id, valid_member_ids
                ),
            ));
        }
        self.reassign_reply_required_message(
            run_id,
            actor_id,
            peer_id,
            message_id,
            target_actor_id,
            MAILBOX_RESOLUTION_TRANSFERRED,
        )
        .await
    }

    pub async fn takeover_reply_required_message(
        &self,
        run_id: &str,
        actor_id: &str,
        peer_id: &str,
        message_id: i64,
        target_actor_id: &str,
    ) -> Result<TeamActorMessageRecord, agenthub_team_actor::ActorServiceError> {
        let member_specs = self
            .load_member_specs_for_run(run_id)
            .await
            .map_err(map_actor_service_error)?;
        let target_actor_id = target_actor_id.trim();
        if target_actor_id.is_empty() {
            return Err(agenthub_team_actor::ActorServiceError::new(
                agenthub_team_actor::ActorServiceErrorCode::BadRequest,
                "target_actor_id is required",
            ));
        }
        if target_actor_id == actor_id {
            return Err(agenthub_team_actor::ActorServiceError::new(
                agenthub_team_actor::ActorServiceErrorCode::BadRequest,
                "reply-required mailbox work cannot be taken over by the acting actor",
            ));
        }
        if !member_specs
            .iter()
            .any(|member| member.member_id == target_actor_id)
        {
            let valid_member_ids = member_specs
                .iter()
                .map(|member| member.member_id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(agenthub_team_actor::ActorServiceError::new(
                agenthub_team_actor::ActorServiceErrorCode::BadRequest,
                format!(
                    "target_actor_id `{}` must reference a canonical team member_id; valid member_ids: {}",
                    target_actor_id, valid_member_ids
                ),
            ));
        }
        self.reassign_reply_required_message(
            run_id,
            actor_id,
            peer_id,
            message_id,
            target_actor_id,
            MAILBOX_RESOLUTION_TAKEN_OVER,
        )
        .await
    }

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

        self.reassign_reply_required_message(
            run_id,
            actor_id,
            peer_id,
            message_id,
            &coordinator_actor_id,
            MAILBOX_RESOLUTION_ESCALATED,
        )
        .await
    }

    async fn reassign_reply_required_message(
        &self,
        run_id: &str,
        actor_id: &str,
        peer_id: &str,
        message_id: i64,
        target_actor_id: &str,
        resolution_kind: &str,
    ) -> Result<TeamActorMessageRecord, agenthub_team_actor::ActorServiceError> {
        let target_actor_id_owned = target_actor_id.to_string();
        let delivery = self
            .manager
            .resolve_mailbox_recipient_deliveries(std::slice::from_ref(&target_actor_id_owned))
            .await
            .map_err(map_actor_service_error)?
            .into_iter()
            .next()
            .ok_or_else(|| {
                agenthub_team_actor::ActorServiceError::new(
                    agenthub_team_actor::ActorServiceErrorCode::BadRequest,
                    format!(
                        "run team cannot resolve mailbox delivery target for actor `{target_actor_id}`"
                    ),
                )
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
            return Err(agenthub_team_actor::ActorServiceError::new(
                agenthub_team_actor::ActorServiceErrorCode::BadRequest,
                "only human-originated reply-required mailbox work can be escalated, transferred, or taken over",
            ));
        }
        if super::mailbox_reply_obligations::reply_obligation_is_terminal(&message) {
            return Err(agenthub_team_actor::ActorServiceError::new(
                agenthub_team_actor::ActorServiceErrorCode::BadRequest,
                "mailbox work is already in a terminal state",
            ));
        }

        let release_payload = super::mailbox_reply_obligations::build_mailbox_resolution_payload(
            &message.payload,
            resolution_kind,
            actor_id,
            target_actor_id,
            now,
        );
        let released_payload_json = serde_json::to_string(&release_payload).map_err(|err| {
            agenthub_team_actor::ActorServiceError::new(
                agenthub_team_actor::ActorServiceErrorCode::Internal,
                err.to_string(),
            )
        })?;
        let reassigned_payload = normalize_actor_message_envelope_payload(
            &message.from_actor_id,
            target_actor_id,
            &message.message_kind,
            match resolution_kind {
                MAILBOX_RESOLUTION_ESCALATED => {
                    super::mailbox_reply_obligations::build_escalated_mailbox_payload(
                        &message.payload,
                        message.message_id,
                        actor_id,
                        target_actor_id,
                        actor_id,
                        now,
                    )
                }
                MAILBOX_RESOLUTION_TRANSFERRED => {
                    super::mailbox_reply_obligations::build_transferred_mailbox_payload(
                        &message.payload,
                        message.message_id,
                        actor_id,
                        target_actor_id,
                        actor_id,
                        now,
                    )
                }
                MAILBOX_RESOLUTION_TAKEN_OVER => {
                    super::mailbox_reply_obligations::build_taken_over_mailbox_payload(
                        &message.payload,
                        message.message_id,
                        actor_id,
                        target_actor_id,
                        target_actor_id,
                        now,
                    )
                }
                _ => unreachable!(),
            },
        );
        let reassigned_payload_json =
            serde_json::to_string(&reassigned_payload).map_err(|err| {
                agenthub_team_actor::ActorServiceError::new(
                    agenthub_team_actor::ActorServiceErrorCode::Internal,
                    err.to_string(),
                )
            })?;

        if message.handling_disposition == ActorMessageHandlingDisposition::Claimed
            && resolution_kind != MAILBOX_RESOLUTION_TAKEN_OVER
        {
            apply_thread_claim_transition(
                &mut tx,
                &message,
                &TriageActorMessageCommand {
                    run_id: run_id.to_string(),
                    actor_id: actor_id.to_string(),
                    peer_id: peer_id.to_string(),
                    message_id,
                    disposition: ActorMessageHandlingDisposition::Released,
                    reason: None,
                    handled_at: now,
                },
            )
            .await
            .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?;
        }

        let released = sqlx::query(
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
              AND handling_disposition = ?8
            "#,
        )
        .bind(actor_id)
        .bind(now)
        .bind(&released_payload_json)
        .bind(message_id)
        .bind(run_id)
        .bind(actor_id)
        .bind(peer_id)
        .bind(message.handling_disposition.as_str())
        .execute(&mut *tx)
        .await
        .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?;
        if released.rows_affected() == 0 {
            return Err(agenthub_team_actor::ActorServiceError::new(
                agenthub_team_actor::ActorServiceErrorCode::Conflict,
                "mailbox work was already escalated, transferred, or taken over by a concurrent request",
            ));
        }

        let reassign_idempotency_key = format!("mailbox-reassign:{message_id}");
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
        .bind(run_id)
        .bind(&message.from_actor_id)
        .bind(&message.from_peer_id)
        .bind(target_actor_id)
        .bind(&delivery.to_peer_id)
        .bind(&message.channel)
        .bind(transport_raw)
        .bind(route_json)
        .bind(reassigned_payload_json)
        .bind(message.message_kind.as_str())
        .bind(status_raw)
        .bind(now)
        .bind(&reassign_idempotency_key)
        .execute(&mut *tx)
        .await
        .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?;
        let reassigned_message_id = if inserted.rows_affected() == 1 {
            inserted.last_insert_rowid()
        } else {
            super::mailbox::fetch_message_by_idempotency(
                &mut tx,
                run_id,
                &message.from_actor_id,
                &message.from_peer_id,
                &reassign_idempotency_key,
            )
            .await
            .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?
            .message_id
        };
        if resolution_kind == MAILBOX_RESOLUTION_TAKEN_OVER {
            sqlx::query(
                r#"
                UPDATE team_actor_messages
                SET
                    handling_disposition = 'claimed',
                    handled_by_actor_id = ?1,
                    handled_at = ?2
                WHERE id = ?3
                  AND run_id = ?4
                  AND to_actor_id = ?1
                "#,
            )
            .bind(target_actor_id)
            .bind(now)
            .bind(reassigned_message_id)
            .bind(run_id)
            .execute(&mut *tx)
            .await
            .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?;
            apply_thread_claim_takeover(
                &mut tx,
                &message,
                run_id,
                target_actor_id,
                reassigned_message_id,
                now,
            )
            .await
            .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?;
        }

        tx.commit()
            .await
            .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?;
        fetch_enriched_message_by_id(&self.manager.db, reassigned_message_id)
            .await
            .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))
    }
}
