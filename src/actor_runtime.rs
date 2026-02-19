use std::path::PathBuf;

use anyhow::{Context, ensure};

use crate::acp::AcpActorSkillContext;

pub(crate) const DEFAULT_ACTOR_CHANNEL: &str = "default";

fn pathbuf_to_utf8(path: PathBuf, field: &str) -> anyhow::Result<String> {
    path.into_os_string()
        .into_string()
        .map_err(|_| anyhow::anyhow!("{field} must be valid UTF-8"))
}

fn canonicalize_actor_cli_path(path: &str) -> anyhow::Result<PathBuf> {
    let trimmed = path.trim();
    ensure!(
        !trimmed.is_empty(),
        "actor_runtime.actor_cli_path must not be empty"
    );
    let input = PathBuf::from(trimmed);
    let absolute = if input.is_absolute() {
        input
    } else {
        std::env::current_dir()
            .context("resolve current directory for actor_runtime.actor_cli_path")?
            .join(input)
    };
    std::fs::canonicalize(&absolute)
        .with_context(|| format!("actor_runtime.actor_cli_path is invalid: {}", trimmed))
}

pub(crate) fn default_actor_cli_path() -> anyhow::Result<String> {
    let exe = std::env::current_exe().context("resolve current executable for actor cli")?;
    let canonical = std::fs::canonicalize(&exe).with_context(|| {
        format!(
            "resolve canonical executable path for actor cli: {}",
            exe.display()
        )
    })?;
    pathbuf_to_utf8(canonical, "actor_runtime.actor_cli_path")
}

pub(crate) fn normalize_actor_cli_path(raw: Option<&str>) -> anyhow::Result<String> {
    let default_path = default_actor_cli_path()?;
    let default_canonical = canonicalize_actor_cli_path(&default_path)?;
    let configured = raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_path.as_str());
    let configured_canonical = canonicalize_actor_cli_path(configured)?;

    ensure!(
        configured_canonical == default_canonical,
        "actor_runtime.actor_cli_path is not allowed"
    );
    pathbuf_to_utf8(configured_canonical, "actor_runtime.actor_cli_path")
}

pub(crate) fn normalize_actor_context(
    context: AcpActorSkillContext,
) -> anyhow::Result<AcpActorSkillContext> {
    let default_channel = if context.default_channel.trim().is_empty() {
        DEFAULT_ACTOR_CHANNEL.to_string()
    } else {
        context.default_channel.trim().to_string()
    };
    let actor_cli_path = normalize_actor_cli_path(Some(context.actor_cli_path.as_str()))?;
    let member_role = context
        .member_role
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    Ok(AcpActorSkillContext {
        run_id: context.run_id.trim().to_string(),
        actor_id: context.actor_id.trim().to_string(),
        default_channel,
        actor_cli_path,
        member_role,
    })
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_ACTOR_CHANNEL, default_actor_cli_path, normalize_actor_cli_path};

    #[test]
    fn default_actor_cli_path_resolves_existing_binary() {
        let path = default_actor_cli_path().expect("resolve default actor cli path");
        assert!(!path.trim().is_empty());
    }

    #[test]
    fn normalize_actor_cli_path_accepts_default_path() {
        let default_path = default_actor_cli_path().expect("resolve default actor cli path");
        let normalized = normalize_actor_cli_path(Some(default_path.as_str()))
            .expect("normalize default actor cli path");
        assert!(!normalized.is_empty());
    }

    #[test]
    fn normalize_actor_cli_path_rejects_non_default_path() {
        let err = normalize_actor_cli_path(Some("/tmp/not-agenthub-cli"))
            .expect_err("non-default actor cli path should be rejected");
        let message = err.to_string();
        assert!(
            message.contains("invalid") || message.contains("not allowed"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn default_channel_constant_is_stable() {
        assert_eq!(DEFAULT_ACTOR_CHANNEL, "default");
    }
}
