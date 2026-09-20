use std::str::FromStr;

use agenthub_agent_domain::{
    loop_metrics::*,
    loop_runtime::{LoopExitReason, LoopOutcomeKind},
};
use sqlx::{Row, Sqlite, Transaction, sqlite::SqliteRow};

use super::{LoopStore, LoopStoreError, history::history_id};

impl LoopStore {
    /// Caller authorizes the historical Team. All facts come from one read snapshot and survive
    /// executor exit. The event window is bounded; pending/wait gauges and duplicate totals are not.
    pub async fn metrics(
        &self,
        team_id: &str,
        actor_id: &str,
        now: i64,
        window_seconds: i64,
    ) -> anyhow::Result<LoopMetricsSnapshot> {
        history_id(team_id)?;
        history_id(actor_id)?;
        anyhow::ensure!(
            now >= 0 && (1..=604800).contains(&window_seconds),
            LoopStoreError::InvalidHistoryQuery
        );
        let since = now.saturating_sub(window_seconds).max(0);
        let mut tx = self.pool.begin().await?;
        let policy = sqlx::query(
            "SELECT state, no_progress_count FROM loop_policies WHERE team_id = ? AND actor_id = ?",
        )
        .bind(team_id)
        .bind(actor_id)
        .fetch_optional(&mut *tx)
        .await?;
        let pending = sqlx::query("SELECT COUNT(*) AS count, MAX(CASE WHEN created_at <= ? THEN ? - created_at END) AS oldest, \
            COALESCE(SUM(due_at <= ?), 0) AS due_count, MAX(CASE WHEN due_at <= ? AND created_at <= ? THEN ? - MAX(created_at, due_at) END) AS due_oldest \
            FROM loop_activations WHERE team_id = ? AND actor_id = ? AND state = 'pending'")
            .bind(now).bind(now).bind(now).bind(now).bind(now).bind(now)
            .bind(team_id).bind(actor_id).fetch_one(&mut *tx).await?;
        let duplicates = sqlx::query(
            "SELECT COALESCE(SUM(duplicate_count), 0) AS total, \
            COALESCE(SUM(duplicate_observation_started_at IS NULL), 0) AS unknown \
            FROM loop_trigger_sources WHERE team_id = ? AND actor_id = ?",
        )
        .bind(team_id)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await?;
        let outcomes = counts::<LoopOutcomeKind>(sqlx::query("SELECT json_extract(a.outcome_json, '$.kind') AS kind, COUNT(*) AS count \
            FROM loop_activation_events e JOIN loop_activations a ON a.id = e.activation_id \
            WHERE a.team_id = ? AND a.actor_id = ? AND e.kind = 'outcome_recorded' AND e.created_at BETWEEN ? AND ? \
            GROUP BY json_extract(a.outcome_json, '$.kind') ORDER BY kind")
            .bind(team_id).bind(actor_id).bind(since).bind(now).fetch_all(&mut *tx).await?)?;
        let exits = counts::<LoopExitReason>(sqlx::query("SELECT e.exit_reason_code AS kind, COUNT(*) AS count \
            FROM loop_activation_events e JOIN loop_activations a ON a.id = e.activation_id \
            WHERE a.team_id = ? AND a.actor_id = ? AND e.kind = 'cleanup_verified' AND e.exit_reason_code IS NOT NULL AND e.created_at BETWEEN ? AND ? \
            GROUP BY e.exit_reason_code ORDER BY kind")
            .bind(team_id).bind(actor_id).bind(since).bind(now).fetch_all(&mut *tx).await?)?;
        let exits_without_reason = sqlx::query_scalar("SELECT COUNT(*) FROM loop_activation_events e JOIN loop_activations a ON a.id = e.activation_id \
            WHERE a.team_id = ? AND a.actor_id = ? AND e.kind = 'cleanup_verified' AND e.exit_reason_code IS NULL AND e.created_at BETWEEN ? AND ?")
            .bind(team_id).bind(actor_id).bind(since).bind(now).fetch_one(&mut *tx).await?;
        let retries = sqlx::query_scalar("SELECT COUNT(*) FROM loop_activation_events e JOIN loop_activations a ON a.id = e.activation_id \
            WHERE a.team_id = ? AND a.actor_id = ? AND e.kind = 'admitted' AND e.created_at BETWEEN ? AND ? \
            AND EXISTS(SELECT 1 FROM loop_activation_events earlier WHERE earlier.activation_id = a.id AND earlier.kind = 'admitted' AND earlier.id < e.id)")
            .bind(team_id).bind(actor_id).bind(since).bind(now).fetch_one(&mut *tx).await?;
        let progress = sqlx::query("SELECT COUNT(*) AS total, COALESCE(SUM(NOT EXISTS(SELECT 1 FROM loop_progress_receipts p WHERE p.activation_id = a.id)), 0) AS no_progress \
            FROM loop_activations a WHERE a.team_id = ? AND a.actor_id = ? AND a.state IN ('finished', 'interrupted') AND a.finished_at BETWEEN ? AND ?")
            .bind(team_id).bind(actor_id).bind(since).bind(now).fetch_one(&mut *tx).await?;
        let admission_latency = durations(&mut tx, team_id, actor_id, since, now, false).await?;
        let running_duration = durations(&mut tx, team_id, actor_id, since, now, true).await?;
        let unsettled = sqlx::query("SELECT COUNT(*) AS count, MAX(CASE WHEN e.created_at <= ? THEN ? - e.created_at END) AS age \
            FROM loop_execution_reservations r JOIN loop_activations a ON a.id = r.activation_id \
            JOIN loop_activation_events e ON e.activation_id = a.id AND e.generation = r.generation AND e.kind = 'running' \
            WHERE a.team_id = ? AND a.actor_id = ? AND NOT EXISTS(SELECT 1 FROM loop_activation_events c WHERE c.activation_id = a.id AND c.generation = r.generation AND c.kind = 'cleanup_verified')")
            .bind(now).bind(now).bind(team_id).bind(actor_id).fetch_one(&mut *tx).await?;
        let mut waits = Vec::new();
        for kind in LoopWaitMetricKind::ALL {
            let row = sqlx::query("SELECT COUNT(*) AS count, MAX(CASE WHEN started_at <= ? THEN ? - started_at END) AS oldest \
                FROM (SELECT COALESCE((SELECT f.created_at FROM loop_registration_firings f WHERE f.registration_id = r.id ORDER BY f.first_cursor DESC LIMIT 1), r.created_at) AS started_at \
                FROM loop_registrations r WHERE r.team_id = ? AND r.actor_id = ? AND r.state = 'active' AND json_extract(r.input_json, '$.schedule.kind') = ?)")
                .bind(now).bind(now).bind(team_id).bind(actor_id).bind(kind.as_str()).fetch_one(&mut *tx).await?;
            waits.push(LoopWaitMetrics {
                kind,
                count: row.try_get("count")?,
                oldest_age_seconds: row.try_get("oldest")?,
            });
        }
        // Only the most recently admitted activation can describe the actor's current business
        // wait. A future pending activation does not end that wait; a newer admission does.
        let current_wait = sqlx::query("SELECT json_extract(a.outcome_json, '$.wait_reason') AS reason, e.created_at \
            FROM loop_activations a JOIN loop_activation_events e ON e.activation_id = a.id AND e.kind = 'outcome_recorded' \
            WHERE a.id = (SELECT newer.activation_id FROM loop_activation_events newer JOIN loop_activations n ON n.id = newer.activation_id \
              WHERE n.team_id = ? AND n.actor_id = ? AND newer.kind = 'admitted' ORDER BY newer.id DESC LIMIT 1) \
            AND json_extract(a.outcome_json, '$.kind') = 'waiting' AND a.state != 'canceled'")
            .bind(team_id).bind(actor_id).fetch_optional(&mut *tx).await?
            .map(|row| -> anyhow::Result<LoopBusinessWaitMetrics> {
                let recorded_at = row.try_get("created_at")?;
                Ok(LoopBusinessWaitMetrics { reason: row.try_get::<&str, _>("reason")?.parse()?, recorded_at,
                    age_seconds: (now >= recorded_at).then_some(now.saturating_sub(recorded_at)) })
            }).transpose()?;
        let mem_counts = counts(sqlx::query("SELECT e.kind, COUNT(*) AS count FROM loop_activation_events e \
            JOIN loop_activations a ON a.id = e.activation_id WHERE a.team_id = ? AND a.actor_id = ? \
            AND e.kind IN ('mem_context_ready', 'mem_context_unavailable', 'mem_context_missing', 'mem_context_invalid') AND e.created_at BETWEEN ? AND ? GROUP BY e.kind ORDER BY e.kind")
            .bind(team_id).bind(actor_id).bind(since).bind(now).fetch_all(&mut *tx).await?)?;
        let latest_mem = sqlx::query("SELECT e.kind, e.created_at FROM loop_activation_events e \
            JOIN loop_activations a ON a.id = e.activation_id WHERE a.team_id = ? AND a.actor_id = ? \
            AND e.kind IN ('mem_context_ready', 'mem_context_unavailable', 'mem_context_missing', 'mem_context_invalid') ORDER BY e.id DESC LIMIT 1")
            .bind(team_id).bind(actor_id).fetch_optional(&mut *tx).await?
            .map(|row| -> anyhow::Result<LoopMemObservation> {
                Ok(LoopMemObservation { kind: row.try_get::<&str, _>("kind")?.parse()?, observed_at: row.try_get("created_at")? })
            }).transpose()?;
        let snapshot = LoopMetricsSnapshot {
            observed_at: now,
            window_start: since,
            policy_state: policy
                .as_ref()
                .map(|row| -> anyhow::Result<_> { row.try_get::<&str, _>("state")?.parse() })
                .transpose()?,
            pending: LoopPendingMetrics {
                count: pending.try_get("count")?,
                oldest_age_seconds: pending.try_get("oldest")?,
                due_count: pending.try_get("due_count")?,
                oldest_due_age_seconds: pending.try_get("due_oldest")?,
            },
            admission_latency,
            running_duration,
            unsettled_run_count: unsettled.try_get("count")?,
            oldest_unsettled_run_age_seconds: unsettled.try_get("age")?,
            startup_failures: exits
                .iter()
                .filter(|entry| entry.kind == LoopExitReason::StartupFailed)
                .map(|entry| entry.count)
                .sum(),
            outcomes,
            exits,
            exits_without_reason,
            retries,
            duplicates: LoopDuplicateMetrics {
                suppressed_total: duplicates.try_get("total")?,
                sources_with_unknown_baseline: duplicates.try_get("unknown")?,
            },
            progress: LoopProgressMetrics {
                finalized_activations: progress.try_get("total")?,
                no_progress_activations: progress.try_get("no_progress")?,
                current_no_progress_streak: policy
                    .as_ref()
                    .map(|row| row.try_get("no_progress_count"))
                    .transpose()?,
            },
            waits,
            current_wait,
            mem: LoopMemMetrics {
                latest: latest_mem,
                observations: mem_counts,
            },
        };
        tx.commit().await?;
        Ok(snapshot)
    }
}

