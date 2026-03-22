use std::path::Path;

use crate::acp::AcpActorSkillContext;
use crate::agent::{AgentConfig, AgentEvent, AgentRecord, OutputStream};
use agenthub_team_actor::{
    ActorAckRequest, ActorAckResponse, ActorInboxRequest, ActorInboxResponse, ActorMailboxService,
    ActorMessageRecord, ActorMessageStatus, ActorMessageTransport, ActorSendRequest,
    ActorSendResponse, ActorServiceError, ActorServiceErrorCode,
};
use async_trait::async_trait;
use tonic::{
    Code, Request,
    metadata::MetadataValue,
    transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity},
};

use super::auth::{InternalAuthz, InternalAuthzConfig, InternalRole};
use super::p2p::{CredentialProvider, NodeCredentialRequest, P2PTransport};
use super::proto::agenthub::internal::v1::team_internal_control_client::TeamInternalControlClient;
use super::proto::agenthub::internal::v1::{
    AckActorMessageRequest as GrpcAckActorMessageRequest,
    DeleteManagedAgentRequest as GrpcDeleteManagedAgentRequest,
    EnsureAgentRecordRequest as GrpcEnsureAgentRecordRequest,
    GetAgentRecordRequest as GrpcGetAgentRecordRequest,
    ListActorInboxRequest as GrpcListActorInboxRequest,
    ListAgentEventsRequest as GrpcListAgentEventsRequest,
    ListAgentEventsResponse as GrpcListAgentEventsResponse,
    SendActorMessageRequest as GrpcSendActorMessageRequest,
    SendAgentInputRequest as GrpcSendAgentInputRequest,
    StartManagedAgentRequest as GrpcStartManagedAgentRequest,
    StopManagedAgentRequest as GrpcStopManagedAgentRequest,
};
use super::tls::{InternalGrpcSecurityMode, install_rustls_crypto_provider};

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
        let mut endpoint = Endpoint::from_shared(config.target.trim().to_string())?;
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

    pub async fn ensure_agent_record(
        &self,
        agent_id: &str,
        config: &AgentConfig,
        source: &str,
    ) -> anyhow::Result<AgentRecord> {
        let mut client = self.client();
        let response = client
            .ensure_agent_record(self.control_request(GrpcEnsureAgentRecordRequest {
                agent_id: agent_id.trim().to_string(),
                config_json: serde_json::to_string(config)?,
                source: source.trim().to_string(),
            })?)
            .await
            .map_err(map_grpc_status_anyhow)?
            .into_inner();
        Ok(serde_json::from_str(&response.agent_json)?)
    }

    #[allow(dead_code)]
    pub async fn get_agent_record(&self, agent_id: &str) -> anyhow::Result<AgentRecord> {
        let mut client = self.client();
        let response = client
            .get_agent_record(self.control_request(GrpcGetAgentRecordRequest {
                agent_id: agent_id.trim().to_string(),
            })?)
            .await
            .map_err(map_grpc_status_anyhow)?
            .into_inner();
        Ok(serde_json::from_str(&response.agent_json)?)
    }

    pub async fn start_managed_agent(
        &self,
        agent_id: &str,
        actor_context: Option<&AcpActorSkillContext>,
    ) -> anyhow::Result<String> {
        let mut client = self.client();
        let response = client
            .start_managed_agent(
                self.control_request(GrpcStartManagedAgentRequest {
                    agent_id: agent_id.trim().to_string(),
                    actor_context_json: actor_context
                        .map(serde_json::to_string)
                        .transpose()?
                        .unwrap_or_default(),
                })?,
            )
            .await
            .map_err(map_grpc_status_anyhow)?
            .into_inner();
        Ok(response.session_id)
    }

    pub async fn stop_managed_agent(&self, agent_id: &str) -> anyhow::Result<()> {
        let mut client = self.client();
        client
            .stop_managed_agent(self.control_request(GrpcStopManagedAgentRequest {
                agent_id: agent_id.trim().to_string(),
            })?)
            .await
            .map_err(map_grpc_status_anyhow)?;
        Ok(())
    }

    pub async fn delete_managed_agent(&self, agent_id: &str) -> anyhow::Result<()> {
        let mut client = self.client();
        client
            .delete_managed_agent(self.control_request(GrpcDeleteManagedAgentRequest {
                agent_id: agent_id.trim().to_string(),
            })?)
            .await
            .map_err(map_grpc_status_anyhow)?;
        Ok(())
    }

    pub async fn send_agent_input(
        &self,
        agent_id: &str,
        input: &str,
        message_id: Option<&str>,
        session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let mut client = self.client();
        client
            .send_agent_input(self.control_request(GrpcSendAgentInputRequest {
                agent_id: agent_id.trim().to_string(),
                input: input.to_string(),
                message_id: message_id.unwrap_or_default().to_string(),
                session_id: session_id.unwrap_or_default().to_string(),
            })?)
            .await
            .map_err(map_grpc_status_anyhow)?;
        Ok(())
    }

    pub async fn list_agent_events(
        &self,
        agent_id: &str,
        limit: i64,
        session_id: Option<&str>,
        before_event_id: Option<i64>,
    ) -> anyhow::Result<Vec<AgentEvent>> {
        let mut client = self.client();
        let response = client
            .list_agent_events(self.control_request(GrpcListAgentEventsRequest {
                agent_id: agent_id.trim().to_string(),
                limit,
                before_event_id: before_event_id.unwrap_or_default(),
                session_id: session_id.unwrap_or_default().to_string(),
            })?)
            .await
            .map_err(map_grpc_status_anyhow)?
            .into_inner();
        parse_agent_events(response)
    }
}

