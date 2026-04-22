use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, AckActorMessageCommand, AckActorMessageResult,
    ActorAckRequest, ActorAckResponse, ActorInboxRequest, ActorInboxResponse, ActorMailbox,
    ActorMailboxError, ActorMailboxService, ActorMailboxStore, ActorSendRequest, ActorSendResponse,
    ActorServiceError, ActorServiceErrorCode, CreatePendingMessageResult, ListActorInboxQuery,
    PendingRemoteRelayRecord, RelayRemotePendingCommand, RelayRemotePendingResult,
    SendActorMessageCommand, actor_message_fingerprint,
};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::{Map, Value};
use sqlx::sqlite::SqliteRow;
use sqlx::{Error as SqlxError, Executor, QueryBuilder, Row, Sqlite, SqlitePool};
use thiserror::Error;
use uuid::Uuid;

use super::codec::{
    parse_team_actor_message_row, team_actor_message_status_to_str,
    team_actor_message_transport_to_str,
};
use super::{
    TEAM_SHARED_THREAD_BOOTSTRAP_KIND, TEAM_SHARED_THREAD_TITLE, TeamConversationStreamEvent,
    TeamManager, TeamMemberSpecView, fetch_canonical_shared_thread_target, parse_team_member_specs,
    redact_sensitive_json,
};
use crate::agent::normalize_target_node_id;
use crate::team::{
    TeamActorMessageRecord, TeamActorMessageStatus, TeamActorMessageTransport,
    TeamConversationMessageRecord,
};

const TEAM_SHARED_THREAD_BOOTSTRAP_SOURCE: &str = "server_canonical_reply";
const TEAM_SPECIAL_USER_ACTOR_ALIAS: &str = "user";
const TEAM_SPECIAL_USER_ACTOR_PREFIX: &str = "user:";
const SQLITE_READONLY_BASE_CODE: i32 = 8;

#[derive(Debug, Clone)]
struct CanonicalChatReply {
    text: String,
    correlation_id: Option<String>,
}

#[derive(Debug, Clone)]
struct SharedThreadTarget {
    task_id: String,
    conversation_id: String,
}

#[derive(Debug, Clone)]
struct ResolvedChannelMailboxTarget {
    team_id: String,
    task_id: String,
    conversation_id: String,
    recipient_actor_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct ResolvedMailboxRecipientDelivery {
    actor_id: String,
    to_peer_id: String,
    transport: TeamActorMessageTransport,
    route: Option<Value>,
}

#[derive(Debug, Clone, Copy)]
pub struct TeamRemoteRelayWorkerSettings {
    pub poll_interval_secs: i64,
    pub batch_limit: i64,
    pub max_attempts: i64,
    pub retry_delay_secs: i64,
}

impl Default for TeamRemoteRelayWorkerSettings {
    fn default() -> Self {
        Self {
            poll_interval_secs: 5,
            batch_limit: 128,
            max_attempts: 5,
            retry_delay_secs: 15,
        }
    }
}

impl TeamManager {
    pub fn is_actor_message_idempotency_conflict(err: &anyhow::Error) -> bool {
        err.downcast_ref::<SqlActorMailboxStoreError>()
            .is_some_and(|cause| matches!(cause, SqlActorMailboxStoreError::IdempotencyConflict))
    }

    pub fn actor_mailbox_service(&self) -> TeamActorMailboxService {
        TeamActorMailboxService::new(self.clone())
    }

