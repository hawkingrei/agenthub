use std::{
    collections::HashMap,
    io::Cursor,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};

use agent_client_protocol_legacy::{
    AgentSideConnection, Client, ClientCapabilities, ReadTextFileRequest, SessionId,
    WriteTextFileRequest,
};
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
                let read_text_file =
                    Self::client().read_text_file(ReadTextFileRequest::new(session_id, path));
                let response = read_text_file
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
                let read_text_file = Self::client().read_text_file(
                    ReadTextFileRequest::new(session_id, path)
                        .limit(limit.try_into().unwrap_or(u32::MAX)),
                );
                let response = read_text_file
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
                    .write_text_file(WriteTextFileRequest::new(session_id, path, content))
                    .await
                    .map(|_| ())
                    .map_err(|e| std::io::Error::other(e.to_string()));
                tx.send(response).ok();
            }
        }
    }

    fn client() -> &'static AgentSideConnection {
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
    use super::{AcpFs, LocalSpawner, ensure_path_within_root};
    use crate::{ACP_CLIENT, spawn_acp_io_task};
    use agent_client_protocol_legacy::{Agent, AgentSideConnection, Client};
    use agent_client_protocol_legacy::{
        AuthenticateRequest, AuthenticateResponse, ClientCapabilities, FileSystemCapabilities,
        Implementation, InitializeRequest, InitializeResponse, LoadSessionRequest,
        LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest,
        PromptResponse, ReadTextFileRequest, ReadTextFileResponse, SessionId,
        SetSessionConfigOptionRequest, SetSessionConfigOptionResponse, SetSessionModeRequest,
        SetSessionModeResponse, StopReason, WriteTextFileRequest,
    };
    use std::{
        collections::HashMap,
        fs,
        path::{Path, PathBuf},
        process::Command,
        sync::{
            Arc, Mutex,
            atomic::{AtomicU64, Ordering},
        },
        thread,
        time::Duration,
        time::{SystemTime, UNIX_EPOCH},
    };

    const DEADLOCK_CHILD_ENV: &str = "AGENTHUB_CODEX_ACP_APPLY_PATCH_DEADLOCK_CHILD";
    const DEADLOCK_CHILD_TEST: &str = "local_spawner::tests::apply_patch_verification_child";

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

    #[test]
    fn apply_patch_verification_does_not_deadlock_over_acp_fs() {
        if std::env::var_os(DEADLOCK_CHILD_ENV).is_some() {
            return;
        }

        let current_exe = std::env::current_exe().expect("resolve current test binary");
        let mut child = Command::new(current_exe)
            .arg("--exact")
            .arg(DEADLOCK_CHILD_TEST)
            .arg("--nocapture")
            .env(DEADLOCK_CHILD_ENV, "1")
            .spawn()
            .expect("spawn deadlock child");

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().expect("poll deadlock child") {
                assert!(status.success(), "child exited with {status}");
                return;
            }

            if std::time::Instant::now() >= deadline {
                drop(child.kill());
                drop(child.wait());
                panic!("child timed out; apply_patch ACP fs roundtrip deadlocked");
            }

            thread::sleep(Duration::from_millis(25));
        }
    }

    #[test]
    fn apply_patch_verification_child() {
        if std::env::var_os(DEADLOCK_CHILD_ENV).is_none() {
            return;
        }

        reproduce_apply_patch_roundtrip();
    }

    fn reproduce_apply_patch_roundtrip() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime");
        let local_set = tokio::task::LocalSet::new();

        runtime.block_on(local_set.run_until(async move {
            let client = TestClient::new();
            let agent = TestAgent;
            let temp_dir = tempfile::Builder::new()
                .prefix("agenthub-codex-acp-deadlock-")
                .tempdir()
                .expect("create temp dir");
            let root = temp_dir.path().to_path_buf();
            let session_id = SessionId::new("test-session");
            let (client_to_agent_rx, client_to_agent_tx) = piper::pipe(1024);
            let (agent_to_client_rx, agent_to_client_tx) = piper::pipe(1024);
            let (client_ready_tx, client_ready_rx) = std::sync::mpsc::channel();

            let source_dir = root.join("src");
            fs::create_dir_all(&source_dir).expect("create test dirs");
            let file_path = fs::canonicalize(&source_dir)
                .expect("canonicalize source dir")
                .join("client.rs");
            client.add_file_content(file_path.clone(), "fn old() {}\n".to_string());

            let _client_thread = thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("build remote client runtime");
                let local_set = tokio::task::LocalSet::new();

                runtime.block_on(local_set.run_until(async move {
                    let (_client_side, client_io_task) = agent_client_protocol_legacy::ClientSideConnection::new(
                        client,
                        client_to_agent_tx,
                        agent_to_client_rx,
                        |fut| {
                            tokio::task::spawn_local(fut);
                        },
                    );
                    client_ready_tx.send(()).expect("signal remote client ready");
                    client_io_task.await.expect("run remote client io task");
                }));
            });

            client_ready_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("wait for remote client bootstrap");

            let (agent_side, agent_io_task) = AgentSideConnection::new(
                agent,
                agent_to_client_tx,
                client_to_agent_rx,
                |fut| {
                    tokio::task::spawn_local(fut);
                },
            );

            ACP_CLIENT
                .set(Arc::new(agent_side))
                .expect("install ACP client for child");

            let _agent_io =
                spawn_acp_io_task("agenthub-codex-acp-test-agent-io", agent_io_task)
                    .expect("spawn agent io thread");

            tokio::task::yield_now().await;

            let capabilities = Arc::new(Mutex::new(
                ClientCapabilities::new().fs(FileSystemCapabilities::new().read_text_file(true)),
            ));
            let session_roots = Arc::new(Mutex::new(HashMap::from([(
                session_id.clone(),
                root.clone(),
            )])));
            let fs = AcpFs::new(session_id, capabilities, LocalSpawner::new(), session_roots);

            let patch = "*** Begin Patch\n*** Update File: src/client.rs\n@@\n-fn old() {}\n+fn new() {}\n*** End Patch";
            let argv = vec!["apply_patch".to_string(), patch.to_string()];
            let result =
                codex_apply_patch::maybe_parse_apply_patch_verified(&argv, &root, &fs);

            assert!(
                matches!(result, codex_apply_patch::MaybeApplyPatchVerified::Body(_)),
                "expected verified patch body, got {result:?}"
            );
            drop(temp_dir);
        }));
    }

    #[derive(Clone)]
    struct TestClient {
        file_contents: Arc<Mutex<HashMap<PathBuf, String>>>,
    }

    impl TestClient {
        fn new() -> Self {
            Self {
                file_contents: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        fn add_file_content(&self, path: PathBuf, content: String) {
            self.file_contents.lock().unwrap().insert(path, content);
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Client for TestClient {
        async fn request_permission(
            &self,
            _arguments: agent_client_protocol_legacy::RequestPermissionRequest,
        ) -> agent_client_protocol_legacy::Result<agent_client_protocol_legacy::RequestPermissionResponse>
        {
            unimplemented!()
        }

        async fn write_text_file(
            &self,
            _arguments: WriteTextFileRequest,
        ) -> agent_client_protocol_legacy::Result<agent_client_protocol_legacy::WriteTextFileResponse> {
            unimplemented!()
        }

        async fn read_text_file(
            &self,
            arguments: ReadTextFileRequest,
        ) -> agent_client_protocol_legacy::Result<ReadTextFileResponse> {
            let contents = self.file_contents.lock().unwrap();
            let content = contents
                .get(&arguments.path)
                .cloned()
                .unwrap_or_else(|| "default content".to_string());
            Ok(ReadTextFileResponse::new(content))
        }

        async fn session_notification(
            &self,
            _args: agent_client_protocol_legacy::SessionNotification,
        ) -> agent_client_protocol_legacy::Result<()> {
            Ok(())
        }

        async fn create_terminal(
            &self,
            _args: agent_client_protocol_legacy::CreateTerminalRequest,
        ) -> agent_client_protocol_legacy::Result<agent_client_protocol_legacy::CreateTerminalResponse> {
            unimplemented!()
        }

        async fn terminal_output(
            &self,
            _args: agent_client_protocol_legacy::TerminalOutputRequest,
        ) -> agent_client_protocol_legacy::Result<agent_client_protocol_legacy::TerminalOutputResponse> {
            unimplemented!()
        }

        async fn kill_terminal(
            &self,
            _args: agent_client_protocol_legacy::KillTerminalRequest,
        ) -> agent_client_protocol_legacy::Result<agent_client_protocol_legacy::KillTerminalResponse> {
            unimplemented!()
        }

        async fn release_terminal(
            &self,
            _args: agent_client_protocol_legacy::ReleaseTerminalRequest,
        ) -> agent_client_protocol_legacy::Result<agent_client_protocol_legacy::ReleaseTerminalResponse> {
            unimplemented!()
        }

        async fn wait_for_terminal_exit(
            &self,
            _args: agent_client_protocol_legacy::WaitForTerminalExitRequest,
        ) -> agent_client_protocol_legacy::Result<
            agent_client_protocol_legacy::WaitForTerminalExitResponse,
        >
        {
            unimplemented!()
        }
    }

    #[derive(Clone)]
    struct TestAgent;

    #[async_trait::async_trait(?Send)]
    impl Agent for TestAgent {
        async fn initialize(
            &self,
            arguments: InitializeRequest,
        ) -> agent_client_protocol_legacy::Result<InitializeResponse> {
            Ok(InitializeResponse::new(arguments.protocol_version)
                .agent_info(Implementation::new("test-agent", "0.0.0").title("Test Agent")))
        }

        async fn authenticate(
            &self,
            _arguments: AuthenticateRequest,
        ) -> agent_client_protocol_legacy::Result<AuthenticateResponse> {
            Ok(AuthenticateResponse::default())
        }

        async fn new_session(
            &self,
            _arguments: NewSessionRequest,
        ) -> agent_client_protocol_legacy::Result<NewSessionResponse> {
            Ok(NewSessionResponse::new(SessionId::new("unused")))
        }

        async fn load_session(
            &self,
            _arguments: LoadSessionRequest,
        ) -> agent_client_protocol_legacy::Result<LoadSessionResponse> {
            Ok(LoadSessionResponse::new())
        }

        async fn set_session_mode(
            &self,
            _arguments: SetSessionModeRequest,
        ) -> agent_client_protocol_legacy::Result<SetSessionModeResponse> {
            Ok(SetSessionModeResponse::new())
        }

        async fn prompt(
            &self,
            _arguments: PromptRequest,
        ) -> agent_client_protocol_legacy::Result<PromptResponse> {
            Ok(PromptResponse::new(StopReason::EndTurn))
        }

        async fn cancel(
            &self,
            _arguments: agent_client_protocol_legacy::CancelNotification,
        ) -> agent_client_protocol_legacy::Result<()> {
            Ok(())
        }

        async fn set_session_config_option(
            &self,
            _args: SetSessionConfigOptionRequest,
        ) -> agent_client_protocol_legacy::Result<SetSessionConfigOptionResponse> {
            Ok(SetSessionConfigOptionResponse::new(vec![]))
        }
    }
}
