use std::path::{Path, PathBuf};
use std::sync::Arc;

use agenthub_acp_core::AcpSkill;
use agenthub_managed_skills::{ManagedSkillKind, managed_skill_contents, managed_skill_name};

pub const LOOP_ACTIVATION_CONTRACT_VERSION: &str = "agenthub-loop-v1";

/// In-memory launch configuration. Resolve once; never persist skill bodies as trace data.
#[derive(Clone)]
pub struct AcpLoopLaunchConfig {
    pub require_resume: bool,
    pub mode_id: Option<String>,
    pub model_id: Option<String>,
    pub config: Vec<(String, String)>,
    pub(super) skills: Vec<AcpSkill>,
    runtime_skill: Option<Arc<LoopRuntimeSkill>>,
    mcp_proxies: Vec<McpProxyLauncher>,
}

struct LoopRuntimeSkill {
    directory: PathBuf,
    contents: String,
}

impl LoopRuntimeSkill {
    fn create() -> anyhow::Result<Self> {
        let directory =
            std::env::temp_dir().join(format!("agenthub-loop-skills-{}", uuid::Uuid::new_v4()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&directory)?;
        let skill = Self {
            directory,
            contents: managed_skill_contents(ManagedSkillKind::TeamLoopRuntime),
        };
        std::fs::write(skill.path(), &skill.contents)?;
        Ok(skill)
    }

    fn path(&self) -> PathBuf {
        self.directory.join("SKILL.md")
    }
}

impl Drop for LoopRuntimeSkill {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.path());
        let _ = std::fs::remove_dir(&self.directory);
    }
}

#[derive(Clone)]
struct McpProxyLauncher {
    executable: PathBuf,
    credential_file: PathBuf,
    server_id: String,
    binding_fingerprint: String,
}

impl AcpLoopLaunchConfig {
    pub fn resolve(workdir: &Path, require_resume: bool) -> Self {
        let mut skills = super::load_skills(workdir);
        // Legacy Team skills carry phase/watchdog instructions that conflict with loop mode.
        skills.retain(|skill| {
            !ManagedSkillKind::ALL
                .iter()
                .any(|kind| skill.name.eq_ignore_ascii_case(managed_skill_name(*kind)))
        });
        Self {
            require_resume,
            mode_id: None,
            model_id: None,
            config: Vec::new(),
            skills: super::dedupe_skills(skills),
            runtime_skill: None,
            mcp_proxies: Vec::new(),
        }
    }

    /// Pin the managed procedure in a private launch artifact, without rewriting a user's skills.
    pub fn install_loop_runtime_skill(&mut self) -> anyhow::Result<()> {
        if self.runtime_skill.is_some() {
            return Ok(());
        }
        let runtime = Arc::new(LoopRuntimeSkill::create()?);
        self.skills.push(agenthub_acp_core::build_skill(
            managed_skill_name(ManagedSkillKind::TeamLoopRuntime).into(),
            runtime.path().to_string_lossy().into_owned(),
            &runtime.contents,
        ));
        self.runtime_skill = Some(runtime);
        Ok(())
    }

    /// Only local proxy references cross into ACP; endpoints and upstream headers have no field.
    pub fn add_mcp_proxy(
        &mut self,
        executable: &Path,
        credential_file: &Path,
        server_id: &str,
        binding_fingerprint: &str,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.mcp_proxies.len() < 32
                && executable.is_absolute()
                && credential_file.is_absolute()
                && !server_id.is_empty()
                && server_id.len() <= 128
                && server_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
                && binding_fingerprint.len() == 64
                && binding_fingerprint
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
                && !self
                    .mcp_proxies
                    .iter()
                    .any(|proxy| proxy.server_id == server_id),
            "invalid MCP proxy launch reference"
        );
        self.mcp_proxies.push(McpProxyLauncher {
            executable: executable.into(),
            credential_file: credential_file.into(),
            server_id: server_id.into(),
            binding_fingerprint: binding_fingerprint.into(),
        });
        Ok(())
    }

    pub(super) fn mcp_servers(&self) -> Vec<agent_client_protocol::schema::v1::McpServer> {
        use agent_client_protocol::schema::v1::{EnvVariable, McpServer, McpServerStdio};
        self.mcp_proxies
            .iter()
            .map(|proxy| {
                McpServer::Stdio(
                    McpServerStdio::new(proxy.server_id.clone(), proxy.executable.clone())
                        .args(vec![
                            "mcp-proxy".into(),
                            "--server-id".into(),
                            proxy.server_id.clone(),
                        ])
                        .env(vec![EnvVariable::new(
                            "AGENTHUB_LOOP_CREDENTIAL_FILE",
                            proxy.credential_file.to_string_lossy(),
                        )]),
                )
            })
            .collect()
    }

    pub fn fingerprint_material(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(&serde_json::json!({
            "require_resume": self.require_resume,
            "mode_id": self.mode_id,
            "model_id": self.model_id,
            "config": self.config,
            "skills": self.skills.iter().filter(|skill| {
                skill.name != managed_skill_name(ManagedSkillKind::TeamLoopRuntime)
            }).map(|skill| (&skill.name, &skill.path, &skill.instructions)).collect::<Vec<_>>(),
            // The random artifact path is transport metadata, not a configuration revision.
            "runtime_skill": self.runtime_skill.as_ref().map(|skill| &skill.contents),
            "mcp_proxies": self.mcp_proxies.iter().map(|proxy| (&proxy.server_id, &proxy.executable, &proxy.binding_fingerprint)).collect::<Vec<_>>(),
        }))?)
    }
}

impl super::AcpActorSkillContext {
    pub fn is_loop_activation(&self) -> bool {
        self.contract_version.as_deref() == Some(LOOP_ACTIVATION_CONTRACT_VERSION)
    }
}

#[cfg(all(test, unix))]
mod tests;
