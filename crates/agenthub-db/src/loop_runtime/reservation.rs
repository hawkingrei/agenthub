use agenthub_agent_domain::loop_runtime::{LoopPolicy, LoopReservation, validate_loop_id};
use sqlx::{Row, Sqlite, Transaction, sqlite::SqliteRow};

use super::{
    LoopStore, LoopStoreError,
    policy::{parse_policy, require_member},
};

impl LoopStore {
    /// Commit before any OS spawn, while holding the guardian witness lock.
    /// A recovery that removed an unstarted reservation makes this CAS fail.
    pub async fn authorize_guarded_spawn(
        &self,
        expected: &LoopReservation,
        now: i64,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = require_live_reservation(&mut tx, expected, now).await?;
        let changed = sqlx::query("UPDATE loop_execution_reservations SET executor_state = 'guarded' WHERE actor_id = ? AND executor_state = 'unstarted'")
            .bind(&current.actor_id).execute(&mut *tx).await?.rows_affected();
        anyhow::ensure!(changed == 1, LoopStoreError::InvalidState);
        tx.commit().await?;
        Ok(())
    }

    pub async fn expired_foreign_reservations(
        &self,
        owner_id: &str,
        after_actor: &str,
        now: i64,
    ) -> anyhow::Result<Vec<LoopReservation>> {
        let rows = sqlx::query("SELECT r.*, p.team_id FROM loop_execution_reservations r JOIN loop_policies p ON p.actor_id = r.actor_id WHERE r.owner_id != ? AND r.lease_expires_at <= ? AND r.actor_id > ? ORDER BY r.actor_id LIMIT 128")
            .bind(owner_id).bind(now).bind(after_actor).fetch_all(&self.pool).await?;
        rows.iter().map(parse_reservation).collect()
    }

    pub async fn reservation(
        &self,
        team_id: &str,
        actor_id: &str,
    ) -> anyhow::Result<Option<LoopReservation>> {
        sqlx::query("SELECT r.*, p.team_id FROM loop_execution_reservations r JOIN loop_policies p ON p.actor_id = r.actor_id WHERE p.team_id = ? AND r.actor_id = ?")
            .bind(team_id).bind(actor_id).fetch_optional(&self.pool).await?
            .as_ref().map(parse_reservation).transpose()
    }

    /// Explicit manual execution shares the actor's writer reservation, not automatic-start budgets.
    pub async fn reserve_manual(
        &self,
        team_id: &str,
        actor_id: &str,
        owner_id: &str,
        now: i64,
    ) -> anyhow::Result<LoopReservation> {
        validate_loop_id(owner_id)?;
        anyhow::ensure!(now >= 0, "invalid reservation time");
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        require_member(&mut tx, team_id, actor_id).await?;
        let policy = sqlx::query("SELECT * FROM loop_policies WHERE team_id = ? AND actor_id = ?")
            .bind(team_id)
            .bind(actor_id)
            .fetch_optional(&mut *tx)
            .await?
            .as_ref()
            .map(parse_policy)
            .transpose()?
            .ok_or(LoopStoreError::Disabled)?;
        let reservation = reserve_in_transaction(&mut tx, &policy, None, owner_id, now).await?;
        tx.commit().await?;
        Ok(reservation)
    }

