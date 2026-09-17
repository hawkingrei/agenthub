use crate::loop_runtime::{LoopPolicyUpdate, LoopStore};
use agenthub_agent_domain::{
    app_events::AppEventNotification,
    loop_runtime::{LoopLimits, LoopPolicyState, LoopSessionPolicy},
};

use super::*;

impl Fixture {
    pub(super) async fn event_intake(&self) -> RegisteredApp {
        let app = self.event_app().await;
        self.store
            .configure_event_key(&app.id, 0, Some("SIGNED_EVENT_KEY"), 13)
            .await
            .unwrap();
        self.store
            .configure_event_route(
                AppEventRouteUpdate {
                    app_id: &app.id,
                    team_id: "team",
                    actor_id: "worker",
                    expected_revision: 0,
                    classes: &["changed".into()].into(),
                },
                14,
            )
            .await
            .unwrap();
        self.event_policy(LoopPolicyState::Suspended, 0, &LoopLimits::default())
            .await;
        app
    }

    pub(super) async fn event_policy(
        &self,
        state: LoopPolicyState,
        revision: i64,
        limits: &LoopLimits,
    ) {
        LoopStore::new(self.store.pool.clone())
            .configure(
                LoopPolicyUpdate {
                    team_id: "team",
                    actor_id: "worker",
                    expected_revision: revision,
                    state,
                    session_policy: LoopSessionPolicy::Fresh,
                    limits,
                },
                15,
            )
            .await
            .unwrap();
    }
}

pub(super) fn notification(cursor: i64) -> AppEventNotification {
    AppEventNotification {
        schema_version: 1,
        event_id: format!("event-{cursor}"),
        cursor,
        team_id: "team".into(),
        actor_id: "worker".into(),
        event_class: "changed".into(),
    }
}

fn rejected(result: anyhow::Result<AppEventReceipt>, expected: AppEventIntakeError) {
    assert_eq!(
        result.unwrap_err().downcast_ref::<AppEventIntakeError>(),
        Some(&expected)
    );
}

#[tokio::test]
async fn signed_event_receipts_and_cursors_survive_reopen_without_reactivation() {
    let mut fixture = Fixture::new().await;
    let app = fixture.event_intake().await;
    let event = notification(5);
    let original = fixture
        .store
        .accept_signed_event(&app.id, 1, &event, 100)
        .await
        .unwrap();
    fixture.store.pool.close().await;
    fixture.store = AppRegistry::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    let duplicate = fixture
        .store
        .accept_signed_event(&app.id, 1, &event, 101)
        .await
        .unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.trigger_id, original.trigger_id);
    assert_eq!(duplicate.activation_id, original.activation_id);
    rejected(
        fixture
            .store
            .accept_signed_event(&app.id, 1, &notification(4), 102)
            .await,
        AppEventIntakeError::CursorReplay,
    );
    let stored: (String, String, String, i64) = sqlx::query_as(
        "SELECT team_id, actor_id, event_class, cursor FROM app_event_receipts WHERE app_id = ? AND event_id = ?")
        .bind(&app.id).bind(&event.event_id).fetch_one(&fixture.store.pool).await.unwrap();
    assert_eq!(
        stored,
        ("team".into(), "worker".into(), "changed".into(), 5)
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM loop_activations")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    fixture.close().await;
}

