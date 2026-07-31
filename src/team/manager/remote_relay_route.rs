use std::collections::HashMap;

use agenthub_team_actor::{ActorRelayError, ActorServiceError, ActorServiceErrorCode};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chrono::Utc;
use hmac::{Hmac, KeyInit, Mac};
use reqwest::{Method, header};
use serde_json::{Value, json};
use sha2::Sha256;

use super::remote_relay_types::{
    GrpcRemoteRelayRouteValue, HttpRemoteRelayRouteValue, ParsedRemoteRelayRoute,
    RELAY_DEFAULT_TIMEOUT_MS, RELAY_TIMEOUT_MAX_MS, RELAY_TIMEOUT_MIN_MS, RemoteRelayAuthValue,
    RemoteRelaySigningValue, TeamRemoteRelayError,
};
use crate::internal::p2p::build_message_metadata;
use crate::team::TeamActorMessageRecord;

pub(super) fn parse_remote_route(
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

pub(super) fn parse_route_method(
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

pub(super) fn relay_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    timeout_ms
        .unwrap_or(RELAY_DEFAULT_TIMEOUT_MS)
        .clamp(RELAY_TIMEOUT_MIN_MS, RELAY_TIMEOUT_MAX_MS)
}

pub(super) fn parse_route_header_name(
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

pub(super) fn parse_route_header_value(
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

pub(super) fn apply_route_headers(
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

pub(super) fn apply_route_auth(
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

pub(super) fn apply_route_signing(
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
            let signature = STANDARD.encode(mac.finalize().into_bytes().as_slice());
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

pub(super) fn build_remote_relay_envelope(message: &TeamActorMessageRecord) -> Value {
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
        "message_kind": message.message_kind.as_str(),
        "channel": &message.channel,
        "transport": message.transport.as_str(),
        "created_at": message.created_at,
        "payload": &message.payload,
    })
}

pub(super) fn map_grpc_actor_error(
    err: ActorServiceError,
) -> ActorRelayError<TeamRemoteRelayError> {
    let ActorServiceError { code, message } = err;
    let relay_error = TeamRemoteRelayError::GrpcRequest(message);
    match code {
        ActorServiceErrorCode::TooManyRequests | ActorServiceErrorCode::Internal => {
            ActorRelayError::retryable(relay_error)
        }
        _ => ActorRelayError::permanent(relay_error),
    }
}
