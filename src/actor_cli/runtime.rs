use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use agenthub_team_actor::{
    ActorInboxRequest, ActorInboxResponse, ActorMailboxService, ActorServiceError,
    actor_inbox_with_auto_ack,
};

use crate::actor_runtime_env::{
    ACTOR_RUNTIME_ACTOR_ID_ENV, ACTOR_RUNTIME_CURRENT_RUN_ID_ENV,
    connect_runtime_internal_mailbox_service, maybe_remote_mailbox_service, normalized_env_var,
};
use crate::agent::AGENT_NODE_MAIN_ID;
use crate::internal::auth::InternalAction;
use crate::internal::client::{InternalGrpcMailboxClient, InternalGrpcPeerClientConfig};
use crate::internal::tls::{InternalGrpcSecurityMode, ensure_shared_secret, ensure_tls_material};
use crate::team::{
    TeamManager, build_actor_mailbox_immediate_hint_prompt, plan_actor_mailbox_immediate_hint,
};

fn configure_actor_cli_internal_grpc(
    manager: &TeamManager,
    peer_client: Option<InternalGrpcPeerClientConfig>,
) {
    if let Some(peer_client) = peer_client {
        manager.configure_internal_grpc_relay(
            PathBuf::from(&peer_client.cert_dir).as_path(),
            peer_client.security_mode,
        );
        manager.configure_internal_grpc_peer_client(Some(peer_client));
    }
}

fn build_actor_cli_internal_grpc_peer_client(
    config: &agenthub_config::AppConfig,
) -> anyhow::Result<Option<InternalGrpcPeerClientConfig>> {
    if !config.internal_grpc_enabled() {
        return Ok(None);
    }
    let cert_dir = PathBuf::from(config.internal_grpc_cert_dir());
    let security_mode = InternalGrpcSecurityMode::parse(&config.internal_grpc_security_mode())?;
    let shared_secret = ensure_shared_secret(&cert_dir, config.internal_grpc_auth_shared_secret())?;
    let _ = ensure_tls_material(&cert_dir, security_mode)?;
    Ok(Some(InternalGrpcPeerClientConfig {
        shared_secret,
        expected_issuer: config.internal_grpc_auth_issuer(),
        expected_audience: config.internal_grpc_auth_audience(),
        source_node_id: AGENT_NODE_MAIN_ID.to_string(),
        cert_dir: cert_dir.to_string_lossy().to_string(),
        security_mode,
    }))
}

pub(super) fn actor_cli_internal_grpc_hint_target(listen_addr: &str) -> Option<String> {
    let parsed = listen_addr.parse::<SocketAddr>().ok()?;
    Some(format!("https://127.0.0.1:{}", parsed.port()))
}

pub(super) async fn init_actor_mailbox_hint_client_from_config(
    config: &agenthub_config::AppConfig,
) -> anyhow::Result<Option<InternalGrpcMailboxClient>> {
    match maybe_remote_mailbox_service().await {
        Ok(Some(client)) => return Ok(Some(client)),
        Ok(None) => {}
        Err(err) => {
            tracing::debug!(
                "skip mailbox hint client because remote mailbox service init failed: {err}"
            );
            return Ok(None);
        }
    }
    let Some(peer_client) = (match build_actor_cli_internal_grpc_peer_client(config) {
        Ok(peer_client) => peer_client,
        Err(err) => {
            tracing::debug!(
                "skip mailbox hint client because internal grpc peer config is unavailable: {err}"
            );
            return Ok(None);
        }
    }) else {
        return Ok(None);
    };
    let Some(target) = actor_cli_internal_grpc_hint_target(&config.internal_grpc_listen_addr())
    else {
        tracing::debug!(
            listen_addr = %config.internal_grpc_listen_addr(),
            "skip mailbox hint client because internal grpc listen address is not a socket address"
        );
        return Ok(None);
    };
    let client = match InternalGrpcMailboxClient::connect_peer(
        &peer_client,
        &target,
        Some("localhost"),
        vec![InternalAction::AgentManage.as_str().to_string()],
    )
    .await
    {
        Ok(client) => client,
        Err(err) => {
            tracing::debug!(
                target = %target,
                "skip mailbox hint client because internal grpc connect failed: {err}"
            );
            return Ok(None);
        }
    };
    Ok(Some(client))
}

