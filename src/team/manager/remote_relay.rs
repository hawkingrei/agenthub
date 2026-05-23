use agenthub_team_actor::{ActorMessageRelay, ActorRelayError};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::agent::AGENT_NODE_MAIN_ID;
use crate::internal::client::{InternalGrpcMailboxClient, InternalGrpcPeerClientConfig};
use crate::internal::tls::InternalGrpcSecurityMode;
use crate::team::TeamActorMessageRecord;

use super::remote_relay_route::parse_remote_route;

pub(super) const RELAY_DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub(super) const RELAY_TIMEOUT_MIN_MS: u64 = 100;
pub(super) const RELAY_TIMEOUT_MAX_MS: u64 = 60_000;

#[derive(Clone)]
pub(super) struct TeamRemoteRelayAdapter {
    pub(super) db: SqlitePool,
    pub(super) http_client: reqwest::Client,
    pub(super) grpc_tls_defaults: Arc<Mutex<Option<GrpcRelayTlsDefaults>>>,
    pub(super) grpc_peer_client_config: Arc<Mutex<Option<InternalGrpcPeerClientConfig>>>,
    pub(super) grpc_client_cache:
        Arc<Mutex<HashMap<GrpcRelayClientCacheKey, InternalGrpcMailboxClient>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GrpcRelayTlsDefaults {
    pub(super) ca_cert_path: Option<String>,
    pub(super) client_cert_path: Option<String>,
    pub(super) client_key_path: Option<String>,
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
        let _peer_config = self
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

        let mut route = serde_json::Map::new();
        route.insert("kind".to_string(), json!("grpc"));
        route.insert("grpc_target".to_string(), json!(grpc_target));
        route.insert(
            "target_node_id".to_string(),
            json!(normalized_target_node_id),
        );
        if let Some(value) = tls_server_name.as_deref() {
            route.insert("tls_server_name".to_string(), json!(value));
        }

        Ok(Value::Object(route))
    }
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

#[cfg(test)]
mod tests;
