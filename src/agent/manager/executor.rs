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
    pub extra_env: Vec<(String, String)>,
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
        for (key, value) in &request.extra_env {
            command.env(key, value);
        }
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

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{AgentExecutor, LocalExecutionRequest, LocalExecutor};
    use crate::agent::manager::ProxyPolicy;

    fn temp_workdir(prefix: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{unique}"))
    }

    #[tokio::test]
    async fn local_executor_applies_extra_env_pairs() {
        let workdir = temp_workdir("agenthub-executor-extra-env");
        std::fs::create_dir_all(&workdir).expect("create temp workdir");

        let executor = LocalExecutor::new(ProxyPolicy::default());
        let request = LocalExecutionRequest {
            command_path: "/bin/sh".to_string(),
            args: vec![
                "-c".to_string(),
                "printf 'RUST_BACKTRACE=%s\\n' \"$RUST_BACKTRACE\"".to_string(),
            ],
            workdir: workdir.to_string_lossy().to_string(),
            actor_context: None,
            extra_env: vec![("RUST_BACKTRACE".to_string(), "1".to_string())],
        };

        let spawned = executor
            .spawn_process(request)
            .await
            .expect("spawn process");
        let output = spawned
            .child
            .wait_with_output()
            .await
            .expect("wait for process output");

        assert!(output.status.success(), "process should exit successfully");
        let stdout = String::from_utf8(output.stdout).expect("decode stdout");
        assert_eq!(stdout.trim(), "RUST_BACKTRACE=1");

        let _ = std::fs::remove_dir_all(&workdir);
    }
}
