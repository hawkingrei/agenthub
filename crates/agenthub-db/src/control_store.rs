//! ControlStore foundation: shared conditional-update, idempotent-insert, and audit primitives for
//! control-plane authority writes (see `docs/features/control-store.md`).
//!
//! This module formalizes patterns already proven in production -- `src/team/manager/teamspace.rs`'s
//! generation-fenced claims and guarded completions, `conversation_idempotency.rs`'s
//! insert-then-fingerprint-compare idempotency -- as a shared, typed decision layer instead of leaving
//! each authority table to hand-roll its own CAS guard, unique-violation matcher, and audit call.
//!
//! SQLite remains the authority and callers still perform their own table-specific SQL; this module
//! only centralizes the *decision* logic (did the guarded write actually apply? is a unique-index
//! conflict a safe replay or a real conflict? what does an audit record look like?) so it stops being
//! reimplemented per call site. It is not a query builder and does not thread transactions through
//! caller-supplied closures, which would require boxing futures across an unstable HRTB boundary for
//! little benefit given every table's SQL already differs.

use sqlx::{Sqlite, Transaction};
use thiserror::Error;

/// The SQLite result code for a `UNIQUE`/`PRIMARY KEY` constraint violation (`SQLITE_CONSTRAINT_UNIQUE`).
///
/// Previously duplicated as a local `const` in at least `src/team/manager/manager_consts.rs` and
/// `src/api/teams.rs`.
pub const SQLITE_CONSTRAINT_UNIQUE_CODE: &str = "2067";

#[derive(Debug, Error)]
pub enum ControlStoreError {
    /// A guarded (conditional) update did not affect exactly one row, meaning the precondition it
    /// encoded in its `WHERE` clause no longer held -- someone else already changed, released, or
    /// completed the entity.
    #[error("conditional update precondition no longer holds")]
    PreconditionFailed,
    /// An idempotency-key replay was attempted with a payload that does not match the original write.
    #[error("idempotency key reused with a different payload")]
    IdempotencyConflict,
}

/// Require a guarded `UPDATE`/`INSERT ... ON CONFLICT` to have affected exactly one row.
///
/// SQLite has no `RETURNING`-based optimistic-lock primitive that distinguishes "the predicate matched
/// zero rows" from "the row does not exist"; checking `rows_affected()` after the fact is the existing,
/// correct pattern (see `team_goal_forks` completion, `teamspace.rs`). This just gives that check a
/// name and a typed error instead of an ad hoc `anyhow::bail!` at each call site.
pub fn require_guarded_write_applied(rows_affected: u64) -> Result<(), ControlStoreError> {
    if rows_affected != 1 {
        return Err(ControlStoreError::PreconditionFailed);
    }
    Ok(())
}

/// Compute the next fencing generation from a possibly-absent current value.
///
/// `current` must come from a `SELECT` performed inside the same transaction as the subsequent write,
/// so the generation decision is never based on a stale read (see `claim_execution_entity`,
/// `teamspace.rs`). This function only names the arithmetic; it does not perform the read itself,
/// because the read is a table-specific query.
pub fn next_fencing_generation(current: Option<i64>) -> i64 {
    current.map(|generation| generation + 1).unwrap_or(1)
}

/// Whether a `sqlx::Error` is a `UNIQUE` constraint violation on the given index's column list.
///
/// Matching on the constraint's column list (as SQLite reports it in the error message), not just the
/// result code, avoids conflating two different unique indexes on the same table. Generalizes
/// `is_task_conversation_message_idempotency_unique_violation`
/// (`src/team/manager/conversation_idempotency.rs`) and its sibling in `mailbox_queries.rs`.
pub fn is_unique_violation(err: &sqlx::Error, unique_index_columns: &str) -> bool {
    match err {
        sqlx::Error::Database(db_err) => {
            db_err.code().as_deref() == Some(SQLITE_CONSTRAINT_UNIQUE_CODE)
                && db_err.message().contains(unique_index_columns)
        }
        _ => false,
    }
}

