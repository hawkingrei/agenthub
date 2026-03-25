use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

pub(crate) struct TempManagedSkillsHome {
    root: PathBuf,
}

impl TempManagedSkillsHome {
    pub(crate) fn new(prefix: &str) -> Self {
        let root = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create temp managed skills home");
        Self { root }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TempManagedSkillsHome {
    fn drop(&mut self) {
        if self.root.exists() {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
