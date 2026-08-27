use std::{
    fmt::Write as _,
    fs::{File, OpenOptions, TryLockError},
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug)]
pub(crate) struct DaemonInstanceGuard {
    lock_file: File,
    lock_path: PathBuf,
    db_path: PathBuf,
    node_id: String,
    owner_id: String,
    owner_pid: i64,
    started_at: i64,
    generation: Option<agenthub_db::DaemonGeneration>,
}

#[derive(Serialize)]
struct LockMetadata<'a> {
    node_id: &'a str,
    db_path: String,
    owner_id: &'a str,
    owner_pid: i64,
    started_at: i64,
    generation: Option<i64>,
}

impl DaemonInstanceGuard {
    pub(crate) fn acquire(db_path: &Path, node_id: &str) -> anyhow::Result<Self> {
        let db_path = canonicalize_db_path(db_path)?;
        let lock_path = lock_path_for(&db_path, node_id)?;
        let lock_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            // Truncation must happen only after this process owns the lock.
            .truncate(false)
            .open(&lock_path)
            .with_context(|| format!("open daemon instance lock {}", lock_path.display()))?;

        match lock_file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                anyhow::bail!(
                    "daemon instance already owns node {node_id:?} for database {}; lock: {}",
                    db_path.display(),
                    lock_path.display()
                );
            }
            Err(TryLockError::Error(error)) => {
                return Err(error).with_context(|| {
                    format!("acquire daemon instance lock {}", lock_path.display())
                });
            }
        }

        let mut guard = Self {
            lock_file,
            lock_path,
            db_path,
            node_id: node_id.to_string(),
            owner_id: Uuid::new_v4().to_string(),
            owner_pid: i64::from(std::process::id()),
            started_at: chrono::Utc::now().timestamp(),
            generation: None,
        };
        guard.write_metadata()?;
        Ok(guard)
    }

    pub(crate) async fn claim_generation(&mut self, pool: &SqlitePool) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.generation.is_none(),
            "daemon generation has already been claimed"
        );
        let generation = agenthub_db::claim_daemon_generation(
            pool,
            &self.node_id,
            &self.owner_id,
            self.owner_pid,
            self.started_at,
        )
        .await?;
        self.generation = Some(generation);
        self.write_metadata()?;

        tracing::info!(
            node_id = %self.node_id,
            owner_id = %self.owner_id,
            generation = self.generation().expect("generation was just claimed"),
            db_path = %self.db_path.display(),
            lock_path = %self.lock_path.display(),
            "daemon instance ownership claimed"
        );
        Ok(())
    }

    pub(crate) async fn verify_current(&self, pool: &SqlitePool) -> anyhow::Result<()> {
        let generation = self
            .generation
            .as_ref()
            .context("daemon generation has not been claimed")?;
        if !agenthub_db::is_current_daemon_generation(pool, generation).await? {
            anyhow::bail!(
                "daemon generation {} for node {:?} is no longer current",
                generation.generation,
                self.node_id
            );
        }
        Ok(())
    }

    fn generation(&self) -> Option<i64> {
        self.generation
            .as_ref()
            .map(|generation| generation.generation)
    }

    fn write_metadata(&mut self) -> anyhow::Result<()> {
        let generation = self.generation();
        self.lock_file
            .set_len(0)
            .with_context(|| format!("truncate daemon lock {}", self.lock_path.display()))?;
        self.lock_file
            .seek(SeekFrom::Start(0))
            .with_context(|| format!("seek daemon lock {}", self.lock_path.display()))?;
        serde_json::to_writer_pretty(
            &mut self.lock_file,
            &LockMetadata {
                node_id: &self.node_id,
                db_path: self.db_path.display().to_string(),
                owner_id: &self.owner_id,
                owner_pid: self.owner_pid,
                started_at: self.started_at,
                generation,
            },
        )
        .with_context(|| format!("write daemon lock metadata {}", self.lock_path.display()))?;
        self.lock_file
            .write_all(b"\n")
            .with_context(|| format!("finish daemon lock metadata {}", self.lock_path.display()))?;
        self.lock_file
            .sync_data()
            .with_context(|| format!("sync daemon lock metadata {}", self.lock_path.display()))?;
        Ok(())
    }
}

fn canonicalize_db_path(db_path: &Path) -> anyhow::Result<PathBuf> {
    if db_path.exists() {
        return db_path
            .canonicalize()
            .with_context(|| format!("canonicalize database path {}", db_path.display()));
    }

    let file_name = db_path
        .file_name()
        .context("database path must include a file name")?;
    let parent = db_path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("create database directory {}", parent.display()))?;
    let canonical_parent = parent
        .canonicalize()
        .with_context(|| format!("canonicalize database directory {}", parent.display()))?;
    Ok(canonical_parent.join(file_name))
}

