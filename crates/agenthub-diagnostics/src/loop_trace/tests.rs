use std::path::PathBuf;

use agenthub_agent_domain::loop_runtime::{
    LoopAdmission, LoopCleanupDisposition, LoopContinuation, LoopLimits, LoopOutcome,
    LoopOutcomeKind, LoopReservation, LoopSessionPolicy, LoopSourceReferences, LoopToolStatus,
    LoopToolSurface, LoopTriggerInput, LoopTriggerKind,
};
use agenthub_db::loop_runtime::LoopPolicyUpdate;
use uuid::Uuid;

use super::*;
use crate::agent_trace::{
    AgentTraceAvailability, AgentTraceLiveOverlay, AgentTraceRuntimeSummary, apply_live_overlay,
    collect_from_pool, render_human,
};

struct Fixture {
    directory: PathBuf,
    pool: SqlitePool,
    store: LoopStore,
    now: i64,
}

impl Fixture {
    async fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("loop-diagnostics-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let pool = agenthub_db::init_db_at_path(&directory.join("control.sqlite"))
            .await
            .unwrap();
        sqlx::query("INSERT INTO agents(id, name, workdir, command, args, worktree_mode, status, created_at, updated_at) \
            VALUES ('actor', 'Actor', '/tmp', 'fixture', '[]', 'use_existing', 'exited', 1, 1)").execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO team_definitions(id, name, spec_json, created_at, updated_at) \
            VALUES ('team', 'Team', ?, 1, 1)",
        )
        .bind(serde_json::json!({"members":[{"member_id":"actor"}]}).to_string())
        .execute(&pool)
        .await
        .unwrap();
        let now = i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        )
        .unwrap()
            - 10;
        let store = LoopStore::new(pool.clone());
        store
            .configure(
                LoopPolicyUpdate {
                    actor_id: "actor",
                    team_id: "team",
                    expected_revision: 0,
                    state: LoopPolicyState::Enabled,
                    session_policy: LoopSessionPolicy::Fresh,
                    limits: &LoopLimits::default(),
                },
                now,
            )
            .await
            .unwrap();
        Self {
            directory,
            pool,
            store,
            now,
        }
    }

    async fn pending(&self) -> String {
        let mut input = LoopTriggerInput {
            actor_id: "actor".into(),
            team_id: "team".into(),
            kind: LoopTriggerKind::Operator,
            source_key: "private-first".into(),
            due_at: None,
            references: LoopSourceReferences::default(),
        };
        let first = self.store.accept_trigger(&input, self.now).await.unwrap();
        input.source_key = "private-coalesced".into();
        self.store.accept_trigger(&input, self.now).await.unwrap();
        first.activation_id
    }

    async fn running(&self, id: &str) -> LoopReservation {
        let LoopAdmission::Admitted(reservation) = self
            .store
            .admit("team", id, "private-daemon-owner", self.now)
            .await
            .unwrap()
        else {
            panic!("not admitted")
        };
        sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES ('session', 'actor', 'running', ?)")
            .bind(self.now).execute(&self.pool).await.unwrap();
        let reservation = self
            .store
            .bind_session(&reservation, "session", self.now)
            .await
            .unwrap();
        self.store
            .mark_running(&reservation, self.now)
            .await
            .unwrap();
        reservation
    }

    async fn trace(&self, id: &str, now: i64) -> ActivationTrace {
        collect(
            &self.pool,
            self.store.activation("team", id).await.unwrap().unwrap(),
            1,
            now,
        )
        .await
        .unwrap()
    }

    async fn close(self) {
        self.pool.close().await;
        std::fs::remove_dir_all(self.directory).unwrap();
    }
}

