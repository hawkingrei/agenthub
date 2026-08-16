use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::internal::client::{InternalGrpcMailboxClient, InternalGrpcPeerClientConfig};

pub(super) const RELAY_DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub(super) const RELAY_TIMEOUT_MIN_MS: u64 = 100;
pub(super) const RELAY_TIMEOUT_MAX_MS: u64 = 60_000;

/// Kept below the 600s TTL that `issue_registered_grpc_access_token` mints so cached clients are
/// dropped, and a fresh access token obtained, before the token they were built with expires.
pub(super) const RELAY_GRPC_CLIENT_CACHE_TTL: Duration = Duration::from_secs(240);

#[derive(Clone)]
pub(super) struct TeamRemoteRelayAdapter {
    pub(super) db: SqlitePool,
    pub(super) http_client: Client,
    pub(super) grpc_tls_defaults: Arc<Mutex<Option<GrpcRelayTlsDefaults>>>,
    pub(super) grpc_peer_client_config: Arc<Mutex<Option<InternalGrpcPeerClientConfig>>>,
    pub(super) grpc_client_cache:
        Arc<Mutex<HashMap<GrpcRelayClientCacheKey, CachedGrpcRelayClient>>>,
}

#[derive(Clone)]
pub(super) struct CachedGrpcRelayClient {
    pub(super) client: InternalGrpcMailboxClient,
    pub(super) inserted_at: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GrpcRelayTlsDefaults {
    pub(super) ca_cert_path: Option<String>,
    pub(super) client_cert_path: Option<String>,
    pub(super) client_key_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct GrpcRelayClientCacheKey {
    pub(super) target: String,
    pub(super) access_token: String,
    pub(super) ca_cert_path: Option<String>,
    pub(super) tls_server_name: Option<String>,
    pub(super) client_cert_path: Option<String>,
    pub(super) client_key_path: Option<String>,
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
    #[error("internal gRPC peer client config is unavailable")]
    GrpcPeerClientUnavailable,
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

#[derive(Debug, Deserialize)]
pub(super) struct HttpRemoteRelayRouteValue {
    #[serde(default)]
    pub(super) kind: Option<String>,
    pub(super) endpoint: String,
    #[serde(default)]
    pub(super) method: Option<String>,
    #[serde(default)]
    pub(super) headers: HashMap<String, String>,
    #[serde(default)]
    pub(super) timeout_ms: Option<u64>,
    #[serde(default)]
    pub(super) auth: Option<RemoteRelayAuthValue>,
    #[serde(default)]
    pub(super) signing: Option<RemoteRelaySigningValue>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GrpcRemoteRelayRouteValue {
    #[serde(default)]
    pub(super) kind: Option<String>,
    pub(super) grpc_target: String,
    #[serde(default)]
    pub(super) access_token: Option<String>,
    #[serde(default)]
    pub(super) tls_server_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum RemoteRelayAuthValue {
    Bearer { token: String },
    Header { name: String, value: String },
    Basic { username: String, password: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum RemoteRelaySigningValue {
    HmacSha256 {
        secret: String,
        #[serde(default)]
        header: Option<String>,
        #[serde(default)]
        timestamp_header: Option<String>,
    },
}

#[derive(Debug)]
pub(super) enum ParsedRemoteRelayRoute {
    Http(HttpRemoteRelayRouteValue),
    Grpc(GrpcRemoteRelayRouteValue),
}
