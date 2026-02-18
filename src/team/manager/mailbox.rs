use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use agenthub_team_actor::{
    AckActorMessageCommand, AckActorMessageResult, ActorAckRequest, ActorAckResponse,
    ActorInboxRequest, ActorInboxResponse, ActorMailbox, ActorMailboxError, ActorMailboxService,
    ActorMailboxStore, ActorMessageRelay, ActorRelayError, ActorSendRequest, ActorSendResponse,
    ActorServiceError, ActorServiceErrorCode, CreatePendingMessageResult, ListActorInboxQuery,
    PendingRemoteRelayRecord, RelayRemotePendingCommand, RelayRemotePendingResult,
    SendActorMessageCommand, actor_message_fingerprint,
};
use async_trait::async_trait;
use base64::encode_config;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::{Method, Url, header};
use serde::Deserialize;
use serde_json::Value;
use sha2::Sha256;
use sqlx::{Error as SqlxError, Row, Sqlite, SqlitePool};
use thiserror::Error;

use super::TeamManager;
use super::codec::{
    parse_team_actor_message_row, team_actor_message_status_to_str,
    team_actor_message_transport_to_str,
};
use crate::team::{TeamActorMessageRecord, TeamActorMessageStatus, TeamActorMessageTransport};

const RELAY_DEFAULT_TIMEOUT_MS: u64 = 30_000;
const RELAY_TIMEOUT_MIN_MS: u64 = 100;
const RELAY_TIMEOUT_MAX_MS: u64 = 60_000;

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
            to_actor_id,
            channel,
            transport,
            route,
            payload,
            idempotency_key,
        } = request;
        let now = Utc::now().timestamp();
        let mailbox = self.actor_mailbox();
        let result = mailbox
            .send_with_result(SendActorMessageCommand {
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
        Ok((result.message, result.created))
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
        let relay = shared_remote_relay_adapter();
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
}

pub struct SendActorMessageInput<'a> {
    pub run_id: &'a str,
    pub from_actor_id: &'a str,
    pub to_actor_id: &'a str,
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
}