pub(super) async fn maybe_notify_actor_new_mailbox_message_type_from_cli(
    manager: &TeamManager,
    config: &agenthub_config::AppConfig,
    send_result: &agenthub_team_actor::ActorSendResponse,
) -> anyhow::Result<()> {
    let Some(plan) = plan_actor_mailbox_immediate_hint(
        manager,
        send_result.message.run_id.as_str(),
        send_result,
    )
    .await?
    else {
        return Ok(());
    };
    let Some(client) = init_actor_mailbox_hint_client_from_config(config).await? else {
        tracing::debug!(
            run_id = %send_result.message.run_id,
            targets = ?plan.target_actor_ids,
            reason = ?plan.reason,
            "skip mailbox hint push because no agent input channel is available"
        );
        return Ok(());
    };
    let prompt =
        build_actor_mailbox_immediate_hint_prompt(send_result.message.run_id.as_str(), plan.reason);
    for target_actor_id in plan.target_actor_ids {
        if let Err(err) = client
            .send_agent_input(&target_actor_id, &prompt, None, None)
            .await
        {
            tracing::debug!(
                run_id = %send_result.message.run_id,
                actor_id = %target_actor_id,
                reason = ?plan.reason,
                "skip mailbox hint push because agent input is unavailable: {}",
                err
            );
        }
    }
    Ok(())
}

pub(super) async fn init_team_manager() -> anyhow::Result<(TeamManager, agenthub_config::AppConfig)>
{
    let db = agenthub_db::init_db().await?;
    let manager = TeamManager::new(db);
    let (config, _) = agenthub_config::AppConfig::load_with_info()?;
    let peer_client = build_actor_cli_internal_grpc_peer_client(&config)?;
    configure_actor_cli_internal_grpc(&manager, peer_client);
    Ok((manager, config))
}

pub(super) fn actor_runtime_internal_control_requested() -> bool {
    normalized_env_var(ACTOR_RUNTIME_ACTOR_ID_ENV).is_some()
        && normalized_env_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).is_some()
}

pub(super) async fn init_actor_mailbox_service(
    manager: &TeamManager,
    config: &agenthub_config::AppConfig,
    actor_id: &str,
    run_id: &str,
) -> anyhow::Result<Arc<dyn ActorMailboxService>> {
    if actor_runtime_internal_control_requested() {
        let client = connect_runtime_internal_mailbox_service(
            actor_id,
            Some(run_id),
            &[
                InternalAction::MessageSend,
                InternalAction::InboxList,
                InternalAction::MessageAck,
            ],
        )
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "team runtime mailbox control is unavailable because internal gRPC is not configured"
            )
        })?;
        return Ok(Arc::new(client));
    }
    let _ = config;
    Ok(Arc::new(manager.actor_mailbox_service()))
}

pub(super) async fn init_actor_permission_review_client(
    actor_id: &str,
) -> anyhow::Result<Option<InternalGrpcMailboxClient>> {
    if !actor_runtime_internal_control_requested() {
        return Ok(None);
    }
    connect_runtime_internal_mailbox_service(
        actor_id,
        normalized_env_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).as_deref(),
        &[InternalAction::PermissionReview],
    )
    .await
}

pub(super) async fn load_actor_inbox<S: ActorMailboxService + ?Sized>(
    service: &S,
    request: ActorInboxRequest,
    auto_ack: bool,
) -> Result<ActorInboxResponse, ActorServiceError> {
    if auto_ack {
        actor_inbox_with_auto_ack(service, request).await
    } else {
        service.actor_inbox(request).await
    }
}

pub(super) fn map_actor_service_error(operation: &str, err: ActorServiceError) -> anyhow::Error {
    anyhow::anyhow!("{operation} failed ({:?}): {}", err.code, err.message)
}
