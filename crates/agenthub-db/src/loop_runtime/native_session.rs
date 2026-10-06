use agenthub_agent_domain::loop_runtime::{
    LoopLaunchSnapshot, LoopReservation, LoopSessionPolicy, validate_loop_id,
};
use sqlx::{Connection, Row, Sqlite, Transaction};

use super::{LoopStore, LoopStoreError};

impl LoopStore {
    /// Consume one opening attempt before sending create/resume. None means first/fresh
    /// conversation; an existing resume binding never falls back to creating another one.
    pub async fn begin_native_session(
        &self,
        expected: &LoopReservation,
        configuration_digest: &str,
        now: i64,
    ) -> anyhow::Result<Option<String>> {
        anyhow::ensure!(
            configuration_digest.len() == 64
                && configuration_digest.bytes().all(|b| b.is_ascii_hexdigit()),
            LoopStoreError::InvalidState
        );
        let mut connection = self.pool.acquire().await?;
        sqlx::query("PRAGMA synchronous = FULL")
            .execute(&mut *connection)
            .await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        let launch = require_opening_owner(&mut tx, expected, now).await?;
        let previous = sqlx::query("SELECT * FROM loop_native_sessions WHERE actor_id = ?")
            .bind(&expected.actor_id)
            .fetch_optional(&mut *tx)
            .await?;
        let mut native_session_id: Option<String> = None;
        if let Some(previous) = previous {
            anyhow::ensure!(
                previous.try_get::<&str, _>("team_id")? == expected.team_id,
                LoopStoreError::ScopeMismatch
            );
            let generation: i64 = previous.try_get("generation")?;
            anyhow::ensure!(
                generation < expected.generation,
                LoopStoreError::InvalidState
            );
            // Never infer descendant retirement from lease expiry or local session status.
            let retired: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loop_activation_events WHERE activation_id = ? AND generation = ? AND kind = 'cleanup_verified')")
                .bind(previous.try_get::<&str, _>("activation_id")?).bind(generation)
                .fetch_one(&mut *tx).await?;
            anyhow::ensure!(retired, LoopStoreError::ReservationHeld);
            if launch.session_policy == LoopSessionPolicy::Resume {
                anyhow::ensure!(
                    previous.try_get::<&str, _>("configuration_digest")? == configuration_digest,
                    LoopStoreError::NativeConfigurationChanged
                );
                anyhow::ensure!(
                    previous.try_get::<&str, _>("state")? == "bound",
                    LoopStoreError::NativeOpeningUncertain
                );
                native_session_id = previous.try_get("native_session_id")?;
                anyhow::ensure!(
                    native_session_id.is_some(),
                    LoopStoreError::NativeOpeningUncertain
                );
            }
        }
        sqlx::query("INSERT INTO loop_native_sessions(actor_id, team_id, activation_id, generation, local_session_id, configuration_digest, native_session_id, state, updated_at) \
            VALUES (?, ?, ?, ?, ?, ?, ?, 'opening', ?) ON CONFLICT(actor_id) DO UPDATE SET \
            team_id = excluded.team_id, activation_id = excluded.activation_id, generation = excluded.generation, \
            local_session_id = excluded.local_session_id, configuration_digest = excluded.configuration_digest, \
            native_session_id = excluded.native_session_id, state = 'opening', updated_at = excluded.updated_at")
            .bind(&expected.actor_id).bind(&expected.team_id).bind(&expected.activation_id)
            .bind(expected.generation).bind(&expected.session_id).bind(configuration_digest)
            .bind(&native_session_id).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(native_session_id)
    }

    /// Called only after committing the accepted opening receipt and its event stream.
    pub async fn bind_native_session(
        &self,
        expected: &LoopReservation,
        native_session_id: &str,
        now: i64,
    ) -> anyhow::Result<()> {
        validate_loop_id(native_session_id)?;
        let mut connection = self.pool.acquire().await?;
        sqlx::query("PRAGMA synchronous = FULL")
            .execute(&mut *connection)
            .await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        require_opening_owner(&mut tx, expected, now).await?;
        let changed = sqlx::query("UPDATE loop_native_sessions SET native_session_id = ?, state = 'bound', updated_at = ? \
            WHERE actor_id = ? AND team_id = ? AND activation_id = ? AND generation = ? AND local_session_id = ? \
            AND (native_session_id IS NULL OR native_session_id = ?)")
            .bind(native_session_id).bind(now).bind(&expected.actor_id).bind(&expected.team_id)
            .bind(&expected.activation_id).bind(expected.generation).bind(&expected.session_id)
            .bind(native_session_id).execute(&mut *tx).await?.rows_affected();
        anyhow::ensure!(changed == 1, LoopStoreError::InvalidState);
        tx.commit().await?;
        Ok(())
    }
}

async fn require_opening_owner(
    tx: &mut Transaction<'_, Sqlite>,
    expected: &LoopReservation,
    now: i64,
) -> anyhow::Result<LoopLaunchSnapshot> {
    LoopStore::verify_executor_phase_tx(tx, expected, now, true).await?;
    let row = sqlx::query(
        "SELECT a.launch_json FROM loop_activations a \
        JOIN loop_execution_reservations r ON r.activation_id = a.id \
        JOIN agent_sessions s ON s.id = r.session_id AND s.agent_id = r.actor_id \
        WHERE a.id = ? AND a.state = 'starting' AND r.executor_state = 'guarded' \
        AND r.session_id = ? AND a.session_id = r.session_id AND s.ended_at IS NULL",
    )
    .bind(&expected.activation_id)
    .bind(&expected.session_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(LoopStoreError::InvalidState)?;
    let launch: LoopLaunchSnapshot = serde_json::from_str(row.try_get("launch_json")?)?;
    anyhow::ensure!(launch.provider_id == "rara", LoopStoreError::InvalidState);
    Ok(launch)
}
