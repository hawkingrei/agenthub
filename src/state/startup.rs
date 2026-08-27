use std::sync::Arc;

use sqlx::SqlitePool;

use crate::agent::{
    AgentManager, AgentTimeTriggerManager, AgentTimeTriggerWorker, AgentTimeTriggerWorkerSettings,
};
use crate::team::{
    TeamMailboxRuntimeDeliveryWorker, TeamMailboxRuntimeDeliveryWorkerSettings,
    TeamMailboxUnreadHintWorker, TeamMailboxUnreadHintWorkerSettings, TeamManager,
};

use super::AppState;

impl AppState {
    pub(super) async fn spawn_startup_workers(
        db: &SqlitePool,
        agents: &Arc<AgentManager>,
        teams: &Arc<TeamManager>,
        body_store: &Option<crate::message_body_store::SharedBodyStore>,
        auto_migrate_conversation_bodies: bool,
    ) -> anyhow::Result<()> {
        let daemon_tasks = agents.daemon_tasks();
        if let Some(store) = body_store {
            Self::spawn_message_body_outbox_drainer(daemon_tasks, db.clone(), store.clone())?;
            if auto_migrate_conversation_bodies {
                Self::spawn_conversation_body_backfill(daemon_tasks, db.clone(), store.clone())?;
            }
        }
        #[cfg(feature = "rocksdb")]
        if let (Some(index), Some(read_repair)) = (
            &teams.message_index_for_repair(),
            &teams.read_repair_scheduler_for_repair(),
        ) {
            Self::spawn_message_index_read_repair_worker(
                daemon_tasks,
                teams.clone(),
                index.clone(),
                read_repair.clone(),
            )?;
        }
        TeamMailboxUnreadHintWorker::new(teams.clone(), agents.clone())
            .spawn(daemon_tasks, TeamMailboxUnreadHintWorkerSettings::default())?;
        TeamMailboxRuntimeDeliveryWorker::new(teams.clone(), agents.clone()).spawn(
            daemon_tasks,
            TeamMailboxRuntimeDeliveryWorkerSettings::default(),
        )?;
        let trigger_manager = Arc::new(AgentTimeTriggerManager::new(db.clone()));
        let recovered_dispatching = trigger_manager.reset_inflight_on_startup().await?;
        if recovered_dispatching > 0 {
            tracing::info!(
                recovered_dispatching,
                "agent time triggers reset to scheduled on startup"
            );
        }
        AgentTimeTriggerWorker::new(trigger_manager, agents.clone())
            .spawn(daemon_tasks, AgentTimeTriggerWorkerSettings::default())?;
        Ok(())
    }

    /// Stages existing SQLite compatibility bodies into the body store in the background. The durable
    /// checkpoint makes this resumable, while SQLite remains readable throughout Phase 1. It also
    /// repairs any sentinel rows left by the earlier move-out implementation before proceeding.
    fn spawn_conversation_body_backfill(
        daemon_tasks: &crate::daemon_tasks::DaemonTaskGroup,
        db: SqlitePool,
        store: crate::message_body_store::SharedBodyStore,
    ) -> anyhow::Result<()> {
        const BACKFILL_BATCH: usize = 256;
        // Pace the background backfill so it yields the single SQLite writer between batches and does
        // not starve concurrent writes (incoming messages, agent triggers) on startup.
        const BACKFILL_INTER_BATCH_DELAY: std::time::Duration =
            std::time::Duration::from_millis(25);
        let cancellation = daemon_tasks.background_cancellation();
        daemon_tasks.spawn_background_job("conversation-body-backfill", async move {
            let backfill = async move {
                match crate::team::count_pending_conversation_body_migration(&db).await {
                    Ok(0) => return,
                    Ok(remaining) => {
                        tracing::info!(
                            remaining,
                            "staging SQLite conversation bodies into the body store"
                        );
                    }
                    Err(error) => {
                        tracing::warn!(error = %error, "conversation body backfill: failed to count pending rows; skipping this startup");
                        return;
                    }
                }
                let staged_at = chrono::Utc::now().timestamp();
                match crate::team::migrate_conversation_bodies_into_store(
                    &db,
                    store.as_ref(),
                    BACKFILL_BATCH,
                    staged_at,
                    BACKFILL_INTER_BATCH_DELAY,
                )
                .await
                {
                    Ok(report) => {
                        tracing::info!(
                            restored = report.restored,
                            staged = report.staged,
                            confirmed = report.drained,
                            "conversation body backfill complete"
                        );
                    }
                    Err(error) => {
                        tracing::warn!(error = %error, "conversation body backfill failed; will retry on next startup");
                    }
                }
            };
            tokio::select! {
                _ = cancellation.cancelled() => {}
                _ = backfill => {}
            }
            Ok(())
        })
    }

    /// Periodically drains the SQLite message-body outbox into the body store, so staged bodies are
    /// moved into the tiered (compressed) store shortly after they are written.
    fn spawn_message_body_outbox_drainer(
        daemon_tasks: &crate::daemon_tasks::DaemonTaskGroup,
        db: SqlitePool,
        store: crate::message_body_store::SharedBodyStore,
    ) -> anyhow::Result<()> {
        const DRAIN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
        const DRAIN_BATCH: usize = 256;
        let cancellation = daemon_tasks.background_cancellation();
        daemon_tasks.spawn_background_worker("message-body-outbox", async move {
            let mut interval = tokio::time::interval(DRAIN_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = cancellation.cancelled() => break,
                    _ = interval.tick() => {}
                }
                // Keep draining while batches come back full so a backlog (e.g. on startup) isn't
                // paced at one batch per interval; wait for the next tick once it is (nearly) empty.
                loop {
                    if cancellation.is_cancelled() {
                        break;
                    }
                    match agenthub_db::message_body_outbox::drain_into(
                        &db,
                        store.as_ref(),
                        DRAIN_BATCH,
                    )
                    .await
                    {
                        Ok(drained) => {
                            if drained > 0 {
                                tracing::debug!(
                                    drained,
                                    "message body outbox drained into body store"
                                );
                            }
                            if drained < DRAIN_BATCH {
                                break;
                            }
                        }
                        Err(error) => {
                            tracing::warn!(error = %error, "message body outbox drain failed");
                            break;
                        }
                    }
                }
            }
            Ok(())
        })
    }

    #[cfg(feature = "rocksdb")]
    fn spawn_message_index_read_repair_worker(
        daemon_tasks: &crate::daemon_tasks::DaemonTaskGroup,
        teams: Arc<TeamManager>,
        index: crate::message_body_store::SharedIndexStore,
        read_repair: crate::message_body_store::SharedReadRepairScheduler,
    ) -> anyhow::Result<()> {
        const DRAIN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
        const DRAIN_BATCH: i64 = 256;
        let cancellation = daemon_tasks.background_cancellation();
        daemon_tasks.spawn_background_worker("message-index-read-repair", async move {
            let mut interval = tokio::time::interval(DRAIN_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = cancellation.cancelled() => break,
                    _ = interval.tick() => {}
                }
                match teams
                    .drain_message_index_read_repairs(
                        index.as_ref(),
                        read_repair.as_ref(),
                        DRAIN_BATCH,
                    )
                    .await
                {
                    Ok(0) => {}
                    Ok(repaired_streams) => {
                        tracing::debug!(repaired_streams, "message index read repairs completed")
                    }
                    Err(error) => tracing::warn!(
                        error = %error,
                        "message index read repair worker failed"
                    ),
                }
            }
            Ok(())
        })
    }
}
