use anyhow::Context;

use super::archive_documents::AgentEventArchiveMigrationCounts;
use super::archive_migration::{
    migrate_main_agent_events_to_archive, migrate_per_agent_events_to_archive,
};
use super::{AgentEventArchiveSnapshot, TeamManager};
use agenthub_message_archive::MessageArchiveStoreRef;

impl TeamManager {
    pub(super) async fn message_archive_table_max_id(
        &self,
        table_name: &str,
    ) -> anyhow::Result<i64> {
        let sql = match table_name {
            "team_conversation_messages" => {
                "SELECT COALESCE(MAX(id), 0) FROM team_conversation_messages"
            }
            "team_run_events" => "SELECT COALESCE(MAX(id), 0) FROM team_run_events",
            "team_actor_messages" => "SELECT COALESCE(MAX(id), 0) FROM team_actor_messages",
            _ => anyhow::bail!("unsupported archive migration table: {table_name}"),
        };
        Ok(sqlx::query_scalar::<_, i64>(sql)
            .fetch_one(&self.db)
            .await?)
    }

    pub(super) async fn migrate_agent_events_to_archive(
        &self,
        archive: &MessageArchiveStoreRef,
        batch_size: usize,
        snapshot: &AgentEventArchiveSnapshot,
    ) -> anyhow::Result<AgentEventArchiveMigrationCounts> {
        let mut counts = AgentEventArchiveMigrationCounts::default();
        counts.add(
            migrate_main_agent_events_to_archive(
                &self.db,
                archive,
                batch_size,
                snapshot.main_max_id,
            )
            .await
            .context("migrate main agent_events to message archive")?,
        );

        for (agent_id, max_id) in &snapshot.per_agent_max_ids {
            let pool = self.event_dbs.pool_for_agent(agent_id).await?;
            counts.add(
                migrate_per_agent_events_to_archive(
                    &self.db, &pool, archive, agent_id, batch_size, *max_id,
                )
                .await
                .with_context(|| {
                    format!("migrate per-agent agent_events for agent_id={agent_id}")
                })?,
            );
        }
        Ok(counts)
    }

    pub(super) async fn message_archive_agent_event_snapshot(
        &self,
    ) -> anyhow::Result<AgentEventArchiveSnapshot> {
        let main_max_id =
            sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(id), 0) FROM agent_events")
                .fetch_one(&self.db)
                .await?;
        let agent_ids = sqlx::query_scalar::<_, String>("SELECT id FROM agents ORDER BY id ASC")
            .fetch_all(&self.db)
            .await?;
        let mut per_agent_max_ids = Vec::new();
        for agent_id in agent_ids {
            let db_path = self.event_dbs.db_path_for_agent(&agent_id);
            if !tokio::fs::try_exists(&db_path).await? {
                continue;
            }
            let pool = self.event_dbs.pool_for_agent(&agent_id).await?;
            let max_id =
                sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(id), 0) FROM agent_events")
                    .fetch_one(&pool)
                    .await?;
            per_agent_max_ids.push((agent_id, max_id));
        }
        Ok(AgentEventArchiveSnapshot {
            main_max_id,
            per_agent_max_ids,
        })
    }
}
