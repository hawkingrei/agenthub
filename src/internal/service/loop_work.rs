use agenthub_db::loop_runtime::{LoopStore, LoopStoreError};

use super::*;

impl TeamInternalControlService {
    pub(super) async fn get_loop_work_source_request(
        &self,
        request: Request<GetLoopWorkSourceRequest>,
    ) -> Result<Response<GetLoopWorkSourceResponse>, Status> {
        let (principal, _guard) = self
            .authenticate_execution(request.metadata(), false)
            .await?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamRead)?;
        let execution = principal
            .loop_execution
            .as_ref()
            .ok_or_else(|| Status::permission_denied("activation-scoped credentials required"))?;
        let source_id = request.into_inner().source_id;
        validate_identifier(&source_id)?;
        let reservation = LoopStore::new(self.deps.db.clone())
            .executor_reservation(
                principal.actor_id.as_deref().unwrap_or_default(),
                principal.run_id.as_deref().unwrap_or_default(),
                &execution.activation_id,
                execution.generation,
            )
            .await
            .map_err(map_loop_work_error)?;
        let detail = self
            .deps
            .teams
            .loop_work_source(&reservation, &source_id)
            .await
            .map_err(map_loop_work_error)?;
        Ok(Response::new(GetLoopWorkSourceResponse {
            source_json: serde_json::to_string(&detail).map_err(map_serde_status)?,
        }))
    }

    pub(super) async fn get_loop_work_request(
        &self,
        request: Request<GetLoopWorkRequest>,
    ) -> Result<Response<GetLoopWorkResponse>, Status> {
        let (principal, _guard) = self
            .authenticate_execution(request.metadata(), false)
            .await?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamRead)?;
        let execution = principal
            .loop_execution
            .as_ref()
            .ok_or_else(|| Status::permission_denied("activation-scoped credentials required"))?;
        let store = LoopStore::new(self.deps.db.clone());
        let reservation = store
            .executor_reservation(
                principal.actor_id.as_deref().unwrap_or_default(),
                principal.run_id.as_deref().unwrap_or_default(),
                &execution.activation_id,
                execution.generation,
            )
            .await
            .map_err(map_loop_work_error)?;
        let input = request.into_inner();
        if let Some(cursor) = optional_trimmed(&input.after_source_id) {
            validate_identifier(cursor)?;
        }
        if input.limit > 256 {
            return Err(Status::invalid_argument("limit must not exceed 256"));
        }
        let page = store
            .work_context(
                &reservation,
                optional_trimmed(&input.after_source_id),
                if input.limit == 0 { 64 } else { input.limit },
                chrono::Utc::now().timestamp(),
            )
            .await
            .map_err(map_loop_work_error)?;
        Ok(Response::new(GetLoopWorkResponse {
            page_json: serde_json::to_string(&page).map_err(map_serde_status)?,
        }))
    }

    pub(super) async fn activate_loop_member_request(
        &self,
        request: Request<ActivateLoopMemberRequest>,
    ) -> Result<Response<ActivateLoopMemberResponse>, Status> {
        let (principal, _guard) = self
            .authenticate_execution(request.metadata(), false)
            .await?;
        self.authz
            .ensure_permission(&principal, InternalAction::LoopActivate)?;
        if principal.loop_execution.is_none() {
            return Err(Status::permission_denied(
                "activation-scoped credentials required",
            ));
        }
        let context = self
            .load_team_context(
                &principal,
                None,
                None,
                principal.actor_id.as_deref().unwrap_or_default(),
            )
            .await?;
        let input = request.into_inner();
        let member = required_field(&input.member_id, "member_id")?;
        let key = required_field(&input.source_key, "source_key")?;
        for id in std::iter::once(member)
            .chain(std::iter::once(key))
            .chain(optional_trimmed(&input.task_id))
        {
            validate_identifier(id)?;
        }
        ensure_team_member_access(&self.deps.teams, &context.team_id, member).await?;
        let receipt = self
            .deps
            .teams
            .request_loop_activation(
                &context.team_id,
                member,
                key,
                optional_trimmed(&input.task_id),
            )
            .await
            .map_err(map_loop_work_error)?;
        Ok(Response::new(ActivateLoopMemberResponse {
            receipt_json: serde_json::to_string(&receipt).map_err(map_serde_status)?,
        }))
    }
}

pub(super) fn map_loop_work_error(error: anyhow::Error) -> Status {
    match error.downcast_ref::<LoopStoreError>() {
        Some(LoopStoreError::Capacity) => Status::resource_exhausted(error.to_string()),
        Some(LoopStoreError::ScopeMismatch) => Status::permission_denied(error.to_string()),
        Some(_) => Status::failed_precondition(error.to_string()),
        None => {
            tracing::error!(error = %error, "loop work request failed");
            Status::internal("loop work request failed")
        }
    }
}

fn validate_identifier(value: &str) -> Result<(), Status> {
    agenthub_agent_domain::loop_runtime::validate_loop_id(value)
        .map_err(|_| Status::invalid_argument("invalid bounded loop identifier"))
}