#[tokio::test]
async fn signed_event_duplicates_serialize_and_preserve_one_firing_with_history() {
    let fixture = Fixture::new().await;
    let app = fixture.event_intake().await;
    let event = notification(1);
    let (a, b) = tokio::join!(
        fixture.store.accept_signed_event(&app.id, 1, &event, 100),
        fixture.store.accept_signed_event(&app.id, 1, &event, 100),
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_ne!(a.duplicate, b.duplicate);
    assert_eq!(a.trigger_id, b.trigger_id);
    assert_eq!(a.activation_id, b.activation_id);
    fixture
        .store
        .accept_signed_event(&app.id, 1, &notification(2), 101)
        .await
        .unwrap();
    let retry = fixture
        .store
        .accept_signed_event(&app.id, 1, &event, 102)
        .await
        .unwrap();
    assert!(retry.duplicate);
    assert_eq!(retry.trigger_id, a.trigger_id);
    let loops = LoopStore::new(fixture.store.pool.clone());
    let history = loops
        .activation_source_history("team", "worker", &a.activation_id, None, 10)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(history.sources.len(), 2);
    let refs = &history.sources[0].references;
    assert_eq!(refs.app_id.as_deref(), Some(app.id.as_str()));
    let attribution = refs.app_event.as_ref().unwrap();
    assert_eq!(attribution.event_id, "event-1");
    assert_eq!(attribution.version, 1);
    assert_eq!(attribution.cursor, 1);
    let duplicate_count: i64 =
        sqlx::query_scalar("SELECT duplicate_count FROM loop_trigger_sources WHERE id = ?")
            .bind(&a.trigger_id)
            .fetch_one(&fixture.store.pool)
            .await
            .unwrap();
    assert_eq!(duplicate_count, 2);
    let activation = loops
        .activation("team", &a.activation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(activation.state.as_str(), "pending");
    assert_eq!(
        loops.policy("team", "worker").await.unwrap().unwrap().state,
        LoopPolicyState::Suspended
    );
    fixture
        .store
        .revoke_event_route(&app.id, "team", "worker", 1, 103)
        .await
        .unwrap();
    assert_eq!(
        loops
            .activation_source_history("team", "worker", &a.activation_id, None, 10)
            .await
            .unwrap()
            .unwrap(),
        history
    );
    rejected(
        fixture
            .store
            .accept_signed_event(&app.id, 1, &event, 104)
            .await,
        AppEventIntakeError::Unauthorized,
    );
    fixture.close().await;
}

#[tokio::test]
async fn signed_event_rejects_mutated_ids_old_cursors_and_unapproved_routes() {
    let fixture = Fixture::new().await;
    let app = fixture.event_intake().await;
    let event = notification(10);
    fixture
        .store
        .accept_signed_event(&app.id, 1, &event, 100)
        .await
        .unwrap();
    let mut mutated = event.clone();
    mutated.cursor = 11;
    rejected(
        fixture
            .store
            .accept_signed_event(&app.id, 1, &mutated, 100)
            .await,
        AppEventIntakeError::IdConflict,
    );
    for cursor in [1, 9, 10] {
        let mut old = notification(cursor);
        old.event_id = format!("unseen-{cursor}");
        rejected(
            fixture
                .store
                .accept_signed_event(&app.id, 1, &old, 100)
                .await,
            AppEventIntakeError::CursorReplay,
        );
    }
    for field in ["class", "actor", "team"] {
        let mut denied = notification(11);
        match field {
            "class" => denied.event_class = "written".into(),
            "actor" => denied.actor_id = "outsider".into(),
            _ => denied.team_id = "elsewhere".into(),
        }
        rejected(
            fixture
                .store
                .accept_signed_event(&app.id, 1, &denied, 100)
                .await,
            AppEventIntakeError::Unauthorized,
        );
    }
    let audit = fixture.store.event_denials(&app.id).await.unwrap();
    assert_eq!(
        audit
            .iter()
            .map(|row| (row.code.as_str(), row.count))
            .collect::<Vec<_>>(),
        vec![
            ("cursor_replay", 3),
            ("id_conflict", 1),
            ("unauthorized", 3)
        ]
    );
    let cursor: i64 = sqlx::query_scalar("SELECT cursor FROM app_event_cursors WHERE app_id = ?")
        .bind(&app.id)
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(cursor, 10);
    fixture
        .store
        .accept_signed_event(&app.id, 1, &notification(11), 101)
        .await
        .unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn signed_event_rechecks_key_rotation_and_app_revocation_before_duplicate_lookup() {
    let fixture = Fixture::new().await;
    let app = fixture.event_intake().await;
    let event = notification(1);
    let original = fixture
        .store
        .accept_signed_event(&app.id, 1, &event, 100)
        .await
        .unwrap();
    fixture
        .store
        .configure_event_key(&app.id, 1, Some("ROTATED_EVENT_KEY"), 101)
        .await
        .unwrap();
    rejected(
        fixture
            .store
            .accept_signed_event(&app.id, 1, &event, 102)
            .await,
        AppEventIntakeError::SigningAuthority,
    );
    let duplicate = fixture
        .store
        .accept_signed_event(&app.id, 2, &event, 102)
        .await
        .unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.trigger_id, original.trigger_id);
    fixture
        .store
        .revoke_app(&app.id, "owner", 1, 103)
        .await
        .unwrap();
    rejected(
        fixture
            .store
            .accept_signed_event(&app.id, 2, &event, 104)
            .await,
        AppEventIntakeError::SigningAuthority,
    );
    assert!(
        fixture
            .store
            .event_denials(&app.id)
            .await
            .unwrap()
            .is_empty()
    );
    fixture.close().await;
}

#[tokio::test]
async fn failed_receipt_write_rolls_back_cursor_budget_and_canonical_trigger() {
    let fixture = Fixture::new().await;
    let app = fixture.event_intake().await;
    sqlx::raw_sql("CREATE TRIGGER reject_app_receipt BEFORE INSERT ON app_event_receipts BEGIN SELECT RAISE(ABORT, 'fixture persistence failure'); END;")
        .execute(&fixture.store.pool).await.unwrap();
    assert!(
        fixture
            .store
            .accept_signed_event(&app.id, 1, &notification(1), 100)
            .await
            .is_err()
    );
    for query in [
        "SELECT COUNT(*) FROM app_event_cursors",
        "SELECT COUNT(*) FROM app_event_budgets",
        "SELECT COUNT(*) FROM app_event_receipts",
        "SELECT COUNT(*) FROM loop_trigger_sources",
        "SELECT COUNT(*) FROM loop_activations",
    ] {
        let count: i64 = sqlx::query_scalar(query)
            .fetch_one(&fixture.store.pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "{query}");
    }
    sqlx::raw_sql("DROP TRIGGER reject_app_receipt")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    let accepted = fixture
        .store
        .accept_signed_event(&app.id, 1, &notification(1), 100)
        .await
        .unwrap();
    assert!(!accepted.duplicate);
    fixture.close().await;
}

#[tokio::test]
async fn disabled_or_full_loop_intake_does_not_consume_event_identity_or_budget() {
    let fixture = Fixture::new().await;
    let app = fixture.event_intake().await;
    let mut limits = LoopLimits::default();
    fixture
        .event_policy(LoopPolicyState::Disabled, 1, &limits)
        .await;
    rejected(
        fixture
            .store
            .accept_signed_event(&app.id, 1, &notification(1), 100)
            .await,
        AppEventIntakeError::Disabled,
    );
    limits.sources_per_activation = 1;
    limits.pending_per_actor = 1;
    fixture
        .event_policy(LoopPolicyState::Suspended, 2, &limits)
        .await;
    fixture
        .store
        .accept_signed_event(&app.id, 1, &notification(1), 100)
        .await
        .unwrap();
    rejected(
        fixture
            .store
            .accept_signed_event(&app.id, 1, &notification(2), 101)
            .await,
        AppEventIntakeError::Capacity,
    );
    let cursor: i64 = sqlx::query_scalar("SELECT cursor FROM app_event_cursors WHERE app_id = ?")
        .bind(&app.id)
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(cursor, 1);
    let used: i64 = sqlx::query_scalar(
        "SELECT accepted_count FROM app_event_budgets WHERE scope_kind = 'app' AND scope_id = ?",
    )
    .bind(&app.id)
    .fetch_one(&fixture.store.pool)
    .await
    .unwrap();
    assert_eq!(used, 1);
    limits.sources_per_activation = 2;
    fixture
        .event_policy(LoopPolicyState::Suspended, 3, &limits)
        .await;
    fixture
        .store
        .accept_signed_event(&app.id, 1, &notification(2), 102)
        .await
        .unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn event_storms_are_bounded_without_charging_duplicates_or_clock_rollback() {
    let fixture = Fixture::new().await;
    let app = fixture.event_intake().await;
    for cursor in 1..=30 {
        fixture
            .store
            .accept_signed_event(&app.id, 1, &notification(cursor), 1000)
            .await
            .unwrap();
    }
    for now in [999, 1000, 1059] {
        rejected(
            fixture
                .store
                .accept_signed_event(&app.id, 1, &notification(31), now)
                .await,
            AppEventIntakeError::Capacity,
        );
    }
    assert!(
        fixture
            .store
            .accept_signed_event(&app.id, 1, &notification(1), 1059)
            .await
            .unwrap()
            .duplicate
    );
    fixture
        .store
        .accept_signed_event(&app.id, 1, &notification(31), 1060)
        .await
        .unwrap();
    let audit = fixture.store.event_denials(&app.id).await.unwrap();
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].count, 3);
    // App and Team caps also bind a member below its own limit; their partial writes roll back.
    for (kind, id) in [("app", app.id.as_str()), ("team", "team")] {
        sqlx::query("UPDATE app_event_budgets SET accepted_count = 120 WHERE scope_kind = ? AND scope_id = ?")
            .bind(kind).bind(id).execute(&fixture.store.pool).await.unwrap();
        rejected(
            fixture
                .store
                .accept_signed_event(&app.id, 1, &notification(32), 1061)
                .await,
            AppEventIntakeError::Capacity,
        );
        sqlx::query(
            "UPDATE app_event_budgets SET accepted_count = 1 WHERE scope_kind = ? AND scope_id = ?",
        )
        .bind(kind)
        .bind(id)
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    }
    fixture
        .store
        .accept_signed_event(&app.id, 1, &notification(32), 1061)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app_event_receipts")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(count, 32);
    fixture.close().await;
}
