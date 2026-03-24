use agent_client_protocol::{ContentBlock, TextContent};
use agenthub_acp_core::{AcpSkill, build_skill};
use agenthub_managed_skills::{
    ManagedSkillKind, managed_skill_contents, managed_skill_doc, managed_skill_name,
};
use agenthub_text::truncate_chars;

use crate::AcpActorSkillContext;

const FALLBACK_ACTOR_RUNTIME_SKILL_PATH: &str = "builtin://agenthub/actor-runtime";

pub(super) fn build_actor_runtime_skill() -> AcpSkill {
    match managed_skill_doc(ManagedSkillKind::ActorRuntime, None) {
        Ok(doc) if doc.path.exists() => build_skill(
            doc.name.to_string(),
            doc.path.to_string_lossy().to_string(),
            &doc.contents,
        ),
        Ok(doc) => build_skill(
            doc.name.to_string(),
            FALLBACK_ACTOR_RUNTIME_SKILL_PATH.to_string(),
            &doc.contents,
        ),
        Err(_) => build_skill(
            managed_skill_name(ManagedSkillKind::ActorRuntime).to_string(),
            FALLBACK_ACTOR_RUNTIME_SKILL_PATH.to_string(),
            managed_skill_contents(ManagedSkillKind::ActorRuntime).as_str(),
        ),
    }
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
    use super::{
        AcpActorSkillContext, build_actor_runtime_context_block, build_actor_runtime_skill,
    };
    use agent_client_protocol::ContentBlock;

    #[test]
    fn actor_runtime_skill_uses_static_name() {
        let skill = build_actor_runtime_skill();
        assert_eq!(skill.name, "agenthub-actor-runtime");
        assert!(skill.instructions.contains("AgentHub Actor Runtime Skill"));
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
