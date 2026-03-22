use crate::internal::client::{
    InternalGrpcMailboxClient, InternalGrpcMailboxClientConfig, normalize_existing_path,
};

pub(crate) const ACTOR_RUNTIME_TEAM_ID_ENV: &str = "AGENTHUB_ACTOR_TEAM_ID";
pub(crate) const ACTOR_RUNTIME_CURRENT_RUN_ID_ENV: &str = "AGENTHUB_ACTOR_CURRENT_RUN_ID";
pub(crate) const ACTOR_RUNTIME_ACTOR_ID_ENV: &str = "AGENTHUB_ACTOR_ID";
pub(crate) const ACTOR_RUNTIME_AGENT_ID_ENV: &str = "AGENTHUB_ACTOR_AGENT_ID";
pub(crate) const ACTOR_RUNTIME_CHANNEL_ENV: &str = "AGENTHUB_ACTOR_CHANNEL";
pub(crate) const ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV: &str = "AGENTHUB_INTERNAL_GRPC_TARGET";
pub(crate) const ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV: &str = "AGENTHUB_INTERNAL_GRPC_TOKEN";
pub(crate) const ACTOR_RUNTIME_INTERNAL_GRPC_CA_CERT_ENV: &str =
    "AGENTHUB_INTERNAL_GRPC_CA_CERT_PATH";
pub(crate) const ACTOR_RUNTIME_INTERNAL_GRPC_TLS_SERVER_NAME_ENV: &str =
    "AGENTHUB_INTERNAL_GRPC_TLS_SERVER_NAME";
pub(crate) const ACTOR_RUNTIME_INTERNAL_GRPC_CLIENT_CERT_ENV: &str =
    "AGENTHUB_INTERNAL_GRPC_CLIENT_CERT_PATH";
pub(crate) const ACTOR_RUNTIME_INTERNAL_GRPC_CLIENT_KEY_ENV: &str =
    "AGENTHUB_INTERNAL_GRPC_CLIENT_KEY_PATH";

pub(crate) fn normalized_env_var(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) async fn maybe_remote_mailbox_service()
-> anyhow::Result<Option<InternalGrpcMailboxClient>> {
    let Some(target) = normalized_env_var(ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV) else {
        return Ok(None);
    };
    let access_token =
        normalized_env_var(ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV).ok_or_else(|| {
            anyhow::anyhow!(
                "{} is required when {} is set",
                ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV,
                ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV
            )
        })?;
    let ca_cert_path = normalize_existing_path(
        normalized_env_var(ACTOR_RUNTIME_INTERNAL_GRPC_CA_CERT_ENV).as_deref(),
        ACTOR_RUNTIME_INTERNAL_GRPC_CA_CERT_ENV,
    )?;
    let client_cert_path = normalize_existing_path(
        normalized_env_var(ACTOR_RUNTIME_INTERNAL_GRPC_CLIENT_CERT_ENV).as_deref(),
        ACTOR_RUNTIME_INTERNAL_GRPC_CLIENT_CERT_ENV,
    )?;
    let client_key_path = normalize_existing_path(
        normalized_env_var(ACTOR_RUNTIME_INTERNAL_GRPC_CLIENT_KEY_ENV).as_deref(),
        ACTOR_RUNTIME_INTERNAL_GRPC_CLIENT_KEY_ENV,
    )?;
    let tls_server_name = normalized_env_var(ACTOR_RUNTIME_INTERNAL_GRPC_TLS_SERVER_NAME_ENV);
    let client = InternalGrpcMailboxClient::connect(InternalGrpcMailboxClientConfig {
        target,
        access_token,
        ca_cert_path,
        tls_server_name,
        client_cert_path,
        client_key_path,
    })
    .await?;
    Ok(Some(client))
}
