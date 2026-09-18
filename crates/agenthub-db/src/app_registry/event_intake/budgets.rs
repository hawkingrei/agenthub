use agenthub_agent_domain::app_events::AppEventNotification;
use sqlx::{Row, Sqlite, Transaction};

use super::AppEventIntakeError;

const WINDOW_SECONDS: i64 = 60;

/// These intake caps supplement, rather than replace, pending-work and execution admission budgets.
pub(super) async fn admit(
    tx: &mut Transaction<'_, Sqlite>,
    app_id: &str,
    event: &AppEventNotification,
    now: i64,
) -> anyhow::Result<()> {
    for (kind, id, limit) in [
        ("app", app_id, 120),
        ("actor", event.actor_id.as_str(), 30),
        ("team", event.team_id.as_str(), 120),
    ] {
        let previous = sqlx::query("SELECT window_started_at, accepted_count FROM app_event_budgets WHERE scope_kind = ? AND scope_id = ?")
            .bind(kind).bind(id).fetch_optional(&mut **tx).await?;
        let (started, count) = match previous {
            Some(row) if now.saturating_sub(row.try_get("window_started_at")?) < WINDOW_SECONDS => {
                (
                    row.try_get::<i64, _>("window_started_at")?,
                    row.try_get::<i64, _>("accepted_count")?,
                )
            }
            _ => (now, 0),
        };
        anyhow::ensure!(count < limit, AppEventIntakeError::Capacity);
        sqlx::query("INSERT INTO app_event_budgets(scope_kind, scope_id, window_started_at, accepted_count) VALUES (?, ?, ?, ?) \
            ON CONFLICT(scope_kind, scope_id) DO UPDATE SET window_started_at = excluded.window_started_at, accepted_count = excluded.accepted_count")
            .bind(kind).bind(id).bind(started).bind(count + 1).execute(&mut **tx).await?;
    }
    Ok(())
}
