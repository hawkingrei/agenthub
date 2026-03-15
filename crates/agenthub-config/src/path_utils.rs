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

#[cfg(test)]
mod tests {
    use super::expand_tilde;

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
}
