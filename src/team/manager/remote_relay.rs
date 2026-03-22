use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ActorMessageRelay, ActorMessageTransport, ActorRelayError,
    ActorSendRequest, ActorServiceError, ActorServiceErrorCode,
};
use async_trait::async_trait;
use base64::encode_config;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::{Method, Url, header};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::Sha256;
use sqlx::{Row, SqlitePool};

use crate::agent::AGENT_NODE_MAIN_ID;
use crate::internal::auth::{InternalAction, InternalAuthz, InternalAuthzConfig, InternalRole};
use crate::internal::client::{
    InternalGrpcMailboxClient, InternalGrpcMailboxClientConfig, InternalGrpcPeerClientConfig,
};
use crate::internal::p2p::{
    CredentialProvider, NodeCredentialRequest, NodeTransportMetadata, P2PTransport,
    build_message_metadata,
};
use crate::internal::tls::InternalGrpcSecurityMode;
use crate::team::TeamActorMessageRecord;

const RELAY_DEFAULT_TIMEOUT_MS: u64 = 30_000;
const RELAY_TIMEOUT_MIN_MS: u64 = 100;
const RELAY_TIMEOUT_MAX_MS: u64 = 60_000;

#[derive(Clone)]
pub(super) struct TeamRemoteRelayAdapter {
    db: SqlitePool,
    http_client: reqwest::Client,
    grpc_tls_defaults: Arc<Mutex<Option<GrpcRelayTlsDefaults>>>,
    grpc_peer_client_config: Arc<Mutex<Option<InternalGrpcPeerClientConfig>>>,
    grpc_client_cache: Arc<Mutex<HashMap<GrpcRelayClientCacheKey, InternalGrpcMailboxClient>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GrpcRelayTlsDefaults {
    ca_cert_path: Option<String>,
    client_cert_path: Option<String>,
    client_key_path: Option<String>,
}

impl GrpcRelayTlsDefaults {
    pub(super) fn from_cert_dir(cert_dir: &Path, security_mode: InternalGrpcSecurityMode) -> Self {
        let ca_cert_path = path_to_string_if_exists(&cert_dir.join("ca-cert.pem"));
        let (client_cert_path, client_key_path) = if security_mode == InternalGrpcSecurityMode::Mtls
        {
            (
                path_to_string_if_exists(&cert_dir.join("client-cert.pem")),
                path_to_string_if_exists(&cert_dir.join("client-key.pem")),
            )
        } else {
            (None, None)
        };
        Self {
            ca_cert_path,
            client_cert_path,
            client_key_path,
        }
    }
}

fn path_to_string_if_exists(path: &Path) -> Option<String> {
    path.exists().then(|| path.to_string_lossy().to_string())
}

impl TeamRemoteRelayAdapter {
    pub(super) fn new(db: SqlitePool) -> Self {
        let builder = reqwest::Client::builder()
            .pool_max_idle_per_host(32)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .danger_accept_invalid_certs(false);

        #[cfg(test)]
        let builder = builder.no_proxy();

        let http_client = builder.build().unwrap_or_else(|err| {
            tracing::warn!(
                "build relay client failed, fallback to default client: {}",
                err
            );
            reqwest::Client::new()
        });
        Self {
            db,
            http_client,
            grpc_tls_defaults: Arc::new(Mutex::new(None)),
            grpc_peer_client_config: Arc::new(Mutex::new(None)),
            grpc_client_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(super) fn configure_grpc_tls_defaults(&self, defaults: Option<GrpcRelayTlsDefaults>) {
        *self
            .grpc_tls_defaults
            .lock()
            .expect("lock grpc tls defaults") = defaults;
        self.grpc_client_cache
            .lock()
            .expect("lock grpc client cache")
            .clear();
    }

    pub(super) fn configure_grpc_peer_client(&self, config: Option<InternalGrpcPeerClientConfig>) {
        *self
            .grpc_peer_client_config
            .lock()
            .expect("lock grpc peer client config") = config;
    }

    pub(super) async fn build_registered_grpc_route_for_target_node(
        &self,
        target_node_id: &str,
    ) -> anyhow::Result<Value> {
        let normalized_target_node_id = target_node_id.trim();
        if normalized_target_node_id.is_empty() || normalized_target_node_id == AGENT_NODE_MAIN_ID {
            anyhow::bail!("target node id must reference a registered remote agent node");
        }
        let peer_config = self
            .grpc_peer_client_config
            .lock()
            .expect("lock grpc peer client config")
            .clone()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "internal gRPC peer config is missing; remote channel mailbox delivery is unavailable"
                )
            })?;
        let row = sqlx::query(
            r#"
            SELECT grpc_target, tls_server_name
            FROM agent_nodes
            WHERE id = ?1
            "#,
        )
        .bind(normalized_target_node_id)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("agent node '{}' not found", normalized_target_node_id))?;
        let grpc_target = row.get::<String, _>("grpc_target").trim().to_string();
        anyhow::ensure!(
            !grpc_target.is_empty(),
            "agent node '{}' does not have a valid gRPC target",
            normalized_target_node_id
        );
        let tls_server_name = row
            .try_get::<Option<String>, _>("tls_server_name")
            .ok()
            .flatten()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let authz = InternalAuthz::new(InternalAuthzConfig {
            shared_secret: peer_config.shared_secret.clone(),
            expected_issuer: peer_config.expected_issuer.clone(),
            expected_audience: peer_config.expected_audience.clone(),
        });
        let issued = authz.issue_node_access_token(NodeCredentialRequest {
            source_node_id: peer_config.source_node_id.clone(),
            role: InternalRole::Leader.as_str().to_string(),
            actor_id: None,
            run_id: None,
            permissions: vec![InternalAction::MessageSend.as_str().to_string()],
            scope: Vec::new(),
            audience: Vec::new(),
            ttl_seconds: 600,
        })?;