    pub fn spawn_remote_relay_worker(self: Arc<Self>, settings: TeamRemoteRelayWorkerSettings) {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(
                settings.poll_interval_secs.max(1) as u64,
            ));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                match self
                    .relay_remote_messages_once(
                        settings.batch_limit,
                        settings.max_attempts,
                        settings.retry_delay_secs,
                    )
                    .await
                {
                    Ok(summary) => {
                        if summary.scanned > 0 {
                            tracing::debug!(
                                scanned = summary.scanned,
                                delivered = summary.delivered,
                                retried = summary.retried,
                                dead_lettered = summary.dead_lettered,
                                "team relay worker tick"
                            );
                        }
                    }
                    Err(err) => {
                        tracing::warn!("team relay worker tick failed: {}", err);
                    }
                }
            }
        });
    }

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
            idempotency_key,
        } = request;
        let should_emit_human_visible_reply = should_persist_human_visible_chat_reply_for_payload(
            &transport,
            to_actor_id,
            to_peer_id,
            from_actor_id,
            &payload,
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
                route,
                payload,
                idempotency_key: idempotency_key.map(str::to_string),
                created_at: now,
            })
            .await
            .map_err(map_actor_mailbox_store_error)?;
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
            .list_inbox(ListActorInboxQuery {
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
            .ack(AckActorMessageCommand {
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
                RelayRemotePendingCommand {
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
        })
    }

    async fn resolve_channel_mailbox_target(
        &self,
        run_id: &str,
        channel_id: &str,
        from_actor_id: &str,
    ) -> anyhow::Result<ResolvedChannelMailboxTarget> {
        let trimmed_channel_id = channel_id.trim();
        if trimmed_channel_id.is_empty() {
            anyhow::bail!("channel_id must be a non-empty string");
        }

        let team_id =
            sqlx::query_scalar::<_, String>("SELECT team_id FROM team_runs WHERE id = ?1")
                .bind(run_id)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| anyhow::anyhow!("run not found"))?;

        let (task_id, conversation_id, mode) =
            if trimmed_channel_id.eq_ignore_ascii_case(TEAM_SHARED_THREAD_TITLE) {
                let (task_id, conversation_id) = self
                    .ensure_shared_thread_target_for_team(&team_id, from_actor_id)
                    .await?;
                (task_id, conversation_id, "group_chat".to_string())
            } else {
                let row = sqlx::query(
                    r#"
                SELECT task_id, id AS conversation_id, mode
                FROM team_conversations
                WHERE team_id = ?1 AND id = ?2
                LIMIT 1
                "#,
                )
                .bind(&team_id)
                .bind(trimmed_channel_id)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "channel '{}' not found for current team",
                        trimmed_channel_id
                    )
                })?;
                (
                    row.get::<String, _>("task_id"),
                    row.get::<String, _>("conversation_id"),
                    row.get::<String, _>("mode"),
                )
            };

        if mode.trim() != "group_chat" {
            anyhow::bail!(
                "channel '{}' is not a group_chat channel",
                trimmed_channel_id
            );
        }

        let team = self.get_team(&team_id).await?;
        let member_ids = parse_team_member_specs(&team.spec)?
            .into_iter()
            .map(|member| member.member_id)
            .filter(|member_id| member_id != from_actor_id)
            .collect::<Vec<_>>();

        Ok(ResolvedChannelMailboxTarget {
            team_id,
            task_id,
            conversation_id,
            recipient_actor_ids: member_ids,
        })
    }

    async fn extract_channel_mention_actor_ids(
        &self,
        run_id: &str,
        payload: &Value,
    ) -> anyhow::Result<Vec<String>> {
        let team_id =
            sqlx::query_scalar::<_, String>("SELECT team_id FROM team_runs WHERE id = ?1")
                .bind(run_id)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| anyhow::anyhow!("run not found"))?;
        let team = self.get_team(&team_id).await?;
        let member_ids = parse_team_member_specs(&team.spec)?
            .into_iter()
            .map(|member| member.member_id)
            .collect::<BTreeSet<_>>();
        Ok(collect_channel_mention_actor_ids(payload, &member_ids))
    }

    async fn has_agents_target_node_id_column(&self) -> anyhow::Result<bool> {
        if let Some(cached) = *self
            .agents_target_node_id_column
            .lock()
            .expect("lock agents target_node_id column cache")
        {
            return Ok(cached);
        }
        let rows = sqlx::query("PRAGMA table_info(agents)")
            .fetch_all(&self.db)
            .await?;
        let has_column = rows.into_iter().any(|row| {
            row.get::<String, _>("name")
                .trim()
                .eq_ignore_ascii_case("target_node_id")
        });
        *self
            .agents_target_node_id_column
            .lock()
            .expect("lock agents target_node_id column cache") = Some(has_column);
        Ok(has_column)
    }

    async fn resolve_channel_recipient_deliveries(
        &self,
        recipient_actor_ids: &[String],
    ) -> anyhow::Result<Vec<ResolvedMailboxRecipientDelivery>> {
        if recipient_actor_ids.is_empty() {
            return Ok(Vec::new());
        }
        if !self.has_agents_target_node_id_column().await? {
            return Ok(recipient_actor_ids
                .iter()
                .map(|actor_id| ResolvedMailboxRecipientDelivery {
                    actor_id: actor_id.clone(),
                    to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
                    transport: TeamActorMessageTransport::Local,
                    route: None,
                })
                .collect());
        }

        let mut builder =
            QueryBuilder::<Sqlite>::new("SELECT id, target_node_id FROM agents WHERE id IN (");
        let mut separated = builder.separated(", ");
        for actor_id in recipient_actor_ids {
            separated.push_bind(actor_id.as_str());
        }
        separated.push_unseparated(")");
        let rows = builder.build().fetch_all(&self.db).await?;
        let mut target_node_by_actor = std::collections::HashMap::with_capacity(rows.len());
        for row in rows {
            let actor_id: String = row.get("id");
            let target_node_id = normalize_target_node_id(
                row.try_get::<Option<String>, _>("target_node_id")
                    .ok()
                    .flatten()
                    .as_deref(),
            );
            target_node_by_actor.insert(actor_id, target_node_id);
        }

        let mut out = Vec::with_capacity(recipient_actor_ids.len());
        for actor_id in recipient_actor_ids {
            if let Some(target_node_id) = target_node_by_actor
                .get(actor_id.as_str())
                .and_then(|value| value.as_deref())
            {
                out.push(ResolvedMailboxRecipientDelivery {
                    actor_id: actor_id.clone(),
                    to_peer_id: ACTOR_NODE_PEER_ID.to_string(),
                    transport: TeamActorMessageTransport::Remote,
                    route: Some(
                        self.remote_relay_adapter
                            .build_registered_grpc_route_for_target_node(target_node_id)
                            .await?,
                    ),
                });
            } else {
                out.push(ResolvedMailboxRecipientDelivery {
                    actor_id: actor_id.clone(),
                    to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
                    transport: TeamActorMessageTransport::Local,
                    route: None,
                });
            }
        }
        Ok(out)
    }

    async fn find_channel_message_by_correlation_id(
        &self,
        conversation_id: &str,
        from_actor_id: &str,
        correlation_id: &str,
    ) -> anyhow::Result<Option<TeamConversationMessageRecord>> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                conversation_id,
                task_id,
                from_actor_id,
                to_actor_id,
                route,
                payload_json,
                created_at
            FROM team_conversation_messages
            WHERE conversation_id = ?1
              AND from_actor_id = ?2
              AND route = 'group_chat'
              AND trim(COALESCE(json_extract(payload_json, '$.correlation_id'), '')) = ?3
            ORDER BY id DESC
            LIMIT 1
            "#,
        )
        .bind(conversation_id)
        .bind(from_actor_id)
        .bind(correlation_id)
        .fetch_optional(&self.db)
        .await?;
        row.map(|row| {
            super::codec::parse_team_conversation_message_row(&row)
                .map_err(|err| anyhow::anyhow!(err.to_string()))
        })
        .transpose()
    }
}

