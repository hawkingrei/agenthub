use super::{MessageArchiveScopeFallback, TeamManager, TeamMessageArchiveMigrationReport};
use agenthub_message_archive::{MessageSearchHit, MessageSearchQuery};

impl TeamManager {
    pub async fn search_message_archive(
        &self,
        query: &MessageSearchQuery,
    ) -> anyhow::Result<Vec<MessageSearchHit>> {
        let Some(archive) = self.message_archive.as_ref() else {
            return Ok(Vec::new());
        };
        archive.search(query).await
    }

    pub async fn migrate_team_messages_to_archive(
        &self,
        batch_size: usize,
    ) -> anyhow::Result<TeamMessageArchiveMigrationReport> {
        let Some(archive) = self.message_archive.as_ref() else {
            anyhow::bail!("message archive is not configured");
        };
        let mut report = TeamMessageArchiveMigrationReport::default();
        let batch_size = batch_size.clamp(1, 1000);
        let batch_limit = i64::try_from(batch_size)?;
        let max_conversation_message_id = self
            .message_archive_table_max_id("team_conversation_messages")
            .await?;
        let max_run_event_id = self.message_archive_table_max_id("team_run_events").await?;
        let max_actor_message_id = self
            .message_archive_table_max_id("team_actor_messages")
            .await?;
        let agent_event_snapshot = self.message_archive_agent_event_snapshot().await?;
        let mut run_scope_cache =
            std::collections::HashMap::<String, MessageArchiveScopeFallback>::new();
        let mut task_conversation_cache =
            std::collections::HashMap::<(String, String), Option<String>>::new();

        self.migrate_team_conversation_messages_to_archive(
            archive,
            batch_limit,
            max_conversation_message_id,
            &mut report,
        )
        .await?;
        self.migrate_team_run_events_to_archive(
            archive,
            batch_limit,
            max_run_event_id,
            &mut run_scope_cache,
            &mut task_conversation_cache,
            &mut report,
        )
        .await?;
        self.migrate_team_actor_messages_to_archive(
            archive,
            batch_limit,
            max_actor_message_id,
            &mut run_scope_cache,
            &mut task_conversation_cache,
            &mut report,
        )
        .await?;

        let agent_event_counts = self
            .migrate_agent_events_to_archive(archive, batch_size, &agent_event_snapshot)
            .await?;
        report.agent_events = agent_event_counts.agent_events;
        report.aggregated_acp_messages = agent_event_counts.aggregated_acp_messages;
        Ok(report)
    }
}
