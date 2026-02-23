use crate::{CliArgs, LinkerdogRuntimeConfig, agent::LinkerdogAgent};
use agent_client_protocol::AgentSideConnection;
use anyhow::Result;
use clap::Parser;
use std::io::Result as IoResult;
use std::sync::{Arc, OnceLock};
use tokio::task::LocalSet;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing_subscriber::EnvFilter;

pub static ACP_CLIENT: OnceLock<Arc<AgentSideConnection>> = OnceLock::new();

pub async fn run_main(config: LinkerdogRuntimeConfig) -> IoResult<()> {
    if tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .is_err()
    {
        // already initialized by parent process/test runtime
    }

    let agent = std::rc::Rc::new(LinkerdogAgent::new(config));

    let stdin = tokio::io::stdin().compat();
    let stdout = tokio::io::stdout().compat_write();

    LocalSet::new()
        .run_until(async move {
            let (client, io_task) = AgentSideConnection::new(agent, stdout, stdin, |fut| {
                tokio::task::spawn_local(fut);
            });

            if ACP_CLIENT.set(Arc::new(client)).is_err() {
                return Err(std::io::Error::other("ACP client already set"));
            }

            io_task
                .await
                .map_err(|e| std::io::Error::other(format!("ACP I/O error: {e}")))
        })
        .await?;

    Ok(())
}

pub async fn run_from_args<I>(args: I) -> Result<()>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    let normalized = crate::normalize_cli_args(args);
    let cli = CliArgs::parse_from(normalized);
    let config = LinkerdogRuntimeConfig::from_raw_overrides(&cli.raw_overrides)
        .map_err(anyhow::Error::msg)?;
    run_main(config).await.map_err(anyhow::Error::from)
}
