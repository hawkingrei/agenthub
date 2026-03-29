use super::help::{actor_topic_usage, actor_usage};
use super::output::{actor_output_preference_for_command, write_actor_output};
use super::parse::compute_time_trigger_fire_at;
use super::runtime::{
    init_actor_control_client, init_actor_mailbox_service, init_actor_permission_review_client,
    load_actor_inbox, map_actor_service_error,
};
use super::{
    ActorCommand, ActorOutputMode, ActorSendPayloadSource, MAX_TIME_TRIGGER_DELAY_SECONDS,
};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, ActorAckRequest, ActorAckResponse, ActorInboxRequest,
    ActorMailboxService, ActorMessageStatus,
};
use anyhow::Context;
use chrono::Utc;

use crate::actor_runtime_env::{ACTOR_RUNTIME_TEAM_ID_ENV, normalized_env_var};
use crate::internal::auth::InternalAction;
use crate::internal::client::{InternalGrpcMailboxClient, InternalTeamTaskPatch};
use crate::team::{
    TeamActorMessageTransport, TeamTaskDetailRecord, TeamTaskListQuery, TeamTaskRecord,
    TeamTaskStatus,
};

const TEAM_SHARED_THREAD_TITLE: &str = "all";
const TEAM_SHARED_THREAD_BOOTSTRAP_KIND: &str = "shared_thread";
const TEAM_SHARED_THREAD_LOOKUP_LIMIT: i64 = 500;
const ACTOR_INBOX_RUN_ID_RESOLUTION_HINT: &str =
    "retry with --run-id <run_id> explicitly if team shared-thread inference is unavailable";

fn has_shared_thread_title(task: &TeamTaskRecord) -> bool {
    task.title
        .trim()
        .eq_ignore_ascii_case(TEAM_SHARED_THREAD_TITLE)
}

fn has_shared_thread_bootstrap_kind(task: &TeamTaskRecord) -> bool {
    task.context
        .get("bootstrap_kind")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| {
            value
                .trim()
                .eq_ignore_ascii_case(TEAM_SHARED_THREAD_BOOTSTRAP_KIND)
        })
}

fn shared_thread_sort_key(task: &TeamTaskRecord) -> (i64, i64, &str) {
    (task.updated_at, task.created_at, task.id.as_str())
}

pub(super) fn resolve_shared_thread_task_id(tasks: &[TeamTaskRecord]) -> Option<&str> {
    tasks
        .iter()
        .filter(|task| has_shared_thread_bootstrap_kind(task))
        .max_by_key(|task| shared_thread_sort_key(task))
        .or_else(|| {
            tasks
                .iter()
                .filter(|task| has_shared_thread_title(task))
                .max_by_key(|task| shared_thread_sort_key(task))
        })
        .map(|task| task.id.as_str())
}

pub(super) fn require_shared_thread_task_id<'a>(
    team_id: &str,
    tasks: &'a [TeamTaskRecord],
) -> anyhow::Result<&'a str> {
    resolve_shared_thread_task_id(tasks).ok_or_else(|| {
        anyhow::anyhow!(
            "shared thread is missing for team {}; create/open the shared thread first",
            team_id
        )
    })
}

async fn list_shared_thread_tasks_for_team(
    client: &InternalGrpcMailboxClient,
    actor_id: &str,
    team_id: &str,
) -> anyhow::Result<Vec<TeamTaskRecord>> {
    client
        .list_team_tasks(
            actor_id,
            &TeamTaskListQuery {
                team_id: Some(team_id.to_string()),
                run_id: None,
                limit: TEAM_SHARED_THREAD_LOOKUP_LIMIT,
                status: None,
                task_id: None,
                assigned_member_id: None,
                topic: None,
                include_shared_thread: true,
            },
        )
        .await
}

async fn resolve_shared_thread_detail_for_team(
    client: &InternalGrpcMailboxClient,
    actor_id: &str,
    team_id: &str,
) -> anyhow::Result<TeamTaskDetailRecord> {
    let tasks = list_shared_thread_tasks_for_team(client, actor_id, team_id).await?;
    let task_id = require_shared_thread_task_id(team_id, &tasks)?;
    client
        .get_team_task(actor_id, Some(team_id), None, task_id, 1)
        .await
}

