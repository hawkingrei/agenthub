use agenthub_team_actor::{
    ActorMessageHandlingDisposition, ActorThreadClaimStatus, SendActorMessageCommand,
    TriageActorMessageCommand, actor_message_fingerprint, derive_actor_message_topic_metadata,
    parse_actor_message_task_relation, parse_actor_thread_claim_status,
};
use sqlx::{Row, Sqlite};

use super::mailbox::ACTOR_THREAD_CLAIM_DEFAULT_LEASE_SECS;
use super::mailbox_store::{ActorMessageTaskLinkRecord, SqlActorMailboxStoreError};
use crate::team::TeamActorMessageRecord;

pub(super) async fn fetch_latest_task_link_for_message(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
    message_id: i64,
) -> Result<Option<ActorMessageTaskLinkRecord>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT task_id, relation
        FROM team_actor_message_links
        WHERE run_id = ?1 AND message_id = ?2
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(run_id)
    .bind(message_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let relation_raw: String = row.get("relation");
    let Some(relation) = parse_actor_message_task_relation(&relation_raw) else {
        return Ok(None);
    };
    Ok(Some(ActorMessageTaskLinkRecord {
        task_id: row.get("task_id"),
        relation,
    }))
}

pub(super) async fn apply_thread_claim_transition(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    message: &TeamActorMessageRecord,
    cmd: &TriageActorMessageCommand,
) -> Result<(), SqlActorMailboxStoreError> {
    let explicit_task_id = fetch_latest_task_link_for_message(tx, &cmd.run_id, cmd.message_id)
        .await?
        .map(|link| link.task_id);
    let Some(topic) = derive_actor_message_topic_metadata(
        message.message_id,
        &message.payload,
        explicit_task_id.as_deref(),
    ) else {
        return Ok(());
    };
    match cmd.disposition {
        ActorMessageHandlingDisposition::Claimed => {
            let existing = sqlx::query(
                r#"
                SELECT owner_actor_id, claim_status, lease_expires_at
                FROM team_actor_thread_claims
                WHERE run_id = ?1 AND topic_key = ?2
                LIMIT 1
                "#,
            )
            .bind(&cmd.run_id)
            .bind(&topic.topic_key)
            .fetch_optional(&mut **tx)
            .await?;
            if let Some(existing) = existing {
                let owner_actor_id: String = existing.get("owner_actor_id");
                let claim_status_raw: String = existing.get("claim_status");
                let lease_expires_at = existing
                    .try_get::<Option<i64>, _>("lease_expires_at")
                    .unwrap_or(None);
                let is_active = parse_actor_thread_claim_status(&claim_status_raw)
                    == Some(ActorThreadClaimStatus::Claimed)
                    && lease_expires_at.is_none_or(|value| value > cmd.handled_at);
                if is_active && owner_actor_id != cmd.actor_id {
                    return Err(SqlActorMailboxStoreError::ThreadClaimConflict { owner_actor_id });
                }
            }
            sqlx::query(
                r#"
                INSERT INTO team_actor_thread_claims (
                    run_id,
                    topic_key,
                    task_id,
                    root_message_id,
                    owner_actor_id,
                    claim_status,
                    claimed_message_id,
                    claimed_at,
                    lease_expires_at,
                    updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, 'claimed', ?6, ?7, ?8, ?7)
                ON CONFLICT(run_id, topic_key) DO UPDATE SET
                    task_id = excluded.task_id,
                    root_message_id = excluded.root_message_id,
                    owner_actor_id = excluded.owner_actor_id,
                    claim_status = excluded.claim_status,
                    claimed_message_id = excluded.claimed_message_id,
                    claimed_at = excluded.claimed_at,
                    lease_expires_at = excluded.lease_expires_at,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(&cmd.run_id)
            .bind(&topic.topic_key)
            .bind(&topic.task_id)
            .bind(topic.root_message_id)
            .bind(&cmd.actor_id)
            .bind(cmd.message_id)
            .bind(cmd.handled_at)
            .bind(cmd.handled_at + ACTOR_THREAD_CLAIM_DEFAULT_LEASE_SECS)
            .execute(&mut **tx)
            .await?;
        }
        ActorMessageHandlingDisposition::Released | ActorMessageHandlingDisposition::Completed => {
            let existing = sqlx::query(
                r#"
                SELECT owner_actor_id, claim_status, lease_expires_at
                FROM team_actor_thread_claims
                WHERE run_id = ?1 AND topic_key = ?2
                LIMIT 1
                "#,
            )
            .bind(&cmd.run_id)
            .bind(&topic.topic_key)
            .fetch_optional(&mut **tx)
            .await?;
            let Some(existing) = existing else {
                return Err(SqlActorMailboxStoreError::ThreadClaimOwnershipRequired);
            };
            let owner_actor_id: String = existing.get("owner_actor_id");
            let claim_status_raw: String = existing.get("claim_status");
            let lease_expires_at = existing
                .try_get::<Option<i64>, _>("lease_expires_at")
                .unwrap_or(None);
            let is_active = parse_actor_thread_claim_status(&claim_status_raw)
                == Some(ActorThreadClaimStatus::Claimed)
                && lease_expires_at.is_none_or(|value| value > cmd.handled_at);
            if owner_actor_id != cmd.actor_id && is_active {
                return Err(SqlActorMailboxStoreError::ThreadClaimConflict { owner_actor_id });
            }
            if owner_actor_id != cmd.actor_id || !is_active {
                return Err(SqlActorMailboxStoreError::ThreadClaimOwnershipRequired);
            }
            let claim_status = match cmd.disposition {
                ActorMessageHandlingDisposition::Released => "released",
                ActorMessageHandlingDisposition::Completed => "completed",
                _ => unreachable!(),
            };
            let updated = sqlx::query(
                r#"
                UPDATE team_actor_thread_claims
                SET
                    claim_status = ?1,
                    lease_expires_at = ?2,
                    updated_at = ?2
                WHERE run_id = ?3
                  AND topic_key = ?4
                  AND owner_actor_id = ?5
                  AND claim_status = 'claimed'
                "#,
            )
            .bind(claim_status)
            .bind(cmd.handled_at)
            .bind(&cmd.run_id)
            .bind(&topic.topic_key)
            .bind(&cmd.actor_id)
            .execute(&mut **tx)
            .await?;
            if updated.rows_affected() == 0 {
                return Err(SqlActorMailboxStoreError::ThreadClaimOwnershipRequired);
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn ensure_idempotency_compatible(
    cmd: &SendActorMessageCommand,
    existing: &TeamActorMessageRecord,
) -> Result<(), SqlActorMailboxStoreError> {
    let incoming_fp = actor_message_fingerprint(
        &cmd.run_id,
        &cmd.from_actor_id,
        &cmd.from_peer_id,
        &cmd.to_actor_id,
        &cmd.to_peer_id,
        &cmd.channel,
        cmd.transport.as_str(),
        cmd.route.as_ref(),
        &cmd.payload,
    );
    let existing_fp = actor_message_fingerprint(
        &existing.run_id,
        &existing.from_actor_id,
        &existing.from_peer_id,
        &existing.to_actor_id,
        &existing.to_peer_id,
        &existing.channel,
        existing.transport.as_str(),
        existing.route.as_ref(),
        &existing.payload,
    );
    if incoming_fp != existing_fp {
        return Err(SqlActorMailboxStoreError::IdempotencyConflict);
    }
    Ok(())
}