    pub async fn renew(
        &self,
        reservation: &LoopReservation,
        now: i64,
    ) -> anyhow::Result<LoopReservation> {
        anyhow::ensure!(now >= 0, "invalid renewal time");
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = require_live_reservation(&mut tx, reservation, now).await?;
        let expires = now
            .checked_add(i64::from(current.lease_seconds))
            .ok_or_else(|| anyhow::anyhow!("lease expiry overflow"))?;
        sqlx::query("UPDATE loop_execution_reservations SET lease_expires_at = MAX(lease_expires_at, ?) WHERE actor_id = ?")
            .bind(expires).bind(&current.actor_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(LoopReservation {
            lease_expires_at: current.lease_expires_at.max(expires),
            ..current
        })
    }
}

pub(super) async fn reserve_in_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    policy: &LoopPolicy,
    activation_id: Option<&str>,
    owner_id: &str,
    now: i64,
) -> anyhow::Result<LoopReservation> {
    let held: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM loop_execution_reservations WHERE actor_id = ?)",
    )
    .bind(&policy.actor_id)
    .fetch_one(&mut **tx)
    .await?;
    if held {
        return Err(LoopStoreError::ReservationHeld.into());
    }
    let generation = policy
        .generation
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("execution generation exhausted"))?;
    let expires = now
        .checked_add(i64::from(policy.limits.lease_seconds))
        .ok_or_else(|| anyhow::anyhow!("lease expiry overflow"))?;
    sqlx::query("UPDATE loop_policies SET generation = ?, updated_at = ? WHERE actor_id = ?")
        .bind(generation)
        .bind(now)
        .bind(&policy.actor_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "INSERT INTO loop_execution_reservations(actor_id, activation_id, generation, owner_id, lease_expires_at, lease_seconds, renewal_seconds, created_at, executor_state) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'unstarted')",
    ).bind(&policy.actor_id).bind(activation_id).bind(generation).bind(owner_id).bind(expires)
        .bind(policy.limits.lease_seconds).bind(policy.limits.renewal_seconds).bind(now).execute(&mut **tx).await?;
    Ok(LoopReservation {
        actor_id: policy.actor_id.clone(),
        team_id: policy.team_id.clone(),
        activation_id: activation_id.map(str::to_owned),
        generation,
        owner_id: owner_id.into(),
        lease_expires_at: expires,
        lease_seconds: policy.limits.lease_seconds,
        renewal_seconds: policy.limits.renewal_seconds,
        session_id: None,
        created_at: now,
    })
}

pub(crate) async fn require_live_reservation(
    tx: &mut Transaction<'_, Sqlite>,
    expected: &LoopReservation,
    now: i64,
) -> anyhow::Result<LoopReservation> {
    let reservation = require_matching_reservation(tx, expected).await?;
    anyhow::ensure!(
        reservation.lease_expires_at > now,
        LoopStoreError::StaleLease
    );
    if let Some(id) = &reservation.activation_id {
        let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loop_activations WHERE id = ? AND state IN ('starting', 'running', 'finalizing'))")
            .bind(id).fetch_one(&mut **tx).await?;
        anyhow::ensure!(active, LoopStoreError::InvalidState);
    }
    Ok(reservation)
}

pub(super) async fn require_matching_reservation(
    tx: &mut Transaction<'_, Sqlite>,
    expected: &LoopReservation,
) -> anyhow::Result<LoopReservation> {
    let row = sqlx::query(
        "SELECT r.*, p.team_id FROM loop_execution_reservations r JOIN loop_policies p ON p.actor_id = r.actor_id \
         WHERE r.actor_id = ? AND p.team_id = ? AND r.activation_id IS ? AND r.generation = ? AND r.owner_id = ?",
    ).bind(&expected.actor_id).bind(&expected.team_id).bind(&expected.activation_id)
        .bind(expected.generation).bind(&expected.owner_id)
        .fetch_optional(&mut **tx).await?;
    row.as_ref()
        .map(parse_reservation)
        .transpose()?
        .ok_or_else(|| LoopStoreError::StaleLease.into())
}

fn parse_reservation(row: &SqliteRow) -> anyhow::Result<LoopReservation> {
    Ok(LoopReservation {
        actor_id: row.try_get("actor_id")?,
        team_id: row.try_get("team_id")?,
        activation_id: row.try_get("activation_id")?,
        generation: row.try_get("generation")?,
        owner_id: row.try_get("owner_id")?,
        lease_expires_at: row.try_get("lease_expires_at")?,
        lease_seconds: row.try_get("lease_seconds")?,
        renewal_seconds: row.try_get("renewal_seconds")?,
        session_id: row.try_get("session_id")?,
        created_at: row.try_get("created_at")?,
    })
}