async fn resolve_inbox_run_id(actor_id: &str, run_id: Option<String>) -> anyhow::Result<String> {
    if let Some(run_id) = run_id {
        return Ok(run_id);
    }

    let team_id = normalized_env_var(ACTOR_RUNTIME_TEAM_ID_ENV).ok_or_else(|| {
        anyhow::anyhow!(
            "run_id is required (use --run-id <run_id> or set {ACTOR_RUNTIME_TEAM_ID_ENV} so inbox can resolve the canonical shared-thread mailbox run)"
        )
    })?;
    let client = init_actor_control_client(
        actor_id,
        None,
        &[InternalAction::TeamRead],
        "actor inbox run-id resolution",
    )
    .await?;
    let context = client
        .describe_team_context(Some(team_id.as_str()), None, actor_id)
        .await
        .with_context(|| {
            format!(
                "failed to infer inbox run_id from team scope for team {}; {ACTOR_INBOX_RUN_ID_RESOLUTION_HINT}",
                team_id
            )
        })?;
    let detail = resolve_shared_thread_detail_for_team(&client, actor_id, &context.team_id)
        .await
        .with_context(|| {
            format!(
                "failed to infer inbox run_id from canonical shared thread for team {}; {ACTOR_INBOX_RUN_ID_RESOLUTION_HINT}",
                context.team_id
            )
        })?;
    detail
        .latest_run
        .map(|run| run.id)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "shared-thread mailbox run is missing for team {}; open # all or send one shared-thread message first, or pass --run-id explicitly",
                context.team_id
            )
        })
}

