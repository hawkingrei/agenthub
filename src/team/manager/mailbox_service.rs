use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;

use super::mailbox::{
    MAILBOX_RESOLUTION_ESCALATED, SendActorMessageInput, SqlActorMailboxStore,
    apply_thread_claim_transition, build_channel_mailbox_forward_payload,
    fetch_enriched_message_by_id, fetch_message_for_actor, map_actor_service_error,
    optional_trimmed, required_trimmed_field, validate_direct_mailbox_target_for_member_specs,
};
use super::mailbox_payloads::{
    channel_payload_correlation_id, ensure_channel_message_correlation_id,
    normalize_channel_message_payload,
};
use super::{TeamManager, TeamMemberSpecView, parse_team_member_specs};
use crate::team::{TeamActorMessageRecord, TeamActorMessageStatus, TeamActorMessageTransport};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ActorAckRequest, ActorAckResponse, ActorInboxRequest, ActorInboxResponse,
    ActorMailboxService, ActorMessageHandlingDisposition, ActorMessageKind, ActorSendRequest,
    ActorSendResponse, ActorServiceError, ActorServiceErrorCode, ActorTaskLinkRequest,
    ActorTaskLinkResponse, ActorTriageRequest, ActorTriageResponse, ListActorInboxQuery,
    TriageActorMessageCommand, normalize_actor_message_envelope_payload,
};

#[derive(Clone)]
pub struct TeamActorMailboxService {
    manager: TeamManager,
}

impl TeamActorMailboxService {
    pub fn new(manager: TeamManager) -> Self {
        Self { manager }
    }

    async fn validate_direct_send_target(
        &self,
        run_id: &str,
        to_actor_id: &str,
    ) -> Result<(), ActorServiceError> {
        let member_specs = self
            .load_member_specs_for_run(run_id)
            .await
            .map_err(map_actor_service_error)?;
        validate_direct_mailbox_target_for_member_specs(&member_specs, to_actor_id)
    }

