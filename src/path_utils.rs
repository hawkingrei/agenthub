use std::path::Path;

pub(crate) fn expand_tilde(path: &str) -> String {
    if path == "~" {
        return std::env::var("HOME").unwrap_or_else(|_| path.to_string());
    }
    if let Some(stripped) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return Path::new(&home).join(stripped).to_string_lossy().to_string();
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
