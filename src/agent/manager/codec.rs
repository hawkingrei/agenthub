use std::path::Path;

use crate::agent::{AgentStatus, OutputStream, WorktreeMode};

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

#[cfg(test)]
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