#[tokio::test]
async fn activation_trace_renders_safe_event_attribution_after_exit_and_reopen() {
    let mut fixture = Fixture::new().await;
    let receipt = fixture
        .store
        .accept_trigger(
            &LoopTriggerInput {
                actor_id: "actor".into(),
                team_id: "team".into(),
                kind: LoopTriggerKind::AppEvent,
                source_key: "private-delivery-key".into(),
                due_at: None,
                references: LoopSourceReferences {
                    app_id: Some("app-a".into()),
                    app_event: Some(agenthub_agent_domain::app_events::AppEventAttribution {
                        event_id: "change-7".into(),
                        event_class: "changed".into(),
                        cursor: 7,
                        version: 2,
                    }),
                    ..Default::default()
                },
            },
            fixture.now,
        )
        .await
        .unwrap();
    fixture
        .store
        .cancel("team", &receipt.activation_id, fixture.now + 1)
        .await
        .unwrap();
    fixture.pool.close().await;
    fixture.pool = SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(fixture.directory.join("control.sqlite"))
            .read_only(true),
    )
    .await
    .unwrap();
    fixture.store = LoopStore::new(fixture.pool.clone());
    let trace = fixture.trace(&receipt.activation_id, fixture.now + 2).await;
    let rendered = render(&trace).join("\n");
    assert!(rendered.contains("app=app-a event=change-7 class=changed cursor=7 version=2"));
    assert!(!rendered.contains("private-delivery-key"));
    fixture.close().await;
}

