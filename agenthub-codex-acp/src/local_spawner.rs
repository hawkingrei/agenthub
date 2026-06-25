use std::{
    collections::HashMap,
    io::Cursor,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};

use agent_client_protocol::ConnectionTo;
use agent_client_protocol::schema::v1::{
    ClientCapabilities, ReadTextFileRequest, SessionId, WriteTextFileRequest,
};
use agent_client_protocol::Client;
use codex_apply_patch::StdFs;
use tokio::sync::mpsc;

use crate::ACP_CLIENT;

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn ensure_path_within_root(root: &Path, path: &Path) -> std::io::Result<PathBuf> {
    let abs_root = std::fs::canonicalize(root)?;
    let raw_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        abs_root.join(path)
    };
    let normalized = normalize_lexical(raw_path.as_path());

    // Resolve symlinks in the deepest existing ancestor, while still allowing
    // non-existent trailing path segments (new files/dirs).
    let mut existing_prefix = normalized.clone();
    let mut suffix = vec![];
    while !existing_prefix.exists() {
        let Some(name) = existing_prefix.file_name() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid path: {}", normalized.display()),
            ));
        };
        suffix.push(name.to_owned());
        let Some(parent) = existing_prefix.parent() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid path: {}", normalized.display()),
            ));
        };
        existing_prefix = parent.to_path_buf();
    }

    let mut abs_path = std::fs::canonicalize(existing_prefix)?;
    for name in suffix.iter().rev() {
        abs_path.push(name);
    }
    let abs_path = normalize_lexical(abs_path.as_path());

    if abs_path.starts_with(&abs_root) {
        Ok(abs_path)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "access to {} denied (outside session root {})",
                abs_path.display(),
                abs_root.display()
            ),
        ))
    }
}

#[derive(Debug)]
pub enum FsTask {
    ReadFile {
        session_id: SessionId,
        path: PathBuf,
        tx: std::sync::mpsc::Sender<std::io::Result<String>>,
    },
    ReadFileLimit {
        session_id: SessionId,
        path: PathBuf,
        limit: usize,
        tx: tokio::sync::oneshot::Sender<std::io::Result<String>>,
    },
    WriteFile {
        session_id: SessionId,
        path: PathBuf,
        content: String,
        tx: std::sync::mpsc::Sender<std::io::Result<()>>,
    },
}

impl FsTask {
    async fn run(self) {
        match self {
            FsTask::ReadFile {
                session_id,
                path,
                tx,
            } => {
                let response = Self::client()
                    .send_request(ReadTextFileRequest::new(session_id, path))
                    .block_task()
                    .await
                    .map(|response| response.content)
                    .map_err(|e| std::io::Error::other(e.to_string()));
                tx.send(response).ok();
            }
            FsTask::ReadFileLimit {
                session_id,
                path,
                limit,
                tx,
            } => {
                let response = Self::client()
                    .send_request(
                        ReadTextFileRequest::new(session_id, path)
                            .limit(limit.try_into().unwrap_or(u32::MAX)),
                    )
                    .block_task()
                    .await
                    .map(|response| response.content)
                    .map_err(|e| std::io::Error::other(e.to_string()));
                tx.send(response).ok();
            }
            FsTask::WriteFile {
                session_id,
                path,
                content,
                tx,
            } => {
                let response = Self::client()
                    .send_request(WriteTextFileRequest::new(session_id, path, content))
                    .block_task()
                    .await
                    .map(|_| ())
                    .map_err(|e| std::io::Error::other(e.to_string()));
                tx.send(response).ok();
            }
        }
    }

    fn client() -> &'static ConnectionTo<Client> {
        ACP_CLIENT.get().expect("Missing ACP client")
    }
}

pub struct AcpFs {
    client_capabilities: Arc<Mutex<ClientCapabilities>>,
    local_spawner: LocalSpawner,
    session_id: SessionId,
    session_roots: Arc<Mutex<HashMap<SessionId, PathBuf>>>,
}

impl AcpFs {
    pub fn new(
        session_id: SessionId,
        client_capabilities: Arc<Mutex<ClientCapabilities>>,
        local_spawner: LocalSpawner,
        session_roots: Arc<Mutex<HashMap<SessionId, PathBuf>>>,
    ) -> Self {
        Self {
            client_capabilities,
            local_spawner,
            session_id,
            session_roots,
        }
    }

    fn session_root(&self) -> std::io::Result<PathBuf> {
        self.session_roots
            .lock()
            .unwrap()
            .get(&self.session_id)
            .cloned()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "session root not registered",
                )
            })
    }

    fn ensure_within_root(&self, path: &std::path::Path) -> std::io::Result<PathBuf> {
        ensure_path_within_root(self.session_root()?.as_path(), path)
    }
}

impl codex_apply_patch::Fs for AcpFs {
    fn read_to_string(&self, path: &std::path::Path) -> std::io::Result<String> {
        if !self.client_capabilities.lock().unwrap().fs.read_text_file {
            return StdFs.read_to_string(path);
        }
        let path = self.ensure_within_root(path)?;
        let (tx, rx) = std::sync::mpsc::channel();
        self.local_spawner.spawn(FsTask::ReadFile {
            session_id: self.session_id.clone(),
            path,
            tx,
        });
        rx.recv()
            .map_err(|e| std::io::Error::other(e.to_string()))
            .flatten()
    }

