use crate::loop_runtime::{LoopPolicyUpdate, LoopStore};
use crate::runtime_events::{
    RuntimeEventStore, RuntimeRequestAck, RuntimeRequestIntent, RuntimeRequestKind,
};
use agenthub_agent_domain::loop_runtime::{
    LoopAdmission, LoopDeferralReason, LoopLimits, LoopPolicyState, LoopSessionPolicy,
    LoopSourceReferences, LoopTriggerInput, LoopTriggerKind,
};

use super::*;

mod binding;

struct Fixture {
    directory: std::path::PathBuf,
    store: NativeSessionStore,
}

impl Fixture {
    async fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("native-owners-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let pool = crate::init_db_at_path(&directory.join("control.sqlite"))
            .await
            .unwrap();
        for agent in ["agent", "other"] {
            sqlx::query("INSERT INTO agents(id, name, workdir, command, args, worktree_mode, status, created_at, updated_at) VALUES (?, ?, '/tmp', 'rara', '[]', 'use_existing', 'created', 1, 1)")
                .bind(agent).bind(agent).execute(&pool).await.unwrap();
        }
        Self {
            directory,
            store: NativeSessionStore::new(pool),
        }
    }

    async fn reserve(&self, agent: &str) -> NativeExecutionOwner {
        self.store
            .reserve(agent, &uuid::Uuid::new_v4().to_string(), "daemon", 100)
            .await
            .unwrap()
    }

    async fn guarded(&self, agent: &str) -> NativeExecutionOwner {
        let owner = self.reserve(agent).await;
        self.store.authorize_spawn(&owner, 101).await.unwrap();
        sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES (?, ?, 'running', 101)")
            .bind(&owner.local_session_id).bind(agent).execute(&self.store.pool).await.unwrap();
        owner
    }

    async fn reopen(&mut self) {
        self.store.pool.close().await;
        self.store = NativeSessionStore::new(
            crate::init_db_at_path(&self.directory.join("control.sqlite"))
                .await
                .unwrap(),
        );
    }

    async fn close(self) {
        self.store.pool.close().await;
        std::fs::remove_dir_all(self.directory).unwrap();
    }
}

#[tokio::test]
async fn standalone_ownership_survives_reopen_and_requires_exact_cleanup() {
    let mut fixture = Fixture::new().await;
    let first = fixture.guarded("agent").await;
    fixture.reopen().await;
    fixture.store.verify_live(&first).await.unwrap();
    assert!(!fixture.store.cleanup_unstarted(&first, 200).await.unwrap());
    sqlx::query("UPDATE agent_sessions SET ended_at = 200, status = 'exited' WHERE id = ?")
        .bind(&first.local_session_id)
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    assert!(fixture.store.verify_live(&first).await.is_err());
    assert!(
        fixture
            .store
            .reserve("agent", "next", "replacement-daemon", i64::MAX)
            .await
            .is_err(),
        "elapsed time and a closed local session cannot release descendants"
    );
    for field in 0..4 {
        let mut stale = first.clone();
        match field {
            0 => stale.agent_id = "other".into(),
            1 => stale.owner_id = "other-daemon".into(),
            2 => stale.local_session_id = "other-local".into(),
            _ => stale.generation += 1,
        }
        assert!(fixture.store.cleanup_verified(&stale, 201).await.is_err());
        assert!(fixture.store.authorize_spawn(&stale, 201).await.is_err());
    }
    fixture.store.cleanup_verified(&first, 202).await.unwrap();
    let second = fixture.guarded("agent").await;
    assert_eq!(second.generation, first.generation + 1);
    fixture.store.cleanup_verified(&first, 203).await.unwrap();
    assert!(fixture.store.cleanup_unstarted(&first, 203).await.unwrap());
    fixture.store.verify_live(&second).await.unwrap();
    assert_eq!(
        fixture
            .store
            .active_owner("agent")
            .await
            .unwrap()
            .unwrap()
            .owner,
        second
    );
    fixture.close().await;
}

#[tokio::test]
async fn standalone_spawn_and_unstarted_cleanup_have_one_winner() {
    let fixture = Fixture::new().await;
    let owner = fixture.reserve("agent").await;
    let (spawn, cleanup) = tokio::join!(
        fixture.store.authorize_spawn(&owner, 102),
        fixture.store.cleanup_unstarted(&owner, 102),
    );
    if cleanup.unwrap() {
        assert!(spawn.is_err());
        assert!(fixture.store.active_owner("agent").await.unwrap().is_none());
    } else {
        spawn.unwrap();
        assert_eq!(
            fixture
                .store
                .active_owner("agent")
                .await
                .unwrap()
                .unwrap()
                .state,
            NativeExecutionState::Guarded
        );
        assert!(fixture.store.authorize_spawn(&owner, 103).await.is_err());
    }
    fixture.close().await;
}

#[tokio::test]
async fn standalone_racing_reservations_never_admit_two_writers() {
    let fixture = Fixture::new().await;
    let (one, two) = tokio::join!(
        fixture.store.reserve("agent", "one", "daemon-one", 100),
        fixture.store.reserve("agent", "two", "daemon-two", 100)
    );
    assert_ne!(one.is_ok(), two.is_ok());
    let owner = one.or(two).unwrap();
    fixture.store.cleanup_unstarted(&owner, 101).await.unwrap();
    let next = fixture.reserve("agent").await;
    assert_eq!(next.generation, 2);
    assert!(fixture.store.authorize_spawn(&owner, 102).await.is_err());
    fixture.close().await;
}