fn counts<T: FromStr<Err = anyhow::Error>>(
    rows: Vec<SqliteRow>,
) -> anyhow::Result<Vec<LoopMetricCount<T>>> {
    rows.iter()
        .map(|row| {
            Ok(LoopMetricCount {
                kind: row.try_get::<&str, _>("kind")?.parse()?,
                count: row.try_get("count")?,
            })
        })
        .collect()
}

async fn durations(
    tx: &mut Transaction<'_, Sqlite>,
    team_id: &str,
    actor_id: &str,
    since: i64,
    now: i64,
    running: bool,
) -> anyhow::Result<LoopWallDurationMetrics> {
    let intervals = if running {
        "SELECT r.created_at AS started_at, c.created_at AS ended_at FROM loop_activations a \
         JOIN loop_activation_events r ON r.activation_id = a.id AND r.kind = 'running' \
         JOIN loop_activation_events c ON c.activation_id = a.id AND c.generation = r.generation AND c.kind = 'cleanup_verified' \
         WHERE a.team_id = ? AND a.actor_id = ? AND c.created_at BETWEEN ? AND ?"
    } else {
        "SELECT MAX(a.created_at, a.due_at) AS started_at, e.created_at AS ended_at FROM loop_activations a \
         JOIN loop_activation_events e ON e.activation_id = a.id AND e.kind = 'admitted' \
         WHERE a.team_id = ? AND a.actor_id = ? AND e.created_at BETWEEN ? AND ? \
         AND NOT EXISTS(SELECT 1 FROM loop_activation_events earlier WHERE earlier.activation_id = a.id AND earlier.kind = 'admitted' AND earlier.id < e.id)"
    };
    let mut query = sqlx::QueryBuilder::<Sqlite>::new(
        "SELECT COUNT(CASE WHEN ended_at >= started_at THEN 1 END) AS samples, \
        COALESCE(SUM(CASE WHEN ended_at >= started_at THEN ended_at - started_at ELSE 0 END), 0) AS total, \
        MAX(CASE WHEN ended_at >= started_at THEN ended_at - started_at END) AS maximum, \
        COALESCE(SUM(ended_at < started_at), 0) AS clock_regressions FROM (",
    );
    query.push(intervals).push(")");
    let row = query
        .build()
        .bind(team_id)
        .bind(actor_id)
        .bind(since)
        .bind(now)
        .fetch_one(&mut **tx)
        .await?;
    Ok(LoopWallDurationMetrics {
        samples: row.try_get("samples")?,
        total_seconds: row.try_get("total")?,
        maximum_seconds: row.try_get("maximum")?,
        clock_regressions: row.try_get("clock_regressions")?,
    })
}
