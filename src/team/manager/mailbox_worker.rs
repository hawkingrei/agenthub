use std::sync::Arc;
use std::time::Duration;

use super::{TeamManager, TeamRemoteRelayWorkerSettings};

impl TeamManager {
    pub fn spawn_remote_relay_worker(
        self: Arc<Self>,
        daemon_tasks: &crate::daemon_tasks::DaemonTaskGroup,
        settings: TeamRemoteRelayWorkerSettings,
    ) -> anyhow::Result<()> {
        let cancellation = daemon_tasks.background_cancellation();
        daemon_tasks.spawn_background_worker("team-remote-relay", async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(
                settings.poll_interval_secs.max(1) as u64,
            ));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = cancellation.cancelled() => break,
                    _ = ticker.tick() => {}
                }
                match self
                    .relay_remote_messages_once(
                        settings.batch_limit,
                        settings.max_attempts,
                        settings.retry_delay_secs,
                    )
                    .await
                {
                    Ok(summary) => {
                        if summary.scanned > 0 {
                            tracing::debug!(
                                scanned = summary.scanned,
                                delivered = summary.delivered,
                                retried = summary.retried,
                                dead_lettered = summary.dead_lettered,
                                "team relay worker tick"
                            );
                        }
                    }
                    Err(err) => {
                        tracing::warn!("team relay worker tick failed: {}", err);
                    }
                }
            }
            Ok(())
        })
    }
}
