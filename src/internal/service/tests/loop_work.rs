use super::loop_activation::fixture;
use super::*;
use crate::internal::auth::LoopExecutionClaims;
use crate::internal::p2p::NodeCredentialRequest;
use agenthub_agent_domain::loop_runtime::{
    LoopLimits, LoopPolicyState, LoopReservation, LoopSessionPolicy, LoopTriggerKind,
};
use agenthub_db::loop_runtime::{LoopPolicyUpdate, LoopStore};

pub(super) fn work_token(
    authz: &InternalAuthz,
    run: &str,
    reservation: &LoopReservation,
    permissions: Vec<InternalAction>,
) -> String {
    authz
        .issue_loop_access_token(
            NodeCredentialRequest {
                source_node_id: "main".into(),
                role: "worker".into(),
                actor_id: Some("reviewer".into()),
                run_id: Some(run.into()),
                permissions: permissions
                    .into_iter()
                    .map(|action| action.as_str().into())
                    .collect(),
                scope: vec![],
                audience: vec![],
                ttl_seconds: 600,
            },
            LoopExecutionClaims {
                activation_id: reservation.activation_id.clone().unwrap(),
                generation: reservation.generation,
            },
        )
        .unwrap()
        .access_token
}

#[tokio::test]
async fn loop_work_rpc_uses_signed_provenance_and_idempotent_business_requests() {
    use crate::internal::proto::agenthub::internal::v1::{
        ActivateLoopMemberRequest, GetLoopWorkRequest, GetLoopWorkSourceRequest,
    };
    let (state, service, authz, run, reservation) = fixture().await;
    sqlx::query("UPDATE team_definitions SET spec_json = json_set(spec_json, '$.execution_mode', 'loop') WHERE id = ?")
        .bind(&run.team_id).execute(&state.db).await.unwrap();
    let store = LoopStore::new(state.db.clone());
    store
        .configure(
            LoopPolicyUpdate {
                actor_id: "planner",
                team_id: &run.team_id,
                expected_revision: 0,
                state: LoopPolicyState::Suspended,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits::default(),
            },
            chrono::Utc::now().timestamp(),
        )
        .await
        .unwrap();
    let credential = work_token(
        &authz,
        &run.id,
        &reservation,
        vec![InternalAction::TeamRead, InternalAction::LoopActivate],
    );
    let page = service
        .get_loop_work(authenticated_request(
            GetLoopWorkRequest {
                after_source_id: String::new(),
                limit: 1,
            },
            &credential,
        ))
        .await
        .unwrap()
        .into_inner();
    let page: agenthub_agent_domain::loop_runtime::LoopWorkPage =
        serde_json::from_str(&page.page_json).unwrap();
    assert_eq!(
        page.activation.id,
        reservation.activation_id.as_deref().unwrap()
    );
    assert_eq!(page.sources.len(), 1);
    service
        .get_loop_work_source(authenticated_request(
            GetLoopWorkSourceRequest {
                source_id: page.sources[0].id.clone(),
            },
            &credential,
        ))
        .await
        .unwrap();
    let input = ActivateLoopMemberRequest {
        member_id: "planner".into(),
        source_key: "report:1".into(),
        task_id: String::new(),
    };
    let first = service
        .activate_loop_member(authenticated_request(input.clone(), &credential))
        .await
        .unwrap()
        .into_inner();
    let first: agenthub_agent_domain::loop_runtime::LoopTriggerReceipt =
        serde_json::from_str(&first.receipt_json).unwrap();
    let second = service
        .activate_loop_member(authenticated_request(input, &credential))
        .await
        .unwrap()
        .into_inner();
    let second: agenthub_agent_domain::loop_runtime::LoopTriggerReceipt =
        serde_json::from_str(&second.receipt_json).unwrap();
    assert!(second.duplicate);
    assert_eq!(first.trigger_id, second.trigger_id);
    let source = store
        .triggers(&run.team_id, &first.activation_id)
        .await
        .unwrap()
        .remove(0);
    assert_eq!(
        source.input.references.scheduling_actor_id.as_deref(),
        Some("reviewer")
    );
    assert_eq!(
        source.input.references.scheduling_activation_id,
        reservation.activation_id
    );
    assert!(source.input.references.scheduling_user_id.is_none());
    assert_eq!(source.input.kind, LoopTriggerKind::MemberRequest);
    let error = service
        .get_loop_work_source(authenticated_request(
            GetLoopWorkSourceRequest {
                source_id: first.trigger_id,
            },
            &credential,
        ))
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::PermissionDenied);
    let tasks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM team_tasks")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(tasks, 0);
}

#[tokio::test]
async fn loop_work_rpc_rejects_legacy_stale_out_of_team_and_over_budget_requests() {
    use crate::internal::proto::agenthub::internal::v1::{
        ActivateLoopMemberRequest, GetLoopWorkRequest,
    };
    let (state, service, authz, run, reservation) = fixture().await;
    let credential = work_token(
        &authz,
        &run.id,
        &reservation,
        vec![InternalAction::TeamRead, InternalAction::LoopActivate],
    );
    let read = || GetLoopWorkRequest {
        after_source_id: String::new(),
        limit: 1,
    };
    let legacy = issue_token(
        &authz,
        InternalRole::Worker,
        Some("reviewer"),
        Some(&run.id),
    );
    assert_eq!(
        service
            .get_loop_work(authenticated_request(read(), &legacy))
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    let denied = work_token(
        &authz,
        &run.id,
        &reservation,
        vec![InternalAction::TeamRead],
    );
    let request = || ActivateLoopMemberRequest {
        member_id: "planner".into(),
        source_key: "one".into(),
        task_id: String::new(),
    };
    assert_eq!(
        service
            .activate_loop_member(authenticated_request(request(), &denied))
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    assert_eq!(
        service
            .get_loop_work(authenticated_request(
                GetLoopWorkRequest {
                    limit: 257,
                    ..read()
                },
                &credential
            ))
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    assert_eq!(
        service
            .activate_loop_member(authenticated_request(
                ActivateLoopMemberRequest {
                    member_id: "foreign".into(),
                    ..request()
                },
                &credential
            ))
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    sqlx::query("UPDATE team_definitions SET spec_json = json_set(spec_json, '$.execution_mode', 'loop') WHERE id = ?")
        .bind(&run.team_id).execute(&state.db).await.unwrap();
    let store = LoopStore::new(state.db.clone());
    store
        .configure(
            LoopPolicyUpdate {
                actor_id: "planner",
                team_id: &run.team_id,
                expected_revision: 0,
                state: LoopPolicyState::Suspended,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits {
                    pending_per_actor: 1,
                    sources_per_activation: 1,
                    ..Default::default()
                },
            },
            chrono::Utc::now().timestamp(),
        )
        .await
        .unwrap();
    service
        .activate_loop_member(authenticated_request(request(), &credential))
        .await
        .unwrap();
    assert_eq!(
        service
            .activate_loop_member(authenticated_request(
                ActivateLoopMemberRequest {
                    source_key: "two".into(),
                    ..request()
                },
                &credential
            ))
            .await
            .unwrap_err()
            .code(),
        Code::ResourceExhausted
    );
    sqlx::query(
        "UPDATE loop_execution_reservations SET lease_expires_at = 1 WHERE actor_id = 'reviewer'",
    )
    .execute(&state.db)
    .await
    .unwrap();
    assert_eq!(
        service
            .get_loop_work(authenticated_request(read(), &credential))
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
}
