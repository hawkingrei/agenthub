use serde_json::Value;

use super::mailbox::{map_actor_service_error, validate_direct_mailbox_target_for_member_specs};
use super::{TeamManager, TeamMemberSpecView, parse_team_member_specs};
use agenthub_team_actor::{ActorServiceError, ActorServiceErrorCode};

#[derive(Clone)]
pub struct TeamActorMailboxService {
    pub(super) manager: TeamManager,
}

impl TeamActorMailboxService {
    pub fn new(manager: TeamManager) -> Self {
        Self { manager }
    }

    pub(super) async fn validate_direct_send_target(
        &self,
        run_id: &str,
        to_actor_id: &str,
    ) -> Result<(), ActorServiceError> {
        let member_specs = self
            .load_member_specs_for_run(run_id)
            .await
            .map_err(map_actor_service_error)?;
        validate_direct_mailbox_target_for_member_specs(&member_specs, to_actor_id)
    }

    pub(super) fn validate_direct_remote_route(
        route: Option<&Value>,
    ) -> Result<(), ActorServiceError> {
        let Some(route) = route else {
            return Err(ActorServiceError::new(
                ActorServiceErrorCode::BadRequest,
                "route is required for remote transport",
            ));
        };
        let Some(object) = route.as_object() else {
            return Err(ActorServiceError::new(
                ActorServiceErrorCode::BadRequest,
                "route must be a JSON object for remote transport",
            ));
        };
        let has_http_route = object
            .get("endpoint")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());
        let has_grpc_route = object
            .get("grpc_target")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());
        if has_http_route || has_grpc_route {
            return Ok(());
        }
        Err(ActorServiceError::new(
            ActorServiceErrorCode::BadRequest,
            "route must contain endpoint or grpc_target for remote transport",
        ))
    }

    pub(super) async fn load_member_specs_for_run(
        &self,
        run_id: &str,
    ) -> anyhow::Result<Vec<TeamMemberSpecView>> {
        let team_id =
            sqlx::query_scalar::<_, String>("SELECT team_id FROM team_runs WHERE id = ?1")
                .bind(run_id)
                .fetch_optional(&self.manager.db)
                .await?
                .ok_or_else(|| anyhow::anyhow!("run not found"))?;
        let team = self.manager.get_team(&team_id).await?;
        parse_team_member_specs(&team.spec)
    }
}
