use crate::internal::auth::LoopExecutionClaims;
use crate::internal::p2p::NodeCredentialRequest;
use crate::internal::proto::agenthub::internal::v1::FinishLoopActivationRequest;
use agenthub_agent_domain::loop_runtime::{
    LoopAdmission, LoopCleanupDisposition, LoopLimits, LoopOutcome, LoopOutcomeKind,
    LoopPolicyState, LoopReservation, LoopSessionPolicy, LoopSourceReferences, LoopTriggerInput,
    LoopTriggerKind,
};
use agenthub_db::loop_runtime::{LoopPolicyUpdate, LoopStore};

use super::*;

async fn fixture() -> (
    crate::state::AppState,
    TeamInternalControlService,
    InternalAuthz,
    crate::team::TeamRunRecord,
    LoopReservation,
) {
    let state = build_test_state().await;
    let run = create_team_run(&state).await;
    let store = LoopStore::new(state.db.clone());
    let now = chrono::Utc::now().timestamp();
    store
        .configure(
            LoopPolicyUpdate {
                actor_id: "reviewer",
                team_id: &run.team_id,
                expected_revision: 0,
                state: LoopPolicyState::Enabled,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits::default(),
            },
            now,
        )
        .await
        .unwrap();
    let trigger = store
        .accept_trigger(
            &LoopTriggerInput {
                actor_id: "reviewer".into(),
                team_id: run.team_id.clone(),
                kind: LoopTriggerKind::Operator,
                source_key: "fixture".into(),
                due_at: None,
                references: LoopSourceReferences::default(),
            },
            now,
        )
        .await
        .unwrap();
    let LoopAdmission::Admitted(reservation) = store
        .admit(&run.team_id, &trigger.activation_id, "fixture-daemon", now)
        .await
        .unwrap()
    else {
        panic!("not admitted");
    };
    sqlx::query(
        "INSERT INTO loop_mailbox_partitions(run_id, team_id, created_at) VALUES (?, ?, ?)",
    )
    .bind(&run.id)
    .bind(&run.team_id)
    .bind(now)
    .execute(&state.db)
    .await
    .unwrap();
    sqlx::query("UPDATE loop_activations SET mailbox_run_id = ? WHERE id = ?")
        .bind(&run.id)
        .bind(&trigger.activation_id)
        .execute(&state.db)
        .await
        .unwrap();
    let session = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES (?, 'reviewer', 'running', ?)")
        .bind(&session).bind(now).execute(&state.db).await.unwrap();
    let reservation = store
        .bind_session(&reservation, &session, now)
        .await
        .unwrap();
    store.mark_running(&reservation, now).await.unwrap();
    let authz = build_authz();
    let service = TeamInternalControlService::new(
        control_deps(&state),
        authz.clone(),
        InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap".into(),
    );
    (state, service, authz, run, reservation)
}

fn token(
    authz: &InternalAuthz,
    actor_id: &str,
    run_id: &str,
    reservation: &LoopReservation,
) -> String {
    authz
        .issue_loop_access_token(
            NodeCredentialRequest {
                source_node_id: "main".into(),
                role: "worker".into(),
                actor_id: Some(actor_id.into()),
                run_id: Some(run_id.into()),
                permissions: vec![InternalAction::LoopFinish.as_str().into()],
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

fn request() -> FinishLoopActivationRequest {
    FinishLoopActivationRequest {
        outcome_json: serde_json::to_string(&LoopOutcome {
            kind: LoopOutcomeKind::NoActionableWork,
            wait_reason: None,
            task_note_id: None,
            continuation: None,
        })
        .unwrap(),
    }
}

#[tokio::test]
async fn loop_finish_transport_requires_signed_identity_and_preserves_receipt_after_cleanup() {
    let (state, service, authz, run, reservation) = fixture().await;
    let valid = token(&authz, "reviewer", &run.id, &reservation);
    let response = service
        .finish_loop_activation(authenticated_request(request(), &valid))
        .await
        .unwrap()
        .into_inner();
    let store = LoopStore::new(state.db.clone());
    assert!(
        store
            .reservation(&run.team_id, "reviewer")
            .await
            .unwrap()
            .is_some(),
        "transport fixture has no supervisor cleanup proof"
    );
    store
        .cleanup_verified(
            &reservation,
            LoopCleanupDisposition::Exited,
            chrono::Utc::now().timestamp(),
        )
        .await
        .unwrap();
    let replay = service
        .finish_loop_activation(authenticated_request(request(), &valid))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(replay.receipt_json, response.receipt_json);
    state
        .agents
        .daemon_tasks()
        .shutdown_runtime(std::time::Duration::from_secs(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn loop_finish_transport_rejects_legacy_wrong_actor_run_generation_and_oversize() {
    let (state, service, authz, run, reservation) = fixture().await;
    let legacy = authz
        .issue_access_token(
            InternalRole::Worker,
            Some("reviewer"),
            Some(&run.id),
            vec!["*".into()],
            600,
        )
        .unwrap()
        .0;
    let wrong_actor = token(&authz, "planner", &run.id, &reservation);
    let wrong_run = token(&authz, "reviewer", "different-run", &reservation);
    let mut stale = reservation.clone();
    stale.generation += 1;
    let stale = token(&authz, "reviewer", &run.id, &stale);
    for invalid in [legacy, wrong_actor, wrong_run, stale] {
        assert_eq!(
            service
                .finish_loop_activation(authenticated_request(request(), &invalid))
                .await
                .unwrap_err()
                .code(),
            Code::PermissionDenied
        );
    }
    let valid = token(&authz, "reviewer", &run.id, &reservation);
    assert_eq!(
        service
            .finish_loop_activation(authenticated_request(
                FinishLoopActivationRequest {
                    outcome_json: "x".repeat(16_385)
                },
                &valid
            ))
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    assert!(
        LoopStore::new(state.db.clone())
            .reservation(&run.team_id, "reviewer")
            .await
            .unwrap()
            .is_some()
    );
}