pub struct SendActorMessageInput<'a> {
    pub run_id: &'a str,
    pub from_actor_id: &'a str,
    pub from_peer_id: &'a str,
    pub to_actor_id: &'a str,
    pub to_peer_id: &'a str,
    pub channel: &'a str,
    pub transport: TeamActorMessageTransport,
    pub route: Option<Value>,
    pub payload: Value,
    pub idempotency_key: Option<&'a str>,
}

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

    #[allow(clippy::too_many_arguments)]
    async fn send_channel_message(
        &self,
        run_id: &str,
        from_actor_id: &str,
        from_peer_id: &str,
        channel_id: &str,
        channel: &str,
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
                .send_actor_message_with_created(SendActorMessageInput {
                    run_id,
                    from_actor_id,
                    from_peer_id,
                    to_actor_id: delivery.actor_id.as_str(),
                    to_peer_id: delivery.to_peer_id.as_str(),
                    channel,
                    transport: delivery.transport.clone(),
                    route: delivery.route.clone(),
                    payload: forwarded_payload,
                    idempotency_key: Some(fanout_idempotency_key.as_str()),
                })
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
                    request.payload,
                    idempotency_key,
                )
                .await
                .map_err(map_actor_service_error);
        }
        let to_actor_id = to_actor_id.expect("validated actor target");
        if transport == TeamActorMessageTransport::Local {
            self.validate_direct_send_target(run_id, to_actor_id)
                .await?;
        }

        let (message, created) = self
            .manager
            .send_actor_message_with_created(SendActorMessageInput {
                run_id,
                from_actor_id,
                from_peer_id,
                to_actor_id,
                to_peer_id,
                channel,
                transport,
                route: request.route,
                payload: request.payload,
                idempotency_key,
            })
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
}

#[derive(Clone)]
struct SqlActorMailboxStore {
    db: SqlitePool,
}

#[derive(Debug, Error)]
enum SqlActorMailboxStoreError {
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error("actor message idempotency conflict")]
    IdempotencyConflict,
}

#[derive(Debug)]
struct ActorInboxSnapshot {
    messages: Vec<TeamActorMessageRecord>,
    pending_count: i64,
}

async fn count_pending_inbox_on_executor<'e, E>(
    executor: E,
    run_id: &str,
    actor_id: &str,
    peer_id: &str,
) -> Result<i64, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_actor_messages
        WHERE run_id = ?1
          AND to_actor_id = ?2
          AND status = 'pending'
          AND to_peer_id = ?3
        "#,
    )
    .bind(run_id)
    .bind(actor_id)
    .bind(peer_id)
    .fetch_one(executor)
    .await
}

async fn list_inbox_rows_on_executor<'e, E>(
    executor: E,
    query: &ListActorInboxQuery,
) -> Result<Vec<SqliteRow>, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    if query.include_delivered {
        if let Some(after_id) = query.after_id {
            sqlx::query(
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
                    status,
                    created_at,
                    delivered_at
                FROM team_actor_messages
                WHERE run_id = ?1 AND to_actor_id = ?2 AND to_peer_id = ?3 AND id > ?4
                ORDER BY id ASC
                LIMIT ?5
                "#,
            )
            .bind(&query.run_id)
            .bind(&query.actor_id)
            .bind(&query.peer_id)
            .bind(after_id)
            .bind(query.limit)
            .fetch_all(executor)
            .await
        } else {
            sqlx::query(
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
                    status,
                    created_at,
                    delivered_at
                FROM team_actor_messages
                WHERE run_id = ?1 AND to_actor_id = ?2 AND to_peer_id = ?3
                ORDER BY id ASC
                LIMIT ?4
                "#,
            )
            .bind(&query.run_id)
            .bind(&query.actor_id)
            .bind(&query.peer_id)
            .bind(query.limit)
            .fetch_all(executor)
            .await
        }
    } else if let Some(after_id) = query.after_id {
        sqlx::query(
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
                status,
                created_at,
                delivered_at
            FROM team_actor_messages
            WHERE run_id = ?1 AND to_actor_id = ?2 AND to_peer_id = ?3 AND status = 'pending' AND id > ?4
            ORDER BY id ASC
            LIMIT ?5
            "#,
        )
        .bind(&query.run_id)
        .bind(&query.actor_id)
        .bind(&query.peer_id)
        .bind(after_id)
        .bind(query.limit)
        .fetch_all(executor)
        .await
    } else {
        sqlx::query(
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
                status,
                created_at,
                delivered_at
            FROM team_actor_messages
            WHERE run_id = ?1 AND to_actor_id = ?2 AND to_peer_id = ?3 AND status = 'pending'
            ORDER BY id ASC
            LIMIT ?4
            "#,
        )
        .bind(&query.run_id)
        .bind(&query.actor_id)
        .bind(&query.peer_id)
        .bind(query.limit)
        .fetch_all(executor)
        .await
    }
}

fn parse_inbox_rows(rows: Vec<SqliteRow>) -> Result<Vec<TeamActorMessageRecord>, sqlx::Error> {
    let mut messages = Vec::with_capacity(rows.len());
    for row in rows {
        messages.push(
            parse_team_actor_message_row(&row)
                .map_err(|err| sqlx::Error::Protocol(err.to_string()))?,
        );
    }
    Ok(messages)
}

impl SqlActorMailboxStore {
    async fn read_inbox_snapshot(
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
        let rows = list_inbox_rows_on_executor(&mut *tx, query).await?;
        let messages = parse_inbox_rows(rows)?;
        tx.commit().await?;
        Ok(ActorInboxSnapshot {
            messages,
            pending_count,
        })
    }
}