fn parse_transport(raw: &str) -> ActorMessageTransport {
    match raw.trim() {
        "remote" => ActorMessageTransport::Remote,
        _ => ActorMessageTransport::Local,
    }
}

fn parse_output_stream(raw: &str) -> OutputStream {
    match raw.trim() {
        "stderr" => OutputStream::Stderr,
        "system" => OutputStream::System,
        "acp" => OutputStream::Acp,
        _ => OutputStream::Stdout,
    }
}

fn parse_status(raw: &str) -> ActorMessageStatus {
    match raw.trim() {
        "delivered" => ActorMessageStatus::Delivered,
        "dead_letter" => ActorMessageStatus::DeadLetter,
        _ => ActorMessageStatus::Pending,
    }
}

fn parse_message(
    message: super::proto::agenthub::internal::v1::ActorMessage,
) -> Result<ActorMessageRecord, ActorServiceError> {
    let from_actor_kind = agenthub_team_actor::infer_actor_identity_kind(&message.from_actor_id);
    let to_actor_kind = agenthub_team_actor::infer_actor_identity_kind(&message.to_actor_id);
    let route = if message.route_json.trim().is_empty() {
        None
    } else {
        Some(serde_json::from_str(&message.route_json).map_err(|err| {
            ActorServiceError::new(
                ActorServiceErrorCode::Internal,
                format!("decode route_json: {}", err),
            )
        })?)
    };
    let payload = serde_json::from_str(&message.payload_json).map_err(|err| {
        ActorServiceError::new(
            ActorServiceErrorCode::Internal,
            format!("decode payload_json: {}", err),
        )
    })?;
    Ok(ActorMessageRecord {
        message_id: message.message_id,
        run_id: message.run_id,
        from_actor_id: message.from_actor_id,
        from_peer_id: if message.from_peer_id.trim().is_empty() {
            "main".to_string()
        } else {
            message.from_peer_id
        },
        from_actor_kind,
        to_actor_id: message.to_actor_id,
        to_peer_id: if message.to_peer_id.trim().is_empty() {
            "main".to_string()
        } else {
            message.to_peer_id
        },
        to_actor_kind,
        channel: message.channel,
        transport: parse_transport(&message.transport),
        route,
        payload,
        status: parse_status(&message.status),
        created_at: message.created_at,
        delivered_at: (message.delivered_at > 0).then_some(message.delivered_at),
    })
}

