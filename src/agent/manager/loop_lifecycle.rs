use std::time::Duration;

use agenthub_agent_domain::loop_runtime::{LoopCleanupDisposition, LoopReservation};
use agenthub_db::loop_runtime::LoopStore;
use chrono::Utc;

use super::AgentManager;

impl AgentManager {
    pub(crate) fn schedule_loop_finalization(
        &self,
        reservation: LoopReservation,
    ) -> anyhow::Result<()> {
        let manager = self.clone();
        self.daemon_tasks.spawn_runtime_task(
            format!(
                "loop-finalize:{}:{}",
                reservation.actor_id, reservation.generation
            ),
            async move { manager.fence_loop_reservation(&reservation).await },
        )
    }
    pub(super) async fn cleanup_observed_session(
        &self,
        actor_id: &str,
        session_id: &str,
        child: &super::SharedSupervisedChild,
    ) -> anyhow::Result<()> {
        self.process_supervisor
            .stop_session_or_child(session_id, child)
            .await?;
        self.release_loop_after_cleanup(actor_id, Some(session_id), LoopCleanupDisposition::Exited)
            .await
    }

    pub(super) async fn reserve_manual_loop_start(&self, actor_id: &str) -> anyhow::Result<bool> {
        let team: Option<String> =
            sqlx::query_scalar("SELECT team_id FROM loop_policies WHERE actor_id = ?")
                .bind(actor_id)
                .fetch_optional(&self.db)
                .await?;
        let Some(team) = team else {
            return Ok(false);
        };
        // Other platforms keep legacy execution until they expose equivalent cleanup evidence.
        anyhow::ensure!(
            cfg!(target_os = "linux"),
            "loop execution currently requires Linux process-group verification"
        );
        let store = LoopStore::new(self.db.clone());
        let reservation = store
            .reserve_manual(&team, actor_id, &self.loop_owner_id, Utc::now().timestamp())
            .await?;
        self.track_loop_reservation(reservation).await?;
        Ok(true)
    }

    pub(super) async fn track_loop_reservation(
        &self,
        reservation: LoopReservation,
    ) -> anyhow::Result<()> {
        let actor_id = reservation.actor_id.clone();
        self.loop_reservations
            .lock()
            .await
            .insert(actor_id.clone(), reservation.clone());
        let manager = self.clone();
        let cancellation = self.daemon_tasks.runtime_cancellation();
        let result = self.daemon_tasks.spawn_runtime_task(format!("loop-lease:{actor_id}:{}", reservation.generation), async move {
            let store = LoopStore::new(manager.db.clone());
            let interval = Duration::from_secs(u64::from(reservation.renewal_seconds));
            loop {
                tokio::select! {
                    _ = cancellation.cancelled() => return Ok(()),
                    _ = tokio::time::sleep(interval) => {}
                }
                let current = manager.loop_reservations.lock().await.get(&actor_id).cloned();
                let Some(current) = current.filter(|current| current.generation == reservation.generation) else { return Ok(()); };
                if let Err(error) = store.renew(&current, Utc::now().timestamp()).await {
                    tracing::warn!(actor_id, generation = current.generation, %error, "loop lease renewal failed; fencing the local executor");
                    manager.fence_loop_reservation(&current).await?;
                    return Ok(());
                }
                if let Some(id) = &current.activation_id {
                    match store.activation(&current.team_id, id).await {
                        Ok(Some(activation)) if activation.state == agenthub_agent_domain::loop_runtime::LoopActivationState::Finalizing => {
                            manager.fence_loop_reservation(&current).await?;
                            return Ok(());
                        }
                        Err(error) => {
                            tracing::warn!(actor_id, %error, "could not inspect loop lifecycle; fencing executor");
                            manager.fence_loop_reservation(&current).await?;
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
        });
        if result.is_err() {
            self.release_loop_after_cleanup(
                &reservation.actor_id,
                None,
                LoopCleanupDisposition::StartupFailed,
            )
            .await?;
        }
        result
    }

    pub(super) async fn fence_loop_reservation(
        &self,
        expected: &LoopReservation,
    ) -> anyhow::Result<()> {
        // A startup may still spawn or publish its handle. Do not release its reservation
        // until the complete attempt has left the supervisor's startup critical section.
        while self.starting.lock().await.contains(&expected.actor_id) {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let mut reservations = self.loop_reservations.lock().await;
        let Some(current) = reservations
            .get(&expected.actor_id)
            .filter(|current| {
                current.generation == expected.generation && current.owner_id == expected.owner_id
            })
            .cloned()
        else {
            return Ok(());
        };
        // Keep the actor fence locked while stopping. A delayed renewal failure must never
        // look up and terminate whichever newer session happens to be running now.
        if let Some(session_id) = &current.session_id {
            self.process_supervisor.stop_session(session_id).await?;
        } else {
            self.process_supervisor
                .stop_actor(&current.actor_id)
                .await?;
        }
        LoopStore::new(self.db.clone())
            .cleanup_verified(
                &current,
                LoopCleanupDisposition::Exited,
                Utc::now().timestamp(),
            )
            .await?;
        reservations.remove(&current.actor_id);
        drop(reservations);
        if let Some(session_id) = &current.session_id {
            Self::finalize_process_exit(
                &self.db,
                &self.event_dbs,
                self.idle_gc.clone(),
                &self.inner,
                &self.push,
                &current.actor_id,
                session_id,
                false,
            )
            .await;
        }
        Ok(())
    }

    pub(super) async fn bind_loop_session(
        &self,
        actor_id: &str,
        session_id: &str,
    ) -> anyhow::Result<()> {
        let mut reservations = self.loop_reservations.lock().await;
        if let Some(current) = reservations.get_mut(actor_id) {
            *current = LoopStore::new(self.db.clone())
                .bind_session(current, session_id, Utc::now().timestamp())
                .await?;
        }
        Ok(())
    }

    /// Invoke only after the supervisor has stopped this session, or before any spawn was possible.
    pub(super) async fn release_loop_after_cleanup(
        &self,
        actor_id: &str,
        session_id: Option<&str>,
        disposition: LoopCleanupDisposition,
    ) -> anyhow::Result<()> {
        let mut reservations = self.loop_reservations.lock().await;
        let Some(current) = reservations.get(actor_id) else {
            return Ok(());
        };
        if session_id.is_some() && current.session_id.as_deref() != session_id {
            return Ok(());
        }
        LoopStore::new(self.db.clone())
            .cleanup_verified(current, disposition, Utc::now().timestamp())
            .await?;
        reservations.remove(actor_id);
        Ok(())
    }
}