#[tokio::test]
async fn standalone_scope_changes_revoke_input_without_erasing_cleanup_obligations() {
    let fixture = Fixture::new().await;
    let owner = fixture.guarded("agent").await;
    let mut tx = fixture.store.pool.begin().await.unwrap();
    assert!(
        LoopStore::require_scope_quiescent_tx(&mut tx, "agent")
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    for command in ["codex", "rara"] {
        sqlx::query("UPDATE agents SET command = ? WHERE id = 'agent'")
            .bind(command)
            .execute(&fixture.store.pool)
            .await
            .unwrap();
        assert_eq!(
            fixture.store.verify_live(&owner).await.is_ok(),
            command == "rara"
        );
    }
    sqlx::query("INSERT INTO team_definitions(id, name, spec_json, created_at, updated_at) VALUES ('team', 'team', ?, 100, 100)")
        .bind(serde_json::json!({"members":[{"member_id":"agent"}]}).to_string()).execute(&fixture.store.pool).await.unwrap();
    assert!(fixture.store.verify_live(&owner).await.is_err());
    let loops = LoopStore::new(fixture.store.pool.clone());
    loops
        .configure(
            LoopPolicyUpdate {
                actor_id: "agent",
                team_id: "team",
                expected_revision: 0,
                state: LoopPolicyState::Enabled,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits::default(),
            },
            102,
        )
        .await
        .unwrap();
    let trigger = loops
        .accept_trigger(
            &LoopTriggerInput {
                actor_id: "agent".into(),
                team_id: "team".into(),
                kind: LoopTriggerKind::Operator,
                source_key: "change-mode".into(),
                due_at: None,
                references: LoopSourceReferences::default(),
            },
            103,
        )
        .await
        .unwrap();
    assert_eq!(
        loops
            .admit("team", &trigger.activation_id, "loop-daemon", 104)
            .await
            .unwrap(),
        LoopAdmission::Deferred(LoopDeferralReason::Reserved)
    );
    assert!(
        loops
            .reserve_manual("team", "agent", "loop-daemon", 104)
            .await
            .is_err()
    );
    assert!(fixture.store.clear_conversation("agent").await.is_err());
    fixture.store.cleanup_verified(&owner, 105).await.unwrap();
    assert!(matches!(
        loops
            .admit("team", &trigger.activation_id, "loop-daemon", 120)
            .await
            .unwrap(),
        LoopAdmission::Admitted(_)
    ));
    assert!(
        fixture
            .store
            .reserve("agent", "standalone", "daemon", 121)
            .await
            .is_err()
    );
    fixture.close().await;
}

#[tokio::test]
async fn standalone_identity_deletion_requires_retirement_and_removes_only_its_bindings() {
    let fixture = Fixture::new().await;
    let owner = fixture.guarded("agent").await;
    let other = fixture.guarded("other").await;
    fixture
        .store
        .begin_conversation(&owner, &"a".repeat(64), LoopSessionPolicy::Fresh, 102)
        .await
        .unwrap();
    fixture
        .store
        .bind_conversation(&owner, "native", 103)
        .await
        .unwrap();
    fixture.store.cleanup_verified(&owner, 104).await.unwrap();
    let mut tx = fixture
        .store
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap();
    assert!(
        LoopStore::require_scope_quiescent_tx(&mut tx, "agent")
            .await
            .unwrap()
            .is_none()
    );
    sqlx::query("DELETE FROM agent_sessions WHERE agent_id = 'agent'")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DELETE FROM agents WHERE id = 'agent'")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let owners: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM native_execution_owners WHERE agent_id = 'agent'")
            .fetch_one(&fixture.store.pool)
            .await
            .unwrap();
    let bindings: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM native_standalone_conversations WHERE agent_id = 'agent'",
    )
    .fetch_one(&fixture.store.pool)
    .await
    .unwrap();
    assert_eq!((owners, bindings), (0, 0));
    fixture.store.verify_live(&other).await.unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn standalone_invalid_identity_and_exhausted_generation_fail_closed() {
    let fixture = Fixture::new().await;
    assert!(
        fixture
            .store
            .reserve("missing", "local", "daemon", 1)
            .await
            .is_err()
    );
    for (local, daemon, now) in [
        ("", "daemon", 1),
        ("local", "bad\nowner", 1),
        ("local", "daemon", -1),
    ] {
        assert!(
            fixture
                .store
                .reserve("agent", local, daemon, now)
                .await
                .is_err()
        );
    }
    let owner = fixture.reserve("agent").await;
    fixture.store.cleanup_unstarted(&owner, 101).await.unwrap();
    sqlx::query("UPDATE native_execution_owners SET generation = ? WHERE agent_id = 'agent'")
        .bind(i64::MAX)
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .reserve("agent", "new", "daemon", 102)
            .await
            .is_err()
    );
    fixture.close().await;
}
