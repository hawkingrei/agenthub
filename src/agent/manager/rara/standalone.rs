use agenthub_db::native_sessions::{NativeExecutionOwner, NativeSessionStore};
use agenthub_rara::{ControlRequest, Handshake};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;

use super::*;

#[derive(Clone)]
pub(super) struct StandaloneOwner {
    pub store: NativeSessionStore,
    pub reservation: NativeExecutionOwner,
    pub operations: Arc<RwLock<()>>,
}

impl AgentManager {
    #[cfg(target_os = "linux")]
    pub(in crate::agent::manager) async fn prepare_standalone_native_execution(
        &self,
        agent: &str,
        local: &str,
    ) -> anyhow::Result<Arc<crate::executor_guardian::CleanupWitness>> {
        self.cleanup_standalone_native_execution(agent, None)
            .await?;
        let store = NativeSessionStore::new(self.db.clone());
        let owner = store
            .reserve(
                agent,
                local,
                &self.loop_owner_id,
                chrono::Utc::now().timestamp(),
            )
            .await?;
        let witness = Arc::new(
            crate::executor_guardian::CleanupWitness::prepare_standalone(
                self.event_dbs.base_dir(),
                &owner,
            )?,
        );
        store
            .authorize_spawn(&owner, chrono::Utc::now().timestamp())
            .await?;
        Ok(witness)
    }

    pub(in crate::agent::manager) async fn cleanup_standalone_native_execution(
        &self,
        agent: &str,
        local: Option<&str>,
    ) -> anyhow::Result<()> {
        let _operations = self.loop_operation_gate(agent).await.write_owned().await;
        let store = NativeSessionStore::new(self.db.clone());
        let Some(record) = store.active_owner(agent).await? else {
            return Ok(());
        };
        if local.is_some_and(|local| local != record.owner.local_session_id) {
            return Ok(());
        }
        let owner = &record.owner;
        let now = chrono::Utc::now().timestamp();
        if store.cleanup_unstarted(owner, now).await? {
            #[cfg(target_os = "linux")]
            crate::executor_guardian::CleanupWitness::retire_standalone(
                self.event_dbs.base_dir(),
                owner,
            );
            return Ok(());
        }
        #[cfg(not(target_os = "linux"))]
        anyhow::bail!("standalone native recovery requires a Linux executor guardian");
        #[cfg(target_os = "linux")]
        {
            let _proof = crate::executor_guardian::CleanupWitness::verify_standalone(
                self.event_dbs.base_dir(),
                owner,
            )?
            .ok_or(agenthub_db::native_sessions::NativeSessionError::ReservationHeld)?;
            self.permissions
                .interrupt_session_permissions(&owner.local_session_id)
                .await?;
            let pool = self.event_dbs.pool_for_agent(agent).await?;
            if let Some(events) =
                agenthub_db::runtime_events::RuntimeEventStore::load(pool, &owner.local_session_id)
                    .await?
            {
                events.close(now).await?;
            }
            store.cleanup_verified(owner, now).await?;
            crate::executor_guardian::CleanupWitness::retire_standalone(
                self.event_dbs.base_dir(),
                owner,
            );
            Ok(())
        }
    }

    pub(super) async fn begin_standalone_native_conversation(
        &self,
        agent: &str,
        local: &str,
        config: &RaraLaunchConfig,
        handshake: &Handshake,
        workspace: &std::path::Path,
    ) -> anyhow::Result<(ControlRequest, Option<NativeExecutionOwner>)> {
        let store = NativeSessionStore::new(self.db.clone());
        let owner = store.active_owner(agent).await?;
        let Some(owner) = owner.map(|record| record.owner) else {
            anyhow::ensure!(
                !cfg!(target_os = "linux")
                    && config.standalone_session_policy
                        == agenthub_config::RaraSessionPolicy::Fresh,
                "standalone native execution owner is missing"
            );
            return Ok((ControlRequest::CreateSession, None));
        };
        anyhow::ensure!(
            owner.local_session_id == local && owner.owner_id == self.loop_owner_id,
            "standalone native execution owner changed"
        );
        let policy = match config.standalone_session_policy {
            agenthub_config::RaraSessionPolicy::Fresh => {
                agenthub_agent_domain::loop_runtime::LoopSessionPolicy::Fresh
            }
            agenthub_config::RaraSessionPolicy::Resume => {
                agenthub_agent_domain::loop_runtime::LoopSessionPolicy::Resume
            }
        };
        let sources: String = Sha256::digest(serde_json::to_vec(&(
            "standalone-native-v1",
            std::fs::canonicalize(workspace)?,
        ))?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
        let digest = super::continuity::configuration_digest(&sources, config, handshake)?;
        if policy == agenthub_agent_domain::loop_runtime::LoopSessionPolicy::Resume {
            handshake.require_durable_resume()?;
            if let Some(previous) = store.opening_to_reconcile(&owner, &digest).await? {
                let pool = self.event_dbs.pool_for_agent(agent).await?;
                if let Some(events) =
                    agenthub_db::runtime_events::RuntimeEventStore::load(pool, &previous).await?
                    && let Some(evidence) = events.accepted_closed_opening().await?
                {
                    store
                        .reconcile_opening(
                            &owner,
                            &digest,
                            &evidence,
                            chrono::Utc::now().timestamp(),
                        )
                        .await?;
                }
            }
        }
        let native = store
            .begin_conversation(&owner, &digest, policy, chrono::Utc::now().timestamp())
            .await?;
        let request = native.map_or(ControlRequest::CreateSession, |session_id| {
            ControlRequest::ResumeSession { session_id }
        });
        Ok((request, Some(owner)))
    }

    pub(in crate::agent::manager) async fn clear_standalone_native_conversation(
        &self,
        agent: &str,
    ) -> anyhow::Result<()> {
        let _configuration = self.configuration_gate(agent).await.lock_owned().await;
        // A clear request cannot stop a live process as an implicit side effect.
        self.cleanup_standalone_native_execution(agent, None)
            .await?;
        NativeSessionStore::new(self.db.clone())
            .clear_conversation(agent)
            .await
    }
}