fn map_grpc_status(status: tonic::Status) -> ActorServiceError {
    let code = match status.code() {
        Code::InvalidArgument => ActorServiceErrorCode::BadRequest,
        Code::Unauthenticated => ActorServiceErrorCode::Unauthorized,
        Code::PermissionDenied => ActorServiceErrorCode::Forbidden,
        Code::NotFound => ActorServiceErrorCode::NotFound,
        Code::AlreadyExists | Code::Aborted => ActorServiceErrorCode::Conflict,
        Code::FailedPrecondition => ActorServiceErrorCode::Gone,
        Code::ResourceExhausted => ActorServiceErrorCode::TooManyRequests,
        Code::Unavailable | Code::DeadlineExceeded => ActorServiceErrorCode::TooManyRequests,
        _ => ActorServiceErrorCode::Internal,
    };
    ActorServiceError::new(code, status.message().to_string())
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

#[async_trait]
impl ActorMailboxService for InternalGrpcMailboxClient {
    async fn actor_send(
        &self,
        request: ActorSendRequest,
    ) -> Result<ActorSendResponse, ActorServiceError> {
        let from_actor_kind =
            agenthub_team_actor::infer_actor_identity_kind(&request.from_actor_id);
        let to_actor_id = request.to_actor_id.clone().unwrap_or_default();
        let to_actor_kind = agenthub_team_actor::infer_actor_identity_kind(&to_actor_id);
        let request_channel = request.channel.clone();
        let request_transport = request.transport.clone();
        let request_channel_id = request.channel_id.clone();
        let grpc_channel = request_channel
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let grpc_transport = request_transport
            .clone()
            .unwrap_or(ActorMessageTransport::Local)
            .as_str()
            .to_string();
        let payload_json = serde_json::to_string(&request.payload).map_err(|err| {
            ActorServiceError::new(
                ActorServiceErrorCode::BadRequest,
                format!("serialize payload: {}", err),
            )
        })?;
        let route_json = request
            .route
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|err| {
                ActorServiceError::new(
                    ActorServiceErrorCode::BadRequest,
                    format!("serialize route: {}", err),
                )
            })?
            .unwrap_or_default();
        let mut client = self.client();
        let response = client
            .send_actor_message(self.request(GrpcSendActorMessageRequest {
                run_id: request.run_id.clone(),
                from_actor_id: request.from_actor_id.clone(),
                to_actor_id: to_actor_id.clone(),
                channel: grpc_channel,
                transport: grpc_transport,
                route_json,
                payload_json,
                idempotency_key: request.idempotency_key.unwrap_or_default(),
                from_peer_id: request.from_peer_id.clone().unwrap_or_default(),
                to_peer_id: request.to_peer_id.clone().unwrap_or_default(),
                channel_id: request_channel_id.unwrap_or_default(),
            })?)
            .await
            .map_err(map_grpc_status)?
            .into_inner();
        Ok(ActorSendResponse {
            message_id: response.message_id,
            state: parse_status(&response.status),
            deduped: false,
            created_at: 0,
            message: ActorMessageRecord {
                message_id: response.message_id,
                run_id: request.run_id,
                from_actor_id: request.from_actor_id,
                from_peer_id: request.from_peer_id.unwrap_or_else(|| "main".to_string()),
                from_actor_kind,
                to_actor_id,
                to_peer_id: request.to_peer_id.unwrap_or_else(|| "main".to_string()),
                to_actor_kind,
                channel: request_channel.unwrap_or_else(|| "default".to_string()),
                transport: request_transport.unwrap_or(ActorMessageTransport::Local),
                route: request.route,
                payload: request.payload,
                status: parse_status(&response.status),
                created_at: 0,
                delivered_at: None,
            },
        })
    }

    async fn actor_inbox(
        &self,
        request: ActorInboxRequest,
    ) -> Result<ActorInboxResponse, ActorServiceError> {
        let mut client = self.client();
        let response = client
            .list_actor_inbox(
                self.request(GrpcListActorInboxRequest {
                    run_id: request.run_id,
                    actor_id: request.actor_id,
                    limit: request.limit.unwrap_or(20),
                    after_message_id: request.cursor.unwrap_or_default(),
                    include_delivered: request
                        .states
                        .as_ref()
                        .is_some_and(|states| states.contains(&ActorMessageStatus::Delivered)),
                })?,
            )
            .await
            .map_err(map_grpc_status)?
            .into_inner();
        let mut messages = Vec::with_capacity(response.messages.len());
        for message in response.messages {
            messages.push(parse_message(message)?);
        }
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
        let mut client = self.client();
        let response = client
            .ack_actor_message(self.request(GrpcAckActorMessageRequest {
                run_id: request.run_id,
                actor_id: request.actor_id,
                message_id: request.message_id,
            })?)
            .await
            .map_err(map_grpc_status)?
            .into_inner();
        let message = response.message.ok_or_else(|| {
            ActorServiceError::new(ActorServiceErrorCode::Internal, "missing ack message")
        })?;
        let message = parse_message(message)?;
        Ok(ActorAckResponse {
            message_id: message.message_id,
            state: message.status.clone(),
            acked_at: message.delivered_at.unwrap_or(message.created_at),
            message,
        })
    }
}

#[async_trait]
impl P2PTransport for InternalGrpcMailboxClient {
    async fn send_p2p_message(
        &self,
        request: ActorSendRequest,
    ) -> Result<ActorSendResponse, ActorServiceError> {
        self.actor_send(request).await
    }

    async fn list_p2p_inbox(
        &self,
        request: ActorInboxRequest,
    ) -> Result<ActorInboxResponse, ActorServiceError> {
        self.actor_inbox(request).await
    }

    async fn ack_p2p_message(
        &self,
        request: ActorAckRequest,
    ) -> Result<ActorAckResponse, ActorServiceError> {
        self.actor_ack(request).await
    }
}

