use std::path::{Path, PathBuf};

use agenthub_acp_core::AcpSkill;

pub const LOOP_ACTIVATION_CONTRACT_VERSION: &str = "agenthub-loop-v1";

/// In-memory launch configuration. Resolve once; never persist skill bodies as trace data.
#[derive(Clone)]
pub struct AcpLoopLaunchConfig {
    pub require_resume: bool,
    pub mode_id: Option<String>,
    pub model_id: Option<String>,
    pub config: Vec<(String, String)>,
    pub(super) skills: Vec<AcpSkill>,
    mcp_proxies: Vec<McpProxyLauncher>,
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
        skills.retain(|skill| !super::is_reserved_team_role_skill(&skill.name));
        Self {
            require_resume,
            mode_id: None,
            model_id: None,
            config: Vec::new(),
            skills: super::dedupe_skills(skills),
            mcp_proxies: Vec::new(),
        }
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
            self.mcp_proxies.len() < 8
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
            "skills": self.skills.iter().map(|skill| (&skill.name, &skill.path, &skill.instructions)).collect::<Vec<_>>(),
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