#[async_trait]
impl ActorMailboxStore for SqlActorMailboxStore {
    type Error = SqlActorMailboxStoreError;

    async fn create_pending_message(
        &self,
        cmd: &SendActorMessageCommand,
    ) -> Result<CreatePendingMessageResult, Self::Error> {
        let route_json = cmd
            .route
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|err| sqlx::Error::Protocol(err.to_string()))?;
        let payload_json = serde_json::to_string(&cmd.payload)
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
                status,
                created_at,
                idempotency_key
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
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
        .bind(status_raw)
        .bind(cmd.created_at)
        .bind(&cmd.idempotency_key)
        .execute(&mut *tx)
        .await?;

        let (message, created) = if inserted.rows_affected() == 1 {
            let message_id = inserted.last_insert_rowid();
            maybe_persist_human_visible_chat_reply(&mut tx, cmd).await?;
            let message = fetch_message_by_id(&mut tx, message_id).await?;
            (message, true)
        } else if let Some(idempotency_key) = cmd.idempotency_key.as_deref() {
            let message = fetch_message_by_idempotency(
                &mut tx,
                &cmd.run_id,
                &cmd.from_actor_id,
                &cmd.from_peer_id,
                idempotency_key,
            )
            .await?;
            ensure_idempotency_compatible(cmd, &message)?;
            (message, false)
        } else {
            return Err(sqlx::Error::Protocol(
                "insert was ignored without idempotency_key".to_string(),
            )
            .into());
        };
        tx.commit().await?;
        Ok(CreatePendingMessageResult { message, created })
    }

    async fn list_inbox(
        &self,
        query: &ListActorInboxQuery,
    ) -> Result<Vec<TeamActorMessageRecord>, Self::Error> {
        let rows = list_inbox_rows_on_executor(&self.db, query).await?;
        parse_inbox_rows(rows).map_err(Into::into)
    }

    async fn ack_message(
        &self,
        cmd: &AckActorMessageCommand,
    ) -> Result<AckActorMessageResult, Self::Error> {
        let mut tx = self.db.begin().await?;
        let update = sqlx::query(
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
        .await?;

        let message_row = sqlx::query(
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
                status,
                created_at,
                delivered_at
            FROM team_actor_messages
            WHERE id = ?1 AND run_id = ?2 AND to_actor_id = ?3 AND to_peer_id = ?4
            "#,
        )
        .bind(cmd.message_id)
        .bind(&cmd.run_id)
        .bind(&cmd.actor_id)
        .bind(&cmd.peer_id)
        .fetch_one(&mut *tx)
        .await?;
        let message = parse_team_actor_message_row(&message_row)
            .map_err(|err| sqlx::Error::Protocol(err.to_string()))?;
        tx.commit().await?;
        Ok(AckActorMessageResult {
            message,
            status_changed: update.rows_affected() > 0,
        })
    }

    async fn list_remote_pending_messages(
        &self,
        limit: i64,
        now: i64,
    ) -> Result<Vec<PendingRemoteRelayRecord>, Self::Error> {
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

    async fn mark_remote_retry(
        &self,
        run_id: &str,
        message_id: i64,
        ts: i64,
        attempt: i64,
        next_retry_at: i64,
        error: &str,
    ) -> Result<(), Self::Error> {
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

    async fn mark_remote_dead_letter(
        &self,
        run_id: &str,
        message_id: i64,
        ts: i64,
        attempt: i64,
        error: &str,
    ) -> Result<(), Self::Error> {
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

    async fn append_run_event(
        &self,
        run_id: &str,
        event_type: &str,
        ts: i64,
        payload: Value,
    ) -> Result<(), Self::Error> {
        sqlx::query(
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
        Ok(())
    }
}

async fn maybe_persist_human_visible_chat_reply(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    cmd: &SendActorMessageCommand,
) -> Result<(), sqlx::Error> {
    if !should_persist_human_visible_chat_reply(cmd) {
        return Ok(());
    }
    let Some(reply) = resolve_canonical_chat_reply(&cmd.payload) else {
        return Ok(());
    };
    let shared_thread = resolve_or_create_shared_thread_for_run(tx, &cmd.run_id, cmd).await?;
    let payload = build_canonical_chat_payload(&reply);
    let payload_json = redact_sensitive_json(&payload).to_string();
    sqlx::query(
        r#"
        INSERT INTO team_conversation_messages (
            conversation_id,
            task_id,
            from_actor_id,
            to_actor_id,
            route,
            payload_json,
            created_at
        )
        VALUES (?1, ?2, ?3, NULL, 'group_chat', ?4, ?5)
        "#,
    )
    .bind(&shared_thread.conversation_id)
    .bind(&shared_thread.task_id)
    .bind(&cmd.from_actor_id)
    .bind(payload_json)
    .bind(cmd.created_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn should_persist_human_visible_chat_reply(cmd: &SendActorMessageCommand) -> bool {
    should_persist_human_visible_chat_reply_for_payload(
        &cmd.transport,
        &cmd.to_actor_id,
        &cmd.to_peer_id,
        &cmd.from_actor_id,
        &cmd.payload,
    )
}

pub(super) fn should_persist_human_visible_chat_reply_for_payload(
    transport: &agenthub_team_actor::ActorMessageTransport,
    to_actor_id: &str,
    to_peer_id: &str,
    from_actor_id: &str,
    payload: &Value,
) -> bool {
    *transport == agenthub_team_actor::ActorMessageTransport::Local
        && to_peer_id == ACTOR_MAIN_PEER_ID
        && is_human_actor_id(to_actor_id)
        && !is_human_actor_id(from_actor_id)
        && resolve_canonical_chat_reply(payload).is_some()
}

fn is_human_actor_id(actor_id: &str) -> bool {
    let trimmed = actor_id.trim();
    if trimmed == TEAM_SPECIAL_USER_ACTOR_ALIAS {
        return true;
    }
    if let Some(suffix) = trimmed.strip_prefix(TEAM_SPECIAL_USER_ACTOR_PREFIX) {
        return !suffix.trim().is_empty();
    }
    false
}

fn normalize_channel_message_payload(payload: Value) -> Value {
    let payload_obj = match payload {
        Value::Object(map) => map,
        Value::String(text) => {
            let mut payload_obj = Map::new();
            payload_obj.insert(
                "type".to_string(),
                Value::String("chat_message".to_string()),
            );
            payload_obj.insert("text".to_string(), Value::String(text));
            payload_obj
        }
        other => {
            let mut payload_obj = Map::new();
            payload_obj.insert(
                "type".to_string(),
                Value::String("chat_message".to_string()),
            );
            payload_obj.insert("text".to_string(), Value::String(other.to_string()));
            payload_obj
        }
    };
    Value::Object(payload_obj)
}

fn ensure_channel_message_correlation_id(
    payload: Value,
    fallback_correlation_id: Option<&str>,
) -> Value {
    let mut payload_obj = match normalize_channel_message_payload(payload) {
        Value::Object(map) => map,
        _ => unreachable!("channel payload normalization should always yield an object"),
    };
    let has_correlation_id = payload_obj
        .get("correlation_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    if !has_correlation_id {
        payload_obj.insert(
            "correlation_id".to_string(),
            Value::String(
                fallback_correlation_id
                    .map(str::to_string)
                    .unwrap_or_else(|| Uuid::new_v4().to_string()),
            ),
        );
    }
    Value::Object(payload_obj)
}

fn channel_payload_correlation_id(payload: &Value) -> Option<&str> {
    payload
        .get("correlation_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn build_channel_mailbox_forward_payload(
    source_payload: &Value,
    target: &ResolvedChannelMailboxTarget,
    channel_id: &str,
    authority_message_id: i64,
    mention_actor_ids: &[String],
) -> Value {
    let mut payload_obj = match source_payload {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    let mentions = mention_actor_ids
        .iter()
        .cloned()
        .map(Value::String)
        .collect::<Vec<_>>();
    payload_obj.insert(
        "channel_id".to_string(),
        Value::String(channel_id.to_string()),
    );
    payload_obj.insert("team_id".to_string(), Value::String(target.team_id.clone()));
    payload_obj.insert(
        "channel_conversation_id".to_string(),
        Value::String(target.conversation_id.clone()),
    );
    payload_obj.insert("task_id".to_string(), Value::String(target.task_id.clone()));
    payload_obj.insert(
        "authority_message_id".to_string(),
        Value::Number(authority_message_id.into()),
    );
    payload_obj.insert(
        "delivery_scope".to_string(),
        Value::String("channel_broadcast".to_string()),
    );
    payload_obj.insert(
        "mention_actor_ids".to_string(),
        Value::Array(mentions.clone()),
    );
    payload_obj.insert("mentioned_actor_ids".to_string(), Value::Array(mentions));
    Value::Object(payload_obj)
}

fn collect_channel_mention_actor_ids(
    payload: &Value,
    member_ids: &BTreeSet<String>,
) -> Vec<String> {
    let mut mentioned = BTreeSet::new();
    for key in ["mention_actor_ids", "mentioned_actor_ids"] {
        if let Some(explicit_mentions) = payload.get(key).and_then(Value::as_array) {
            for mention in explicit_mentions {
                if let Some(actor_id) = mention.as_str().map(str::trim)
                    && member_ids.contains(actor_id)
                {
                    mentioned.insert(actor_id.to_string());
                }
            }
        }
    }

    let text = match payload {
        Value::String(text) => Some(text.as_str()),
        Value::Object(map) => map.get("text").and_then(Value::as_str),
        _ => None,
    };
    if let Some(text) = text {
        for actor_id in member_ids {
            if find_raw_actor_mention_range(text, actor_id).is_some() {
                mentioned.insert(actor_id.clone());
            }
        }
    }

    mentioned.into_iter().collect()
}

fn is_valid_mention_char(raw: u8) -> bool {
    matches!(raw, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_')
}

fn is_email_local_char(raw: u8) -> bool {
    matches!(
        raw,
        b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'%' | b'+' | b'-'
    )
}

fn find_raw_actor_mention_range(text: &str, actor_id: &str) -> Option<(usize, usize)> {
    let needle = format!("@{actor_id}");
    let bytes = text.as_bytes();
    let mut cursor = 0usize;
    while let Some(found) = text[cursor..].find(needle.as_str()) {
        let start = cursor + found;
        let end = start + needle.len();
        let left_ok = start == 0 || !is_email_local_char(bytes[start - 1]);
        let right_ok = end >= bytes.len() || !is_valid_mention_char(bytes[end]);
        if left_ok && right_ok {
            return Some((start, end));
        }
        cursor = end;
    }
    None
}

fn resolve_canonical_chat_reply(payload: &Value) -> Option<CanonicalChatReply> {
    match payload {
        Value::Object(map) => resolve_canonical_chat_reply_from_map(map),
        Value::String(text) => {
            if let Some(parsed) = parse_stringified_json_payload(text)
                && let Some(reply) = resolve_canonical_chat_reply(&parsed)
            {
                return Some(reply);
            }
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(CanonicalChatReply {
                    text: text.clone(),
                    correlation_id: None,
                })
            }
        }
        _ => None,
    }
}

fn resolve_canonical_chat_reply_from_map(map: &Map<String, Value>) -> Option<CanonicalChatReply> {
    let payload_type = map
        .get("type")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if !payload_type.is_empty() && payload_type != "chat_message" {
        return None;
    }
    let text = map
        .get("text")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let correlation_id = map
        .get("correlation_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Some(CanonicalChatReply {
        text,
        correlation_id,
    })
}

fn parse_stringified_json_payload(raw: &str) -> Option<Value> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

fn build_canonical_chat_payload(reply: &CanonicalChatReply) -> Value {
    let mut payload = Map::new();
    payload.insert(
        "type".to_string(),
        Value::String("chat_message".to_string()),
    );
    payload.insert("text".to_string(), Value::String(reply.text.clone()));
    if let Some(correlation_id) = reply.correlation_id.as_deref() {
        payload.insert(
            "correlation_id".to_string(),
            Value::String(correlation_id.to_string()),
        );
    }
    Value::Object(payload)
}

async fn resolve_or_create_shared_thread_for_run(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
    cmd: &SendActorMessageCommand,
) -> Result<SharedThreadTarget, sqlx::Error> {
    let team_id = resolve_team_id_for_run(tx, run_id).await?;
    if let Some(existing) = fetch_shared_thread_for_team(tx, &team_id).await? {
        return Ok(existing);
    }

    let now = cmd.created_at;
    let task_id = Uuid::new_v4().to_string();
    let conversation_id = Uuid::new_v4().to_string();
    let context_json = serde_json::json!({
        "bootstrap_kind": TEAM_SHARED_THREAD_BOOTSTRAP_KIND,
        "bootstrap_source": TEAM_SHARED_THREAD_BOOTSTRAP_SOURCE,
    })
    .to_string();

    sqlx::query(
        r#"
        INSERT INTO team_tasks (
            id,
            team_id,
            title,
            status,
            created_by_actor_id,
            context_json,
            created_at,
            updated_at
        )
        VALUES (?1, ?2, ?3, 'open', ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&task_id)
    .bind(&team_id)
    .bind(TEAM_SHARED_THREAD_TITLE)
    .bind(&cmd.from_actor_id)
    .bind(context_json)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO team_conversations (
            id,
            team_id,
            task_id,
            mode,
            topic,
            created_at,
            updated_at
        )
        VALUES (?1, ?2, ?3, 'group_chat', ?4, ?5, ?6)
        "#,
    )
    .bind(&conversation_id)
    .bind(&team_id)
    .bind(&task_id)
    .bind(TEAM_SHARED_THREAD_TITLE)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    Ok(SharedThreadTarget {
        task_id,
        conversation_id,
    })
}

async fn resolve_team_id_for_run(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
) -> Result<String, sqlx::Error> {
    let row = sqlx::query("SELECT team_id FROM team_runs WHERE id = ?1")
        .bind(run_id)
        .fetch_one(&mut **tx)
        .await?;
    row.try_get("team_id")
}

async fn fetch_shared_thread_for_team(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    team_id: &str,
) -> Result<Option<SharedThreadTarget>, sqlx::Error> {
    Ok(fetch_canonical_shared_thread_target(&mut **tx, team_id)
        .await?
        .map(|target| SharedThreadTarget {
            task_id: target.task_id,
            conversation_id: target.conversation_id,
        }))
}

async fn fetch_message_by_id(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    message_id: i64,
) -> Result<TeamActorMessageRecord, sqlx::Error> {
    let row = sqlx::query(
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
            status,
            created_at,
            delivered_at
        FROM team_actor_messages
        WHERE id = ?1
        "#,
    )
    .bind(message_id)
    .fetch_one(&mut **tx)
    .await?;
    parse_team_actor_message_row(&row).map_err(|err| sqlx::Error::Protocol(err.to_string()))
}

async fn fetch_message_by_idempotency(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
    from_actor_id: &str,
    from_peer_id: &str,
    idempotency_key: &str,
) -> Result<TeamActorMessageRecord, sqlx::Error> {
    let row = sqlx::query(
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
            status,
            created_at,
            delivered_at
        FROM team_actor_messages
        WHERE run_id = ?1 AND from_actor_id = ?2 AND from_peer_id = ?3 AND idempotency_key = ?4
        LIMIT 1
        "#,
    )
    .bind(run_id)
    .bind(from_actor_id)
    .bind(from_peer_id)
    .bind(idempotency_key)
    .fetch_one(&mut **tx)
    .await?;
    parse_team_actor_message_row(&row).map_err(|err| sqlx::Error::Protocol(err.to_string()))
}

fn ensure_idempotency_compatible(
    cmd: &SendActorMessageCommand,
    existing: &TeamActorMessageRecord,
) -> Result<(), SqlActorMailboxStoreError> {
    let incoming_fp = actor_message_fingerprint(
        &cmd.run_id,
        &cmd.from_actor_id,
        &cmd.from_peer_id,
        &cmd.to_actor_id,
        &cmd.to_peer_id,
        &cmd.channel,
        cmd.transport.as_str(),
        cmd.route.as_ref(),
        &cmd.payload,
    );
    let existing_fp = actor_message_fingerprint(
        &existing.run_id,
        &existing.from_actor_id,
        &existing.from_peer_id,
        &existing.to_actor_id,
        &existing.to_peer_id,
        &existing.channel,
        existing.transport.as_str(),
        existing.route.as_ref(),
        &existing.payload,
    );
    if incoming_fp != existing_fp {
        return Err(SqlActorMailboxStoreError::IdempotencyConflict);
    }
    Ok(())
}

fn required_trimmed_field<'a>(
    value: &'a str,
    field: &'static str,
) -> Result<&'a str, ActorServiceError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ActorServiceError::new(
            ActorServiceErrorCode::BadRequest,
            format!("{field} is required"),
        ));
    }
    Ok(trimmed)
}

fn optional_trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|raw| !raw.is_empty())
}

