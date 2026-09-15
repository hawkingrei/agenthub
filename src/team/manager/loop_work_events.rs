use std::collections::{BTreeMap, HashSet};

use agenthub_agent_domain::loop_runtime::{
    LoopSourceReferences, LoopTriggerInput, LoopTriggerKind,
};
use agenthub_db::loop_runtime::LoopStore;
use agenthub_team_actor::{ACTOR_MAIN_PEER_ID, SendActorMessageCommand};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{Sqlite, Transaction};

use super::TeamManager;
use crate::team::mentions::extract_task_message_mention_actor_ids;
use crate::team::{TeamConversationMessageRecord, TeamConversationRecord};

impl TeamManager {
    pub(crate) async fn request_loop_activation(
        &self,
        team_id: &str,
        member_id: &str,
        source_key: &str,
        task_id: Option<&str>,
    ) -> anyhow::Result<agenthub_agent_domain::loop_runtime::LoopTriggerReceipt> {
        agenthub_agent_domain::loop_runtime::validate_loop_id(source_key)?;
        let context = crate::team::loop_context::scheduling_context();
        let kind = match (&context.actor_id, &context.user_id) {
            (Some(_), None) => LoopTriggerKind::MemberRequest,
            (None, Some(_)) => LoopTriggerKind::Operator,
            _ => anyhow::bail!("an authenticated scheduling identity is required"),
        };
        let digest = Sha256::digest(serde_json::to_vec(&(
            &context.actor_id,
            &context.user_id,
            source_key,
        ))?);
        let key = format!("{}:{}", kind.as_str(), super::hex_encode(&digest));
        let mut input = LoopTriggerInput {
            actor_id: member_id.into(),
            team_id: team_id.into(),
            kind,
            source_key: key,
            due_at: None,
            references: LoopSourceReferences {
                task_id: task_id.map(str::to_owned),
                scheduling_actor_id: context.actor_id,
                scheduling_activation_id: context.activation_id,
                scheduling_user_id: context.user_id,
                ..Default::default()
            },
        };
        let mut tx = self.db.begin_with("BEGIN IMMEDIATE").await?;
        anyhow::ensure!(
            loop_spec(&mut tx, team_id).await?.is_some(),
            agenthub_db::loop_runtime::LoopStoreError::ScopeMismatch
        );
        let previous: Option<String> = sqlx::query_scalar("SELECT input_json FROM loop_trigger_sources WHERE actor_id = ? AND team_id = ? AND source_kind = ? AND source_key = ?")
            .bind(member_id).bind(team_id).bind(kind.as_str()).bind(&input.source_key).fetch_optional(&mut *tx).await?;
        if let Some(previous) = previous {
            let previous: LoopTriggerInput = serde_json::from_str(&previous)?;
            // Retry a business request across activations without rewriting its original provenance.
            input.references.scheduling_activation_id =
                previous.references.scheduling_activation_id;
        }
        let receipt =
            LoopStore::accept_in_transaction(&mut tx, &input, chrono::Utc::now().timestamp())
                .await?;
        tx.commit().await?;
        Ok(receipt)
    }
}

async fn loop_spec(
    tx: &mut Transaction<'_, Sqlite>,
    team_id: &str,
) -> anyhow::Result<Option<Value>> {
    let raw: String = sqlx::query_scalar("SELECT spec_json FROM team_definitions WHERE id = ?")
        .bind(team_id)
        .fetch_one(&mut **tx)
        .await?;
    let spec: Value = serde_json::from_str(&raw)?;
    Ok(TeamManager::uses_loop_execution(&spec).then_some(spec))
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

async fn accept_target(
    tx: &mut Transaction<'_, Sqlite>,
    mut input: LoopTriggerInput,
    canonical_author: Option<&str>,
    now: i64,
) -> anyhow::Result<()> {
    let enabled: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loop_policies WHERE team_id = ? AND actor_id = ? AND state IN ('enabled', 'suspended'))")
        .bind(&input.team_id).bind(&input.actor_id).fetch_one(&mut **tx).await?;
    if !enabled {
        return Ok(());
    }
    let context = crate::team::loop_context::scheduling_context();
    let actor = context.actor_id.as_deref().or(canonical_author);
    if let Some(actor) = actor {
        let member: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM team_definitions t, json_each(t.spec_json, '$.members') m WHERE t.id = ? AND json_extract(m.value, '$.member_id') = ?)")
            .bind(&input.team_id).bind(actor).fetch_one(&mut **tx).await?;
        if member {
            input.references.scheduling_actor_id = Some(actor.to_owned());
        }
    }
    if context.actor_id == input.references.scheduling_actor_id {
        input.references.scheduling_activation_id = context.activation_id;
    }
    input.references.scheduling_user_id = context.user_id;
    LoopStore::accept_in_transaction(tx, &input, now).await?;
    Ok(())
}

