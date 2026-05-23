use agenthub_team_actor::ActorRelayError;
use reqwest::Url;
use sqlx::Row;

use super::remote_relay::{
    GrpcRelayClientCacheKey, GrpcRemoteRelayRouteValue, TeamRemoteRelayAdapter,
    TeamRemoteRelayError,
};
use crate::agent::AGENT_NODE_MAIN_ID;
use crate::internal::auth::InternalRole;
use crate::internal::auth::{InternalAction, InternalAuthz, InternalAuthzConfig};
use crate::internal::client::{InternalGrpcMailboxClient, InternalGrpcMailboxClientConfig};
use crate::internal::p2p::{CredentialProvider, NodeCredentialRequest};

#[derive(Debug)]
pub(super) struct ResolvedGrpcRelayRoute {
    pub(super) grpc_target: String,
    pub(super) tls_server_name: Option<String>,
}

impl TeamRemoteRelayAdapter {
    pub(super) async fn resolve_registered_grpc_route(
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

    pub(super) fn issue_registered_grpc_access_token(
        &self,
    ) -> Result<String, ActorRelayError<TeamRemoteRelayError>> {
        let peer_config = self
            .grpc_peer_client_config
            .lock()
            .expect("lock grpc peer client config")
            .clone()
            .ok_or_else(|| {
                ActorRelayError::permanent(TeamRemoteRelayError::GrpcPeerClientUnavailable)
            })?;
        let authz = InternalAuthz::new(InternalAuthzConfig {
            shared_secret: peer_config.shared_secret.clone(),
            expected_issuer: peer_config.expected_issuer.clone(),
            expected_audience: peer_config.expected_audience.clone(),
        });
        let issued = authz
            .issue_node_access_token(NodeCredentialRequest {
                source_node_id: peer_config.source_node_id.clone(),
                role: InternalRole::Coordinator.as_str().to_string(),
                actor_id: None,
                run_id: None,
                permissions: vec![InternalAction::MessageSend.as_str().to_string()],
                scope: Vec::new(),
                audience: Vec::new(),
                ttl_seconds: 600,
            })
            .map_err(|err| {
                ActorRelayError::permanent(TeamRemoteRelayError::InvalidRoute(format!(
                    "issue relay access token failed: {err}"
                )))
            })?;
        Ok(issued.access_token)
    }

    pub(super) async fn grpc_client_for_route(
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