fn validate_direct_mailbox_target_for_member_specs(
    member_specs: &[TeamMemberSpecView],
    to_actor_id: &str,
) -> Result<(), ActorServiceError> {
    let trimmed_target = to_actor_id.trim();
    if trimmed_target.is_empty() {
        return Err(ActorServiceError::new(
            ActorServiceErrorCode::BadRequest,
            "actor send target is required",
        ));
    }
    if is_human_actor_id(trimmed_target) {
        return Ok(());
    }
    if member_specs
        .iter()
        .any(|member| member.member_id == trimmed_target)
    {
        return Ok(());
    }
    let matching_role_member_ids = member_specs
        .iter()
        .filter(|member| member.role.eq_ignore_ascii_case(trimmed_target))
        .map(|member| member.member_id.as_str())
        .collect::<Vec<_>>();
    if matching_role_member_ids.len() == 1 {
        return Err(ActorServiceError::new(
            ActorServiceErrorCode::BadRequest,
            format!(
                "actor send target `{}` is not a canonical team member_id; use `{}` instead",
                trimmed_target, matching_role_member_ids[0]
            ),
        ));
    }
    if !matching_role_member_ids.is_empty() {
        return Err(ActorServiceError::new(
            ActorServiceErrorCode::BadRequest,
            format!(
                "actor send target `{}` matches multiple team members with role `{}`; matching member_ids: {}",
                trimmed_target,
                trimmed_target,
                matching_role_member_ids.join(", ")
            ),
        ));
    }
    let valid_member_ids = member_specs
        .iter()
        .map(|member| member.member_id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Err(ActorServiceError::new(
        ActorServiceErrorCode::BadRequest,
        format!(
            "actor send target `{}` must reference spec.members[].member_id or human mailbox `user` / `user:<id>`; valid member_ids: {}",
            trimmed_target, valid_member_ids
        ),
    ))
}

fn is_row_not_found(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<SqlxError>(),
        Some(SqlxError::RowNotFound)
    )
}