/// Canonical channel/thread work is accepted before post-commit delivery replicas can be lost.
pub(super) async fn stage_conversation_event(
    tx: &mut Transaction<'_, Sqlite>,
    conversation: &TeamConversationRecord,
    message: &TeamConversationMessageRecord,
    body_store: Option<&dyn agenthub_message_store::MessageBodyStore>,
) -> anyhow::Result<()> {
    let Some(spec) = loop_spec(tx, &conversation.team_id).await? else {
        return Ok(());
    };
    let members = members(&spec);
    let ids: HashSet<String> = members.keys().map(|id| (*id).to_owned()).collect();
    let mut targets = BTreeMap::new();
    if let Some(actor) = message
        .to_actor_id
        .as_deref()
        .filter(|actor| ids.contains(*actor))
    {
        targets.insert(actor.to_owned(), "addressed");
    }
    for actor in extract_task_message_mention_actor_ids(&message.payload, &ids) {
        targets.entry(actor).or_insert("mention");
    }
    let thread_id = message
        .payload
        .get("thread_root_message_id")
        .and_then(Value::as_i64);
    if let Some(root) = thread_id {
        let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM team_conversation_messages WHERE id = ? AND conversation_id = ? AND id < ?)")
            .bind(root).bind(&conversation.id).bind(message.message_id).fetch_one(&mut **tx).await?;
        anyhow::ensure!(
            valid,
            agenthub_db::loop_runtime::LoopStoreError::ScopeMismatch
        );
        let participants =
            thread_participants(tx, conversation, root, message.message_id, &ids, body_store)
                .await?;
        for actor in participants {
            if members.get(actor.as_str()).is_some_and(|member| {
                member
                    .get("loop_intake")
                    .and_then(|policy| policy.get("engaged_thread_replies"))
                    .and_then(Value::as_bool)
                    .unwrap_or(true)
            }) {
                targets.entry(actor).or_insert("thread_reply");
            }
        }
    }
    targets.remove(&message.from_actor_id);
    for (actor, reason) in targets {
        accept_target(
            tx,
            LoopTriggerInput {
                actor_id: actor,
                team_id: conversation.team_id.clone(),
                kind: LoopTriggerKind::Message,
                source_key: format!("conversation:{}:{reason}", message.message_id),
                due_at: None,
                references: LoopSourceReferences {
                    task_id: Some(conversation.task_id.clone()),
                    conversation_message_id: Some(message.message_id),
                    thread_id,
                    ..Default::default()
                },
            },
            Some(&message.from_actor_id),
            message.created_at,
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn stage_mailbox_event(
    tx: &mut Transaction<'_, Sqlite>,
    message: &SendActorMessageCommand,
    message_id: i64,
) -> anyhow::Result<()> {
    if message.to_peer_id != ACTOR_MAIN_PEER_ID {
        return Ok(());
    }
    let team_id: String = sqlx::query_scalar("SELECT team_id FROM team_runs WHERE id = ?")
        .bind(&message.run_id)
        .fetch_one(&mut **tx)
        .await?;
    let Some(spec) = loop_spec(tx, &team_id).await? else {
        return Ok(());
    };
    let replica = if message
        .payload
        .get("delivery_scope")
        .and_then(Value::as_str)
        == Some("channel_broadcast")
    {
        Some((
            message
                .payload
                .get("authority_message_id")
                .and_then(Value::as_i64),
            message
                .payload
                .get("channel_conversation_id")
                .and_then(Value::as_str),
        ))
    } else if message.payload.get("task_message_id").is_some()
        && message.payload.get("task_conversation_id").is_some()
    {
        Some((
            message
                .payload
                .get("task_message_id")
                .and_then(Value::as_i64),
            message
                .payload
                .get("task_conversation_id")
                .and_then(Value::as_str),
        ))
    } else {
        None
    };
    if let Some((source_id, conversation_id)) = replica {
        let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM team_conversation_messages m JOIN team_conversations c ON c.id = m.conversation_id WHERE m.id = ? AND m.conversation_id = ? AND c.team_id = ?)")
            .bind(source_id).bind(conversation_id).bind(&team_id).fetch_one(&mut **tx).await?;
        anyhow::ensure!(
            valid,
            agenthub_db::loop_runtime::LoopStoreError::ScopeMismatch
        );
        // Recovery reads the committed canonical source even when these delivery copies lag.
        return Ok(());
    }
    if !members(&spec).contains_key(message.to_actor_id.as_str())
        || message.to_actor_id == message.from_actor_id
    {
        return Ok(());
    }
    accept_target(
        tx,
        LoopTriggerInput {
            actor_id: message.to_actor_id.clone(),
            team_id,
            kind: LoopTriggerKind::Message,
            source_key: format!("mailbox:{message_id}:addressed"),
            due_at: None,
            references: LoopSourceReferences {
                mailbox_message_id: Some(message_id),
                ..Default::default()
            },
        },
        Some(&message.from_actor_id),
        message.created_at,
    )
    .await
}

pub(super) async fn stage_assignment_event(
    tx: &mut Transaction<'_, Sqlite>,
    task_id: &str,
    canonical_author: Option<&str>,
) -> anyhow::Result<()> {
    let (team_id, actor, revision, status): (String, Option<String>, i64, String) = sqlx::query_as(
        "SELECT team_id, assigned_member_id, updated_at, status FROM team_tasks WHERE id = ?",
    )
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await?;
    let Some(actor) = actor else {
        return Ok(());
    };
    if matches!(status.as_str(), "completed" | "canceled")
        || loop_spec(tx, &team_id).await?.is_none()
    {
        return Ok(());
    }
    accept_target(
        tx,
        LoopTriggerInput {
            actor_id: actor,
            team_id,
            kind: LoopTriggerKind::Assignment,
            source_key: format!("task:{task_id}:assignment:{revision}"),
            due_at: None,
            references: LoopSourceReferences {
                task_id: Some(task_id.into()),
                ..Default::default()
            },
        },
        canonical_author,
        chrono::Utc::now().timestamp(),
    )
    .await
}

async fn thread_participants(
    tx: &mut Transaction<'_, Sqlite>,
    conversation: &TeamConversationRecord,
    root: i64,
    before: i64,
    members: &HashSet<String>,
    body_store: Option<&dyn agenthub_message_store::MessageBodyStore>,
) -> anyhow::Result<HashSet<String>> {
    let mut participants = HashSet::new();
    let mut after = 0;
    loop {
        let rows = sqlx::query("SELECT * FROM team_conversation_messages WHERE conversation_id = ? AND (id = ? OR thread_root_message_id = ?) AND id > ? AND id < ? ORDER BY id LIMIT 64")
            .bind(&conversation.id).bind(root).bind(root).bind(after).bind(before).fetch_all(&mut **tx).await?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            let (mut message, moved) =
                super::codec_rows::parse_team_conversation_message_row(&row)?;
            after = message.message_id;
            if moved {
                super::conversation_body::rehydrate_conversation_body_in_tx(
                    tx,
                    body_store,
                    &mut message,
                )
                .await?;
            }
            if members.contains(&message.from_actor_id) {
                participants.insert(message.from_actor_id);
            }
            participants.extend(extract_task_message_mention_actor_ids(
                &message.payload,
                members,
            ));
        }
        if participants.len() == members.len() {
            break;
        }
    }
    Ok(participants)
}