pub(super) async fn ack_actor_messages<S: ActorMailboxService + ?Sized>(
    service: &S,
    run_id: &str,
    actor_id: &str,
    message_ids: &[i64],
) -> anyhow::Result<Vec<ActorAckResponse>> {
    let mut responses = Vec::with_capacity(message_ids.len());
    for &message_id in message_ids {
        let response = service
            .actor_ack(ActorAckRequest {
                run_id: run_id.to_string(),
                actor_id: actor_id.to_string(),
                message_id,
                ack_token: None,
                result: None,
            })
            .await
            .map_err(|err| map_actor_service_error("actor ack", err))
            .with_context(|| format!("failed to ack message_id={message_id}"))?;
        responses.push(response);
    }
    Ok(responses)
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
        ActorCommand::TeamMembers {
            team_id,
            run_id,
            actor_id,
        } => {
            let client = init_actor_control_client(
                &actor_id,
                run_id.as_deref(),
                &[InternalAction::TeamRead],
                "actor team context control",
            )
            .await?;
            let team_context = client
                .describe_team_context(team_id.as_deref(), run_id.as_deref(), &actor_id)
                .await?;
            write_actor_output(&team_context, output_mode, output_preference)?;
        }
        ActorCommand::TeamTasks { query, actor_id } => {
            let client = init_actor_control_client(
                &actor_id,
                query.run_id.as_deref(),
                &[InternalAction::TeamRead],
                "actor team task control",
            )
            .await?;
            let tasks = client.list_team_tasks(&actor_id, &query).await?;
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
            let client = init_actor_control_client(
                &actor_id,
                None,
                &[InternalAction::TeamTaskWrite],
                "actor team task control",
            )
            .await?;
            let output = client
                .create_team_task(
                    &team_id,
                    &actor_id,
                    &title,
                    status.as_str(),
                    topic.as_deref(),
                    &context,
                )
                .await?;
            write_actor_output(&output, output_mode, output_preference)?;
        }
        ActorCommand::TeamTaskShow {
            team_id,
            run_id,
            actor_id,
            task_id,
            message_limit,
        } => {
            let client = init_actor_control_client(
                &actor_id,
                run_id.as_deref(),
                &[InternalAction::TeamRead],
                "actor team task control",
            )
            .await?;
            let detail = client
                .get_team_task(
                    &actor_id,
                    team_id.as_deref(),
                    run_id.as_deref(),
                    &task_id,
                    message_limit,
                )
                .await?;
            write_actor_output(&detail, output_mode, output_preference)?;
        }
        ActorCommand::TeamTaskUpdate {
            team_id,
            actor_id,
            task_ids,
            status,
            assigned_member_id,
            clear_assigned_member_id,
            context,
            context_merge,
        } => {
            let client = init_actor_control_client(
                &actor_id,
                None,
                &[InternalAction::TeamTaskWrite],
                "actor team task control",
            )
            .await?;
            if status.is_none()
                && assigned_member_id.is_none()
                && !clear_assigned_member_id
                && context.is_none()
                && context_merge.is_none()
            {
                return Err(anyhow::anyhow!(
                    "team-task-update requires --status, --assigned-member-id, --unassign, --context-json, or --context-merge-json"
                ));
            }
            let mut tasks = Vec::with_capacity(task_ids.len());
            for task_id in task_ids {
                let task = client
                    .update_team_task(
                        &team_id,
                        &actor_id,
                        &task_id,
                        InternalTeamTaskPatch {
                            status: status.as_ref().map(TeamTaskStatus::as_str),
                            assigned_member_id: assigned_member_id.as_deref(),
                            clear_assigned_member_id,
                            context_json: context.as_ref(),
                            context_merge_json: context_merge.as_ref(),
                        },
                    )
                    .await?;
                tasks.push(task);
            }
            if tasks.len() == 1 {
                let task = tasks.pop().expect("single task update result");
                write_actor_output(&task, output_mode, output_preference)?;
            } else {
                write_actor_output(&tasks, output_mode, output_preference)?;
            }
        }
        ActorCommand::TeamTaskNote {
            team_id,
            run_id,
            actor_id,
            task_id,
            shared_thread,
            kind,
            text,
        } => {
            let client = init_actor_control_client(
                &actor_id,
                run_id.as_deref(),
                &[InternalAction::TeamRead, InternalAction::TeamTaskWrite],
                "actor team task control",
            )
            .await?;
            let resolved_task_id = if shared_thread {
                let context = client
                    .describe_team_context(team_id.as_deref(), run_id.as_deref(), &actor_id)
                    .await?;
                let tasks =
                    list_shared_thread_tasks_for_team(&client, &actor_id, &context.team_id).await?;
                require_shared_thread_task_id(&context.team_id, &tasks)?.to_string()
            } else {
                task_id
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("task_id is required"))?
            };
            let message = client
                .append_team_task_note(
                    &actor_id,
                    team_id.as_deref(),
                    run_id.as_deref(),
                    &resolved_task_id,
                    kind.as_str(),
                    &text,
                )
                .await?;
            write_actor_output(&message, output_mode, output_preference)?;
        }
        ActorCommand::Inbox {
            run_id,
            actor_id,
            limit,
            after_id,
            include_delivered,
            auto_ack,
        } => {
            let run_id = resolve_inbox_run_id(&actor_id, run_id).await?;
            let service = init_actor_mailbox_service(&actor_id, &run_id).await?;
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
            message_ids,
        } => {
            let service = init_actor_mailbox_service(&actor_id, &run_id).await?;
            let messages =
                ack_actor_messages(service.as_ref(), &run_id, &actor_id, &message_ids).await?;
            if messages.len() == 1 {
                let message = messages
                    .into_iter()
                    .next()
                    .expect("single ack response should be present");
                write_actor_output(&message, output_mode, output_preference)?;
            } else {
                write_actor_output(&messages, output_mode, output_preference)?;
            }
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
            let service = init_actor_mailbox_service(&from_actor_id, &run_id).await?;
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
            if payload_source == ActorSendPayloadSource::Payload {
                eprintln!(
                    "warning: prefer --text or --text-file for markdown-rich mailbox messages; --payload-json and --payload-file are best reserved for structured machine-readable coordination"
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
            let client = init_actor_control_client(
                &actor_id,
                None,
                &[InternalAction::TimeTriggerManage],
                "actor time trigger control",
            )
            .await?;
            let record = client
                .create_time_trigger(
                    &actor_id,
                    &message,
                    compute_time_trigger_fire_at(Utc::now().timestamp(), delay_seconds),
                )
                .await?;
            write_actor_output(&record, output_mode, output_preference)?;
        }
        ActorCommand::TimeTriggerList { actor_id, limit } => {
            let client = init_actor_control_client(
                &actor_id,
                None,
                &[InternalAction::TimeTriggerManage],
                "actor time trigger control",
            )
            .await?;
            let records = client.list_time_triggers(actor_id.as_str(), limit).await?;
            write_actor_output(&records, output_mode, output_preference)?;
        }
        ActorCommand::TimeTriggerCancel {
            actor_id,
            trigger_id,
        } => {
            let client = init_actor_control_client(
                &actor_id,
                None,
                &[InternalAction::TimeTriggerManage],
                "actor time trigger control",
            )
            .await?;
            let output = client
                .cancel_time_trigger(actor_id.as_str(), trigger_id.as_str())
                .await?;
            write_actor_output(&output, output_mode, output_preference)?;
        }
        ActorCommand::PermissionReviewRespond {
            team_id,
            actor_id,
            permission_ids,
            option_id,
            outcome,
        } => {
            let client = init_actor_permission_review_client(&actor_id).await?;
            if permission_ids.len() == 1 {
                let response = client
                    .respond_permission_review(
                        &team_id,
                        &actor_id,
                        &permission_ids[0],
                        option_id.as_deref(),
                        outcome.as_deref(),
                    )
                    .await?;
                write_actor_output(&response, output_mode, output_preference)?;
            } else {
                let mut responses = Vec::with_capacity(permission_ids.len());
                for permission_id in permission_ids {
                    let response = client
                        .respond_permission_review(
                            &team_id,
                            &actor_id,
                            &permission_id,
                            option_id.as_deref(),
                            outcome.as_deref(),
                        )
                        .await
                        .with_context(|| {
                            format!(
                                "failed to respond permission review for permission_id={permission_id}"
                            )
                        })?;
                    responses.push(response);
                }
                write_actor_output(&responses, output_mode, output_preference)?;
            }
        }
    }
    Ok(())
}