    fn write(&self, path: &std::path::Path, contents: &[u8]) -> std::io::Result<()> {
        if !self.client_capabilities.lock().unwrap().fs.write_text_file {
            return StdFs.write(path, contents);
        }
        let path = self.ensure_within_root(path)?;
        let (tx, rx) = std::sync::mpsc::channel();
        self.local_spawner.spawn(FsTask::WriteFile {
            session_id: self.session_id.clone(),
            path,
            content: String::from_utf8(contents.to_vec())
                .map_err(|e| std::io::Error::other(e.to_string()))?,
            tx,
        });
        rx.recv()
            .map_err(|e| std::io::Error::other(e.to_string()))
            .flatten()
    }
}

impl codex_core::codex::Fs for AcpFs {
    fn file_buffer(
        &self,
        path: &std::path::Path,
        limit: usize,
    ) -> std::pin::Pin<
        Box<
            dyn Future<Output = std::io::Result<Box<dyn tokio::io::AsyncBufRead + Unpin + Send>>>
                + Send,
        >,
    > {
        if !self.client_capabilities.lock().unwrap().fs.read_text_file {
            return StdFs.file_buffer(path, limit);
        }
        let path = match self.ensure_within_root(path) {
            Ok(path) => path,
            Err(e) => return Box::pin(async move { Err(e) }),
        };
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.local_spawner.spawn(FsTask::ReadFileLimit {
            session_id: self.session_id.clone(),
            path,
            limit,
            tx,
        });
        Box::pin(async move {
            let file = rx
                .await
                .map_err(|e| std::io::Error::other(e.to_string()))
                .flatten()?;

            Ok(Box::new(tokio::io::BufReader::new(Cursor::new(file.into_bytes()))) as _)
        })
    }
}

#[derive(Clone)]
pub struct LocalSpawner {
    send: mpsc::UnboundedSender<FsTask>,
}

impl LocalSpawner {
    pub fn new() -> Self {
        let (send, mut recv) = mpsc::unbounded_channel::<FsTask>();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        std::thread::spawn(move || {
            let local = tokio::task::LocalSet::new();

            local.spawn_local(async move {
                while let Some(new_task) = recv.recv().await {
                    tokio::task::spawn_local(new_task.run());
                }
                // If the while loop returns, then all the LocalSpawner
                // objects have been dropped.
            });

            // This will return once all senders are dropped and all
            // spawned tasks have returned.
            rt.block_on(local);
        });

        Self { send }
    }

    pub fn spawn(&self, task: FsTask) {
        self.send
            .send(task)
            .expect("Thread with LocalSet has shut down.");
    }
}

#[cfg(test)]
mod tests {
    use super::ensure_path_within_root;
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicU64, Ordering},
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_root() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let tick = NEXT.fetch_add(1, Ordering::Relaxed);
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "agenthub-codex-acp-local-spawner-{now_nanos}-{}-{tick}",
            std::process::id()
        ))
    }

    #[test]
    fn resolves_relative_paths_against_root() {
        let root = temp_root();
        fs::create_dir_all(root.join("nested")).unwrap();
        let canonical_root = fs::canonicalize(&root).unwrap();

        let resolved =
            ensure_path_within_root(root.as_path(), PathBuf::from("nested/file.txt").as_path())
                .unwrap();
        assert_eq!(resolved, canonical_root.join("nested/file.txt"));

        drop(fs::remove_dir_all(root));
    }

    #[test]
    fn rejects_paths_escaping_root() {
        let root = temp_root();
        fs::create_dir_all(&root).unwrap();
        let escaped = root.join("../outside.txt");

        let err = ensure_path_within_root(root.as_path(), escaped.as_path()).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);

        drop(fs::remove_dir_all(root));
    }

    #[test]
    fn accepts_absolute_paths_under_root() {
        let root = temp_root();
        fs::create_dir_all(root.join("nested")).unwrap();
        let canonical_root = fs::canonicalize(&root).unwrap();
        let inside = canonical_root.join("nested/inside.txt");

        let resolved = ensure_path_within_root(root.as_path(), inside.as_path()).unwrap();
        assert_eq!(resolved, inside);

        drop(fs::remove_dir_all(root));
    }

    #[test]
    fn allows_nonexistent_paths_under_root() {
        let root = temp_root();
        fs::create_dir_all(&root).unwrap();
        let canonical_root = fs::canonicalize(&root).unwrap();
        let unresolved = PathBuf::from("new/deep/file.txt");

        let resolved = ensure_path_within_root(root.as_path(), unresolved.as_path()).unwrap();
        assert_eq!(resolved, canonical_root.join("new/deep/file.txt"));

        drop(fs::remove_dir_all(root));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape_under_root() {
        use std::os::unix::fs::symlink;

        let root = temp_root();
        let outside = temp_root().with_file_name(format!(
            "{}-outside",
            root.file_name().unwrap().to_string_lossy()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("link_out")).unwrap();

        let err =
            ensure_path_within_root(root.as_path(), Path::new("link_out/secret.txt")).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);

        drop(fs::remove_dir_all(root));
        drop(fs::remove_dir_all(outside));
    }

}