#[tokio::test]
async fn activation_trace_never_falls_back_to_unrelated_session_evidence() {
    let fixture = Fixture::new().await;
    let id = fixture.pending().await;
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES ('unrelated', 'actor', 'running', ?)")
        .bind(fixture.now).execute(&fixture.pool).await.unwrap();
    sqlx::query("INSERT INTO acp_permission_requests(id, agent_id, session_id, tool_call_id, options_json, status, created_at) \
        VALUES ('unrelated-permission', 'actor', 'unrelated', 'unrelated-tool', '[]', 'pending', ?)")
        .bind(fixture.now).execute(&fixture.pool).await.unwrap();
    let events = SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(fixture.directory.join("actor.db"))
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE agent_events(id INTEGER PRIMARY KEY, session_id TEXT, seq TEXT, ts INTEGER, stream TEXT, message BLOB)")
        .execute(&events).await.unwrap();
    sqlx::query("INSERT INTO agent_events VALUES (1, 'unrelated', '1', 1, 'acp', ?)")
        .bind(br#"{"type":"tool_call","id":"unrelated-tool"}"#.as_slice())
        .execute(&events)
        .await
        .unwrap();
    let request = AgentTraceRequest {
        activation_id: Some(id.clone()),
        ..Default::default()
    };
    let pending = collect_from_pool(&fixture.pool, fixture.directory.clone(), request.clone())
        .await
        .unwrap();
    assert!(pending.session.is_none());
    assert!(pending.events.event_db_exists);
    assert!(pending.events.recent.is_empty());
    assert_eq!(pending.permissions.pending_count, 0);

    fixture.running(&id).await;
    sqlx::query("INSERT INTO agent_events VALUES (2, 'session', '2', 2, 'acp', ?)")
        .bind(br#"{"type":"tool_call","id":"selected-tool"}"#.as_slice())
        .execute(&events)
        .await
        .unwrap();
    let historical = collect_from_pool(&fixture.pool, fixture.directory.clone(), request)
        .await
        .unwrap();
    assert_eq!(historical.session.as_ref().unwrap().id, "session");
    assert_eq!(historical.events.count, 1);
    assert_eq!(
        historical.events.latest.as_ref().unwrap().session_id,
        "session"
    );
    assert_eq!(historical.permissions.pending_count, 0);
    assert!(
        !serde_json::to_string(&historical)
            .unwrap()
            .contains("unrelated-tool")
    );
    events.close().await;
    fixture.close().await;
}

#[tokio::test]
async fn activation_trace_reopens_completed_history_with_coalesced_sources_and_next_wake() {
    let mut fixture = Fixture::new().await;
    let id = fixture.pending().await;
    let executor = fixture.running(&id).await;
    let observation = fixture
        .store
        .begin_tool_observation(
            &executor,
            LoopToolSurface::ControlRpc,
            "list_tasks",
            None,
            fixture.now,
        )
        .await
        .unwrap();
    fixture
        .store
        .complete_tool_observation(observation, LoopToolStatus::Succeeded, fixture.now + 1)
        .await
        .unwrap();
    fixture
        .store
        .finish(
            &executor,
            &LoopOutcome {
                kind: LoopOutcomeKind::NoActionableWork,
                wait_reason: None,
                task_note_id: None,
                continuation: Some(LoopContinuation {
                    due_at: fixture.now + 300,
                    task_id: None,
                }),
            },
            fixture.now + 2,
        )
        .await
        .unwrap();
    fixture
        .store
        .cleanup_verified(&executor, LoopCleanupDisposition::Exited, fixture.now + 3)
        .await
        .unwrap();
    sqlx::query("UPDATE agent_sessions SET status = 'exited', ended_at = ? WHERE id = 'session'")
        .bind(fixture.now + 3)
        .execute(&fixture.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES ('unrelated-session', 'actor', 'running', ?)")
        .bind(fixture.now + 4).execute(&fixture.pool).await.unwrap();
    sqlx::query("UPDATE team_definitions SET spec_json = '{\"members\":[]}' WHERE id = 'team'")
        .execute(&fixture.pool)
        .await
        .unwrap();
    fixture.pool.close().await;
    fixture.pool = SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(fixture.directory.join("control.sqlite"))
            .read_only(true),
    )
    .await
    .unwrap();
    fixture.store = LoopStore::new(fixture.pool.clone());
    let request = AgentTraceRequest {
        activation_id: Some(id.clone()),
        event_limit: 1,
        ..Default::default()
    };
    let report = collect_from_pool(&fixture.pool, fixture.directory.join("events"), request)
        .await
        .unwrap();
    assert_eq!(report.verdict.layer, AgentTraceStallLayer::LoopCompleted);
    assert_eq!(report.target.activation_id.as_deref(), Some(id.as_str()));
    assert_eq!(report.session.as_ref().unwrap().id, "session");
    assert!(!report.team.as_ref().unwrap().member_found);
    let trace = report.activation.as_ref().unwrap();
    assert_eq!(trace.sources.sources.len(), 1);
    assert!(trace.sources.next_cursor.is_some() && trace.events.next_cursor.is_some());
    assert_eq!(
        trace.latest_event.as_ref().unwrap().kind.as_str(),
        "cleanup_verified"
    );
    assert_eq!(
        trace.continuation.as_ref().unwrap().activation_id,
        trace.actor_next_wake.as_ref().unwrap().activation_id
    );
    let json = serde_json::to_string(&report).unwrap();
    let human = render_human(&report);
    for output in [&json, &human] {
        for private in [
            "private-first",
            "private-coalesced",
            "private-daemon-owner",
            "source_key",
            "input_json",
            "owner_id",
        ] {
            assert!(!output.contains(private), "{private}");
        }
    }
    assert!(human.contains("loop.continuation:") && human.contains("loop.source:"));
    let actor = resolve(
        &fixture.pool,
        &AgentTraceRequest {
            agent_id: Some("actor".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        actor.id,
        trace.actor_next_wake.as_ref().unwrap().activation_id
    );
    fixture.close().await;
}

#[tokio::test]
async fn activation_trace_classifies_pending_suspended_open_tools_and_unfenced_interruption() {
    let fixture = Fixture::new().await;
    let id = fixture.pending().await;
    assert_eq!(
        verdict(&fixture.trace(&id, fixture.now).await).layer,
        AgentTraceStallLayer::PendingNotAdmitted
    );
    fixture
        .store
        .configure(
            LoopPolicyUpdate {
                actor_id: "actor",
                team_id: "team",
                expected_revision: 1,
                state: LoopPolicyState::Suspended,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits::default(),
            },
            fixture.now,
        )
        .await
        .unwrap();
    assert_eq!(
        verdict(&fixture.trace(&id, fixture.now).await).layer,
        AgentTraceStallLayer::LoopSuspended
    );
    fixture
        .store
        .configure(
            LoopPolicyUpdate {
                actor_id: "actor",
                team_id: "team",
                expected_revision: 2,
                state: LoopPolicyState::Enabled,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits::default(),
            },
            fixture.now,
        )
        .await
        .unwrap();
    let executor = fixture.running(&id).await;
    let completed = fixture
        .store
        .begin_tool_observation(
            &executor,
            LoopToolSurface::ControlRpc,
            "first_page",
            None,
            fixture.now,
        )
        .await
        .unwrap();
    fixture
        .store
        .complete_tool_observation(completed, LoopToolStatus::Succeeded, fixture.now)
        .await
        .unwrap();
    let _open = fixture
        .store
        .begin_tool_observation(
            &executor,
            LoopToolSurface::ControlRpc,
            "later_page",
            None,
            fixture.now,
        )
        .await
        .unwrap();
    fixture
        .store
        .renew(&executor, fixture.now + 40)
        .await
        .unwrap();
    let trace = fixture.trace(&id, fixture.now + 65).await;
    assert_eq!(trace.tools.tools[0].status, LoopToolStatus::Succeeded);
    assert!(trace.oldest_unsettled_tool_id.is_some());
    assert_eq!(
        verdict(&trace).layer,
        AgentTraceStallLayer::ToolBoundaryStall
    );
    fixture
        .store
        .interrupt_expired(fixture.now + 101)
        .await
        .unwrap();
    assert_eq!(
        verdict(&fixture.trace(&id, fixture.now + 101).await).layer,
        AgentTraceStallLayer::LeaseExpiredUnfenced
    );
    fixture
        .store
        .cleanup_verified(&executor, LoopCleanupDisposition::Exited, fixture.now + 102)
        .await
        .unwrap();
    assert_eq!(
        verdict(&fixture.trace(&id, fixture.now + 102).await).layer,
        AgentTraceStallLayer::LoopInterrupted
    );
    fixture.close().await;
}

#[tokio::test]
async fn activation_trace_distinguishes_dependency_wait_from_missing_requested_continuation() {
    let fixture = Fixture::new().await;
    let id = fixture.pending().await;
    let executor = fixture.running(&id).await;
    fixture
        .store
        .finish(
            &executor,
            &LoopOutcome {
                kind: LoopOutcomeKind::Waiting,
                wait_reason: Some(LoopWaitReason::Dependency),
                task_note_id: None,
                continuation: Some(LoopContinuation {
                    due_at: fixture.now + 300,
                    task_id: None,
                }),
            },
            fixture.now + 1,
        )
        .await
        .unwrap();
    fixture
        .store
        .cleanup_verified(&executor, LoopCleanupDisposition::Exited, fixture.now + 2)
        .await
        .unwrap();
    assert_eq!(
        verdict(&fixture.trace(&id, fixture.now + 3).await).layer,
        AgentTraceStallLayer::WaitingDependency
    );
    sqlx::query("DELETE FROM loop_finish_receipts WHERE activation_id = ?")
        .bind(&id)
        .execute(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(
        verdict(&fixture.trace(&id, fixture.now + 3).await).layer,
        AgentTraceStallLayer::ContinuationMissing
    );
    fixture.close().await;
}

#[tokio::test]
async fn activation_trace_joins_live_overlay_only_for_the_selected_running_session() {
    let fixture = Fixture::new().await;
    let id = fixture.pending().await;
    fixture.running(&id).await;
    let mut report = collect_from_pool(
        &fixture.pool,
        fixture.directory.join("events"),
        AgentTraceRequest {
            activation_id: Some(id),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let overlay = |session: &str| AgentTraceLiveOverlay {
        runtime: AgentTraceRuntimeSummary {
            ownership: "local".into(),
            active_session_id: Some(session.into()),
            live_state_source: "live".into(),
        },
        provider_adapter: AgentTraceAvailability {
            status: "prompt_stale".into(),
            note: "fixture".into(),
            details: serde_json::Value::Null,
        },
        sse: AgentTraceAvailability {
            status: "available".into(),
            note: "fixture".into(),
            details: serde_json::Value::Null,
        },
    };
    apply_live_overlay(&mut report, overlay("unrelated-session"));
    assert_eq!(report.verdict.layer, AgentTraceStallLayer::LoopRunning);
    apply_live_overlay(&mut report, overlay("session"));
    assert_eq!(
        report.verdict.layer,
        AgentTraceStallLayer::ProviderPromptStale
    );
    assert!(
        AgentTraceRequest {
            activation_id: Some("activation".into()),
            agent_id: Some("actor".into()),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        AgentTraceRequest {
            activation_id: Some("activation".into()),
            session_id: Some("session".into()),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    fixture.close().await;
}
