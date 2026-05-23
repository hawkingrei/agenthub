use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, AckActorMessageCommand, AckActorMessageResult,
    ActorMailbox, ActorMessageHandlingDisposition, ActorMessageKind, ActorMessageTaskRelation,
    LinkActorMessageTaskCommand, LinkActorMessageTaskResult, RelayRemotePendingCommand,
    RelayRemotePendingResult, SendActorMessageCommand, TriageActorMessageCommand,
    infer_actor_message_kind, normalize_actor_message_envelope_payload,
};
use chrono::Utc;
use serde_json::Value;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};
use tokio::sync::Semaphore;

use super::codec::parse_team_actor_message_row;
pub(super) use super::mailbox_errors::{
    map_actor_mailbox_store_error, map_actor_service_error, optional_trimmed,
    required_trimmed_field, validate_direct_mailbox_target_for_member_specs,
};
pub(super) use super::mailbox_mentions::{
    build_channel_mailbox_forward_payload, find_raw_actor_mention_range,
};
use super::mailbox_payloads::{
    collect_channel_mention_actor_ids, should_persist_human_visible_chat_reply_for_payload,
};
use super::mailbox_service::TeamActorMailboxService;
pub(super) use super::mailbox_shared_thread::maybe_persist_human_visible_chat_reply;
pub(super) use super::mailbox_store::{
    SqlActorMailboxStore, SqlActorMailboxStoreError, enrich_actor_messages,
};
pub(super) use super::mailbox_threads::apply_thread_claim_transition;
use super::{
    TEAM_SHARED_THREAD_TITLE, TeamConversationStreamEvent, TeamManager, TeamReplyObligationSummary,
    parse_team_member_specs,
};
use crate::agent::normalize_target_node_id;
use crate::team::{
    TeamActorMessageRecord, TeamActorMessageTransport, TeamConversationMessageRecord,
};

pub(super) const TEAM_SPECIAL_USER_ACTOR_ALIAS: &str = "user";
pub(super) const TEAM_SPECIAL_USER_ACTOR_PREFIX: &str = "user:";
pub(super) const MAILBOX_RESOLUTION_ESCALATED: &str = "escalated";
const MAILBOX_RUN_EVENT_ARCHIVE_MAX_CONCURRENCY: usize = 4;
pub(super) const ACTOR_THREAD_CLAIM_DEFAULT_LEASE_SECS: i64 = 30 * 60;

static MAILBOX_RUN_EVENT_ARCHIVE_SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();

pub(super) fn mailbox_run_event_archive_semaphore() -> Arc<Semaphore> {
    MAILBOX_RUN_EVENT_ARCHIVE_SEMAPHORE
        .get_or_init(|| Arc::new(Semaphore::new(MAILBOX_RUN_EVENT_ARCHIVE_MAX_CONCURRENCY)))
        .clone()
}

