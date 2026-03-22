use std::process::Stdio;

use async_trait::async_trait;
use tokio::process::{Child, Command};

use crate::acp::{AcpActorSkillContext, AcpRuntimeLocation};

use super::{
    ACTOR_RUNTIME_ACTOR_ID_ENV, ACTOR_RUNTIME_CHANNEL_ENV, ACTOR_RUNTIME_CLI_ENV,
    ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, ACTOR_RUNTIME_TEAM_ID_ENV, ProxyPolicy,
};

#[derive(Debug, Clone)]
pub(super) struct LocalExecutionRequest {
    pub command_path: String,
    pub args: Vec<String>,
    pub workdir: String,
    pub actor_context: Option<AcpActorSkillContext>,
}

pub(super) struct SpawnedLocalProcess {
    pub child: Child,
    pub runtime_location: AcpRuntimeLocation,
}

#[async_trait]
pub(super) trait AgentExecutor: Send + Sync {
    async fn spawn_process(
        &self,
        request: LocalExecutionRequest,
    ) -> anyhow::Result<SpawnedLocalProcess>;
}

#[derive(Debug, Clone)]
pub(super) struct LocalExecutor {
    proxy_policy: ProxyPolicy,
}

impl LocalExecutor {
    pub(super) fn new(proxy_policy: ProxyPolicy) -> Self {
        Self { proxy_policy }
    }
}

#[async_trait]
impl AgentExecutor for LocalExecutor {
    async fn spawn_process(
        &self,
        request: LocalExecutionRequest,
    ) -> anyhow::Result<SpawnedLocalProcess> {
        let mut command = Command::new(&request.command_path);
        command
            .current_dir(&request.workdir)
            .args(&request.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.proxy_policy.apply_to_command(&mut command);
        if let Some(context) = request.actor_context.as_ref() {
            command.env(ACTOR_RUNTIME_ACTOR_ID_ENV, &context.actor_id);
            command.env(ACTOR_RUNTIME_CHANNEL_ENV, &context.default_channel);
            command.env(ACTOR_RUNTIME_CLI_ENV, &context.actor_cli_path);
            if let Some(team_id) = context
                .team_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                command.env(ACTOR_RUNTIME_TEAM_ID_ENV, team_id);
            }
            if let Some(run_id) = context
                .current_run_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                command.env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, run_id);
            }
        }

        let child = command.spawn()?;
        Ok(SpawnedLocalProcess {
            child,
            runtime_location: AcpRuntimeLocation::LocalProcess,
        })
    }
}
