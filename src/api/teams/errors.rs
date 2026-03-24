use agenthub_team_actor::{ActorServiceError, ActorServiceErrorCode};
use sqlx::Error as SqlxError;

use crate::api::error::ApiError;
use crate::team::{TeamRunResumeError, TeamRuntimeStartError};

pub(super) fn map_create_team_error(err: anyhow::Error) -> ApiError {
    if is_unique_team_name_violation(&err) {
        return ApiError::conflict("team name already exists");
    }
    map_team_internal_error(err)
}

pub(super) fn map_submit_step_error(err: anyhow::Error) -> ApiError {
    if is_unique_step_attempt_violation(&err) {
        return ApiError::conflict("step already exists for run");
    }
    map_team_internal_error(err)
}

pub(super) fn map_actor_service_api_error(err: ActorServiceError) -> ApiError {
    match err.code {
        ActorServiceErrorCode::BadRequest | ActorServiceErrorCode::UnprocessableEntity => {
            ApiError::bad_request(&err.message)
        }
        ActorServiceErrorCode::Unauthorized | ActorServiceErrorCode::Forbidden => {
            ApiError::unauthorized(&err.message)
        }
        ActorServiceErrorCode::NotFound | ActorServiceErrorCode::Gone => {
            ApiError::not_found(&err.message)
        }
        ActorServiceErrorCode::Conflict => ApiError::conflict(&err.message),
        ActorServiceErrorCode::TooManyRequests | ActorServiceErrorCode::Internal => {
            map_team_internal_error(anyhow::anyhow!("{}", err.message))
        }
    }
}

pub(super) fn map_not_found_error(err: anyhow::Error, msg: &str) -> ApiError {
    if is_row_not_found(&err) {
        return ApiError::not_found(msg);
    }
    map_team_internal_error(err)
}

pub(super) fn map_resume_run_error(err: anyhow::Error) -> ApiError {
    if matches!(
        err.downcast_ref::<TeamRunResumeError>(),
        Some(TeamRunResumeError::CompletedRun)
    ) {
        return ApiError::conflict("completed run cannot be resumed; use restart");
    }
    map_not_found_error(err, "run not found")
}

pub(super) fn map_runtime_start_error(err: anyhow::Error) -> ApiError {
    if let Some(runtime_err) = err.downcast_ref::<TeamRuntimeStartError>() {
        tracing::warn!("team runtime start rejected: {}", runtime_err);
        return match runtime_err {
            TeamRuntimeStartError::InvalidConfig(_)
            | TeamRuntimeStartError::MissingMemberAgent(_) => {
                ApiError::bad_request(&runtime_err.to_string())
            }
            TeamRuntimeStartError::MemberRuntimeStart(_) => {
                ApiError::conflict(&runtime_err.to_string())
            }
        };
    }
    map_team_internal_error(err)
}

pub(super) fn map_team_internal_error(err: anyhow::Error) -> ApiError {
    tracing::error!("team api internal error: {}", err);
    ApiError::from(anyhow::anyhow!("internal server error"))
}

fn is_row_not_found(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<SqlxError>(),
        Some(SqlxError::RowNotFound)
    )
}

fn is_unique_team_name_violation(err: &anyhow::Error) -> bool {
    is_unique_violation_for(err, "team_definitions.name")
}

fn is_unique_step_attempt_violation(err: &anyhow::Error) -> bool {
    is_unique_violation_for(
        err,
        "team_steps.run_id, team_steps.step_key, team_steps.attempt",
    )
}

fn is_unique_violation_for(err: &anyhow::Error, constraint: &str) -> bool {
    match err.downcast_ref::<SqlxError>() {
        Some(SqlxError::Database(db_err)) => {
            db_err.code().as_deref() == Some(super::SQLITE_CONSTRAINT_UNIQUE_CODE)
                && db_err.message().contains(constraint)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use axum::response::IntoResponse;

    use super::map_runtime_start_error;
    use crate::team::TeamRuntimeStartError;

    #[test]
    fn map_runtime_start_error_maps_typed_runtime_config_errors_to_bad_request() {
        let api_err = map_runtime_start_error(
            TeamRuntimeStartError::InvalidConfig("bad runtime config".to_string()).into(),
        );
        assert_eq!(
            api_err.into_response().status(),
            axum::http::StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn map_runtime_start_error_maps_member_runtime_failures_to_conflict() {
        let api_err = map_runtime_start_error(
            TeamRuntimeStartError::MemberRuntimeStart("member runtime failed".to_string()).into(),
        );
        assert_eq!(
            api_err.into_response().status(),
            axum::http::StatusCode::CONFLICT
        );
    }

    #[test]
    fn map_runtime_start_error_keeps_unknown_errors_internal() {
        let api_err = map_runtime_start_error(anyhow::anyhow!("unexpected"));
        assert_eq!(
            api_err.into_response().status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
