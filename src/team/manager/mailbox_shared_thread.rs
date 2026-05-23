use sqlx::Sqlite;
use uuid::Uuid;

use super::mailbox::{SharedThreadTarget, resolve_team_id_for_run};
use super::mailbox_payloads::{
    build_canonical_chat_payload, channel_payload_correlation_id, resolve_canonical_chat_reply,
    should_persist_human_visible_chat_reply_for_payload,
};
use super::payload_utils::redact_sensitive_json;
use super::{
    TEAM_SHARED_THREAD_BOOTSTRAP_KIND, TEAM_SHARED_THREAD_TITLE,
    fetch_canonical_shared_thread_target,
};
use agenthub_team_actor::SendActorMessageCommand;

const TEAM_SHARED_THREAD_BOOTSTRAP_SOURCE: &str = "server_canonical_reply";

pub(super) async fn maybe_persist_human_visible_chat_reply(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    cmd: &SendActorMessageCommand,
) -> Result<(), sqlx::Error> {
    if !should_persist_human_visible_chat_reply(cmd) {
        return Ok(());
    }
    let Some(reply) = resolve_canonical_chat_reply(&cmd.payload) else {
        return Ok(());
    };
    let shared_thread = resolve_or_create_shared_thread_for_run(tx, &cmd.run_id, cmd).await?;
    let payload = build_canonical_chat_payload(&reply);
    let payload_json = redact_sensitive_json(&payload).to_string();
    let correlation_id = channel_payload_correlation_id(&payload).unwrap_or_default();
    sqlx::query(
        r#"
        INSERT INTO team_conversation_messages (
            conversation_id,
            task_id,
            from_actor_id,
            to_actor_id,
            route,
            correlation_id,
            group_id,
            payload_json,
            created_at
        )
        VALUES (?1, ?2, ?3, NULL, 'group_chat', ?4, (SELECT group_id FROM team_tasks WHERE id = ?2), ?5, ?6)
        "#,
    )
    .bind(&shared_thread.conversation_id)
    .bind(&shared_thread.task_id)
    .bind(&cmd.from_actor_id)
    .bind(correlation_id)
    .bind(payload_json)
    .bind(cmd.created_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) fn should_persist_human_visible_chat_reply(cmd: &SendActorMessageCommand) -> bool {
    should_persist_human_visible_chat_reply_for_payload(
        &cmd.transport,
        &cmd.to_actor_id,
        &cmd.to_peer_id,
        &cmd.from_actor_id,
        &cmd.payload,
    )
}

pub(super) async fn resolve_or_create_shared_thread_for_run(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
    cmd: &SendActorMessageCommand,
) -> Result<SharedThreadTarget, sqlx::Error> {
    let team_id = resolve_team_id_for_run(tx, run_id).await?;
    if let Some(existing) = fetch_shared_thread_for_team(tx, &team_id).await? {
        return Ok(existing);
    }

    let now = cmd.created_at;
    let task_id = Uuid::new_v4().to_string();
    let conversation_id = Uuid::new_v4().to_string();
    let context_json = serde_json::json!({
        "bootstrap_kind": TEAM_SHARED_THREAD_BOOTSTRAP_KIND,
        "bootstrap_source": TEAM_SHARED_THREAD_BOOTSTRAP_SOURCE,
    })
    .to_string();

    sqlx::query(
        r#"
        INSERT INTO team_tasks (
            id,
            team_id,
            group_id,
            title,
            status,
            priority,
            created_by_actor_id,
            context_json,
            created_at,
            updated_at
        )
        VALUES (?1, ?2, (SELECT group_id FROM team_definitions WHERE id = ?2), ?3, 'open', 'medium', ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&task_id)
    .bind(&team_id)
    .bind(TEAM_SHARED_THREAD_TITLE)
    .bind(&cmd.from_actor_id)
    .bind(context_json)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO team_conversations (
            id,
            team_id,
            task_id,
            mode,
            topic,
            created_at,
            updated_at
        )
        VALUES (?1, ?2, ?3, 'group_chat', ?4, ?5, ?6)
        "#,
    )
    .bind(&conversation_id)
    .bind(&team_id)
    .bind(&task_id)
    .bind(TEAM_SHARED_THREAD_TITLE)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    Ok(SharedThreadTarget {
        task_id,
        conversation_id,
    })
}

pub(super) async fn fetch_shared_thread_for_team(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    team_id: &str,
) -> Result<Option<SharedThreadTarget>, sqlx::Error> {
    Ok(fetch_canonical_shared_thread_target(&mut **tx, team_id)
        .await?
        .map(|target| SharedThreadTarget {
            task_id: target.task_id,
            conversation_id: target.conversation_id,
        }))
}
