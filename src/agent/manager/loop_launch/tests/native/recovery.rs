use agent_client_protocol::schema::v1::{RequestPermissionOutcome, SelectedPermissionOutcome};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use super::*;

async fn marker(fixture: &Fixture, file: &str) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !fixture.directory.join(file).exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

async fn pending_permission(fixture: &Fixture) -> String {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let id: Option<String> = sqlx::query_scalar("SELECT id FROM acp_permission_requests WHERE agent_id = 'worker' AND status = 'pending' ORDER BY created_at LIMIT 1")
                .fetch_optional(&fixture.state.db).await.unwrap();
            if let Some(id) = id { return id; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap()
}

#[tokio::test]
async fn native_reentry_declines_use_canonical_finish_without_opening_restored_approval() {
    for mode in [
        "reentry-wait-mismatch",
        "reentry-wait-clarification",
        "reentry-recovery-mismatch",
        "reentry-foreign",
        "reentry-query-prefix",
        "reentry-result-prefix",
        "reentry-phase-changed",
        "reentry-duplicate",
    ] {
        let fixture = fixture(mode).await;
        let rejected = !mode.contains("mismatch") && !mode.contains("clarification");
        let activation = if rejected {
            let reservation = fixture.admit(mode).await;
            assert!(
                fixture
                    .state
                    .agents
                    .execute_loop_activation(fixture.state.teams.clone(), reservation.clone())
                    .await
                    .is_err()
            );
            fixture
                .state
                .agents
                .fence_loop_reservation(&reservation)
                .await
                .unwrap();
            LoopStore::new(fixture.state.db.clone())
                .activation(
                    &fixture.team_id,
                    reservation.activation_id.as_deref().unwrap(),
                )
                .await
                .unwrap()
                .unwrap()
        } else {
            fixture.execute(mode).await
        };
        assert_eq!(
            activation.state,
            if rejected {
                LoopActivationState::Interrupted
            } else {
                LoopActivationState::Finished
            }
        );
        assert_eq!(activation.outcome.is_some(), !rejected);
        let permissions: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM acp_permission_requests WHERE agent_id = 'worker'",
        )
        .fetch_one(&fixture.state.db)
        .await
        .unwrap();
        assert_eq!(permissions, 0);
        let log = std::fs::read_to_string(fixture.directory.join("native-requests.jsonl")).unwrap();
        assert_eq!(
            log.matches("evaluate_reentry").count(),
            usize::from(mode != "reentry-query-prefix")
        );
        assert!(
            !log.contains("submit_guarded_prompt")
                && !log.contains("submit_user_prompt")
                && !log.contains("answer_shell_approval")
        );
        fixture.close().await;
    }
}

#[tokio::test]
async fn native_reentry_opens_approval_only_after_matching_durable_ack() {
    let fixture = fixture("reentry-wait-hold").await;
    let (activation, ()) = tokio::join!(fixture.execute("restored-approval"), async {
        marker(&fixture, "native-entry-evaluated").await;
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM acp_permission_requests WHERE agent_id = 'worker'",
        )
        .fetch_one(&fixture.state.db)
        .await
        .unwrap();
        assert_eq!(
            count, 0,
            "an event without a durable ACK cannot open an approval"
        );
        std::fs::write(fixture.directory.join("native-entry-release"), "release").unwrap();
        let id = pending_permission(&fixture).await;
        fixture
            .state
            .agents
            .permissions
            .respond(
                &id,
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                    "once".to_owned(),
                )),
                Some("once".into()),
                Some("operator".into()),
            )
            .await
            .unwrap();
    });
    assert_eq!(activation.state, LoopActivationState::Interrupted);
    let log = std::fs::read_to_string(fixture.directory.join("native-requests.jsonl")).unwrap();
    assert_eq!(log.matches("answer_shell_approval").count(), 1);
    assert!(!log.contains("submit_guarded_prompt") && !log.contains("submit_user_prompt"));
    fixture.close().await;
}

#[tokio::test]
async fn native_reentry_rejects_approval_after_membership_revocation() {
    let fixture = fixture("reentry-wait-hold").await;
    let (activation, ()) = tokio::join!(fixture.execute("revoked-approval"), async {
        marker(&fixture, "native-entry-evaluated").await;
        std::fs::write(fixture.directory.join("native-entry-release"), "release").unwrap();
        let id = pending_permission(&fixture).await;
        sqlx::query("UPDATE team_definitions SET spec_json = json_set(spec_json, '$.members', json('[]')) WHERE id = ?")
            .bind(&fixture.team_id).execute(&fixture.state.db).await.unwrap();
        fixture
            .state
            .agents
            .permissions
            .respond(
                &id,
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                    "once".to_owned(),
                )),
                Some("once".into()),
                Some("operator".into()),
            )
            .await
            .unwrap();
    });
    assert_eq!(activation.state, LoopActivationState::Interrupted);
    let log = std::fs::read_to_string(fixture.directory.join("native-requests.jsonl")).unwrap();
    assert!(!log.contains("answer_shell_approval"));
    fixture.close().await;
}

