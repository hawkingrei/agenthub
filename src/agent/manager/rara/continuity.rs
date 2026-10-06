use std::path::Path;

use agenthub_agent_domain::loop_runtime::LoopReservation;
use agenthub_db::loop_runtime::LoopStore;
use agenthub_rara::{ControlRequest, Handshake};
use chrono::Utc;
use sha2::{Digest, Sha256};

use super::{AgentManager, NativeLoopContext, NativeLoopSources, RaraLaunchConfig};

impl NativeLoopSources {
    pub(in crate::agent::manager) fn new(
        launch: crate::acp::AcpLoopLaunchConfig,
        context: NativeLoopContext,
        workspace: &Path,
        prompt: &crate::agent::manager::loop_launch::RolePrompt,
        required_capabilities: Option<&serde_json::Value>,
        unavailable_memory: Option<&str>,
    ) -> anyhow::Result<Self> {
        let mut stable_launch = launch.clone();
        // Policy chooses how to open a conversation; changing fresh to resume must
        // not itself invalidate an otherwise identical conversation configuration.
        stable_launch.require_resume = false;
        let mut digest = Sha256::new();
        digest.update(stable_launch.fingerprint_material()?);
        digest.update(serde_json::to_vec(&(
            std::fs::canonicalize(workspace)?,
            &context.name,
            &context.card,
            &prompt.version,
            &prompt.entry,
            required_capabilities,
            unavailable_memory,
            super::LOOP_SOURCE_VERSION,
        ))?);
        Ok(Self {
            launch,
            context,
            continuity_digest: digest
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        })
    }
}

impl AgentManager {
    pub(super) async fn begin_native_conversation(
        &self,
        agent_id: &str,
        local_session_id: &str,
        config: &RaraLaunchConfig,
        handshake: &Handshake,
    ) -> anyhow::Result<(ControlRequest, Option<LoopReservation>)> {
        let reservation = self.loop_reservations.lock().await.get(agent_id).cloned();
        let Some(reservation) = reservation else {
            return Ok((ControlRequest::CreateSession, None));
        };
        anyhow::ensure!(
            reservation.session_id.as_deref() == Some(local_session_id),
            "native conversation local owner changed"
        );
        let source_digest = self
            .loop_credentials
            .lock()
            .await
            .get(agent_id)
            .and_then(|credentials| credentials.native_sources.as_ref())
            .map(|sources| sources.continuity_digest.clone())
            .ok_or_else(|| anyhow::anyhow!("native conversation configuration is missing"))?;
        let digest = configuration_digest(&source_digest, config, handshake)?;
        let native = LoopStore::new(self.db.clone())
            .begin_native_session(&reservation, &digest, Utc::now().timestamp())
            .await?;
        let request = match native {
            Some(session_id) => {
                handshake.require_methods(&["session.resume"])?;
                ControlRequest::ResumeSession { session_id }
            }
            None => ControlRequest::CreateSession,
        };
        Ok((request, Some(reservation)))
    }
}

fn configuration_digest(
    sources: &str,
    config: &RaraLaunchConfig,
    handshake: &Handshake,
) -> anyhow::Result<String> {
    Ok(Sha256::digest(serde_json::to_vec(&(
        sources,
        &config.binary,
        &config.default_provider,
        &config.default_model,
        handshake.protocol_version,
        &handshake.runtime_version,
        &handshake.provider,
        &handshake.model,
    ))?)
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::manager::loop_launch::RolePrompt;
    use agenthub_agent_domain::loop_runtime::LoopTaskContext;

    #[test]
    fn continuity_digest_ignores_activation_work_and_credential_rotation() {
        let directory =
            std::env::temp_dir().join(format!("native-continuity-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let context = NativeLoopContext {
            name: "Worker".into(),
            card: crate::team::TeamMemberCardRecord {
                card_id: "card".into(),
                schema_version: "1".into(),
                description: "Review changes".into(),
                role: "worker".into(),
                skills: vec![],
                capability_tags: vec![],
            },
            tasks: vec![],
            source_ids: vec!["trigger-one".into()],
        };
        let prompt = RolePrompt {
            version: "v1".into(),
            entry: "Review the assigned work".into(),
        };
        let make = |resume, credential: &str, fingerprint: &str| {
            let mut launch = crate::acp::AcpLoopLaunchConfig::resolve(directory.as_path(), resume);
            launch.install_loop_runtime_skill().unwrap();
            launch
                .add_mcp_proxy(
                    Path::new("/bin/proxy"),
                    Path::new(credential),
                    "mem",
                    fingerprint,
                )
                .unwrap();
            launch
        };
        let first = NativeLoopSources::new(
            make(false, "/private/one", &"a".repeat(64)),
            context.clone(),
            directory.as_path(),
            &prompt,
            None,
            None,
        )
        .unwrap();
        let mut next = context.clone();
        next.source_ids = vec!["trigger-two".into()];
        next.tasks.push(LoopTaskContext {
            task_id: "task".into(),
            title: "New work".into(),
            summary: None,
            memory_prefix: "scoped".into(),
        });
        let second = NativeLoopSources::new(
            make(true, "/private/two", &"a".repeat(64)),
            next,
            directory.as_path(),
            &prompt,
            None,
            None,
        )
        .unwrap();
        assert_eq!(first.continuity_digest, second.continuity_digest);
        for change in 0..5 {
            let mut context = context.clone();
            let mut prompt = RolePrompt {
                version: prompt.version.clone(),
                entry: prompt.entry.clone(),
            };
            let mut workspace = directory.as_path().to_path_buf();
            let mut fingerprint = "a".repeat(64);
            let mut unavailable = None;
            match change {
                0 => context.card.description = "Different responsibility".into(),
                1 => prompt.entry = "Different entry".into(),
                2 => {
                    workspace = directory.as_path().join("other");
                    std::fs::create_dir(&workspace).unwrap();
                }
                3 => fingerprint = "b".repeat(64),
                _ => unavailable = Some("missing-binding"),
            }
            let changed = NativeLoopSources::new(
                make(false, "/private/one", &fingerprint),
                context,
                &workspace,
                &prompt,
                None,
                unavailable,
            )
            .unwrap();
            assert_ne!(first.continuity_digest, changed.continuity_digest);
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}
