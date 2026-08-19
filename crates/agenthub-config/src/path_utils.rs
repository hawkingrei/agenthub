use std::path::Path;

pub fn expand_tilde(path: &str) -> String {
    if !path.starts_with('~') {
        return path.to_string();
    }

    // Fallback to current dir if HOME is not set, for consistency with other parts of the codebase.
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());

    if path == "~" {
        return home;
    }

    if let Some(rest) = path.strip_prefix("~/") {
        return Path::new(&home).join(rest).to_string_lossy().to_string();
    }

    path.to_string()
}

pub fn normalize_path(path: &str) -> String {
    let mut normalized = std::path::PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::{expand_tilde, normalize_path};

    #[test]
    fn expand_tilde_uses_home_join_for_relative_paths() {
        let home = std::env::var("HOME").expect("HOME");
        assert_eq!(
            expand_tilde("~/worktrees"),
            std::path::Path::new(&home)
                .join("worktrees")
                .to_string_lossy()
                .to_string()
        );
    }

    #[test]
    fn normalize_path_resolves_dot_and_parent() {
        assert_eq!(normalize_path("/a/b/./c"), "/a/b/c");
        assert_eq!(normalize_path("/a/b/../c"), "/a/c");
        assert_eq!(normalize_path("/a/./b/../c/."), "/a/c");
    }
}
