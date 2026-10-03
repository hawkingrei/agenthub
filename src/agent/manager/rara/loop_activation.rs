use agenthub_agent_domain::loop_runtime::LoopReservation;
use agenthub_rara::SourceRegistration;
use serde_json::json;

use super::*;

#[derive(Clone)]
pub(in crate::agent::manager) struct NativeLoopSources {
    pub launch: crate::acp::AcpLoopLaunchConfig,
    pub context: NativeLoopContext,
}

impl AgentManager {
    pub(in crate::agent::manager) async fn prepare_loop_entry(
        &self,
        reservation: &LoopReservation,
        context: &AcpActorSkillContext,
        entry: String,
    ) -> anyhow::Result<String> {
        let runtime = {
            let handles = self.inner.read().await;
            let handle = handles
                .get(&reservation.actor_id)
                .filter(|handle| Some(&handle.session_id) == reservation.session_id.as_ref())
                .ok_or_else(|| anyhow::anyhow!("direct loop session ownership is unavailable"))?;
            match &handle.input {
                AgentInput::Rara(runtime) => runtime.clone(),
                AgentInput::Acp(_) => return Ok(entry),
                AgentInput::Stdin(_) => anyhow::bail!("loop entry requires a supported runtime"),
            }
        };
        let pinned = self
            .loop_credentials
            .lock()
            .await
            .get(&reservation.actor_id)
            .and_then(|credentials| credentials.native_sources.clone())
            .ok_or_else(|| anyhow::anyhow!("direct loop source configuration is unavailable"))?;
        let binding = json!({
            "version": LOOP_SOURCE_VERSION,
            "activation_id": reservation.activation_id,
            "generation": reservation.generation,
            "local_session_id": reservation.session_id,
            "outer_actor": context,
            "name": pinned.context.name,
            "card": pinned.context.card,
        });
        let mut sources = vec![SourceRegistration::Prompt {
            source_id: "loop-activation-context-v2".into(),
            content: format!(
                "{entry}\n\nOuter activation binding: {binding}\nNative subagents execute within this outer activation. Their native identities are not Team members, mailbox identities, task owners, or independent activation credentials."
            ),
        }];
        for (index, task) in pinned.context.tasks.iter().enumerate() {
            sources.push(SourceRegistration::Prompt {
                source_id: format!("loop-task-context-{index}"),
                content: serde_json::to_string(task)?,
            });
        }
        for (index, (name, content)) in pinned.launch.inline_skill_sources().into_iter().enumerate()
        {
            sources.push(SourceRegistration::Skill {
                source_id: format!("loop-skill-{index}"),
                name,
                content,
            });
        }
        runtime.register_loop_sources(sources).await?;
        Ok("Run the registered loop activation once using its role, recovery, and finish contract.".into())
    }
}
