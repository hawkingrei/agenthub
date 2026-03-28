use super::*;

#[test]
fn resolve_team_leader_member_id_supports_legacy_fallbacks() {
    assert_eq!(
        super::resolve_team_leader_member_id(&json!({
            "members":[{"member_id":"planner","role":"leader"}]
        }))
        .expect("resolve from role"),
        "planner"
    );
    assert_eq!(
        super::resolve_team_leader_member_id(&json!({
            "entrypoint":"planner"
        }))
        .expect("resolve from entrypoint"),
        "planner"
    );
}

#[test]
fn actor_service_error_code_maps_to_expected_grpc_status() {
    let cases = [
        (
            agenthub_team_actor::ActorServiceErrorCode::BadRequest,
            Code::InvalidArgument,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::UnprocessableEntity,
            Code::InvalidArgument,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::Unauthorized,
            Code::Unauthenticated,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::Forbidden,
            Code::PermissionDenied,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::NotFound,
            Code::NotFound,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::Conflict,
            Code::AlreadyExists,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::Gone,
            Code::FailedPrecondition,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::TooManyRequests,
            Code::ResourceExhausted,
        ),
        (
            agenthub_team_actor::ActorServiceErrorCode::Internal,
            Code::Internal,
        ),
    ];

    for (actor_code, grpc_code) in cases {
        let status = map_actor_service_status(agenthub_team_actor::ActorServiceError::new(
            actor_code, "boom",
        ));
        assert_eq!(status.code(), grpc_code);
    }
}

#[test]
fn parse_team_task_status_reports_trimmed_input() {
    let err = super::super::parse_team_task_status("  waiting  ").expect_err("invalid status");
    assert_eq!(
        err.message(),
        "invalid task status 'waiting', expected one of: open, in_progress, in_review, completed, canceled"
    );
}
