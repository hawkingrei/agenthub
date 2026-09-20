use agenthub_agent_domain::loop_runtime::LoopReservation;
use agenthub_db::loop_runtime::{LoopStore, LoopStoreError};

use super::{TeamManager, codec_rows::parse_team_conversation_message_row};
use crate::team::loop_context::LoopWorkSourceDetail;

impl TeamManager {
    /// Only a source bound to this executor can select an exact canonical message.
    pub(crate) async fn loop_work_source(
        &self,
        reservation: &LoopReservation,
        source_id: &str,
    ) -> anyhow::Result<LoopWorkSourceDetail> {
        let source = LoopStore::new(self.db.clone())
            .work_source(reservation, source_id, chrono::Utc::now().timestamp())
            .await?;
        let conversation_message = if let Some(id) = source.input.references.conversation_message_id
        {
            let row = sqlx::query("SELECT m.* FROM team_conversation_messages m JOIN team_conversations c ON c.id = m.conversation_id WHERE m.id = ? AND c.team_id = ?")
                .bind(id).bind(&reservation.team_id).fetch_optional(&self.db).await?
                .ok_or(LoopStoreError::ScopeMismatch)?;
            let (mut message, moved) = parse_team_conversation_message_row(&row)?;
            if moved {
                self.rehydrate_moved_conversation_payload(&mut message)
                    .await?;
            }
            Some(message)
        } else {
            None
        };
        let mailbox_message = if let Some(id) = source.input.references.mailbox_message_id {
            let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM team_actor_messages m JOIN team_runs r ON r.id = m.run_id WHERE m.id = ? AND r.team_id = ? AND m.to_actor_id = ?)")
                .bind(id).bind(&reservation.team_id).bind(&reservation.actor_id).fetch_one(&self.db).await?;
            anyhow::ensure!(valid, LoopStoreError::ScopeMismatch);
            Some(super::mailbox_queries::fetch_enriched_message_by_id(&self.db, id).await?)
        } else {
            None
        };
        Ok(LoopWorkSourceDetail {
            source,
            conversation_message,
            mailbox_message,
        })
    }
}
