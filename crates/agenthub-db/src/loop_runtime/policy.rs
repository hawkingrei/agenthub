use agenthub_agent_domain::loop_runtime::{
    LoopLimits, LoopPolicy, LoopPolicyState, LoopSessionPolicy, validate_loop_id,
};
use sqlx::{Row, Sqlite, Transaction, sqlite::SqliteRow};

use super::{LoopStore, LoopStoreError};

pub struct LoopPolicyUpdate<'a> {
    pub actor_id: &'a str,
    pub team_id: &'a str,
    pub expected_revision: i64,
    pub state: LoopPolicyState,
    pub session_policy: LoopSessionPolicy,
    pub limits: &'a LoopLimits,
}

impl LoopStore {
    pub async fn policy(
        &self,
        team_id: &str,
        actor_id: &str,
    ) -> anyhow::Result<Option<LoopPolicy>> {
        sqlx::query("SELECT * FROM loop_policies WHERE team_id = ? AND actor_id = ?")
            .bind(team_id)
            .bind(actor_id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(parse_policy)
            .transpose()
    }

    /// Configuration changes preserve generations, budgets, accepted work, and mailbox identity.
    pub async fn configure(
        &self,
        update: LoopPolicyUpdate<'_>,
        now: i64,
    ) -> anyhow::Result<LoopPolicy> {
        validate_loop_id(update.actor_id)?;
        validate_loop_id(update.team_id)?;
        update.limits.validate()?;
        anyhow::ensure!(now >= 0, "invalid policy timestamp");
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        require_member(&mut tx, update.team_id, update.actor_id).await?;
        let existing = sqlx::query("SELECT * FROM loop_policies WHERE actor_id = ?")
            .bind(update.actor_id)
            .fetch_optional(&mut *tx)
            .await?;
        if let Some(row) = existing.as_ref() {
            let current = parse_policy(row)?;
            if current.team_id != update.team_id {
                return Err(LoopStoreError::ScopeMismatch.into());
            }
            if current.revision != update.expected_revision {
                return Err(LoopStoreError::RevisionConflict.into());
            }
        } else if update.expected_revision != 0 {
            return Err(LoopStoreError::RevisionConflict.into());
        }
        let revision = update
            .expected_revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("loop policy revision exhausted"))?;
        let row = sqlx::query(
            "INSERT INTO loop_policies(actor_id, team_id, state, session_policy, revision, limits_json, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(actor_id) DO UPDATE SET \
             state = excluded.state, session_policy = excluded.session_policy, revision = excluded.revision, \
             limits_json = excluded.limits_json, updated_at = excluded.updated_at RETURNING *",
        )
        .bind(update.actor_id)
        .bind(update.team_id)
        .bind(update.state.as_str())
        .bind(update.session_policy.as_str())
        .bind(revision)
        .bind(serde_json::to_string(update.limits)?)
        .bind(now)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        let policy = parse_policy(&row)?;
        tx.commit().await?;
        Ok(policy)
    }
}

pub(super) async fn require_member(
    tx: &mut Transaction<'_, Sqlite>,
    team_id: &str,
    actor_id: &str,
) -> anyhow::Result<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM team_definitions t, json_each(t.spec_json, '$.members') m \
         WHERE t.id = ? AND json_extract(m.value, '$.member_id') = ?)",
    )
    .bind(team_id)
    .bind(actor_id)
    .fetch_one(&mut **tx)
    .await?;
    if !exists {
        return Err(LoopStoreError::ScopeMismatch.into());
    }
    Ok(())
}

pub(super) fn parse_policy(row: &SqliteRow) -> anyhow::Result<LoopPolicy> {
    Ok(LoopPolicy {
        actor_id: row.try_get("actor_id")?,
        team_id: row.try_get("team_id")?,
        state: row.try_get::<&str, _>("state")?.parse()?,
        session_policy: row.try_get::<&str, _>("session_policy")?.parse()?,
        revision: row.try_get("revision")?,
        mailbox_run_id: row.try_get("mailbox_run_id")?,
        limits: serde_json::from_str(row.try_get("limits_json")?)?,
        generation: row.try_get("generation")?,
        no_progress_count: row.try_get("no_progress_count")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