/// Outcome of resolving an idempotency-key replay against the row that already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdempotentReplay {
    /// The existing row's fingerprint matches the incoming write; safe to return it as-if-inserted.
    SamePayload,
    /// The existing row's fingerprint does not match; the key was reused for a different write.
    DifferentPayload,
}

/// Resolve an idempotency-key replay once the caller has fetched the conflicting row and computed
/// whether its fingerprint matches the incoming write.
///
/// Centralizes the decision every idempotent-insert call site otherwise reimplements: "same key, same
/// payload" is a safe replay; "same key, different payload" is a real conflict, not a replay.
pub fn resolve_idempotent_replay(outcome: IdempotentReplay) -> Result<(), ControlStoreError> {
    match outcome {
        IdempotentReplay::SamePayload => Ok(()),
        IdempotentReplay::DifferentPayload => Err(ControlStoreError::IdempotencyConflict),
    }
}

/// A control-plane audit record, matching the `team_audit_events` schema (`agenthub-db` `init_db`).
pub struct AuditEvent<'a> {
    pub team_id: &'a str,
    pub actor_user_id: Option<&'a str>,
    pub event_kind: &'a str,
    pub subject_kind: &'a str,
    pub subject_id: &'a str,
    pub detail: serde_json::Value,
    pub created_at: i64,
}

