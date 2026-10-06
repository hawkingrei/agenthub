use agenthub_agent_domain::loop_runtime::LoopReservation;
use agenthub_rara::SourceRegistration;
use serde_json::json;

use super::*;

#[derive(Clone)]
pub(in crate::agent::manager) struct NativeLoopSources {
    pub launch: crate::acp::AcpLoopLaunchConfig,
    pub context: NativeLoopContext,
    pub continuity_digest: String,
}

impl AgentManager {
    pub(in crate::agent::manager) async fn prepare_loop_entry(
        &self,
        teams: &crate::team::TeamManager,
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
        let mut work = Vec::with_capacity(pinned.context.source_ids.len());
        for source in &pinned.context.source_ids {
            work.push(teams.loop_work_source(reservation, source).await?);
        }
        let guard = agenthub_rara::SemanticGuardContext {
            role: context
                .member_role
                .clone()
                .ok_or_else(|| anyhow::anyhow!("native guard role is missing"))?,
            card: serde_json::to_string(&pinned.context.card)?,
            work: serde_json::to_string(&json!({"tasks":pinned.context.tasks,"sources":work}))?,
        };
        let task_id =
            (pinned.context.tasks.len() == 1).then(|| pinned.context.tasks[0].task_id.clone());
        let prompt = "Run the registered loop activation once using its role, recovery, and finish contract.";
        agenthub_rara::GuardedPrompt {
            prompt: prompt.into(),
            context: guard.clone(),
        }
        .validate()?;
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
        for server in pinned.launch.mcp_servers() {
            let agent_client_protocol::schema::v1::McpServer::Stdio(server) = server else {
                anyhow::bail!("native loop MCP sources require a local proxy");
            };
            sources.push(SourceRegistration::Mcp(agenthub_rara::McpSource {
                source_id: server.name,
                command: server
                    .command
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("native proxy executable must be UTF-8"))?
                    .to_owned(),
                args: server.args,
                env: server
                    .env
                    .into_iter()
                    .map(|entry| (entry.name, entry.value))
                    .collect(),
            }));
        }
        runtime.register_loop_sources(sources).await?;
        let request_id = format!(
            "loop-entry:{}:{}",
            reservation.activation_id.as_deref().unwrap_or_default(),
            reservation.generation
        );
        runtime
            .configure_loop_guard(guard, task_id, request_id)
            .await?;
        Ok(prompt.into())
    }
}
