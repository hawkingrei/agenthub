use std::path::Path;

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
        }
    }

    pub fn fingerprint_material(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(&serde_json::json!({
            "require_resume": self.require_resume,
            "mode_id": self.mode_id,
            "model_id": self.model_id,
            "config": self.config,
            "skills": self.skills.iter().map(|skill| (&skill.name, &skill.path, &skill.instructions)).collect::<Vec<_>>(),
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
