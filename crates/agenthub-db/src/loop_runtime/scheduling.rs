use agenthub_agent_domain::loop_runtime::{
    LoopPolicyState, LoopTriggerInput, LoopTriggerKind, validate_loop_id,
};
use agenthub_agent_domain::loop_scheduling::{
    LoopRegistration, LoopRegistrationInput, LoopRegistrationPage, LoopRegistrationReceipt,
    LoopSchedule,
};
use sqlx::{Row, Sqlite, Transaction, sqlite::SqliteRow};
use uuid::Uuid;

use super::{
    LoopStore, LoopStoreError,
    intake::validate_references,
    policy::{parse_policy, require_member},
};

pub(super) const IDLE_CHECK_AT: i64 = i64::MAX;

impl LoopStore {
    pub async fn register_schedule(
        &self,
        input: &LoopRegistrationInput,
        now: i64,
    ) -> anyhow::Result<LoopRegistrationReceipt> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let receipt = Self::register_schedule_tx(&mut tx, input, now).await?;
        tx.commit().await?;
        Ok(receipt)
    }

    /// Read the dependency and install its observation latch in the same canonical write lock.
    pub async fn register_schedule_tx(
        tx: &mut Transaction<'_, Sqlite>,
        input: &LoopRegistrationInput,
        now: i64,
    ) -> anyhow::Result<LoopRegistrationReceipt> {
        input.validate()?;
        anyhow::ensure!(now >= 0, "invalid registration timestamp");
        require_member(tx, &input.team_id, &input.actor_id).await?;
        if let Some(row) = sqlx::query(
            "SELECT * FROM loop_registrations WHERE actor_id = ? AND team_id = ? AND source_key = ?",
        )
        .bind(&input.actor_id).bind(&input.team_id).bind(&input.source_key)
        .fetch_optional(&mut **tx).await?
        {
            let registration = parse_registration(&row)?;
            let mut retry = input.clone();
            // A stable business request may be retried by a later activation of the same actor.
            retry.references.scheduling_activation_id =
                registration.input.references.scheduling_activation_id.clone();
            anyhow::ensure!(retry == registration.input, LoopStoreError::IdempotencyConflict);
            return Ok(LoopRegistrationReceipt { registration, duplicate: true });
        }
        let policy = sqlx::query("SELECT * FROM loop_policies WHERE actor_id = ? AND team_id = ?")
            .bind(&input.actor_id)
            .bind(&input.team_id)
            .fetch_optional(&mut **tx)
            .await?
            .as_ref()
            .map(parse_policy)
            .transpose()?
            .ok_or(LoopStoreError::Disabled)?;
        anyhow::ensure!(
            policy.state != LoopPolicyState::Disabled,
            LoopStoreError::Disabled
        );
        validate_references(tx, &registration_trigger(input)).await?;
        require_registration_origin(tx, input).await?;
        require_registration_capacity(tx, input, policy.limits.standing_per_actor).await?;
        let (dependency_task_id, thread_root_message_id) = match &input.schedule {
            LoopSchedule::TaskStatus { task_id, .. } => (Some(task_id.as_str()), None),
            LoopSchedule::ThreadReply {
                root_message_id, ..
            } => (None, Some(*root_message_id)),
            _ => (None, None),
        };
        let next_due_at = match input.schedule {
            LoopSchedule::Due { due_at } => Some(due_at),
            LoopSchedule::Recurring { first_at, .. } => Some(first_at),
            _ => None,
        };
        let (observed_cursor, matches, pending_cursor) =
            super::scheduling_observation::initial_observation(tx, input).await?;
        let pending_due_at = pending_cursor.map(|_| now);
        let next_check_at = if pending_cursor.is_some() {
            now
        } else {
            next_due_at.unwrap_or(IDLE_CHECK_AT)
        };
        let row = sqlx::query(
            "INSERT INTO loop_registrations(id, actor_id, team_id, source_key, input_json, state, \
             work_task_id, dependency_task_id, thread_root_message_id, origin_activation_id, next_due_at, \
             observed_cursor, condition_matches, pending_cursor, pending_due_at, next_check_at, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *",
        )
        .bind(Uuid::now_v7().to_string()).bind(&input.actor_id).bind(&input.team_id)
        .bind(&input.source_key).bind(serde_json::to_string(input)?)
        .bind(&input.work_task_id).bind(dependency_task_id).bind(thread_root_message_id)
        .bind(&input.references.scheduling_activation_id).bind(next_due_at)
        .bind(observed_cursor).bind(matches).bind(pending_cursor).bind(pending_due_at)
        .bind(next_check_at).bind(now).bind(now).fetch_one(&mut **tx).await?;
        Ok(LoopRegistrationReceipt {
            registration: parse_registration(&row)?,
            duplicate: false,
        })
    }

    pub async fn registration(
        &self,
        team_id: &str,
        id: &str,
    ) -> anyhow::Result<Option<LoopRegistration>> {
        sqlx::query("SELECT * FROM loop_registrations WHERE team_id = ? AND id = ?")
            .bind(team_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(parse_registration)
            .transpose()
    }

    pub async fn registrations(
        &self,
        team_id: &str,
        actor_id: &str,
        after: Option<&str>,
        limit: u32,
    ) -> anyhow::Result<LoopRegistrationPage> {
        validate_loop_id(team_id)?;
        validate_loop_id(actor_id)?;
        if let Some(after) = after {
            validate_loop_id(after)?;
        }
        anyhow::ensure!((1..=256).contains(&limit), "invalid registration page size");
        let rows = sqlx::query(
            "SELECT * FROM loop_registrations WHERE team_id = ? AND actor_id = ? \
             AND (? IS NULL OR id > ?) ORDER BY id LIMIT ?",
        )
        .bind(team_id)
        .bind(actor_id)
        .bind(after)
        .bind(after)
        .bind(i64::from(limit) + 1)
        .fetch_all(&self.pool)
        .await?;
        let registrations = rows
            .iter()
            .take(limit as usize)
            .map(parse_registration)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let next_cursor = (rows.len() > limit as usize)
            .then(|| registrations.last().expect("positive page size").id.clone());
        Ok(LoopRegistrationPage {
            registrations,
            next_cursor,
        })
    }
}

