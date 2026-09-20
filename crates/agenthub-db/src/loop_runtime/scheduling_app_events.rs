use agenthub_agent_domain::{
    app_events::{AppEventAttribution, AppEventNotification},
    loop_scheduling::{LoopRegistration, LoopRegistrationInput, LoopSchedule},
};
use sqlx::{Row, Sqlite, Transaction};

use super::{LoopStore, LoopStoreError, scheduling_revocation::revoke_registrations};
use crate::app_registry::{AppRegistry, AppStoreError};

pub(super) struct AppEventObservation {
    pub route_revision: i64,
    pub observed_cursor: i64,
    pub pending_cursor: Option<i64>,
}

pub(super) async fn prepare_watch(
    tx: &mut Transaction<'_, Sqlite>,
    input: &LoopRegistrationInput,
) -> anyhow::Result<Option<AppEventObservation>> {
    let LoopSchedule::AppEvent {
        app_id,
        event_class,
        after_cursor,
        ..
    } = &input.schedule
    else {
        return Ok(None);
    };
    let route = AppRegistry::authorize_event_route_tx(
        tx,
        app_id,
        &input.team_id,
        &input.actor_id,
        event_class,
    )
    .await
    .map_err(|error| {
        if authority_denied(&error) {
            LoopStoreError::ScopeMismatch.into()
        } else {
            error
        }
    })?;
    // The same write lock covers history catch-up and watch installation, preventing lost wakeups.
    let row = sqlx::query(
        "SELECT MIN(cursor) AS first, MAX(cursor) AS last FROM app_event_receipts \
        WHERE app_id = ? AND team_id = ? AND actor_id = ? AND event_class = ? AND cursor > ?",
    )
    .bind(app_id)
    .bind(&input.team_id)
    .bind(&input.actor_id)
    .bind(event_class)
    .bind(after_cursor)
    .fetch_one(&mut **tx)
    .await?;
    Ok(Some(AppEventObservation {
        route_revision: route.revision,
        observed_cursor: row
            .try_get::<Option<i64>, _>("last")?
            .unwrap_or(*after_cursor),
        pending_cursor: row.try_get("first")?,
    }))
}

pub(super) async fn install_watch(
    tx: &mut Transaction<'_, Sqlite>,
    registration: &LoopRegistration,
    observation: &AppEventObservation,
) -> anyhow::Result<()> {
    let input = &registration.input;
    let LoopSchedule::AppEvent {
        app_id,
        event_class,
        ..
    } = &input.schedule
    else {
        anyhow::bail!("app event watch requires an app event schedule");
    };
    sqlx::query("INSERT INTO app_event_watches(registration_id, app_id, team_id, actor_id, event_class, route_revision) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&registration.id).bind(app_id).bind(&input.team_id).bind(&input.actor_id).bind(event_class)
        .bind(observation.route_revision).execute(&mut **tx).await?;
    Ok(())
}

pub(super) async fn watch_obsolete(
    tx: &mut Transaction<'_, Sqlite>,
    registration: &LoopRegistration,
) -> anyhow::Result<bool> {
    let input = &registration.input;
    let LoopSchedule::AppEvent {
        app_id,
        event_class,
        ..
    } = &input.schedule
    else {
        return Ok(false);
    };
    let route = match AppRegistry::authorize_event_route_tx(
        tx,
        app_id,
        &input.team_id,
        &input.actor_id,
        event_class,
    )
    .await
    {
        Ok(route) => route,
        Err(error) if authority_denied(&error) => return Ok(true),
        Err(error) => return Err(error),
    };
    let current: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM app_event_watches WHERE registration_id = ? \
        AND app_id = ? AND team_id = ? AND actor_id = ? AND event_class = ? AND route_revision = ?)")
        .bind(&registration.id).bind(app_id).bind(&input.team_id).bind(&input.actor_id).bind(event_class)
        .bind(route.revision).fetch_one(&mut **tx).await?;
    Ok(!current)
}

pub(super) async fn event_attribution(
    tx: &mut Transaction<'_, Sqlite>,
    input: &LoopRegistrationInput,
    cursor: i64,
) -> anyhow::Result<Option<(String, AppEventAttribution)>> {
    let LoopSchedule::AppEvent {
        app_id,
        event_class,
        ..
    } = &input.schedule
    else {
        return Ok(None);
    };
    let row = sqlx::query("SELECT event_id, version FROM app_event_receipts WHERE app_id = ? AND team_id = ? AND actor_id = ? AND event_class = ? AND cursor = ?")
        .bind(app_id).bind(&input.team_id).bind(&input.actor_id).bind(event_class).bind(cursor)
        .fetch_one(&mut **tx).await?;
    Ok(Some((
        app_id.clone(),
        AppEventAttribution {
            event_id: row.try_get("event_id")?,
            event_class: event_class.clone(),
            cursor,
            version: row.try_get("version")?,
        },
    )))
}

fn authority_denied(error: &anyhow::Error) -> bool {
    error.downcast_ref::<AppStoreError>().is_some()
        || matches!(
            error.downcast_ref::<LoopStoreError>(),
            Some(LoopStoreError::ScopeMismatch)
        )
}

impl LoopStore {
    /// Called only after a new authorized receipt was inserted in this transaction, never on retries.
    pub(crate) async fn observe_app_event_schedule_tx(
        tx: &mut Transaction<'_, Sqlite>,
        app_id: &str,
        event: &AppEventNotification,
        route_revision: i64,
        now: i64,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE loop_registrations SET observed_cursor = ?, \
            next_check_at = CASE WHEN pending_cursor IS NULL THEN ? ELSE next_check_at END, \
            pending_cursor = COALESCE(pending_cursor, ?), pending_due_at = COALESCE(pending_due_at, ?), updated_at = ? \
            WHERE state = 'active' AND observed_cursor < ? AND id IN \
            (SELECT registration_id FROM app_event_watches WHERE app_id = ? AND team_id = ? AND actor_id = ? AND event_class = ? AND route_revision = ?)")
            .bind(event.cursor).bind(now).bind(event.cursor).bind(now).bind(now).bind(event.cursor)
            .bind(app_id).bind(&event.team_id).bind(&event.actor_id).bind(&event.event_class).bind(route_revision)
            .execute(&mut **tx).await?;
        Ok(())
    }

    /// Authority changes revoke idle watches immediately and reuse ordinary source cancellation.
    pub(crate) async fn revoke_app_schedules_tx(
        tx: &mut Transaction<'_, Sqlite>,
        app_id: &str,
        team_id: Option<&str>,
        actor_id: Option<&str>,
        now: i64,
    ) -> anyhow::Result<()> {
        let ids: Vec<String> = sqlx::query_scalar("SELECT r.id FROM loop_registrations r JOIN app_event_watches w ON w.registration_id = r.id \
            WHERE r.state != 'revoked' AND w.app_id = ? AND (? IS NULL OR w.team_id = ?) AND (? IS NULL OR w.actor_id = ?)")
            .bind(app_id).bind(team_id).bind(team_id).bind(actor_id).bind(actor_id).fetch_all(&mut **tx).await?;
        revoke_registrations(tx, &ids, now).await
    }
}
