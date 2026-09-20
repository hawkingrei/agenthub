use std::collections::BTreeMap;

use agenthub_agent_domain::loop_runtime::{LoopLimits, LoopPolicyState, LoopSessionPolicy};
use agenthub_db::loop_runtime::{LoopPolicyUpdate, LoopStore, LoopStoreError};
use serde_json::Value;
use sqlx::{Sqlite, Transaction};

use super::TeamManager;

impl TeamManager {
    pub fn uses_loop_execution(spec: &Value) -> bool {
        spec.get("execution_mode").and_then(Value::as_str) == Some("loop")
    }

    pub(super) async fn guard_loop_membership_change_tx(
        tx: &mut Transaction<'_, Sqlite>,
        team_id: &str,
        previous: &Value,
        next: &Value,
    ) -> anyhow::Result<()> {
        let previous_members = members(previous);
        let next_members = members(next);
        let mode_changed = Self::uses_loop_execution(previous) != Self::uses_loop_execution(next);
        for (actor, member) in &previous_members {
            let authority_changed = mode_changed
                || next_members.get(actor).is_none_or(|next| {
                    member.get("role") != next.get("role")
                        || member.get("runtime") != next.get("runtime")
                });
            if authority_changed {
                Self::require_loop_actor_quiescent_tx(tx, actor).await?;
            }
        }
        for actor in next_members.keys() {
            if Self::uses_loop_execution(next) && !previous_members.contains_key(actor) {
                let running: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE agent_id = ? AND ended_at IS NULL) OR EXISTS(SELECT 1 FROM loop_execution_reservations WHERE actor_id = ?)",
                ).bind(actor).bind(actor).fetch_one(&mut **tx).await?;
                anyhow::ensure!(
                    !running,
                    LoopStoreError::ScopeBusy(
                        "stop and fence execution before moving this identity into a Team"
                    )
                );
                Self::require_loop_actor_quiescent_tx(tx, actor).await?;
            }
            let policy_team: Option<String> =
                sqlx::query_scalar("SELECT team_id FROM loop_policies WHERE actor_id = ?")
                    .bind(actor)
                    .fetch_optional(&mut **tx)
                    .await?;
            anyhow::ensure!(
                policy_team.as_deref().is_none_or(|scope| scope == team_id),
                LoopStoreError::ScopeBusy("another Team retains this identity; copy its Card")
            );
            let other_team: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM team_definitions t, json_each(t.spec_json, '$.members') m \
                 WHERE t.id != ?1 AND json_extract(m.value, '$.member_id') = ?2 \
                 AND (?3 OR json_extract(t.spec_json, '$.execution_mode') = 'loop'))",
            ).bind(team_id).bind(actor).bind(Self::uses_loop_execution(next) || policy_team.is_some())
                .fetch_one(&mut **tx).await?;
            anyhow::ensure!(
                !other_team,
                LoopStoreError::ScopeBusy("loop members require one Team identity")
            );
        }
        Ok(())
    }

    pub(crate) async fn require_loop_actor_quiescent_tx(
        tx: &mut Transaction<'_, Sqlite>,
        actor_id: &str,
    ) -> anyhow::Result<bool> {
        let Some(policy) = LoopStore::require_scope_quiescent_tx(tx, actor_id).await? else {
            return Ok(false);
        };
        let run_ids: Vec<String> = sqlx::query_scalar("SELECT id FROM team_runs WHERE team_id = ?")
            .bind(&policy.team_id)
            .fetch_all(&mut **tx)
            .await?;
        for run_id in run_ids {
            let snapshots = super::mailbox_reply_obligation_summary::load_reply_obligation_message_snapshots_on_executor(&mut **tx, &run_id).await?;
            let replies = super::mailbox_reply_obligation_summary::summarize_open_reply_obligations_from_snapshots(&snapshots);
            anyhow::ensure!(
                replies
                    .open_by_actor
                    .get(actor_id)
                    .copied()
                    .unwrap_or_default()
                    == 0,
                LoopStoreError::ScopeBusy("resolve canonical reply obligations first")
            );
        }
        let pending_mail: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM team_actor_messages m JOIN team_runs r ON r.id = m.run_id \
             WHERE r.team_id = ? AND m.to_actor_id = ? AND m.status = 'pending')",
        )
        .bind(&policy.team_id)
        .bind(actor_id)
        .fetch_one(&mut **tx)
        .await?;
        anyhow::ensure!(
            !pending_mail,
            LoopStoreError::ScopeBusy("consume or reroute the pending mailbox first")
        );
        Ok(true)
    }

    pub(super) async fn ensure_loop_member_policies_tx(
        tx: &mut Transaction<'_, Sqlite>,
        team_id: &str,
        spec: &Value,
        now: i64,
    ) -> anyhow::Result<()> {
        if !Self::uses_loop_execution(spec) {
            return Ok(());
        }
        for actor in members(spec).keys() {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loop_policies WHERE actor_id = ?)")
                    .bind(actor)
                    .fetch_one(&mut **tx)
                    .await?;
            if !exists {
                LoopStore::configure_tx(
                    tx,
                    LoopPolicyUpdate {
                        actor_id: actor,
                        team_id,
                        expected_revision: 0,
                        state: LoopPolicyState::Disabled,
                        session_policy: LoopSessionPolicy::Fresh,
                        limits: &LoopLimits::default(),
                    },
                    now,
                )
                .await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn guard_loop_team_deletion_tx(
        tx: &mut Transaction<'_, Sqlite>,
        team_id: &str,
    ) -> anyhow::Result<()> {
        let actors: Vec<String> =
            sqlx::query_scalar("SELECT actor_id FROM loop_policies WHERE team_id = ?")
                .bind(team_id)
                .fetch_all(&mut **tx)
                .await?;
        for actor in actors {
            Self::require_loop_actor_quiescent_tx(tx, &actor).await?;
            LoopStore::require_no_retained_history_tx(tx, &actor).await?;
        }
        Ok(())
    }

    pub(super) async fn detach_empty_loop_policies_tx(
        tx: &mut Transaction<'_, Sqlite>,
        team_id: &str,
        previous: &Value,
        next: &Value,
    ) -> anyhow::Result<()> {
        let next = members(next);
        for actor in members(previous)
            .keys()
            .filter(|actor| !next.contains_key(*actor))
        {
            sqlx::query("DELETE FROM loop_policies WHERE actor_id = ?1 AND team_id = ?2 AND NOT EXISTS(SELECT 1 FROM loop_activations WHERE actor_id = ?1) AND NOT EXISTS(SELECT 1 FROM loop_registrations WHERE actor_id = ?1)")
                .bind(actor).bind(team_id).execute(&mut **tx).await?;
            sqlx::query("UPDATE loop_policies SET state = 'disabled', revision = revision + 1 WHERE actor_id = ? AND team_id = ?")
                .bind(actor).bind(team_id).execute(&mut **tx).await?;
        }
        Ok(())
    }
}

fn members(spec: &Value) -> BTreeMap<&str, &Value> {
    spec.get("members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|member| {
            member
                .get("member_id")
                .and_then(Value::as_str)
                .map(|id| (id, member))
        })
        .collect()
}
