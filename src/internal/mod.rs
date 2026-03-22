pub(crate) mod auth;
pub(crate) mod client;
pub(crate) mod p2p;
mod service;
pub(crate) mod tls;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

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
    let addr: SocketAddr = listen_addr.parse()?;
    let mut server_builder = tonic::transport::Server::builder()
        .http2_keepalive_interval(Some(Duration::from_secs(20)))
        .http2_keepalive_timeout(Some(Duration::from_secs(10)))
        .tcp_nodelay(true);
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

    let handle = tokio::spawn(async move {
        if let Err(err) = server_builder.add_service(service).serve(addr).await {
            tracing::error!("internal gRPC server exited with error: {err}");
        }
    });
    tracing::info!("internal gRPC listening on {}", listen_addr);
    Ok(Some(handle))
}
