use std::{path::PathBuf, time::Duration};

use agenthub_mcp::{
    McpTransportError,
    protocol::parse_message,
    stdio::{read_message_blocking, write_message_blocking},
};
use clap::Parser;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use crate::{
    internal::client::InternalGrpcMailboxClient,
    loop_credentials::{LOOP_CREDENTIAL_FILE_ENV, LoopCredentialEnvelope},
};

#[derive(Parser)]
#[command(name = "agenthub mcp-proxy")]
struct Options {
    #[arg(long)]
    server_id: String,
}

#[derive(PartialEq, Eq)]
struct CredentialIdentity {
    actor: String,
    run: String,
    activation: String,
    generation: i64,
    target: String,
    ca: Option<String>,
}

impl From<&LoopCredentialEnvelope> for CredentialIdentity {
    fn from(envelope: &LoopCredentialEnvelope) -> Self {
        Self {
            actor: envelope.actor_id.clone(),
            run: envelope.run_id.clone(),
            activation: envelope.activation_id.clone(),
            generation: envelope.generation,
            target: envelope.target.clone(),
            ca: envelope.ca_cert_path.clone(),
        }
    }
}

struct Connection {
    path: PathBuf,
    identity: CredentialIdentity,
    client: InternalGrpcMailboxClient,
}

impl Connection {
    async fn new(path: PathBuf) -> anyhow::Result<Self> {
        let envelope = read_credentials(&path)?;
        let identity = CredentialIdentity::from(&envelope);
        let client = envelope
            .connect()
            .await
            .map_err(|_| anyhow::anyhow!("MCP daemon connection failed"))?;
        Ok(Self {
            path,
            identity,
            client,
        })
    }

    fn refresh(&mut self) -> anyhow::Result<InternalGrpcMailboxClient> {
        let envelope = read_credentials(&self.path)?;
        anyhow::ensure!(
            CredentialIdentity::from(&envelope) == self.identity,
            "MCP activation identity changed; reconnect is required"
        );
        self.client = self.client.with_mcp_access_token(envelope.access_token);
        Ok(self.client.clone())
    }
}

fn read_credentials(path: &std::path::Path) -> anyhow::Result<LoopCredentialEnvelope> {
    LoopCredentialEnvelope::read(path)
        .map_err(|_| anyhow::anyhow!("MCP activation credentials are unavailable or expired"))
}

struct Output {
    message: Value,
    written: oneshot::Sender<Result<(), McpTransportError>>,
}

pub(crate) async fn run_from_args(args: &[String]) -> anyhow::Result<()> {
    let options = match Options::try_parse_from(
        std::iter::once("mcp-proxy".to_owned()).chain(args.iter().cloned()),
    ) {
        Ok(options) => options,
        Err(error) if crate::cli::is_non_error_clap_exit(&error) => {
            error.print()?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    anyhow::ensure!(
        !options.server_id.is_empty() && options.server_id.len() <= 128,
        "invalid MCP server reference"
    );
    let path = std::env::var_os(LOOP_CREDENTIAL_FILE_ENV)
        .ok_or_else(|| anyhow::anyhow!("MCP activation credential file is required"))?;
    let mut connection = Connection::new(PathBuf::from(path)).await?;
    let session_id = connection
        .refresh()?
        .open_mcp_proxy(options.server_id)
        .await?;
    let (input, mut messages) = mpsc::channel(4);
    std::thread::Builder::new()
        .name("mcp-stdin".into())
        .spawn(move || {
            let mut stdin = std::io::BufReader::new(std::io::stdin());
            loop {
                let message = read_message_blocking(&mut stdin);
                let finished = !matches!(message, Ok(Some(_)));
                if input.blocking_send(message).is_err() || finished {
                    break;
                }
            }
        })?;
    let (output, mut writes) = mpsc::channel::<Output>(4);
    std::thread::Builder::new()
        .name("mcp-stdout".into())
        .spawn(move || {
            let mut stdout = std::io::stdout().lock();
            while let Some(write) = writes.blocking_recv() {
                let result = write_message_blocking(&mut stdout, &write.message);
                let failed = result.is_err();
                let _ = write.written.send(result);
                if failed {
                    break;
                }
            }
        })?;
    let outcome = pump(&mut connection, &session_id, &mut messages, output).await;
    if let Ok(client) = connection.refresh() {
        let _ =
            tokio::time::timeout(Duration::from_secs(5), client.close_mcp_proxy(session_id)).await;
    }
    outcome
}

async fn pump(
    connection: &mut Connection,
    session_id: &str,
    messages: &mut mpsc::Receiver<Result<Option<Value>, McpTransportError>>,
    output: mpsc::Sender<Output>,
) -> anyhow::Result<()> {
    let mut exchanges = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            result = exchanges.join_next(), if !exchanges.is_empty() => {
                result.ok_or_else(||anyhow::anyhow!("MCP exchange disappeared"))?
                    .map_err(|_|anyhow::anyhow!("MCP exchange task failed"))??;
            }
            message = messages.recv() => {
                let message = message.ok_or_else(||anyhow::anyhow!("MCP stdin reader stopped"))??;
                let Some(message) = message else {break;};
                anyhow::ensure!(exchanges.len() < 32, "MCP concurrent exchange limit reached");
                // Wait for admission headers, preserving stdin order through initialize/initialized
                // delivery, then receive each response independently so callbacks can make progress.
                let stream = connection.refresh()?.exchange_mcp_proxy(session_id.into(), message.to_string()).await?;
                let output = output.clone();
                exchanges.spawn(async move {forward(stream, output).await});
            }
        }
    }
    while let Some(result) = exchanges.join_next().await {
        result.map_err(|_| anyhow::anyhow!("MCP exchange task failed"))??;
    }
    Ok(())
}

async fn forward(
    mut stream: tonic::Streaming<crate::internal::proto::agenthub::internal::v1::McpProxyFrame>,
    output: mpsc::Sender<Output>,
) -> anyhow::Result<()> {
    while let Some(frame) = stream
        .message()
        .await
        .map_err(|_| anyhow::anyhow!("MCP proxy response stream disconnected"))?
    {
        if !frame.message_json.is_empty() {
            let message = parse_message(frame.message_json.as_bytes())?;
            let (written, receiver) = oneshot::channel();
            tokio::time::timeout(Duration::from_secs(30), async {
                output
                    .send(Output { message, written })
                    .await
                    .map_err(|_| anyhow::anyhow!("MCP stdout writer stopped"))?;
                receiver
                    .await
                    .map_err(|_| anyhow::anyhow!("MCP stdout write did not finish"))??;
                Ok::<_, anyhow::Error>(())
            })
            .await
            .map_err(|_| anyhow::anyhow!("MCP stdout write deadline expired"))??;
        }
        if frame.finished {
            return Ok(());
        }
    }
    anyhow::bail!("MCP exchange ended without its completion marker")
}
