use std::collections::BTreeSet;

use agenthub_team_actor::{ActorIdentityKind, ActorSendResponse};
use serde_json::Value;

use super::prompts::build_actor_mailbox_immediate_hint_prompt;
use super::types::{
    ActorMailboxImmediateHintDelivery, ActorMailboxImmediateHintPlan,
    ActorMailboxImmediateHintReason, ActorMailboxPriorityClass, TeamMailboxHintAgentNudger,
};
use crate::team::TeamManager;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActorMailboxPriorityPlan {
    priority_class: ActorMailboxPriorityClass,
    immediate_hint: Option<ActorMailboxImmediateHintPlan>,
}

async fn resolve_actor_mailbox_priority_plan(
    manager: &TeamManager,
    run_id: &str,
    send_result: &ActorSendResponse,
) -> anyhow::Result<ActorMailboxPriorityPlan> {
    let message = &send_result.message;
    if message.to_actor_kind != ActorIdentityKind::Agent {
        return Ok(ActorMailboxPriorityPlan {
            priority_class: ActorMailboxPriorityClass::General,
            immediate_hint: None,
        });
    }
    if is_channel_payload(&message.payload) {
        let Some(role) = manager
            .member_role_for_run(run_id, &message.from_actor_id)
            .await?
        else {
            return Ok(ActorMailboxPriorityPlan {
                priority_class: ActorMailboxPriorityClass::General,
                immediate_hint: None,
            });
        };
        if !role.eq_ignore_ascii_case("coordinator") {
            return Ok(ActorMailboxPriorityPlan {
                priority_class: ActorMailboxPriorityClass::General,
                immediate_hint: None,
            });
        }
        let mention_targets =
            collect_channel_mention_actor_ids(&message.payload, &message.from_actor_id);
        return Ok(if mention_targets.is_empty() {
            ActorMailboxPriorityPlan {
                priority_class: ActorMailboxPriorityClass::General,
                immediate_hint: None,
            }
        } else {
            ActorMailboxPriorityPlan {
                priority_class: ActorMailboxPriorityClass::Urgent,
                immediate_hint: Some(ActorMailboxImmediateHintPlan {
                    target_actor_ids: mention_targets,
                    reason: ActorMailboxImmediateHintReason::CoordinatorChannelMention,
                }),
            }
        });
    }
    if message.from_actor_kind == ActorIdentityKind::Agent {
        return Ok(ActorMailboxPriorityPlan {
            priority_class: ActorMailboxPriorityClass::Urgent,
            immediate_hint: Some(ActorMailboxImmediateHintPlan {
                target_actor_ids: vec![message.to_actor_id.clone()],
                reason: ActorMailboxImmediateHintReason::DirectAgentMessage,
            }),
        });
    }
    Ok(ActorMailboxPriorityPlan {
        priority_class: ActorMailboxPriorityClass::General,
        immediate_hint: None,
    })
}

pub(crate) async fn plan_actor_mailbox_immediate_hint(
    manager: &TeamManager,
    run_id: &str,
    send_result: &ActorSendResponse,
) -> anyhow::Result<Option<ActorMailboxImmediateHintPlan>> {
    if send_result.deduped {
        return Ok(None);
    }
    Ok(
        resolve_actor_mailbox_priority_plan(manager, run_id, send_result)
            .await?
            .immediate_hint,
    )
}

pub(crate) async fn dispatch_actor_mailbox_immediate_hint(
    nudger: &dyn TeamMailboxHintAgentNudger,
    run_id: &str,
    plan: &ActorMailboxImmediateHintPlan,
) -> ActorMailboxImmediateHintDelivery {
    let prompt = build_actor_mailbox_immediate_hint_prompt(run_id, plan.reason);
    let mut sent_actor_ids = Vec::new();
    let mut failed_actor_ids = Vec::new();
    for target_actor_id in &plan.target_actor_ids {
        match nudger
            .nudge_mailbox_prompt(target_actor_id, None, prompt.as_str())
            .await
        {
            Ok(()) => sent_actor_ids.push(target_actor_id.clone()),
            Err(err) => {
                tracing::debug!(
                    run_id = %run_id,
                    actor_id = %target_actor_id,
                    reason = ?plan.reason,
                    "skip mailbox hint push because agent input is unavailable: {}",
                    err
                );
                failed_actor_ids.push(target_actor_id.clone());
            }
        }
    }
    ActorMailboxImmediateHintDelivery {
        sent_actor_ids,
        failed_actor_ids,
    }
}

fn is_channel_payload(payload: &Value) -> bool {
    payload
        .as_object()
        .and_then(|obj| obj.get("channel_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

pub(crate) fn collect_channel_mention_actor_ids(
    payload: &Value,
    from_actor_id: &str,
) -> Vec<String> {
    payload
        .as_object()
        .and_then(|obj| {
            obj.get("mentioned_actor_ids")
                .or_else(|| obj.get("mention_actor_ids"))
        })
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|actor_id| !actor_id.is_empty() && *actor_id != from_actor_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(str::to_string)
        .collect()
}
