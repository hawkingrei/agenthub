use agenthub_agent_domain::loop_runtime::LoopTriggerReceipt;
use agenthub_agent_domain::loop_scheduling::{
    LoopRegistration, LoopRegistrationDetail, LoopRegistrationFiring, LoopSchedule,
};
use sqlx::{Acquire, Executor, Row, Sqlite, Transaction};

use super::{
    LoopStore, LoopStoreError,
    scheduling::{IDLE_CHECK_AT, parse_registration, registration_trigger},
    scheduling_revocation::revoke_registrations,
};

const RECONCILE_LIMIT: i64 = 32;
const CAPACITY_RETRY_SECONDS: i64 = 5;

impl LoopStore {
    /// Reconcile bounded durable intent before ordinary admission, including after daemon restart.
    pub async fn reconcile_schedules(
        &self,
        now: i64,
    ) -> anyhow::Result<Vec<LoopRegistrationFiring>> {
        anyhow::ensure!(now >= 0, "invalid reconciliation timestamp");
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM loop_registrations WHERE state = 'active' AND next_check_at <= ? \
             ORDER BY next_check_at, id LIMIT ?",
        )
        .bind(now)
        .bind(RECONCILE_LIMIT)
        .fetch_all(&self.pool)
        .await?;
        let mut firings = Vec::new();
        for id in ids {
            let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
            let row = sqlx::query(
                "SELECT * FROM loop_registrations WHERE id = ? AND state = 'active' AND next_check_at <= ?",
            ).bind(&id).bind(now).fetch_optional(&mut *tx).await?;
            if let Some(row) = row {
                let mut registration = parse_registration(&row)?;
                if registration_obsolete(&mut tx, &registration).await? {
                    revoke_registrations(&mut tx, &[id], now).await?;
                } else {
                    latch_timer(&mut tx, &mut registration, now).await?;
                    if let Some(firing) = accept_firing(&mut tx, &registration, now).await? {
                        firings.push(firing);
                    }
                }
            }
            tx.commit().await?;
        }
        Ok(firings)
    }

    pub async fn registration_firings(
        &self,
        team_id: &str,
        id: &str,
        after: Option<i64>,
        limit: u32,
    ) -> anyhow::Result<Vec<LoopRegistrationFiring>> {
        anyhow::ensure!((1..=256).contains(&limit), "invalid firing page size");
        anyhow::ensure!(
            after.is_none_or(|cursor| cursor >= 0),
            "invalid firing cursor"
        );
        load_firings(&self.pool, team_id, id, after, limit).await
    }

    pub async fn registration_detail(
        &self,
        team_id: &str,
        id: &str,
        after: Option<i64>,
        limit: u32,
    ) -> anyhow::Result<LoopRegistrationDetail> {
        anyhow::ensure!((1..=256).contains(&limit), "invalid firing page size");
        anyhow::ensure!(
            after.is_none_or(|cursor| cursor >= 0),
            "invalid firing cursor"
        );
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT * FROM loop_registrations WHERE team_id = ? AND id = ?")
            .bind(team_id)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(LoopStoreError::ScopeMismatch)?;
        let registration = parse_registration(&row)?;
        let mut firings = load_firings(&mut *tx, team_id, id, after, limit + 1).await?;
        let has_more = firings.len() > limit as usize;
        firings.truncate(limit as usize);
        let next_firing_cursor =
            has_more.then(|| firings.last().expect("positive page size").first_cursor);
        tx.commit().await?;
        Ok(LoopRegistrationDetail {
            registration,
            firings,
            next_firing_cursor,
        })
    }
}

async fn load_firings<'e>(
    executor: impl Executor<'e, Database = Sqlite>,
    team_id: &str,
    id: &str,
    after: Option<i64>,
    limit: u32,
) -> anyhow::Result<Vec<LoopRegistrationFiring>> {
    let rows = sqlx::query(
        "SELECT f.*, s.activation_id FROM loop_registration_firings f \
             JOIN loop_registrations r ON r.id = f.registration_id \
             JOIN loop_trigger_sources s ON s.id = f.trigger_id \
             WHERE r.team_id = ? AND r.id = ? AND (? IS NULL OR f.first_cursor > ?) \
             ORDER BY f.first_cursor LIMIT ?",
    )
    .bind(team_id)
    .bind(id)
    .bind(after)
    .bind(after)
    .bind(limit)
    .fetch_all(executor)
    .await?;
    rows.iter()
        .map(|row| {
            Ok(LoopRegistrationFiring {
                registration_id: row.try_get("registration_id")?,
                first_cursor: row.try_get("first_cursor")?,
                through_cursor: row.try_get("through_cursor")?,
                created_at: row.try_get("created_at")?,
                receipt: LoopTriggerReceipt {
                    trigger_id: row.try_get("trigger_id")?,
                    activation_id: row.try_get("activation_id")?,
                    duplicate: false,
                },
            })
        })
        .collect()
}

