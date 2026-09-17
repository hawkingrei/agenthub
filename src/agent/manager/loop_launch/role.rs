use agenthub_team_prompts::{LOOP_ROLE_PROMPT_VERSION, loop_prompt_for_role};
use serde_json::Value;

use super::{InternalRole, LOOP_ENTRY_PROMPT, LOOP_ENTRY_PROMPT_VERSION};

pub(in crate::agent::manager) struct RolePrompt {
    pub version: String,
    pub entry: String,
}

impl RolePrompt {
    pub fn resolve(spec: &Value, actor: &str, role: InternalRole) -> anyhow::Result<Self> {
        let member = spec["members"]
            .as_array()
            .and_then(|members| members.iter().find(|member| member["member_id"] == actor))
            .ok_or_else(|| anyhow::anyhow!("loop member configuration is missing"))?;
        anyhow::ensure!(
            member["role"].as_str() == Some(role.as_str()),
            "loop role changed before launch"
        );
        let configured = optional_text(member, "prompt")?;
        let append = optional_text(member, "prompt_append")?;
        let body = configured
            .or_else(|| loop_prompt_for_role(role.as_str()))
            .ok_or_else(|| anyhow::anyhow!("loop role prompt is unavailable"))?;
        let mut selected = body.to_owned();
        if let Some(append) = append {
            selected.push_str("\n\n");
            selected.push_str(append);
        }
        anyhow::ensure!(
            selected.len() <= 20_000,
            "loop role prompt exceeds its size limit"
        );
        let source = if configured.is_some() {
            "configured"
        } else {
            LOOP_ROLE_PROMPT_VERSION
        };
        Ok(Self {
            version: format!("{LOOP_ENTRY_PROMPT_VERSION}:{}:{source}", role.as_str()),
            entry: format!("{selected}\n\n{LOOP_ENTRY_PROMPT}"),
        })
    }
}

fn optional_text<'a>(member: &'a Value, key: &str) -> anyhow::Result<Option<&'a str>> {
    match member.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text.trim().is_empty() => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(_) => anyhow::bail!("loop member {key} must be a string"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn selects_only_the_configured_role_and_rejects_invalid_configuration() {
        let mut spec = json!({"members":[
            {"member_id":"leader", "role":"coordinator", "prompt":"Review deployment evidence."},
            {"member_id":"worker", "role":"worker", "prompt_append":"Verify the rollback artifact."}
        ]});
        let leader = RolePrompt::resolve(&spec, "leader", InternalRole::Coordinator).unwrap();
        assert!(leader.entry.starts_with("Review deployment evidence.\n\n"));
        assert!(leader.version.ends_with(":coordinator:configured"));
        assert!(!leader.entry.contains("Verify the rollback artifact."));
        let worker = RolePrompt::resolve(&spec, "worker", InternalRole::Worker).unwrap();
        assert!(worker.entry.starts_with("You are a Team Worker"));
        assert!(worker.entry.contains("Verify the rollback artifact."));
        assert!(!worker.entry.contains("Review deployment evidence."));
        assert!(RolePrompt::resolve(&spec, "worker", InternalRole::Coordinator).is_err());
        assert!(RolePrompt::resolve(&spec, "missing", InternalRole::Worker).is_err());
        for invalid in [json!(17), json!("x".repeat(20_001))] {
            spec["members"][1]["prompt"] = invalid;
            assert!(RolePrompt::resolve(&spec, "worker", InternalRole::Worker).is_err());
        }
        spec["members"][1]["prompt"] = json!(null);
        spec["members"][1]["prompt_append"] = json!({});
        assert!(RolePrompt::resolve(&spec, "worker", InternalRole::Worker).is_err());
    }
}
