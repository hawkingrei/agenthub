use std::path::{Path, PathBuf};

const DAEMON_BINARY_NAME: &str = "agenthubd";

pub(crate) async fn launch_daemon() -> anyhow::Result<()> {
    let current_exe = std::env::current_exe().ok();
    let daemon = resolve_daemon_binary_path(current_exe.as_deref());
    let status = tokio::process::Command::new(&daemon)
        .status()
        .await
        .map_err(|err| {
            anyhow::anyhow!(
                "failed to launch daemon `{}`: {err}",
                daemon.to_string_lossy()
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("daemon exited with status {status}")
    }
}

fn resolve_daemon_binary_path(current_exe: Option<&Path>) -> PathBuf {
    resolve_sibling_daemon_binary_path(current_exe).unwrap_or_else(|| {
        PathBuf::from(format!(
            "{DAEMON_BINARY_NAME}{}",
            std::env::consts::EXE_SUFFIX
        ))
    })
}

fn resolve_sibling_daemon_binary_path(current_exe: Option<&Path>) -> Option<PathBuf> {
    let current = current_exe?;
    let parent = current.parent()?;
    let binary_dir = if parent.file_name().and_then(|name| name.to_str()) == Some("deps") {
        parent.parent()?
    } else {
        parent
    };
    let candidate = binary_dir.join(format!(
        "{DAEMON_BINARY_NAME}{}",
        std::env::consts::EXE_SUFFIX
    ));
    candidate.is_file().then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_installed_sibling_daemon() {
        let root = temp_layout("installed");
        let cli = root.join(format!("agenthub{}", std::env::consts::EXE_SUFFIX));
        let daemon = root.join(format!("agenthubd{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&cli, b"cli").expect("write CLI marker");
        std::fs::write(&daemon, b"daemon").expect("write daemon marker");

        assert_eq!(resolve_daemon_binary_path(Some(&cli)), daemon);
        std::fs::remove_dir_all(root).expect("remove temp layout");
    }

    #[test]
    fn resolves_cargo_test_sibling_daemon() {
        let root = temp_layout("cargo-test");
        let deps = root.join("deps");
        std::fs::create_dir_all(&deps).expect("create deps directory");
        let test_exe = deps.join("agenthub-tests");
        let daemon = root.join(format!("agenthubd{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&test_exe, b"test").expect("write test marker");
        std::fs::write(&daemon, b"daemon").expect("write daemon marker");

        assert_eq!(resolve_daemon_binary_path(Some(&test_exe)), daemon);
        std::fs::remove_dir_all(root).expect("remove temp layout");
    }

    #[test]
    fn falls_back_to_path_lookup() {
        assert_eq!(
            resolve_daemon_binary_path(Some(Path::new("/missing/agenthub"))),
            PathBuf::from(format!(
                "{DAEMON_BINARY_NAME}{}",
                std::env::consts::EXE_SUFFIX
            ))
        );
    }

    fn temp_layout(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "agenthub-daemon-launcher-{label}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create temp layout");
        root
    }
}
