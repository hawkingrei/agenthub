use agenthub_agent_domain::loop_runtime::{
    LoopEventKind, LoopPolicyState, LoopTriggerInput, LoopTriggerReceipt,
};
use sqlx::{Row, Sqlite, Transaction};
use uuid::Uuid;

use super::{
    LoopStore, LoopStoreError,
    policy::{parse_policy, require_member},
};

impl LoopStore {
    pub async fn accept_trigger(
        &self,
        input: &LoopTriggerInput,
        now: i64,
    ) -> anyhow::Result<LoopTriggerReceipt> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let receipt = Self::accept_in_transaction(&mut tx, input, now).await?;
        tx.commit().await?;
        Ok(receipt)
    }

    /// Call inside the canonical task/message transaction to avoid a lost-wake dual write.
    /// The caller must roll back the enclosing transaction on error.
    pub async fn accept_in_transaction(
        tx: &mut Transaction<'_, Sqlite>,
        input: &LoopTriggerInput,
        now: i64,
    ) -> anyhow::Result<LoopTriggerReceipt> {
        input.validate()?;
        anyhow::ensure!(now >= 0, "invalid trigger timestamp");
        require_member(tx, &input.team_id, &input.actor_id).await?;
        let existing = sqlx::query(
            "SELECT id, activation_id, input_json FROM loop_trigger_sources \
             WHERE actor_id = ? AND team_id = ? AND source_kind = ? AND source_key = ?",
        )
        .bind(&input.actor_id)
        .bind(&input.team_id)
        .bind(input.kind.as_str())
        .bind(&input.source_key)
        .fetch_optional(&mut **tx)
        .await?;
        if let Some(row) = existing {
            let original: LoopTriggerInput = serde_json::from_str(row.try_get("input_json")?)?;
            if original != *input {
                return Err(LoopStoreError::IdempotencyConflict.into());
            }
            return Ok(LoopTriggerReceipt {
                trigger_id: row.try_get("id")?,
                activation_id: row.try_get("activation_id")?,
                duplicate: true,
            });
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
        if policy.state == LoopPolicyState::Disabled {
            return Err(LoopStoreError::Disabled.into());
        }
        validate_references(tx, input).await?;
        // Only identical future deadlines coalesce; a scheduled wake never fires early.
        let coalesce_key = input
            .due_at
            .map(|due| format!("due:{due}"))
            .unwrap_or_else(|| "immediate".into());
        let candidate = sqlx::query_scalar::<_, String>(
            "SELECT id FROM loop_activations WHERE actor_id = ? AND state = 'pending' AND coalesce_key = ?",
        )
        .bind(&input.actor_id).bind(&coalesce_key)
        .fetch_optional(&mut **tx).await?;
        let mut activation_id = None;
        if let Some(id) = candidate {
            let sources: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM loop_trigger_sources WHERE activation_id = ?",
            )
            .bind(&id)
            .fetch_one(&mut **tx)
            .await?;
            if sources < i64::from(policy.limits.sources_per_activation) {
                activation_id = Some(id);
            }
        }
        let activation_id = if let Some(id) = activation_id {
            id
        } else {
            let actor_pending: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM loop_activations WHERE actor_id = ? AND state = 'pending'",
            )
            .bind(&input.actor_id)
            .fetch_one(&mut **tx)
            .await?;
            let team_pending: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM loop_activations WHERE team_id = ? AND state = 'pending'",
            )
            .bind(&input.team_id)
            .fetch_one(&mut **tx)
            .await?;
            // The tightest configured Team bound applies to every producer in the Team.
            let team_limit: i64 = sqlx::query_scalar(
                "SELECT MIN(json_extract(limits_json, '$.pending_per_team')) FROM loop_policies \
                 WHERE team_id = ? AND state != 'disabled'",
            )
            .bind(&input.team_id)
            .fetch_one(&mut **tx)
            .await?;
            if actor_pending >= i64::from(policy.limits.pending_per_actor)
                || team_pending >= team_limit
            {
                return Err(LoopStoreError::Capacity.into());
            }
            sqlx::query("UPDATE loop_activations SET coalesce_key = NULL WHERE actor_id = ? AND state = 'pending' AND coalesce_key = ?")
                .bind(&input.actor_id).bind(&coalesce_key).execute(&mut **tx).await?;
            let id = Uuid::now_v7().to_string();
            sqlx::query(
                "INSERT INTO loop_activations(id, actor_id, team_id, state, due_at, next_admission_at, coalesce_key, policy_revision, mailbox_run_id, created_at, updated_at) \
                 VALUES (?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&id).bind(&input.actor_id).bind(&input.team_id).bind(input.due_at.unwrap_or(now))
            .bind(input.due_at.unwrap_or(now))
            .bind(&coalesce_key).bind(policy.revision).bind(&policy.mailbox_run_id).bind(now).bind(now)
            .execute(&mut **tx).await?;
            id
        };
        let trigger_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO loop_trigger_sources(id, activation_id, actor_id, team_id, source_kind, source_key, input_json, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&trigger_id).bind(&activation_id).bind(&input.actor_id).bind(&input.team_id)
        .bind(input.kind.as_str()).bind(&input.source_key).bind(serde_json::to_string(input)?).bind(now)
        .execute(&mut **tx).await?;
        sqlx::query(
            "INSERT INTO loop_activation_events(activation_id, kind, generation, trigger_id, created_at) VALUES (?, ?, 0, ?, ?)",
        )
        .bind(&activation_id).bind(LoopEventKind::TriggerAccepted.as_str()).bind(&trigger_id).bind(now)
        .execute(&mut **tx).await?;
        Ok(LoopTriggerReceipt {
            trigger_id,
            activation_id,
            duplicate: false,
        })
    }
}

