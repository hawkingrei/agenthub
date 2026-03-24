use super::help::{actor_topic_usage, actor_usage};
use super::output::{actor_output_preference_for_command, write_actor_output};
use super::parse::{compute_time_trigger_fire_at, resolve_team_leader_member_id};
use super::runtime::{
    init_actor_mailbox_service, init_actor_permission_review_client, init_team_manager,
    load_actor_inbox, map_actor_service_error,
    maybe_notify_actor_new_mailbox_message_type_from_cli,
};
use super::{
    ActorCommand, ActorOutputMode, ActorSendPayloadSource, MAX_TIME_TRIGGER_DELAY_SECONDS,
    TEAM_SHARED_THREAD_BOOTSTRAP_KIND, TEAM_SHARED_THREAD_TITLE,
};
use agent_client_protocol::{RequestPermissionOutcome, SelectedPermissionOutcome};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, ActorAckRequest, ActorInboxRequest, ActorMessageStatus,
};
use anyhow::Context;
use chrono::Utc;
use serde_json::Value;

use crate::acp::{AcpPermissionRespondResult, AcpPermissionService};
use crate::agent::{AgentTimeTriggerCreateInput, AgentTimeTriggerManager};
use crate::team::{TeamActorMessageTransport, TeamManager, TeamTaskStatus};

async fn load_team_for_context(
    manager: &TeamManager,
    team_id: &str,
    actor_id: &str,
) -> anyhow::Result<crate::team::TeamDefinitionRecord> {
    let team = manager
        .get_team(team_id)
        .await
        .with_context(|| format!("load team failed: {team_id}"))?;
    let is_member = manager
        .team_has_member(&team.id, actor_id)
        .await
        .context("load team members failed")?;
    anyhow::ensure!(is_member, "current actor is not a member of this team");
    Ok(team)
}

async fn ensure_leader_team_access(
    manager: &TeamManager,
    team_id: &str,
    actor_id: &str,
) -> anyhow::Result<crate::team::TeamDefinitionRecord> {
    let team = load_team_for_context(manager, team_id, actor_id).await?;
    let leader_member_id = resolve_team_leader_member_id(&team.spec)?;
    anyhow::ensure!(
        actor_id == leader_member_id,
        "only leader may create or update Team tasks"
    );
    Ok(team)
}