/// Append an audit record inside `tx`, so it commits atomically with the write it documents.
///
/// Generalizes `append_audit_event` (`src/team/manager/teamspace.rs`), which today is only reachable
/// from Teamspace execution-claim/goal/fork code. `team_audit_events`'s columns are already
/// generic (`subject_kind`/`subject_id`, not claim-specific), so this is the same table, opened up as a
/// shared primitive any future authority write can use instead of adding its own audit path.
pub async fn record_audit_event(
    tx: &mut Transaction<'_, Sqlite>,
    event: AuditEvent<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO team_audit_events (
            team_id, actor_user_id, event_kind, subject_kind, subject_id, detail_json, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(event.team_id)
    .bind(event.actor_user_id)
    .bind(event.event_kind)
    .bind(event.subject_kind)
    .bind(event.subject_id)
    .bind(event.detail.to_string())
    .bind(event.created_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{Row, SqlitePool};
    use uuid::Uuid;

    async fn test_pool() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!("agenthub-control-store-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        crate::init_db_at_path(&dir.join("agenthub.db"))
            .await
            .expect("init db")
    }

    /// Seed the minimal valid `team_definitions` row `team_audit_events` foreign-keys against.
    async fn seed_team(pool: &SqlitePool, team_id: &str, name: &str) {
        sqlx::query(
            "INSERT INTO team_definitions (id, name, spec_json, created_at, updated_at) \
             VALUES (?1, ?2, '{}', 1700000000, 1700000000)",
        )
        .bind(team_id)
        .bind(name)
        .execute(pool)
        .await
        .expect("seed team");
    }

    #[test]
    fn guarded_write_requires_exactly_one_row_affected() {
        assert!(require_guarded_write_applied(1).is_ok());
        assert!(matches!(
            require_guarded_write_applied(0),
            Err(ControlStoreError::PreconditionFailed)
        ));
        assert!(matches!(
            require_guarded_write_applied(2),
            Err(ControlStoreError::PreconditionFailed)
        ));
    }

    #[test]
    fn fencing_generation_starts_at_one_and_increments_from_current() {
        assert_eq!(next_fencing_generation(None), 1);
        assert_eq!(next_fencing_generation(Some(1)), 2);
        assert_eq!(next_fencing_generation(Some(41)), 42);
    }

    #[test]
    fn idempotent_replay_accepts_matching_payload_and_rejects_mismatch() {
        assert!(resolve_idempotent_replay(IdempotentReplay::SamePayload).is_ok());
        assert!(matches!(
            resolve_idempotent_replay(IdempotentReplay::DifferentPayload),
            Err(ControlStoreError::IdempotencyConflict)
        ));
    }

    #[tokio::test]
    async fn unique_violation_matches_the_conflicting_indexs_columns_only() {
        let pool = test_pool().await;
        sqlx::query(
            "CREATE TABLE control_store_unique_probe (a TEXT NOT NULL, b TEXT NOT NULL, \
             UNIQUE(a, b))",
        )
        .execute(&pool)
        .await
        .expect("create probe table");
        sqlx::query("INSERT INTO control_store_unique_probe (a, b) VALUES ('x', 'y')")
            .execute(&pool)
            .await
            .expect("first insert");

        let err = sqlx::query("INSERT INTO control_store_unique_probe (a, b) VALUES ('x', 'y')")
            .execute(&pool)
            .await
            .expect_err("second insert must violate the unique index");

        assert!(is_unique_violation(
            &err,
            "control_store_unique_probe.a, control_store_unique_probe.b"
        ));
        assert!(!is_unique_violation(
            &err,
            "some_other_table.unrelated_column"
        ));
    }

    #[tokio::test]
    async fn non_unique_errors_are_never_classified_as_unique_violations() {
        let pool = test_pool().await;
        let err = sqlx::query("SELECT * FROM this_table_does_not_exist")
            .fetch_one(&pool)
            .await
            .expect_err("querying a missing table must fail");
        assert!(!is_unique_violation(&err, "anything"));
    }

    #[tokio::test]
    async fn record_audit_event_commits_inside_the_callers_transaction() {
        let pool = test_pool().await;
        let team_id = format!("team-{}", Uuid::new_v4());
        seed_team(&pool, &team_id, "control-store-audit-team").await;

        let mut tx = pool.begin().await.expect("begin");
        record_audit_event(
            &mut tx,
            AuditEvent {
                team_id: &team_id,
                actor_user_id: Some("user-1"),
                event_kind: "control_store.probe",
                subject_kind: "task",
                subject_id: "task-1",
                detail: serde_json::json!({"note": "control store foundation test"}),
                created_at: 1_700_000_000,
            },
        )
        .await
        .expect("record audit event");
        tx.commit().await.expect("commit");

        let row = sqlx::query(
            "SELECT team_id, actor_user_id, event_kind, subject_kind, subject_id, detail_json \
             FROM team_audit_events WHERE team_id = ?1",
        )
        .bind(&team_id)
        .fetch_one(&pool)
        .await
        .expect("fetch audit row");
        assert_eq!(row.get::<String, _>("team_id"), team_id);
        assert_eq!(
            row.get::<Option<String>, _>("actor_user_id").as_deref(),
            Some("user-1")
        );
        assert_eq!(row.get::<String, _>("event_kind"), "control_store.probe");
        assert_eq!(row.get::<String, _>("subject_kind"), "task");
        assert_eq!(row.get::<String, _>("subject_id"), "task-1");
        assert!(
            row.get::<String, _>("detail_json")
                .contains("control store foundation test")
        );
    }

    #[tokio::test]
    async fn record_audit_event_rolls_back_with_the_rest_of_a_failed_transaction() {
        let pool = test_pool().await;
        let team_id = format!("team-{}", Uuid::new_v4());
        seed_team(&pool, &team_id, "control-store-rollback-team").await;

        let mut tx = pool.begin().await.expect("begin");
        record_audit_event(
            &mut tx,
            AuditEvent {
                team_id: &team_id,
                actor_user_id: None,
                event_kind: "control_store.probe_rollback",
                subject_kind: "task",
                subject_id: "task-2",
                detail: serde_json::json!({}),
                created_at: 1_700_000_000,
            },
        )
        .await
        .expect("record audit event");
        // Simulate the rest of the logical operation failing after the audit call: roll back instead
        // of committing.
        tx.rollback().await.expect("rollback");

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM team_audit_events WHERE event_kind = 'control_store.probe_rollback'",
        )
        .fetch_one(&pool)
        .await
        .expect("count audit rows");
        assert_eq!(
            count, 0,
            "a rolled-back transaction must not leave a partial audit record behind"
        );
    }
}
