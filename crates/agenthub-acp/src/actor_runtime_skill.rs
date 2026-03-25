use std::path::Path;

use agent_client_protocol::{ContentBlock, TextContent};
use agenthub_acp_core::{AcpSkill, build_skill};
use agenthub_managed_skills::{ManagedSkillKind, managed_skill_doc};
use agenthub_text::truncate_chars;
use anyhow::{Result, bail};

use crate::AcpActorSkillContext;

pub(super) fn build_required_managed_skill(
    kind: ManagedSkillKind,
    home_dir: Option<&Path>,
) -> Result<AcpSkill> {
    let doc = managed_skill_doc(kind, home_dir)?;
    if !doc.path.exists() {
        bail!(
            "managed skill '{}' is not materialized at {}; run `agenthub doctor` or fix managed skill installation before starting the actor runtime",
            doc.name,
            doc.path.display()
        );
    }
    Ok(build_skill(
        doc.name.to_string(),
        doc.path.to_string_lossy().to_string(),
        &doc.contents,
    ))
}

pub(super) fn build_actor_runtime_skill() -> Result<AcpSkill> {
    build_required_managed_skill(ManagedSkillKind::ActorRuntime, None)
}

pub(super) fn build_actor_runtime_context_block(context: &AcpActorSkillContext) -> ContentBlock {
    ContentBlock::Text(TextContent::new(build_actor_runtime_context_text(context)))
}

fn build_actor_runtime_context_text(context: &AcpActorSkillContext) -> String {
    let mut lines = vec![
        "AgentHub runtime context:".to_string(),
        "- session_scope: Team member runtime".to_string(),
        format!("- actor_id: {}", context.actor_id),
        format!("- default_channel: {}", context.default_channel),
        format!("- actor_cli_path: {}", context.actor_cli_path),
    ];
    if let Some(team_id) = context
        .team_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("- team_id: {team_id}"));
    }
    if let Some(run_id) = context
        .current_run_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("- current_run_id: {run_id}"));
    } else {
        lines.push("- current_run_id: n/a".to_string());
    }
    if let Some(member_role) = context
        .member_role
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("- member_role: {member_role}"));
    }
    if let Some(contract_version) = context
        .contract_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("- contract_version: {contract_version}"));
    }
    lines.extend(build_continuity_lines(context));
    lines.push(
        "Treat this block as the authoritative runtime identity and run-scope context for the current turn."
            .to_string(),
    );
    lines.join("\n")
}

fn build_continuity_lines(context: &AcpActorSkillContext) -> Vec<String> {
    let Some(continuity) = context.continuity.as_ref() else {
        return Vec::new();
    };
    let summary = truncate_chars(continuity.summary_text.as_str(), 400);
    let history_window = truncate_chars(continuity.history_window.to_string().as_str(), 800);
    let source_session = continuity.source_session_id.as_deref().unwrap_or("n/a");
    vec![
        format!("- continuity_mode: {}", continuity.mode),
        format!("- continuity_source_run_id: {}", continuity.source_run_id),
        format!("- continuity_source_session_id: {source_session}"),
        format!("- continuity_summary: {summary}"),
        format!("- continuity_history_window_json: {history_window}"),
    ]
}

#[cfg(test)]
mod tests {
    use std::fs;

    use agenthub_managed_skills::{ManagedSkillKind, install_managed_skills};

    use super::{
        AcpActorSkillContext, build_actor_runtime_context_block, build_required_managed_skill,
    };
    use agent_client_protocol::ContentBlock;
    use uuid::Uuid;

    struct TempManagedSkillsHome {
        root: std::path::PathBuf,
    }

    impl TempManagedSkillsHome {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "agenthub-acp-managed-skill-home-{}",
                Uuid::new_v4()
            ));
            fs::create_dir_all(&root).expect("create temp managed skills home");
            Self { root }
        }

        fn path(&self) -> &std::path::Path {
            &self.root
        }
    }

    impl Drop for TempManagedSkillsHome {
        fn drop(&mut self) {
            if self.root.exists() {
                let _ = fs::remove_dir_all(&self.root);
            }
        }
    }

    #[test]
    fn actor_runtime_skill_uses_static_name() {
        let home = TempManagedSkillsHome::new();
        install_managed_skills(Some(home.path())).expect("install managed skills");
        let skill = build_required_managed_skill(ManagedSkillKind::ActorRuntime, Some(home.path()))
            .expect("build actor runtime skill");
        assert_eq!(skill.name, "agenthub-actor-runtime");
        assert!(skill.instructions.contains("AgentHub Actor Runtime Skill"));
    }

    #[test]
    fn required_managed_skill_errors_when_not_materialized() {
        let home = TempManagedSkillsHome::new();
        let err = build_required_managed_skill(ManagedSkillKind::ActorRuntime, Some(home.path()))
            .expect_err("missing managed skill should hard fail");
        assert!(
            err.to_string().contains("is not materialized"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn actor_runtime_context_block_includes_dynamic_session_fields() {
        let block = build_actor_runtime_context_block(&AcpActorSkillContext {
            team_id: Some("team-7".to_string()),
            current_run_id: Some("run-42".to_string()),
            actor_id: "planner".to_string(),
            default_channel: "coordination".to_string(),
            actor_cli_path: "/tmp/agenthub".to_string(),
            member_role: Some("leader".to_string()),
            member_skills: Vec::new(),
            contract_version: Some("v1".to_string()),
            continuity: None,
        });
        let ContentBlock::Text(text) = block else {
            panic!("expected text content block");
        };
        assert!(text.text.contains("team_id: team-7"));
        assert!(text.text.contains("current_run_id: run-42"));
        assert!(text.text.contains("actor_id: planner"));
        assert!(text.text.contains("default_channel: coordination"));
        assert!(text.text.contains("contract_version: v1"));
        assert!(text.text.contains("authoritative runtime identity"));
    }
}
