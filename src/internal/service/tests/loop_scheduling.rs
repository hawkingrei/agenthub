use agenthub_agent_domain::loop_runtime::{
    LoopLimits, LoopPolicyState, LoopSessionPolicy, LoopSourceReferences,
};
use agenthub_agent_domain::loop_scheduling::{
    LoopRegistrationInput, LoopRegistrationReceipt, LoopSchedule,
};
use agenthub_db::loop_runtime::{LoopPolicyUpdate, LoopStore};

use super::loop_activation::fixture;
use super::loop_work::work_token;
use super::*;
use crate::internal::proto::agenthub::internal::v1::{
    GetLoopScheduleRequest, ListLoopSchedulesRequest, RegisterLoopScheduleRequest,
    RevokeLoopScheduleRequest,
};

#[tokio::test]
async fn loop_schedule_rpc_derives_provenance_and_authorizes_creation_inspection_and_revocation() {
    let (state, service, authz, run, reservation) = fixture().await;
    let now = chrono::Utc::now().timestamp();
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
            now,
        )
        .await
        .unwrap();
    let token = work_token(
        &authz,
        &run.id,
        &reservation,
        vec![InternalAction::TeamRead, InternalAction::LoopActivate],
    );
    let request = RegisterLoopScheduleRequest {
        member_id: "planner".into(),
        request_json: json!({"source_key":"review:1","schedule":{"kind":"due","due_at":now + 100}})
            .to_string(),
    };
    let created = service
        .register_loop_schedule(authenticated_request(request.clone(), &token))
        .await
        .unwrap()
        .into_inner();
    let created: LoopRegistrationReceipt = serde_json::from_str(&created.receipt_json).unwrap();
    assert_eq!(
        created
            .registration
            .input
            .references
            .scheduling_actor_id
            .as_deref(),
        Some("reviewer")
    );
    assert_eq!(
        created
            .registration
            .input
            .references
            .scheduling_activation_id,
        reservation.activation_id
    );
    assert!(
        created
            .registration
            .input
            .references
            .scheduling_user_id
            .is_none()
    );
    let again = service
        .register_loop_schedule(authenticated_request(request, &token))
        .await
        .unwrap()
        .into_inner();
    assert!(
        serde_json::from_str::<LoopRegistrationReceipt>(&again.receipt_json)
            .unwrap()
            .duplicate
    );
    let page = service
        .list_loop_schedules(authenticated_request(
            ListLoopSchedulesRequest {
                member_id: "planner".into(),
                after_registration_id: String::new(),
                limit: 1,
            },
            &token,
        ))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        serde_json::from_str::<Value>(&page.page_json).unwrap()["registrations"][0]["id"],
        created.registration.id
    );
    let detail = service
        .get_loop_schedule(authenticated_request(
            GetLoopScheduleRequest {
                registration_id: created.registration.id.clone(),
                after_firing_cursor: None,
                limit: 1,
            },
            &token,
        ))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        serde_json::from_str::<Value>(&detail.detail_json).unwrap()["registration"]["state"],
        "active"
    );
    let revoked = service
        .revoke_loop_schedule(authenticated_request(
            RevokeLoopScheduleRequest {
                registration_id: created.registration.id,
            },
            &token,
        ))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        serde_json::from_str::<Value>(&revoked.registration_json).unwrap()["state"],
        "revoked"
    );
    let other = store
        .register_schedule(
            &LoopRegistrationInput {
                actor_id: "planner".into(),
                team_id: run.team_id.clone(),
                source_key: "operator-registration".into(),
                schedule: LoopSchedule::Due { due_at: now + 100 },
                work_task_id: None,
                references: LoopSourceReferences::default(),
            },
            now,
        )
        .await
        .unwrap();
    assert_eq!(
        service
            .revoke_loop_schedule(authenticated_request(
                RevokeLoopScheduleRequest {
                    registration_id: other.registration.id.clone()
                },
                &token
            ))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::PermissionDenied
    );
    // Current coordinator authority is checked from the canonical spec, not a token role label.
    sqlx::query("UPDATE team_definitions SET spec_json = json_set(spec_json, '$.coordinator_member_id', 'reviewer') WHERE id = ?")
        .bind(&run.team_id).execute(&state.db).await.unwrap();
    service
        .revoke_loop_schedule(authenticated_request(
            RevokeLoopScheduleRequest {
                registration_id: other.registration.id,
            },
            &token,
        ))
        .await
        .unwrap();
}

#[tokio::test]
async fn loop_schedule_rpc_rejects_forged_identity_stale_credentials_and_unbounded_reads() {
    let (state, service, authz, run, reservation) = fixture().await;
    sqlx::query("UPDATE team_definitions SET spec_json = json_set(spec_json, '$.execution_mode', 'loop') WHERE id = ?")
        .bind(&run.team_id).execute(&state.db).await.unwrap();
    let token = work_token(
        &authz,
        &run.id,
        &reservation,
        vec![InternalAction::TeamRead, InternalAction::LoopActivate],
    );
    let read_only = work_token(
        &authz,
        &run.id,
        &reservation,
        vec![InternalAction::TeamRead],
    );
    let request = || RegisterLoopScheduleRequest {
        member_id: String::new(),
        request_json: json!({"source_key":"self","schedule":{"kind":"due","due_at":1900000000}})
            .to_string(),
    };
    assert_eq!(
        service
            .register_loop_schedule(authenticated_request(request(), &read_only))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::PermissionDenied
    );
    let mut forged = request();
    forged.request_json = json!({"source_key":"forged","scheduling_actor_id":"planner","schedule":{"kind":"due","due_at":1900000000}}).to_string();
    assert_eq!(
        service
            .register_loop_schedule(authenticated_request(forged, &token))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::InvalidArgument
    );
    let mut huge = request();
    huge.request_json = " ".repeat(16_385);
    assert_eq!(
        service
            .register_loop_schedule(authenticated_request(huge, &token))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::InvalidArgument
    );
    let mut foreign = request();
    foreign.member_id = "outside-team".into();
    assert_eq!(
        service
            .register_loop_schedule(authenticated_request(foreign, &token))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::PermissionDenied
    );
    assert_eq!(
        service
            .list_loop_schedules(authenticated_request(
                ListLoopSchedulesRequest {
                    member_id: String::new(),
                    after_registration_id: String::new(),
                    limit: 257
                },
                &token
            ))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::InvalidArgument
    );
    assert_eq!(
        service
            .get_loop_schedule(authenticated_request(
                GetLoopScheduleRequest {
                    registration_id: "unknown".into(),
                    after_firing_cursor: Some(-1),
                    limit: 1
                },
                &token
            ))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::InvalidArgument
    );
    let created = service
        .register_loop_schedule(authenticated_request(request(), &token))
        .await
        .unwrap()
        .into_inner();
    let created: LoopRegistrationReceipt = serde_json::from_str(&created.receipt_json).unwrap();
    assert_eq!(created.registration.input.actor_id, "reviewer");
    LoopStore::new(state.db.clone())
        .revoke_execution(&reservation, chrono::Utc::now().timestamp())
        .await
        .unwrap();
    assert_eq!(
        service
            .list_loop_schedules(authenticated_request(
                ListLoopSchedulesRequest {
                    member_id: String::new(),
                    after_registration_id: String::new(),
                    limit: 1
                },
                &token
            ))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::PermissionDenied
    );
}
