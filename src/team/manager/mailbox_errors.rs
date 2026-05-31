use agenthub_team_actor::{ActorMailboxError, ActorServiceError, ActorServiceErrorCode};
use sqlx::Error as SqlxError;

use super::mailbox::{TEAM_SPECIAL_USER_ACTOR_ALIAS, TEAM_SPECIAL_USER_ACTOR_PREFIX};
use super::mailbox_store::SqlActorMailboxStoreError;
use super::{TeamManager, TeamMemberSpecView};

pub(super) fn required_trimmed_field<'a>(
    value: &'a str,
    field: &'static str,
) -> Result<&'a str, ActorServiceError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ActorServiceError::new(
            ActorServiceErrorCode::BadRequest,
            format!("{field} is required"),
        ));
    }
    Ok(trimmed)
}

pub(super) fn optional_trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|raw| !raw.is_empty())
}

pub(super) fn validate_direct_mailbox_target_for_member_specs(
    member_specs: &[TeamMemberSpecView],
    to_actor_id: &str,
) -> Result<(), ActorServiceError> {
    let trimmed_target = to_actor_id.trim();
    if trimmed_target.is_empty() {
        return Err(ActorServiceError::new(
            ActorServiceErrorCode::BadRequest,
            "actor send target is required",
        ));
    }
    if trimmed_target == TEAM_SPECIAL_USER_ACTOR_ALIAS
        || trimmed_target
            .strip_prefix(TEAM_SPECIAL_USER_ACTOR_PREFIX)
            .is_some_and(|value| !value.trim().is_empty())
    {
        return Ok(());
    }
    if member_specs
        .iter()
        .any(|member| member.member_id == trimmed_target)
    {
        return Ok(());
    }
    let matching_role_member_ids = member_specs
        .iter()
        .filter(|member| member.role.eq_ignore_ascii_case(trimmed_target))
        .map(|member| member.member_id.as_str())
        .collect::<Vec<_>>();
    if matching_role_member_ids.len() == 1 {
        return Err(ActorServiceError::new(
            ActorServiceErrorCode::BadRequest,
            format!(
                "actor send target `{}` is not a canonical team member_id; use `{}` instead",
                trimmed_target, matching_role_member_ids[0]
            ),
        ));
    }
    if !matching_role_member_ids.is_empty() {
        return Err(ActorServiceError::new(
            ActorServiceErrorCode::BadRequest,
            format!(
                "actor send target `{}` matches multiple team members with role `{}`; matching member_ids: {}",
                trimmed_target,
                trimmed_target,
                matching_role_member_ids.join(", ")
            ),
        ));
    }
    let valid_member_ids = member_specs
        .iter()
        .map(|member| member.member_id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Err(ActorServiceError::new(
        ActorServiceErrorCode::BadRequest,
        format!(
            "actor send target `{}` must reference spec.members[].member_id or human mailbox `user` / `user:<id>`; valid member_ids: {}",
            trimmed_target, valid_member_ids
        ),
    ))
}

fn is_row_not_found(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<SqlxError>(),
        Some(SqlxError::RowNotFound)
    )
}

fn is_readonly_database_error(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        if let Some(SqlxError::Database(db_err)) = cause.downcast_ref::<SqlxError>()
            && let Some(code) = db_err.code().and_then(|raw| raw.parse::<i32>().ok())
            && (code & 0xff) == 8
        {
            return true;
        }

        cause
            .to_string()
            .contains("attempt to write a readonly database")
    })
}