#[async_trait]
impl ActorMailboxService for TeamActorMailboxService {
    async fn actor_send(
        &self,
        request: ActorSendRequest,
    ) -> Result<ActorSendResponse, ActorServiceError> {
        let run_id = required_trimmed_field(&request.run_id, "run_id")?;
        let from_actor_id = required_trimmed_field(&request.from_actor_id, "from_actor_id")?;
        let to_actor_id = required_trimmed_field(&request.to_actor_id, "to_actor_id")?;
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

        let (message, created) = self
            .manager
            .send_actor_message_with_created(SendActorMessageInput {
                run_id,
                from_actor_id,
                to_actor_id,
                channel,
                transport,
                route: request.route,
                payload: request.payload,
                idempotency_key,
            })
            .await
            .map_err(map_actor_service_error)?;

        Ok(ActorSendResponse {
            message_id: message.message_id,
            state: message.status,
            deduped: !created,
            created_at: message.created_at,
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
        let messages = self
            .manager
            .list_actor_inbox(run_id, actor_id, limit, request.cursor, include_delivered)
            .await
            .map_err(map_actor_service_error)?
            .into_iter()
            .filter(|message| states.contains(&message.status))
            .collect::<Vec<_>>();
        let next_cursor = messages.last().map(|message| message.message_id);

        Ok(ActorInboxResponse {
            messages,
            next_cursor,
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

        let message = self
            .manager
            .ack_actor_message(run_id, actor_id, request.message_id)
            .await
            .map_err(map_actor_service_error)?;
        let state = message.status.clone();
        let acked_at = message.delivered_at.unwrap_or(message.created_at);

        Ok(ActorAckResponse {
            message_id: message.message_id,
            state,
            acked_at,
            message,
        })
    }
}

#[derive(Clone)]
struct TeamRemoteRelayAdapter {
    client: reqwest::Client,
}

fn shared_remote_relay_adapter() -> &'static TeamRemoteRelayAdapter {
    static SHARED_RELAY_ADAPTER: OnceLock<TeamRemoteRelayAdapter> = OnceLock::new();
    SHARED_RELAY_ADAPTER.get_or_init(TeamRemoteRelayAdapter::new)
}

impl TeamRemoteRelayAdapter {
    fn new() -> Self {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(32)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .danger_accept_invalid_certs(false)
            .build()
            .unwrap_or_else(|err| {
                tracing::warn!(
                    "build relay client failed, fallback to default client: {}",
                    err
                );
                reqwest::Client::new()
            });
        Self { client }
    }
}

#[derive(Debug, Error)]
enum TeamRemoteRelayError {
    #[error("route is required for remote relay")]
    MissingRoute,
    #[error("route.endpoint is required for remote relay")]
    MissingEndpoint,
    #[error("route.endpoint must be a valid http/https URL")]
    InvalidEndpoint,
    #[error("route.method is invalid or not supported: {0}")]
    UnsupportedMethod(String),
    #[error("route.auth is invalid")]
    InvalidAuth,
    #[error("route.signing is invalid")]
    InvalidSigning,
    #[error("route payload is invalid: {0}")]
    InvalidRoute(String),
    #[error("request build failed: {0}")]
    RequestBuild(String),
    #[error("relay request failed: {0}")]
    RequestTransport(String),
    #[error("relay got retryable response status={status} body={body}")]
    RetryableHttpResponse { status: u16, body: String },
    #[error("relay got permanent response status={status} body={body}")]
    PermanentHttpResponse { status: u16, body: String },
}

#[async_trait]
impl ActorMessageRelay for TeamRemoteRelayAdapter {
    type Error = TeamRemoteRelayError;

    async fn deliver(
        &self,
        message: &TeamActorMessageRecord,
    ) -> Result<(), ActorRelayError<Self::Error>> {
        let route = parse_remote_route(
            message
                .route
                .as_ref()
                .ok_or_else(|| ActorRelayError::permanent(TeamRemoteRelayError::MissingRoute))?,
        )?;

        let endpoint = route.route.endpoint.trim().to_string();
        if endpoint.is_empty() {
            return Err(ActorRelayError::permanent(
                TeamRemoteRelayError::MissingEndpoint,
            ));
        }
        let url = Url::parse(&endpoint)
            .map_err(|_| ActorRelayError::permanent(TeamRemoteRelayError::InvalidEndpoint))?;
        if !(url.scheme() == "http" || url.scheme() == "https") {
            return Err(ActorRelayError::permanent(
                TeamRemoteRelayError::InvalidEndpoint,
            ));
        }

        let method = parse_route_method(route.route.method.as_deref())?;
        let envelope = build_remote_relay_envelope(message);
        let payload_bytes = serde_json::to_vec(&envelope).map_err(|err| {
            ActorRelayError::permanent(TeamRemoteRelayError::RequestBuild(err.to_string()))
        })?;

        let mut request = self
            .client
            .request(method, url)
            .header(header::CONTENT_TYPE, "application/json")
            .body(payload_bytes.clone());
        request = request.timeout(Duration::from_millis(relay_timeout_ms(
            route.route.timeout_ms,
        )));
        request = apply_route_headers(request, &route.route.headers)?;
        request = apply_route_auth(request, route.route.auth.as_ref())?;
        request = apply_route_signing(
            request,
            route.route.signing.as_ref(),
            &payload_bytes,
            message.message_id,
        )?;

        let response = request.send().await.map_err(|err| {
            if err.is_builder() {
                return ActorRelayError::permanent(TeamRemoteRelayError::RequestBuild(
                    err.to_string(),
                ));
            }
            ActorRelayError::retryable(TeamRemoteRelayError::RequestTransport(err.to_string()))
        })?;

        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<body-read-error>".to_string());
        let body_preview = body.chars().take(256).collect::<String>();
        if status.as_u16() == 429 || status.is_server_error() {
            return Err(ActorRelayError::retryable(
                TeamRemoteRelayError::RetryableHttpResponse {
                    status: status.as_u16(),
                    body: body_preview,
                },
            ));
        }
        Err(ActorRelayError::permanent(
            TeamRemoteRelayError::PermanentHttpResponse {
                status: status.as_u16(),
                body: body_preview,
            },
        ))
    }
}

#[derive(Debug, Deserialize)]
struct RemoteRelayRouteValue {
    endpoint: String,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    auth: Option<RemoteRelayAuthValue>,
    #[serde(default)]
    signing: Option<RemoteRelaySigningValue>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RemoteRelayAuthValue {
    Bearer { token: String },
    Header { name: String, value: String },
    Basic { username: String, password: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RemoteRelaySigningValue {
    HmacSha256 {
        secret: String,
        #[serde(default)]
        header: Option<String>,
        #[serde(default)]
        timestamp_header: Option<String>,
    },
}

struct ParsedRemoteRelayRoute {
    route: RemoteRelayRouteValue,
}

fn parse_remote_route(
    route: &Value,
) -> Result<ParsedRemoteRelayRoute, ActorRelayError<TeamRemoteRelayError>> {
    let parsed = serde_json::from_value::<RemoteRelayRouteValue>(route.clone()).map_err(|err| {
        ActorRelayError::permanent(TeamRemoteRelayError::InvalidRoute(err.to_string()))
    })?;
    Ok(ParsedRemoteRelayRoute { route: parsed })
}

fn parse_route_method(
    method: Option<&str>,
) -> Result<Method, ActorRelayError<TeamRemoteRelayError>> {
    let raw = method.unwrap_or("POST").trim();
    let parsed = Method::from_bytes(raw.as_bytes()).map_err(|_| {
        ActorRelayError::permanent(TeamRemoteRelayError::UnsupportedMethod(raw.to_string()))
    })?;
    if matches!(parsed, Method::POST | Method::PUT | Method::PATCH) {
        return Ok(parsed);
    }
    Err(ActorRelayError::permanent(
        TeamRemoteRelayError::UnsupportedMethod(raw.to_string()),
    ))
}

fn relay_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    timeout_ms
        .unwrap_or(RELAY_DEFAULT_TIMEOUT_MS)
        .clamp(RELAY_TIMEOUT_MIN_MS, RELAY_TIMEOUT_MAX_MS)
}

fn parse_route_header_name(
    name: &str,
) -> Result<header::HeaderName, ActorRelayError<TeamRemoteRelayError>> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ActorRelayError::permanent(
            TeamRemoteRelayError::InvalidRoute(
                "route.headers contains empty header name".to_string(),
            ),
        ));
    }
    header::HeaderName::from_bytes(trimmed.as_bytes()).map_err(|_| {
        ActorRelayError::permanent(TeamRemoteRelayError::InvalidRoute(format!(
            "route.headers contains invalid header name '{}'",
            trimmed
        )))
    })
}

fn parse_route_header_value(
    value: &str,
) -> Result<header::HeaderValue, ActorRelayError<TeamRemoteRelayError>> {
    if value.chars().any(|ch| ch == '\r' || ch == '\n') {
        return Err(ActorRelayError::permanent(
            TeamRemoteRelayError::InvalidRoute(
                "route.headers contains invalid control characters in header value".to_string(),
            ),
        ));
    }
    header::HeaderValue::from_str(value).map_err(|_| {
        ActorRelayError::permanent(TeamRemoteRelayError::InvalidRoute(
            "route.headers contains invalid header value".to_string(),
        ))
    })
}

fn apply_route_headers(
    mut request: reqwest::RequestBuilder,
    headers: &HashMap<String, String>,
) -> Result<reqwest::RequestBuilder, ActorRelayError<TeamRemoteRelayError>> {
    for (name, value) in headers {
        let key = parse_route_header_name(name)?;
        let parsed = parse_route_header_value(value)?;
        request = request.header(key, parsed);
    }
    Ok(request)
}

fn apply_route_auth(
    mut request: reqwest::RequestBuilder,
    auth: Option<&RemoteRelayAuthValue>,
) -> Result<reqwest::RequestBuilder, ActorRelayError<TeamRemoteRelayError>> {
    let Some(auth) = auth else {
        return Ok(request);
    };
    match auth {
        RemoteRelayAuthValue::Bearer { token } => {
            let token = token.trim();
            if token.is_empty() {
                return Err(ActorRelayError::permanent(
                    TeamRemoteRelayError::InvalidAuth,
                ));
            }
            request = request.bearer_auth(token);
        }
        RemoteRelayAuthValue::Header { name, value } => {
            let header_name = header::HeaderName::from_bytes(name.trim().as_bytes())
                .map_err(|_| ActorRelayError::permanent(TeamRemoteRelayError::InvalidAuth))?;
            let header_value = parse_route_header_value(value)
                .map_err(|_| ActorRelayError::permanent(TeamRemoteRelayError::InvalidAuth))?;
            request = request.header(header_name, header_value);
        }
        RemoteRelayAuthValue::Basic { username, password } => {
            if username.trim().is_empty() {
                return Err(ActorRelayError::permanent(
                    TeamRemoteRelayError::InvalidAuth,
                ));
            }
            request = request.basic_auth(username, Some(password));
        }
    }
    Ok(request)
}

fn apply_route_signing(
    mut request: reqwest::RequestBuilder,
    signing: Option<&RemoteRelaySigningValue>,
    payload_bytes: &[u8],
    message_id: i64,
) -> Result<reqwest::RequestBuilder, ActorRelayError<TeamRemoteRelayError>> {
    let Some(signing) = signing else {
        return Ok(request);
    };
    match signing {
        RemoteRelaySigningValue::HmacSha256 {
            secret,
            header,
            timestamp_header,
        } => {
            let secret = secret.trim();
            if secret.is_empty() {
                return Err(ActorRelayError::permanent(
                    TeamRemoteRelayError::InvalidSigning,
                ));
            }
            type HmacSha256 = Hmac<Sha256>;
            let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
                .map_err(|_| ActorRelayError::permanent(TeamRemoteRelayError::InvalidSigning))?;
            let timestamp = Utc::now().timestamp().to_string();
            let message_id_text = message_id.to_string();
            mac.update(message_id_text.as_bytes());
            mac.update(b".");
            mac.update(timestamp.as_bytes());
            mac.update(b".");
            mac.update(payload_bytes);
            let signature = encode_config(mac.finalize().into_bytes(), base64::STANDARD);
            let header_name = header
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("X-AgentHub-Signature");
            let ts_header_name = timestamp_header
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("X-AgentHub-Timestamp");
            let msg_id_header_name = "X-AgentHub-Message-Id";
            let signature_header = parse_route_header_name(header_name)
                .map_err(|_| ActorRelayError::permanent(TeamRemoteRelayError::InvalidSigning))?;
            let timestamp_header = parse_route_header_name(ts_header_name)
                .map_err(|_| ActorRelayError::permanent(TeamRemoteRelayError::InvalidSigning))?;
            let message_id_header = parse_route_header_name(msg_id_header_name)
                .map_err(|_| ActorRelayError::permanent(TeamRemoteRelayError::InvalidSigning))?;
            request = request.header(signature_header, format!("hmac-sha256={signature}"));
            request = request.header(timestamp_header, timestamp);
            request = request.header(message_id_header, message_id_text);
        }
    }
    Ok(request)
}

fn build_remote_relay_envelope(message: &TeamActorMessageRecord) -> Value {
    serde_json::json!({
        "schema_version": 1,
        "message_id": message.message_id,
        "run_id": &message.run_id,
        "from_actor_id": &message.from_actor_id,
        "to_actor_id": &message.to_actor_id,
        "channel": &message.channel,
        "transport": message.transport.as_str(),
        "created_at": message.created_at,
        "payload": &message.payload,
    })
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

fn ensure_idempotency_compatible(
    cmd: &SendActorMessageCommand,
    existing: &TeamActorMessageRecord,
) -> Result<(), SqlActorMailboxStoreError> {
    let incoming_fp = actor_message_fingerprint(
        &cmd.run_id,
        &cmd.from_actor_id,
        &cmd.to_actor_id,
        &cmd.channel,
        cmd.transport.as_str(),
        cmd.route.as_ref(),
        &cmd.payload,
    );
    let existing_fp = actor_message_fingerprint(
        &existing.run_id,
        &existing.from_actor_id,
        &existing.to_actor_id,
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

fn is_row_not_found(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<SqlxError>(),
        Some(SqlxError::RowNotFound)
    )
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
    use super::{
        RELAY_DEFAULT_TIMEOUT_MS, RELAY_TIMEOUT_MAX_MS, RELAY_TIMEOUT_MIN_MS,
        RemoteRelaySigningValue, apply_route_signing, parse_route_header_name,
        parse_route_header_value, relay_timeout_ms, shared_remote_relay_adapter,
    };

    #[test]
    fn relay_timeout_ms_defaults_and_clamps() {
        assert_eq!(relay_timeout_ms(None), RELAY_DEFAULT_TIMEOUT_MS);
        assert_eq!(relay_timeout_ms(Some(1)), RELAY_TIMEOUT_MIN_MS);
        assert_eq!(
            relay_timeout_ms(Some(RELAY_TIMEOUT_MAX_MS + 1000)),
            RELAY_TIMEOUT_MAX_MS
        );
    }

    #[test]
    fn parse_route_header_name_rejects_empty_and_invalid() {
        assert!(parse_route_header_name(" ").is_err());
        assert!(parse_route_header_name("bad space").is_err());
        assert!(parse_route_header_name("x-agenthub-header").is_ok());
    }

    #[test]
    fn parse_route_header_value_rejects_control_chars() {
        assert!(parse_route_header_value("ok-value").is_ok());
        assert!(parse_route_header_value("bad\nvalue").is_err());
        assert!(parse_route_header_value("bad\rvalue").is_err());
    }

    #[test]
    fn shared_remote_relay_adapter_is_reused() {
        let first = shared_remote_relay_adapter() as *const _;
        let second = shared_remote_relay_adapter() as *const _;
        assert_eq!(first, second);
    }

    #[test]
    fn apply_route_signing_sets_signature_timestamp_and_message_id_headers() {
        let client = reqwest::Client::new();
        let request = client.post("https://example.com/relay");
        let signed = apply_route_signing(
            request,
            Some(&RemoteRelaySigningValue::HmacSha256 {
                secret: "test-secret".to_string(),
                header: None,
                timestamp_header: None,
            }),
            br#"{"payload":"x"}"#,
            42,
        )
        .expect("sign request");
        let built = signed.build().expect("build signed request");
        let headers = built.headers();

        let signature = headers
            .get("X-AgentHub-Signature")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(signature.starts_with("hmac-sha256="));
        assert!(headers.contains_key("X-AgentHub-Timestamp"));
        assert_eq!(
            headers
                .get("X-AgentHub-Message-Id")
                .and_then(|value| value.to_str().ok()),
            Some("42")
        );
    }
}