fn is_readonly_database_error(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        if let Some(SqlxError::Database(db_err)) = cause.downcast_ref::<SqlxError>()
            && let Some(code) = db_err.code().and_then(|raw| raw.parse::<i32>().ok())
            && (code & 0xff) == SQLITE_READONLY_BASE_CODE
        {
            return true;
        }

        cause
            .to_string()
            .contains("attempt to write a readonly database")
    })
}

fn map_actor_service_error(err: anyhow::Error) -> ActorServiceError {
    if TeamManager::is_actor_message_idempotency_conflict(&err) {
        return ActorServiceError::new(
            ActorServiceErrorCode::Conflict,
            "idempotency_key conflicts with an existing message payload",
        );
    }
    if is_row_not_found(&err) {
        return ActorServiceError::new(ActorServiceErrorCode::NotFound, "message not found");
    }
    if is_readonly_database_error(&err) {
        return ActorServiceError::new(
            ActorServiceErrorCode::Internal,
            "mailbox write failed: attempt to write a readonly database",
        );
    }
    ActorServiceError::new(
        ActorServiceErrorCode::Internal,
        "internal actor mailbox error",
    )
}

fn map_actor_mailbox_store_error(
    err: ActorMailboxError<SqlActorMailboxStoreError>,
) -> anyhow::Error {
    match err {
        ActorMailboxError::Store(store_err) => match store_err {
            SqlActorMailboxStoreError::Sql(sql_err) => anyhow::Error::new(sql_err),
            SqlActorMailboxStoreError::IdempotencyConflict => {
                anyhow::Error::new(SqlActorMailboxStoreError::IdempotencyConflict)
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn mock_member_specs(member_specs: &[(&str, &str)]) -> Vec<TeamMemberSpecView> {
        member_specs
            .iter()
            .map(|(member_id, role)| TeamMemberSpecView {
                member_id: (*member_id).to_string(),
                role: (*role).to_string(),
                description: None,
            })
            .collect()
    }

    #[test]
    fn normalize_channel_message_payload_wraps_non_object_inputs() {
        let text = normalize_channel_message_payload(Value::String("hello".to_string()));
        let number = normalize_channel_message_payload(json!(42));

        assert_eq!(
            text,
            json!({
                "type": "chat_message",
                "text": "hello"
            })
        );
        assert_eq!(
            number,
            json!({
                "type": "chat_message",
                "text": "42"
            })
        );
    }

    #[test]
    fn ensure_channel_message_correlation_id_preserves_existing_or_uses_fallback() {
        let existing = ensure_channel_message_correlation_id(
            json!({
                "type": "chat_message",
                "text": "hello",
                "correlation_id": "corr-existing"
            }),
            Some("corr-fallback"),
        );
        let fallback = ensure_channel_message_correlation_id(
            json!({
                "type": "chat_message",
                "text": "hello"
            }),
            Some("corr-fallback"),
        );

        assert_eq!(
            channel_payload_correlation_id(&existing),
            Some("corr-existing")
        );
        assert_eq!(
            channel_payload_correlation_id(&fallback),
            Some("corr-fallback")
        );
    }

    #[test]
    fn build_channel_mailbox_forward_payload_carries_metadata_and_mentions() {
        let payload = build_channel_mailbox_forward_payload(
            &json!({
                "type": "chat_message",
                "text": "@reviewer please inspect this",
                "correlation_id": "corr-1"
            }),
            &ResolvedChannelMailboxTarget {
                team_id: "team-1".to_string(),
                task_id: "task-1".to_string(),
                conversation_id: "conversation-1".to_string(),
                recipient_actor_ids: vec!["reviewer".to_string()],
            },
            "all",
            42,
            &["reviewer".to_string(), "worker-b".to_string()],
        );

        assert_eq!(
            payload.get("delivery_scope"),
            Some(&Value::String("channel_broadcast".to_string()))
        );
        assert_eq!(
            payload.get("team_id"),
            Some(&Value::String("team-1".to_string()))
        );
        assert_eq!(
            payload.get("channel_conversation_id"),
            Some(&Value::String("conversation-1".to_string()))
        );
        assert_eq!(
            payload.get("task_id"),
            Some(&Value::String("task-1".to_string()))
        );
        assert_eq!(payload.get("authority_message_id"), Some(&json!(42)));
        assert_eq!(
            payload.get("mentioned_actor_ids"),
            Some(&json!(["reviewer", "worker-b"]))
        );
        assert_eq!(channel_payload_correlation_id(&payload), Some("corr-1"));
    }

    #[test]
    fn map_actor_service_error_surfaces_readonly_database_failures() {
        let mapped =
            map_actor_service_error(anyhow::anyhow!("attempt to write a readonly database"));

        assert_eq!(mapped.code, ActorServiceErrorCode::Internal);
        assert_eq!(
            mapped.message,
            "mailbox write failed: attempt to write a readonly database"
        );
    }

    #[test]
    fn human_visible_chat_reply_requires_local_non_human_to_human_chat_message() {
        let payload = json!({
            "type": "chat_message",
            "text": "final answer"
        });

        assert!(should_persist_human_visible_chat_reply_for_payload(
            &agenthub_team_actor::ActorMessageTransport::Local,
            "user",
            ACTOR_MAIN_PEER_ID,
            "worker",
            &payload,
        ));
        assert!(!should_persist_human_visible_chat_reply_for_payload(
            &agenthub_team_actor::ActorMessageTransport::Remote,
            "user",
            ACTOR_MAIN_PEER_ID,
            "worker",
            &payload,
        ));
        assert!(!should_persist_human_visible_chat_reply_for_payload(
            &agenthub_team_actor::ActorMessageTransport::Local,
            "reviewer",
            ACTOR_MAIN_PEER_ID,
            "worker",
            &payload,
        ));
        assert!(!should_persist_human_visible_chat_reply_for_payload(
            &agenthub_team_actor::ActorMessageTransport::Local,
            "user:123",
            ACTOR_MAIN_PEER_ID,
            "user",
            &payload,
        ));
        assert!(!should_persist_human_visible_chat_reply_for_payload(
            &agenthub_team_actor::ActorMessageTransport::Local,
            "user",
            ACTOR_MAIN_PEER_ID,
            "worker",
            &json!({
                "type": "tool_result",
                "text": "not a chat"
            }),
        ));
    }

    #[test]
    fn resolve_canonical_chat_reply_accepts_stringified_json_chat_message() {
        let reply = resolve_canonical_chat_reply(&Value::String(
            r#"{"type":"chat_message","text":"hello","correlation_id":"corr-9"}"#.to_string(),
        ))
        .expect("resolve chat reply");

        assert_eq!(reply.text, "hello");
        assert_eq!(reply.correlation_id.as_deref(), Some("corr-9"));
    }

    #[test]
    fn validate_direct_mailbox_target_rejects_role_alias() {
        let member_specs = mock_member_specs(&[
            ("595d1ae8-fcbd-4111-b5c7-d446a12c044b", "leader"),
            ("c319f933-1358-4418-a111-872304052422", "worker"),
        ]);
        let err = validate_direct_mailbox_target_for_member_specs(&member_specs, "leader")
            .expect_err("role alias should be rejected");
        assert_eq!(err.code, ActorServiceErrorCode::BadRequest);
        assert!(err.message.contains("not a canonical team member_id"));
        assert!(err.message.contains("595d1ae8-fcbd-4111-b5c7-d446a12c044b"));
    }

    #[test]
    fn validate_direct_mailbox_target_allows_member_id_and_human_mailbox() {
        let member_specs = mock_member_specs(&[
            ("595d1ae8-fcbd-4111-b5c7-d446a12c044b", "leader"),
            ("c319f933-1358-4418-a111-872304052422", "worker"),
        ]);
        validate_direct_mailbox_target_for_member_specs(
            &member_specs,
            "595d1ae8-fcbd-4111-b5c7-d446a12c044b",
        )
        .expect("member id should be accepted");
        validate_direct_mailbox_target_for_member_specs(&member_specs, "user")
            .expect("human alias should be accepted");
        validate_direct_mailbox_target_for_member_specs(&member_specs, "user:test")
            .expect("human actor target should be accepted");
    }

    #[test]
    fn validate_direct_mailbox_target_rejects_empty_human_mailbox_suffix() {
        let member_specs = mock_member_specs(&[
            ("595d1ae8-fcbd-4111-b5c7-d446a12c044b", "leader"),
            ("c319f933-1358-4418-a111-872304052422", "worker"),
        ]);
        let err = validate_direct_mailbox_target_for_member_specs(&member_specs, "user:")
            .expect_err("human mailbox target with empty suffix should be rejected");
        assert_eq!(err.code, ActorServiceErrorCode::BadRequest);
        assert!(
            err.message
                .contains("must reference spec.members[].member_id")
        );
    }

    #[test]
    fn validate_direct_mailbox_target_rejects_ambiguous_role_alias() {
        let member_specs = mock_member_specs(&[
            ("planner", "leader"),
            ("worker-a", "worker"),
            ("worker-b", "worker"),
        ]);
        let err = validate_direct_mailbox_target_for_member_specs(&member_specs, "worker")
            .expect_err("ambiguous role alias should be rejected");
        assert_eq!(err.code, ActorServiceErrorCode::BadRequest);
        assert!(err.message.contains("matches multiple team members"));
        assert!(err.message.contains("worker-a"));
        assert!(err.message.contains("worker-b"));
    }
}
