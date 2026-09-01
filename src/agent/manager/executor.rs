use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use async_trait::async_trait;
use tokio::process::Command;

use crate::acp::{AcpActorSkillContext, AcpRuntimeLocation};

use super::supervisor::{
    AgentProcessSupervisor, PendingProcessRegistration, SharedSupervisedChild,
};
use super::{
    ACTOR_RUNTIME_ACTOR_ID_ENV, ACTOR_RUNTIME_CHANNEL_ENV, ACTOR_RUNTIME_CURRENT_RUN_ID_ENV,
    ACTOR_RUNTIME_TEAM_ID_ENV, ProxyPolicy,
};

#[derive(Debug, Clone)]
pub(super) struct LocalExecutionRequest {
    pub agent_id: String,
    pub session_id: String,
    pub command_path: String,
    pub args: Vec<String>,
    pub workdir: String,
    pub actor_context: Option<AcpActorSkillContext>,
    pub extra_env: Vec<(String, String)>,
}

pub(super) struct SpawnedLocalProcess {
    pub child: SharedSupervisedChild,
    pub registration: PendingProcessRegistration,
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
    process_supervisor: AgentProcessSupervisor,
}

impl LocalExecutor {
    pub(super) fn new(
        proxy_policy: ProxyPolicy,
        process_supervisor: AgentProcessSupervisor,
    ) -> Self {
        Self {
            proxy_policy,
            process_supervisor,
        }
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
        // Keep PATH-based resolution, but stabilize sibling helper discovery when
        // AgentHub itself was launched from a build output directory with the
        // daemon next to it.
        if let Some(path) = synthesized_child_path(
            &request.command_path,
            std::env::var_os("PATH"),
            std::env::current_exe().ok().as_deref(),
        ) {
            command.env("PATH", path);
        }
        self.proxy_policy.apply_to_command(&mut command);
        for (key, value) in &request.extra_env {
            command.env(key, value);
        }
        if let Some(context) = request.actor_context.as_ref() {
            command.env(ACTOR_RUNTIME_ACTOR_ID_ENV, &context.actor_id);
            command.env(ACTOR_RUNTIME_CHANNEL_ENV, &context.default_channel);
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

        let (child, registration) = self
            .process_supervisor
            .spawn(request.agent_id, request.session_id, command)
            .await?;
        Ok(SpawnedLocalProcess {
            child,
            registration,
            runtime_location: AcpRuntimeLocation::LocalProcess,
        })
    }
}

fn synthesized_child_path(
    command_path: &str,
    inherited_path: Option<OsString>,
    current_exe: Option<&Path>,
) -> Option<OsString> {
    if Path::new(command_path)
        .file_name()
        .and_then(|name| name.to_str())
        != Some(command_path)
    {
        return inherited_path;
    }
    let sibling_dir = resolve_runtime_sibling_bin_dir(current_exe)?;
    let mut segments: Vec<PathBuf> = inherited_path
        .as_deref()
        .map(std::env::split_paths)
        .map(|paths| paths.collect())
        .unwrap_or_default();
    if segments.iter().any(|path| path == &sibling_dir) {
        return inherited_path;
    }
    segments.insert(0, sibling_dir);
    std::env::join_paths(segments).ok()
}

fn resolve_runtime_sibling_bin_dir(current_exe: Option<&Path>) -> Option<PathBuf> {
    let current = current_exe?;
    let parent = current.parent()?;
    // Test binaries run from `target/*/deps`, but shipped runtime binaries live
    // together in the parent bin directory.
    let sibling_dir = if parent.file_name().and_then(|name| name.to_str()) == Some("deps") {
        parent.parent()?
    } else {
        parent
    };
    if sibling_dir.as_os_str().is_empty() {
        return None;
    }
    Some(sibling_dir.to_path_buf())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use uuid::Uuid;

    use super::{
        AgentExecutor, LocalExecutionRequest, LocalExecutor, resolve_runtime_sibling_bin_dir,
        synthesized_child_path,
    };
    use crate::agent::manager::ProxyPolicy;
    use crate::agent::manager::supervisor::AgentProcessSupervisor;

    fn temp_workdir(prefix: &str) -> std::path::PathBuf {
        let unique = Uuid::new_v4();
        std::env::temp_dir().join(format!("{prefix}-{unique}"))
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn local_executor_applies_extra_env_pairs() {
        let workdir = temp_workdir("agenthub-executor-extra-env");
        std::fs::create_dir_all(&workdir).expect("create temp workdir");

        let process_supervisor = AgentProcessSupervisor::default();
        let executor = LocalExecutor::new(ProxyPolicy::default(), process_supervisor.clone());
        let request = LocalExecutionRequest {
            agent_id: "agent-executor-test".to_string(),
            session_id: "session-executor-test".to_string(),
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
        spawned.registration.commit();
        let child = spawned.child.lock().await.take().expect("take child");
        let output = Box::into_pin(child.wait_with_output())
            .await
            .expect("wait for process output");

        assert!(output.status.success(), "process should exit successfully");
        let stdout = String::from_utf8(output.stdout).expect("decode stdout");
        assert_eq!(stdout.trim(), "RUST_BACKTRACE=1");
        process_supervisor.forget("session-executor-test").await;

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn local_executor_applies_proxy_policy_to_provider_child() {
        let workdir = temp_workdir("agenthub-executor-proxy-env");
        std::fs::create_dir_all(&workdir).expect("create temp workdir");

        let process_supervisor = AgentProcessSupervisor::default();
        let executor = LocalExecutor::new(
            ProxyPolicy::new(vec![
                (
                    "HTTP_PROXY".to_string(),
                    "http://proxy.example:8080".to_string(),
                ),
                (
                    "https_proxy".to_string(),
                    "http://secure-proxy.example:8443".to_string(),
                ),
            ]),
            process_supervisor.clone(),
        );
        let request = LocalExecutionRequest {
            agent_id: "agent-executor-proxy-test".to_string(),
            session_id: "session-executor-proxy-test".to_string(),
            command_path: "/bin/sh".to_string(),
            args: vec![
                "-c".to_string(),
                "printf 'HTTP_PROXY=%s\\nhttps_proxy=%s\\n' \"$HTTP_PROXY\" \"$https_proxy\""
                    .to_string(),
            ],
            workdir: workdir.to_string_lossy().to_string(),
            actor_context: None,
            extra_env: Vec::new(),
        };

        let spawned = executor
            .spawn_process(request)
            .await
            .expect("spawn process");
        spawned.registration.commit();
        let child = spawned.child.lock().await.take().expect("take child");
        let output = Box::into_pin(child.wait_with_output())
            .await
            .expect("wait for process output");

        assert!(output.status.success(), "process should exit successfully");
        let stdout = String::from_utf8(output.stdout).expect("decode stdout");
        assert_eq!(
            stdout,
            "HTTP_PROXY=http://proxy.example:8080\nhttps_proxy=http://secure-proxy.example:8443\n"
        );
        process_supervisor
            .forget("session-executor-proxy-test")
            .await;

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn synthesized_child_path_prepends_sibling_bin_dir_for_bare_commands() {
        let current_exe = PathBuf::from("/tmp/agenthub/target/debug/agenthub");
        let inherited = Some(OsString::from("/usr/bin:/bin"));

        let path = synthesized_child_path(
            "agenthub-codex-acp",
            inherited.clone(),
            Some(current_exe.as_path()),
        )
        .expect("synthesized path");

        let segments: Vec<PathBuf> = std::env::split_paths(&path).collect();
        assert_eq!(segments[0], PathBuf::from("/tmp/agenthub/target/debug"));
        assert_eq!(segments[1], PathBuf::from("/usr/bin"));
        assert_eq!(segments[2], PathBuf::from("/bin"));
    }

    #[test]
    fn synthesized_child_path_leaves_explicit_command_paths_unchanged() {
        let inherited = Some(OsString::from("/usr/bin:/bin"));
        let path = synthesized_child_path(
            "/tmp/bin/agenthub-codex-acp",
            inherited.clone(),
            Some(Path::new("/tmp/agenthub/target/debug/agenthub")),
        );

        assert_eq!(path, inherited);
    }

    #[test]
    fn resolve_runtime_sibling_bin_dir_uses_parent_of_deps_test_binary() {
        let current_exe = Path::new("/tmp/agenthub/target/debug/deps/agenthub-tests");
        let sibling_dir =
            resolve_runtime_sibling_bin_dir(Some(current_exe)).expect("resolve sibling dir");
        assert_eq!(sibling_dir, PathBuf::from("/tmp/agenthub/target/debug"));
    }
}