async fn registration_obsolete(
    tx: &mut Transaction<'_, Sqlite>,
    registration: &LoopRegistration,
) -> anyhow::Result<bool> {
    if super::scheduling_app_events::watch_obsolete(tx, registration).await? {
        return Ok(true);
    }
    sqlx::query_scalar(
        "SELECT (work_task_id IS NOT NULL AND NOT EXISTS \
         (SELECT 1 FROM team_tasks t WHERE t.id = r.work_task_id AND t.team_id = r.team_id AND t.status NOT IN ('completed', 'canceled'))) \
         OR (origin_activation_id IS NOT NULL AND EXISTS \
         (SELECT 1 FROM loop_activations a WHERE a.id = r.origin_activation_id AND a.state = 'canceled')) \
         OR (dependency_task_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM team_tasks t WHERE t.id = r.dependency_task_id AND t.team_id = r.team_id)) \
         OR (thread_root_message_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM team_conversation_messages m WHERE m.id = r.thread_root_message_id)) \
         FROM loop_registrations r WHERE id = ?",
    ).bind(&registration.id).fetch_one(&mut **tx).await.map_err(Into::into)
}

async fn latch_timer(
    tx: &mut Transaction<'_, Sqlite>,
    registration: &mut LoopRegistration,
    now: i64,
) -> anyhow::Result<()> {
    let Some(due) = registration.next_due_at.filter(|due| *due <= now) else {
        return Ok(());
    };
    let (through, next) = match registration.input.schedule {
        LoopSchedule::Due { .. } => (due, None),
        LoopSchedule::Recurring {
            interval_seconds, ..
        } => {
            let interval = i64::from(interval_seconds);
            // Collapse missed intervals in constant time, preserving their represented range.
            let through = due + ((now - due) / interval) * interval;
            (through, through.checked_add(interval))
        }
        _ => return Ok(()),
    };
    registration.pending_cursor.get_or_insert(due);
    registration.pending_due_at.get_or_insert(due);
    registration.observed_cursor = through;
    registration.next_due_at = next;
    sqlx::query(
        "UPDATE loop_registrations SET pending_cursor = ?, pending_due_at = ?, observed_cursor = ?, \
         next_due_at = ?, updated_at = ? WHERE id = ?",
    ).bind(registration.pending_cursor).bind(registration.pending_due_at).bind(through)
        .bind(next).bind(now).bind(&registration.id).execute(&mut **tx).await?;
    Ok(())
}

async fn accept_firing(
    tx: &mut Transaction<'_, Sqlite>,
    registration: &LoopRegistration,
    now: i64,
) -> anyhow::Result<Option<LoopRegistrationFiring>> {
    let Some(first_cursor) = registration.pending_cursor else {
        return Ok(None);
    };
    let mut input = registration_trigger(&registration.input);
    input.source_key = format!("registration:{}:{first_cursor}", registration.id);
    // The registration already enforces the deadline. Immediate sources share ordinary coalescing.
    // The firing table retains the exact timer/event cursor without a separate overdue bucket.
    if let LoopSchedule::ThreadReply { .. } = registration.input.schedule {
        input.references.conversation_message_id = Some(first_cursor);
    }
    if let Some((app_id, attribution)) =
        super::scheduling_app_events::event_attribution(tx, &registration.input, first_cursor)
            .await?
    {
        input.references.app_id = Some(app_id);
        input.references.app_event = Some(attribution);
    }
    let mut savepoint = tx.begin().await?;
    let accepted = LoopStore::accept_in_transaction(&mut savepoint, &input, now).await;
    let receipt = match accepted {
        Ok(receipt) => {
            savepoint.commit().await?;
            receipt
        }
        Err(error) => {
            // Intake owns multiple writes; a failed firing must not leak a partial activation.
            savepoint.rollback().await?;
            match error.downcast_ref::<LoopStoreError>() {
                Some(LoopStoreError::Capacity | LoopStoreError::Disabled) => {
                    sqlx::query("UPDATE loop_registrations SET next_check_at = ?, updated_at = ? WHERE id = ?")
                        .bind(now.saturating_add(CAPACITY_RETRY_SECONDS)).bind(now).bind(&registration.id)
                        .execute(&mut **tx).await?;
                }
                Some(LoopStoreError::ScopeMismatch) => {
                    revoke_registrations(tx, std::slice::from_ref(&registration.id), now).await?;
                }
                _ => return Err(error),
            }
            return Ok(None);
        }
    };
    sqlx::query(
        "INSERT INTO loop_registration_firings(registration_id, first_cursor, through_cursor, trigger_id, created_at) VALUES (?, ?, ?, ?, ?)",
    ).bind(&registration.id).bind(first_cursor).bind(registration.observed_cursor)
        .bind(&receipt.trigger_id).bind(now).execute(&mut **tx).await?;
    let completed = !registration.input.schedule.repeats()
        || matches!(registration.input.schedule, LoopSchedule::Recurring { .. })
            && registration.next_due_at.is_none();
    sqlx::query(
        "UPDATE loop_registrations SET state = ?, pending_cursor = NULL, pending_due_at = NULL, \
         next_check_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(if completed { "completed" } else { "active" })
    .bind(registration.next_due_at.unwrap_or(IDLE_CHECK_AT))
    .bind(now)
    .bind(&registration.id)
    .execute(&mut **tx)
    .await?;
    Ok(Some(LoopRegistrationFiring {
        registration_id: registration.id.clone(),
        first_cursor,
        through_cursor: registration.observed_cursor,
        receipt,
        created_at: now,
    }))
}