fn is_shared_thread_task(task: &crate::team::TeamTaskRecord) -> bool {
    if task
        .title
        .trim()
        .eq_ignore_ascii_case(TEAM_SHARED_THREAD_TITLE)
    {
        return true;
    }
    task.context
        .as_object()
        .and_then(|obj| obj.get("bootstrap_kind"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| value.eq_ignore_ascii_case(TEAM_SHARED_THREAD_BOOTSTRAP_KIND))
}

pub(super) async fn run_actor_command(
    command: ActorCommand,
    output_mode: ActorOutputMode,
) -> anyhow::Result<()> {
    let output_preference = actor_output_preference_for_command(&command);
    match command {
        ActorCommand::Help { topic } => {
            let help = match topic {
                Some(topic) => actor_topic_usage(topic),
                None => actor_usage(),
            };
            println!("{help}");
        }
        ActorCommand::TeamMembers { team_id, run_id } => {
            let db = agenthub_db::init_db().await?;
            let manager = TeamManager::new(db);
            let team_context = manager
                .describe_team_context(team_id.as_deref(), run_id.as_deref())
                .await?;
            write_actor_output(&team_context, output_mode, output_preference)?;
        }
        ActorCommand::TeamTasks {
            team_id,
            actor_id,
            limit,
            status,
            include_shared_thread,
        } => {
            let db = agenthub_db::init_db().await?;
            let manager = TeamManager::new(db);
            let _team = load_team_for_context(&manager, &team_id, &actor_id).await?;
            let mut tasks = manager.list_tasks(&team_id, limit).await?;
            if !include_shared_thread {
                tasks.retain(|task| !is_shared_thread_task(task));
            }
            if let Some(status) = status {
                tasks.retain(|task| task.status == status);
            }
            write_actor_output(&tasks, output_mode, output_preference)?;
        }
        ActorCommand::TeamTaskCreate {
            team_id,
            actor_id,
            title,
            status,
            topic,
            context,
        } => {
            let db = agenthub_db::init_db().await?;
            let manager = TeamManager::new(db);
            let _team = ensure_leader_team_access(&manager, &team_id, &actor_id).await?;
            let (task, conversation) = manager
                .create_task(
                    &team_id,
                    &title,
                    &actor_id,
                    context,
                    "group_chat",
                    topic.as_deref(),
                )
                .await?;
            let task = if status == TeamTaskStatus::Open {
                task
            } else {
                manager.update_task_status(&task.id, status).await?
            };
            let output = serde_json::json!({
                "task": task,
                "conversation": conversation,
            });
            write_actor_output(&output, output_mode, output_preference)?;
        }
        ActorCommand::TeamTaskUpdate {
            team_id,
            actor_id,
            task_id,
            status,
        } => {
            let db = agenthub_db::init_db().await?;
            let manager = TeamManager::new(db);
            let _team = ensure_leader_team_access(&manager, &team_id, &actor_id).await?;
            let existing = manager.get_task(&task_id).await?;
            anyhow::ensure!(
                existing.team_id == team_id,
                "task does not belong to this team"
            );
            let task = manager.update_task_status(&task_id, status).await?;
            write_actor_output(&task, output_mode, output_preference)?;
        }
        ActorCommand::Inbox {
            run_id,
            actor_id,
            limit,
            after_id,
            include_delivered,
            auto_ack,
        } => {
            let (manager, config) = init_team_manager().await?;
            let service = init_actor_mailbox_service(&manager, &config, &actor_id, &run_id).await?;
            let states = if include_delivered {
                Some(vec![
                    ActorMessageStatus::Pending,
                    ActorMessageStatus::Delivered,
                    ActorMessageStatus::DeadLetter,
                ])
            } else {
                Some(vec![ActorMessageStatus::Pending])
            };
            let inbox = load_actor_inbox(
                service.as_ref(),
                ActorInboxRequest {
                    run_id,
                    actor_id,
                    cursor: after_id,
                    limit: Some(limit),
                    states,
                },
                auto_ack,
            )
            .await
            .map_err(|err| map_actor_service_error("actor inbox", err))?;
            write_actor_output(&inbox, output_mode, output_preference)?;
        }
        ActorCommand::Ack {
            run_id,
            actor_id,
            message_id,
        } => {
            let (manager, config) = init_team_manager().await?;
            let service = init_actor_mailbox_service(&manager, &config, &actor_id, &run_id).await?;
            let message = service
                .actor_ack(ActorAckRequest {
                    run_id,
                    actor_id,
                    message_id,
                    ack_token: None,
                    result: None,
                })
                .await
                .map_err(|err| map_actor_service_error("actor ack", err))?;
            write_actor_output(&message, output_mode, output_preference)?;
        }
        ActorCommand::Send {
            run_id,
            from_actor_id,
            to_actor_id,
            channel_id,
            channel,
            transport,
            route,
            payload,
            payload_source,
            idempotency_key,
        } => {
            let (manager, config) = init_team_manager().await?;
            let service =
                init_actor_mailbox_service(&manager, &config, &from_actor_id, &run_id).await?;
            let message = service
                .actor_send(agenthub_team_actor::ActorSendRequest {
                    run_id,
                    from_actor_id,
                    from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
                    to_actor_id,
                    channel_id,
                    to_peer_id: Some(
                        if transport == TeamActorMessageTransport::Remote {
                            ACTOR_NODE_PEER_ID
                        } else {
                            ACTOR_MAIN_PEER_ID
                        }
                        .to_string(),
                    ),
                    channel: Some(channel),
                    transport: Some(transport),
                    route,
                    payload: *payload,
                    idempotency_key,
                })
                .await
                .map_err(|err| map_actor_service_error("actor send", err))?;
            if let Err(err) =
                maybe_notify_actor_new_mailbox_message_type_from_cli(&manager, &config, &message)
                    .await
            {
                tracing::warn!(
                    run_id = %message.message.run_id,
                    message_id = message.message.message_id,
                    "failed to process actor mailbox type hint: {}",
                    err
                );
            }
            if payload_source == ActorSendPayloadSource::Payload {
                eprintln!(
                    "warning: prefer --text for markdown-rich mailbox messages; --payload-json is best reserved for structured machine-readable coordination"
                );
            }
            write_actor_output(&message, output_mode, output_preference)?;
        }
        ActorCommand::TimeTriggerSet {
            actor_id,
            delay_seconds,
            message,
        } => {
            anyhow::ensure!(
                (1..=MAX_TIME_TRIGGER_DELAY_SECONDS).contains(&delay_seconds),
                "delay_seconds must be between 1 and {}",
                MAX_TIME_TRIGGER_DELAY_SECONDS
            );
            let db = agenthub_db::init_db().await?;
            let manager = AgentTimeTriggerManager::new(db);
            let now_ts = Utc::now().timestamp();
            let record = manager
                .create_time_trigger(AgentTimeTriggerCreateInput {
                    agent_id: actor_id.clone(),
                    created_by_actor_id: actor_id,
                    message_text: message,
                    fire_at: compute_time_trigger_fire_at(now_ts, delay_seconds),
                })
                .await?;
            write_actor_output(&record, output_mode, output_preference)?;
        }
        ActorCommand::TimeTriggerList { actor_id, limit } => {
            let db = agenthub_db::init_db().await?;
            let manager = AgentTimeTriggerManager::new(db);
            let records = manager
                .list_triggers_for_agent(actor_id.as_str(), limit)
                .await?;
            write_actor_output(&records, output_mode, output_preference)?;
        }
        ActorCommand::TimeTriggerCancel {
            actor_id,
            trigger_id,
        } => {
            let db = agenthub_db::init_db().await?;
            let manager = AgentTimeTriggerManager::new(db);
            let canceled = manager
                .cancel_trigger(actor_id.as_str(), trigger_id.as_str())
                .await?;
            anyhow::ensure!(canceled, "time trigger not found");
            let output = serde_json::json!({
                "status": "ok",
                "trigger_id": trigger_id,
            });
            write_actor_output(&output, output_mode, output_preference)?;
        }
        ActorCommand::PermissionReviewRespond {
            team_id,
            actor_id,
            permission_id,
            option_id,
            outcome,
        } => {
            let db = agenthub_db::init_db().await?;
            let permissions = AcpPermissionService::new(db.clone());
            let manager = TeamManager::new(db);
            let Some(record) = permissions.get(&permission_id).await? else {
                anyhow::bail!("permission request not found");
            };
            anyhow::ensure!(
                record.team_id.as_deref() == Some(team_id.as_str()),
                "permission request does not belong to this team"
            );
            anyhow::ensure!(
                manager.team_has_member(&team_id, actor_id.as_str()).await?,
                "current actor is not a member of this team"
            );
            let team = manager.get_team(&team_id).await?;
            let leader_member_id = resolve_team_leader_member_id(&team.spec)?;
            anyhow::ensure!(
                record.requester_actor_id.as_deref() != Some(actor_id.as_str()),
                "requester cannot review its own permission request"
            );
            let worker_originated_request = record
                .requester_role
                .as_deref()
                .is_some_and(|role| role.eq_ignore_ascii_case("worker"));
            let active_reviewer =
                record
                    .review_target_actor_id
                    .as_deref()
                    .or(if worker_originated_request {
                        Some(leader_member_id.as_str())
                    } else {
                        None
                    });
            anyhow::ensure!(
                active_reviewer == Some(actor_id.as_str()),
                if worker_originated_request {
                    "leader is the only reviewer for worker-originated permission requests"
                } else {
                    "current actor is not the active reviewer for this permission request"
                }
            );
            if let Some(client) = init_actor_permission_review_client(&actor_id).await? {
                let response = client
                    .respond_permission_review(
                        &team_id,
                        &actor_id,
                        &permission_id,
                        option_id.as_deref(),
                        outcome.as_deref(),
                    )
                    .await?;
                write_actor_output(&response, output_mode, output_preference)?;
                return Ok(());
            }
            if record.status != "pending" {
                let output = serde_json::json!({
                    "status": "already_resolved",
                    "permission_id": permission_id,
                    "request_status": record.status,
                });
                write_actor_output(&output, output_mode, output_preference)?;
                return Ok(());
            }

            let response_outcome = if let Some(option_id) = option_id.as_ref() {
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                    option_id.clone(),
                ))
            } else {
                match outcome.as_deref() {
                    Some("cancelled") | None => RequestPermissionOutcome::Cancelled,
                    Some(other) => {
                        anyhow::bail!("unsupported outcome '{}', expected 'cancelled'", other);
                    }
                }
            };
            let responded = permissions
                .respond(
                    &permission_id,
                    response_outcome,
                    option_id.clone(),
                    Some(actor_id.clone()),
                )
                .await?;
            if matches!(responded, AcpPermissionRespondResult::AlreadyResolved) {
                let request_status = permissions
                    .get(&permission_id)
                    .await?
                    .map(|current| current.status)
                    .unwrap_or_else(|| "resolved".to_string());
                let output = serde_json::json!({
                    "status": "already_resolved",
                    "permission_id": permission_id,
                    "request_status": request_status,
                });
                write_actor_output(&output, output_mode, output_preference)?;
                return Ok(());
            }
            let output = serde_json::json!({
                "status": "ok",
                "permission_id": permission_id,
                "reviewed_by_actor_id": actor_id,
            });
            write_actor_output(&output, output_mode, output_preference)?;
        }
    }
    Ok(())
}
