use std::path::PathBuf;

use crate::internal::client::InternalGrpcPeerClientConfig;
use crate::internal::tls::{
    InternalGrpcSecurityMode, ensure_bootstrap_token, ensure_shared_secret, ensure_tls_material,
};

use super::AppState;

impl AppState {
    pub(super) fn build_agent_node_join_bootstrap(
        config: &agenthub_config::AppConfig,
    ) -> anyhow::Result<crate::agent::AgentNodeJoinBootstrapInfo> {
        if !config.internal_grpc_enabled() {
            return Ok(crate::agent::AgentNodeJoinBootstrapInfo::disabled());
        }

        let cert_dir = PathBuf::from(config.internal_grpc_cert_dir());
        let bootstrap_token =
            ensure_bootstrap_token(&cert_dir, config.internal_grpc_bootstrap_token())?;
        Ok(crate::agent::AgentNodeJoinBootstrapInfo {
            enabled: true,
            bootstrap_token: Some(bootstrap_token),
            grpc_listen_addr: Some(config.internal_grpc_listen_addr()),
            security_mode: Some(config.internal_grpc_security_mode()),
            cert_dir: Some(cert_dir.to_string_lossy().to_string()),
            issuer: Some(
                config
                    .internal_grpc_auth_issuer()
                    .unwrap_or_else(|| "agenthub".to_string()),
            ),
            audience: Some(
                config
                    .internal_grpc_auth_audience()
                    .unwrap_or_else(|| "agenthub-internal".to_string()),
            ),
        })
    }

    pub(super) fn build_internal_grpc_peer_client(
        config: &agenthub_config::AppConfig,
    ) -> anyhow::Result<Option<InternalGrpcPeerClientConfig>> {
        if !config.internal_grpc_enabled() {
            return Ok(None);
        }

        let internal_grpc_cert_dir = PathBuf::from(config.internal_grpc_cert_dir());
        let internal_grpc_security_mode =
            InternalGrpcSecurityMode::parse(&config.internal_grpc_security_mode())?;
        let internal_shared_secret = ensure_shared_secret(
            &internal_grpc_cert_dir,
            config.internal_grpc_auth_shared_secret(),
        )?;
        let _ = ensure_tls_material(&internal_grpc_cert_dir, internal_grpc_security_mode)?;
        Ok(Some(InternalGrpcPeerClientConfig {
            shared_secret: internal_shared_secret,
            expected_issuer: config.internal_grpc_auth_issuer(),
            expected_audience: config.internal_grpc_auth_audience(),
            source_node_id: config.server_node_id()?,
            cert_dir: internal_grpc_cert_dir.to_string_lossy().to_string(),
            security_mode: internal_grpc_security_mode,
        }))
    }
}