#[derive(Debug, Clone)]
pub(super) struct SharedThreadTarget {
    pub(super) task_id: String,
    pub(super) conversation_id: String,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedChannelMailboxTarget {
    pub(super) team_id: String,
    pub(super) task_id: String,
    pub(super) conversation_id: String,
    pub(super) recipient_actor_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedMailboxRecipientDelivery {
    pub(super) actor_id: String,
    pub(super) to_peer_id: String,
    pub(super) transport: TeamActorMessageTransport,
    pub(super) route: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ReplyActorPairKey {
    pub(super) agent_actor_id: String,
    pub(super) human_actor_id: String,
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

    pub(crate) async fn summarize_open_reply_obligations(
        &self,
        run_id: &str,
    ) -> anyhow::Result<TeamReplyObligationSummary> {
        let messages =
            super::mailbox_reply_obligations::load_reply_obligation_message_snapshots_on_executor(
                &self.db, run_id,
            )
            .await?;
        Ok(
            super::mailbox_reply_obligations::summarize_open_reply_obligations_from_snapshots(
                messages.as_slice(),
            ),
        )
    }

    pub fn actor_thread_claim_conflict_owner(err: &anyhow::Error) -> Option<&str> {
        err.downcast_ref::<SqlActorMailboxStoreError>()
            .and_then(|cause| {
                if let SqlActorMailboxStoreError::ThreadClaimConflict { owner_actor_id } = cause {
                    Some(owner_actor_id.as_str())
                } else {
                    None
                }
            })
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
            .link_task(LinkActorMessageTaskCommand {
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
            message_archive: self.message_archive.clone(),
        })
    }

    pub(super) async fn resolve_channel_mailbox_target(
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

    pub(super) async fn extract_channel_mention_actor_ids(
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

    pub(super) async fn resolve_channel_recipient_deliveries(
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

    pub(super) async fn find_channel_message_by_correlation_id(
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
    pub message_kind: Option<ActorMessageKind>,
    pub idempotency_key: Option<&'a str>,
}

pub(super) fn normalize_optional_sqlite_string(raw: Option<String>) -> Option<String> {
    raw.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "null")
}

pub(super) fn parse_optional_sqlite_json_value(
    raw: Option<String>,
) -> Result<Option<Value>, sqlx::Error> {
    let Some(raw) = normalize_optional_sqlite_string(raw) else {
        return Ok(None);
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(value) => Ok(Some(value)),
        Err(_) => Ok(Some(Value::String(raw))),
    }
}

pub(super) async fn resolve_team_id_for_run(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
) -> Result<String, sqlx::Error> {
    let row = sqlx::query("SELECT team_id FROM team_runs WHERE id = ?1")
        .bind(run_id)
        .fetch_one(&mut **tx)
        .await?;
    row.try_get("team_id")
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
            idempotency_key,
            message_kind,
            handling_disposition,
            handled_by_actor_id,
            handled_at,
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

pub(super) async fn fetch_enriched_message_by_id(
    pool: &SqlitePool,
    message_id: i64,
) -> Result<TeamActorMessageRecord, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let mut message = fetch_message_by_id(&mut tx, message_id).await?;
    tx.commit().await?;
    enrich_actor_messages(pool, std::slice::from_mut(&mut message)).await?;
    Ok(message)
}

pub(super) async fn fetch_message_for_actor(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
    actor_id: &str,
    peer_id: &str,
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
            idempotency_key,
            message_kind,
            handling_disposition,
            handled_by_actor_id,
            handled_at,
            status,
            created_at,
            delivered_at
        FROM team_actor_messages
        WHERE id = ?1 AND run_id = ?2 AND to_actor_id = ?3 AND to_peer_id = ?4
        "#,
    )
    .bind(message_id)
    .bind(run_id)
    .bind(actor_id)
    .bind(peer_id)
    .fetch_one(&mut **tx)
    .await?;
    parse_team_actor_message_row(&row).map_err(|err| sqlx::Error::Protocol(err.to_string()))
}

pub(super) async fn fetch_message_by_idempotency(
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
            idempotency_key,
            message_kind,
            handling_disposition,
            handled_by_actor_id,
            handled_at,
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

    fn build_mailbox_record(
        message_id: i64,
        from_actor_id: &str,
        to_actor_id: &str,
        payload: Value,
    ) -> TeamActorMessageRecord {
        TeamActorMessageRecord {
            message_id,
            run_id: "run-1".to_string(),
            from_actor_id: from_actor_id.to_string(),
            from_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            from_actor_kind: agenthub_team_actor::infer_actor_identity_kind(from_actor_id),
            to_actor_id: to_actor_id.to_string(),
            to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            to_actor_kind: agenthub_team_actor::infer_actor_identity_kind(to_actor_id),
            channel: "default".to_string(),
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: payload.clone(),
            idempotency_key: None,
            message_kind: infer_actor_message_kind(from_actor_id, &payload, None),
            status: TeamActorMessageStatus::Pending,
            handling_disposition: ActorMessageHandlingDisposition::Untriaged,
            handled_by_actor_id: None,
            thread_topic_key: None,
            thread_claim_status: None,
            thread_owner_actor_id: None,
            thread_lease_expires_at: None,
            linked_task_id: None,
            linked_task_relation: None,
            created_at: 1_700_000_000 + message_id,
            delivered_at: None,
            handled_at: None,
        }
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
    fn summarize_open_reply_obligations_counts_unanswered_human_requests() {
        let messages = vec![build_mailbox_record(
            1,
            "user",
            "worker",
            json!({
                "type": "chat_message",
                "text": "Need update",
                "requires_user_visible_reply": true
            }),
        )];

        let summary = summarize_open_reply_obligations_from_messages(messages.as_slice());

        assert_eq!(summary.open_total, 1);
        assert_eq!(summary.open_by_actor.get("worker").copied(), Some(1));
        assert_eq!(summary.open_items.len(), 1);
        let obligation = &summary.open_items[0];
        assert_eq!(obligation.agent_actor_id, "worker");
        assert_eq!(obligation.human_actor_id, "user");
        assert_eq!(obligation.source_surface, "mailbox");
        assert_eq!(obligation.text_excerpt.as_deref(), Some("Need update"));
    }

    #[test]
    fn summarize_open_reply_obligations_consumes_visible_reply_credit() {
        let mut inbound = build_mailbox_record(
            1,
            "user",
            "worker",
            json!({
                "type": "chat_message",
                "text": "Need update",
                "requires_user_visible_reply": true
            }),
        );
        inbound.status = TeamActorMessageStatus::Delivered;
        let outbound = build_mailbox_record(
            2,
            "worker",
            "user",
            json!({
                "type": "chat_message",
                "text": "Here is the update"
            }),
        );

        let summary = summarize_open_reply_obligations_from_messages(&[inbound, outbound]);

        assert_eq!(summary.open_total, 0);
        assert!(summary.open_by_actor.is_empty());
    }

    #[test]
    fn summarize_open_reply_obligations_skips_ignored_and_completed_items() {
        let mut ignored = build_mailbox_record(
            1,
            "user",
            "worker",
            json!({
                "type": "chat_message",
                "text": "Need update",
                "requires_user_visible_reply": true
            }),
        );
        ignored.handling_disposition = ActorMessageHandlingDisposition::Ignored;
        let mut completed = build_mailbox_record(
            2,
            "user",
            "reviewer",
            json!({
                "type": "chat_message",
                "text": "Please review",
                "requires_user_visible_reply": true
            }),
        );
        completed.handling_disposition = ActorMessageHandlingDisposition::Completed;

        let summary = summarize_open_reply_obligations_from_messages(&[ignored, completed]);

        assert_eq!(summary.open_total, 0);
        assert!(summary.open_by_actor.is_empty());
    }

    #[test]
    fn validate_direct_mailbox_target_rejects_role_alias() {
        let member_specs = mock_member_specs(&[
            ("595d1ae8-fcbd-4111-b5c7-d446a12c044b", "coordinator"),
            ("c319f933-1358-4418-a111-872304052422", "worker"),
        ]);
        let err = validate_direct_mailbox_target_for_member_specs(&member_specs, "coordinator")
            .expect_err("role alias should be rejected");
        assert_eq!(err.code, ActorServiceErrorCode::BadRequest);
        assert!(err.message.contains("not a canonical team member_id"));
        assert!(err.message.contains("595d1ae8-fcbd-4111-b5c7-d446a12c044b"));
    }

    #[test]
    fn validate_direct_mailbox_target_allows_member_id_and_human_mailbox() {
        let member_specs = mock_member_specs(&[
            ("595d1ae8-fcbd-4111-b5c7-d446a12c044b", "coordinator"),
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
            ("595d1ae8-fcbd-4111-b5c7-d446a12c044b", "coordinator"),
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
            ("planner", "coordinator"),
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
