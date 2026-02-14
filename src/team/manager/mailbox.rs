use std::sync::Arc;
use std::time::Duration;

use agenthub_team_actor::{
    AckActorMessageCommand, AckActorMessageResult, ActorMailbox, ActorMailboxError,
    ActorMailboxStore, ActorMessageRelay, ActorRelayError, CreatePendingMessageResult,
    ListActorInboxQuery, PendingRemoteRelayRecord, RelayRemotePendingCommand,
    RelayRemotePendingResult, SendActorMessageCommand,
};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use sqlx::{Row, Sqlite, SqlitePool};
use thiserror::Error;

use super::TeamManager;
use super::codec::{
    parse_team_actor_message_row, team_actor_message_status_to_str,
    team_actor_message_transport_to_str,
};
use crate::team::{TeamActorMessageRecord, TeamActorMessageStatus, TeamActorMessageTransport};

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

    pub async fn send_actor_message(
        &self,
        run_id: &str,
        from_actor_id: &str,
        to_actor_id: &str,
        channel: &str,
        transport: TeamActorMessageTransport,
        route: Option<Value>,
        payload: Value,
        idempotency_key: Option<&str>,
    ) -> anyhow::Result<TeamActorMessageRecord> {
        let now = Utc::now().timestamp();
        let mailbox = self.actor_mailbox();
        let message = mailbox
            .send(SendActorMessageCommand {
                run_id: run_id.to_string(),
                from_actor_id: from_actor_id.to_string(),
                to_actor_id: to_actor_id.to_string(),
                channel: channel.to_string(),
                transport,
                route,
                payload,
                idempotency_key: idempotency_key.map(str::to_string),
                created_at: now,
            })
            .await
            .map_err(map_actor_mailbox_store_error)?;
        Ok(message)
    }

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
    ) -> anyhow::Result<TeamActorMessageRecord> {
        let now = Utc::now().timestamp();
        let mailbox = self.actor_mailbox();
        let message = mailbox
            .ack(AckActorMessageCommand {
                run_id: run_id.to_string(),
                actor_id: actor_id.to_string(),
                message_id,
                delivered_at: now,
            })
            .await
            .map_err(map_actor_mailbox_store_error)?;
        Ok(message)
    }

    pub async fn relay_remote_messages_once(
        &self,
        limit: i64,
        max_attempts: i64,
        retry_delay_secs: i64,
    ) -> anyhow::Result<RelayRemotePendingResult> {
        let now = Utc::now().timestamp();
        let mailbox = self.actor_mailbox();
        let relay = TeamRemoteRelayAdapter;
        let result = mailbox
            .relay_remote_pending(
                &relay,
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
}

#[derive(Clone, Copy)]
struct TeamRemoteRelayAdapter;

#[derive(Debug, Error)]
enum TeamRemoteRelayError {
    #[error("route.endpoint is required for remote relay")]
    MissingEndpoint,
    #[error("simulated retryable relay failure for endpoint {0}")]
    SimulatedRetryable(String),
    #[error("simulated permanent relay failure for endpoint {0}")]
    SimulatedPermanent(String),
    #[error("relay adapter is not configured for endpoint {0}")]
    UnconfiguredEndpoint(String),
}

#[async_trait]
impl ActorMessageRelay for TeamRemoteRelayAdapter {
    type Error = TeamRemoteRelayError;

    async fn deliver(
        &self,
        message: &TeamActorMessageRecord,
    ) -> Result<(), ActorRelayError<Self::Error>> {
        let endpoint = message
            .route
            .as_ref()
            .and_then(|route| route.get("endpoint"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ActorRelayError::permanent(TeamRemoteRelayError::MissingEndpoint))?;

        // Deterministic mock endpoints keep relay policy tests stable without network calls.
        if endpoint.starts_with("mock://ok") {
            return Ok(());
        }
        if endpoint.starts_with("mock://retry") {
            return Err(ActorRelayError::retryable(
                TeamRemoteRelayError::SimulatedRetryable(endpoint.to_string()),
            ));
        }
        if endpoint.starts_with("mock://dead") {
            return Err(ActorRelayError::permanent(
                TeamRemoteRelayError::SimulatedPermanent(endpoint.to_string()),
            ));
        }

        Err(ActorRelayError::retryable(
            TeamRemoteRelayError::UnconfiguredEndpoint(endpoint.to_string()),
        ))
    }
}

#[derive(Clone)]
struct SqlActorMailboxStore {
    db: SqlitePool,
}

#[async_trait]
impl ActorMailboxStore for SqlActorMailboxStore {
    type Error = sqlx::Error;

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
                to_actor_id,
                channel,
                transport,
                route_json,
                payload_json,
                status,
                created_at,
                idempotency_key
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(&cmd.run_id)
        .bind(&cmd.from_actor_id)
        .bind(&cmd.to_actor_id)
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
            let message = fetch_message_by_id(&mut tx, message_id).await?;
            (message, true)
        } else if let Some(idempotency_key) = cmd.idempotency_key.as_deref() {
            let message = fetch_message_by_idempotency(
                &mut tx,
                &cmd.run_id,
                &cmd.from_actor_id,
                idempotency_key,
            )
            .await?;
            (message, false)
        } else {
            return Err(sqlx::Error::Protocol(
                "insert was ignored without idempotency_key".to_string(),
            ));
        };
        tx.commit().await?;
        Ok(CreatePendingMessageResult { message, created })
    }

    async fn list_inbox(
        &self,
        query: &ListActorInboxQuery,
    ) -> Result<Vec<TeamActorMessageRecord>, Self::Error> {
        let rows = if query.include_delivered {
            if let Some(after_id) = query.after_id {
                sqlx::query(
                    r#"
                    SELECT
                        id,
                        run_id,
                        from_actor_id,
                        to_actor_id,
                        channel,
                        transport,
                        route_json,
                        payload_json,
                        status,
                        created_at,
                        delivered_at
                    FROM team_actor_messages
                    WHERE run_id = ?1 AND to_actor_id = ?2 AND id > ?3
                    ORDER BY id ASC
                    LIMIT ?4
                    "#,
                )
                .bind(&query.run_id)
                .bind(&query.actor_id)
                .bind(after_id)
                .bind(query.limit)
                .fetch_all(&self.db)
                .await?
            } else {
                sqlx::query(
                    r#"
                    SELECT
                        id,
                        run_id,
                        from_actor_id,
                        to_actor_id,
                        channel,
                        transport,
                        route_json,
                        payload_json,
                        status,
                        created_at,
                        delivered_at
                    FROM team_actor_messages
                    WHERE run_id = ?1 AND to_actor_id = ?2
                    ORDER BY id ASC
                    LIMIT ?3
                    "#,
                )
                .bind(&query.run_id)
                .bind(&query.actor_id)
                .bind(query.limit)
                .fetch_all(&self.db)
                .await?
            }
        } else if let Some(after_id) = query.after_id {
            sqlx::query(
                r#"
                SELECT
                    id,
                    run_id,
                    from_actor_id,
                    to_actor_id,
                    channel,
                    transport,
                    route_json,
                    payload_json,
                    status,
                    created_at,
                    delivered_at
                FROM team_actor_messages
                WHERE run_id = ?1 AND to_actor_id = ?2 AND status = 'pending' AND id > ?3
                ORDER BY id ASC
                LIMIT ?4
                "#,
            )
            .bind(&query.run_id)
            .bind(&query.actor_id)
            .bind(after_id)
            .bind(query.limit)
            .fetch_all(&self.db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT
                    id,
                    run_id,
                    from_actor_id,
                    to_actor_id,
                    channel,
                    transport,
                    route_json,
                    payload_json,
                    status,
                    created_at,
                    delivered_at
                FROM team_actor_messages
                WHERE run_id = ?1 AND to_actor_id = ?2 AND status = 'pending'
                ORDER BY id ASC
                LIMIT ?3
                "#,
            )
            .bind(&query.run_id)
            .bind(&query.actor_id)
            .bind(query.limit)
            .fetch_all(&self.db)
            .await?
        };

        let mut messages = Vec::with_capacity(rows.len());
        for row in rows {
            messages.push(
                parse_team_actor_message_row(&row)
                    .map_err(|err| sqlx::Error::Protocol(err.to_string()))?,
            );
        }
        Ok(messages)
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
            WHERE id = ?2 AND run_id = ?3 AND to_actor_id = ?4 AND status = 'pending'
            "#,
        )
        .bind(cmd.delivered_at)
        .bind(cmd.message_id)
        .bind(&cmd.run_id)
        .bind(&cmd.actor_id)
        .execute(&mut *tx)
        .await?;

        let message_row = sqlx::query(
            r#"
            SELECT
                id,
                run_id,
                from_actor_id,
                to_actor_id,
                channel,
                transport,
                route_json,
                payload_json,
                status,
                created_at,
                delivered_at
            FROM team_actor_messages
            WHERE id = ?1 AND run_id = ?2 AND to_actor_id = ?3
            "#,
        )
        .bind(cmd.message_id)
        .bind(&cmd.run_id)
        .bind(&cmd.actor_id)
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
                to_actor_id,
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
            to_actor_id,
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
    idempotency_key: &str,
) -> Result<TeamActorMessageRecord, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            id,
            run_id,
            from_actor_id,
            to_actor_id,
            channel,
            transport,
            route_json,
            payload_json,
            status,
            created_at,
            delivered_at
        FROM team_actor_messages
        WHERE run_id = ?1 AND from_actor_id = ?2 AND idempotency_key = ?3
        LIMIT 1
        "#,
    )
    .bind(run_id)
    .bind(from_actor_id)
    .bind(idempotency_key)
    .fetch_one(&mut **tx)
    .await?;
    parse_team_actor_message_row(&row).map_err(|err| sqlx::Error::Protocol(err.to_string()))
}

fn map_actor_mailbox_store_error(err: ActorMailboxError<sqlx::Error>) -> anyhow::Error {
    match err {
        ActorMailboxError::Store(store_err) => anyhow::Error::new(store_err),
    }
}
