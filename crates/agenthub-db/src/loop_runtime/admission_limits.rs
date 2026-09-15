use agenthub_agent_domain::loop_runtime::{
    LoopActivation, LoopDeferralReason, LoopPolicy, LoopPolicyState,
};
use sqlx::{Sqlite, Transaction};

use super::{LoopStoreError, policy::require_member};

pub(super) async fn deferral_reason(
    tx: &mut Transaction<'_, Sqlite>,
    activation: &LoopActivation,
    policy: &LoopPolicy,
    now: i64,
) -> anyhow::Result<Option<LoopDeferralReason>> {
    if let Err(error) = require_member(tx, &activation.team_id, &activation.actor_id).await {
        if matches!(error.downcast_ref(), Some(LoopStoreError::ScopeMismatch)) {
            return Ok(Some(LoopDeferralReason::MembershipChanged));
        }
        return Err(error);
    }
    match policy.state {
        LoopPolicyState::Disabled => return Ok(Some(LoopDeferralReason::Disabled)),
        LoopPolicyState::Suspended => return Ok(Some(LoopDeferralReason::Suspended)),
        LoopPolicyState::Enabled => {}
    }
    if activation.due_at > now || activation.next_admission_at > now {
        return Ok(Some(LoopDeferralReason::NotDue));
    }
    if let Some(expires) = sqlx::query_scalar::<_, i64>(
        "SELECT lease_expires_at FROM loop_execution_reservations WHERE actor_id = ?",
    )
    .bind(&activation.actor_id)
    .fetch_optional(&mut **tx)
    .await?
    {
        return Ok(Some(if expires <= now {
            LoopDeferralReason::LeaseExpiredUnfenced
        } else {
            LoopDeferralReason::Reserved
        }));
    }
    if activation.attempt_count >= i64::from(policy.limits.startup_attempts) {
        return Ok(Some(LoopDeferralReason::StartupLimit));
    }
    if policy.no_progress_count >= i64::from(policy.limits.consecutive_no_progress) {
        return Ok(Some(LoopDeferralReason::NoProgressLimit));
    }
    let actor_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM loop_activation_events e JOIN loop_activations a ON a.id = e.activation_id \
         WHERE a.actor_id = ? AND e.kind = 'admitted' AND e.created_at > ?",
    ).bind(&activation.actor_id).bind(now.saturating_sub(i64::from(policy.limits.window_seconds)))
        .fetch_one(&mut **tx).await?;
    if actor_count >= i64::from(policy.limits.activations_per_actor) {
        return Ok(Some(LoopDeferralReason::ActorRateLimit));
    }
    let (team_limit, team_window): (i64, i64) = sqlx::query_as(
        "SELECT MIN(json_extract(limits_json, '$.activations_per_team')), MAX(json_extract(limits_json, '$.window_seconds')) \
         FROM loop_policies WHERE team_id = ? AND state != 'disabled'",
    ).bind(&activation.team_id).fetch_one(&mut **tx).await?;
    let team_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM loop_activation_events e JOIN loop_activations a ON a.id = e.activation_id \
         WHERE a.team_id = ? AND e.kind = 'admitted' AND e.created_at > ?",
    ).bind(&activation.team_id).bind(now.saturating_sub(team_window))
        .fetch_one(&mut **tx).await?;
    if team_count >= team_limit {
        return Ok(Some(LoopDeferralReason::TeamRateLimit));
    }
    let conflicting_claim: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM loop_trigger_sources s JOIN team_execution_claims c \
         ON c.entity_id = json_extract(s.input_json, '$.references.task_id') AND c.entity_kind = 'task' \
         WHERE s.activation_id = ? AND s.source_kind IN ('assignment', 'continuation') \
         AND c.released_at IS NULL AND c.owner_member_id != ?)",
    ).bind(&activation.id).bind(&activation.actor_id).fetch_one(&mut **tx).await?;
    if conflicting_claim {
        return Ok(Some(LoopDeferralReason::TaskOwnedElsewhere));
    }
    Ok(None)
}
