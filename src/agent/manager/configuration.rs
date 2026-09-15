use std::future::Future;
use std::sync::Arc;

use tokio::sync::{Mutex, oneshot};

use super::AgentManager;

impl AgentManager {
    pub(super) async fn require_mutable_acp_configuration(
        &self,
        actor_id: &str,
    ) -> anyhow::Result<()> {
        let activation: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM loop_execution_reservations WHERE actor_id = ? AND activation_id IS NOT NULL)",
        ).bind(actor_id).fetch_one(&self.db).await?;
        anyhow::ensure!(
            !activation,
            agenthub_db::loop_runtime::LoopStoreError::ScopeBusy(
                "activation configuration is immutable; update the Card for a later activation"
            )
        );
        Ok(())
    }

    pub(super) async fn configuration_gate(&self, actor_id: &str) -> Arc<Mutex<()>> {
        self.configuration_gates
            .lock()
            .await
            .entry(actor_id.to_owned())
            .or_default()
            .clone()
    }

    /// Keep configuration and its start exclusion owned until the database operation settles.
    /// Callers must perform provider starts after this operation returns, not while holding it.
    pub(crate) async fn configure_actors_owned<T: Send + 'static>(
        &self,
        mut actor_ids: Vec<String>,
        operation: impl Future<Output = anyhow::Result<T>> + Send + 'static,
    ) -> anyhow::Result<T> {
        actor_ids.sort();
        actor_ids.dedup();
        let manager = self.clone();
        let (sender, receiver) = oneshot::channel();
        self.daemon_tasks
            .spawn_runtime_task("actor-configuration", async move {
                let mut guards = Vec::with_capacity(actor_ids.len());
                for actor_id in actor_ids {
                    guards.push(
                        manager
                            .configuration_gate(&actor_id)
                            .await
                            .lock_owned()
                            .await,
                    );
                }
                let result = operation.await;
                drop(guards);
                let _ = sender.send(result);
                Ok(())
            })?;
        receiver
            .await
            .map_err(|_| anyhow::anyhow!("actor configuration did not settle"))?
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn loop_configuration_survives_disconnect_and_excludes_a_start() {
        let state = crate::api::team_tests::build_test_state().await;
        let manager = state.agents.clone();
        let pool = state.db.clone();
        let (entered, ready) = oneshot::channel();
        let (release, released) = oneshot::channel();
        let request = tokio::spawn(async move {
            manager
                .configure_actors_owned(vec!["missing-actor".into()], async move {
                    entered.send(()).unwrap();
                    released.await.unwrap();
                    sqlx::query(
                        "UPDATE agents SET name = 'configuration-settled' WHERE id = 'planner'",
                    )
                    .execute(&pool)
                    .await?;
                    Ok(())
                })
                .await
        });
        ready.await.unwrap();
        request.abort();
        let manager = state.agents.clone();
        let mut start = tokio::spawn(async move { manager.start_agent("missing-actor").await });
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut start)
                .await
                .is_err()
        );
        release.send(()).unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(5), start)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert_eq!(
            state.agents.get_agent("planner").await.unwrap().name,
            "configuration-settled"
        );
    }

    #[tokio::test]
    async fn loop_configuration_orders_and_deduplicates_multi_actor_locks() {
        let state = crate::api::team_tests::build_test_state().await;
        let first = state
            .agents
            .configure_actors_owned(vec!["a".into(), "b".into(), "a".into()], async { Ok(1) });
        let second = state
            .agents
            .configure_actors_owned(vec!["b".into(), "a".into()], async { Ok(2) });
        let (first, second) = tokio::time::timeout(Duration::from_secs(5), async {
            tokio::join!(first, second)
        })
        .await
        .unwrap();
        assert_eq!((first.unwrap(), second.unwrap()), (1, 2));
    }
}
