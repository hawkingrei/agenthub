use agenthub_agent_domain::loop_runtime::LoopSessionPolicy;
use sqlx::Connection;

use crate::require_guarded_write_applied;
use crate::runtime_events::{RuntimeOpeningEvidence, RuntimeRequestKind};

use super::*;

impl NativeSessionStore {
    /// Consume the opening intent before sending a request; unknown openings are never retried.
    pub async fn begin_conversation(
        &self,
        owner: &NativeExecutionOwner,
        configuration_digest: &str,
        policy: LoopSessionPolicy,
        now: i64,
    ) -> anyhow::Result<Option<String>> {
        validate_digest(configuration_digest)?;
        anyhow::ensure!(now >= 0, "invalid native opening time");
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        require_guarded(&mut tx, owner).await?;
        let previous = sqlx::query(
            "SELECT c.*, o.state AS execution_state FROM native_standalone_conversations c \
             JOIN native_execution_owners o ON o.agent_id = c.agent_id AND o.local_session_id = c.local_session_id \
             WHERE c.agent_id = ?",
        ).bind(&owner.agent_id).fetch_optional(&mut *tx).await?;
        let mut native: Option<String> = None;
        if let Some(previous) = previous {
            anyhow::ensure!(
                previous.try_get::<&str, _>("execution_state")? == "retired",
                NativeSessionError::ReservationHeld
            );
            if policy == LoopSessionPolicy::Resume {
                anyhow::ensure!(
                    previous.try_get::<&str, _>("configuration_digest")? == configuration_digest,
                    NativeSessionError::ConfigurationChanged
                );
                anyhow::ensure!(
                    previous.try_get::<&str, _>("state")? == "bound",
                    NativeSessionError::OpeningUncertain
                );
                native = previous.try_get("native_session_id")?;
                anyhow::ensure!(native.is_some(), NativeSessionError::OpeningUncertain);
            }
        }
        sqlx::query(
            "INSERT INTO native_standalone_conversations(agent_id, local_session_id, configuration_digest, native_session_id, state, updated_at) \
             VALUES (?, ?, ?, ?, 'opening', ?) ON CONFLICT(agent_id) DO UPDATE SET \
             local_session_id = excluded.local_session_id, configuration_digest = excluded.configuration_digest, \
             native_session_id = excluded.native_session_id, state = 'opening', updated_at = excluded.updated_at",
        ).bind(&owner.agent_id).bind(&owner.local_session_id).bind(configuration_digest).bind(&native).bind(now)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(native)
    }

    pub async fn bind_conversation(
        &self,
        owner: &NativeExecutionOwner,
        native: &str,
        now: i64,
    ) -> anyhow::Result<()> {
        validate_loop_id(native)?;
        anyhow::ensure!(now >= 0, "invalid native binding time");
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        require_guarded(&mut tx, owner).await?;
        let changed = sqlx::query(
            "UPDATE native_standalone_conversations SET native_session_id = ?, state = 'bound', updated_at = MAX(updated_at, ?) \
             WHERE agent_id = ? AND local_session_id = ? \
             AND (native_session_id IS NULL OR native_session_id = ?)",
        ).bind(native).bind(now).bind(&owner.agent_id).bind(&owner.local_session_id).bind(native)
            .execute(&mut *tx).await?.rows_affected();
        require_guarded_write_applied(changed)?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn opening_to_reconcile(
        &self,
        owner: &NativeExecutionOwner,
        configuration_digest: &str,
    ) -> anyhow::Result<Option<String>> {
        validate_digest(configuration_digest)?;
        let mut tx = self.pool.begin().await?;
        require_guarded(&mut tx, owner).await?;
        let local = sqlx::query_scalar(
            "SELECT c.local_session_id FROM native_standalone_conversations c \
             JOIN native_execution_owners o ON o.agent_id = c.agent_id AND o.local_session_id = c.local_session_id \
             WHERE c.agent_id = ? AND c.configuration_digest = ? AND c.state = 'opening' AND o.state = 'retired'",
        ).bind(&owner.agent_id).bind(configuration_digest).fetch_optional(&mut *tx).await?;
        tx.commit().await?;
        Ok(local)
    }

    /// Only the exact closed event owner can repair an accepted opening after verified retirement.
    pub async fn reconcile_opening(
        &self,
        owner: &NativeExecutionOwner,
        configuration_digest: &str,
        evidence: &RuntimeOpeningEvidence,
        now: i64,
    ) -> anyhow::Result<()> {
        validate_digest(configuration_digest)?;
        anyhow::ensure!(now >= 0, "invalid native reconciliation time");
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        require_guarded(&mut tx, owner).await?;
        let previous = sqlx::query(
            "SELECT c.* FROM native_standalone_conversations c JOIN native_execution_owners o \
             ON o.agent_id = c.agent_id AND o.local_session_id = c.local_session_id \
             WHERE c.agent_id = ? AND c.local_session_id = ? AND c.configuration_digest = ? AND o.state = 'retired'",
        ).bind(&owner.agent_id).bind(&evidence.local_session_id).bind(configuration_digest)
            .fetch_optional(&mut *tx).await?.ok_or(NativeSessionError::OpeningUncertain)?;
        let native: Option<String> = previous.try_get("native_session_id")?;
        let state: &str = previous.try_get("state")?;
        if state == "bound" && native.as_ref() == Some(&evidence.native_session_id) {
            return Ok(());
        }
        let valid = match evidence.kind {
            RuntimeRequestKind::CreateSession => native.is_none(),
            RuntimeRequestKind::ResumeSession => {
                native.as_ref() == Some(&evidence.native_session_id)
            }
            _ => false,
        };
        anyhow::ensure!(
            state == "opening" && valid,
            NativeSessionError::OpeningUncertain
        );
        sqlx::query("UPDATE native_standalone_conversations SET native_session_id = ?, state = 'bound', updated_at = ? WHERE agent_id = ?")
            .bind(&evidence.native_session_id).bind(now).bind(&owner.agent_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Explicit operator reset only. Forget continuity without resetting generation or cleanup evidence.
    pub async fn clear_conversation(&self, agent: &str) -> anyhow::Result<()> {
        validate_loop_id(agent)?;
        let mut connection = self.durable_connection().await?;
        let mut tx = connection.begin_with("BEGIN IMMEDIATE").await?;
        require_standalone(&mut tx, agent).await?;
        anyhow::ensure!(
            !has_held_owner(&mut tx, agent).await?,
            NativeSessionError::ReservationHeld
        );
        sqlx::query("DELETE FROM native_standalone_conversations WHERE agent_id = ?")
            .bind(agent)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

fn validate_digest(digest: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
        NativeSessionError::ConfigurationChanged
    );
    Ok(())
}
