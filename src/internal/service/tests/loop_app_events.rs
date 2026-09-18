use super::{loop_activation::fixture, loop_work::work_token, *};
use crate::internal::proto::agenthub::internal::v1::{
    GetLoopWorkRequest, GetLoopWorkSourceRequest, RegisterLoopScheduleRequest,
};
use agenthub_agent_domain::{
    app_events::{AppEventDeclaration, AppEventNotification},
    app_tools::{AppConnection, AppManifest, AppReplayPolicy, AppTool},
    loop_runtime::{
        LoopAdmission, LoopCleanupDisposition, LoopOutcome, LoopOutcomeKind, LoopWorkPage,
    },
    loop_scheduling::LoopRegistrationReceipt,
};
use agenthub_db::{
    app_registry::{
        AppBindingUpdate, AppEventRouteUpdate, AppGrantUpdate, AppRegistry, RegisterApp,
    },
    loop_runtime::LoopStore,
};

#[tokio::test]
async fn app_event_schedule_rpc_preserves_signed_provenance_and_next_activation_context() {
    let (state, service, authz, run, previous) = fixture().await;
    let now = chrono::Utc::now().timestamp();
    sqlx::query("UPDATE team_definitions SET spec_json = json_set(spec_json, '$.execution_mode', 'loop') WHERE id = ?")
        .bind(&run.team_id).execute(&state.db).await.unwrap();
    sqlx::query("INSERT INTO users(id, username, display_name, role, created_at) VALUES ('event-owner', 'event-owner', 'Event owner', 'admin', 1)")
        .execute(&state.db).await.unwrap();
    let apps = AppRegistry::new(state.db.clone());
    let manifest = AppManifest {
        schema_version: 1,
        scopes: ["read".into()].into(),
        tools: vec![AppTool {
            name: "lookup".into(),
            input_schema: json!({"type":"object"}),
            output_schema: None,
            required_scopes: ["read".into()].into(),
            replay: AppReplayPolicy::ReadOnly,
        }],
        events: vec![AppEventDeclaration {
            name: "changed".into(),
            required_scopes: ["read".into()].into(),
        }],
    };
    let app = apps
        .register(
            RegisterApp {
                owner_user_id: "event-owner",
                name: "Signed notification fixture",
                connection: &AppConnection {
                    endpoint: "https://events.example.test/mcp".into(),
                    credential_env: None,
                    authority: "events.example.test".into(),
                    namespace: "fixture".into(),
                },
                manifest: &manifest,
            },
            now,
        )
        .await
        .unwrap();
    apps.configure_event_key(&app.id, 0, Some("RPC_EVENT_SIGNING_KEY"), now)
        .await
        .unwrap();
    apps.approve_team(
        "event-owner",
        AppGrantUpdate {
            app_id: &app.id,
            team_id: &run.team_id,
            expected_revision: 0,
            scopes: &manifest.scopes,
        },
        now,
    )
    .await
    .unwrap();
    apps.bind_member(
        AppBindingUpdate {
            app_id: &app.id,
            team_id: &run.team_id,
            actor_id: "reviewer",
            version: 1,
            expected_revision: 0,
            scopes: &manifest.scopes,
        },
        now,
    )
    .await
    .unwrap();
    let token = work_token(
        &authz,
        &run.id,
        &previous,
        vec![InternalAction::TeamRead, InternalAction::LoopActivate],
    );
    let request = || {
        RegisterLoopScheduleRequest {
        member_id: String::new(),
        request_json: json!({"source_key":"changes:1","schedule":{"kind":"app_event","app_id":app.id,"event_class":"changed","after_cursor":0,"repeat":true}}).to_string(),
    }
    };
    // A signed executor and an existing tool binding still cannot self-grant event authority.
    assert_eq!(
        service
            .register_loop_schedule(authenticated_request(request(), &token))
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    apps.configure_event_route(
        AppEventRouteUpdate {
            app_id: &app.id,
            team_id: &run.team_id,
            actor_id: "reviewer",
            expected_revision: 0,
            classes: &["changed".into()].into(),
        },
        now,
    )
    .await
    .unwrap();
    let created = service
        .register_loop_schedule(authenticated_request(request(), &token))
        .await
        .unwrap()
        .into_inner();
    let created: LoopRegistrationReceipt = serde_json::from_str(&created.receipt_json).unwrap();
    assert_eq!(
        created
            .registration
            .input
            .references
            .scheduling_activation_id,
        previous.activation_id
    );
    assert_eq!(
        created
            .registration
            .input
            .references
            .scheduling_actor_id
            .as_deref(),
        Some("reviewer")
    );
    assert!(created.registration.input.references.app_event.is_none());
    // The envelope is accepted through the already signature-verified store boundary; HTTP HMAC is covered separately.
    let event = AppEventNotification {
        schema_version: 1,
        event_id: "opaque-change-7".into(),
        cursor: 7,
        team_id: run.team_id.clone(),
        actor_id: "reviewer".into(),
        event_class: "changed".into(),
    };
    let direct = apps
        .accept_signed_event(&app.id, 1, &event, now)
        .await
        .unwrap();
    let loops = LoopStore::new(state.db.clone());
    let firing = loops.reconcile_schedules(now).await.unwrap().remove(0);
    assert_eq!(firing.receipt.activation_id, direct.activation_id);
    loops
        .finish(
            &previous,
            &LoopOutcome {
                kind: LoopOutcomeKind::NoActionableWork,
                wait_reason: None,
                task_note_id: None,
                continuation: None,
            },
            now,
        )
        .await
        .unwrap();
    loops
        .cleanup_verified(&previous, LoopCleanupDisposition::Exited, now)
        .await
        .unwrap();
    let LoopAdmission::Admitted(next) = loops
        .admit(
            &run.team_id,
            &direct.activation_id,
            state.agents.loop_owner_id(),
            now,
        )
        .await
        .unwrap()
    else {
        panic!("event work was not admitted");
    };
    // Reuse the fixture mailbox partition while binding a fresh provider session and executor generation.
    sqlx::query("UPDATE loop_activations SET mailbox_run_id = ? WHERE id = ?")
        .bind(&run.id)
        .bind(&direct.activation_id)
        .execute(&state.db)
        .await
        .unwrap();
    let session = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES (?, 'reviewer', 'running', ?)")
        .bind(&session).bind(now).execute(&state.db).await.unwrap();
    let next = loops.bind_session(&next, &session, now).await.unwrap();
    loops.mark_running(&next, now).await.unwrap();
    let credential = work_token(
        &authz,
        &run.id,
        &next,
        vec![InternalAction::TeamRead, InternalAction::LoopActivate],
    );
    let read = || GetLoopWorkRequest {
        after_source_id: String::new(),
        limit: 10,
    };
    assert_eq!(
        service
            .get_loop_work(authenticated_request(read(), &token))
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    let page = service
        .get_loop_work(authenticated_request(read(), &credential))
        .await
        .unwrap()
        .into_inner();
    assert!(!page.page_json.contains("RPC_EVENT_SIGNING_KEY"));
    let page: LoopWorkPage = serde_json::from_str(&page.page_json).unwrap();
    assert_eq!(page.sources.len(), 2);
    for source in &page.sources {
        assert_eq!(
            source.input.references.app_id.as_deref(),
            Some(app.id.as_str())
        );
        let attribution = source.input.references.app_event.as_ref().unwrap();
        assert_eq!(
            (
                &*attribution.event_id,
                &*attribution.event_class,
                attribution.cursor,
                attribution.version
            ),
            ("opaque-change-7", "changed", 7, 1)
        );
        let detail = service
            .get_loop_work_source(authenticated_request(
                GetLoopWorkSourceRequest {
                    source_id: source.id.clone(),
                },
                &credential,
            ))
            .await
            .unwrap()
            .into_inner();
        assert!(detail.source_json.contains("opaque-change-7"));
    }
    // Business retries by the next activation retain the original registration and provenance.
    let retry = service
        .register_loop_schedule(authenticated_request(request(), &credential))
        .await
        .unwrap()
        .into_inner();
    let retry: LoopRegistrationReceipt = serde_json::from_str(&retry.receipt_json).unwrap();
    assert!(retry.duplicate);
    assert_eq!(retry.registration.id, created.registration.id);
    apps.revoke_event_route(&app.id, &run.team_id, "reviewer", 1, now)
        .await
        .unwrap();
    let page = service
        .get_loop_work(authenticated_request(read(), &credential))
        .await
        .unwrap()
        .into_inner();
    let page: LoopWorkPage = serde_json::from_str(&page.page_json).unwrap();
    assert!(
        page.sources
            .iter()
            .find(|source| source.id == firing.receipt.trigger_id)
            .unwrap()
            .revoked
    );
    assert!(
        !page
            .sources
            .iter()
            .find(|source| source.id == direct.trigger_id)
            .unwrap()
            .revoked
    );
    loops
        .cancel(&run.team_id, &direct.activation_id, now)
        .await
        .unwrap();
    loops
        .cleanup_verified(&next, LoopCleanupDisposition::Exited, now)
        .await
        .unwrap();
    state
        .agents
        .daemon_tasks()
        .shutdown_runtime(std::time::Duration::from_secs(2))
        .await
        .unwrap();
}
