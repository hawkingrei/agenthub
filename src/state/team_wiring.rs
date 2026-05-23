use std::path::Path;
use std::sync::Arc;

use crate::acp::AcpPermissionService;
use crate::agent::AgentManager;
use crate::team::{
    TeamManager, TeamPermissionReviewDispatcher, TeamPermissionReviewDispatcherSettings,
    TeamRemoteRelayWorkerSettings,
};

pub(super) fn configure_team_services(
    teams: &Arc<TeamManager>,
    agents: &Arc<AgentManager>,
    acp_permissions: &Arc<AcpPermissionService>,
    internal_peer_client: Option<&crate::internal::client::InternalGrpcPeerClientConfig>,
) {
    if let Some(peer_client) = internal_peer_client {
        teams.configure_internal_grpc_relay(
            Path::new(&peer_client.cert_dir),
            peer_client.security_mode,
        );
        teams.configure_internal_grpc_peer_client(Some(peer_client.clone()));
    }
    teams
        .clone()
        .spawn_remote_relay_worker(TeamRemoteRelayWorkerSettings::default());
    agents.set_permission_review_dispatcher(Some(Arc::new(TeamPermissionReviewDispatcher::new(
        teams.clone(),
        agents.clone(),
        acp_permissions.clone(),
        TeamPermissionReviewDispatcherSettings::default(),
    ))));
}
