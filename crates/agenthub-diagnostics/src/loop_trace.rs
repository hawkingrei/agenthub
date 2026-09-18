//! Read-only activation diagnostics. Public history APIs do not depend on this debug-only module.

use agenthub_agent_domain::{
    loop_history::{LoopEventHistoryPage, LoopSourceHistoryPage, LoopToolHistoryPage},
    loop_metrics::LoopMetricsSnapshot,
    loop_runtime::{
        LoopActivation, LoopActivationState, LoopEvent, LoopFinishReceipt, LoopPolicyState,
        LoopWaitReason,
    },
    loop_scheduling::LoopSchedule,
};
use agenthub_db::loop_runtime::LoopStore;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use crate::agent_trace::{AgentTraceRequest, AgentTraceStallLayer, AgentTraceVerdict};

#[derive(Debug)]
pub struct ActivationNotFound;

impl std::fmt::Display for ActivationNotFound {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("activation not found")
    }
}

impl std::error::Error for ActivationNotFound {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivationLease {
    pub generation: i64,
    pub expires_at: i64,
    pub expired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivationContinuation {
    pub activation_id: String,
    pub trigger_id: String,
    pub state: LoopActivationState,
    pub due_at: i64,
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivationNextWake {
    pub activation_id: String,
    pub due_at: i64,
    pub next_admission_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivationSchedule {
    pub id: String,
    pub schedule: LoopSchedule,
    pub next_due_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivationTrace {
    pub observed_at: i64,
    pub activation: LoopActivation,
    pub lease: Option<ActivationLease>,
    pub sources: LoopSourceHistoryPage,
    pub events: LoopEventHistoryPage,
    pub tools: LoopToolHistoryPage,
    pub latest_event: Option<LoopEvent>,
    pub continuation: Option<ActivationContinuation>,
    /// Current actor work, distinct from the inspected activation's own continuation.
    pub actor_next_wake: Option<ActivationNextWake>,
    pub active_schedules: Vec<ActivationSchedule>,
    pub next_schedule_cursor: Option<String>,
    pub oldest_unsettled_tool_id: Option<i64>,
    pub metrics: LoopMetricsSnapshot,
}

pub(super) async fn resolve(
    db: &SqlitePool,
    request: &AgentTraceRequest,
) -> anyhow::Result<Option<LoopActivation>> {
    if request.session_id.is_some() {
        return Ok(None);
    }
    let available: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'loop_tool_observations') \
        AND EXISTS(SELECT 1 FROM pragma_table_info('loop_activation_events') WHERE name = 'exit_reason_code')")
        .fetch_one(db).await?;
    if !available {
        anyhow::ensure!(
            request.activation_id.is_none(),
            "activation history requires a migrated control database"
        );
        return Ok(None);
    }
    let actor = request.agent_id.as_deref().or(request.member_id.as_deref());
    let row = sqlx::query("SELECT a.id, a.team_id FROM loop_activations a \
        LEFT JOIN loop_execution_reservations r ON r.activation_id = a.id AND r.generation = a.generation \
        WHERE (? IS NULL OR a.id = ?) AND (? IS NULL OR a.actor_id = ?) AND (? IS NULL OR a.team_id = ?) \
        ORDER BY (r.activation_id IS NOT NULL) DESC, (a.state = 'pending') DESC, \
        CASE WHEN a.state = 'pending' THEN MAX(a.due_at, a.next_admission_at) END, a.created_at DESC, a.id DESC LIMIT 1")
        .bind(&request.activation_id).bind(&request.activation_id).bind(actor).bind(actor)
        .bind(&request.team_id).bind(&request.team_id).fetch_optional(db).await?;
    let Some(row) = row else {
        if request.activation_id.is_some() {
            return Err(ActivationNotFound.into());
        }
        return Ok(None);
    };
    LoopStore::new(db.clone())
        .activation(row.try_get("team_id")?, row.try_get("id")?)
        .await
}

pub(super) async fn collect(
    db: &SqlitePool,
    activation: LoopActivation,
    limit: u32,
    now: i64,
) -> anyhow::Result<ActivationTrace> {
    let store = LoopStore::new(db.clone());
    let team = &activation.team_id;
    let actor = &activation.actor_id;
    let id = &activation.id;
    let lease = store
        .reservation(team, actor)
        .await?
        .filter(|lease| {
            lease.activation_id.as_deref() == Some(id) && lease.generation == activation.generation
        })
        .map(|lease| ActivationLease {
            generation: lease.generation,
            expires_at: lease.lease_expires_at,
            expired: lease.lease_expires_at <= now,
        });
    let sources = store
        .activation_source_history(team, actor, id, None, limit)
        .await?
        .context("activation sources disappeared")?;
    let events = store
        .activation_event_history(team, actor, id, None, limit)
        .await?
        .context("activation events disappeared")?;
    let tools = store
        .activation_tool_history(team, actor, id, None, limit)
        .await?
        .context("activation tools disappeared")?;
    let latest_id: Option<i64> =
        sqlx::query_scalar("SELECT MAX(id) FROM loop_activation_events WHERE activation_id = ?")
            .bind(id)
            .fetch_one(db)
            .await?;
    let latest_event = if let Some(last) = latest_id {
        store
            .events(team, id, last.saturating_sub(1), 1)
            .await?
            .pop()
    } else {
        None
    };
    let continuation = continuation(db, &activation).await?;
    let actor_next_wake = sqlx::query("SELECT id, due_at, next_admission_at FROM loop_activations WHERE team_id = ? AND actor_id = ? AND state = 'pending' \
        ORDER BY MAX(due_at, next_admission_at), created_at, id LIMIT 1")
        .bind(team).bind(actor).fetch_optional(db).await?
        .map(|row| -> anyhow::Result<_> { Ok(ActivationNextWake { activation_id: row.try_get("id")?, due_at: row.try_get("due_at")?, next_admission_at: row.try_get("next_admission_at")? }) }).transpose()?;
    let schedules = sqlx::query("SELECT id, json_extract(input_json, '$.schedule') AS schedule_json, next_due_at FROM loop_registrations \
        WHERE team_id = ? AND actor_id = ? AND state = 'active' ORDER BY id LIMIT ?")
        .bind(team).bind(actor).bind(i64::from(limit) + 1).fetch_all(db).await?;
    let active_schedules = schedules
        .iter()
        .take(limit as usize)
        .map(|row| -> anyhow::Result<_> {
            Ok(ActivationSchedule {
                id: row.try_get("id")?,
                schedule: serde_json::from_str(row.try_get("schedule_json")?)?,
                next_due_at: row.try_get("next_due_at")?,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let next_schedule_cursor = (schedules.len() > limit as usize).then(|| {
        active_schedules
            .last()
            .expect("positive page size")
            .id
            .clone()
    });
    // Search independently of the first tool page so pagination cannot hide an older open call.
    let oldest_unsettled_tool_id = sqlx::query_scalar(
        "SELECT id FROM loop_tool_observations WHERE activation_id = ? AND generation = ? \
        AND status = 'started' AND started_at <= ? ORDER BY started_at, id LIMIT 1",
    )
    .bind(id)
    .bind(activation.generation)
    .bind(now.saturating_sub(60))
    .fetch_optional(db)
    .await?;
    let metrics = store.metrics(team, actor, now, 86400).await?;
    Ok(ActivationTrace {
        observed_at: now,
        activation,
        lease,
        sources,
        events,
        tools,
        latest_event,
        continuation,
        actor_next_wake,
        active_schedules,
        next_schedule_cursor,
        oldest_unsettled_tool_id,
        metrics,
    })
}

async fn continuation(
    db: &SqlitePool,
    activation: &LoopActivation,
) -> anyhow::Result<Option<ActivationContinuation>> {
    let Some(expected) = activation
        .outcome
        .as_ref()
        .and_then(|value| value.continuation.as_ref())
    else {
        return Ok(None);
    };
    let receipt: Option<String> = sqlx::query_scalar(
        "SELECT receipt_json FROM loop_finish_receipts WHERE activation_id = ? AND generation = ?",
    )
    .bind(&activation.id)
    .bind(activation.generation)
    .fetch_optional(db)
    .await?;
    let Some(receipt) = receipt else {
        return Ok(None);
    };
    let receipt: LoopFinishReceipt = serde_json::from_str(&receipt)?;
    let Some(next) = receipt.continuation else {
        return Ok(None);
    };
    let row = sqlx::query("SELECT a.id, a.state, a.due_at, EXISTS(SELECT 1 FROM loop_revoked_sources r WHERE r.trigger_id = s.id) AS revoked \
        FROM loop_activations a JOIN loop_trigger_sources s ON s.activation_id = a.id \
        WHERE a.id = ? AND a.team_id = ? AND a.actor_id = ? AND s.id = ? AND s.source_kind = 'continuation' \
        AND json_extract(s.input_json, '$.references.scheduling_activation_id') = ? \
        AND json_extract(s.input_json, '$.due_at') = ? AND json_extract(s.input_json, '$.references.task_id') IS ?")
        .bind(&next.activation_id).bind(&activation.team_id).bind(&activation.actor_id).bind(&next.trigger_id)
        .bind(&activation.id).bind(expected.due_at).bind(&expected.task_id).fetch_optional(db).await?;
    row.map(|row| -> anyhow::Result<_> {
        Ok(ActivationContinuation {
            activation_id: row.try_get("id")?,
            trigger_id: next.trigger_id,
            state: row.try_get::<&str, _>("state")?.parse()?,
            due_at: row.try_get("due_at")?,
            revoked: row.try_get("revoked")?,
        })
    })
    .transpose()
}

pub(super) fn verdict(trace: &ActivationTrace) -> AgentTraceVerdict {
    use AgentTraceStallLayer as Layer;
    use LoopActivationState as State;
    let activation = &trace.activation;
    let (layer, reason) = if trace.lease.as_ref().is_some_and(|lease| lease.expired) {
        (
            Layer::LeaseExpiredUnfenced,
            "execution lease expired without verified cleanup; process ownership remains unresolved",
        )
    } else if activation
        .outcome
        .as_ref()
        .is_some_and(|outcome| outcome.continuation.is_some())
        && trace.continuation.is_none()
    {
        (
            Layer::ContinuationMissing,
            "the recorded outcome requests a continuation but its durable receipt/source linkage is missing",
        )
    } else if activation.state == State::Pending {
        match trace.metrics.policy_state {
            Some(LoopPolicyState::Suspended) => (
                Layer::LoopSuspended,
                "pending work is retained under a suspended policy",
            ),
            Some(LoopPolicyState::Disabled) => (
                Layer::LoopDisabled,
                "pending work has no enabled admission policy",
            ),
            _ if activation.due_at.max(activation.next_admission_at) > trace.observed_at => (
                Layer::LoopScheduled,
                "work is pending until its due time or admission backoff expires",
            ),
            _ => (
                Layer::PendingNotAdmitted,
                "due work is pending scheduler admission; inspect policy, budgets, and deferral events",
            ),
        }
    } else if trace.lease.is_some()
        && trace.oldest_unsettled_tool_id.is_some()
        && matches!(activation.state, State::Starting | State::Running)
    {
        (
            Layer::ToolBoundaryStall,
            "a tool boundary has no recorded completion for at least 60 seconds; no success or external effect is inferred",
        )
    } else if activation.state == State::Canceled {
        (
            Layer::LoopCanceled,
            "activation was canceled; retained history does not grant execution authority",
        )
    } else if let Some(outcome) = &activation.outcome {
        match outcome.wait_reason {
            Some(LoopWaitReason::Dependency) => (
                Layer::WaitingDependency,
                "the activation recorded a dependency wait; inspect active conditions and addressed work",
            ),
            Some(_) => (
                Layer::LoopWaiting,
                "the activation recorded a business wait; process absence is expected after cleanup",
            ),
            None if activation.state == State::Finalizing => (
                Layer::LoopFinalizing,
                "outcome is durable and executor cleanup is still pending",
            ),
            None => (
                Layer::LoopCompleted,
                "the activation recorded its outcome; task acceptance remains a separate authority",
            ),
        }
    } else {
        match activation.state {
            State::Interrupted => (
                Layer::LoopInterrupted,
                "execution was interrupted without a structured outcome; retained reservations and tool observations may remain uncertain",
            ),
            State::Finalizing => (Layer::LoopFinalizing, "executor finalization is pending"),
            _ => (
                Layer::LoopRunning,
                "durable execution is admitted; provider liveness requires matching live-session evidence",
            ),
        }
    };
    AgentTraceVerdict {
        layer,
        reason: reason.into(),
    }
}

pub(super) fn render(trace: &ActivationTrace) -> Vec<String> {
    let activation = &trace.activation;
    let mut lines = vec![
        format!("loop.activation_id: {}", activation.id),
        format!(
            "loop.state: {} (generation={})",
            activation.state.as_str(),
            activation.generation
        ),
        format!(
            "loop.policy: {}",
            trace
                .metrics
                .policy_state
                .map(|state| state.as_str())
                .unwrap_or("<unknown>")
        ),
        format!(
            "loop.mailbox_run_id: {}",
            activation.mailbox_run_id.as_deref().unwrap_or("<none>")
        ),
        format!(
            "loop.sources: {} (next_cursor={})",
            trace.sources.sources.len(),
            trace.sources.next_cursor.as_deref().unwrap_or("<none>")
        ),
    ];
    if let Some(launch) = &activation.launch {
        lines.push(format!(
            "loop.launch: provider={} prompt_version={} configuration={} workspace={}",
            launch.provider_id,
            launch.entry_prompt_version,
            launch.configuration_digest,
            launch.workspace
        ));
    }
    for source in &trace.sources.sources {
        lines.push(format!(
            "loop.source: {} kind={} task={} revoked={}",
            source.id,
            source.kind.as_str(),
            source.references.task_id.as_deref().unwrap_or("<none>"),
            source.revoked
        ));
    }
    for event in &trace.events.events {
        lines.push(format!(
            "loop.event: {} kind={} generation={} ts={}",
            event.id,
            event.kind.as_str(),
            event.generation,
            event.created_at
        ));
    }
    lines.push(format!(
        "loop.events.next_cursor: {:?}",
        trace.events.next_cursor
    ));
    if let Some(event) = &trace.latest_event {
        lines.push(format!(
            "loop.latest_event: {} kind={} exit_reason={}",
            event.id,
            event.kind.as_str(),
            event
                .exit_reason
                .map(|reason| reason.as_str())
                .unwrap_or("<unknown>")
        ));
    }
    for tool in &trace.tools.tools {
        lines.push(format!(
            "loop.tool: {} surface={} name={} target={} status={} duration_ms={:?}",
            tool.id,
            tool.surface.as_str(),
            tool.tool_name,
            tool.target_ref.as_deref().unwrap_or("<none>"),
            tool.status.as_str(),
            tool.duration_ms
        ));
    }
    lines.push(format!(
        "loop.tools.next_cursor: {:?}",
        trace.tools.next_cursor
    ));
    if let Some(lease) = &trace.lease {
        lines.push(format!(
            "loop.lease: generation={} expires_at={} expired={}",
            lease.generation, lease.expires_at, lease.expired
        ));
    }
    if let Some(outcome) = &activation.outcome {
        lines.push(format!(
            "loop.outcome: {} note_id={:?} wait_reason={}",
            outcome.kind.as_str(),
            outcome.task_note_id,
            outcome
                .wait_reason
                .map(|reason| reason.as_str())
                .unwrap_or("<none>")
        ));
    }
    if let Some(next) = &trace.continuation {
        lines.push(format!(
            "loop.continuation: activation={} source={} state={} due_at={} revoked={}",
            next.activation_id,
            next.trigger_id,
            next.state.as_str(),
            next.due_at,
            next.revoked
        ));
    }
    if let Some(next) = &trace.actor_next_wake {
        lines.push(format!(
            "loop.actor_next_wake: activation={} due_at={} next_admission_at={}",
            next.activation_id, next.due_at, next.next_admission_at
        ));
    }
    for schedule in &trace.active_schedules {
        // LoopSchedule is a typed reference-only projection, never the private registration input.
        lines.push(format!(
            "loop.schedule: {} condition={:?} next_due_at={:?}",
            schedule.id, schedule.schedule, schedule.next_due_at
        ));
    }
    lines.push(format!(
        "loop.schedules.next_cursor: {:?}",
        trace.next_schedule_cursor
    ));
    lines.push(format!(
        "loop.no_progress: {}/{} finalized activations",
        trace.metrics.progress.no_progress_activations,
        trace.metrics.progress.finalized_activations
    ));
    lines.push(format!(
        "loop.mem.latest: {}",
        trace
            .metrics
            .mem
            .latest
            .as_ref()
            .map(|entry| entry.kind.as_str())
            .unwrap_or("<not observed>")
    ));
    lines
}

#[cfg(test)]
mod tests;
