use crate::internal::auth::{InternalAction, InternalAuthz, InternalAuthzConfig, InternalRole};
use crate::internal::client::{
    InternalGrpcMailboxClient, InternalGrpcMailboxClientConfig, normalize_existing_path,
};
use crate::internal::tls::InternalGrpcSecurityMode;

#[cfg(test)]
fn internal_grpc_target_uses_tls(target: &str) -> bool {
    target
        .trim()
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
}

fn actor_runtime_loopback_target(
    listen_addr: &str,
    security_mode: InternalGrpcSecurityMode,
) -> Option<String> {
    let parsed = listen_addr.trim().parse::<std::net::SocketAddr>().ok()?;
    let scheme = if security_mode == InternalGrpcSecurityMode::Disabled {
        "http"
    } else {
        "https"
    };
    Some(format!("{scheme}://127.0.0.1:{}", parsed.port()))
}

type OptionalRemoteMailboxClient = anyhow::Result<Option<InternalGrpcMailboxClient>>;

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

pub(crate) async fn maybe_remote_mailbox_service() -> OptionalRemoteMailboxClient {
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

fn maybe_runtime_tls_paths(
    cert_dir: &str,
    security_mode: InternalGrpcSecurityMode,
) -> (Option<String>, Option<String>, Option<String>) {
    let cert_root = std::path::Path::new(cert_dir);
    let ca_cert_path = cert_root.join("ca-cert.pem");
    let client_cert_path = cert_root.join("client-cert.pem");
    let client_key_path = cert_root.join("client-key.pem");
    let ca = ca_cert_path
        .is_file()
        .then(|| ca_cert_path.to_string_lossy().to_string());
    if security_mode == InternalGrpcSecurityMode::Mtls {
        (
            ca,
            client_cert_path
                .is_file()
                .then(|| client_cert_path.to_string_lossy().to_string()),
            client_key_path
                .is_file()
                .then(|| client_key_path.to_string_lossy().to_string()),
        )
    } else {
        (ca, None, None)
    }
}

pub(crate) async fn connect_runtime_internal_mailbox_service(
    actor_id: &str,
    run_id: Option<&str>,
    permissions: &[InternalAction],
) -> anyhow::Result<Option<InternalGrpcMailboxClient>> {
    if let Some(client) = maybe_remote_mailbox_service().await? {
        return Ok(Some(client));
    }
    let config = agenthub_config::AppConfig::load_with_info()?.0;
    if !config.internal_grpc_enabled() {
        return Ok(None);
    }
    let security_mode = InternalGrpcSecurityMode::parse(&config.internal_grpc_security_mode())?;
    let target = actor_runtime_loopback_target(&config.internal_grpc_listen_addr(), security_mode)
        .ok_or_else(|| anyhow::anyhow!("invalid internal gRPC listen address"))?;
    let cert_dir = config.internal_grpc_cert_dir();
    let authz = InternalAuthz::new(InternalAuthzConfig {
        shared_secret: config
            .internal_grpc_auth_shared_secret()
            .ok_or_else(|| anyhow::anyhow!("internal gRPC auth shared secret is required"))?,
        expected_issuer: config.internal_grpc_auth_issuer(),
        expected_audience: config.internal_grpc_auth_audience(),
    });
    let permissions = permissions
        .iter()
        .map(|permission| permission.as_str().to_string())
        .collect::<Vec<_>>();
    let (access_token, _expires_at) = authz.issue_access_token(
        InternalRole::Worker,
        Some(actor_id),
        run_id,
        permissions,
        600,
    )?;
    let (ca_cert_path, client_cert_path, client_key_path) =
        maybe_runtime_tls_paths(&cert_dir, security_mode);
    let tls_server_name = if security_mode == InternalGrpcSecurityMode::Disabled {
        None
    } else {
        Some("localhost".to_string())
    };
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{
        InternalGrpcSecurityMode, actor_runtime_loopback_target, internal_grpc_target_uses_tls,
        maybe_runtime_tls_paths,
    };
    use uuid::Uuid;

    fn write_tls_file(dir: &Path, name: &str) {
        fs::write(dir.join(name), b"test").expect("write tls file");
    }

    #[test]
    fn actor_runtime_loopback_target_uses_http_when_tls_is_disabled() {
        let target =
            actor_runtime_loopback_target("0.0.0.0:50051", InternalGrpcSecurityMode::Disabled)
                .expect("loopback target");
        assert_eq!(target, "http://127.0.0.1:50051");
    }

    #[test]
    fn actor_runtime_loopback_target_uses_https_when_tls_is_enabled() {
        let target = actor_runtime_loopback_target("0.0.0.0:50051", InternalGrpcSecurityMode::Tls)
            .expect("loopback target");
        assert_eq!(target, "https://127.0.0.1:50051");
    }

    #[test]
    fn internal_grpc_target_uses_tls_tracks_scheme() {
        assert!(internal_grpc_target_uses_tls("https://127.0.0.1:50051"));
        assert!(!internal_grpc_target_uses_tls("http://127.0.0.1:50051"));
    }

    #[test]
    fn actor_runtime_loopback_target_rejects_invalid_addresses() {
        assert!(
            actor_runtime_loopback_target("not-a-socket", InternalGrpcSecurityMode::Disabled)
                .is_none()
        );
    }

    #[test]
    fn maybe_runtime_tls_paths_only_returns_ca_for_non_mtls_modes() {
        let dir =
            std::env::temp_dir().join(format!("agenthub-runtime-tls-non-mtls-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create tls dir");
        write_tls_file(&dir, "ca-cert.pem");
        write_tls_file(&dir, "client-cert.pem");
        write_tls_file(&dir, "client-key.pem");

        let (ca_cert, client_cert, client_key) = maybe_runtime_tls_paths(
            dir.to_string_lossy().as_ref(),
            InternalGrpcSecurityMode::Tls,
        );
        assert_eq!(
            ca_cert.as_deref(),
            Some(dir.join("ca-cert.pem").to_string_lossy().as_ref())
        );
        assert!(client_cert.is_none());
        assert!(client_key.is_none());
    }

    #[test]
    fn maybe_runtime_tls_paths_returns_mtls_material_when_present() {
        let dir =
            std::env::temp_dir().join(format!("agenthub-runtime-tls-mtls-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create tls dir");
        write_tls_file(&dir, "ca-cert.pem");
        write_tls_file(&dir, "client-cert.pem");
        write_tls_file(&dir, "client-key.pem");

        let (ca_cert, client_cert, client_key) = maybe_runtime_tls_paths(
            dir.to_string_lossy().as_ref(),
            InternalGrpcSecurityMode::Mtls,
        );
        assert_eq!(
            ca_cert.as_deref(),
            Some(dir.join("ca-cert.pem").to_string_lossy().as_ref())
        );
        assert_eq!(
            client_cert.as_deref(),
            Some(dir.join("client-cert.pem").to_string_lossy().as_ref())
        );
        assert_eq!(
            client_key.as_deref(),
            Some(dir.join("client-key.pem").to_string_lossy().as_ref())
        );
    }
}
