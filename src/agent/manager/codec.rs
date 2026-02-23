use std::path::Path;

use crate::agent::{AgentStatus, OutputStream, WorktreeMode};

use super::{
    ACP_PROVIDER_CODEX, ACP_PROVIDER_GEMINI, ACP_PROVIDER_KIMI, ACP_PROVIDER_LINKERDOG,
    AgentManager,
};

#[derive(Clone, Copy)]
struct AcpProviderSpec {
    id: &'static str,
    command_names: &'static [&'static str],
    require_args: Option<fn(&[String]) -> bool>,
    use_configured_binary: bool,
}

const ACP_PROVIDER_SPECS: &[AcpProviderSpec] = &[
    AcpProviderSpec {
        id: ACP_PROVIDER_GEMINI,
        command_names: &["gemini"],
        require_args: Some(requires_gemini_acp_flag),
        use_configured_binary: false,
    },
    AcpProviderSpec {
        id: ACP_PROVIDER_KIMI,
        command_names: &["kimi"],
        require_args: Some(requires_acp_subcommand),
        use_configured_binary: false,
    },
    AcpProviderSpec {
        id: ACP_PROVIDER_LINKERDOG,
        command_names: &["linkerdog"],
        require_args: Some(requires_acp_subcommand),
        use_configured_binary: false,
    },
    AcpProviderSpec {
        id: ACP_PROVIDER_LINKERDOG,
        command_names: &["linkerdog-acp"],
        require_args: None,
        use_configured_binary: false,
    },
    AcpProviderSpec {
        id: ACP_PROVIDER_CODEX,
        command_names: &["agenthub-codex-acp", "codex-acp"],
        require_args: None,
        use_configured_binary: true,
    },
];

fn requires_gemini_acp_flag(args: &[String]) -> bool {
    args.iter()
        .any(|arg| arg == "--experimental-acp" || arg.starts_with("--experimental-acp="))
}

fn requires_acp_subcommand(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "acp")
}

pub(super) fn status_to_str(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Created => "created",
        AgentStatus::Running => "running",
        AgentStatus::Stopped => "stopped",
        AgentStatus::Exited => "exited",
        AgentStatus::Failed => "failed",
    }
}

pub(super) fn status_from_str(status: &str) -> AgentStatus {
    match status {
        "running" => AgentStatus::Running,
        "stopped" => AgentStatus::Stopped,
        "exited" => AgentStatus::Exited,
        "failed" => AgentStatus::Failed,
        _ => AgentStatus::Created,
    }
}

pub(super) fn stream_to_str(stream: &OutputStream) -> &'static str {
    match stream {
        OutputStream::Stdout => "stdout",
        OutputStream::Stderr => "stderr",
        OutputStream::System => "system",
        OutputStream::Acp => "acp",
    }
}

pub(super) fn stream_from_str(stream: &str) -> OutputStream {
    match stream {
        "stdout" => OutputStream::Stdout,
        "stderr" => OutputStream::Stderr,
        "acp" => OutputStream::Acp,
        _ => OutputStream::System,
    }
}

pub(super) fn worktree_mode_to_str(mode: &WorktreeMode) -> &'static str {
    match mode {
        WorktreeMode::UseExisting => "use_existing",
        WorktreeMode::CreateWorktree => "create_worktree",
        WorktreeMode::ReuseWorktree => "reuse_worktree",
    }
}

pub(super) fn worktree_mode_from_opt(mode: Option<String>) -> WorktreeMode {
    match mode.as_deref() {
        Some("create_worktree") => WorktreeMode::CreateWorktree,
        Some("reuse_worktree") => WorktreeMode::ReuseWorktree,
        _ => WorktreeMode::UseExisting,
    }
}

pub(super) fn is_dir_empty(path: &Path) -> anyhow::Result<bool> {
    if !path.exists() {
        return Ok(true);
    }
    let mut entries = std::fs::read_dir(path)?;
    Ok(entries.next().is_none())
}

pub(super) fn is_acp_message(line: &str) -> bool {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('{') {
        return false;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return false;
    };
    let Some(obj) = value.as_object() else {
        return false;
    };
    obj.get("type")
        .and_then(|v| v.as_str())
        .map(|ty| !ty.trim().is_empty())
        .unwrap_or(false)
}

impl AgentManager {
    pub fn acp_provider_for_agent(&self, command: &str, args: &[String]) -> Option<&'static str> {
        acp_provider_for_agent_with_binary(&self.codex_acp_binary, command, args)
    }

    pub(super) fn resolve_command_path(&self, command: &str, provider: Option<&str>) -> String {
        if provider != Some(ACP_PROVIDER_CODEX) {
            return command.to_string();
        }
        let configured = &self.codex_acp_binary;
        if configured == command {
            return command.to_string();
        }
        let configured_path = Path::new(configured);
        if configured_path.is_absolute() || configured_path.exists() {
            return configured.to_string();
        }
        command.to_string()
    }
}

pub(super) fn acp_provider_for_agent_with_binary(
    codex_acp_binary: &str,
    command: &str,
    args: &[String],
) -> Option<&'static str> {
    let spec = acp_provider_for_command_with_binary(codex_acp_binary, command)?;
    if let Some(require_args) = spec.require_args
        && !require_args(args)
    {
        return None;
    }
    Some(spec.id)
}

fn acp_provider_for_command_with_binary(
    codex_acp_binary: &str,
    command: &str,
) -> Option<&'static AcpProviderSpec> {
    let command_name = Path::new(command).file_name().and_then(|n| n.to_str())?;
    let configured_name = Path::new(codex_acp_binary)
        .file_name()
        .and_then(|n| n.to_str());
    for spec in ACP_PROVIDER_SPECS {
        let matched_name = spec.command_names.contains(&command_name);
        let matched_configured_binary = spec.use_configured_binary
            && (command == codex_acp_binary || configured_name == Some(command_name));
        if matched_name || matched_configured_binary {
            return Some(spec);
        }
    }
    None
}

pub(super) fn expand_tilde(path: &str) -> String {
    if path == "~" {
        return std::env::var("HOME").unwrap_or_else(|_| path.to_string());
    }
    if let Some(stripped) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{}/{}", home, stripped);
    }
    path.to_string()
}

pub(super) fn normalize_path(path: &str) -> String {
    let mut parts = Vec::new();
    for comp in std::path::Path::new(path).components() {
        match comp {
            std::path::Component::RootDir => parts.clear(),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::Normal(seg) => {
                parts.push(seg.to_string_lossy().to_string());
            }
            _ => {}
        }
    }
    format!("/{}", parts.join("/"))
}

pub(super) fn is_path_allowed(target: &str, allowed: &str) -> bool {
    let target = normalize_path(target);
    let allowed = normalize_path(allowed);
    if target == allowed {
        return true;
    }
    if !target.starts_with(&allowed) {
        return false;
    }
    target.chars().nth(allowed.len()) == Some('/')
}
