use crate::{next_fencing_generation, require_guarded_write_applied};

use sqlx::Connection;

use super::*;

impl NativeSessionStore {
    /// Reserve before spawning. A retired predecessor is retained as cleanup evidence.
    pub async fn reserve(
        &self,
        agent: &str,
        local_session: &str,
        owner: &str,
        now: i64,
    ) -> anyhow::Result<NativeExecutionOwner> {
        for id in [agent, local_session, owner] {
            validate_loop_id(id)?;
        }
        anyhow::ensure!(now >= 0, "invalid native reservation time");
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        require_standalone(&mut tx, agent).await?;
        let loop_held: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM loop_execution_reservations WHERE actor_id = ?)",
        )
        .bind(agent)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(
            !loop_held && !has_held_owner(&mut tx, agent).await?,
            NativeSessionError::ReservationHeld
        );
        let previous: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(generation) FROM native_execution_owners WHERE agent_id = ?",
        )
        .bind(agent)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(
            previous != Some(i64::MAX),
            "native execution generation exhausted"
        );
        let generation = next_fencing_generation(previous);
        sqlx::query(
            "INSERT INTO native_execution_owners(agent_id, generation, local_session_id, owner_id, state, created_at, updated_at) \
             VALUES (?, ?, ?, ?, 'reserved', ?, ?)",
        ).bind(agent).bind(generation).bind(local_session).bind(owner).bind(now).bind(now)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(NativeExecutionOwner {
            agent_id: agent.into(),
            local_session_id: local_session.into(),
            owner_id: owner.into(),
            generation,
        })
    }

    pub async fn active_owner(&self, agent: &str) -> anyhow::Result<Option<NativeExecutionRecord>> {
        sqlx::query(
            "SELECT * FROM native_execution_owners WHERE agent_id = ? AND state != 'retired'",
        )
        .bind(agent)
        .fetch_optional(&self.pool)
        .await?
        .as_ref()
        .map(parse_record)
        .transpose()
    }

    /// Commit while holding the matching guardian witness lock, before any OS spawn.
    pub async fn authorize_spawn(
        &self,
        owner: &NativeExecutionOwner,
        now: i64,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(now >= 0, "invalid native spawn time");
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        require_standalone(&mut tx, &owner.agent_id).await?;
        anyhow::ensure!(
            require_owner(&mut tx, owner).await? == NativeExecutionState::Reserved,
            NativeSessionError::InvalidState
        );
        let changed = sqlx::query("UPDATE native_execution_owners SET state = 'guarded', updated_at = ? WHERE agent_id = ? AND generation = ? AND state = 'reserved'")
            .bind(now).bind(&owner.agent_id).bind(owner.generation).execute(&mut *tx).await?.rows_affected();
        require_guarded_write_applied(changed)?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn verify_live(&self, owner: &NativeExecutionOwner) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        require_guarded(&mut tx, owner).await?;
        tx.commit().await?;
        Ok(())
    }

    /// The spawn authorization CAS prevents an owner retired here from spawning later.
    pub async fn cleanup_unstarted(
        &self,
        owner: &NativeExecutionOwner,
        now: i64,
    ) -> anyhow::Result<bool> {
        anyhow::ensure!(now >= 0, "invalid native cleanup time");
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        match require_owner(&mut tx, owner).await? {
            NativeExecutionState::Guarded => return Ok(false),
            NativeExecutionState::Retired => return Ok(true),
            NativeExecutionState::Reserved => {}
        }
        retire(&mut tx, owner, now).await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Trusted supervisor only: keep exact descendant-cleanup proof alive through this commit.
    pub async fn cleanup_verified(
        &self,
        owner: &NativeExecutionOwner,
        now: i64,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(now >= 0, "invalid native cleanup time");
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        require_owner(&mut tx, owner).await?;
        retire(&mut tx, owner, now).await?;
        tx.commit().await?;
        Ok(())
    }
}

async fn retire(
    tx: &mut Transaction<'_, Sqlite>,
    owner: &NativeExecutionOwner,
    now: i64,
) -> anyhow::Result<()> {
    let changed = sqlx::query("UPDATE native_execution_owners SET state = 'retired', updated_at = MAX(updated_at, ?) WHERE agent_id = ? AND generation = ?")
        .bind(now).bind(&owner.agent_id).bind(owner.generation).execute(&mut **tx).await?.rows_affected();
    require_guarded_write_applied(changed)?;
    Ok(())
}
