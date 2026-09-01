use std::collections::{HashMap, VecDeque};
use std::error::Error as StdError;
use std::fmt;
use std::io::{Error as IoError, ErrorKind, Result as IoResult};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use codex_app_server_protocol::{
    ClientInfo, ClientNotification, ClientRequest, InitializeCapabilities, InitializeParams,
    InitializeResponse, JSONRPCError, JSONRPCErrorError, JSONRPCMessage, JSONRPCNotification,
    JSONRPCRequest, JSONRPCResponse, RequestId, Result as JsonRpcResult, ServerNotification,
    ServerRequest,
};
use serde::de::DeserializeOwned;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, Lines};
use tokio::process::{Child, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tracing::warn;

pub const SUPPORTED_CODEX_VERSION: &str = "0.150.1";

const APP_SERVER_CHANNEL_CAPACITY: usize = 128;
const VERSION_CHECK_TIMEOUT: Duration = Duration::from_secs(10);
const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(15);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const INITIALIZE_REQUEST_ID: &str = "initialize";

pub type RequestResult = Result<JsonRpcResult, JSONRPCErrorError>;

#[derive(Debug, Clone)]
pub struct CodexRuntime {
    inner: Arc<CodexRuntimeInner>,
}

#[derive(Debug)]
struct CodexRuntimeInner {
    binary: PathBuf,
    config_overrides: Vec<String>,
    enable_collab: bool,
}

impl CodexRuntime {
    pub async fn resolve(
        binary: impl AsRef<Path>,
        config_overrides: Vec<String>,
        enable_collab: bool,
    ) -> IoResult<Self> {
        let binary = resolve_executable(binary.as_ref())?;
        validate_codex_version(&binary).await?;
        Ok(Self {
            inner: Arc::new(CodexRuntimeInner {
                binary,
                config_overrides,
                enable_collab,
            }),
        })
    }

    pub fn binary(&self) -> &Path {
        &self.inner.binary
    }

    fn app_server_command(&self, cwd: &Path) -> Command {
        // Keep the ACP worker environment so service-managed proxy and provider
        // credentials cross the explicit process boundary unchanged.
        let mut command = Command::new(&self.inner.binary);
        command.arg("app-server");
        for value in &self.inner.config_overrides {
            command.arg("-c").arg(value);
        }
        if self.inner.enable_collab {
            command.arg("--enable").arg("collab");
        }
        command
            .arg("--stdio")
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        command
    }

    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self {
            inner: Arc::new(CodexRuntimeInner {
                binary: PathBuf::from("codex"),
                config_overrides: Vec::new(),
                enable_collab: false,
            }),
        }
    }
}

fn resolve_executable(binary: &Path) -> IoResult<PathBuf> {
    if binary.as_os_str().is_empty() {
        return Err(IoError::new(
            ErrorKind::InvalidInput,
            "Codex runtime binary must not be empty",
        ));
    }

    if binary.is_absolute() || binary.components().count() > 1 {
        let absolute = if binary.is_absolute() {
            binary.to_path_buf()
        } else {
            std::env::current_dir()?.join(binary)
        };
        return canonical_executable(&absolute).map_err(|err| {
            IoError::new(
                err.kind(),
                format!(
                    "official Codex {SUPPORTED_CODEX_VERSION} executable `{}` could not be used: {err}",
                    absolute.display()
                ),
            )
        });
    }

    let path = std::env::var_os("PATH").ok_or_else(|| {
        IoError::new(
            ErrorKind::NotFound,
            format!(
                "official Codex {SUPPORTED_CODEX_VERSION} is required, but PATH is not set; configure [codex_acp].runtime_binary with an absolute path"
            ),
        )
    })?;
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(binary);
        if let Ok(path) = canonical_executable(&candidate) {
            return Ok(path);
        }
        if !std::env::consts::EXE_SUFFIX.is_empty() {
            let candidate = directory.join(format!(
                "{}{}",
                binary.to_string_lossy(),
                std::env::consts::EXE_SUFFIX
            ));
            if let Ok(path) = canonical_executable(&candidate) {
                return Ok(path);
            }
        }
    }

    Err(IoError::new(
        ErrorKind::NotFound,
        format!(
            "official Codex {SUPPORTED_CODEX_VERSION} executable `{}` was not found; install Codex or configure [codex_acp].runtime_binary with an absolute path",
            binary.display()
        ),
    ))
}

fn canonical_executable(path: &Path) -> IoResult<PathBuf> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(IoError::new(
            ErrorKind::InvalidInput,
            format!("Codex runtime path `{}` is not a file", path.display()),
        ));
    }
    std::fs::canonicalize(path)
}