pub fn normalize_existing_path(
    raw: Option<&str>,
    field_name: &str,
) -> anyhow::Result<Option<String>> {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let path = Path::new(value);
    if !path.exists() {
        anyhow::bail!("{} does not exist: {}", field_name, value);
    }
    Ok(Some(value.to_string()))
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::path::Path;
    use std::path::PathBuf;
    use std::time::Duration;

    use crate::agent::{AgentConfig, AgentNodeConfig, WorktreeMode};
    use agenthub_team_actor::{
        ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, ActorInboxRequest, ActorMailboxService,
        ActorMessageStatus, ActorMessageTransport,
    };
    use serde_json::{Value, json};
    use tonic::transport::{Certificate, Identity, ServerTlsConfig};
    use uuid::Uuid;

    use super::{InternalGrpcMailboxClient, InternalGrpcMailboxClientConfig};
    use crate::api::team_tests::build_test_state;
    use crate::internal::auth::{InternalAction, InternalAuthz, InternalAuthzConfig, InternalRole};
    use crate::internal::p2p::NodeTransportMetadata;
    use crate::internal::proto::agenthub::internal::v1::team_internal_control_server::TeamInternalControlServer;
    use crate::internal::service::TeamInternalControlService;
    use crate::internal::tls::{
        InternalGrpcSecurityMode, ensure_tls_material, install_rustls_crypto_provider,
    };
    use crate::team::{SendActorMessageInput, TeamActorMessageTransport};

    const TEST_INTERNAL_SHARED_SECRET: &str = "agenthub-internal-client-test-secret";

    fn build_authz() -> InternalAuthz {
        InternalAuthz::new(InternalAuthzConfig {
            shared_secret: TEST_INTERNAL_SHARED_SECRET.to_string(),
            expected_issuer: Some("agenthub".to_string()),
            expected_audience: Some("agenthub-internal".to_string()),
        })
    }

    fn issue_token(
        authz: &InternalAuthz,
        run_id: Option<&str>,
        permissions: Vec<String>,
    ) -> String {
        let (token, _expires_at) = authz
            .issue_access_token(InternalRole::Leader, None, run_id, permissions, 600)
            .expect("issue internal token");
        token
    }

    fn issue_mailbox_token(authz: &InternalAuthz, run_id: &str) -> String {
        issue_token(
            authz,
            Some(run_id),
            vec![
                InternalAction::MessageSend.as_str().to_string(),
                InternalAction::InboxList.as_str().to_string(),
                InternalAction::MessageAck.as_str().to_string(),
            ],
        )
    }

    fn issue_agent_manage_token(authz: &InternalAuthz) -> String {
        issue_token(
            authz,
            None,
            vec![InternalAction::AgentManage.as_str().to_string()],
        )
    }

    fn test_cert_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "agenthub-internal-client-{}-{}",
            name,
            Uuid::new_v4()
        ))
    }

    struct StartedInternalGrpcServer {
        addr: SocketAddr,
        handle: tokio::task::JoinHandle<()>,
    }

    async fn spawn_mtls_internal_grpc_server(
        state: crate::state::AppState,
        authz: InternalAuthz,
        cert_dir: PathBuf,
    ) -> StartedInternalGrpcServer {
        let server = TeamInternalControlServer::new(TeamInternalControlService::new(
            state,
            authz,
            InternalGrpcSecurityMode::Mtls,
            cert_dir.clone(),
            "bootstrap-token".to_string(),
        ));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        drop(listener);

        let server_cert_pem =
            std::fs::read(cert_dir.join("server-cert.pem")).expect("read server cert pem");
        let server_key_pem =
            std::fs::read(cert_dir.join("server-key.pem")).expect("read server key pem");
        let ca_cert_pem = std::fs::read(cert_dir.join("ca-cert.pem")).expect("read ca cert pem");
        let handle = tokio::spawn(async move {
            let tls = ServerTlsConfig::new()
                .identity(Identity::from_pem(server_cert_pem, server_key_pem))
                .client_ca_root(Certificate::from_pem(ca_cert_pem))
                .client_auth_optional(true);
            tonic::transport::Server::builder()
                .tls_config(tls)
                .expect("tls config")
                .add_service(server)
                .serve(addr)
                .await
                .expect("serve internal grpc");
        });

        tokio::time::sleep(Duration::from_millis(150)).await;
        StartedInternalGrpcServer { addr, handle }
    }

    fn mtls_client_config(
        addr: SocketAddr,
        access_token: String,
        cert_dir: &Path,
    ) -> InternalGrpcMailboxClientConfig {
        InternalGrpcMailboxClientConfig {
            target: format!("https://{}", addr),
            access_token,
            ca_cert_path: Some(cert_dir.join("ca-cert.pem").to_string_lossy().to_string()),
            tls_server_name: Some("localhost".to_string()),
            client_cert_path: Some(
                cert_dir
                    .join("client-cert.pem")
                    .to_string_lossy()
                    .to_string(),
            ),
            client_key_path: Some(
                cert_dir
                    .join("client-key.pem")
                    .to_string_lossy()
                    .to_string(),
            ),
        }
    }

    fn grpc_relay_route(
        addr: SocketAddr,
        access_token: &str,
        source_node_id: &str,
        target_node_id: &str,
    ) -> Value {
        let mut route = serde_json::Map::new();
        route.insert("kind".to_string(), json!("grpc"));
        route.insert(
            "grpc_target".to_string(),
            json!(format!("https://{}", addr)),
        );
        route.insert("access_token".to_string(), json!(access_token));
        route.insert("tls_server_name".to_string(), json!("localhost"));
        NodeTransportMetadata {
            cluster_id: "agenthub".to_string(),
            source_node_id: source_node_id.to_string(),
            target_node_id: target_node_id.to_string(),
            broadcast_id: None,
            correlation_id: None,
            idempotency_key: None,
            scope: vec!["node:p2p".to_string()],
            audience: vec!["agenthub-internal".to_string()],
            issued_at: chrono::Utc::now().timestamp(),
            expires_at: chrono::Utc::now().timestamp() + 600,
            kid: "shared-hs256-test".to_string(),
            payload_digest: None,
        }
        .apply_to_route(&mut route);
        Value::Object(route)
    }

    async fn seed_team_run(
        state: &crate::state::AppState,
        team_id: &str,
        team_name: &str,
        run_id: &str,
    ) {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO team_definitions (
                id,
                name,
                description,
                spec_json,
                owner_user_id,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6)
            "#,
        )
        .bind(team_id)
        .bind(team_name)
        .bind("grpc relay pipeline test team")
        .bind(
            json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner"},
                    {"member_id":"reviewer"}
                ]
            })
            .to_string(),
        )
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert team definition");

        sqlx::query(
            r#"
            INSERT INTO team_runs (
                id,
                team_id,
                context_id,
                status,
                input_json,
                created_at
            )
            VALUES (?1, ?2, ?3, 'working', ?4, ?5)
            "#,
        )
        .bind(run_id)
        .bind(team_id)
        .bind(format!("ctx-{run_id}"))
        .bind(json!({"prompt":"validate grpc relay pipeline"}).to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert team run");
    }

    async fn seed_safe_path(state: &crate::state::AppState, path: &std::path::Path) {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO safe_paths (path, created_at)
            VALUES (?1, ?2)
            "#,
        )
        .bind(path.to_string_lossy().to_string())
        .bind(chrono::Utc::now().timestamp())
        .execute(&state.db)
        .await
        .expect("insert safe path");
    }

    async fn configure_remote_grpc_relay(
        state: &crate::state::AppState,
        cert_dir: &Path,
        node_id: &str,
        addr: SocketAddr,
    ) {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS agent_nodes (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                grpc_target TEXT NOT NULL,
                tls_server_name TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&state.db)
        .await
        .expect("create agent_nodes table");
        state
            .teams
            .configure_internal_grpc_relay(cert_dir, InternalGrpcSecurityMode::Mtls);
        state
            .agents
            .create_agent_node(AgentNodeConfig {
                id: node_id.to_string(),
                name: format!("Node {node_id}"),
                grpc_target: format!("https://{}", addr),
                tls_server_name: Some("localhost".to_string()),
                default_worktree_root: None,
            })
            .await
            .expect("create agent node");
    }

    #[tokio::test]
    async fn remote_actor_grpc_pipeline_delivers_and_acks_over_tls() {
        install_rustls_crypto_provider();
        let source_state = build_test_state().await;
        let remote_state = build_test_state().await;
        let team_id = format!("team-{}", Uuid::new_v4());
        let team_name = format!("grpc-relay-team-{}", Uuid::new_v4());
        let run_id = format!("run-{}", Uuid::new_v4());
        seed_team_run(&source_state, &team_id, &team_name, &run_id).await;
        seed_team_run(&remote_state, &team_id, &team_name, &run_id).await;

        let cert_dir = test_cert_dir("relay-pipeline");
        ensure_tls_material(&cert_dir, InternalGrpcSecurityMode::Mtls)
            .expect("generate tls material")
            .expect("tls material");
        let authz = build_authz();
        let access_token = issue_mailbox_token(&authz, &run_id);

        let server =
            spawn_mtls_internal_grpc_server(remote_state.clone(), authz.clone(), cert_dir.clone())
                .await;
        configure_remote_grpc_relay(&source_state, &cert_dir, "node-remote", server.addr).await;

        let route = grpc_relay_route(server.addr, &access_token, "node-source", "node-remote");

        let sent = source_state
            .teams
            .send_actor_message(SendActorMessageInput {
                run_id: &run_id,
                from_actor_id: "planner",
                from_peer_id: ACTOR_MAIN_PEER_ID,
                to_actor_id: "reviewer",
                to_peer_id: ACTOR_NODE_PEER_ID,
                channel: "coordination",
                transport: TeamActorMessageTransport::Remote,
                route: Some(route),
                payload: json!({"type":"chat_message","text":"review this patch"}),
                idempotency_key: Some("grpc-relay-pipeline"),
            })
            .await
            .expect("send remote actor message");
        assert_eq!(sent.status, crate::team::TeamActorMessageStatus::Pending);

        let relay_result = source_state
            .teams
            .relay_remote_messages_once(100, 3, 30)
            .await
            .expect("relay remote messages");
        assert_eq!(relay_result.scanned, 1);
        assert_eq!(relay_result.delivered, 1);
        assert_eq!(relay_result.retried, 0);
        assert_eq!(relay_result.dead_lettered, 0);

        let client = InternalGrpcMailboxClient::connect(InternalGrpcMailboxClientConfig {
            ..mtls_client_config(server.addr, issue_mailbox_token(&authz, &run_id), &cert_dir)
        })
        .await
        .expect("connect grpc mailbox client");

        let inbox = client
            .actor_inbox(ActorInboxRequest {
                run_id: run_id.clone(),
                actor_id: "reviewer".to_string(),
                cursor: None,
                limit: Some(20),
                states: None,
            })
            .await
            .expect("list remote inbox");
        assert_eq!(inbox.messages.len(), 1);
        let pending = &inbox.messages[0];
        assert_eq!(pending.from_actor_id, "planner");
        assert_eq!(pending.to_actor_id, "reviewer");
        assert_eq!(pending.channel, "coordination");
        assert_eq!(pending.transport, ActorMessageTransport::Local);
        assert_eq!(pending.payload["text"], "review this patch");
        assert_eq!(pending.status, ActorMessageStatus::Pending);
        assert_eq!(pending.from_peer_id, "node-source");
        assert_eq!(pending.to_peer_id, "main");

        let ack = client
            .actor_ack(agenthub_team_actor::ActorAckRequest {
                run_id: run_id.clone(),
                actor_id: "reviewer".to_string(),
                message_id: pending.message_id,
                ack_token: None,
                result: None,
            })
            .await
            .expect("ack remote inbox message");
        assert_eq!(ack.state, ActorMessageStatus::Delivered);
        assert!(ack.acked_at >= ack.message.created_at);

        let delivered_inbox = client
            .actor_inbox(ActorInboxRequest {
                run_id,
                actor_id: "reviewer".to_string(),
                cursor: None,
                limit: Some(20),
                states: Some(vec![ActorMessageStatus::Delivered]),
            })
            .await
            .expect("list delivered remote inbox");
        assert_eq!(delivered_inbox.messages.len(), 1);
        assert_eq!(
            delivered_inbox.messages[0].status,
            ActorMessageStatus::Delivered
        );

        server.handle.abort();
    }

    // This is an in-process transport regression test. The blackbox multi-process
    // p2p pipeline lives in `tests/distributed_p2p_pipeline.rs`.
    #[tokio::test]
    async fn bidirectional_actor_grpc_pipeline_relays_seeded_messages_between_in_process_states() {
        install_rustls_crypto_provider();
        let node_a_state = build_test_state().await;
        let node_b_state = build_test_state().await;
        let team_id = format!("team-{}", Uuid::new_v4());
        let team_name = format!("grpc-p2p-team-{}", Uuid::new_v4());
        let run_id = format!("run-{}", Uuid::new_v4());
        seed_team_run(&node_a_state, &team_id, &team_name, &run_id).await;
        seed_team_run(&node_b_state, &team_id, &team_name, &run_id).await;

        let cert_dir = test_cert_dir("bidirectional-relay-pipeline");
        ensure_tls_material(&cert_dir, InternalGrpcSecurityMode::Mtls)
            .expect("generate tls material")
            .expect("tls material");
        let authz = build_authz();
        let access_token = issue_mailbox_token(&authz, &run_id);

        let node_a_server =
            spawn_mtls_internal_grpc_server(node_a_state.clone(), authz.clone(), cert_dir.clone())
                .await;
        let node_b_server =
            spawn_mtls_internal_grpc_server(node_b_state.clone(), authz.clone(), cert_dir.clone())
                .await;
        configure_remote_grpc_relay(&node_a_state, &cert_dir, "node-b", node_b_server.addr).await;
        configure_remote_grpc_relay(&node_b_state, &cert_dir, "node-a", node_a_server.addr).await;

        let route_to_a = grpc_relay_route(node_a_server.addr, &access_token, "node-b", "node-a");
        let route_to_b = grpc_relay_route(node_b_server.addr, &access_token, "node-a", "node-b");

        node_a_state
            .teams
            .send_actor_message(SendActorMessageInput {
                run_id: &run_id,
                from_actor_id: "planner-a",
                from_peer_id: ACTOR_MAIN_PEER_ID,
                to_actor_id: "reviewer-b",
                to_peer_id: ACTOR_NODE_PEER_ID,
                channel: "coordination",
                transport: TeamActorMessageTransport::Remote,
                route: Some(route_to_b.clone()),
                payload: json!({
                    "type":"chat_message",
                    "text":"node-a-1",
                    "sequence":1,
                    "correlation_id":"corr-a-1"
                }),
                idempotency_key: Some("p2p-a-1"),
            })
            .await
            .expect("send first seeded node-a message");
        node_a_state
            .teams
            .send_actor_message(SendActorMessageInput {
                run_id: &run_id,
                from_actor_id: "planner-a",
                from_peer_id: ACTOR_MAIN_PEER_ID,
                to_actor_id: "reviewer-b",
                to_peer_id: ACTOR_NODE_PEER_ID,
                channel: "coordination",
                transport: TeamActorMessageTransport::Remote,
                route: Some(route_to_b),
                payload: json!({
                    "type":"chat_message",
                    "text":"node-a-2",
                    "sequence":2,
                    "correlation_id":"corr-a-2"
                }),
                idempotency_key: Some("p2p-a-2"),
            })
            .await
            .expect("send second seeded node-a message");
        node_b_state
            .teams
            .send_actor_message(SendActorMessageInput {
                run_id: &run_id,
                from_actor_id: "reviewer-b",
                from_peer_id: ACTOR_MAIN_PEER_ID,
                to_actor_id: "planner-a",
                to_peer_id: ACTOR_NODE_PEER_ID,
                channel: "coordination",
                transport: TeamActorMessageTransport::Remote,
                route: Some(route_to_a),
                payload: json!({
                    "type":"chat_message",
                    "text":"node-b-1",
                    "sequence":1,
                    "correlation_id":"corr-b-1"
                }),
                idempotency_key: Some("p2p-b-1"),
            })
            .await
            .expect("send seeded node-b reply");

        let relay_from_a = node_a_state
            .teams
            .relay_remote_messages_once(100, 3, 30)
            .await
            .expect("relay seeded node-a messages");
        assert_eq!(relay_from_a.scanned, 2);
        assert_eq!(relay_from_a.delivered, 2);
        assert_eq!(relay_from_a.retried, 0);
        assert_eq!(relay_from_a.dead_lettered, 0);

        let relay_from_b = node_b_state
            .teams
            .relay_remote_messages_once(100, 3, 30)
            .await
            .expect("relay seeded node-b reply");
        assert_eq!(relay_from_b.scanned, 1);
        assert_eq!(relay_from_b.delivered, 1);
        assert_eq!(relay_from_b.retried, 0);
        assert_eq!(relay_from_b.dead_lettered, 0);

        let node_a_client = InternalGrpcMailboxClient::connect(mtls_client_config(
            node_a_server.addr,
            issue_mailbox_token(&authz, &run_id),
            &cert_dir,
        ))
        .await
        .expect("connect node-a grpc mailbox client");
        let node_b_client = InternalGrpcMailboxClient::connect(mtls_client_config(
            node_b_server.addr,
            issue_mailbox_token(&authz, &run_id),
            &cert_dir,
        ))
        .await
        .expect("connect node-b grpc mailbox client");

        let node_b_inbox = node_b_client
            .actor_inbox(ActorInboxRequest {
                run_id: run_id.clone(),
                actor_id: "reviewer-b".to_string(),
                cursor: None,
                limit: Some(20),
                states: None,
            })
            .await
            .expect("list node-b seeded inbox");
        assert_eq!(node_b_inbox.messages.len(), 2);
        assert_eq!(node_b_inbox.messages[0].payload["text"], "node-a-1");
        assert_eq!(node_b_inbox.messages[1].payload["text"], "node-a-2");
        assert_eq!(
            node_b_inbox
                .messages
                .iter()
                .map(|message| message.payload["sequence"].as_i64().unwrap_or_default())
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(node_b_inbox.messages.iter().all(|message| {
            message.transport == ActorMessageTransport::Local
                && message.route.is_none()
                && message.status == ActorMessageStatus::Pending
        }));
        assert!(
            node_b_inbox
                .messages
                .iter()
                .all(|message| message.from_peer_id == "node-a" && message.to_peer_id == "main")
        );

        let node_a_inbox = node_a_client
            .actor_inbox(ActorInboxRequest {
                run_id: run_id.clone(),
                actor_id: "planner-a".to_string(),
                cursor: None,
                limit: Some(20),
                states: None,
            })
            .await
            .expect("list node-a seeded inbox");
        assert_eq!(node_a_inbox.messages.len(), 1);
        assert_eq!(node_a_inbox.messages[0].payload["text"], "node-b-1");
        assert_eq!(
            node_a_inbox.messages[0].transport,
            ActorMessageTransport::Local
        );
        assert_eq!(node_a_inbox.messages[0].from_peer_id, "node-b");
        assert_eq!(node_a_inbox.messages[0].to_peer_id, "main");
        assert!(node_a_inbox.messages[0].route.is_none());
        assert_eq!(node_a_inbox.messages[0].status, ActorMessageStatus::Pending);

        for message in &node_b_inbox.messages {
            let ack = node_b_client
                .actor_ack(agenthub_team_actor::ActorAckRequest {
                    run_id: run_id.clone(),
                    actor_id: "reviewer-b".to_string(),
                    message_id: message.message_id,
                    ack_token: None,
                    result: None,
                })
                .await
                .expect("ack node-b seeded inbox message");
            assert_eq!(ack.state, ActorMessageStatus::Delivered);
        }

        let node_a_ack = node_a_client
            .actor_ack(agenthub_team_actor::ActorAckRequest {
                run_id: run_id.clone(),
                actor_id: "planner-a".to_string(),
                message_id: node_a_inbox.messages[0].message_id,
                ack_token: None,
                result: None,
            })
            .await
            .expect("ack node-a seeded inbox message");
        assert_eq!(node_a_ack.state, ActorMessageStatus::Delivered);

        let node_b_delivered = node_b_client
            .actor_inbox(ActorInboxRequest {
                run_id: run_id.clone(),
                actor_id: "reviewer-b".to_string(),
                cursor: None,
                limit: Some(20),
                states: Some(vec![ActorMessageStatus::Delivered]),
            })
            .await
            .expect("list delivered node-b inbox");
        assert_eq!(node_b_delivered.messages.len(), 2);
        assert!(
            node_b_delivered
                .messages
                .iter()
                .all(|message| message.status == ActorMessageStatus::Delivered)
        );

        let node_a_delivered = node_a_client
            .actor_inbox(ActorInboxRequest {
                run_id,
                actor_id: "planner-a".to_string(),
                cursor: None,
                limit: Some(20),
                states: Some(vec![ActorMessageStatus::Delivered]),
            })
            .await
            .expect("list delivered node-a inbox");
        assert_eq!(node_a_delivered.messages.len(), 1);
        assert_eq!(
            node_a_delivered.messages[0].status,
            ActorMessageStatus::Delivered
        );

        node_a_server.handle.abort();
        node_b_server.handle.abort();
    }

    #[tokio::test]
    async fn remote_agent_grpc_control_starts_inputs_and_lists_events_over_tls() {
        install_rustls_crypto_provider();
        let remote_state = build_test_state().await;
        let workdir_root =
            std::env::temp_dir().join(format!("agenthub-remote-agent-{}", Uuid::new_v4()));
        let workdir = workdir_root.join("workspace");
        std::fs::create_dir_all(&workdir).expect("create workdir");
        seed_safe_path(&remote_state, &workdir_root).await;

        let cert_dir = test_cert_dir("remote-agent-control");
        ensure_tls_material(&cert_dir, InternalGrpcSecurityMode::Mtls)
            .expect("generate tls material")
            .expect("tls material");
        let authz = build_authz();

        let server =
            spawn_mtls_internal_grpc_server(remote_state.clone(), authz.clone(), cert_dir.clone())
                .await;

        let client = InternalGrpcMailboxClient::connect(mtls_client_config(
            server.addr,
            issue_agent_manage_token(&authz),
            &cert_dir,
        ))
        .await
        .expect("connect grpc control client");

        let agent_id = format!("remote-agent-{}", Uuid::new_v4());
        let agent = client
            .ensure_agent_record(
                &agent_id,
                &AgentConfig {
                    name: "Remote Control".to_string(),
                    workdir: workdir.to_string_lossy().to_string(),
                    command: "/bin/sh".to_string(),
                    args: vec![
                        "-lc".to_string(),
                        "printf 'ready\\n'; IFS= read -r line; printf 'echo:%s\\n' \"$line\"; sleep 1"
                            .to_string(),
                    ],
                    target_node_id: None,
                    worktree_mode: WorktreeMode::UseExisting,
                    worktree_repo: None,
                    worktree_ref: None,
                    code_mode: false,
                    agent_loop_enabled: false,
                    agent_loop_idle_seconds: None,
                    agent_loop_prompt: None,
                },
                "manual",
            )
            .await
            .expect("ensure remote agent");
        assert_eq!(agent.id, agent_id);
        assert_eq!(agent.status, crate::agent::AgentStatus::Created);
        assert!(agent.target_node_id.is_none());

        let session_id = client
            .start_managed_agent(&agent_id, None)
            .await
            .expect("start remote agent");
        assert!(!session_id.trim().is_empty());

        let ready_events = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let events = client
                    .list_agent_events(&agent_id, 50, Some(&session_id), None)
                    .await
                    .expect("list ready events");
                if events.iter().any(|event| event.message.contains("ready")) {
                    break events;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("ready events timeout");
        assert!(
            ready_events
                .iter()
                .any(|event| event.message.contains("ready"))
        );

        client
            .send_agent_input(&agent_id, "ping", None, Some(&session_id))
            .await
            .expect("send agent input");

        let echoed_events = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let events = client
                    .list_agent_events(&agent_id, 100, Some(&session_id), None)
                    .await
                    .expect("list echoed events");
                if events
                    .iter()
                    .any(|event| event.message.contains("echo:ping"))
                {
                    break events;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("echo events timeout");
        assert!(
            echoed_events
                .iter()
                .any(|event| event.message.contains("echo:ping"))
        );

        client
            .stop_managed_agent(&agent_id)
            .await
            .expect("stop remote agent");
        let stopped_agent = remote_state
            .agents
            .get_agent(&agent_id)
            .await
            .expect("load stopped remote agent");
        assert_eq!(stopped_agent.status, crate::agent::AgentStatus::Stopped);

        server.handle.abort();
    }
}