fn lock_path_for(db_path: &Path, node_id: &str) -> anyhow::Result<PathBuf> {
    let parent = db_path
        .parent()
        .context("canonical database path must include a parent directory")?;
    let mut hasher = Sha256::new();
    hasher.update(db_path.as_os_str().as_encoded_bytes());
    hasher.update([0]);
    hasher.update(node_id.as_bytes());
    let mut digest = String::with_capacity(64);
    for byte in hasher.finalize() {
        write!(&mut digest, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(parent.join(format!(".agenthub-daemon-{digest}.lock")))
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use uuid::Uuid;

    use super::DaemonInstanceGuard;

    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("agenthub-daemon-lock-{name}-{}", Uuid::new_v4()))
    }

    #[test]
    fn lock_scope_is_database_and_node_and_drop_releases_it() {
        let first_dir = unique_temp_dir("scope-first");
        let second_dir = unique_temp_dir("scope-second");
        let first_db = first_dir.join("agenthub.db");
        let second_db = second_dir.join("agenthub.db");

        let first = DaemonInstanceGuard::acquire(&first_db, "main").expect("acquire first lock");
        let same_db_same_node = DaemonInstanceGuard::acquire(&first_db, "main")
            .expect_err("same database and node must conflict");
        assert!(same_db_same_node.to_string().contains("already owns node"));

        let same_db_other_node = DaemonInstanceGuard::acquire(&first_db, "node-east")
            .expect("different node can share database");
        let same_node_other_db = DaemonInstanceGuard::acquire(&second_db, "main")
            .expect("same node can use a different database");

        let lock_path = first.lock_path.clone();
        let metadata: Value = serde_json::from_slice(
            &std::fs::read(&lock_path).expect("read diagnostic lock metadata"),
        )
        .expect("parse diagnostic lock metadata");
        assert_eq!(metadata["node_id"], "main");
        assert_eq!(metadata["generation"], Value::Null);
        assert!(
            metadata["owner_id"]
                .as_str()
                .is_some_and(|id| !id.is_empty())
        );

        drop(first);
        let replacement =
            DaemonInstanceGuard::acquire(&first_db, "main").expect("dropping guard releases lock");

        drop((replacement, same_db_other_node, same_node_other_db));
        std::fs::remove_dir_all(first_dir).expect("remove first temp directory");
        std::fs::remove_dir_all(second_dir).expect("remove second temp directory");
    }

    #[cfg(unix)]
    #[test]
    fn canonical_paths_share_the_same_lock_scope() {
        use std::os::unix::fs::symlink;

        let dir = unique_temp_dir("canonical");
        let real_dir = dir.join("real");
        let alias_dir = dir.join("alias");
        std::fs::create_dir_all(&real_dir).expect("create real directory");
        symlink(&real_dir, &alias_dir).expect("create directory symlink");

        let first = DaemonInstanceGuard::acquire(&real_dir.join("agenthub.db"), "main")
            .expect("acquire canonical lock");
        DaemonInstanceGuard::acquire(&alias_dir.join("agenthub.db"), "main")
            .expect_err("symlinked database path must conflict");

        drop(first);
        std::fs::remove_dir_all(dir).expect("remove temp directory");
    }

    #[tokio::test]
    async fn claimed_generation_updates_metadata_and_rejects_stale_owner() {
        let dir = unique_temp_dir("generation");
        let db_path = dir.join("agenthub.db");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory database");
        let mut guard =
            DaemonInstanceGuard::acquire(&db_path, "main").expect("acquire daemon lock");

        guard
            .claim_generation(&pool)
            .await
            .expect("claim daemon generation");
        guard
            .verify_current(&pool)
            .await
            .expect("claimed generation is current");

        let metadata: Value = serde_json::from_slice(
            &std::fs::read(&guard.lock_path).expect("read lock metadata after claim"),
        )
        .expect("parse lock metadata after claim");
        assert_eq!(metadata["generation"], 1);

        agenthub_db::claim_daemon_generation(
            &pool,
            "main",
            "replacement-owner",
            999,
            guard.started_at + 1,
        )
        .await
        .expect("simulate a replacement owner");
        let stale_error = guard
            .verify_current(&pool)
            .await
            .expect_err("old generation must be fenced");
        assert!(stale_error.to_string().contains("is no longer current"));

        drop(guard);
        pool.close().await;
        std::fs::remove_dir_all(dir).expect("remove temp directory");
    }
}
