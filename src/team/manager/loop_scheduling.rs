use agenthub_agent_domain::loop_runtime::LoopSourceReferences;
use agenthub_agent_domain::loop_scheduling::{
    LoopRegistrationInput, LoopRegistrationReceipt, LoopScheduleRequest,
};
use agenthub_db::loop_runtime::{LoopStore, LoopStoreError};
use sha2::{Digest, Sha256};

use super::TeamManager;

impl TeamManager {
    pub(crate) async fn request_loop_schedule(
        &self,
        team_id: &str,
        member_id: &str,
        request: &LoopScheduleRequest,
    ) -> anyhow::Result<LoopRegistrationReceipt> {
        request.validate()?;
        let context = crate::team::loop_context::scheduling_context();
        anyhow::ensure!(
            matches!(
                (&context.actor_id, &context.user_id),
                (Some(_), None) | (None, Some(_))
            ),
            "an authenticated scheduling identity is required"
        );
        let digest = Sha256::digest(serde_json::to_vec(&(
            &context.actor_id,
            &context.user_id,
            &request.source_key,
        ))?);
        let input = LoopRegistrationInput {
            actor_id: member_id.into(),
            team_id: team_id.into(),
            source_key: format!("schedule:{}", super::hex_encode(&digest)),
            schedule: request.schedule.clone(),
            work_task_id: request.work_task_id.clone(),
            references: LoopSourceReferences {
                task_id: request.work_task_id.clone(),
                scheduling_actor_id: context.actor_id,
                scheduling_activation_id: context.activation_id,
                scheduling_user_id: context.user_id,
                ..Default::default()
            },
        };
        let mut tx = self.db.begin_with("BEGIN IMMEDIATE").await?;
        let spec: String =
            sqlx::query_scalar("SELECT spec_json FROM team_definitions WHERE id = ?")
                .bind(team_id)
                .fetch_one(&mut *tx)
                .await?;
        anyhow::ensure!(
            Self::uses_loop_execution(&serde_json::from_str(&spec)?),
            LoopStoreError::ScopeMismatch
        );
        let receipt =
            LoopStore::register_schedule_tx(&mut tx, &input, chrono::Utc::now().timestamp())
                .await?;
        tx.commit().await?;
        Ok(receipt)
    }
}
