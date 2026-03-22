use std::path::{Path, PathBuf};

const AGENTHUB_BINARY_NAME: &str = "agenthub";
// Cargo exposes this as CARGO_BIN_EXE_<name>, where <name> is the binary target name as-is.
const AGENTHUB_BINARY_ENV: &str = "CARGO_BIN_EXE_agenthub";

pub(crate) fn resolve_agenthub_binary_path() -> Option<PathBuf> {
    let current_exe = std::env::current_exe().ok();
    resolve_agenthub_binary_path_from(current_exe.as_deref())
}

fn resolve_agenthub_binary_path_from(current_exe: Option<&Path>) -> Option<PathBuf> {
    resolve_cargo_provided_agenthub_binary_path()
        .or_else(|| resolve_sibling_agenthub_binary_path(current_exe))
}

fn resolve_cargo_provided_agenthub_binary_path() -> Option<PathBuf> {
    let path = std::env::var(AGENTHUB_BINARY_ENV).ok()?;
    std::fs::canonicalize(path).ok()
}

fn resolve_sibling_agenthub_binary_path(current_exe: Option<&Path>) -> Option<PathBuf> {
    let current = current_exe?;
    let sibling = current
        .parent()
        .and_then(|parent| parent.parent())
        .map(|dir| {
            dir.join(format!(
                "{AGENTHUB_BINARY_NAME}{}",
                std::env::consts::EXE_SUFFIX
            ))
        })?;
    std::fs::canonicalize(sibling).ok()
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use uuid::Uuid;

    use super::*;

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct TempBinaryLayout {
        root: PathBuf,
        current_exe: PathBuf,
        sibling_binary: PathBuf,
        env_binary: PathBuf,
    }

    struct EnvVarGuard;

    impl Drop for TempBinaryLayout {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            unsafe {
                std::env::remove_var(AGENTHUB_BINARY_ENV);
            }
        }
    }

    #[test]
    fn resolve_agenthub_binary_path_prefers_cargo_env() {
        let _guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("lock env mutex");
        let layout = create_temp_binary_layout();
        let _env_guard = EnvVarGuard;
        unsafe {
            std::env::set_var(AGENTHUB_BINARY_ENV, &layout.env_binary);
        }

        let resolved = resolve_agenthub_binary_path_from(Some(&layout.current_exe))
            .expect("resolve path from cargo env");

        assert_eq!(
            resolved,
            std::fs::canonicalize(&layout.env_binary).expect("canonical env binary")
        );
    }

    #[test]
    fn resolve_agenthub_binary_path_falls_back_to_sibling_binary() {
        let _guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("lock env mutex");
        let layout = create_temp_binary_layout();
        unsafe {
            std::env::remove_var(AGENTHUB_BINARY_ENV);
        }

        let resolved = resolve_agenthub_binary_path_from(Some(&layout.current_exe))
            .expect("resolve sibling binary");

        assert_eq!(
            resolved,
            std::fs::canonicalize(&layout.sibling_binary).expect("canonical sibling binary")
        );
    }

    fn create_temp_binary_layout() -> TempBinaryLayout {
        let root = std::env::temp_dir().join(format!("agenthub-binary-{}", Uuid::new_v4()));
        let debug_dir = root.join("target").join("debug");
        let deps_dir = debug_dir.join("deps");
        let env_dir = root.join("env-bin");
        std::fs::create_dir_all(&deps_dir).expect("create deps dir");
        std::fs::create_dir_all(&env_dir).expect("create env dir");

        let current_exe = deps_dir.join(format!("agenthub-tests{}", std::env::consts::EXE_SUFFIX));
        let sibling_binary = debug_dir.join(format!(
            "{AGENTHUB_BINARY_NAME}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let env_binary = env_dir.join(format!(
            "{AGENTHUB_BINARY_NAME}{}",
            std::env::consts::EXE_SUFFIX
        ));

        std::fs::write(&current_exe, b"test").expect("write current exe marker");
        std::fs::write(&sibling_binary, b"test").expect("write sibling binary marker");
        std::fs::write(&env_binary, b"test").expect("write env binary marker");

        TempBinaryLayout {
            root,
            current_exe,
            sibling_binary,
            env_binary,
        }
    }
}