async fn reconcile(app: &axum::Router, token: Option<&str>, payload: &Value) -> StatusCode {
    let mut request =
        Request::post("/agents/worker/runtime/recovery").header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    app.clone()
        .oneshot(request.body(Body::from(payload.to_string())).unwrap())
        .await
        .unwrap()
        .status()
}

async fn query(
    app: &axum::Router,
    token: Option<&str>,
    local_session_id: &str,
) -> (StatusCode, Value) {
    let mut request = Request::get(format!(
        "/agents/worker/runtime/recovery?local_session_id={local_session_id}"
    ));
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn native_recovery_api_requires_current_owner_and_explicit_note_without_replay() {
    let fixture = fixture("reentry-recovery-hold").await;
    let token = crate::api::team_tests::create_auth_token(&fixture.state).await;
    let app = crate::api::router(fixture.state.clone());
    let (activation, ()) = tokio::join!(fixture.execute("reconcile-api"), async {
        marker(&fixture, "native-entry-evaluated").await;
        let (local, native): (String, String) = sqlx::query_as("SELECT local_session_id, native_session_id FROM loop_native_sessions WHERE actor_id = 'worker'")
            .fetch_one(&fixture.state.db).await.unwrap();
        let log = std::fs::read_to_string(fixture.directory.join("native-requests.jsonl")).unwrap();
        let request: Value = serde_json::from_str(log.lines().next().unwrap()).unwrap();
        let payload = json!({"local_session_id":local, "target":{"runtime_id":request["payload"]["runtime_id"],"session_id":native,"recovery_id":"recovery-token"},
            "note":"Old executor retired; effects inspected"});
        assert_eq!(
            reconcile(&app, None, &payload).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            reconcile(&app, Some(&token), &payload).await,
            StatusCode::CONFLICT
        );
        assert_eq!(query(&app, None, &local).await.0, StatusCode::UNAUTHORIZED);
        assert_eq!(
            query(&app, Some(&token), &local).await.0,
            StatusCode::CONFLICT
        );
        std::fs::write(fixture.directory.join("native-entry-release"), "release").unwrap();
        let runtime = {
            let handles = fixture.state.agents.inner.read().await;
            let AgentInput::Rara(runtime) = &handles["worker"].input else {
                panic!("native");
            };
            runtime.clone()
        };
        // Query delivery has already committed; wait for the held entry receipt to commit too.
        let events = fixture
            .state
            .agents
            .event_dbs
            .pool_for_agent("worker")
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_control_receipts WHERE kind = 'evaluate_reentry' AND status = 'accepted'")
                    .fetch_one(&events).await.unwrap();
                if count == 1 { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.unwrap();
        assert_eq!(
            query(&app, Some(&token), "previous-session").await.0,
            StatusCode::CONFLICT
        );
        let (status, current) = query(&app, Some(&token), &local).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(current["local_session_id"], local);
        assert_eq!(current["runtime_id"], payload["target"]["runtime_id"]);
        assert_eq!(current["session_id"], native);
        assert_eq!(
            current["recovery"]["blocked"]["recovery_id"],
            "recovery-token"
        );
        let mut invalid = payload.clone();
        invalid["note"] = json!("");
        assert_eq!(
            reconcile(&app, Some(&token), &invalid).await,
            StatusCode::BAD_REQUEST
        );
        for field in ["runtime_id", "session_id", "recovery_id"] {
            invalid = payload.clone();
            invalid["target"][field] = json!("foreign");
            assert_eq!(
                reconcile(&app, Some(&token), &invalid).await,
                StatusCode::CONFLICT
            );
        }
        assert_eq!(
            reconcile(&app, Some(&token), &payload).await,
            StatusCode::OK
        );
        assert_eq!(
            reconcile(&app, Some(&token), &payload).await,
            StatusCode::OK
        );
        let log = std::fs::read_to_string(fixture.directory.join("native-requests.jsonl")).unwrap();
        assert_eq!(log.matches("resolve_recovery").count(), 1);
        assert!(!log.contains("submit_user_prompt") && !log.contains("answer_shell_approval"));
        let (status, current) = query(&app, Some(&token), &local).await;
        assert_eq!(status, StatusCode::OK);
        assert!(current["recovery"]["blocked"].is_null());
        assert_eq!(
            current["recovery"]["last_resolution"]["note"],
            payload["note"]
        );
        runtime
            .send_input(
                "Run current work explicitly",
                Some("after-recovery"),
                None,
                None,
            )
            .await
            .unwrap();
    });
    assert_eq!(activation.state, LoopActivationState::Interrupted);
    fixture.close().await;
}