pub(super) async fn validate_references(
    tx: &mut Transaction<'_, Sqlite>,
    input: &LoopTriggerInput,
) -> anyhow::Result<()> {
    let references = &input.references;
    if let Some(user_id) = &references.scheduling_user_id {
        let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = ?)")
            .bind(user_id)
            .fetch_one(&mut **tx)
            .await?;
        anyhow::ensure!(valid, LoopStoreError::ScopeMismatch);
    }
    if let Some(task_id) = &references.task_id {
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM team_tasks WHERE id = ? AND team_id = ?)",
        )
        .bind(task_id)
        .bind(&input.team_id)
        .fetch_one(&mut **tx)
        .await?;
        if !valid {
            return Err(LoopStoreError::ScopeMismatch.into());
        }
    }
    if let Some(actor_id) = &references.scheduling_actor_id {
        require_member(tx, &input.team_id, actor_id).await?;
    }
    if let Some(activation_id) = &references.scheduling_activation_id {
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM loop_activations WHERE id = ? AND team_id = ? AND actor_id = ?)",
        ).bind(activation_id).bind(&input.team_id).bind(&references.scheduling_actor_id)
            .fetch_one(&mut **tx).await?;
        if !valid {
            return Err(LoopStoreError::ScopeMismatch.into());
        }
    }
    if let Some(message_id) = references.mailbox_message_id {
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM team_actor_messages m JOIN team_runs r ON r.id = m.run_id \
             WHERE m.id = ? AND r.team_id = ? AND m.to_actor_id = ?)",
        )
        .bind(message_id)
        .bind(&input.team_id)
        .bind(&input.actor_id)
        .fetch_one(&mut **tx)
        .await?;
        if !valid {
            return Err(LoopStoreError::ScopeMismatch.into());
        }
    }
    for message_id in [references.conversation_message_id, references.thread_id]
        .into_iter()
        .flatten()
    {
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM team_conversation_messages m JOIN team_conversations c ON c.id = m.conversation_id \
             WHERE m.id = ? AND c.team_id = ?)",
        ).bind(message_id).bind(&input.team_id).fetch_one(&mut **tx).await?;
        if !valid {
            return Err(LoopStoreError::ScopeMismatch.into());
        }
    }
    Ok(())
}
