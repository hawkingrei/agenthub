use agenthub_agent_domain::loop_runtime::{LoopPolicy, LoopPolicyState};
use sqlx::{Sqlite, Transaction};

use super::{LoopStore, LoopStoreError, policy::parse_policy};

impl LoopStore {
    /// Call inside the same write transaction as the scope mutation. Runtime callers also
    /// serialize configuration against local starts, including actors not yet opted in.
    pub async fn require_scope_quiescent_tx(
        tx: &mut Transaction<'_, Sqlite>,
        actor_id: &str,
    ) -> anyhow::Result<Option<LoopPolicy>> {
        let row = sqlx::query("SELECT * FROM loop_policies WHERE actor_id = ?")
            .bind(actor_id)
            .fetch_optional(&mut **tx)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let policy = parse_policy(&row)?;
        anyhow::ensure!(
            policy.state != LoopPolicyState::Enabled,
            LoopStoreError::ScopeBusy("suspend automatic admission first")
        );
        let busy: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM loop_execution_reservations WHERE actor_id = ?1) \
             OR EXISTS(SELECT 1 FROM loop_activations WHERE actor_id = ?1 AND state IN ('pending', 'starting', 'running', 'finalizing')) \
             OR EXISTS(SELECT 1 FROM agent_sessions WHERE agent_id = ?1 AND ended_at IS NULL)",
        ).bind(actor_id).fetch_one(&mut **tx).await?;
        anyhow::ensure!(
            !busy,
            LoopStoreError::ScopeBusy("execution or accepted work is retained")
        );
        // Lease expiry alone cannot prove that a task's external writer has stopped.
        let owns_work: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM team_execution_claims WHERE owner_member_id = ?1 AND released_at IS NULL) \
             OR EXISTS(SELECT 1 FROM team_goal_leases WHERE owner_member_id = ?1 AND released_at IS NULL) \
             OR EXISTS(SELECT 1 FROM team_tasks WHERE assigned_member_id = ?1 AND status NOT IN ('completed', 'canceled'))",
        ).bind(actor_id).fetch_one(&mut **tx).await?;
        anyhow::ensure!(
            !owns_work,
            LoopStoreError::ScopeBusy("release or reassign canonical work first")
        );
        let permission: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM acp_permission_requests WHERE status = 'pending' AND (agent_id = ?1 OR review_target_actor_id = ?1))",
        ).bind(actor_id).fetch_one(&mut **tx).await?;
        anyhow::ensure!(
            !permission,
            LoopStoreError::ScopeBusy("permission requests remain unresolved")
        );
        Ok(Some(policy))
    }

    pub async fn require_no_retained_history_tx(
        tx: &mut Transaction<'_, Sqlite>,
        actor_id: &str,
    ) -> anyhow::Result<()> {
        let retained: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loop_activations WHERE actor_id = ?)")
                .bind(actor_id)
                .fetch_one(&mut **tx)
                .await?;
        anyhow::ensure!(
            !retained,
            LoopStoreError::ScopeBusy(
                "activation history retains this identity; copy the Card for a new scope"
            )
        );
        Ok(())
    }
}
