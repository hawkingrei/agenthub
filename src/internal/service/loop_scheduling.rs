use agenthub_agent_domain::loop_runtime::validate_loop_id;
use agenthub_agent_domain::loop_scheduling::{LoopRegistrationInput, LoopScheduleRequest};
use agenthub_db::loop_runtime::LoopStore;

use super::loop_work::map_loop_work_error;
use super::*;

impl TeamInternalControlService {
    pub(super) async fn register_loop_schedule_request(
        &self,
        request: Request<RegisterLoopScheduleRequest>,
    ) -> Result<Response<RegisterLoopScheduleResponse>, Status> {
        let (principal, _guard) = self
            .authenticate_execution(request.metadata(), false)
            .await?;
        self.authz
            .ensure_permission(&principal, InternalAction::LoopActivate)?;
        require_executor(&principal)?;
        let context = self
            .load_team_context(
                &principal,
                None,
                None,
                principal.actor_id.as_deref().unwrap_or_default(),
            )
            .await?;
        let input = request.into_inner();
        let member = optional_trimmed(&input.member_id)
            .unwrap_or_else(|| principal.actor_id.as_deref().unwrap_or_default());
        validate_id(member)?;
        if input.request_json.len() > 16_384 {
            return Err(Status::invalid_argument(
                "schedule request exceeds 16384 bytes",
            ));
        }
        let intent: LoopScheduleRequest = serde_json::from_str(&input.request_json)
            .map_err(|_| Status::invalid_argument("invalid schedule request"))?;
        intent
            .validate()
            .map_err(|_| Status::invalid_argument("invalid bounded schedule"))?;
        let receipt = self
            .deps
            .teams
            .request_loop_schedule(&context.team_id, member, &intent)
            .await
            .map_err(map_loop_work_error)?;
        Ok(Response::new(RegisterLoopScheduleResponse {
            receipt_json: serde_json::to_string(&receipt).map_err(map_serde_status)?,
        }))
    }

    pub(super) async fn list_loop_schedules_request(
        &self,
        request: Request<ListLoopSchedulesRequest>,
    ) -> Result<Response<ListLoopSchedulesResponse>, Status> {
        let (principal, _guard) = self
            .authenticate_execution(request.metadata(), false)
            .await?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamRead)?;
        require_executor(&principal)?;
        let context = self
            .load_team_context(
                &principal,
                None,
                None,
                principal.actor_id.as_deref().unwrap_or_default(),
            )
            .await?;
        let input = request.into_inner();
        let member = optional_trimmed(&input.member_id)
            .unwrap_or_else(|| principal.actor_id.as_deref().unwrap_or_default());
        validate_id(member)?;
        ensure_team_member_access(&self.deps.teams, &context.team_id, member).await?;
        let after = optional_trimmed(&input.after_registration_id);
        if let Some(id) = after {
            validate_id(id)?;
        }
        let page = LoopStore::new(self.deps.db.clone())
            .registrations(&context.team_id, member, after, page_limit(input.limit)?)
            .await
            .map_err(map_loop_work_error)?;
        Ok(Response::new(ListLoopSchedulesResponse {
            page_json: serde_json::to_string(&page).map_err(map_serde_status)?,
        }))
    }

    pub(super) async fn get_loop_schedule_request(
        &self,
        request: Request<GetLoopScheduleRequest>,
    ) -> Result<Response<GetLoopScheduleResponse>, Status> {
        let (principal, _guard) = self
            .authenticate_execution(request.metadata(), false)
            .await?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamRead)?;
        require_executor(&principal)?;
        let context = self
            .load_team_context(
                &principal,
                None,
                None,
                principal.actor_id.as_deref().unwrap_or_default(),
            )
            .await?;
        let input = request.into_inner();
        validate_id(&input.registration_id)?;
        if input.after_firing_cursor.is_some_and(|cursor| cursor < 0) {
            return Err(Status::invalid_argument("invalid firing cursor"));
        }
        let detail = LoopStore::new(self.deps.db.clone())
            .registration_detail(
                &context.team_id,
                &input.registration_id,
                input.after_firing_cursor,
                page_limit(input.limit)?,
            )
            .await
            .map_err(map_loop_work_error)?;
        Ok(Response::new(GetLoopScheduleResponse {
            detail_json: serde_json::to_string(&detail).map_err(map_serde_status)?,
        }))
    }

    pub(super) async fn revoke_loop_schedule_request(
        &self,
        request: Request<RevokeLoopScheduleRequest>,
    ) -> Result<Response<RevokeLoopScheduleResponse>, Status> {
        let (principal, _guard) = self
            .authenticate_execution(request.metadata(), false)
            .await?;
        self.authz
            .ensure_permission(&principal, InternalAction::LoopActivate)?;
        require_executor(&principal)?;
        let actor = principal.actor_id.as_deref().unwrap_or_default();
        let context = self
            .load_team_context(&principal, None, None, actor)
            .await?;
        let input = request.into_inner();
        validate_id(&input.registration_id)?;
        let mut tx = self
            .deps
            .db
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(map_schedule_sql_error)?;
        let raw: String = sqlx::query_scalar(
            "SELECT input_json FROM loop_registrations WHERE team_id = ? AND id = ?",
        )
        .bind(&context.team_id)
        .bind(&input.registration_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_schedule_sql_error)?
        .ok_or_else(|| Status::permission_denied("registration is outside current scope"))?;
        let registration: LoopRegistrationInput =
            serde_json::from_str(&raw).map_err(map_serde_status)?;
        let raw_spec: String =
            sqlx::query_scalar("SELECT spec_json FROM team_definitions WHERE id = ?")
                .bind(&context.team_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_schedule_sql_error)?;
        let spec: Value = serde_json::from_str(&raw_spec).map_err(map_serde_status)?;
        if registration.actor_id != actor
            && registration.references.scheduling_actor_id.as_deref() != Some(actor)
            && resolve_team_coordinator_member_id(&spec)?.as_str() != actor
        {
            return Err(Status::permission_denied(
                "only the target, creator, or current coordinator can revoke a schedule",
            ));
        }
        let registration = LoopStore::revoke_schedule_tx(
            &mut tx,
            &context.team_id,
            &input.registration_id,
            chrono::Utc::now().timestamp(),
        )
        .await
        .map_err(map_loop_work_error)?;
        tx.commit().await.map_err(map_schedule_sql_error)?;
        Ok(Response::new(RevokeLoopScheduleResponse {
            registration_json: serde_json::to_string(&registration).map_err(map_serde_status)?,
        }))
    }
}

fn require_executor(principal: &super::super::auth::InternalPrincipal) -> Result<(), Status> {
    if principal.loop_execution.is_none() {
        return Err(Status::permission_denied(
            "activation-scoped credentials required",
        ));
    }
    Ok(())
}

fn validate_id(value: &str) -> Result<(), Status> {
    validate_loop_id(value).map_err(|_| Status::invalid_argument("invalid bounded loop identifier"))
}

fn page_limit(limit: u32) -> Result<u32, Status> {
    if limit > 256 {
        return Err(Status::invalid_argument("limit must not exceed 256"));
    }
    Ok(if limit == 0 { 64 } else { limit })
}

fn map_schedule_sql_error(error: sqlx::Error) -> Status {
    map_loop_work_error(error.into())
}
