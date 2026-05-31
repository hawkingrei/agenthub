use agenthub_team_actor::{
    ActorMessageHandlingDisposition, LinkActorMessageTaskCommand, LinkActorMessageTaskResult,
    TriageActorMessageCommand, TriageActorMessageResult,
};
use sqlx::Error as SqlxError;
use sqlx::Sqlite;

use super::mailbox_queries::{
    fetch_enriched_message_by_id, fetch_message_by_id, fetch_message_for_actor,
    resolve_team_id_for_run,
};
use super::mailbox_store::{SqlActorMailboxStore, SqlActorMailboxStoreError};
use super::mailbox_threads::apply_thread_claim_transition;

async fn ensure_reply_required_completion_allowed(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    message: &crate::team::TeamActorMessageRecord,
) -> Result<(), SqlActorMailboxStoreError> {
    if super::mailbox_reply_obligations::reply_actor_pair_for_inbound_obligation(message).is_none()
    {
        return Ok(());
    }
    let messages =
        super::mailbox_reply_obligation_summary::load_reply_obligation_message_snapshots_on_executor(
            &mut **tx,
            &message.run_id,
        )
        .await?;
    if super::mailbox_reply_obligation_summary::has_visible_reply_credit_for_message(
        &messages,
        message.message_id,
    ) {
        return Ok(());
    }
    Err(SqlActorMailboxStoreError::ReplyRequiredVisibleOutcomeMissing)
}

fn trimmed_triage_reason(cmd: &TriageActorMessageCommand) -> Option<&str> {
    cmd.reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn build_reply_required_ignore_payload(
    message: &crate::team::TeamActorMessageRecord,
    cmd: &TriageActorMessageCommand,
) -> Result<Option<serde_json::Value>, SqlActorMailboxStoreError> {
    if cmd.disposition != ActorMessageHandlingDisposition::Ignored {
        return Ok(None);
    }
    let is_reply_required =
        super::mailbox_reply_obligations::reply_actor_pair_for_inbound_obligation(message)
            .is_some();
    let reason = trimmed_triage_reason(cmd);
    if is_reply_required && reason.is_none() {
        return Err(SqlActorMailboxStoreError::ReplyRequiredIgnoreReasonMissing);
    }
    Ok(reason.map(|reason| {
        super::mailbox_reply_obligations::build_ignored_mailbox_payload(
            &message.payload,
            &cmd.actor_id,
            reason,
            cmd.handled_at,
        )
    }))
}

impl SqlActorMailboxStore {
    pub(super) async fn triage_message_impl(
        &self,
        cmd: &TriageActorMessageCommand,
    ) -> Result<TriageActorMessageResult, SqlActorMailboxStoreError> {
        let mut tx = self.db.begin().await?;
        let message = match fetch_message_for_actor(
            &mut tx,
            &cmd.run_id,
            &cmd.actor_id,
            &cmd.peer_id,
            cmd.message_id,
        )
        .await
        {
            Ok(message) => message,
            Err(SqlxError::RowNotFound)
                if matches!(
                    cmd.disposition,
                    ActorMessageHandlingDisposition::Claimed
                        | ActorMessageHandlingDisposition::Watching
                        | ActorMessageHandlingDisposition::Released
                        | ActorMessageHandlingDisposition::Completed
                ) =>
            {
                fetch_message_by_id(&mut tx, cmd.message_id).await?
            }
            Err(err) => return Err(err.into()),
        };
        if cmd.disposition == ActorMessageHandlingDisposition::Completed {
            ensure_reply_required_completion_allowed(&mut tx, &message).await?;
        }
        let ignore_payload = build_reply_required_ignore_payload(&message, cmd)?;
        apply_thread_claim_transition(&mut tx, &message, cmd).await?;
        let update = sqlx::query(
            r#"
            UPDATE team_actor_messages
            SET
                handling_disposition = ?1,
                handled_by_actor_id = ?2,
                handled_at = ?3,
                payload_json = COALESCE(?4, payload_json)
            WHERE id = ?5
              AND run_id = ?6
              AND to_actor_id = ?7
              AND to_peer_id = ?8
              AND handling_disposition <> ?1
            "#,
        )
        .bind(cmd.disposition.as_str())
        .bind(&cmd.actor_id)
        .bind(cmd.handled_at)
        .bind(
            ignore_payload
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|err| sqlx::Error::Protocol(err.to_string()))?,
        )
        .bind(cmd.message_id)
        .bind(&cmd.run_id)
        .bind(&cmd.actor_id)
        .bind(&cmd.peer_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let message = fetch_enriched_message_by_id(&self.db, cmd.message_id).await?;
        Ok(TriageActorMessageResult {
            message,
            handling_changed: update.rows_affected() > 0,
        })
    }

    pub(super) async fn link_message_task_impl(
        &self,
        cmd: &LinkActorMessageTaskCommand,
    ) -> Result<LinkActorMessageTaskResult, SqlActorMailboxStoreError> {
        let mut tx = self.db.begin().await?;
        fetch_message_for_actor(
            &mut tx,
            &cmd.run_id,
            &cmd.actor_id,
            &cmd.peer_id,
            cmd.message_id,
        )
        .await?;
        let team_id = resolve_team_id_for_run(&mut tx, &cmd.run_id).await?;
        let task_exists = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT 1
            FROM team_tasks
            WHERE id = ?1 AND team_id = ?2
            LIMIT 1
            "#,
        )
        .bind(&cmd.task_id)
        .bind(&team_id)
        .fetch_optional(&mut *tx)
        .await?;
        if task_exists.is_none() {
            return Err(sqlx::Error::RowNotFound.into());
        }
        let inserted = sqlx::query(
            r#"
            INSERT OR IGNORE INTO team_actor_message_links (
                run_id,
                message_id,
                task_id,
                relation,
                created_by_actor_id,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&cmd.run_id)
        .bind(cmd.message_id)
        .bind(&cmd.task_id)
        .bind(cmd.relation.as_str())
        .bind(&cmd.actor_id)
        .bind(cmd.linked_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let message = fetch_enriched_message_by_id(&self.db, cmd.message_id).await?;
        Ok(LinkActorMessageTaskResult {
            message,
            task_id: cmd.task_id.clone(),
            relation: cmd.relation.clone(),
            created: inserted.rows_affected() > 0,
        })
    }
}