        let mut route = serde_json::Map::new();
        route.insert("kind".to_string(), json!("grpc"));
        route.insert("grpc_target".to_string(), json!(grpc_target));
        route.insert("access_token".to_string(), json!(issued.access_token));
        if let Some(value) = tls_server_name.as_deref() {
            route.insert("tls_server_name".to_string(), json!(value));
        }
        NodeTransportMetadata {
            cluster_id: issued.cluster_id,
            source_node_id: issued.source_node_id,
            target_node_id: normalized_target_node_id.to_string(),
            broadcast_id: None,
            correlation_id: None,
            idempotency_key: None,
            scope: issued.scope,
            audience: issued.audience,
            issued_at: issued.issued_at,
            expires_at: issued.expires_at,
            kid: issued.kid,
            payload_digest: None,
        }
        .apply_to_route(&mut route);

        Ok(Value::Object(route))
    }

    async fn deliver_http(
        &self,
        message: &TeamActorMessageRecord,
        route: HttpRemoteRelayRouteValue,
    ) -> Result<(), ActorRelayError<TeamRemoteRelayError>> {
        let endpoint = route.endpoint.trim().to_string();
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

        let method = parse_route_method(route.method.as_deref())?;
        let envelope = build_remote_relay_envelope(message);
        let payload_bytes = serde_json::to_vec(&envelope).map_err(|err| {
            ActorRelayError::permanent(TeamRemoteRelayError::RequestBuild(err.to_string()))
        })?;

        let mut request = self
            .http_client
            .request(method, url)
            .header(header::CONTENT_TYPE, "application/json")
            .body(payload_bytes.clone());
        request = request.timeout(Duration::from_millis(relay_timeout_ms(route.timeout_ms)));
        request = apply_route_headers(request, &route.headers)?;
        request = apply_route_auth(request, route.auth.as_ref())?;
        request = apply_route_signing(
            request,
            route.signing.as_ref(),
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

    async fn deliver_grpc(
        &self,
        message: &TeamActorMessageRecord,
        route: GrpcRemoteRelayRouteValue,
    ) -> Result<(), ActorRelayError<TeamRemoteRelayError>> {
        let metadata = build_message_metadata(message);
        let resolved_route = self
            .resolve_registered_grpc_route(&metadata.target_node_id, &route)
            .await?;
        let access_token = route.access_token.trim();
        if access_token.is_empty() {
            return Err(ActorRelayError::permanent(
                TeamRemoteRelayError::MissingAccessToken,
            ));
        }
        let grpc_tls_defaults = self
            .grpc_tls_defaults
            .lock()
            .expect("lock grpc tls defaults")
            .clone()
            .ok_or_else(|| ActorRelayError::permanent(TeamRemoteRelayError::GrpcTlsUnavailable))?;

        let client = self
            .grpc_client_for_route(InternalGrpcMailboxClientConfig {
                target: resolved_route.grpc_target,
                access_token: access_token.to_string(),
                ca_cert_path: grpc_tls_defaults.ca_cert_path,
                tls_server_name: resolved_route.tls_server_name,
                client_cert_path: grpc_tls_defaults.client_cert_path,
                client_key_path: grpc_tls_defaults.client_key_path,
            })
            .await?;

        client
            .send_p2p_message(ActorSendRequest {
                run_id: message.run_id.clone(),
                from_actor_id: message.from_actor_id.clone(),
                from_peer_id: Some(metadata.source_node_id),
                to_actor_id: Some(message.to_actor_id.clone()),
                channel_id: None,
                to_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
                channel: Some(message.channel.clone()),
                transport: Some(ActorMessageTransport::Local),
                route: None,
                payload: message.payload.clone(),
                idempotency_key: Some(format!(
                    "remote-relay:{}:{}",
                    message.run_id, message.message_id
                )),
            })
            .await
            .map_err(map_grpc_actor_error)?;
        Ok(())
    }

    async fn resolve_registered_grpc_route(
        &self,
        target_node_id: &str,
        route: &GrpcRemoteRelayRouteValue,
    ) -> Result<ResolvedGrpcRelayRoute, ActorRelayError<TeamRemoteRelayError>> {
        let normalized_target_node_id = target_node_id.trim();
        if normalized_target_node_id.is_empty() || normalized_target_node_id == AGENT_NODE_MAIN_ID {
            return Err(ActorRelayError::permanent(
                TeamRemoteRelayError::InvalidRoute(
                    "route target_node_id must reference a registered remote agent node"
                        .to_string(),
                ),
            ));
        }

        let row = sqlx::query(
            r#"
            SELECT grpc_target, tls_server_name
            FROM agent_nodes
            WHERE id = ?1
            "#,
        )
        .bind(normalized_target_node_id)
        .fetch_optional(&self.db)
        .await
        .map_err(|err| {
            ActorRelayError::permanent(TeamRemoteRelayError::InvalidRoute(format!(
                "resolve registered gRPC route failed: {err}"
            )))
        })?
        .ok_or_else(|| {
            ActorRelayError::permanent(TeamRemoteRelayError::InvalidRoute(format!(
                "target agent node '{}' is not registered for gRPC relay",
                normalized_target_node_id
            )))
        })?;

        let registered_grpc_target = row.get::<String, _>("grpc_target");
        let registered_tls_server_name = row
            .try_get::<Option<String>, _>("tls_server_name")
            .ok()
            .flatten()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let requested_grpc_target = route.grpc_target.trim();
        if requested_grpc_target.is_empty() {
            return Err(ActorRelayError::permanent(
                TeamRemoteRelayError::MissingGrpcTarget,
            ));
        }
        let url = Url::parse(requested_grpc_target)
            .map_err(|_| ActorRelayError::permanent(TeamRemoteRelayError::InvalidGrpcTarget))?;
        if url.scheme() != "https" {
            return Err(ActorRelayError::permanent(
                TeamRemoteRelayError::InvalidGrpcTarget,
            ));
        }
        if requested_grpc_target != registered_grpc_target {
            return Err(ActorRelayError::permanent(
                TeamRemoteRelayError::InvalidRoute(format!(
                    "route.grpc_target must match registered agent node '{}'",
                    normalized_target_node_id
                )),
            ));
        }

        let requested_tls_server_name = route
            .tls_server_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if requested_tls_server_name != registered_tls_server_name.as_deref() {
            return Err(ActorRelayError::permanent(
                TeamRemoteRelayError::InvalidRoute(format!(
                    "route.tls_server_name must match registered agent node '{}'",
                    normalized_target_node_id
                )),
            ));
        }

        Ok(ResolvedGrpcRelayRoute {
            grpc_target: registered_grpc_target,
            tls_server_name: registered_tls_server_name,
        })
    }

    async fn grpc_client_for_route(
        &self,
        config: InternalGrpcMailboxClientConfig,
    ) -> Result<InternalGrpcMailboxClient, ActorRelayError<TeamRemoteRelayError>> {
        let cache_key = GrpcRelayClientCacheKey {
            target: config.target.clone(),
            access_token: config.access_token.clone(),
            ca_cert_path: config.ca_cert_path.clone(),
            tls_server_name: config.tls_server_name.clone(),
            client_cert_path: config.client_cert_path.clone(),
            client_key_path: config.client_key_path.clone(),
        };
        if let Some(client) = self
            .grpc_client_cache
            .lock()
            .expect("lock grpc client cache")
            .get(&cache_key)
            .cloned()
        {
            return Ok(client);
        }

        let client = InternalGrpcMailboxClient::connect(config)
            .await
            .map_err(|err| {
                ActorRelayError::retryable(TeamRemoteRelayError::GrpcConnect(err.to_string()))
            })?;
        self.grpc_client_cache
            .lock()
            .expect("lock grpc client cache")
            .insert(cache_key, client.clone());
        Ok(client)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GrpcRelayClientCacheKey {
    target: String,
    access_token: String,
    ca_cert_path: Option<String>,
    tls_server_name: Option<String>,
    client_cert_path: Option<String>,
    client_key_path: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum TeamRemoteRelayError {
    #[error("route is required for remote relay")]
    MissingRoute,
    #[error("route.endpoint is required for remote relay")]
    MissingEndpoint,
    #[error("route.endpoint must be a valid http/https URL")]
    InvalidEndpoint,
    #[error("route.grpc_target is required for remote relay")]
    MissingGrpcTarget,
    #[error("route.grpc_target must be a valid https URL")]
    InvalidGrpcTarget,
    #[error("route.access_token is required for gRPC relay")]
    MissingAccessToken,
    #[error("internal gRPC relay TLS defaults are unavailable")]
    GrpcTlsUnavailable,
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
    #[error("gRPC connect failed: {0}")]
    GrpcConnect(String),
    #[error("gRPC request failed: {0}")]
    GrpcRequest(String),
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
        match route {
            ParsedRemoteRelayRoute::Http(route) => self.deliver_http(message, route).await,
            ParsedRemoteRelayRoute::Grpc(route) => self.deliver_grpc(message, route).await,
        }
    }
}

#[derive(Debug, Deserialize)]
struct HttpRemoteRelayRouteValue {
    #[serde(default)]
    kind: Option<String>,
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
struct GrpcRemoteRelayRouteValue {
    #[serde(default)]
    kind: Option<String>,
    grpc_target: String,
    access_token: String,
    #[serde(default)]
    tls_server_name: Option<String>,
}

#[derive(Debug)]
struct ResolvedGrpcRelayRoute {
    grpc_target: String,
    tls_server_name: Option<String>,
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

#[derive(Debug)]
enum ParsedRemoteRelayRoute {
    Http(HttpRemoteRelayRouteValue),
    Grpc(GrpcRemoteRelayRouteValue),
}

fn parse_remote_route(
    route: &Value,
) -> Result<ParsedRemoteRelayRoute, ActorRelayError<TeamRemoteRelayError>> {
    let object = route.as_object().ok_or_else(|| {
        ActorRelayError::permanent(TeamRemoteRelayError::InvalidRoute(
            "route must be a JSON object".to_string(),
        ))
    })?;
    if object.contains_key("grpc_target") {
        for forbidden_key in ["ca_cert_path", "client_cert_path", "client_key_path"] {
            if object.contains_key(forbidden_key) {
                return Err(ActorRelayError::permanent(
                    TeamRemoteRelayError::InvalidRoute(format!(
                        "route.{forbidden_key} is not allowed for gRPC relay"
                    )),
                ));
            }
        }
        let parsed =
            serde_json::from_value::<GrpcRemoteRelayRouteValue>(route.clone()).map_err(|err| {
                ActorRelayError::permanent(TeamRemoteRelayError::InvalidRoute(err.to_string()))
            })?;
        if let Some(kind) = parsed.kind.as_deref()
            && kind.trim() != "grpc"
        {
            return Err(ActorRelayError::permanent(
                TeamRemoteRelayError::InvalidRoute(format!(
                    "route.kind '{}' is incompatible with grpc_target",
                    kind.trim()
                )),
            ));
        }
        return Ok(ParsedRemoteRelayRoute::Grpc(parsed));
    }
    if object.contains_key("endpoint") {
        let parsed =
            serde_json::from_value::<HttpRemoteRelayRouteValue>(route.clone()).map_err(|err| {
                ActorRelayError::permanent(TeamRemoteRelayError::InvalidRoute(err.to_string()))
            })?;
        if let Some(kind) = parsed.kind.as_deref()
            && kind.trim() != "http"
        {
            return Err(ActorRelayError::permanent(
                TeamRemoteRelayError::InvalidRoute(format!(
                    "route.kind '{}' is incompatible with endpoint",
                    kind.trim()
                )),
            ));
        }
        return Ok(ParsedRemoteRelayRoute::Http(parsed));
    }
    Err(ActorRelayError::permanent(
        TeamRemoteRelayError::InvalidRoute(
            "route must contain either endpoint or grpc_target".to_string(),
        ),
    ))
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
    let metadata = build_message_metadata(message);
    json!({
        "schema_version": 1,
        "message_id": message.message_id,
        "run_id": &message.run_id,
        "cluster_id": metadata.cluster_id,
        "source_node_id": metadata.source_node_id,
        "target_node_id": metadata.target_node_id,
        "broadcast_id": metadata.broadcast_id,
        "correlation_id": metadata.correlation_id,
        "idempotency_key": metadata.idempotency_key,
        "scope": metadata.scope,
        "audience": metadata.audience,
        "issued_at": metadata.issued_at,
        "expires_at": metadata.expires_at,
        "kid": metadata.kid,
        "payload_digest": metadata.payload_digest,
        "from_actor_id": &message.from_actor_id,
        "from_peer_id": &message.from_peer_id,
        "from_actor_kind": &message.from_actor_kind,
        "to_actor_id": &message.to_actor_id,
        "to_peer_id": &message.to_peer_id,
        "to_actor_kind": &message.to_actor_kind,
        "channel": &message.channel,
        "transport": message.transport.as_str(),
        "created_at": message.created_at,
        "payload": &message.payload,
    })
}

fn map_grpc_actor_error(err: ActorServiceError) -> ActorRelayError<TeamRemoteRelayError> {
    let ActorServiceError { code, message } = err;
    let relay_error = TeamRemoteRelayError::GrpcRequest(message);
    match code {
        ActorServiceErrorCode::TooManyRequests | ActorServiceErrorCode::Internal => {
            ActorRelayError::retryable(relay_error)
        }
        _ => ActorRelayError::permanent(relay_error),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GrpcRelayTlsDefaults, ParsedRemoteRelayRoute, RELAY_DEFAULT_TIMEOUT_MS,
        RELAY_TIMEOUT_MAX_MS, RELAY_TIMEOUT_MIN_MS, RemoteRelaySigningValue,
        TeamRemoteRelayAdapter, apply_route_signing, parse_remote_route, parse_route_header_name,
        parse_route_header_value, relay_timeout_ms,
    };
    use crate::internal::p2p::NodeTransportMetadata;
    use crate::team::{TeamActorMessageRecord, TeamActorMessageStatus, TeamActorMessageTransport};
    use agenthub_team_actor::{ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, ActorIdentityKind};
    use chrono::Utc;
    use serde_json::json;
    use sqlx::SqlitePool;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn test_db() -> SqlitePool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect sqlite")
    }

    async fn create_agent_nodes_table(db: &SqlitePool) {
        sqlx::query(
            r#"
            CREATE TABLE agent_nodes (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                grpc_target TEXT NOT NULL,
                tls_server_name TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create agent_nodes table");
    }

    async fn insert_agent_node(
        db: &SqlitePool,
        node_id: &str,
        grpc_target: &str,
        tls_server_name: Option<&str>,
    ) {
        sqlx::query(
            r#"
            INSERT INTO agent_nodes (id, name, grpc_target, tls_server_name, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, 1, 1)
            "#,
        )
        .bind(node_id)
        .bind(format!("Node {node_id}"))
        .bind(grpc_target)
        .bind(tls_server_name)
        .execute(db)
        .await
        .expect("insert agent node");
    }

    fn grpc_route_value(
        grpc_target: &str,
        access_token: &str,
        tls_server_name: &str,
    ) -> super::GrpcRemoteRelayRouteValue {
        super::GrpcRemoteRelayRouteValue {
            kind: Some("grpc".to_string()),
            grpc_target: grpc_target.to_string(),
            access_token: access_token.to_string(),
            tls_server_name: Some(tls_server_name.to_string()),
        }
    }

    fn test_remote_message(
        target_node_id: &str,
        grpc_target: &str,
        tls_server_name: &str,
    ) -> TeamActorMessageRecord {
        let mut route = serde_json::Map::new();
        route.insert("kind".to_string(), json!("grpc"));
        route.insert("grpc_target".to_string(), json!(grpc_target));
        route.insert("access_token".to_string(), json!("secret-token"));
        route.insert("tls_server_name".to_string(), json!(tls_server_name));
        NodeTransportMetadata {
            cluster_id: "agenthub".to_string(),
            source_node_id: "node-a".to_string(),
            target_node_id: target_node_id.to_string(),
            broadcast_id: None,
            correlation_id: None,
            idempotency_key: Some("relay-test".to_string()),
            scope: vec!["node:p2p".to_string()],
            audience: vec!["agenthub-internal".to_string()],
            issued_at: Utc::now().timestamp(),
            expires_at: Utc::now().timestamp() + 600,
            kid: "shared-hs256-test".to_string(),
            payload_digest: None,
        }
        .apply_to_route(&mut route);

        TeamActorMessageRecord {
            message_id: 1,
            run_id: "run-1".to_string(),
            from_actor_id: "planner".to_string(),
            from_peer_id: "node-a".to_string(),
            from_actor_kind: ActorIdentityKind::Agent,
            to_actor_id: "reviewer".to_string(),
            to_peer_id: ACTOR_NODE_PEER_ID.to_string(),
            to_actor_kind: ActorIdentityKind::Agent,
            channel: "coordination".to_string(),
            transport: TeamActorMessageTransport::Remote,
            route: Some(json!(route)),
            payload: json!({"type":"chat_message","text":"hello"}),
            status: TeamActorMessageStatus::Pending,
            created_at: Utc::now().timestamp(),
            delivered_at: None,
        }
    }

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
    fn parse_remote_route_accepts_grpc_variant() {
        let parsed = parse_remote_route(&json!({
            "kind": "grpc",
            "grpc_target": "https://node.example.internal:50051",
            "access_token": "secret-token",
            "tls_server_name": "node.example.internal",
        }))
        .expect("parse grpc route");
        match parsed {
            ParsedRemoteRelayRoute::Grpc(route) => {
                assert_eq!(route.grpc_target, "https://node.example.internal:50051");
                assert_eq!(route.access_token, "secret-token");
                assert_eq!(
                    route.tls_server_name.as_deref(),
                    Some("node.example.internal")
                );
            }
            ParsedRemoteRelayRoute::Http(_) => panic!("expected grpc route"),
        }
    }

    #[test]
    fn parse_remote_route_rejects_path_based_tls_fields_for_grpc() {
        let err = parse_remote_route(&json!({
            "kind": "grpc",
            "grpc_target": "https://node.example.internal:50051",
            "access_token": "secret-token",
            "tls_server_name": "node.example.internal",
            "ca_cert_path": "/tmp/ca.pem",
        }))
        .expect_err("gRPC relay route should reject path-based TLS overrides");
        assert!(
            err.to_string()
                .contains("route.ca_cert_path is not allowed"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn grpc_relay_requires_registered_target_node_match() {
        let db = test_db().await;
        create_agent_nodes_table(&db).await;
        insert_agent_node(
            &db,
            "node-b",
            "https://node-b.internal:50051",
            Some("node-b.internal"),
        )
        .await;

        let adapter = TeamRemoteRelayAdapter::new(db);
        adapter.configure_grpc_tls_defaults(Some(GrpcRelayTlsDefaults {
            ca_cert_path: Some("/tmp/ca.pem".to_string()),
            client_cert_path: None,
            client_key_path: None,
        }));

        let route = super::GrpcRemoteRelayRouteValue {
            kind: Some("grpc".to_string()),
            grpc_target: "https://node-b.internal:50051".to_string(),
            access_token: "secret-token".to_string(),
            tls_server_name: Some("node-b.internal".to_string()),
        };

        adapter
            .resolve_registered_grpc_route("node-b", &route)
            .await
            .expect("registered node should resolve");

        let mismatch = super::GrpcRemoteRelayRouteValue {
            grpc_target: "https://evil.example:50051".to_string(),
            ..route
        };
        let err = adapter
            .resolve_registered_grpc_route("node-b", &mismatch)
            .await
            .expect_err("mismatched grpc target should fail");
        assert!(
            err.to_string()
                .contains("route.grpc_target must match registered agent node"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn grpc_relay_rejects_main_target_node() {
        let db = test_db().await;
        create_agent_nodes_table(&db).await;
        let adapter = TeamRemoteRelayAdapter::new(db);
        let err = adapter
            .resolve_registered_grpc_route(
                ACTOR_MAIN_PEER_ID,
                &grpc_route_value(
                    "https://node-b.internal:50051",
                    "secret-token",
                    "node-b.internal",
                ),
            )
            .await
            .expect_err("main node target should be rejected");
        assert!(
            err.to_string()
                .contains("must reference a registered remote agent node"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn grpc_relay_requires_registered_tls_server_name_match() {
        let db = test_db().await;
        create_agent_nodes_table(&db).await;
        insert_agent_node(
            &db,
            "node-b",
            "https://node-b.internal:50051",
            Some("node-b.internal"),
        )
        .await;
        let adapter = TeamRemoteRelayAdapter::new(db);
        let err = adapter
            .resolve_registered_grpc_route(
                "node-b",
                &grpc_route_value(
                    "https://node-b.internal:50051",
                    "secret-token",
                    "wrong.internal",
                ),
            )
            .await
            .expect_err("tls server name mismatch should fail");
        assert!(
            err.to_string()
                .contains("route.tls_server_name must match registered agent node"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn grpc_relay_rejects_unregistered_target_node() {
        let db = test_db().await;
        create_agent_nodes_table(&db).await;
        let adapter = TeamRemoteRelayAdapter::new(db);
        let err = adapter
            .resolve_registered_grpc_route(
                "node-missing",
                &grpc_route_value(
                    "https://node-b.internal:50051",
                    "secret-token",
                    "node-b.internal",
                ),
            )
            .await
            .expect_err("missing registry entry should fail");
        assert!(
            err.to_string().contains("is not registered for gRPC relay"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn deliver_grpc_requires_cluster_tls_defaults() {
        let db = test_db().await;
        create_agent_nodes_table(&db).await;
        insert_agent_node(
            &db,
            "node-b",
            "https://node-b.internal:50051",
            Some("node-b.internal"),
        )
        .await;
        let adapter = TeamRemoteRelayAdapter::new(db);
        let message =
            test_remote_message("node-b", "https://node-b.internal:50051", "node-b.internal");
        let err = adapter
            .deliver_grpc(
                &message,
                grpc_route_value(
                    "https://node-b.internal:50051",
                    "secret-token",
                    "node-b.internal",
                ),
            )
            .await
            .expect_err("missing tls defaults should fail");
        assert!(
            err.to_string().contains("TLS defaults are unavailable"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn deliver_grpc_requires_access_token() {
        let db = test_db().await;
        create_agent_nodes_table(&db).await;
        insert_agent_node(
            &db,
            "node-b",
            "https://node-b.internal:50051",
            Some("node-b.internal"),
        )
        .await;
        let adapter = TeamRemoteRelayAdapter::new(db);
        adapter.configure_grpc_tls_defaults(Some(GrpcRelayTlsDefaults {
            ca_cert_path: Some("/tmp/ca.pem".to_string()),
            client_cert_path: None,
            client_key_path: None,
        }));
        let message =
            test_remote_message("node-b", "https://node-b.internal:50051", "node-b.internal");
        let err = adapter
            .deliver_grpc(
                &message,
                grpc_route_value("https://node-b.internal:50051", "   ", "node-b.internal"),
            )
            .await
            .expect_err("missing access token should fail");
        assert!(
            err.to_string().contains("route.access_token is required"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn grpc_relay_tls_defaults_follow_security_mode() {
        let dir = std::env::temp_dir().join(format!(
            "agenthub-remote-relay-tls-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(dir.join("ca-cert.pem"), "ca").expect("write ca");
        std::fs::write(dir.join("client-cert.pem"), "cert").expect("write client cert");
        std::fs::write(dir.join("client-key.pem"), "key").expect("write client key");

        let tls_only = GrpcRelayTlsDefaults::from_cert_dir(
            &dir,
            crate::internal::tls::InternalGrpcSecurityMode::Tls,
        );
        assert!(tls_only.ca_cert_path.is_some());
        assert!(tls_only.client_cert_path.is_none());
        assert!(tls_only.client_key_path.is_none());

        let mtls = GrpcRelayTlsDefaults::from_cert_dir(
            &dir,
            crate::internal::tls::InternalGrpcSecurityMode::Mtls,
        );
        assert!(mtls.ca_cert_path.is_some());
        assert!(mtls.client_cert_path.is_some());
        assert!(mtls.client_key_path.is_some());

        let _ = std::fs::remove_dir_all(dir);
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
