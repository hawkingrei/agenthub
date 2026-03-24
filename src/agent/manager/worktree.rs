use std::path::Path;

use tokio::process::Command;

use crate::path_utils::normalize_path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GitWorktreeEntry {
    pub(super) path: String,
    pub(super) head: Option<String>,
    pub(super) branch: Option<String>,
}

pub(super) fn git_command_without_fsmonitor() -> Command {
    let mut command = Command::new("git");
    command.arg("-c").arg("core.fsmonitor=false");
    command
}

pub(super) async fn repo_find_worktree_entry(
    repo: &str,
    workdir: &str,
) -> anyhow::Result<Option<GitWorktreeEntry>> {
    let output = git_command_without_fsmonitor()
        .arg("-C")
        .arg(repo)
        .arg("worktree")
        .arg("list")
        .arg("--porcelain")
        .output()
        .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr.trim();
        if reason.is_empty() {
            anyhow::bail!("git worktree list failed");
        }
        anyhow::bail!("git worktree list failed: {reason}");
    }
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let workdir = workdir.to_string();
    tokio::task::spawn_blocking(move || Ok(find_matching_worktree_entry(&stdout, &workdir)))
        .await
        .map_err(|err| anyhow::anyhow!("git worktree path normalization task failed: {err}"))?
}

fn find_matching_worktree_entry(stdout: &str, workdir: &str) -> Option<GitWorktreeEntry> {
    let target = normalize_worktree_path(workdir);
    parse_worktree_list(stdout)
        .into_iter()
        .find(|entry| normalize_worktree_path(&entry.path) == target)
}

pub(super) fn parse_worktree_list(stdout: &str) -> Vec<GitWorktreeEntry> {
    let mut entries = Vec::new();
    let mut current: Option<GitWorktreeEntry> = None;
    for line in stdout.lines() {
        if line.is_empty() {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            continue;
        }
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            current = Some(GitWorktreeEntry {
                path: path.to_string(),
                head: None,
                branch: None,
            });
            continue;
        }
        if let Some(head) = line.strip_prefix("HEAD ") {
            if let Some(entry) = current.as_mut() {
                entry.head = Some(head.to_string());
            }
            continue;
        }
        if let Some(branch) = line.strip_prefix("branch ")
            && let Some(entry) = current.as_mut()
        {
            entry.branch = Some(branch.to_string());
        }
    }
    if let Some(entry) = current.take() {
        entries.push(entry);
    }
    entries
}

fn trim_ref_prefix(value: &str) -> &str {
    value.strip_prefix("refs/heads/").unwrap_or(value)
}

pub(super) fn is_hex_sha(value: &str) -> bool {
    let len = value.len();
    (7..=64).contains(&len) && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub(super) fn worktree_ref_matches(entry: &GitWorktreeEntry, expected_ref: &str) -> bool {
    let expected = expected_ref.trim();
    if expected.eq_ignore_ascii_case("HEAD") {
        return true;
    }

    let expected_branch = trim_ref_prefix(expected);
    if !expected_branch.is_empty()
        && let Some(branch) = entry.branch.as_deref()
        && trim_ref_prefix(branch) == expected_branch
    {
        return true;
    }

    if is_hex_sha(expected)
        && let Some(head) = entry.head.as_deref()
    {
        return head.starts_with(expected);
    }

    false
}

fn normalize_worktree_path(path: &str) -> String {
    let canonical = std::fs::canonicalize(path).or_else(|err| {
        tracing::warn!(
            path = %path,
            error = %err,
            "failed to canonicalize worktree path"
        );
        if Path::new(path).is_absolute() {
            return Ok(Path::new(path).to_path_buf());
        }
        std::env::current_dir().map(|cwd| cwd.join(path))
    });
    let canonical = canonical
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string());
    normalize_path(&canonical)
}

#[cfg(test)]
mod tests {
    use super::{GitWorktreeEntry, is_hex_sha, parse_worktree_list, worktree_ref_matches};

    #[test]
    fn parse_worktree_list_extracts_entries() {
        let stdout = r#"
worktree /tmp/repo
HEAD 0000000000000000000000000000000000000000
branch refs/heads/main

worktree /tmp/repo/worktrees/agent-a
HEAD 1111111111111111111111111111111111111111
branch refs/heads/agent-a
"#;
        let entries = parse_worktree_list(stdout);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].path, "/tmp/repo/worktrees/agent-a");
        assert_eq!(entries[1].branch.as_deref(), Some("refs/heads/agent-a"));
    }

    #[test]
    fn worktree_ref_matches_accepts_head() {
        let entry = GitWorktreeEntry {
            path: "/tmp/repo/worktrees/agent-a".to_string(),
            head: Some("1111111111111111111111111111111111111111".to_string()),
            branch: Some("refs/heads/agent-a".to_string()),
        };
        assert!(worktree_ref_matches(&entry, "HEAD"));
    }

    #[test]
    fn worktree_ref_matches_accepts_matching_branch() {
        let entry = GitWorktreeEntry {
            path: "/tmp/repo/worktrees/agent-a".to_string(),
            head: Some("1111111111111111111111111111111111111111".to_string()),
            branch: Some("refs/heads/agent-a".to_string()),
        };
        assert!(worktree_ref_matches(&entry, "refs/heads/agent-a"));
        assert!(worktree_ref_matches(&entry, "agent-a"));
        assert!(!worktree_ref_matches(&entry, "agent-b"));
    }

    #[test]
    fn worktree_ref_matches_accepts_matching_commit_prefix() {
        let entry = GitWorktreeEntry {
            path: "/tmp/repo/worktrees/agent-a".to_string(),
            head: Some("1111111111111111111111111111111111111111".to_string()),
            branch: None,
        };
        assert!(worktree_ref_matches(&entry, "1111111"));
        assert!(!worktree_ref_matches(&entry, "2222222"));
    }

    #[test]
    fn is_hex_sha_accepts_sha256_length() {
        assert!(is_hex_sha(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
    }
}
