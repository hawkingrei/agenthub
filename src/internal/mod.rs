pub(crate) mod auth;
pub(crate) mod client;
pub(crate) mod p2p;
mod service;
pub(crate) mod tls;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use tonic::transport::server::TcpIncoming;
use tonic::transport::{Certificate, Identity, ServerTlsConfig};

use crate::state::AppState;
use agenthub_config::AppConfig;

use self::auth::{InternalAuthz, InternalAuthzConfig};
use self::service::TeamInternalControlService;
use self::tls::{
    InternalGrpcSecurityMode, ensure_bootstrap_token, ensure_shared_secret, ensure_tls_material,
    install_rustls_crypto_provider,
};

pub mod proto {
    pub mod agenthub {
        pub mod internal {
            pub mod v1 {
                include!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/internal/proto/agenthub.internal.v1.rs"
                ));
            }
        }
    }
}

fn bind_internal_grpc_incoming(listen_addr: &str) -> anyhow::Result<(SocketAddr, TcpIncoming)> {
    let addr: SocketAddr = listen_addr
        .parse()
        .map_err(anyhow::Error::from)
        .map_err(|err| anyhow::anyhow!("parse internal gRPC listen addr '{listen_addr}': {err}"))?;
    let incoming = TcpIncoming::bind(addr)
        .map_err(anyhow::Error::from)
        .map_err(|err| anyhow::anyhow!("bind internal gRPC listen addr '{listen_addr}': {err}"))?
        .with_nodelay(Some(true));
    let bound_addr = incoming
        .local_addr()
        .map_err(anyhow::Error::from)
        .map_err(|err| {
            anyhow::anyhow!("resolve internal gRPC bound addr '{listen_addr}': {err}")
        })?;
    Ok((bound_addr, incoming))
}

pub async fn maybe_spawn_internal_grpc(
    state: AppState,
    config: &AppConfig,
) -> anyhow::Result<Option<tokio::task::JoinHandle<()>>> {
    if !config.internal_grpc_enabled() {
        tracing::info!("internal gRPC disabled");
        return Ok(None);
    }

    install_rustls_crypto_provider();
    let mode = InternalGrpcSecurityMode::parse(&config.internal_grpc_security_mode())?;
    let cert_dir = PathBuf::from(config.internal_grpc_cert_dir());
    let shared_secret = ensure_shared_secret(&cert_dir, config.internal_grpc_auth_shared_secret())?;
    let bootstrap_token =
        ensure_bootstrap_token(&cert_dir, config.internal_grpc_bootstrap_token())?;
    let authz = InternalAuthz::new(InternalAuthzConfig {
        shared_secret,
        expected_issuer: config
            .internal_grpc_auth_issuer()
            .or_else(|| Some("agenthub".to_string())),
        expected_audience: config
            .internal_grpc_auth_audience()
            .or_else(|| Some("agenthub-internal".to_string())),
    });

    let listen_addr = config.internal_grpc_listen_addr();
    let mut server_builder = tonic::transport::Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(20)))
        .http2_keepalive_timeout(Some(Duration::from_secs(10)));
    if let Some(material) = ensure_tls_material(&cert_dir, mode)? {
        let mut tls = ServerTlsConfig::new().identity(Identity::from_pem(
            material.server_cert_pem,
            material.server_key_pem,
        ));
        if mode == InternalGrpcSecurityMode::Mtls {
            tls = tls
                .client_ca_root(Certificate::from_pem(material.ca_cert_pem))
                .client_auth_optional(true);
            tracing::info!(
                "internal gRPC security mode: mtls (optional client auth for bootstrap, cert dir: {})",
                cert_dir.display()
            );
        } else {
            tracing::info!(
                "internal gRPC security mode: tls (cert dir: {})",
                cert_dir.display()
            );
        }
        server_builder = server_builder.tls_config(tls)?;
    } else {
        tracing::warn!("internal gRPC security mode: disabled (dev/testing only)");
    }

    let service =
        proto::agenthub::internal::v1::team_internal_control_server::TeamInternalControlServer::new(
            TeamInternalControlService::new(state, authz, mode, cert_dir, bootstrap_token),
        );

    let (bound_addr, incoming) = bind_internal_grpc_incoming(&listen_addr)?;
    let handle = tokio::spawn(async move {
        if let Err(err) = server_builder
            .add_service(service)
            .serve_with_incoming(incoming)
            .await
        {
            tracing::error!(
                error = %err,
                error_debug = ?err,
                "internal gRPC server exited with error"
            );
        }
    });
    tracing::info!("internal gRPC listening on {}", bound_addr);
    Ok(Some(handle))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use agenthub_config::{AppConfig, InternalGrpcConfig, InternalGrpcSecurityConfig};
    use uuid::Uuid;

    use super::maybe_spawn_internal_grpc;

    fn test_internal_grpc_cert_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("agenthub-internal-grpc-{name}-{}", Uuid::new_v4()))
    }

    fn test_internal_grpc_config(listen_addr: String, cert_dir: &std::path::Path) -> AppConfig {
        AppConfig {
            internal_grpc: Some(InternalGrpcConfig {
                enabled: Some(true),
                listen: Some(listen_addr),
                security: Some(InternalGrpcSecurityConfig {
                    mode: Some("disabled".to_string()),
                    cert_dir: Some(cert_dir.to_string_lossy().to_string()),
                }),
                auth: None,
                bootstrap: None,
            }),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn maybe_spawn_internal_grpc_fails_fast_when_listen_addr_is_occupied() {
        let occupied = std::net::TcpListener::bind("127.0.0.1:0").expect("bind occupied port");
        let listen_addr = occupied
            .local_addr()
            .expect("occupied listener addr")
            .to_string();
        let cert_dir = test_internal_grpc_cert_dir("occupied");
        let state = crate::api::team_tests::build_test_state().await;
        let config = test_internal_grpc_config(listen_addr.clone(), &cert_dir);

        let err = maybe_spawn_internal_grpc(state, &config)
            .await
            .expect_err("occupied listener should fail before startup continues");
        let message = err.to_string();
        assert!(message.contains("bind internal gRPC listen addr"));
        assert!(message.contains(&listen_addr));
    }
}
