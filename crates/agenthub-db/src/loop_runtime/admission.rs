use agenthub_agent_domain::loop_runtime::{
    LoopActivationState, LoopAdmission, LoopDeferralReason, LoopEventKind, LoopReservation,
    validate_loop_id,
};
use sqlx::{Sqlite, Transaction};

use super::{
    LoopStore, admission_limits::deferral_reason, parse_activation, policy::parse_policy,
    reservation::reserve_in_transaction,
};

impl LoopStore {
    pub async fn admit(
        &self,
        team_id: &str,
        activation_id: &str,
        owner_id: &str,
        now: i64,
    ) -> anyhow::Result<LoopAdmission> {
        validate_loop_id(owner_id)?;
        anyhow::ensure!(now >= 0, "invalid admission time");
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(row) = sqlx::query("SELECT * FROM loop_activations WHERE id = ? AND team_id = ?")
            .bind(activation_id)
            .bind(team_id)
            .fetch_optional(&mut *tx)
            .await?
        else {
            return Ok(LoopAdmission::NotPending);
        };
        let activation = parse_activation(&row)?;
        if activation.state != LoopActivationState::Pending {
            return Ok(LoopAdmission::NotPending);
        }
        if super::lifecycle::retire_inactive_task_sources(&mut tx, &activation, now).await? {
            tx.commit().await?;
            return Ok(LoopAdmission::NotPending);
        }
        let row = sqlx::query("SELECT * FROM loop_policies WHERE actor_id = ? AND team_id = ?")
            .bind(&activation.actor_id)
            .bind(team_id)
            .fetch_one(&mut *tx)
            .await?;
        let policy = parse_policy(&row)?;
        if let Some(reason) = deferral_reason(&mut tx, &activation, &policy, now).await? {
            if reason != LoopDeferralReason::NotDue {
                record_deferral(&mut tx, activation_id, reason, now).await?;
            }
            tx.commit().await?;
            return Ok(LoopAdmission::Deferred(reason));
        }
        let reservation =
            reserve_in_transaction(&mut tx, &policy, Some(activation_id), owner_id, now).await?;
        sqlx::query(
            "UPDATE loop_activations SET state = 'starting', generation = ?, attempt_count = attempt_count + 1, \
             policy_revision = ?, updated_at = ? WHERE id = ?",
        ).bind(reservation.generation).bind(policy.revision).bind(now).bind(activation_id)
            .execute(&mut *tx).await?;
        sqlx::query("INSERT INTO loop_activation_events(activation_id, kind, generation, created_at) VALUES (?, ?, ?, ?)")
            .bind(activation_id).bind(LoopEventKind::Admitted.as_str()).bind(reservation.generation).bind(now)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(LoopAdmission::Admitted(reservation))
    }

    /// Bounded scans defer ineligible actors so they do not starve other due work.
    pub async fn admit_next(
        &self,
        owner_id: &str,
        now: i64,
    ) -> anyhow::Result<Option<LoopReservation>> {
        validate_loop_id(owner_id)?;
        anyhow::ensure!(now >= 0, "invalid admission time");
        for _ in 0..32 {
            let candidate: Option<(String, String)> = sqlx::query_as(
                "SELECT team_id, id FROM loop_activations WHERE state = 'pending' AND due_at <= ? AND next_admission_at <= ? \
                 ORDER BY next_admission_at, created_at, id LIMIT 1",
            ).bind(now).bind(now).fetch_optional(&self.pool).await?;
            let Some((team_id, id)) = candidate else {
                return Ok(None);
            };
            if let LoopAdmission::Admitted(reservation) =
                self.admit(&team_id, &id, owner_id, now).await?
            {
                return Ok(Some(reservation));
            }
        }
        Ok(None)
    }
}

async fn record_deferral(
    tx: &mut Transaction<'_, Sqlite>,
    activation_id: &str,
    reason: LoopDeferralReason,
    now: i64,
) -> anyhow::Result<()> {
    let retry_at = now
        .checked_add(5)
        .ok_or_else(|| anyhow::anyhow!("admission retry overflow"))?;
    sqlx::query("UPDATE loop_activations SET next_admission_at = MAX(due_at, ?), updated_at = ? WHERE id = ?")
        .bind(retry_at).bind(now).bind(activation_id).execute(&mut **tx).await?;
    // Reconciliation ticks do not append unbounded copies of the same deferral.
    sqlx::query(
        "INSERT INTO loop_activation_events(activation_id, kind, generation, reason_code, created_at) \
         SELECT id, 'deferred', generation, ?, ? FROM loop_activations WHERE id = ? AND NOT EXISTS ( \
           SELECT 1 FROM loop_activation_events WHERE id = (SELECT MAX(id) FROM loop_activation_events WHERE activation_id = ?) \
           AND kind = 'deferred' AND reason_code = ?)",
    ).bind(reason.as_str()).bind(now).bind(activation_id).bind(activation_id).bind(reason.as_str())
        .execute(&mut **tx).await?;
    Ok(())
}
