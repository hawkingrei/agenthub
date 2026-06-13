use std::sync::Arc;

use sqlx::SqlitePool;

use crate::agent::{
    AgentManager, AgentTimeTriggerManager, AgentTimeTriggerWorker, AgentTimeTriggerWorkerSettings,
};
use crate::team::{TeamMailboxUnreadHintWorker, TeamMailboxUnreadHintWorkerSettings, TeamManager};

use super::AppState;

impl AppState {
    pub(super) async fn spawn_startup_workers(
        db: &SqlitePool,
        agents: &Arc<AgentManager>,
        teams: &Arc<TeamManager>,
        body_store: &Option<crate::message_body_store::SharedBodyStore>,
        auto_migrate_conversation_bodies: bool,
    ) -> anyhow::Result<()> {
        if let Some(store) = body_store {
            Self::spawn_message_body_outbox_drainer(db.clone(), store.clone());
            if auto_migrate_conversation_bodies {
                Self::spawn_conversation_body_backfill(db.clone(), store.clone());
            }
        }
        let _mailbox_hint_handle = TeamMailboxUnreadHintWorker::new(teams.clone(), agents.clone())
            .spawn(TeamMailboxUnreadHintWorkerSettings::default());
        let trigger_manager = Arc::new(AgentTimeTriggerManager::new(db.clone()));
        let recovered_dispatching = trigger_manager.reset_inflight_on_startup().await?;
        if recovered_dispatching > 0 {
            tracing::info!(
                recovered_dispatching,
                "agent time triggers reset to scheduled on startup"
            );
        }
        let _agent_trigger_handle = AgentTimeTriggerWorker::new(trigger_manager, agents.clone())
            .spawn(AgentTimeTriggerWorkerSettings::default());
        Ok(())
    }

    /// Backfills existing inline conversation bodies into the body store once, in the background, so a
    /// database that predates the store being enabled is migrated automatically after an upgrade and
    /// restart. It is safe to run on every startup: it is idempotent and resumable (already-moved rows
    /// are skipped), and the read path serves both inline and moved rows, so online traffic during the
    /// backfill is unaffected. Once the database is fully migrated each startup only pays a quick count.
    fn spawn_conversation_body_backfill(
        db: SqlitePool,
        store: crate::message_body_store::SharedBodyStore,
    ) {
        const BACKFILL_BATCH: usize = 256;
        tokio::spawn(async move {
            match crate::team::count_inline_conversation_bodies(&db).await {
                Ok(0) => return,
                Ok(remaining) => {
                    tracing::info!(
                        remaining,
                        "backfilling inline conversation bodies into the body store"
                    );
                }
                Err(error) => {
                    tracing::warn!(error = %error, "conversation body backfill: failed to count inline bodies; skipping this startup");
                    return;
                }
            }
            let staged_at = chrono::Utc::now().timestamp();
            match crate::team::migrate_conversation_bodies_into_store(
                &db,
                store.as_ref(),
                BACKFILL_BATCH,
                staged_at,
            )
            .await
            {
                Ok(report) => {
                    tracing::info!(
                        migrated = report.migrated,
                        confirmed = report.drained,
                        "conversation body backfill complete"
                    );
                }
                Err(error) => {
                    tracing::warn!(error = %error, "conversation body backfill failed; will retry on next startup");
                }
            }
        });
    }

    /// Periodically drains the SQLite message-body outbox into the body store, so staged bodies are
    /// moved into the tiered (compressed) store shortly after they are written.
    fn spawn_message_body_outbox_drainer(
        db: SqlitePool,
        store: crate::message_body_store::SharedBodyStore,
    ) {
        const DRAIN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
        const DRAIN_BATCH: usize = 256;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(DRAIN_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                // Keep draining while batches come back full so a backlog (e.g. on startup) isn't
                // paced at one batch per interval; wait for the next tick once it is (nearly) empty.
                loop {
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
        });
    }
}
