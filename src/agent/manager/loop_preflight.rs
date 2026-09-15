use std::path::{Path, PathBuf};
use std::sync::Arc;

use agenthub_agent_domain::loop_runtime::LoopSessionPolicy;
use serde::Serialize;
use serde_json::Value;

use crate::team::TeamManager;

use super::{AgentManager, WorktreeMode};

const LOCAL_CAPABILITIES: &[&str] = &["actor_control", "structured_finish", "fresh_session"];

#[derive(Debug, Serialize)]
pub(crate) struct LoopPreflight {
    pub ready: bool,
    pub provider_id: Option<String>,
    pub capabilities: Vec<&'static str>,
    pub blockers: Vec<&'static str>,
    pub warnings: Vec<&'static str>,
}

impl AgentManager {
    pub(crate) fn with_loop_app_config(mut self, config: agenthub_config::AppConfig) -> Self {
        self.loop_app_config = Arc::new(config);
        self
    }

    pub(crate) async fn loop_preflight(
        &self,
        team_id: &str,
        spec: &Value,
        actor_id: &str,
        session_policy: LoopSessionPolicy,
    ) -> anyhow::Result<LoopPreflight> {
        let agent = self.get_agent(actor_id).await?;
        let mut blockers = Vec::new();
        let mut warnings = Vec::new();
        let mut capabilities = LOCAL_CAPABILITIES.to_vec();
        if !TeamManager::uses_loop_execution(spec) {
            blockers.push("team_loop_mode_required");
        }
        if !cfg!(target_os = "linux") {
            blockers.push("executor_guardian_unavailable");
        }
        if agent.target_node_id.is_some() {
            blockers.push("remote_loop_unsupported");
        }
        let member = spec
            .get("members")
            .and_then(Value::as_array)
            .and_then(|members| {
                members.iter().find(|member| {
                    member.get("member_id").and_then(Value::as_str) == Some(actor_id)
                })
            });
        let role = member
            .and_then(|member| member.get("role"))
            .and_then(Value::as_str);
        if !matches!(role, Some("coordinator" | "worker")) {
            blockers.push("member_role_required");
        }
        let scopes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM team_definitions t WHERE EXISTS(SELECT 1 FROM json_each(t.spec_json, '$.members') m WHERE json_extract(m.value, '$.member_id') = ?)")
            .bind(actor_id).fetch_one(&self.db).await?;
        if scopes != 1 {
            blockers.push("unique_team_membership_required");
        }
        let provider = self.acp_provider_spec_for_agent(&agent.command, &agent.args);
        if provider.is_none() {
            blockers.push("local_acp_provider_required");
        }
        if self.loop_control_endpoint.read().await.is_none() {
            blockers.push("actor_control_unavailable");
        }
        let workdir = super::expand_tilde(&agent.workdir);
        if !Path::new(&workdir).is_dir()
            && (!matches!(agent.worktree_mode, WorktreeMode::CreateWorktree)
                || !Path::new(&workdir).parent().is_some_and(Path::is_dir))
        {
            blockers.push("workspace_unavailable");
        }
        if matches!(agent.worktree_mode, WorktreeMode::CreateWorktree)
            && !agent
                .worktree_repo
                .as_deref()
                .map(super::expand_tilde)
                .is_some_and(|repo| Path::new(&repo).is_dir())
        {
            blockers.push("worktree_repository_unavailable");
        }
        if let Some(role) = role {
            let context = crate::team::build_team_member_actor_context_for_role(
                team_id, None, actor_id, role,
            );
            let repo = agent.worktree_repo.as_deref().map(super::expand_tilde);
            if super::build_runtime_start_policy(
                &agent,
                Some(&context),
                &workdir,
                repo.as_deref(),
                None,
            )
            .is_err()
            {
                blockers.push("workspace_policy_invalid");
            }
        }
        let (command, _) = self.resolve_launch_command(&agent.command, &agent.args, provider);
        if executable_path(&command, Path::new(&workdir)).is_none() {
            blockers.push("provider_binary_unavailable");
        }
        if (agent.runtime_model.is_some() || agent.thinking_level.is_some())
            && !provider.is_some_and(|provider| matches!(provider.id, "codex" | "claude"))
        {
            blockers.push("runtime_profile_unsupported");
        }
        if session_policy == LoopSessionPolicy::Resume {
            warnings.push("resume_capability_is_negotiated_before_entry");
        }
        if crate::mcp_proxy::configured::has_mem_binding(&self.loop_app_config, team_id) {
            if crate::mcp_proxy::configured::resolve_mem(
                &self.loop_app_config,
                team_id,
                actor_id,
                |key| std::env::var(key).ok(),
            )
            .is_err()
            {
                blockers.push("mem_binding_unavailable");
            } else if self.mcp_proxy().is_err()
                || crate::mcp_proxy::configured::shim_executable().is_err()
            {
                blockers.push("mem_proxy_unavailable");
            } else {
                capabilities.push("nowledge_mem");
            }
        }
        if let Some(required) = spec.get("required_capabilities") {
            if let Some(required) = required.as_array() {
                for capability in required {
                    match capability.as_str() {
                        Some(capability) if capabilities.contains(&capability) => {}
                        Some("nowledge_mem") => {
                            blockers.push("mem_binding_unavailable");
                            if self.mcp_proxy().is_err()
                                || crate::mcp_proxy::configured::shim_executable().is_err()
                            {
                                blockers.push("mem_proxy_unavailable");
                            }
                        }
                        _ => blockers.push("required_capability_unavailable"),
                    }
                }
            } else {
                blockers.push("required_capabilities_invalid");
            }
        }
        let reservation: Option<(String, i64)> = sqlx::query_as(
            "SELECT owner_id, lease_expires_at FROM loop_execution_reservations WHERE actor_id = ?",
        )
        .bind(actor_id)
        .fetch_optional(&self.db)
        .await?;
        if reservation.as_ref().is_some_and(|(owner, expires_at)| {
            owner != self.loop_owner_id() || *expires_at <= chrono::Utc::now().timestamp()
        }) {
            blockers.push("unfenced_executor_retained");
        }
        if reservation.is_none()
            && (self.process_supervisor.has_actor_process(actor_id).await
                || self.inner.read().await.contains_key(actor_id))
        {
            blockers.push("legacy_executor_must_stop");
        }
        blockers.sort_unstable();
        blockers.dedup();
        Ok(LoopPreflight {
            ready: blockers.is_empty(),
            provider_id: provider.map(|provider| provider.id.into()),
            capabilities,
            blockers,
            warnings,
        })
    }
}

fn executable_path(command: &str, workdir: &Path) -> Option<PathBuf> {
    let executable = |path: &Path| {
        let Ok(metadata) = path.metadata() else {
            return false;
        };
        if !metadata.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            true
        }
    };
    if Path::new(command).is_absolute() || Path::new(command).components().count() > 1 {
        let path = workdir.join(command);
        return executable(&path).then_some(path);
    }
    let path = super::executor::synthesized_child_path(
        "agenthub",
        std::env::var_os("PATH"),
        std::env::current_exe().ok().as_deref(),
    )?;
    std::env::split_paths(&path)
        .map(|directory| workdir.join(directory).join(command))
        .find(|path| executable(path))
}
