use std::time::Duration;

use agenthub_internal_p2p::P2PTransport;
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ActorMessageTransport, ActorRelayError, ActorSendRequest,
};
use reqwest::{Url, header};

use super::remote_relay::{
    GrpcRemoteRelayRouteValue, HttpRemoteRelayRouteValue, TeamRemoteRelayAdapter,
    TeamRemoteRelayError,
};
use super::remote_relay_route::{
    apply_route_auth, apply_route_headers, apply_route_signing, build_remote_relay_envelope,
    map_grpc_actor_error, parse_route_method, relay_timeout_ms,
};
use crate::internal::p2p::build_message_metadata;
use crate::team::TeamActorMessageRecord;

impl TeamRemoteRelayAdapter {
    pub(super) async fn deliver_http(
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

    pub(super) async fn deliver_grpc(
        &self,
        message: &TeamActorMessageRecord,
        route: GrpcRemoteRelayRouteValue,
    ) -> Result<(), ActorRelayError<TeamRemoteRelayError>> {
        let metadata = build_message_metadata(message);
        let resolved_route = self
            .resolve_registered_grpc_route(&metadata.target_node_id, &route)
            .await?;
        let access_token = route
            .access_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .map(Ok)
            .unwrap_or_else(|| self.issue_registered_grpc_access_token())?;
        let grpc_tls_defaults = self
            .grpc_tls_defaults
            .lock()
            .expect("lock grpc tls defaults")
            .clone()
            .ok_or_else(|| ActorRelayError::permanent(TeamRemoteRelayError::GrpcTlsUnavailable))?;

        let client = self
            .grpc_client_for_route(crate::internal::client::InternalGrpcMailboxClientConfig {
                target: resolved_route.grpc_target,
                access_token,
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
                message_kind: Some(message.message_kind.clone()),
            })
            .await
            .map_err(map_grpc_actor_error)?;
        Ok(())
    }
}
