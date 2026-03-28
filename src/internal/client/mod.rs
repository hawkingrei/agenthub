mod control;
mod mailbox;

#[cfg(test)]
mod tests;

use std::path::Path;

use crate::agent::{AgentEvent, OutputStream};
use agenthub_team_actor::{ActorServiceError, ActorServiceErrorCode};
use tonic::{
    Request,
    metadata::MetadataValue,
    transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity},
};

use super::auth::{InternalAuthz, InternalAuthzConfig, InternalRole};
use super::p2p::{CredentialProvider, NodeCredentialRequest};
use super::proto::agenthub::internal::v1::ListAgentEventsResponse as GrpcListAgentEventsResponse;
use super::proto::agenthub::internal::v1::team_internal_control_client::TeamInternalControlClient;
use super::tls::{InternalGrpcSecurityMode, install_rustls_crypto_provider};

pub use mailbox::normalize_existing_path;

#[derive(Debug, Clone)]
pub struct InternalGrpcMailboxClientConfig {
    pub target: String,
    pub access_token: String,
    pub ca_cert_path: Option<String>,
    pub tls_server_name: Option<String>,
    pub client_cert_path: Option<String>,
    pub client_key_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InternalGrpcPeerClientConfig {
    pub shared_secret: String,
    pub expected_issuer: Option<String>,
    pub expected_audience: Option<String>,
    pub source_node_id: String,
    pub cert_dir: String,
    pub security_mode: InternalGrpcSecurityMode,
}

#[derive(Clone)]
pub struct InternalGrpcMailboxClient {
    channel: Channel,
    access_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct InternalPermissionReviewResponse {
    pub status: String,
    pub permission_id: String,
    pub request_status: String,
    pub reviewed_by_actor_id: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct InternalTeamTaskPatch<'a> {
    pub status: Option<&'a str>,
    pub assigned_member_id: Option<&'a str>,
    pub clear_assigned_member_id: bool,
    pub context_json: Option<&'a serde_json::Value>,
    pub context_merge_json: Option<&'a serde_json::Value>,
}

impl InternalGrpcMailboxClient {
    pub async fn connect_peer(
        config: &InternalGrpcPeerClientConfig,
        target: &str,
        tls_server_name: Option<&str>,
        permissions: Vec<String>,
    ) -> anyhow::Result<Self> {
        let authz = InternalAuthz::new(InternalAuthzConfig {
            shared_secret: config.shared_secret.clone(),
            expected_issuer: config.expected_issuer.clone(),
            expected_audience: config.expected_audience.clone(),
        });
        let access_token = authz
            .issue_node_access_token(NodeCredentialRequest {
                source_node_id: config.source_node_id.clone(),
                role: InternalRole::Leader.as_str().to_string(),
                actor_id: None,
                run_id: None,
                permissions,
                scope: Vec::new(),
                audience: Vec::new(),
                ttl_seconds: 600,
            })?
            .access_token;
        let cert_dir = Path::new(&config.cert_dir);
        let ca_cert_path = tls_path_if_exists(cert_dir.join("ca-cert.pem"));
        let client_cert_path = tls_path_if_exists(cert_dir.join("client-cert.pem"));
        let client_key_path = tls_path_if_exists(cert_dir.join("client-key.pem"));
        let (client_cert_path, client_key_path) =
            if config.security_mode == InternalGrpcSecurityMode::Mtls {
                (client_cert_path, client_key_path)
            } else {
                (None, None)
            };
        Self::connect(InternalGrpcMailboxClientConfig {
            target: target.trim().to_string(),
            access_token,
            ca_cert_path,
            tls_server_name: tls_server_name.map(str::to_string),
            client_cert_path,
            client_key_path,
        })
        .await
    }

    pub async fn connect(config: InternalGrpcMailboxClientConfig) -> anyhow::Result<Self> {
        install_rustls_crypto_provider();
        let target = config.target.trim().to_string();
        let mut endpoint = Endpoint::from_shared(target.clone())?;
        if target
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
        {
            let mut tls = ClientTlsConfig::new();
            if let Some(server_name) = config
                .tls_server_name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                tls = tls.domain_name(server_name.to_string());
            }
            if let Some(ca_cert_path) = config
                .ca_cert_path
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let pem = std::fs::read(ca_cert_path)?;
                tls = tls.ca_certificate(Certificate::from_pem(pem));
            }
            if let (Some(client_cert_path), Some(client_key_path)) = (
                config
                    .client_cert_path
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty()),
                config
                    .client_key_path
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty()),
            ) {
                let cert_pem = std::fs::read(client_cert_path)?;
                let key_pem = std::fs::read(client_key_path)?;
                tls = tls.identity(Identity::from_pem(cert_pem, key_pem));
            }
            endpoint = endpoint.tls_config(tls)?;
        }
        let channel = endpoint.connect().await?;
        Ok(Self {
            channel,
            access_token: config.access_token.trim().to_string(),
        })
    }

    fn client(&self) -> TeamInternalControlClient<Channel> {
        TeamInternalControlClient::new(self.channel.clone())
    }

    fn request<T>(&self, payload: T) -> Result<Request<T>, ActorServiceError> {
        let metadata = MetadataValue::try_from(format!("Bearer {}", self.access_token.trim()))
            .map_err(|err| {
                ActorServiceError::new(
                    ActorServiceErrorCode::Unauthorized,
                    format!("invalid internal access token metadata: {}", err),
                )
            })?;
        let mut request = Request::new(payload);
        request.metadata_mut().insert("authorization", metadata);
        Ok(request)
    }

    fn control_request<T>(&self, payload: T) -> anyhow::Result<Request<T>> {
        self.request(payload)
            .map_err(|err| anyhow::anyhow!("{:?}: {}", err.code, err.message))
    }
}

fn parse_json_response<T>(raw: &str, field: &str) -> anyhow::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(raw).map_err(|err| anyhow::anyhow!("decode {field}: {err}"))
}

fn parse_output_stream(raw: &str) -> OutputStream {
    match raw.trim() {
        "stderr" => OutputStream::Stderr,
        "system" => OutputStream::System,
        "acp" => OutputStream::Acp,
        _ => OutputStream::Stdout,
    }
}

fn map_grpc_status_anyhow(status: tonic::Status) -> anyhow::Error {
    anyhow::anyhow!("internal gRPC {}: {}", status.code(), status.message())
}

fn tls_path_if_exists(path: impl AsRef<Path>) -> Option<String> {
    let path = path.as_ref();
    path.exists()
        .then(|| path.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
}

fn parse_agent_events(response: GrpcListAgentEventsResponse) -> anyhow::Result<Vec<AgentEvent>> {
    let mut events = Vec::with_capacity(response.events.len());
    for event in response.events {
        events.push(AgentEvent {
            event_id: event.event_id,
            agent_id: event.agent_id,
            session_id: event.session_id,
            seq: event.seq,
            ts: event.ts,
            stream: parse_output_stream(&event.stream),
            message: event.message,
        });
    }
    Ok(events)
}
