use std::path::{Path, PathBuf};

pub(super) const GLOBAL_GITIGNORE_ENTRY: &str = ".agenthubmemory";
pub(super) const GLOBAL_GITIGNORE_FILENAME: &str = ".gitignore_global";
pub(super) const DEFAULT_GIT_IGNORE_SUBPATH: &str = "git/ignore";

pub(super) fn ensure_global_gitignore_agenthubmemory() -> anyhow::Result<()> {
    let Some(home) = std::env::var_os("HOME").filter(|home| !home.is_empty()) else {
        return Ok(());
    };

    let home_path = PathBuf::from(home);
    for gitignore_path in resolve_global_gitignore_paths(&home_path) {
        append_gitignore_entry(&gitignore_path, GLOBAL_GITIGNORE_ENTRY)?;
    }
    Ok(())
}

pub(super) fn resolve_global_gitignore_paths(home_path: &Path) -> Vec<PathBuf> {
    let mut paths = vec![home_path.join(GLOBAL_GITIGNORE_FILENAME)];
    let xdg_root = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_path.join(".config"));
    let default_ignore_path = xdg_root.join(DEFAULT_GIT_IGNORE_SUBPATH);
    if !paths.iter().any(|path| path == &default_ignore_path) {
        paths.push(default_ignore_path);
    }
    paths
}

pub(super) fn append_gitignore_entry(path: &Path, entry: &str) -> anyhow::Result<()> {
    let existing = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };

    if existing.lines().any(|line| line.trim() == entry) {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(entry);
    updated.push('\n');
    std::fs::write(path, updated)?;
    Ok(())
}
