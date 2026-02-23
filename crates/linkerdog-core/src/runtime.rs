use crate::{LinkerdogRuntimeConfig, agent::LinkerdogAgent};
use agent_client_protocol::{AgentSideConnection, Error as AcpError};
use std::io::{Error as IoError, Result as IoResult};
use std::sync::{Arc, OnceLock};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::task::LocalSet;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing_subscriber::EnvFilter;

pub static ACP_CLIENT: OnceLock<Arc<AgentSideConnection>> = OnceLock::new();

fn init_tracing_subscriber() {
    if tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .is_err()
    {
        // already initialized by parent process/test runtime
    }
}

fn map_acp_io_error(err: AcpError) -> IoError {
    IoError::other(format!("ACP I/O error: {err}"))
}

fn set_once_arc<T>(
    slot: &OnceLock<Arc<T>>,
    value: Arc<T>,
    duplicate_message: &'static str,
) -> IoResult<()> {
    slot.set(value)
        .map_err(|_| IoError::other(duplicate_message))
}

async fn run_local_connection<R, W>(
    agent: std::rc::Rc<LinkerdogAgent>,
    incoming: R,
    outgoing: W,
    client_slot: &OnceLock<Arc<AgentSideConnection>>,
) -> IoResult<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let stdin = incoming.compat();
    let stdout = outgoing.compat_write();

    LocalSet::new()
        .run_until(async move {
            let (client, io_task) = AgentSideConnection::new(agent, stdout, stdin, |fut| {
                tokio::task::spawn_local(fut);
            });

            set_once_arc(client_slot, Arc::new(client), "ACP client already set")?;
            io_task.await.map_err(map_acp_io_error)
        })
        .await
}

pub async fn run_main(config: LinkerdogRuntimeConfig) -> IoResult<()> {
    init_tracing_subscriber();
    let agent = std::rc::Rc::new(LinkerdogAgent::new(config));
    run_local_connection(agent, tokio::io::stdin(), tokio::io::stdout(), &ACP_CLIENT).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_once_arc_rejects_second_value() {
        let slot: OnceLock<Arc<u8>> = OnceLock::new();
        set_once_arc(&slot, Arc::new(1_u8), "duplicate").expect("first set");

        let err =
            set_once_arc(&slot, Arc::new(2_u8), "duplicate").expect_err("second set should fail");
        assert!(err.to_string().contains("duplicate"));
        assert_eq!(**slot.get().expect("slot value"), 1_u8);
    }

    #[test]
    fn map_acp_io_error_keeps_context() {
        let io_err = map_acp_io_error(AcpError::invalid_params().data("bad-request"));
        assert!(io_err.to_string().contains("ACP I/O error"));
        assert!(io_err.to_string().contains("bad-request"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_local_connection_with_empty_streams_finishes_and_sets_client() {
        let slot: OnceLock<Arc<AgentSideConnection>> = OnceLock::new();
        let agent = std::rc::Rc::new(LinkerdogAgent::new(LinkerdogRuntimeConfig::default()));

        run_local_connection(agent, tokio::io::empty(), tokio::io::sink(), &slot)
            .await
            .expect("run local connection");
        assert!(slot.get().is_some());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_local_connection_rejects_duplicate_client_slot() {
        let slot: OnceLock<Arc<AgentSideConnection>> = OnceLock::new();
        let first = std::rc::Rc::new(LinkerdogAgent::new(LinkerdogRuntimeConfig::default()));
        run_local_connection(first, tokio::io::empty(), tokio::io::sink(), &slot)
            .await
            .expect("first run");

        let second = std::rc::Rc::new(LinkerdogAgent::new(LinkerdogRuntimeConfig::default()));
        let err = run_local_connection(second, tokio::io::empty(), tokio::io::sink(), &slot)
            .await
            .expect_err("duplicate run should fail");
        assert!(err.to_string().contains("ACP client already set"));
    }
}