async fn validate_codex_version(binary: &Path) -> IoResult<()> {
    let mut command = Command::new(binary);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = timeout(VERSION_CHECK_TIMEOUT, command.output())
        .await
        .map_err(|_| {
            IoError::new(
                ErrorKind::TimedOut,
                format!(
                    "timed out checking Codex runtime version from `{}`",
                    binary.display()
                ),
            )
        })?
        .map_err(|err| {
            IoError::new(
                err.kind(),
                format!(
                    "failed to execute Codex runtime `{}`: {err}",
                    binary.display()
                ),
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(IoError::other(format!(
            "Codex runtime `{}` failed version preflight: {}",
            binary.display(),
            stderr.trim()
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(version) = parse_codex_version(&stdout) else {
        return Err(IoError::new(
            ErrorKind::InvalidData,
            format!(
                "Codex runtime `{}` returned an unrecognized version string: {}",
                binary.display(),
                stdout.trim()
            ),
        ));
    };
    if version != SUPPORTED_CODEX_VERSION {
        return Err(IoError::new(
            ErrorKind::Unsupported,
            format!(
                "unsupported Codex runtime version {version}; AgentHub requires {SUPPORTED_CODEX_VERSION}"
            ),
        ));
    }
    Ok(())
}

fn parse_codex_version(output: &str) -> Option<&str> {
    output
        .split_whitespace()
        .find(|part| part.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
}

#[derive(Debug)]
pub enum TypedRequestError {
    Transport {
        method: String,
        source: IoError,
    },
    Server {
        method: String,
        source: JSONRPCErrorError,
    },
    Deserialize {
        method: String,
        source: serde_json::Error,
    },
}

impl fmt::Display for TypedRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport { method, source } => {
                write!(formatter, "{method} transport error: {source}")
            }
            Self::Server { method, source } => {
                write!(
                    formatter,
                    "{method} failed: {} (code {})",
                    source.message, source.code
                )?;
                if let Some(data) = source.data.as_ref() {
                    write!(formatter, ", data: {data}")?;
                }
                Ok(())
            }
            Self::Deserialize { method, source } => {
                write!(formatter, "{method} response decode error: {source}")
            }
        }
    }
}

impl StdError for TypedRequestError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Transport { source, .. } => Some(source),
            Self::Server { .. } => None,
            Self::Deserialize { source, .. } => Some(source),
        }
    }
}

#[derive(Debug, Clone)]
pub enum StdioServerEvent {
    ServerNotification(Box<ServerNotification>),
    ServerRequest(Box<ServerRequest>),
    Disconnected { message: String },
}

enum ClientCommand {
    Request {
        request: Box<JSONRPCRequest>,
        response_tx: oneshot::Sender<IoResult<RequestResult>>,
    },
    ResolveServerRequest {
        request_id: RequestId,
        result: JsonRpcResult,
        response_tx: oneshot::Sender<IoResult<()>>,
    },
    RejectServerRequest {
        request_id: RequestId,
        error: JSONRPCErrorError,
        response_tx: oneshot::Sender<IoResult<()>>,
    },
    Shutdown {
        response_tx: oneshot::Sender<IoResult<()>>,
    },
}

pub struct StdioAppServerClient {
    command_tx: mpsc::Sender<ClientCommand>,
    event_rx: mpsc::UnboundedReceiver<StdioServerEvent>,
    pending_events: VecDeque<StdioServerEvent>,
    worker_handle: JoinHandle<()>,
}

#[derive(Clone)]
pub struct StdioAppServerRequestHandle {
    command_tx: mpsc::Sender<ClientCommand>,
}

impl StdioAppServerClient {
    pub async fn start(runtime: &CodexRuntime, cwd: &Path) -> IoResult<Self> {
        let mut child = runtime.app_server_command(cwd).spawn().map_err(|err| {
            IoError::new(
                err.kind(),
                format!(
                    "failed to start `{}` app-server: {err}",
                    runtime.binary().display()
                ),
            )
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| IoError::other("Codex app-server child did not expose piped stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| IoError::other("Codex app-server child did not expose piped stdout"))?;
        let mut writer = BufWriter::new(stdin);
        let mut lines = BufReader::new(stdout).lines();
        let pending_events = match initialize_connection(&mut writer, &mut lines).await {
            Ok(events) => events,
            Err(err) => {
                drop(writer);
                drop(child.start_kill());
                drop(child.wait().await);
                return Err(err);
            }
        };

        let (command_tx, command_rx) = mpsc::channel(APP_SERVER_CHANNEL_CAPACITY);
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let worker_handle = tokio::spawn(run_worker(child, writer, lines, command_rx, event_tx));
        Ok(Self {
            command_tx,
            event_rx,
            pending_events: pending_events.into(),
            worker_handle,
        })
    }

    pub fn request_handle(&self) -> StdioAppServerRequestHandle {
        StdioAppServerRequestHandle {
            command_tx: self.command_tx.clone(),
        }
    }

    pub async fn request_typed<T>(&self, request: ClientRequest) -> Result<T, TypedRequestError>
    where
        T: DeserializeOwned,
    {
        self.request_handle().request_typed(request).await
    }

    pub async fn resolve_server_request(
        &self,
        request_id: RequestId,
        result: JsonRpcResult,
    ) -> IoResult<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(ClientCommand::ResolveServerRequest {
                request_id,
                result,
                response_tx,
            })
            .await
            .map_err(|_| closed_channel_error("resolve"))?;
        response_rx
            .await
            .map_err(|_| closed_channel_error("resolve response"))?
    }

    pub async fn reject_server_request(
        &self,
        request_id: RequestId,
        error: JSONRPCErrorError,
    ) -> IoResult<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(ClientCommand::RejectServerRequest {
                request_id,
                error,
                response_tx,
            })
            .await
            .map_err(|_| closed_channel_error("reject"))?;
        response_rx
            .await
            .map_err(|_| closed_channel_error("reject response"))?
    }

    pub async fn next_event(&mut self) -> Option<StdioServerEvent> {
        if let Some(event) = self.pending_events.pop_front() {
            return Some(event);
        }
        self.event_rx.recv().await
    }

    pub async fn shutdown(self) -> IoResult<()> {
        let Self {
            command_tx,
            event_rx,
            pending_events: _,
            worker_handle,
        } = self;
        drop(event_rx);
        let mut worker_handle = worker_handle;
        let (response_tx, response_rx) = oneshot::channel();
        if command_tx
            .send(ClientCommand::Shutdown { response_tx })
            .await
            .is_ok()
            && let Ok(Ok(result)) = timeout(SHUTDOWN_TIMEOUT, response_rx).await
        {
            result?;
        }
        if timeout(SHUTDOWN_TIMEOUT, &mut worker_handle).await.is_err() {
            worker_handle.abort();
            drop(worker_handle.await);
        }
        Ok(())
    }
}

impl StdioAppServerRequestHandle {
    async fn request(&self, request: ClientRequest) -> IoResult<RequestResult> {
        let request = jsonrpc_request_from_client_request(request)?;
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(ClientCommand::Request {
                request: Box::new(request),
                response_tx,
            })
            .await
            .map_err(|_| closed_channel_error("request"))?;
        response_rx
            .await
            .map_err(|_| closed_channel_error("request response"))?
    }

    pub async fn request_typed<T>(&self, request: ClientRequest) -> Result<T, TypedRequestError>
    where
        T: DeserializeOwned,
    {
        let method = request.method_name().to_string();
        let response =
            self.request(request)
                .await
                .map_err(|source| TypedRequestError::Transport {
                    method: method.clone(),
                    source,
                })?;
        let result = response.map_err(|source| TypedRequestError::Server {
            method: method.clone(),
            source,
        })?;
        serde_json::from_value(result)
            .map_err(|source| TypedRequestError::Deserialize { method, source })
    }
}

async fn initialize_connection(
    writer: &mut BufWriter<tokio::process::ChildStdin>,
    lines: &mut Lines<BufReader<ChildStdout>>,
) -> IoResult<Vec<StdioServerEvent>> {
    let request_id = RequestId::String(INITIALIZE_REQUEST_ID.to_string());
    let request = ClientRequest::Initialize {
        request_id: request_id.clone(),
        params: InitializeParams {
            client_info: ClientInfo {
                name: "agenthub-codex-acp".to_string(),
                title: Some("AgentHub Codex ACP".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: Some(InitializeCapabilities {
                experimental_api: true,
                request_attestation: false,
                extensions: None,
                opt_out_notification_methods: None,
                mcp_server_openai_form_elicitation: false,
            }),
        },
    };
    write_message(
        writer,
        &JSONRPCMessage::Request(jsonrpc_request_from_client_request(request)?),
    )
    .await?;

    let mut pending_events = Vec::new();
    let response = timeout(INITIALIZE_TIMEOUT, async {
        loop {
            let message = read_message(lines).await?.ok_or_else(|| {
                IoError::new(
                    ErrorKind::UnexpectedEof,
                    "Codex app-server closed during initialize",
                )
            })?;
            match message {
                JSONRPCMessage::Response(response) if response.id == request_id => {
                    break Ok(response);
                }
                JSONRPCMessage::Error(error) if error.id == request_id => {
                    break Err(IoError::other(format!(
                        "Codex app-server rejected initialize: {}",
                        error.error.message
                    )));
                }
                JSONRPCMessage::Notification(notification) => {
                    if let Some(event) = event_from_notification(notification) {
                        pending_events.push(event);
                    }
                }
                JSONRPCMessage::Request(request) => {
                    match ServerRequest::try_from(request.clone()) {
                        Ok(request) => {
                            pending_events.push(StdioServerEvent::ServerRequest(Box::new(request)));
                        }
                        Err(err) => {
                            write_message(
                                writer,
                                &JSONRPCMessage::Error(JSONRPCError {
                                    error: JSONRPCErrorError {
                                        code: -32601,
                                        message: format!(
                                            "unsupported Codex app-server request `{}`: {err}",
                                            request.method
                                        ),
                                        data: None,
                                    },
                                    id: request.id,
                                }),
                            )
                            .await?;
                        }
                    }
                }
                JSONRPCMessage::Response(_) | JSONRPCMessage::Error(_) => {}
            }
        }
    })
    .await
    .map_err(|_| {
        IoError::new(
            ErrorKind::TimedOut,
            "timed out waiting for Codex app-server initialize response",
        )
    })??;

    validate_initialize_response(response)?;
    write_message(
        writer,
        &JSONRPCMessage::Notification(jsonrpc_notification_from_client_notification(
            ClientNotification::Initialized,
        )?),
    )
    .await?;
    Ok(pending_events)
}

fn validate_initialize_response(response: JSONRPCResponse) -> IoResult<()> {
    let response: InitializeResponse = serde_json::from_value(response.result).map_err(|err| {
        IoError::new(
            ErrorKind::InvalidData,
            format!("invalid Codex app-server initialize response: {err}"),
        )
    })?;
    let version = response
        .user_agent
        .split_once('/')
        .and_then(|(_, suffix)| suffix.split_whitespace().next())
        .ok_or_else(|| {
            IoError::new(
                ErrorKind::InvalidData,
                format!(
                    "Codex app-server returned an unrecognized user agent: {}",
                    response.user_agent
                ),
            )
        })?;
    if version != SUPPORTED_CODEX_VERSION {
        return Err(IoError::new(
            ErrorKind::Unsupported,
            format!(
                "Codex app-server version {version} does not match supported version {SUPPORTED_CODEX_VERSION}"
            ),
        ));
    }
    Ok(())
}

async fn run_worker(
    mut child: Child,
    writer: BufWriter<tokio::process::ChildStdin>,
    mut lines: Lines<BufReader<ChildStdout>>,
    mut command_rx: mpsc::Receiver<ClientCommand>,
    event_tx: mpsc::UnboundedSender<StdioServerEvent>,
) {
    let mut pending_requests =
        HashMap::<RequestId, oneshot::Sender<IoResult<RequestResult>>>::new();
    let mut disconnect_message = "Codex app-server worker stopped".to_string();
    let mut graceful_shutdown = false;
    let mut writer = Some(writer);

    loop {
        tokio::select! {
            command = command_rx.recv() => {
                let Some(command) = command else {
                    graceful_shutdown = true;
                    drop(stop_child(&mut child, &mut writer).await);
                    break;
                };
                match command {
                    ClientCommand::Request { request, response_tx } => {
                        let request_id = request.id.clone();
                        if pending_requests.contains_key(&request_id) {
                            drop(response_tx.send(Err(IoError::new(
                                ErrorKind::InvalidInput,
                                format!("duplicate Codex app-server request id `{request_id}`"),
                            ))));
                            continue;
                        }
                        pending_requests.insert(request_id.clone(), response_tx);
                        if let Err(err) = write_message(
                            writer.as_mut().expect("app-server stdin is available"),
                            &JSONRPCMessage::Request(*request),
                        ).await {
                            if let Some(response_tx) = pending_requests.remove(&request_id) {
                                drop(response_tx.send(Err(IoError::new(
                                    err.kind(),
                                    err.to_string(),
                                ))));
                            }
                            disconnect_message = err.to_string();
                            break;
                        }
                    }
                    ClientCommand::ResolveServerRequest { request_id, result, response_tx } => {
                        let result = write_message(
                            writer.as_mut().expect("app-server stdin is available"),
                            &JSONRPCMessage::Response(JSONRPCResponse { id: request_id, result }),
                        ).await;
                        drop(response_tx.send(result));
                    }
                    ClientCommand::RejectServerRequest { request_id, error, response_tx } => {
                        let result = write_message(
                            writer.as_mut().expect("app-server stdin is available"),
                            &JSONRPCMessage::Error(JSONRPCError { error, id: request_id }),
                        ).await;
                        drop(response_tx.send(result));
                    }
                    ClientCommand::Shutdown { response_tx } => {
                        graceful_shutdown = true;
                        let result = stop_child(&mut child, &mut writer).await;
                        drop(response_tx.send(result));
                        break;
                    }
                }
            }
            message = read_message(&mut lines) => {
                match message {
                    Ok(Some(JSONRPCMessage::Response(response))) => {
                        if let Some(response_tx) = pending_requests.remove(&response.id) {
                            drop(response_tx.send(Ok(Ok(response.result))));
                        } else {
                            warn!(request_id = %response.id, "ignoring unmatched Codex app-server response");
                        }
                    }
                    Ok(Some(JSONRPCMessage::Error(error))) => {
                        if let Some(response_tx) = pending_requests.remove(&error.id) {
                            drop(response_tx.send(Ok(Err(error.error))));
                        } else {
                            warn!(request_id = %error.id, "ignoring unmatched Codex app-server error response");
                        }
                    }
                    Ok(Some(JSONRPCMessage::Notification(notification))) => {
                        if let Some(event) = event_from_notification(notification)
                            && event_tx.send(event).is_err()
                        {
                            graceful_shutdown = true;
                            drop(stop_child(&mut child, &mut writer).await);
                            break;
                        }
                    }
                    Ok(Some(JSONRPCMessage::Request(request))) => {
                        let request_id = request.id.clone();
                        let method = request.method.clone();
                        match ServerRequest::try_from(request) {
                            Ok(request) => {
                                if event_tx
                                    .send(StdioServerEvent::ServerRequest(Box::new(request)))
                                    .is_err()
                                {
                                    graceful_shutdown = true;
                                    drop(stop_child(&mut child, &mut writer).await);
                                    break;
                                }
                            }
                            Err(err) => {
                                let result = write_message(
                                    writer.as_mut().expect("app-server stdin is available"),
                                    &JSONRPCMessage::Error(JSONRPCError {
                                        error: JSONRPCErrorError {
                                            code: -32601,
                                            message: format!(
                                                "unsupported Codex app-server request `{method}`: {err}"
                                            ),
                                            data: None,
                                        },
                                        id: request_id,
                                    }),
                                ).await;
                                if let Err(err) = result {
                                    disconnect_message = err.to_string();
                                    break;
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        disconnect_message = "Codex app-server closed stdout".to_string();
                        break;
                    }
                    Err(err) => {
                        disconnect_message = err.to_string();
                        break;
                    }
                }
            }
        }
    }

    for (_, response_tx) in pending_requests {
        drop(response_tx.send(Err(IoError::new(
            ErrorKind::BrokenPipe,
            disconnect_message.clone(),
        ))));
    }
    if !graceful_shutdown {
        drop(writer.take());
        drop(event_tx.send(StdioServerEvent::Disconnected {
            message: disconnect_message,
        }));
        drop(child.start_kill());
        drop(child.wait().await);
    }
}

async fn stop_child(
    child: &mut Child,
    writer: &mut Option<BufWriter<tokio::process::ChildStdin>>,
) -> IoResult<()> {
    if let Some(mut writer) = writer.take() {
        drop(writer.shutdown().await);
        drop(writer);
    }
    match timeout(SHUTDOWN_TIMEOUT, child.wait()).await {
        Ok(result) => result.map(|_| ()),
        Err(_) => {
            child.start_kill()?;
            child.wait().await.map(|_| ())
        }
    }
}

async fn read_message(
    lines: &mut Lines<BufReader<ChildStdout>>,
) -> IoResult<Option<JSONRPCMessage>> {
    loop {
        let Some(line) = lines.next_line().await? else {
            return Ok(None);
        };
        if line.trim().is_empty() {
            continue;
        }
        let message = serde_json::from_str(&line).map_err(|err| {
            IoError::new(
                ErrorKind::InvalidData,
                format!("Codex app-server emitted invalid JSONL: {err}"),
            )
        })?;
        return Ok(Some(message));
    }
}

async fn write_message(
    writer: &mut BufWriter<tokio::process::ChildStdin>,
    message: &JSONRPCMessage,
) -> IoResult<()> {
    let payload = serde_json::to_vec(message).map_err(IoError::other)?;
    writer.write_all(&payload).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await
}

fn event_from_notification(notification: JSONRPCNotification) -> Option<StdioServerEvent> {
    ServerNotification::try_from(notification)
        .map(|notification| StdioServerEvent::ServerNotification(Box::new(notification)))
        .ok()
}

fn jsonrpc_request_from_client_request(request: ClientRequest) -> IoResult<JSONRPCRequest> {
    serde_json::to_value(request)
        .and_then(serde_json::from_value)
        .map_err(IoError::other)
}

fn jsonrpc_notification_from_client_notification(
    notification: ClientNotification,
) -> IoResult<JSONRPCNotification> {
    serde_json::to_value(notification)
        .and_then(serde_json::from_value)
        .map_err(IoError::other)
}

fn closed_channel_error(operation: &str) -> IoError {
    IoError::new(
        ErrorKind::BrokenPipe,
        format!("Codex app-server {operation} channel is closed"),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        CodexRuntime, SUPPORTED_CODEX_VERSION, TypedRequestError, parse_codex_version,
        validate_initialize_response,
    };
    #[cfg(unix)]
    use super::{StdioAppServerClient, StdioServerEvent};
    #[cfg(unix)]
    use codex_app_server_protocol::{
        ClientRequest, JSONRPCErrorError, JSONRPCResponse, RequestId, ServerNotification,
        ServerRequest,
    };
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::path::{Path, PathBuf};
    #[cfg(unix)]
    use std::time::Duration;

    #[cfg(unix)]
    fn write_executable(path: &Path, script: &str) {
        std::fs::write(path, script).expect("write fake Codex runtime");
        let mut permissions = std::fs::metadata(path)
            .expect("read fake runtime metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("make fake runtime executable");
    }

    #[cfg(unix)]
    async fn resolve_fake_runtime(
        temp_dir: &tempfile::TempDir,
        app_server_body: &str,
    ) -> CodexRuntime {
        let runtime_path = temp_dir.path().join("codex");
        let script = format!(
            r#"#!/bin/sh
set -eu
if [ "${{1:-}}" = "--version" ]; then
  printf '%s\n' 'codex-cli {SUPPORTED_CODEX_VERSION}'
  exit 0
fi
{app_server_body}
"#
        );
        write_executable(&runtime_path, &script);
        CodexRuntime::resolve(runtime_path, Vec::new(), false)
            .await
            .expect("resolve fake Codex runtime")
    }

    #[cfg(unix)]
    fn initialized_app_server(body: &str) -> String {
        format!(
            r#"if [ "${{1:-}}" != "app-server" ]; then
  exit 64
fi
IFS= read -r initialize_request
printf '%s\n' '{{"id":"initialize","result":{{"userAgent":"codex_cli_rs/{SUPPORTED_CODEX_VERSION}","codexHome":"/tmp","platformFamily":"unix","platformOs":"linux"}}}}'
IFS= read -r initialized_notification
{body}
"#
        )
    }

    #[cfg(unix)]
    async fn wait_for_json_lines(path: &Path, expected: usize) -> Vec<serde_json::Value> {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Ok(contents) = std::fs::read_to_string(path) {
                    let lines = contents.lines().collect::<Vec<_>>();
                    if lines.len() >= expected {
                        return lines
                            .into_iter()
                            .map(|line| serde_json::from_str(line).expect("parse recorded JSONL"))
                            .collect();
                    }
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("fake runtime should record complete JSONL responses")
    }

    #[cfg(unix)]
    async fn wait_for_path(path: &Path) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while !path.exists() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("fake runtime should reach synchronization point");
    }

    #[cfg(unix)]
    fn memory_reset(request_id: i64) -> ClientRequest {
        ClientRequest::MemoryReset {
            request_id: RequestId::Integer(request_id),
            params: None,
        }
    }

    #[test]
    fn parses_official_codex_version_output() {
        assert_eq!(
            parse_codex_version("codex-cli 0.150.1\n"),
            Some(SUPPORTED_CODEX_VERSION)
        );
    }

    #[test]
    fn rejects_version_output_without_numeric_token() {
        assert_eq!(parse_codex_version("codex-cli unknown\n"), None);
    }

    #[cfg(unix)]
    #[test]
    fn typed_request_errors_preserve_method_and_source_context() {
        let transport = TypedRequestError::Transport {
            method: "thread/start".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "closed"),
        };
        assert_eq!(
            transport.to_string(),
            "thread/start transport error: closed"
        );
        assert!(std::error::Error::source(&transport).is_some());

        let server = TypedRequestError::Server {
            method: "turn/start".to_string(),
            source: JSONRPCErrorError {
                code: -32000,
                message: "provider rejected request".to_string(),
                data: Some(serde_json::json!({ "retryable": false })),
            },
        };
        assert!(server.to_string().contains("turn/start failed"));
        assert!(server.to_string().contains("code -32000"));
        assert!(server.to_string().contains("retryable"));
        assert!(std::error::Error::source(&server).is_none());

        let decode_source = serde_json::from_value::<u64>(serde_json::json!("not-a-number"))
            .expect_err("string must not decode as u64");
        let deserialize = TypedRequestError::Deserialize {
            method: "memory/reset".to_string(),
            source: decode_source,
        };
        assert!(
            deserialize
                .to_string()
                .contains("memory/reset response decode error")
        );
        assert!(std::error::Error::source(&deserialize).is_some());
    }

    #[cfg(unix)]
    #[test]
    fn initialize_response_validation_rejects_invalid_or_incompatible_metadata() {
        let response = |result| JSONRPCResponse {
            id: RequestId::String("initialize".to_string()),
            result,
        };
        assert!(
            validate_initialize_response(response(serde_json::json!({
                "userAgent": format!("codex_cli_rs/{SUPPORTED_CODEX_VERSION}"),
                "codexHome": "/tmp",
                "platformFamily": "unix",
                "platformOs": "linux"
            })))
            .is_ok()
        );

        let invalid = validate_initialize_response(response(serde_json::json!({})))
            .expect_err("missing initialize fields must fail");
        assert_eq!(invalid.kind(), std::io::ErrorKind::InvalidData);

        let unrecognized = validate_initialize_response(response(serde_json::json!({
            "userAgent": "codex_cli_rs",
            "codexHome": "/tmp",
            "platformFamily": "unix",
            "platformOs": "linux"
        })))
        .expect_err("unversioned user agent must fail");
        assert_eq!(unrecognized.kind(), std::io::ErrorKind::InvalidData);
        assert!(unrecognized.to_string().contains("unrecognized user agent"));

        let incompatible = validate_initialize_response(response(serde_json::json!({
            "userAgent": "codex_cli_rs/0.149.0",
            "codexHome": "/tmp",
            "platformFamily": "unix",
            "platformOs": "linux"
        })))
        .expect_err("incompatible app-server must fail");
        assert_eq!(incompatible.kind(), std::io::ErrorKind::Unsupported);
        assert!(incompatible.to_string().contains("does not match"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn reports_runtime_path_and_version_preflight_failures() {
        let empty = CodexRuntime::resolve(PathBuf::new(), Vec::new(), false)
            .await
            .expect_err("empty runtime path must fail");
        assert_eq!(empty.kind(), std::io::ErrorKind::InvalidInput);

        let directory = tempfile::tempdir().expect("create temp directory");
        let not_a_file = CodexRuntime::resolve(directory.path(), Vec::new(), false)
            .await
            .expect_err("runtime directory must fail");
        assert_eq!(not_a_file.kind(), std::io::ErrorKind::InvalidInput);
        assert!(not_a_file.to_string().contains("is not a file"));

        for (script, kind, message) in [
            (
                "#!/bin/sh\nprintf '%s\\n' 'version failed' >&2\nexit 17\n",
                std::io::ErrorKind::Other,
                "failed version preflight",
            ),
            (
                "#!/bin/sh\nprintf '%s\\n' 'codex-cli unknown'\n",
                std::io::ErrorKind::InvalidData,
                "unrecognized version string",
            ),
        ] {
            let temp_dir = tempfile::tempdir().expect("create temp directory");
            let runtime_path = temp_dir.path().join("codex");
            write_executable(&runtime_path, script);
            let error = CodexRuntime::resolve(runtime_path, Vec::new(), false)
                .await
                .expect_err("invalid version preflight must fail");
            assert_eq!(error.kind(), kind);
            assert!(error.to_string().contains(message));
        }

        let temp_dir = tempfile::tempdir().expect("create temp directory");
        let runtime_path = temp_dir.path().join("codex");
        std::fs::write(
            &runtime_path,
            format!("#!/bin/sh\nprintf '%s\\n' 'codex-cli {SUPPORTED_CODEX_VERSION}'\n"),
        )
        .expect("write non-executable runtime");
        let not_executable = CodexRuntime::resolve(runtime_path, Vec::new(), false)
            .await
            .expect_err("non-executable runtime must fail");
        assert_eq!(not_executable.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(not_executable.to_string().contains("failed to execute"));
    }

    #[tokio::test]
    async fn reports_missing_codex_runtime_with_install_guidance() {
        let temp_dir = tempfile::tempdir().expect("create temp directory");
        let error = CodexRuntime::resolve(temp_dir.path().join("missing-codex"), Vec::new(), false)
            .await
            .expect_err("missing runtime must fail");

        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        assert!(error.to_string().contains("official Codex 0.150.1"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_incompatible_installed_codex_version() {
        let temp_dir = tempfile::tempdir().expect("create temp directory");
        let runtime_path = temp_dir.path().join("codex");
        std::fs::write(
            &runtime_path,
            "#!/bin/sh\nprintf '%s\\n' 'codex-cli 0.149.0'\n",
        )
        .expect("write fake Codex runtime");
        let mut permissions = std::fs::metadata(&runtime_path)
            .expect("read fake runtime metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&runtime_path, permissions).expect("make fake runtime executable");

        let error = CodexRuntime::resolve(&runtime_path, Vec::new(), false)
            .await
            .expect_err("incompatible runtime must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
        assert!(
            error
                .to_string()
                .contains("unsupported Codex runtime version 0.149.0")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn initialization_surfaces_server_error_invalid_json_and_eof() {
        for (body, kind, message) in [
            (
                r#"IFS= read -r initialize_request
printf '%s\n' '{"id":"initialize","error":{"code":-32000,"message":"not ready"}}'
"#,
                std::io::ErrorKind::Other,
                "rejected initialize",
            ),
            (
                r#"IFS= read -r initialize_request
printf '%s\n' '{invalid-json'
"#,
                std::io::ErrorKind::InvalidData,
                "invalid JSONL",
            ),
            (
                r#"IFS= read -r initialize_request
exit 0
"#,
                std::io::ErrorKind::UnexpectedEof,
                "closed during initialize",
            ),
        ] {
            let temp_dir = tempfile::tempdir().expect("create temp directory");
            let runtime = resolve_fake_runtime(&temp_dir, body).await;
            let error = match StdioAppServerClient::start(&runtime, temp_dir.path()).await {
                Ok(_) => panic!("invalid initialize sequence must fail"),
                Err(error) => error,
            };
            assert_eq!(error.kind(), kind);
            assert!(error.to_string().contains(message));
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn initialization_buffers_supported_events_and_rejects_unknown_requests() {
        let temp_dir = tempfile::tempdir().expect("create temp directory");
        let response_path = temp_dir.path().join("initialize-response.jsonl");
        let body = format!(
            r#"IFS= read -r initialize_request
printf '\n'
printf '%s\n' '{{"id":100,"result":{{"ignored":true}}}}'
printf '%s\n' '{{"id":101,"error":{{"code":-32000,"message":"ignored"}}}}'
printf '%s\n' '{{"method":"thread/deleted","params":{{"threadId":"thread-init"}}}}'
printf '%s\n' '{{"id":9,"method":"attestation/generate","params":{{}}}}'
printf '%s\n' '{{"id":10,"method":"unknown/request","params":{{}}}}'
printf '%s\n' '{{"id":"initialize","result":{{"userAgent":"codex_cli_rs/{SUPPORTED_CODEX_VERSION}","codexHome":"/tmp","platformFamily":"unix","platformOs":"linux"}}}}'
IFS= read -r unsupported_response
printf '%s\n' "$unsupported_response" > '{response_path}'
IFS= read -r initialized_notification
while IFS= read -r ignored; do :; done
"#,
            response_path = response_path.display(),
        );
        let runtime = resolve_fake_runtime(&temp_dir, &body).await;
        let mut client = StdioAppServerClient::start(&runtime, temp_dir.path())
            .await
            .expect("start fake app-server");

        let notification = client.next_event().await.expect("buffered notification");
        let StdioServerEvent::ServerNotification(notification) = notification else {
            panic!("expected buffered server notification");
        };
        let ServerNotification::ThreadDeleted(params) = *notification else {
            panic!("expected thread/deleted notification");
        };
        assert_eq!(params.thread_id, "thread-init");

        let request = client.next_event().await.expect("buffered server request");
        let StdioServerEvent::ServerRequest(request) = request else {
            panic!("expected buffered server request");
        };
        assert!(matches!(
            *request,
            ServerRequest::AttestationGenerate { .. }
        ));

        let responses = wait_for_json_lines(&response_path, 1).await;
        assert_eq!(responses[0]["id"], serde_json::json!(10));
        assert_eq!(responses[0]["error"]["code"], serde_json::json!(-32601));
        assert!(
            responses[0]["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("unknown/request"))
        );

        client.shutdown().await.expect("stop fake app-server");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn reports_app_server_spawn_failure_after_successful_preflight() {
        let temp_dir = tempfile::tempdir().expect("create temp directory");
        let runtime = resolve_fake_runtime(
            &temp_dir,
            &initialized_app_server("while IFS= read -r ignored; do :; done\n"),
        )
        .await;
        std::fs::remove_file(runtime.binary()).expect("remove temporary fake runtime");

        let error = match StdioAppServerClient::start(&runtime, temp_dir.path()).await {
            Ok(_) => panic!("removed app-server executable must fail"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        assert!(error.to_string().contains("failed to start"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn correlates_out_of_order_responses_and_maps_typed_errors() {
        let temp_dir = tempfile::tempdir().expect("create temp directory");
        let runtime = resolve_fake_runtime(
            &temp_dir,
            &initialized_app_server(
                r#"IFS= read -r first_request
IFS= read -r second_request
printf '%s\n' '{"id":999,"result":{"ignored":true}}'
printf '%s\n' '{"id":998,"error":{"code":-32001,"message":"ignored"}}'
printf '%s\n' '{"id":22,"result":{"value":2}}'
printf '%s\n' '{"id":21,"error":{"code":-32000,"message":"provider rejected request","data":{"retryable":false}}}'
IFS= read -r third_request
printf '%s\n' '{"id":23,"result":"not-a-number"}'
while IFS= read -r ignored; do :; done
"#,
            ),
        )
        .await;
        let client = StdioAppServerClient::start(&runtime, temp_dir.path())
            .await
            .expect("start fake app-server");
        let first = client.request_handle();
        let second = client.request_handle();

        let (first_result, second_result) = tokio::join!(
            first.request_typed::<serde_json::Value>(memory_reset(21)),
            second.request_typed::<serde_json::Value>(memory_reset(22)),
        );
        let first_error = first_result.expect_err("first request must receive server error");
        let TypedRequestError::Server { method, source } = first_error else {
            panic!("expected typed server error");
        };
        assert_eq!(method, "memory/reset");
        assert_eq!(source.code, -32000);
        assert_eq!(source.data, Some(serde_json::json!({ "retryable": false })));
        assert_eq!(
            second_result.expect("second request must correlate by id"),
            serde_json::json!({ "value": 2 })
        );

        let deserialize = client
            .request_typed::<u64>(memory_reset(23))
            .await
            .expect_err("invalid typed response must fail");
        assert!(matches!(deserialize, TypedRequestError::Deserialize { .. }));

        client.shutdown().await.expect("stop fake app-server");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn forwards_live_events_and_writes_server_request_responses() {
        let temp_dir = tempfile::tempdir().expect("create temp directory");
        let response_path = temp_dir.path().join("live-responses.jsonl");
        let body = initialized_app_server(&format!(
            r#"printf '%s\n' '{{"method":"thread/deleted","params":{{"threadId":"thread-live"}}}}'
printf '%s\n' '{{"id":31,"method":"attestation/generate","params":{{}}}}'
IFS= read -r resolved_response
printf '%s\n' "$resolved_response" > '{response_path}'
printf '%s\n' '{{"id":32,"method":"unknown/request","params":{{}}}}'
IFS= read -r rejected_response
printf '%s\n' "$rejected_response" >> '{response_path}'
while IFS= read -r ignored; do :; done
"#,
            response_path = response_path.display(),
        ));
        let runtime = resolve_fake_runtime(&temp_dir, &body).await;
        let mut client = StdioAppServerClient::start(&runtime, temp_dir.path())
            .await
            .expect("start fake app-server");

        let notification = client.next_event().await.expect("live notification");
        let StdioServerEvent::ServerNotification(notification) = notification else {
            panic!("expected live server notification");
        };
        assert!(matches!(
            *notification,
            ServerNotification::ThreadDeleted(_)
        ));

        let request = client.next_event().await.expect("live server request");
        let StdioServerEvent::ServerRequest(request) = request else {
            panic!("expected live server request");
        };
        assert!(matches!(
            *request,
            ServerRequest::AttestationGenerate { .. }
        ));
        client
            .resolve_server_request(RequestId::Integer(31), serde_json::json!({ "token": "ok" }))
            .await
            .expect("resolve server request");

        let responses = wait_for_json_lines(&response_path, 2).await;
        assert_eq!(responses[0]["id"], serde_json::json!(31));
        assert_eq!(responses[0]["result"], serde_json::json!({ "token": "ok" }));
        assert_eq!(responses[1]["id"], serde_json::json!(32));
        assert_eq!(responses[1]["error"]["code"], serde_json::json!(-32601));

        client.shutdown().await.expect("stop fake app-server");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn disconnect_fails_pending_requests_and_closes_command_channels() {
        let temp_dir = tempfile::tempdir().expect("create temp directory");
        let runtime = resolve_fake_runtime(
            &temp_dir,
            &initialized_app_server(
                r#"IFS= read -r client_request
printf '%s\n' '{invalid-json'
"#,
            ),
        )
        .await;
        let mut client = StdioAppServerClient::start(&runtime, temp_dir.path())
            .await
            .expect("start fake app-server");
        let handle = client.request_handle();

        let error = handle
            .request_typed::<serde_json::Value>(memory_reset(41))
            .await
            .expect_err("disconnect must fail pending request");
        let TypedRequestError::Transport { source, .. } = error else {
            panic!("expected transport error");
        };
        assert_eq!(source.kind(), std::io::ErrorKind::BrokenPipe);
        assert!(source.to_string().contains("invalid JSONL"));

        let event = client.next_event().await.expect("disconnect event");
        let StdioServerEvent::Disconnected { message } = event else {
            panic!("expected disconnect event");
        };
        assert!(message.contains("invalid JSONL"));

        let resolve_error = client
            .resolve_server_request(RequestId::Integer(42), serde_json::json!({}))
            .await
            .expect_err("closed worker must reject responses");
        assert_eq!(resolve_error.kind(), std::io::ErrorKind::BrokenPipe);
        let reject_error = client
            .reject_server_request(
                RequestId::Integer(43),
                JSONRPCErrorError {
                    code: -32601,
                    message: "unsupported".to_string(),
                    data: None,
                },
            )
            .await
            .expect_err("closed worker must reject errors");
        assert_eq!(reject_error.kind(), std::io::ErrorKind::BrokenPipe);

        client.shutdown().await.expect("join disconnected worker");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn duplicate_in_flight_request_ids_are_rejected_without_losing_first_response() {
        let temp_dir = tempfile::tempdir().expect("create temp directory");
        let accepted_path = temp_dir.path().join("first-request-accepted");
        let release_path = temp_dir.path().join("release-first-response");
        let runtime = resolve_fake_runtime(
            &temp_dir,
            &initialized_app_server(&format!(
                r#"IFS= read -r first_request
: > '{accepted_path}'
while [ ! -f '{release_path}' ]; do sleep 0.01; done
printf '%s\n' '{{"id":44,"result":{{"accepted":true}}}}'
while IFS= read -r ignored; do :; done
"#,
                accepted_path = accepted_path.display(),
                release_path = release_path.display(),
            )),
        )
        .await;
        let client = StdioAppServerClient::start(&runtime, temp_dir.path())
            .await
            .expect("start fake app-server");
        let first = client.request_handle();
        let second = client.request_handle();

        let first_task = tokio::spawn(async move { first.request(memory_reset(44)).await });
        wait_for_path(&accepted_path).await;
        let duplicate = second
            .request(memory_reset(44))
            .await
            .expect_err("duplicate id must fail");
        std::fs::write(&release_path, b"release").expect("release first response");
        let first_result = first_task.await.expect("join first request");
        assert_eq!(
            first_result
                .expect("first request transport")
                .expect("first request response"),
            serde_json::json!({ "accepted": true })
        );
        assert_eq!(duplicate.kind(), std::io::ErrorKind::InvalidInput);
        assert!(duplicate.to_string().contains("duplicate"));

        client.shutdown().await.expect("stop fake app-server");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launches_installed_codex_app_server_over_jsonl_stdio() {
        let temp_dir = tempfile::tempdir().expect("create temp directory");
        let runtime_path = temp_dir.path().join("codex");
        let server_response_path = temp_dir.path().join("server-response.json");
        std::fs::write(
            &runtime_path,
            format!(
                r#"#!/bin/sh
set -eu
if [ "${{1:-}}" = "--version" ]; then
  printf '%s\n' 'codex-cli {SUPPORTED_CODEX_VERSION}'
  exit 0
fi
if [ "${{1:-}}" != "app-server" ]; then
  exit 64
fi
if [ -z "${{PATH:-}}" ]; then
  exit 67
fi
case " $* " in
  *" -c model=test-model "*) ;;
  *) exit 65 ;;
esac
case " $* " in
  *" --enable collab "*) ;;
  *) exit 66 ;;
esac
IFS= read -r initialize_request
printf '%s\n' '{{"id":"initialize","result":{{"userAgent":"codex_cli_rs/{SUPPORTED_CODEX_VERSION}","codexHome":"/tmp","platformFamily":"unix","platformOs":"linux"}}}}'
IFS= read -r initialized_notification
IFS= read -r client_request
printf '%s\n' '{{"id":7,"result":{{"ok":true}}}}'
printf '%s\n' '{{"id":9,"method":"attestation/generate","params":{{}}}}'
IFS= read -r server_response
printf '%s\n' "$server_response" > '{server_response_path}'
while IFS= read -r ignored; do :; done
"#,
                server_response_path = server_response_path.display(),
            ),
        )
        .expect("write fake Codex runtime");
        let mut permissions = std::fs::metadata(&runtime_path)
            .expect("read fake runtime metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&runtime_path, permissions).expect("make fake runtime executable");

        let runtime =
            CodexRuntime::resolve(&runtime_path, vec!["model=test-model".to_string()], true)
                .await
                .expect("resolve fake Codex runtime");
        assert_eq!(runtime.binary(), runtime_path.canonicalize().unwrap());

        let mut client = StdioAppServerClient::start(&runtime, temp_dir.path())
            .await
            .expect("start fake app-server");
        let result = client
            .request_handle()
            .request(ClientRequest::MemoryReset {
                request_id: RequestId::Integer(7),
                params: None,
            })
            .await
            .expect("send request")
            .expect("receive successful response");
        assert_eq!(result, serde_json::json!({ "ok": true }));

        let event = client.next_event().await.expect("receive server request");
        let StdioServerEvent::ServerRequest(request) = event else {
            panic!("expected server request");
        };
        assert!(matches!(
            *request,
            ServerRequest::AttestationGenerate { .. }
        ));
        client
            .reject_server_request(
                RequestId::Integer(9),
                JSONRPCErrorError {
                    code: -32601,
                    message: "unsupported by ACP adapter".to_string(),
                    data: None,
                },
            )
            .await
            .expect("reject server request");
        let server_response = tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                if let Ok(response) = std::fs::read_to_string(&server_response_path)
                    && response.contains("\"code\":-32601")
                {
                    break response;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("fake runtime should record the complete server response");
        assert!(server_response.contains("\"id\":9"));
        assert!(server_response.contains("\"code\":-32601"));

        client.shutdown().await.expect("stop fake app-server");
    }
}