pub(super) fn registration_trigger(input: &LoopRegistrationInput) -> LoopTriggerInput {
    let mut references = input.references.clone();
    if let LoopSchedule::TaskStatus { task_id, .. } = &input.schedule {
        references.task_id = Some(task_id.clone());
    }
    if let LoopSchedule::ThreadReply {
        root_message_id, ..
    } = input.schedule
    {
        references.thread_id = Some(root_message_id);
    }
    LoopTriggerInput {
        actor_id: input.actor_id.clone(),
        team_id: input.team_id.clone(),
        kind: match input.schedule {
            LoopSchedule::Due { .. } | LoopSchedule::Recurring { .. } => LoopTriggerKind::Scheduled,
            _ => LoopTriggerKind::Dependency,
        },
        source_key: input.source_key.clone(),
        due_at: None,
        references,
    }
}

async fn require_registration_origin(
    tx: &mut Transaction<'_, Sqlite>,
    input: &LoopRegistrationInput,
) -> anyhow::Result<()> {
    if let Some(task_id) = &input.work_task_id {
        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM team_tasks WHERE id = ? AND team_id = ? AND status NOT IN ('completed', 'canceled'))",
        ).bind(task_id).bind(&input.team_id).fetch_one(&mut **tx).await?;
        anyhow::ensure!(active, LoopStoreError::InvalidState);
    }
    if let Some(id) = &input.references.scheduling_activation_id {
        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM loop_activations WHERE id = ? AND state != 'canceled')",
        )
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
        anyhow::ensure!(active, LoopStoreError::InvalidState);
    }
    Ok(())
}

async fn require_registration_capacity(
    tx: &mut Transaction<'_, Sqlite>,
    input: &LoopRegistrationInput,
    actor_limit: u32,
) -> anyhow::Result<()> {
    let actor_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM loop_registrations WHERE actor_id = ? AND state = 'active'",
    )
    .bind(&input.actor_id)
    .fetch_one(&mut **tx)
    .await?;
    let team_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM loop_registrations WHERE team_id = ? AND state = 'active'",
    )
    .bind(&input.team_id)
    .fetch_one(&mut **tx)
    .await?;
    let team_limit: i64 = sqlx::query_scalar(
        "SELECT MIN(json_extract(limits_json, '$.standing_per_team')) FROM loop_policies \
         WHERE team_id = ? AND state != 'disabled'",
    )
    .bind(&input.team_id)
    .fetch_one(&mut **tx)
    .await?;
    anyhow::ensure!(
        actor_count < i64::from(actor_limit) && team_count < team_limit,
        LoopStoreError::Capacity
    );
    Ok(())
}

pub(super) fn parse_registration(row: &SqliteRow) -> anyhow::Result<LoopRegistration> {
    Ok(LoopRegistration {
        id: row.try_get("id")?,
        input: serde_json::from_str(row.try_get("input_json")?)?,
        state: serde_json::from_value(serde_json::Value::String(row.try_get("state")?))?,
        next_due_at: row.try_get("next_due_at")?,
        observed_cursor: row.try_get("observed_cursor")?,
        pending_cursor: row.try_get("pending_cursor")?,
        pending_due_at: row.try_get("pending_due_at")?,
        next_check_at: row.try_get("next_check_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
