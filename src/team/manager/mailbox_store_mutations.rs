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
        apply_thread_claim_transition(&mut tx, &message, cmd).await?;
        let update = sqlx::query(
            r#"
            UPDATE team_actor_messages
            SET
                handling_disposition = ?1,
                handled_by_actor_id = ?2,
                handled_at = ?3
            WHERE id = ?4
              AND run_id = ?5
              AND to_actor_id = ?6
              AND to_peer_id = ?7
              AND handling_disposition <> ?1
            "#,
        )
        .bind(cmd.disposition.as_str())
        .bind(&cmd.actor_id)
        .bind(cmd.handled_at)
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