    fn validate_direct_remote_route(route: Option<&Value>) -> Result<(), ActorServiceError> {
        let Some(route) = route else {
            return Err(ActorServiceError::new(
                ActorServiceErrorCode::BadRequest,
                "route is required for remote transport",
            ));
        };
        let Some(object) = route.as_object() else {
            return Err(ActorServiceError::new(
                ActorServiceErrorCode::BadRequest,
                "route must be a JSON object for remote transport",
            ));
        };
        let has_http_route = object
            .get("endpoint")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());
        let has_grpc_route = object
            .get("grpc_target")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());
        if has_http_route || has_grpc_route {
            return Ok(());
        }
        Err(ActorServiceError::new(
            ActorServiceErrorCode::BadRequest,
            "route must contain endpoint or grpc_target for remote transport",
        ))
    }

    async fn load_member_specs_for_run(
        &self,
        run_id: &str,
    ) -> anyhow::Result<Vec<TeamMemberSpecView>> {
        let team_id =
            sqlx::query_scalar::<_, String>("SELECT team_id FROM team_runs WHERE id = ?1")
                .bind(run_id)
                .fetch_optional(&self.manager.db)
                .await?
                .ok_or_else(|| anyhow::anyhow!("run not found"))?;
        let team = self.manager.get_team(&team_id).await?;
        parse_team_member_specs(&team.spec)
    }

    pub async fn escalate_reply_required_message_to_coordinator(
        &self,
        run_id: &str,
        actor_id: &str,
        peer_id: &str,
        message_id: i64,
    ) -> Result<TeamActorMessageRecord, ActorServiceError> {
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
                ActorServiceError::new(ActorServiceErrorCode::Internal, err.to_string())
            })?;
        let transport_raw = super::codec::team_actor_message_transport_to_str(&delivery.transport);
        let status_raw =
            super::codec::team_actor_message_status_to_str(&TeamActorMessageStatus::Pending);

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
            return Err(ActorServiceError::new(
                ActorServiceErrorCode::BadRequest,
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
            ActorServiceError::new(ActorServiceErrorCode::Internal, err.to_string())
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
            ActorServiceError::new(ActorServiceErrorCode::Internal, err.to_string())
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

    #[allow(clippy::too_many_arguments)]
    async fn send_channel_message(
        &self,
        run_id: &str,
        from_actor_id: &str,
        from_peer_id: &str,
        channel_id: &str,
        channel: &str,
        message_kind: Option<ActorMessageKind>,
        payload: Value,
        idempotency_key: Option<&str>,
    ) -> anyhow::Result<ActorSendResponse> {
        let target = self
            .manager
            .resolve_channel_mailbox_target(run_id, channel_id, from_actor_id)
            .await?;
        if target.recipient_actor_ids.is_empty() {
            anyhow::bail!("channel '{}' has no recipient agents", channel_id);
        }
        let recipient_deliveries = self
            .manager
            .resolve_channel_recipient_deliveries(&target.recipient_actor_ids)
            .await?;

        let mention_actor_ids = self
            .manager
            .extract_channel_mention_actor_ids(run_id, &payload)
            .await?;
        let normalized_payload = normalize_channel_message_payload(payload);

        let base_idempotency_key = idempotency_key.map(str::to_string).unwrap_or_else(|| {
            agenthub_team_actor::build_default_actor_channel_idempotency_key(
                run_id,
                from_actor_id,
                from_peer_id,
                channel_id,
                channel,
                TeamActorMessageTransport::Local.as_str(),
                None,
                &normalized_payload,
            )
        });
        let canonical_payload = ensure_channel_message_correlation_id(
            normalized_payload,
            Some(base_idempotency_key.as_str()),
        );
        let (authority_message_id, source_payload) = if let Some(existing) = self
            .manager
            .find_channel_message_by_correlation_id(
                &target.conversation_id,
                from_actor_id,
                channel_payload_correlation_id(&canonical_payload)
                    .expect("canonical channel payload should carry correlation_id"),
            )
            .await?
        {
            (existing.message_id, existing.payload)
        } else {
            let canonical_message = self
                .manager
                .append_task_conversation_message(
                    &target.task_id,
                    from_actor_id,
                    None,
                    "group_chat",
                    canonical_payload.clone(),
                )
                .await?;
            (canonical_message.message_id, canonical_payload.clone())
        };

        let mut first_result = None;
        let mut any_created = false;
        for delivery in &recipient_deliveries {
            let forwarded_payload = build_channel_mailbox_forward_payload(
                &source_payload,
                &target,
                channel_id,
                authority_message_id,
                mention_actor_ids.as_slice(),
            );
            let fanout_idempotency_key =
                agenthub_team_actor::build_actor_channel_fanout_idempotency_key(
                    &base_idempotency_key,
                    delivery.actor_id.as_str(),
                );
            let result = self
                .manager
                .send_actor_message_with_created_kind(
                    SendActorMessageInput {
                        run_id,
                        from_actor_id,
                        from_peer_id,
                        to_actor_id: delivery.actor_id.as_str(),
                        to_peer_id: delivery.to_peer_id.as_str(),
                        channel,
                        transport: delivery.transport.clone(),
                        route: delivery.route.clone(),
                        payload: forwarded_payload,
                        message_kind: None,
                        idempotency_key: Some(fanout_idempotency_key.as_str()),
                    },
                    message_kind.clone(),
                )
                .await?;
            any_created |= result.1;
            if first_result.is_none() {
                first_result = Some(result);
            }
        }

        let (message, created) = first_result.expect("channel fanout should produce a message");
        Ok(ActorSendResponse {
            message_id: message.message_id,
            state: message.status.clone(),
            deduped: !created && !any_created,
            created_at: message.created_at,
            message,
        })
    }
}

#[async_trait]
impl ActorMailboxService for TeamActorMailboxService {
    async fn actor_send(
        &self,
        request: ActorSendRequest,
    ) -> Result<ActorSendResponse, ActorServiceError> {
        let run_id = required_trimmed_field(&request.run_id, "run_id")?;
        let from_actor_id = required_trimmed_field(&request.from_actor_id, "from_actor_id")?;
        let from_peer_id =
            optional_trimmed(request.from_peer_id.as_deref()).unwrap_or(ACTOR_MAIN_PEER_ID);
        let to_actor_id = optional_trimmed(request.to_actor_id.as_deref());
        let channel_id = optional_trimmed(request.channel_id.as_deref());
        let (to_actor_id, channel_id) = match (to_actor_id, channel_id) {
            (Some(to_actor_id), None) => (Some(to_actor_id), None),
            (None, Some(channel_id)) => (None, Some(channel_id)),
            (Some(_), Some(_)) => {
                return Err(ActorServiceError::new(
                    ActorServiceErrorCode::BadRequest,
                    "to_actor_id and channel_id cannot be used together",
                ));
            }
            (None, None) => {
                return Err(ActorServiceError::new(
                    ActorServiceErrorCode::BadRequest,
                    "to_actor_id or channel_id is required",
                ));
            }
        };
        let to_peer_id =
            optional_trimmed(request.to_peer_id.as_deref()).unwrap_or(ACTOR_MAIN_PEER_ID);
        let channel = request
            .channel
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("default");
        let transport = request
            .transport
            .unwrap_or(TeamActorMessageTransport::Local);
        let idempotency_key = optional_trimmed(request.idempotency_key.as_deref());

        if request.route.is_some() && channel_id.is_some() {
            return Err(ActorServiceError::new(
                ActorServiceErrorCode::BadRequest,
                "channel mailbox target does not support route",
            ));
        }
        if let Some(channel_id) = channel_id {
            return self
                .send_channel_message(
                    run_id,
                    from_actor_id,
                    from_peer_id,
                    channel_id,
                    channel,
                    request.message_kind,
                    request.payload,
                    idempotency_key,
                )
                .await
                .map_err(map_actor_service_error);
        }
        let to_actor_id = to_actor_id.expect("validated actor target");
        match transport {
            TeamActorMessageTransport::Local => {
                self.validate_direct_send_target(run_id, to_actor_id)
                    .await?;
            }
            TeamActorMessageTransport::Remote => {
                Self::validate_direct_remote_route(request.route.as_ref())?;
                if to_peer_id == ACTOR_MAIN_PEER_ID {
                    return Err(ActorServiceError::new(
                        ActorServiceErrorCode::BadRequest,
                        "to_peer_id must not be 'main' for remote transport",
                    ));
                }
            }
        }

        let (message, created) = self
            .manager
            .send_actor_message_with_created_kind(
                SendActorMessageInput {
                    run_id,
                    from_actor_id,
                    from_peer_id,
                    to_actor_id,
                    to_peer_id,
                    channel,
                    transport,
                    route: request.route,
                    payload: request.payload,
                    message_kind: None,
                    idempotency_key,
                },
                request.message_kind,
            )
            .await
            .map_err(map_actor_service_error)?;
        let message_id = message.message_id;
        let state = message.status.clone();
        let created_at = message.created_at;

        Ok(ActorSendResponse {
            message_id,
            state,
            deduped: !created,
            created_at,
            message,
        })
    }

    async fn actor_inbox(
        &self,
        request: ActorInboxRequest,
    ) -> Result<ActorInboxResponse, ActorServiceError> {
        let run_id = required_trimmed_field(&request.run_id, "run_id")?;
        let actor_id = required_trimmed_field(&request.actor_id, "actor_id")?;
        let limit = request.limit.unwrap_or(50).clamp(1, 1000);
        let include_delivered = request
            .states
            .as_ref()
            .is_some_and(|states| states.contains(&TeamActorMessageStatus::Delivered));
        let states = request
            .states
            .unwrap_or_else(|| vec![TeamActorMessageStatus::Pending]);
        let snapshot = SqlActorMailboxStore {
            db: self.manager.db.clone(),
            message_archive: None,
        }
        .read_inbox_snapshot(&ListActorInboxQuery {
            run_id: run_id.to_string(),
            actor_id: actor_id.to_string(),
            peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            limit,
            after_id: request.cursor,
            include_delivered,
        })
        .await
        .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?;
        let messages = snapshot
            .messages
            .into_iter()
            .filter(|message| states.contains(&message.status))
            .collect::<Vec<_>>();
        let next_cursor = messages.last().map(|message| message.message_id);

        Ok(ActorInboxResponse {
            messages,
            next_cursor,
            pending_count: snapshot.pending_count,
        })
    }

    async fn actor_ack(
        &self,
        request: ActorAckRequest,
    ) -> Result<ActorAckResponse, ActorServiceError> {
        let run_id = required_trimmed_field(&request.run_id, "run_id")?;
        let actor_id = required_trimmed_field(&request.actor_id, "actor_id")?;
        if request.message_id <= 0 {
            return Err(ActorServiceError::new(
                ActorServiceErrorCode::BadRequest,
                "message_id must be positive",
            ));
        }

        let result = self
            .manager
            .ack_actor_message(run_id, actor_id, request.message_id)
            .await
            .map_err(map_actor_service_error)?;
        let message = result.message;
        let state = message.status.clone();
        let acked_at = message.delivered_at.unwrap_or(message.created_at);

        Ok(ActorAckResponse {
            message_id: message.message_id,
            state,
            acked_at,
            status_changed: result.status_changed,
            message,
        })
    }

    async fn actor_triage(
        &self,
        request: ActorTriageRequest,
    ) -> Result<ActorTriageResponse, ActorServiceError> {
        let run_id = required_trimmed_field(&request.run_id, "run_id")?;
        let actor_id = required_trimmed_field(&request.actor_id, "actor_id")?;
        if request.message_id <= 0 {
            return Err(ActorServiceError::new(
                ActorServiceErrorCode::BadRequest,
                "message_id must be positive",
            ));
        }

        let result = self
            .manager
            .triage_actor_message(run_id, actor_id, request.message_id, request.disposition)
            .await
            .map_err(map_actor_service_error)?;
        let triaged_at = result.message.handled_at.unwrap_or(
            result
                .message
                .delivered_at
                .unwrap_or(result.message.created_at),
        );

        Ok(ActorTriageResponse {
            message_id: result.message.message_id,
            disposition: result.message.handling_disposition.clone(),
            triaged_at,
            handling_changed: result.handling_changed,
            message: result.message,
        })
    }

    async fn actor_task_link(
        &self,
        request: ActorTaskLinkRequest,
    ) -> Result<ActorTaskLinkResponse, ActorServiceError> {
        let run_id = required_trimmed_field(&request.run_id, "run_id")?;
        let actor_id = required_trimmed_field(&request.actor_id, "actor_id")?;
        let task_id = required_trimmed_field(&request.task_id, "task_id")?;
        if request.message_id <= 0 {
            return Err(ActorServiceError::new(
                ActorServiceErrorCode::BadRequest,
                "message_id must be positive",
            ));
        }

        let result = self
            .manager
            .link_actor_message_task(
                run_id,
                actor_id,
                request.message_id,
                task_id,
                request.relation,
            )
            .await
            .map_err(map_actor_service_error)?;
        let linked_at = result
            .message
            .handled_at
            .unwrap_or(result.message.created_at);

        Ok(ActorTaskLinkResponse {
            message_id: result.message.message_id,
            task_id: result.task_id,
            relation: result.relation,
            linked_at,
            created: result.created,
            message: result.message,
        })
    }
}