pub(super) fn map_actor_service_error(err: anyhow::Error) -> ActorServiceError {
    if TeamManager::is_actor_message_idempotency_conflict(&err) {
        return ActorServiceError::new(
            ActorServiceErrorCode::Conflict,
            "idempotency_key conflicts with an existing message payload",
        );
    }
    if let Some(owner_actor_id) = TeamManager::actor_thread_claim_conflict_owner(&err) {
        return ActorServiceError::new(
            ActorServiceErrorCode::Conflict,
            format!(
                "mailbox topic is already claimed by actor `{owner_actor_id}`; switch to watch/release flow or wait for lease expiry"
            ),
        );
    }
    if err
        .downcast_ref::<SqlActorMailboxStoreError>()
        .is_some_and(|cause| {
            matches!(
                cause,
                SqlActorMailboxStoreError::ThreadClaimOwnershipRequired
            )
        })
    {
        return ActorServiceError::new(
            ActorServiceErrorCode::Conflict,
            "mailbox topic must be actively claimed by the acting actor before release or complete",
        );
    }
    if err
        .downcast_ref::<SqlActorMailboxStoreError>()
        .is_some_and(|cause| {
            matches!(
                cause,
                SqlActorMailboxStoreError::ReplyRequiredVisibleOutcomeMissing
            )
        })
    {
        return ActorServiceError::new(
            ActorServiceErrorCode::UnprocessableEntity,
            "reply-required mailbox work cannot be completed before a visible reply is emitted or the item is explicitly escalated/transferred/taken over",
        );
    }
    if err
        .downcast_ref::<SqlActorMailboxStoreError>()
        .is_some_and(|cause| {
            matches!(
                cause,
                SqlActorMailboxStoreError::ReplyRequiredIgnoreReasonMissing
            )
        })
    {
        return ActorServiceError::new(
            ActorServiceErrorCode::UnprocessableEntity,
            "reply-required mailbox work cannot be ignored without an explicit reason",
        );
    }
    if err
        .downcast_ref::<SqlActorMailboxStoreError>()
        .is_some_and(|cause| {
            matches!(
                cause,
                SqlActorMailboxStoreError::ThreadClaimTakeoverRequired
            )
        })
    {
        return ActorServiceError::new(
            ActorServiceErrorCode::BadRequest,
            "mailbox takeover requires a currently claimed topic",
        );
    }
    if err
        .downcast_ref::<SqlActorMailboxStoreError>()
        .is_some_and(|cause| {
            matches!(
                cause,
                SqlActorMailboxStoreError::ReplyRequiredEscalationAlreadyAtCoordinator
            )
        })
    {
        return ActorServiceError::new(
            ActorServiceErrorCode::BadRequest,
            "reply-required mailbox work is already owned by the coordinator",
        );
    }
    if err
        .downcast_ref::<SqlActorMailboxStoreError>()
        .is_some_and(|cause| {
            matches!(
                cause,
                SqlActorMailboxStoreError::ReplyRequiredEscalationTargetUnavailable
            )
        })
    {
        return ActorServiceError::new(
            ActorServiceErrorCode::BadRequest,
            "run team cannot resolve a coordinator mailbox delivery target",
        );
    }
    if is_row_not_found(&err) {
        return ActorServiceError::new(ActorServiceErrorCode::NotFound, "message not found");
    }
    if is_readonly_database_error(&err) {
        return ActorServiceError::new(
            ActorServiceErrorCode::Internal,
            "mailbox write failed: attempt to write a readonly database",
        );
    }
    let detail_chain = err
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>();
    ActorServiceError::new(
        ActorServiceErrorCode::Internal,
        format!("internal actor mailbox error: {}", detail_chain.join(" | ")),
    )
}

pub(super) fn map_actor_mailbox_store_error(
    err: ActorMailboxError<SqlActorMailboxStoreError>,
) -> anyhow::Error {
    match err {
        ActorMailboxError::Store(store_err) => match store_err {
            SqlActorMailboxStoreError::Sql(sql_err) => anyhow::Error::new(sql_err),
            SqlActorMailboxStoreError::IdempotencyConflict => {
                anyhow::Error::new(SqlActorMailboxStoreError::IdempotencyConflict)
            }
            SqlActorMailboxStoreError::ThreadClaimConflict { owner_actor_id } => {
                anyhow::Error::new(SqlActorMailboxStoreError::ThreadClaimConflict {
                    owner_actor_id,
                })
            }
            SqlActorMailboxStoreError::ThreadClaimOwnershipRequired => {
                anyhow::Error::new(SqlActorMailboxStoreError::ThreadClaimOwnershipRequired)
            }
            SqlActorMailboxStoreError::ThreadClaimTakeoverRequired => {
                anyhow::Error::new(SqlActorMailboxStoreError::ThreadClaimTakeoverRequired)
            }
            SqlActorMailboxStoreError::ReplyRequiredVisibleOutcomeMissing => {
                anyhow::Error::new(SqlActorMailboxStoreError::ReplyRequiredVisibleOutcomeMissing)
            }
            SqlActorMailboxStoreError::ReplyRequiredIgnoreReasonMissing => {
                anyhow::Error::new(SqlActorMailboxStoreError::ReplyRequiredIgnoreReasonMissing)
            }
            SqlActorMailboxStoreError::ReplyRequiredEscalationAlreadyAtCoordinator => {
                anyhow::Error::new(
                    SqlActorMailboxStoreError::ReplyRequiredEscalationAlreadyAtCoordinator,
                )
            }
            SqlActorMailboxStoreError::ReplyRequiredEscalationTargetUnavailable => {
                anyhow::Error::new(
                    SqlActorMailboxStoreError::ReplyRequiredEscalationTargetUnavailable,
                )
            }
        },
    }
}
